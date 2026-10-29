// SPDX-License-Identifier: GPL-3.0-or-later

//! Native PipeWire `Audio/Source` for decoded ATVV PCM.

#![forbid(unsafe_code)]

mod source;

pub use source::{AudioSource, AudioSourceConfig, AudioSourceError, PcmQueue};
