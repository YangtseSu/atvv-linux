// SPDX-License-Identifier: GPL-3.0-or-later

//! Native PipeWire `Audio/Source` for decoded ATVV PCM.

#![forbid(unsafe_code)]

mod resample;
mod source;

pub use resample::Resampler;
pub use source::{
    AudioSource, AudioSourceConfig, AudioSourceError, CHANNELS, CONTRACT_VERSION, MEDIA_CLASS,
    PcmQueue, RECORDING_KEYS, SAMPLE_FORMAT, contract_properties,
};
