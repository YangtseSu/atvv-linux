// SPDX-License-Identifier: GPL-3.0-or-later

//! Session orchestration: transport, timers, audio decoding, input events.

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::{Duration, Instant};

use atvv_config::{Config, ConfigDocument, VinputMode};
use atvv_input::{Action, ActionMap, DictationMode, InputOptions, InputRuntime, RemoteEvent};
use atvv_protocol::adpcm::Decoder;
use atvv_protocol::caps::{Codec, InteractionModel};
use atvv_protocol::commands;
use atvv_protocol::events::{AudioStopReason, CtlEvent};
use atvv_protocol::frame::FrameAccumulator;
use atvv_protocol::profile::{DeviceProfile, NibbleOrder};
use atvv_protocol::session::{Session, SessionAction, SessionState};
use thiserror::Error;

use crate::audio::PcmSink;
use crate::capture::{self, CaptureCell};
use crate::exec::ExecEngine;
use crate::status::{
    AudioView, CapsView, ConfigCell, ConfigView, DeviceView, InputView, StatusCell, StatusView,
    VinputView,
};
use crate::transport::{Transport, TransportEvent};
use crate::vinput::VinputControl;

/// Request from D-Bus or the CLI.
#[derive(Debug, Clone)]
pub enum Command {
    StartVoice,
    StopVoice,
    ToggleVoice,
    /// Re-read `daemon.toml`; the reply carries the result.
    ReloadConfig {
        reply: Sender<Result<(), String>>,
    },
    /// Replace the configuration document; the reply carries the result.
    SetConfig {
        json: String,
        reply: Sender<Result<(), String>>,
    },
    /// Run one checked action once (GUI test mode).
    RunAction {
        action: String,
        reply: Sender<Result<(), String>>,
    },
    Shutdown,
}

/// Events forwarded to D-Bus signals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonEvent {
    StateChanged(String),
    VoiceStarted,
    VoiceStopped,
    /// Logical button activity while capture mode is active.
    ButtonEvent {
        button: String,
        trigger: String,
        pressed: bool,
    },
}

/// Signal transport; D-Bus implements this, tests record it.
pub trait EventSink: Send + Sync {
    fn publish(&self, event: DaemonEvent);
}

/// Discards events; useful for tests and diagnostics.
pub struct NullSink;

impl EventSink for NullSink {
    fn publish(&self, _event: DaemonEvent) {}
}

#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub profile: DeviceProfile,
    pub address: String,
    /// PipeWire `node.name`.
    pub source_name: String,
    pub config_path: PathBuf,
    pub config_loaded: bool,
    /// Validated configuration; the daemon applies changes live.
    pub config: Config,
    pub verbose: bool,
    pub tick: Duration,
    pub keepalive_interval: Duration,
    pub caps_timeout: Duration,
    pub frame_timeout: Duration,
    pub reconnect_delay: Duration,
}

impl DaemonConfig {
    #[must_use]
    pub fn new(profile: DeviceProfile, address: impl Into<String>) -> Self {
        Self {
            profile,
            address: address.into(),
            source_name: format!("atvv-{}", profile.id),
            config_path: atvv_config::default_path(),
            config_loaded: false,
            config: Config::defaults(&profile),
            verbose: false,
            tick: Duration::from_millis(10),
            keepalive_interval: Duration::from_secs(10),
            caps_timeout: Duration::from_secs(2),
            frame_timeout: Duration::from_secs(2),
            reconnect_delay: Duration::from_secs(1),
        }
    }
}

/// Commands and status for the D-Bus interface.
#[derive(Clone)]
pub struct DaemonHandle {
    commands: Sender<Command>,
    status: StatusCell,
    config: ConfigCell,
    capture: CaptureCell,
}

impl DaemonHandle {
    pub fn send(&self, command: Command) -> Result<(), CommandError> {
        self.commands
            .send(command)
            .map_err(|_| CommandError::Closed)
    }

    #[must_use]
    pub fn status(&self) -> StatusCell {
        Arc::clone(&self.status)
    }

    /// Active configuration document (`GetConfig`).
    #[must_use]
    pub fn config(&self) -> ConfigCell {
        Arc::clone(&self.config)
    }

    /// Capture mode flag shared with the D-Bus interface.
    #[must_use]
    pub fn capture(&self) -> CaptureCell {
        Arc::clone(&self.capture)
    }

    #[must_use]
    pub fn commands(&self) -> Sender<Command> {
        self.commands.clone()
    }
}

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("the daemon is not running")]
    Closed,
}

/// The daemon event loop.
pub struct Daemon {
    profile: DeviceProfile,
    address: String,
    transport: Box<dyn Transport>,
    audio: Box<dyn PcmSink>,
    input: Option<InputRuntime>,
    signals: Arc<dyn EventSink>,
    status: StatusCell,
    config_cell: ConfigCell,
    capture: CaptureCell,
    commands: Receiver<Command>,

    vinput: Option<Box<dyn VinputControl>>,
    exec: Option<ExecEngine>,

    session: Session,
    accumulator: Option<FrameAccumulator>,
    decoder: Decoder,
    swap: Vec<u8>,
    pcm: Vec<i16>,

    gain: f32,
    config: Config,
    config_path: PathBuf,
    config_loaded: bool,
    input_retry_at: Option<Instant>,

    connected: bool,
    last_status_at: Instant,
    last_error: Option<String>,

    caps_deadline: Option<Instant>,
    caps_attempts: u8,
    last_audio: Option<Instant>,
    next_keepalive: Option<Instant>,
    discard_until: Option<Instant>,
    reconnect_at: Option<Instant>,

    frames: u64,
    samples: u64,
    dropped_frames: u64,
    traced_audio: bool,
    input_degraded: Vec<String>,
    last_button: Option<String>,
    voice_key: Option<bool>,

    verbose: bool,
    tick: Duration,
    keepalive_interval: Duration,
    caps_timeout: Duration,
    frame_timeout: Duration,
    reconnect_delay: Duration,
}

impl Daemon {
    pub fn new(
        config: DaemonConfig,
        transport: Box<dyn Transport>,
        audio: Box<dyn PcmSink>,
        signals: Arc<dyn EventSink>,
        vinput: Option<Box<dyn VinputControl>>,
    ) -> (Self, DaemonHandle) {
        let (sender, commands) = mpsc::channel();
        let input_trigger = trigger_for(&config.config);
        let (input, input_degraded) = start_input(
            config.profile,
            config.config.input,
            input_trigger,
            &config.config.map,
            config.config.gesture,
        );
        let input_view = InputView {
            enabled: config.config.input,
            available: input.is_some(),
            keyboard: input.as_ref().is_some_and(InputRuntime::keyboard_available),
            grabbed: grabbed_paths(&input),
            degraded: input_degraded.clone(),
            last_button: None,
            voice_key: None,
        };
        let vinput_view = VinputView {
            mode: config.config.vinput.name().to_string(),
            trigger: config.config.trigger_key.clone(),
            available: vinput.as_ref().is_some_and(|control| control.available()),
            last_error: vinput
                .as_ref()
                .and_then(|control| control.last_error().map(str::to_string)),
        };
        let contract =
            crate::status::contract_view(&config.profile, audio.name(), &input_view, &vinput_view);
        let status = StatusView {
            state: "disconnected".to_string(),
            connected: false,
            streaming: false,
            device: DeviceView {
                id: config.profile.id.to_string(),
                name: config.profile.display_name.to_string(),
                model: config.profile.model.to_string(),
                address: config.address.clone(),
                transport: transport.kind().to_string(),
            },
            caps: None,
            audio: AudioView {
                source: audio.name().to_string(),
                available: audio.is_available(),
                streaming: false,
                queued_samples: audio.queued_samples(),
                gain_db: config.config.gain_db,
                frames: 0,
                samples: 0,
                dropped_frames: 0,
                degraded: audio.degraded().map(str::to_string),
            },
            input: input_view,
            vinput: vinput_view,
            contract,
            config: ConfigView {
                path: Some(config.config_path.display().to_string()),
                loaded: config.config_loaded,
                gain_db: config.config.gain_db,
                input: config.config.input,
                allow_exec: config.config.allow_exec,
            },
            last_error: None,
        };
        let status = crate::status::cell(status);
        let config_cell = crate::status::config_cell(ConfigDocument::from_config(&config.config));
        let capture = capture::cell();
        let handle = DaemonHandle {
            commands: sender,
            status: Arc::clone(&status),
            config: Arc::clone(&config_cell),
            capture: Arc::clone(&capture),
        };
        // A lazy transport (BlueZ) starts disconnected. Ask for an immediate
        // reconnect so the daemon owns D-Bus and reports status even when the
        // remote is asleep or BlueZ is still connecting.
        let reconnect_at = transport.needs_connect().then(Instant::now);
        let daemon = Self {
            profile: config.profile,
            address: config.address,
            transport,
            audio,
            input,
            signals,
            status,
            config_cell,
            capture,
            commands,
            vinput,
            exec: None,
            session: Session::new(config.profile),
            accumulator: None,
            decoder: Decoder::new(),
            swap: Vec::new(),
            pcm: Vec::new(),
            gain: gain_factor(config.config.gain_db),
            config: config.config,
            config_path: config.config_path,
            config_loaded: config.config_loaded,
            input_retry_at: None,
            connected: false,
            last_status_at: Instant::now(),
            last_error: None,
            caps_deadline: None,
            caps_attempts: 0,
            last_audio: None,
            next_keepalive: None,
            discard_until: None,
            reconnect_at,
            frames: 0,
            samples: 0,
            dropped_frames: 0,
            traced_audio: false,
            input_degraded,
            last_button: None,
            voice_key: None,
            verbose: config.verbose,
            tick: config.tick,
            keepalive_interval: config.keepalive_interval,
            caps_timeout: config.caps_timeout,
            frame_timeout: config.frame_timeout,
            reconnect_delay: config.reconnect_delay,
        };
        (daemon, handle)
    }

    pub fn run(mut self) {
        self.trace("daemon started");
        self.publish_status();
        loop {
            loop {
                match self.commands.try_recv() {
                    Ok(command) => {
                        if self.on_command(command) {
                            self.trace("shutdown requested");
                            return;
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        self.trace("command channel closed");
                        return;
                    }
                }
            }

            self.poll_input();
            self.poll_exec();
            let now = Instant::now();
            let wait = self.next_wait(now);
            match self.transport.poll(wait) {
                Ok(Some(event)) => self.on_transport_event(event),
                Ok(None) => {}
                Err(error) => {
                    self.last_error = Some(format!("transport: {error}"));
                    self.trace(format!("transport error: {error}"));
                    // Do not spin on a closed transport.
                    std::thread::sleep(self.tick);
                }
            }
            self.poll_input();
            self.poll_exec();
            self.on_timers(Instant::now());
        }
    }

    fn trace(&self, message: impl fmt::Display) {
        if self.verbose {
            eprintln!("atvvd: {message}");
        }
    }

    fn write(&mut self, bytes: &[u8]) {
        self.trace(format!("send {bytes:02x?}"));
        if let Err(error) = self.transport.write_command(bytes) {
            self.last_error = Some(format!("write failed: {error}"));
            self.trace(format!("write failed: {error}"));
        }
    }

    fn execute(&mut self, actions: Vec<SessionAction>) {
        let now = Instant::now();
        for action in actions {
            match action {
                SessionAction::Send(bytes) => self.write(&bytes),
                SessionAction::VoiceStarted => self.on_voice_started(now),
                SessionAction::VoiceStopped => self.on_voice_stopped(now),
            }
        }
        self.publish_status();
    }

    fn on_voice_started(&mut self, now: Instant) {
        let frame_size = self
            .session
            .caps()
            .map_or(self.profile.frame_size, |caps| caps.frame_size);
        self.accumulator = FrameAccumulator::new(frame_size).ok();
        self.decoder = Decoder::new();
        self.frames = 0;
        self.samples = 0;
        self.dropped_frames = 0;
        self.traced_audio = false;
        self.discard_until = None;
        self.last_audio = Some(now);
        self.next_keepalive = Some(now + self.keepalive_interval);
        self.signals.publish(DaemonEvent::VoiceStarted);
        self.trace("voice started");
    }

    fn on_voice_stopped(&mut self, now: Instant) {
        self.accumulator = None;
        self.last_audio = None;
        self.next_keepalive = None;
        self.discard_until =
            Some(now + Duration::from_millis(u64::from(self.profile.post_stop_discard_ms)));
        self.signals.publish(DaemonEvent::VoiceStopped);
        self.trace("voice stopped");
    }

    fn on_transport_event(&mut self, event: TransportEvent) {
        match event {
            TransportEvent::Connected { address, name } => self.on_connected(address, name),
            TransportEvent::Disconnected { reason } => self.on_disconnected(&reason),
            TransportEvent::Control(bytes) => self.on_control(&bytes),
            TransportEvent::Audio(bytes) => self.on_audio(&bytes, Instant::now()),
        }
    }

    fn on_connected(&mut self, address: String, name: String) {
        if self.connected {
            let actions = self.session.disconnect();
            self.execute(actions);
        }
        self.connected = true;
        self.session = Session::new(self.profile);
        self.accumulator = None;
        self.caps_deadline = Some(Instant::now() + self.caps_timeout);
        self.caps_attempts = 0;
        self.last_audio = None;
        self.next_keepalive = None;
        self.discard_until = None;
        self.reconnect_at = None;
        self.last_error = None;
        // The HID-over-GATT event node appears at or shortly after the link;
        // try now and retry from `on_timers` if discovery is not ready yet.
        self.attach_input(Instant::now());
        self.trace(format!("connected to {name} ({address})"));
        let get_caps = commands::get_caps();
        self.write(&get_caps);
        self.publish_status();
    }

    fn on_disconnected(&mut self, reason: &str) {
        if !self.connected {
            return;
        }
        self.connected = false;
        self.last_error = Some(format!("disconnected: {reason}"));
        self.accumulator = None;
        self.caps_deadline = None;
        self.last_audio = None;
        self.next_keepalive = None;
        self.discard_until = None;
        self.reconnect_at = Some(Instant::now() + self.reconnect_delay);
        self.trace(format!("disconnected: {reason}"));
        let actions = self.session.disconnect();
        self.execute(actions);
    }

    fn on_control(&mut self, bytes: &[u8]) {
        self.trace(format!("control {bytes:02x?}"));
        match CtlEvent::parse(bytes) {
            CtlEvent::CapsResp(caps) => {
                self.caps_deadline = None;
                self.trace(format!(
                    "caps v{} codecs=0x{:02x} interaction={:?} frame={}",
                    caps.version, caps.codec_mask, caps.interaction, caps.frame_size
                ));
                let actions = self.session.on_caps(caps);
                self.execute(actions);
            }
            CtlEvent::AudioSync {
                predictor,
                step_index,
                ..
            } => {
                self.decoder.reset(predictor, step_index);
                self.trace(format!(
                    "audio sync predictor={predictor} step_index={step_index}"
                ));
            }
            CtlEvent::MicOpenError(code) => {
                self.last_error = Some(format!("MIC_OPEN error 0x{code:04x}"));
                self.trace(format!("mic open error 0x{code:04x}"));
                let actions = self.session.on_event(CtlEvent::MicOpenError(code));
                self.execute(actions);
            }
            CtlEvent::Unknown(raw) => {
                self.trace(format!("unknown control notification {raw:02x?}"));
            }
            event => {
                let actions = self.session.on_event(event);
                self.execute(actions);
            }
        }
    }

    fn on_audio(&mut self, bytes: &[u8], now: Instant) {
        if let Some(until) = self.discard_until {
            if now < until {
                self.dropped_frames += 1;
                self.maybe_publish(now);
                return;
            }
            self.discard_until = None;
        }

        // Some firmware sends the 0x0A sync on the audio characteristic.
        if bytes.first() == Some(&0x0A) && bytes.len() >= 7 {
            if let CtlEvent::AudioSync {
                predictor,
                step_index,
                ..
            } = CtlEvent::parse(bytes)
            {
                self.decoder.reset(predictor, step_index);
                return;
            }
        }

        if self.session.state() != SessionState::Streaming {
            self.dropped_frames += 1;
            self.maybe_publish(now);
            return;
        }

        let Some(accumulator) = &mut self.accumulator else {
            return;
        };
        let frames = accumulator.append(bytes);
        if frames.is_empty() {
            return;
        }
        self.last_audio = Some(now);
        for frame in frames {
            self.output_frame(&frame);
        }
        self.maybe_publish(now);
    }

    fn maybe_publish(&mut self, now: Instant) {
        if now.duration_since(self.last_status_at) >= Duration::from_millis(200) {
            self.publish_status();
        }
    }

    fn output_frame(&mut self, frame: &[u8]) {
        let needed = frame.len() * 2;
        if self.pcm.len() != needed {
            self.pcm.resize(needed, 0);
        }

        let mut decoder = self.decoder;
        let written = match self.profile.nibble_order {
            NibbleOrder::HighFirst => decoder.decode_into(frame, &mut self.pcm),
            NibbleOrder::LowFirst => {
                self.swap.clear();
                self.swap
                    .extend(frame.iter().map(|byte| byte.rotate_right(4)));
                decoder.decode_into(&self.swap, &mut self.pcm)
            }
        };
        self.decoder = decoder;

        if self.verbose && !self.traced_audio {
            let raw = &frame[..frame.len().min(16)];
            let peak = self.pcm[..written].iter().map(|sample| sample.abs()).max();
            self.trace(format!(
                "first frame {} B raw {raw:02x?} peak {}",
                frame.len(),
                peak.unwrap_or(0)
            ));
            self.traced_audio = true;
        }

        if self.gain != 1.0 {
            for sample in &mut self.pcm[..written] {
                let scaled = f32::from(*sample) * self.gain;
                *sample = scaled.clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16;
            }
        }
        self.audio.push(&self.pcm[..written]);
        self.frames += 1;
        self.samples += written as u64;
    }

    fn on_command(&mut self, command: Command) -> bool {
        match command {
            Command::Shutdown => return true,
            Command::StartVoice => self.request_voice(true),
            Command::StopVoice => self.request_voice(false),
            Command::ToggleVoice => {
                let active = matches!(
                    self.session.state(),
                    SessionState::Opening | SessionState::Streaming
                );
                self.request_voice(!active);
            }
            Command::ReloadConfig { reply } => {
                let result = self.reload_config();
                if let Some(error) = result.as_ref().err() {
                    self.last_error = Some(format!("reload: {error}"));
                    self.trace(format!("config reload failed: {error}"));
                }
                let _ = reply.send(result);
            }
            Command::SetConfig { json, reply } => {
                let result = self.set_config(&json);
                let failed = result.as_ref().err().cloned();
                if let Some(error) = failed {
                    self.last_error = Some(format!("set config: {error}"));
                    self.trace(format!("set config failed: {error}"));
                }
                let _ = reply.send(result);
            }
            Command::RunAction { action, reply } => {
                let result = self.run_action(&action);
                let failed = result.as_ref().err().cloned();
                if let Some(error) = failed {
                    self.last_error = Some(format!("run action: {error}"));
                    self.trace(format!("run action failed: {error}"));
                }
                let _ = reply.send(result);
            }
        }
        self.publish_status();
        false
    }

    /// Execute one already-checked action for the GUI's test mode.
    ///
    /// Key and chord actions use the running input runtime, `exec` enters the
    /// serial command worker, and dictation drives the ATVV session. The GUI
    /// never emits keys or runs commands itself.
    fn run_action(&mut self, raw: &str) -> Result<(), String> {
        let action = atvv_config::parse_checked_action(raw, self.config.allow_exec)
            .map_err(|error| error.to_string())?;
        self.trace(format!("run action {action}"));
        match action {
            Action::None => Ok(()),
            Action::Key { .. } | Action::Combo { .. } => self
                .input
                .as_ref()
                .ok_or_else(|| "the input runtime is not available".to_string())?
                .tap_action(&action),
            Action::Exec { command } => {
                self.run_exec(&command);
                Ok(())
            }
            Action::Dictation(mode) => {
                match mode {
                    DictationMode::Start => self.request_voice(true),
                    DictationMode::Stop => self.request_voice(false),
                    DictationMode::Toggle => {
                        let active = matches!(
                            self.session.state(),
                            SessionState::Opening | SessionState::Streaming
                        );
                        self.request_voice(!active);
                    }
                }
                Ok(())
            }
        }
    }

    fn request_voice(&mut self, start: bool) {
        if !self.connected {
            self.last_error = Some("the device is not connected".to_string());
            return;
        }
        match (start, self.session.state()) {
            (true, SessionState::Ready) => {
                self.trace("manual MIC_OPEN");
                let actions = self.session.on_start_search();
                self.execute(actions);
            }
            (true, SessionState::Opening | SessionState::Streaming) => {
                self.trace("voice is already active");
            }
            (true, SessionState::Connected | SessionState::Disconnected) => {
                self.last_error = Some("capabilities are not ready".to_string());
            }
            (false, SessionState::Opening | SessionState::Streaming) => {
                self.trace("manual MIC_CLOSE");
                let actions = self.session.on_start_search();
                self.execute(actions);
            }
            (false, SessionState::Ready) => {}
            (false, SessionState::Connected | SessionState::Disconnected) => {
                self.last_error = Some("the device is not streaming".to_string());
            }
        }
    }

    fn reload_config(&mut self) -> Result<(), String> {
        let loaded = atvv_config::load(&self.config_path, &self.profile)
            .map_err(|error| error.to_string())?;
        let existed = loaded.existed;
        self.apply_config(loaded.config)?;
        self.config_loaded = existed;
        self.last_error = None;
        self.trace(format!(
            "config reloaded: gain={} dB input={} buttons={}",
            self.config.gain_db,
            self.config.input,
            self.config.map.actions().count()
        ));
        Ok(())
    }

    /// Validate, apply, and persist a JSON configuration document.
    ///
    /// The runtime is applied before the file is written; a failed apply
    /// leaves the previous map active, and a failed write rolls the runtime
    /// back.
    fn set_config(&mut self, json: &str) -> Result<(), String> {
        let document: ConfigDocument = serde_json::from_str(json)
            .map_err(|error| format!("invalid configuration document: {error}"))?;
        let next = document
            .validate(&self.profile)
            .map_err(|error| error.to_string())?;
        let previous = self.config.clone();
        self.apply_config(next)?;
        if let Err(error) = atvv_config::save_document(&self.config_path, &document, &self.profile)
        {
            let _ = self.apply_config(previous);
            return Err(format!(
                "cannot save {}: {error}",
                self.config_path.display()
            ));
        }
        self.config_loaded = true;
        self.last_error = None;
        Ok(())
    }

    /// Apply a validated configuration to the live runtime.
    ///
    /// The input runtime is recreated only when the map, timing, trigger, or
    /// vinput mode changes; its uinput key registration depends on the map.
    /// On failure the previous runtime is restored and the previous
    /// configuration stays active.
    fn apply_config(&mut self, next: Config) -> Result<(), String> {
        let old = self.config.clone();
        let restart = old.input != next.input
            || old.map != next.map
            || old.gesture != next.gesture
            || old.trigger_key != next.trigger_key
            || old.vinput != next.vinput;
        let had_input = self.input.is_some();

        if restart {
            if !next.input {
                self.input = None;
                self.input_degraded.clear();
                self.input_retry_at = None;
            } else if had_input {
                // Dropping the old runtime releases the evdev grab and the
                // uinput device, so the new map can register its own keys.
                self.input = None;
                self.input_degraded.clear();
                let (input, degraded) = start_input(
                    self.profile,
                    true,
                    trigger_for(&next),
                    &next.map,
                    next.gesture,
                );
                if input.is_some() {
                    self.input = input;
                    self.input_degraded = degraded;
                    self.input_retry_at = None;
                } else {
                    let (previous_input, previous_degraded) = start_input(
                        self.profile,
                        old.input,
                        trigger_for(&old),
                        &old.map,
                        old.gesture,
                    );
                    let restored = previous_input.is_some();
                    self.input = previous_input;
                    self.input_degraded = previous_degraded;
                    self.input_retry_at =
                        (!restored).then(|| Instant::now() + Duration::from_secs(2));
                    return Err(format!(
                        "the input runtime did not accept the new configuration: {}",
                        degraded.join("; ")
                    ));
                }
            } else {
                self.input_retry_at = Some(Instant::now());
            }
        }

        self.gain = gain_factor(next.gain_db);
        self.config = next;
        self.publish_config_document();
        Ok(())
    }

    fn publish_config_document(&self) {
        let document = ConfigDocument::from_config(&self.config);
        *crate::status::lock_config(&self.config_cell) = document;
    }

    /// Start the input runtime when its HID node is present.
    ///
    /// The link can come up before the HID-over-GATT node, and the node is
    /// recreated on every reconnect, so a failed discovery is retried.
    fn attach_input(&mut self, now: Instant) {
        if !self.config.input || self.input.is_some() {
            return;
        }
        let trigger = trigger_for(&self.config).map(str::to_string);
        let (input, degraded) = start_input(
            self.profile,
            true,
            trigger.as_deref(),
            &self.config.map,
            self.config.gesture,
        );
        let attached = input.is_some();
        self.input = input;
        self.input_degraded = degraded;
        self.input_retry_at = if attached {
            None
        } else {
            Some(now + Duration::from_secs(2))
        };
        self.trace(if attached {
            "input attached"
        } else {
            "input node not ready; retrying"
        });
    }

    fn poll_exec(&mut self) {
        let Some(engine) = &self.exec else {
            return;
        };
        let mut results = Vec::new();
        while let Some(result) = engine.try_recv() {
            results.push(result);
        }
        for result in results {
            match result.outcome {
                Ok(()) => self.trace(format!("exec {:?} finished", result.command)),
                Err(error) => {
                    let message = format!("exec {:?}: {error}", result.command);
                    self.last_error = Some(message.clone());
                    self.trace(message);
                    self.publish_status();
                }
            }
        }
    }

    fn poll_input(&mut self) {
        let mut events = Vec::new();
        if let Some(input) = &self.input {
            loop {
                match input.try_recv() {
                    Ok(event) => events.push(event),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => break,
                }
            }
        }
        for event in events {
            self.on_input_event(event);
        }
    }

    fn on_input_event(&mut self, event: RemoteEvent) {
        match event {
            RemoteEvent::Voice { pressed } => self.on_voice_key(pressed),
            RemoteEvent::Button {
                button,
                gesture,
                action,
                error,
            } => {
                self.last_button = Some(format!("{button}:{gesture:?}"));
                self.trace(format!(
                    "button {button} {gesture:?} -> {}",
                    action
                        .as_ref()
                        .map_or_else(|| "unmapped".to_string(), ToString::to_string)
                ));
                if let Some(error) = error {
                    self.note_input_degraded(error);
                }
                if capture::is_active(&self.capture) {
                    self.signals.publish(DaemonEvent::ButtonEvent {
                        button: button.to_string(),
                        trigger: gesture
                            .trigger()
                            .map_or("release", atvv_input::Trigger::name)
                            .to_string(),
                        pressed: gesture.trigger().is_some(),
                    });
                }
                match action {
                    Some(Action::Dictation(DictationMode::Start)) => self.request_voice(true),
                    Some(Action::Dictation(DictationMode::Stop)) => self.request_voice(false),
                    Some(Action::Dictation(DictationMode::Toggle)) => {
                        let active = matches!(
                            self.session.state(),
                            SessionState::Opening | SessionState::Streaming
                        );
                        self.request_voice(!active);
                    }
                    Some(Action::Exec { command }) => self.run_exec(&command),
                    _ => {}
                }
            }
            RemoteEvent::Degraded { message } => self.note_input_degraded(message),
            RemoteEvent::DeviceLost { path, error } => {
                self.note_input_degraded(format!("lost {}: {error}", path.display()));
                // Drop the dead readers/keyboard, then re-grab the node when it
                // reappears after a reconnect.
                self.input = None;
                self.input_retry_at = self
                    .connected
                    .then(|| Instant::now() + Duration::from_secs(2));
            }
        }
        self.publish_status();
    }

    /// The RC003 voice key gates microphone data, so the ATVV session is
    /// opened and closed by the key edges, never by PipeWire consumer state.
    fn on_voice_key(&mut self, pressed: bool) {
        self.voice_key = Some(pressed);
        self.trace(format!(
            "voice key {}",
            if pressed { "pressed" } else { "released" }
        ));
        self.request_voice(pressed);
        if self.config.vinput == VinputMode::Dbus
            && let Some(vinput) = &mut self.vinput
        {
            let result = if pressed {
                vinput.start()
            } else {
                vinput.stop()
            };
            if let Err(error) = result {
                self.trace(format!("{error}"));
            }
        }
    }

    fn run_exec(&mut self, command: &str) {
        if !self.config.allow_exec {
            let message = "exec action rejected: allow_exec is false".to_string();
            self.last_error = Some(message.clone());
            self.trace(message);
            return;
        }
        if self.exec.is_none() {
            self.exec = Some(ExecEngine::start());
        }
        let result = self.exec.as_ref().map(|engine| engine.run(command));
        if let Some(Err(error)) = result {
            let message = format!("exec {command:?}: {error}");
            self.last_error = Some(message.clone());
            self.trace(message);
        } else {
            self.trace(format!("exec queued: {command:?}"));
        }
    }

    fn note_input_degraded(&mut self, message: String) {
        if self.input_degraded.contains(&message) {
            return;
        }
        if self.input_degraded.len() >= 16 {
            self.input_degraded.remove(0);
        }
        self.input_degraded.push(message);
    }

    fn next_wait(&self, now: Instant) -> Duration {
        let frame_deadline = (self.session.state() == SessionState::Streaming)
            .then(|| {
                self.last_audio
                    .map(|last| last + self.frame_timeout)
                    .or(Some(now))
            })
            .flatten();
        let mut wait = self.tick;
        for deadline in [
            self.caps_deadline,
            self.next_keepalive,
            self.reconnect_at,
            self.input_retry_at,
            frame_deadline,
        ]
        .into_iter()
        .flatten()
        {
            wait = wait.min(deadline.saturating_duration_since(now));
        }
        wait
    }

    fn on_timers(&mut self, now: Instant) {
        let mut changed = false;

        if let Some(deadline) = self.caps_deadline {
            if now >= deadline {
                if self.caps_attempts < 2 {
                    self.caps_attempts += 1;
                    self.caps_deadline = Some(now + self.caps_timeout);
                    self.trace("CAPS_RESP timeout; retrying GET_CAPS");
                    let get_caps = commands::get_caps();
                    self.write(&get_caps);
                } else {
                    self.caps_deadline = None;
                    self.last_error = Some("CAPS_RESP timeout".to_string());
                    self.trace("CAPS_RESP timeout");
                }
                changed = true;
            }
        }

        if self.session.state() == SessionState::Streaming {
            if let Some(deadline) = self.next_keepalive {
                if now >= deadline {
                    if let Some(payload) = self.session.keepalive() {
                        self.write(&payload);
                        self.trace("keepalive MIC_EXTEND");
                    }
                    self.next_keepalive = Some(now + self.keepalive_interval);
                }
            }

            if let Some(last) = self.last_audio {
                if now.duration_since(last) >= self.frame_timeout {
                    self.last_error = Some("audio frame timeout".to_string());
                    self.trace("audio frame timeout; closing the stream");
                    let actions = self.session.on_audio_stop(AudioStopReason::TransferTimeout);
                    self.execute(actions);
                    changed = true;
                }
            }
        }

        if let Some(deadline) = self.input_retry_at {
            if now >= deadline {
                self.input_retry_at = None;
                if self.config.input && self.input.is_none() {
                    self.attach_input(now);
                }
                changed = true;
            }
        }

        if let Some(deadline) = self.reconnect_at {
            if now >= deadline {
                self.reconnect_at = None;
                self.trace("reconnecting");
                if let Err(error) = self.transport.reconnect() {
                    self.last_error = Some(format!("reconnect failed: {error}"));
                    self.reconnect_at = Some(now + self.reconnect_delay);
                }
                changed = true;
            }
        }

        if changed {
            self.publish_status();
        }
    }

    fn publish_status(&mut self) {
        let view = self.status_view();
        let state = view.state.clone();
        let changed = {
            let mut current = crate::status::lock(&self.status);
            let changed = current.state != state;
            *current = view;
            changed
        };
        self.last_status_at = Instant::now();
        if changed {
            self.signals
                .publish(DaemonEvent::StateChanged(state.clone()));
            self.trace(format!("state: {state}"));
        }
    }

    fn status_view(&self) -> StatusView {
        let caps = self.session.caps().map(|caps| CapsView {
            version: caps.version.to_string(),
            codec: caps
                .best_codec()
                .map_or_else(|| "none".to_string(), codec_name),
            codec_mask: caps.codec_mask,
            interaction: interaction_name(caps.interaction),
            frame_size: caps.frame_size,
        });
        let input = InputView {
            enabled: self.config.input,
            available: self.input.is_some(),
            keyboard: self
                .input
                .as_ref()
                .is_some_and(InputRuntime::keyboard_available),
            grabbed: grabbed_paths(&self.input),
            degraded: self.input_degraded.clone(),
            last_button: self.last_button.clone(),
            voice_key: self.voice_key,
        };
        let vinput = VinputView {
            mode: self.config.vinput.name().to_string(),
            trigger: self.config.trigger_key.clone(),
            available: self
                .vinput
                .as_ref()
                .is_some_and(|control| control.available()),
            last_error: self
                .vinput
                .as_ref()
                .and_then(|control| control.last_error().map(str::to_string)),
        };
        let contract =
            crate::status::contract_view(&self.profile, self.audio.name(), &input, &vinput);
        StatusView {
            state: self.state_name(),
            connected: self.connected,
            streaming: self.session.state() == SessionState::Streaming,
            device: DeviceView {
                id: self.profile.id.to_string(),
                name: self.profile.display_name.to_string(),
                model: self.profile.model.to_string(),
                address: self.address.clone(),
                transport: self.transport.kind().to_string(),
            },
            caps,
            audio: AudioView {
                source: self.audio.name().to_string(),
                available: self.audio.is_available(),
                streaming: self.session.state() == SessionState::Streaming,
                queued_samples: self.audio.queued_samples(),
                gain_db: self.config.gain_db,
                frames: self.frames,
                samples: self.samples,
                dropped_frames: self.dropped_frames,
                degraded: self.audio.degraded().map(str::to_string),
            },
            input,
            vinput,
            contract,
            config: ConfigView {
                path: Some(self.config_path.display().to_string()),
                loaded: self.config_loaded,
                gain_db: self.config.gain_db,
                input: self.config.input,
                allow_exec: self.config.allow_exec,
            },
            last_error: self.last_error.clone(),
        }
    }

    fn state_name(&self) -> String {
        if !self.connected {
            return "disconnected".to_string();
        }
        match self.session.state() {
            SessionState::Connected => "connected",
            SessionState::Ready => "ready",
            SessionState::Opening => "opening",
            SessionState::Streaming => "streaming",
            SessionState::Disconnected => "disconnected",
        }
        .to_string()
    }
}

fn start_input(
    profile: DeviceProfile,
    enabled: bool,
    voice_trigger: Option<&str>,
    map: &ActionMap,
    gesture: atvv_protocol::profile::GestureTiming,
) -> (Option<InputRuntime>, Vec<String>) {
    if !enabled {
        return (None, Vec::new());
    }
    let options = InputOptions {
        voice_trigger: voice_trigger.map(str::to_string),
        gesture: Some(gesture),
    };
    match InputRuntime::start_with_options(profile, map.clone(), options) {
        Ok(runtime) => {
            let degraded = runtime.degraded().to_vec();
            (Some(runtime), degraded)
        }
        Err(error) => (None, vec![error.to_string()]),
    }
}

/// Synthetic key passed to the input runtime, if trigger mode is active.
fn trigger_for(config: &Config) -> Option<&str> {
    if config.vinput == VinputMode::Trigger {
        config.trigger_key.as_deref()
    } else {
        None
    }
}

fn grabbed_paths(input: &Option<InputRuntime>) -> Vec<String> {
    input.as_ref().map_or_else(Vec::new, |runtime| {
        runtime
            .grabbed()
            .iter()
            .map(|path| path.display().to_string())
            .collect()
    })
}

fn gain_factor(db: i8) -> f32 {
    if db == 0 {
        1.0
    } else {
        10.0_f32.powf(f32::from(db) / 20.0)
    }
}

fn codec_name(codec: Codec) -> String {
    match codec {
        Codec::Adpcm8k => "adpcm-8k",
        Codec::Adpcm16k => "adpcm-16k",
    }
    .to_string()
}

fn interaction_name(interaction: InteractionModel) -> String {
    match interaction {
        InteractionModel::OnRequest => "on-request".to_string(),
        InteractionModel::PressToTalk => "press-to-talk".to_string(),
        InteractionModel::HoldToTalk => "hold-to-talk".to_string(),
        InteractionModel::Unknown(value) => format!("unknown-{value:02x}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    use atvv_protocol::profile::{RC003, RemoteButton};

    use crate::transport::{MockConfig, MockHandle, MockTransport};

    struct CountingSink {
        samples: usize,
    }

    impl PcmSink for CountingSink {
        fn push(&mut self, samples: &[i16]) {
            self.samples += samples.len();
        }

        fn name(&self) -> &str {
            "counting"
        }

        fn is_available(&self) -> bool {
            true
        }

        fn queued_samples(&self) -> usize {
            self.samples
        }

        fn degraded(&self) -> Option<&str> {
            None
        }
    }

    struct DegradedSink;

    impl PcmSink for DegradedSink {
        fn push(&mut self, _samples: &[i16]) {}

        fn name(&self) -> &str {
            "pipewire"
        }

        fn is_available(&self) -> bool {
            false
        }

        fn queued_samples(&self) -> usize {
            0
        }

        fn degraded(&self) -> Option<&str> {
            Some("PipeWire is not available")
        }
    }

    #[derive(Default)]
    struct RecordingSink {
        events: Mutex<Vec<DaemonEvent>>,
    }

    impl EventSink for RecordingSink {
        fn publish(&self, event: DaemonEvent) {
            self.events.lock().unwrap().push(event);
        }
    }

    #[derive(Default)]
    struct RecordingVinput {
        calls: Arc<Mutex<Vec<&'static str>>>,
        available: bool,
    }

    impl crate::vinput::VinputControl for RecordingVinput {
        fn start(&mut self) -> Result<(), crate::vinput::VinputError> {
            self.calls.lock().unwrap().push("start");
            self.available = true;
            Ok(())
        }

        fn stop(&mut self) -> Result<(), crate::vinput::VinputError> {
            self.calls.lock().unwrap().push("stop");
            self.available = true;
            Ok(())
        }

        fn available(&self) -> bool {
            self.available
        }

        fn last_error(&self) -> Option<&str> {
            None
        }
    }

    fn start() -> (DaemonHandle, MockHandle, Arc<RecordingSink>) {
        let (transport, mock) = MockTransport::new(MockConfig::default());
        let signals = Arc::new(RecordingSink::default());
        let mut config = DaemonConfig::new(RC003, mock_address(&transport));
        config.config.input = false;
        config.tick = Duration::from_millis(5);
        config.keepalive_interval = Duration::from_millis(40);
        config.frame_timeout = Duration::from_millis(80);
        config.reconnect_delay = Duration::from_millis(100);
        let (daemon, handle) = Daemon::new(
            config,
            Box::new(transport),
            Box::new(CountingSink { samples: 0 }),
            signals.clone(),
            None,
        );
        std::thread::spawn(move || daemon.run());
        (handle, mock, signals)
    }

    fn mock_address(transport: &MockTransport) -> String {
        transport.address().to_string()
    }

    fn wait_for(handle: &DaemonHandle, predicate: impl Fn(&StatusView) -> bool) -> StatusView {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let status = crate::status::lock(&handle.status()).clone();
            if predicate(&status) {
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "timed out; last status: {status:?}"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// A transport that only starts producing events after the daemon asks for
    /// its first connection, like the lazy BlueZ transport.
    struct LazyTransport {
        inner: MockTransport,
        connected: bool,
        connecting: bool,
    }

    impl Transport for LazyTransport {
        fn kind(&self) -> &'static str {
            "lazy-mock"
        }

        fn address(&self) -> &str {
            self.inner.address()
        }

        fn device_name(&self) -> &str {
            self.inner.device_name()
        }

        fn needs_connect(&self) -> bool {
            !self.connected
        }

        fn poll(
            &mut self,
            timeout: Duration,
        ) -> Result<Option<TransportEvent>, crate::transport::TransportError> {
            if !self.connected && !self.connecting {
                std::thread::sleep(timeout);
                return Ok(None);
            }
            let event = self.inner.poll(timeout);
            if matches!(&event, Ok(Some(TransportEvent::Connected { .. }))) {
                self.connected = true;
                self.connecting = false;
            }
            event
        }

        fn write_command(&mut self, bytes: &[u8]) -> Result<(), crate::transport::TransportError> {
            self.inner.write_command(bytes)
        }

        fn reconnect(&mut self) -> Result<(), crate::transport::TransportError> {
            self.connecting = true;
            self.inner.reconnect()
        }
    }

    #[test]
    fn initial_lazy_connect_reaches_ready() {
        let (inner, _mock) = MockTransport::new(MockConfig::default());
        let transport = LazyTransport {
            inner,
            connected: false,
            connecting: false,
        };
        let mut config = DaemonConfig::new(RC003, "02:00:00:00:00:01");
        config.config.input = false;
        config.tick = Duration::from_millis(5);
        config.caps_timeout = Duration::from_millis(200);
        config.reconnect_delay = Duration::from_millis(10);
        let (daemon, handle) = Daemon::new(
            config,
            Box::new(transport),
            Box::new(CountingSink { samples: 0 }),
            Arc::new(NullSink),
            None,
        );
        std::thread::spawn(move || daemon.run());

        let status = wait_for(&handle, |status| status.state == "ready");
        assert!(status.connected);
        handle.send(Command::Shutdown).unwrap();
    }

    #[test]
    fn voice_key_drives_vinput_in_dbus_mode() {
        let (transport, mock) = MockTransport::new(MockConfig::default());
        let mut config = DaemonConfig::new(RC003, mock_address(&transport));
        config.config.input = false;
        config.config.vinput = VinputMode::Dbus;
        let calls = Arc::new(Mutex::new(Vec::new()));
        let vinput = RecordingVinput {
            calls: Arc::clone(&calls),
            available: false,
        };
        let (mut daemon, handle) = Daemon::new(
            config,
            Box::new(transport),
            Box::new(CountingSink { samples: 0 }),
            Arc::new(NullSink),
            Some(Box::new(vinput)),
        );

        daemon.on_input_event(RemoteEvent::Voice { pressed: true });
        daemon.on_input_event(RemoteEvent::Voice { pressed: false });

        assert_eq!(*calls.lock().unwrap(), vec!["start", "stop"]);
        let status = crate::status::lock(&handle.status()).clone();
        assert_eq!(status.vinput.mode, "dbus");
        assert!(status.vinput.available);
        assert_eq!(status.input.voice_key, Some(false));
        let _ = mock;
    }

    #[test]
    fn trigger_mode_is_the_default() {
        let config = DaemonConfig::new(RC003, "02:00:00:00:00:01");
        assert_eq!(config.config.vinput, VinputMode::Trigger);
        assert_eq!(
            config.config.trigger_key.as_deref(),
            Some(atvv_config::DEFAULT_TRIGGER_KEY)
        );
    }

    fn config_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("atvv-daemon-{name}-{}.toml", std::process::id()))
    }

    fn config_daemon(name: &str) -> (Daemon, DaemonHandle, MockHandle, PathBuf) {
        let (transport, mock) = MockTransport::new(MockConfig::default());
        let mut config = DaemonConfig::new(RC003, mock_address(&transport));
        config.config.input = false;
        let path = config_path(name);
        let _ = std::fs::remove_file(&path);
        config.config_path = path.clone();
        let (daemon, handle) = Daemon::new(
            config,
            Box::new(transport),
            Box::new(CountingSink { samples: 0 }),
            Arc::new(NullSink),
            None,
        );
        (daemon, handle, mock, path)
    }

    #[test]
    fn set_config_applies_and_persists_with_validation() {
        let (mut daemon, handle, _mock, path) = config_daemon("set");
        let mut document = ConfigDocument::from_config(&Config::defaults(&RC003));
        document.gain_db = 6;
        document.buttons.get_mut("up").unwrap().single = Some("key:KEY_F13".into());
        daemon
            .set_config(&serde_json::to_string(&document).unwrap())
            .unwrap();

        let config_cell = handle.config();
        {
            let active = crate::status::lock_config(&config_cell);
            assert_eq!(active.gain_db, 6);
            assert_eq!(active.buttons["up"].single.as_deref(), Some("key:KEY_F13"));
        }
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains("gain_db = 6"), "{written}");
        assert!(written.contains("[buttons.up]"), "{written}");
        assert!(written.contains("single = \"key:KEY_F13\""), "{written}");

        let before = crate::status::lock_config(&handle.config()).clone();
        let mut invalid = document.clone();
        invalid.buttons.get_mut("up").unwrap().single = Some("key:not-a-key".into());
        let error = daemon
            .set_config(&serde_json::to_string(&invalid).unwrap())
            .unwrap_err();
        assert!(error.contains("not-a-key"), "{error}");
        assert_eq!(crate::status::lock_config(&handle.config()).clone(), before);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), written);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn reload_applies_button_overrides_and_keeps_the_old_map_on_error() {
        let (mut daemon, handle, _mock, path) = config_daemon("reload");
        std::fs::write(&path, "[buttons.up]\nsingle = \"key:KEY_F13\"\n").unwrap();
        daemon.reload_config().unwrap();
        let config_cell = handle.config();
        {
            let active = crate::status::lock_config(&config_cell);
            assert_eq!(active.buttons["up"].single.as_deref(), Some("key:KEY_F13"));
            assert_eq!(
                active.buttons["ok"].single.as_deref(),
                Some("key:KEY_ENTER")
            );
        }

        std::fs::write(&path, "[buttons.up]\nsingle = \"key:not-a-key\"\n").unwrap();
        let error = daemon.reload_config().unwrap_err();
        assert!(error.contains("not-a-key"), "{error}");
        assert_eq!(
            crate::status::lock_config(&handle.config()).buttons["up"]
                .single
                .as_deref(),
            Some("key:KEY_F13")
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn exec_requires_allow_exec_and_runs_the_command() {
        let (mut daemon, handle, _mock, config_path) = config_daemon("exec");
        let marker =
            std::env::temp_dir().join(format!("atvv-daemon-exec-marker-{}", std::process::id()));
        let _ = std::fs::remove_file(&marker);
        let command = format!("touch {}", marker.display());
        let event = RemoteEvent::Button {
            button: RemoteButton::Ok,
            gesture: atvv_input::Gesture::SingleClick(RemoteButton::Ok),
            action: Some(Action::Exec {
                command: command.clone(),
            }),
            error: None,
        };

        daemon.on_input_event(event.clone());
        assert!(
            daemon
                .last_error
                .as_deref()
                .is_some_and(|error| error.contains("allow_exec"))
        );
        assert!(!marker.exists());

        let mut document = ConfigDocument::from_config(&Config::defaults(&RC003));
        document.allow_exec = true;
        daemon
            .set_config(&serde_json::to_string(&document).unwrap())
            .unwrap();
        daemon.on_input_event(event);

        let deadline = Instant::now() + Duration::from_secs(2);
        while !marker.exists() && Instant::now() < deadline {
            daemon.poll_exec();
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(marker.exists(), "exec command did not run");
        let _ = std::fs::remove_file(&marker);
        let _ = std::fs::remove_file(&config_path);
        let _ = handle;
    }

    #[test]
    fn capture_emits_button_events_only_while_active() {
        let signals = Arc::new(RecordingSink::default());
        let (transport, _mock) = MockTransport::new(MockConfig::default());
        let mut config = DaemonConfig::new(RC003, mock_address(&transport));
        config.config.input = false;
        let (mut daemon, handle) = Daemon::new(
            config,
            Box::new(transport),
            Box::new(CountingSink { samples: 0 }),
            signals.clone(),
            None,
        );
        let event = RemoteEvent::Button {
            button: RemoteButton::Ok,
            gesture: atvv_input::Gesture::SingleClick(RemoteButton::Ok),
            action: None,
            error: None,
        };
        let button_events = || {
            signals
                .events
                .lock()
                .unwrap()
                .iter()
                .filter(|event| matches!(event, DaemonEvent::ButtonEvent { .. }))
                .cloned()
                .collect::<Vec<_>>()
        };

        daemon.on_input_event(event.clone());
        assert!(button_events().is_empty());

        let cell = handle.capture();
        capture::begin(&cell, None);
        daemon.on_input_event(event);
        assert_eq!(
            button_events(),
            vec![DaemonEvent::ButtonEvent {
                button: "ok".to_string(),
                trigger: "single".to_string(),
                pressed: true,
            }]
        );

        capture::end(&cell);
        daemon.on_input_event(RemoteEvent::Button {
            button: RemoteButton::Ok,
            gesture: atvv_input::Gesture::Release(RemoteButton::Ok),
            action: None,
            error: None,
        });
        assert_eq!(button_events().len(), 1);
    }

    #[test]
    #[ignore = "needs /dev/uinput and evdev read access"]
    fn live_map_change_reaches_the_fake_remote() {
        use atvv_input::{FAKE_REMOTE, FakeRemote};
        use evdevil::Evdev;
        use evdevil::event::{EventKind, Key, KeyState};

        let mut fake = FakeRemote::open().expect("fake remote");
        let (transport, _mock) = MockTransport::new(MockConfig::default());
        let path = config_path("live");
        let _ = std::fs::remove_file(&path);
        let mut config = DaemonConfig::new(FAKE_REMOTE, mock_address(&transport));
        config.config.input = true;
        config.config_path = path.clone();
        let (mut daemon, _handle) = Daemon::new(
            config,
            Box::new(transport),
            Box::new(CountingSink { samples: 0 }),
            Arc::new(NullSink),
            None,
        );
        assert!(daemon.input.is_some(), "input runtime did not attach");

        let mut document = ConfigDocument::from_config(&Config::defaults(&FAKE_REMOTE));
        document.buttons.get_mut("up").unwrap().single = Some("key:KEY_F13".into());
        daemon
            .set_config(&serde_json::to_string(&document).unwrap())
            .expect("live SetConfig");

        let keyboard = daemon
            .input
            .as_ref()
            .expect("input runtime after reload")
            .keyboard_path()
            .expect("virtual keyboard")
            .to_path_buf();
        // udev applies the seat ACL after the node appears.
        let deadline = Instant::now() + Duration::from_secs(3);
        let device = loop {
            match Evdev::open(&keyboard) {
                Ok(device) => break device,
                Err(_) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("cannot open {}: {error}", keyboard.display()),
            }
        };
        device.set_nonblocking(true).unwrap();
        let mut reader = device.into_reader().unwrap();

        fake.tap(RemoteButton::Up).expect("tap up");
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut found = false;
        while Instant::now() < deadline && !found {
            for event in reader.events() {
                let Ok(event) = event else {
                    continue;
                };
                if let EventKind::Key(event) = event.kind()
                    && event.key() == Key::KEY_F13
                    && event.state() == KeyState::PRESSED
                {
                    found = true;
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(found, "the new map did not reach the fake remote");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn direct_audio_start_streams_and_stop_echo_closes_once() {
        let (handle, mock, signals) = start();
        wait_for(&handle, |status| status.state == "ready");

        mock.start_audio();
        let status = wait_for(&handle, |status| {
            status.state == "streaming" && status.audio.frames > 0 && status.audio.samples > 0
        });
        assert!(status.audio.frames > 0, "no decoded frames: {status:?}");
        assert!(status.audio.samples > 0, "no decoded samples: {status:?}");

        mock.stop_audio(0x00);
        wait_for(&handle, |status| status.state == "ready");
        assert_eq!(mock.command_count(0x0D), 1);
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(mock.command_count(0x0D), 1, "echo caused a second close");

        let events = signals.events.lock().unwrap().clone();
        assert!(events.contains(&DaemonEvent::VoiceStarted), "{events:?}");
        assert!(events.contains(&DaemonEvent::VoiceStopped), "{events:?}");
        assert!(
            events.contains(&DaemonEvent::StateChanged("streaming".to_string())),
            "{events:?}"
        );

        handle.send(Command::Shutdown).unwrap();
    }

    #[test]
    fn keepalive_discard_and_reconnect() {
        let (handle, mock, _signals) = start();
        wait_for(&handle, |status| status.state == "ready");

        mock.start_audio();
        wait_for(&handle, |status| {
            status.state == "streaming" && status.audio.frames > 0
        });

        let deadline = Instant::now() + Duration::from_secs(1);
        while mock.command_count(0x0E) == 0 {
            assert!(
                Instant::now() < deadline,
                "no MIC_EXTEND keepalive while streaming"
            );
            std::thread::sleep(Duration::from_millis(5));
        }

        mock.stop_audio(0x00);
        let status = wait_for(&handle, |status| status.state == "ready");
        let samples = status.audio.samples;

        // Late audio inside the 300 ms post-stop window is discarded.
        mock.send_audio(vec![0xAA; 120]);
        let status = wait_for(&handle, |status| status.audio.dropped_frames > 0);
        assert_eq!(status.audio.samples, samples);

        mock.disconnect("test link loss");
        wait_for(&handle, |status| status.state == "disconnected");
        wait_for(&handle, |status| status.state == "ready");

        handle.send(Command::Shutdown).unwrap();
    }

    #[test]
    fn frame_timeout_closes_the_stream() {
        let (handle, mock, _signals) = start();
        wait_for(&handle, |status| status.state == "ready");

        mock.start_audio();
        wait_for(&handle, |status| {
            status.state == "streaming" && status.audio.frames > 0
        });
        mock.pause_audio();

        let status = wait_for(&handle, |status| {
            status.state == "ready"
                && status
                    .last_error
                    .as_deref()
                    .is_some_and(|error| error.contains("frame timeout"))
        });
        assert_eq!(status.audio.dropped_frames, 0);
        assert_eq!(mock.command_count(0x0D), 1);
        handle.send(Command::Shutdown).unwrap();
    }

    #[test]
    fn capability_timeout_retries_then_reports() {
        let (transport, mock) = MockTransport::new(MockConfig {
            caps: Vec::new(),
            ..MockConfig::default()
        });
        let mut config = DaemonConfig::new(RC003, mock_address(&transport));
        config.config.input = false;
        config.tick = Duration::from_millis(5);
        config.caps_timeout = Duration::from_millis(30);
        let (daemon, handle) = Daemon::new(
            config,
            Box::new(transport),
            Box::new(CountingSink { samples: 0 }),
            Arc::new(NullSink),
            None,
        );
        std::thread::spawn(move || daemon.run());

        wait_for(&handle, |status| {
            status.last_error.as_deref() == Some("CAPS_RESP timeout")
        });
        assert_eq!(mock.command_count(0x0A), 3);
        handle.send(Command::Shutdown).unwrap();
    }

    #[test]
    fn missing_input_is_degraded_not_fatal() {
        let (transport, mock) = MockTransport::new(MockConfig::default());
        let mut config = DaemonConfig::new(RC003, mock_address(&transport));
        config.config.input = false;
        let (daemon, handle) = Daemon::new(
            config,
            Box::new(transport),
            Box::new(CountingSink { samples: 0 }),
            Arc::new(NullSink),
            None,
        );
        std::thread::spawn(move || daemon.run());
        let status = wait_for(&handle, |status| status.state == "ready");
        assert!(!status.input.available);
        assert!(status.audio.available);
        let _ = mock;
        handle.send(Command::Shutdown).unwrap();
    }

    #[test]
    fn missing_pipewire_is_degraded_not_fatal() {
        let (transport, mock) = MockTransport::new(MockConfig::default());
        let mut config = DaemonConfig::new(RC003, mock_address(&transport));
        config.config.input = false;
        let (daemon, handle) = Daemon::new(
            config,
            Box::new(transport),
            Box::new(DegradedSink),
            Arc::new(NullSink),
            None,
        );
        std::thread::spawn(move || daemon.run());
        let status = wait_for(&handle, |status| status.state == "ready");
        assert!(!status.audio.available);
        assert_eq!(
            status.audio.degraded.as_deref(),
            Some("PipeWire is not available")
        );
        let _ = mock;
        handle.send(Command::Shutdown).unwrap();
    }
}
