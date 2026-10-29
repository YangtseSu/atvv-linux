// SPDX-License-Identifier: GPL-3.0-or-later

//! PCM sinks for decoded ATVV audio.
//!
//! Production uses the resident PipeWire source. The daemon treats a missing
//! PipeWire as degraded: the session still runs and the source state is
//! reported over D-Bus.

use atvv_audio::{AudioSource, AudioSourceConfig};

/// One destination for decoded PCM.
pub trait PcmSink: Send {
    fn push(&mut self, samples: &[i16]);
    fn name(&self) -> &str;
    fn is_available(&self) -> bool;
    fn queued_samples(&self) -> usize;
    fn degraded(&self) -> Option<&str>;
}

/// Native PipeWire `Audio/Source`, resident for the daemon lifetime.
pub struct PipeWireSink {
    name: String,
    source: Option<AudioSource>,
    degraded: Option<String>,
}

impl PipeWireSink {
    pub fn start(config: AudioSourceConfig) -> Self {
        let name = config.name.clone();
        match AudioSource::start(config) {
            Ok(source) => Self {
                name,
                source: Some(source),
                degraded: None,
            },
            Err(error) => Self {
                name,
                source: None,
                degraded: Some(error.to_string()),
            },
        }
    }
}

impl PcmSink for PipeWireSink {
    fn push(&mut self, samples: &[i16]) {
        if let Some(source) = &self.source {
            source.push(samples);
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn is_available(&self) -> bool {
        self.source.is_some()
    }

    fn queued_samples(&self) -> usize {
        self.source.as_ref().map_or(0, AudioSource::queue_len)
    }

    fn degraded(&self) -> Option<&str> {
        self.degraded.as_deref()
    }
}
