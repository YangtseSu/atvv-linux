// SPDX-License-Identifier: GPL-3.0-or-later

//! Stable D-Bus view model.
//!
//! Protocol types never cross the D-Bus boundary. `GetStatus` returns this
//! struct as JSON and the signals carry the same state names.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};

pub type StatusCell = Arc<Mutex<StatusView>>;

#[must_use]
pub fn cell(view: StatusView) -> StatusCell {
    Arc::new(Mutex::new(view))
}

pub fn lock(cell: &StatusCell) -> MutexGuard<'_, StatusView> {
    cell.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusView {
    pub state: String,
    pub connected: bool,
    pub streaming: bool,
    pub device: DeviceView,
    pub caps: Option<CapsView>,
    pub audio: AudioView,
    pub input: InputView,
    pub config: ConfigView,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceView {
    pub id: String,
    pub name: String,
    pub model: String,
    pub address: String,
    pub transport: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapsView {
    pub version: String,
    pub codec: String,
    pub codec_mask: u8,
    pub interaction: String,
    pub frame_size: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioView {
    pub source: String,
    pub available: bool,
    pub streaming: bool,
    pub queued_samples: usize,
    pub gain_db: i8,
    pub frames: u64,
    pub samples: u64,
    pub dropped_frames: u64,
    pub degraded: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputView {
    pub enabled: bool,
    pub available: bool,
    pub keyboard: bool,
    pub grabbed: Vec<String>,
    pub degraded: Vec<String>,
    pub last_button: Option<String>,
    pub voice_key: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigView {
    pub path: Option<String>,
    pub loaded: bool,
    pub gain_db: i8,
    pub input: bool,
}
