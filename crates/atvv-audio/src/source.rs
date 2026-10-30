// SPDX-License-Identifier: GPL-3.0-or-later

//! PipeWire source thread and bounded PCM queue.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam_queue::ArrayQueue;
use pipewire as pw;
use pw::properties::properties;
use pw::spa::param::audio::{AudioFormat, AudioInfoRaw};
use pw::spa::pod::serialize::PodSerializer;
use pw::spa::pod::{Object, Pod, Value};
use pw::spa::utils::Direction;
use pw::stream::{StreamBox, StreamFlags};
use thiserror::Error;

const READY_TIMEOUT: Duration = Duration::from_secs(5);
const RETRY_DELAY: Duration = Duration::from_millis(500);
const SHUTDOWN_POLL: Duration = Duration::from_millis(100);

/// Configuration for one ATVV PipeWire source.
#[derive(Debug, Clone)]
pub struct AudioSourceConfig {
    pub name: String,
    pub description: String,
    pub sample_rate: u32,
    pub channels: u32,
    /// Maximum number of i16 samples buffered ahead of the RT callback.
    pub queue_capacity: usize,
}

impl AudioSourceConfig {
    fn validate(&self) -> Result<(), AudioSourceError> {
        if self.name.trim().is_empty() {
            return Err(AudioSourceError::InvalidConfig("source name is empty"));
        }
        if self.sample_rate == 0 {
            return Err(AudioSourceError::InvalidConfig("sample rate is zero"));
        }
        if self.channels != 1 {
            return Err(AudioSourceError::InvalidConfig(
                "ATVV sources are mono in M0",
            ));
        }
        if self.queue_capacity == 0 {
            return Err(AudioSourceError::InvalidConfig("queue capacity is zero"));
        }
        Ok(())
    }
}

/// A bounded, lock-free queue of interleaved i16 samples.
#[derive(Debug, Clone)]
pub struct PcmQueue {
    inner: Arc<ArrayQueue<i16>>,
}

impl PcmQueue {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(ArrayQueue::new(capacity.max(1))),
        }
    }

    /// Push samples, dropping the oldest sample when the queue is full.
    pub fn push(&self, samples: &[i16]) {
        for &sample in samples {
            if self.inner.push(sample).is_err() {
                let _ = self.inner.pop();
                let _ = self.inner.push(sample);
            }
        }
    }

    #[must_use]
    pub fn pop(&self) -> Option<i16> {
        self.inner.pop()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// A running PipeWire source. Dropping it stops the PipeWire thread.
#[derive(Debug)]
pub struct AudioSource {
    name: String,
    queue: PcmQueue,
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl AudioSource {
    /// Start the source and wait until the stream is connected.
    pub fn start(config: AudioSourceConfig) -> Result<Self, AudioSourceError> {
        config.validate()?;

        let name = config.name.clone();
        let queue = PcmQueue::new(config.queue_capacity);
        let shutdown = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);

        let thread_queue = queue.clone();
        let thread_shutdown = Arc::clone(&shutdown);
        let thread = thread::Builder::new()
            .name("atvv-pipewire".to_string())
            .spawn(move || supervisor(config, thread_queue, thread_shutdown, ready_tx))
            .map_err(|error| AudioSourceError::Thread(error.to_string()))?;

        match ready_rx.recv_timeout(READY_TIMEOUT) {
            Ok(Ok(())) => Ok(Self {
                name,
                queue,
                shutdown,
                thread: Some(thread),
            }),
            Ok(Err(error)) => {
                let _ = thread.join();
                Err(AudioSourceError::Startup(error))
            }
            Err(_) => {
                shutdown.store(true, Ordering::Relaxed);
                let _ = thread.join();
                Err(AudioSourceError::StartupTimeout)
            }
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn push(&self, samples: &[i16]) {
        self.queue.push(samples);
    }

    #[must_use]
    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    /// Stop the source and join the PipeWire thread.
    pub fn stop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for AudioSource {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Debug, Error)]
pub enum AudioSourceError {
    #[error("invalid source config: {0}")]
    InvalidConfig(&'static str),
    #[error("failed to spawn PipeWire thread: {0}")]
    Thread(String),
    #[error("timed out waiting for the PipeWire stream")]
    StartupTimeout,
    #[error("PipeWire source failed to start: {0}")]
    Startup(String),
}

fn supervisor(
    config: AudioSourceConfig,
    queue: PcmQueue,
    shutdown: Arc<AtomicBool>,
    ready: SyncSender<Result<(), String>>,
) {
    pw::init();
    let mut first = true;

    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        let result = run_session(
            &config,
            &queue,
            &shutdown,
            if first { Some(&ready) } else { None },
        );
        match result {
            Ok(()) => {}
            Err(error) => {
                if first {
                    let _ = ready.send(Err(error));
                    return;
                }
                eprintln!("atvv-audio: PipeWire source restart: {error}");
            }
        }

        first = false;
        if shutdown.load(Ordering::Relaxed) {
            break;
        }
        thread::sleep(RETRY_DELAY);
    }
}

fn run_session(
    config: &AudioSourceConfig,
    queue: &PcmQueue,
    shutdown: &Arc<AtomicBool>,
    ready: Option<&SyncSender<Result<(), String>>>,
) -> Result<(), String> {
    let mainloop = pw::main_loop::MainLoopRc::new(None).map_err(|error| error.to_string())?;
    let context =
        pw::context::ContextRc::new(&mainloop, None).map_err(|error| error.to_string())?;
    let core = context
        .connect_rc(None)
        .map_err(|error| error.to_string())?;
    let registry = core.get_registry_rc().map_err(|error| error.to_string())?;

    let failed = Rc::new(Cell::new(None::<String>));
    let _core_listener = core
        .add_listener_local()
        .error({
            let mainloop = mainloop.clone();
            let failed = Rc::clone(&failed);
            move |_id, _seq, res, message| {
                failed.set(Some(format!("core error {res}: {message}")));
                mainloop.quit();
            }
        })
        .register();

    let props = properties! {
        *pw::keys::MEDIA_TYPE => "Audio",
        *pw::keys::MEDIA_CATEGORY => "Capture",
        *pw::keys::MEDIA_CLASS => "Audio/Source",
        *pw::keys::MEDIA_ROLE => "Communication",
        *pw::keys::NODE_NAME => config.name.clone(),
        *pw::keys::NODE_DESCRIPTION => config.description.clone(),
        *pw::keys::AUDIO_CHANNELS => config.channels.to_string(),
        "audio.rate" => config.sample_rate.to_string(),
        "node.virtual" => "true",
        // Keep the node alive between consumers; the graph clock schedules it
        // while a consumer is linked.
        "node.pause-on-idle" => "false",
        "device.description" => config.description.clone(),
        "device.class" => "sound",
        "device.icon-name" => "audio-input-microphone",
    };

    let stream = StreamBox::new(&core, &config.name, props).map_err(|error| error.to_string())?;
    let our_node_id = Rc::new(Cell::new(u32::MAX));

    let _registry_listener = registry
        .add_listener_local()
        .global_remove({
            let mainloop = mainloop.clone();
            let our_node_id = Rc::clone(&our_node_id);
            move |id| {
                if id == our_node_id.get() {
                    mainloop.quit();
                }
            }
        })
        .register();

    let queue = queue.clone();
    let _listener = stream
        .add_local_listener_with_user_data(())
        .state_changed({
            let mainloop = mainloop.clone();
            let failed = Rc::clone(&failed);
            let our_node_id = Rc::clone(&our_node_id);
            move |stream, (), _old, new| match new {
                pw::stream::StreamState::Paused | pw::stream::StreamState::Streaming => {
                    our_node_id.set(stream.node_id());
                }
                pw::stream::StreamState::Error(error) => {
                    failed.set(Some(error));
                    mainloop.quit();
                }
                _ => {}
            }
        })
        .process(move |stream, ()| {
            let Some(mut buffer) = stream.dequeue_buffer() else {
                return;
            };
            let Some(data) = buffer.datas_mut().first_mut() else {
                return;
            };
            let Some(slice) = data.data() else {
                return;
            };

            let stride = std::mem::size_of::<i16>();
            let frames = slice.len() / stride;
            for frame in 0..frames {
                let sample = queue.pop().unwrap_or(0);
                let start = frame * stride;
                slice[start..start + stride].copy_from_slice(&sample.to_le_bytes());
            }

            let chunk = data.chunk_mut();
            *chunk.offset_mut() = 0;
            *chunk.stride_mut() = stride as i32;
            *chunk.size_mut() = (frames * stride) as u32;
        })
        .register()
        .map_err(|error| error.to_string())?;

    let mut audio_info = AudioInfoRaw::new();
    audio_info.set_format(AudioFormat::S16LE);
    audio_info.set_rate(config.sample_rate);
    audio_info.set_channels(config.channels);
    let mut position = [0; pw::spa::param::audio::MAX_CHANNELS];
    position[0] = pw::spa::sys::SPA_AUDIO_CHANNEL_MONO;
    audio_info.set_position(position);

    let values = PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &Value::Object(Object {
            type_: pw::spa::sys::SPA_TYPE_OBJECT_Format,
            id: pw::spa::sys::SPA_PARAM_EnumFormat,
            properties: audio_info.into(),
        }),
    )
    .map_err(|error| error.to_string())?
    .0
    .into_inner();
    let mut params = [Pod::from_bytes(&values).ok_or("failed to build format POD")?];

    stream
        .connect(
            Direction::Output,
            None,
            StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS | StreamFlags::RT_PROCESS,
            &mut params,
        )
        .map_err(|error| error.to_string())?;

    if let Some(ready) = ready {
        let _ = ready.send(Ok(()));
    }

    let _timer = mainloop.loop_().add_timer({
        let mainloop = mainloop.clone();
        let shutdown = Arc::clone(shutdown);
        move |_| {
            if shutdown.load(Ordering::Relaxed) {
                mainloop.quit();
            }
        }
    });
    _timer.update_timer(Some(SHUTDOWN_POLL), Some(SHUTDOWN_POLL));

    mainloop.run();

    if let Some(error) = failed.take() {
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_drops_oldest_when_full() {
        let queue = PcmQueue::new(3);
        queue.push(&[1, 2, 3, 4, 5]);
        assert_eq!(queue.len(), 3);
        assert_eq!(queue.pop(), Some(3));
        assert_eq!(queue.pop(), Some(4));
        assert_eq!(queue.pop(), Some(5));
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn queue_is_bounded() {
        let queue = PcmQueue::new(2);
        queue.push(&[1, 2, 3]);
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn config_rejects_zero_rate_and_multichannel() {
        let mut config = AudioSourceConfig {
            name: "test".into(),
            description: "test".into(),
            sample_rate: 16_000,
            channels: 1,
            queue_capacity: 160,
        };
        assert!(config.validate().is_ok());
        config.sample_rate = 0;
        assert!(matches!(
            config.validate(),
            Err(AudioSourceError::InvalidConfig(_))
        ));
        config.sample_rate = 16_000;
        config.channels = 2;
        assert!(matches!(
            config.validate(),
            Err(AudioSourceError::InvalidConfig(_))
        ));
    }
}
