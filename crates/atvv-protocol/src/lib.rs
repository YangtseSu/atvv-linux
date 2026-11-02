// SPDX-License-Identifier: GPL-3.0-or-later

//! ATVV v1.0 protocol primitives for the Linux userspace stack.
//!
//! This crate is deliberately free of BlueZ, PipeWire, and input-system
//! dependencies so the protocol can be tested on any machine.

#![forbid(unsafe_code)]

pub mod adpcm;
pub mod caps;
pub mod commands;
pub mod events;
pub mod frame;
pub mod gatt;
pub mod profile;
pub mod session;

pub use adpcm::Decoder;
pub use caps::{Capabilities, CapsError, Codec, InteractionModel, ProtocolVersion};
pub use events::{
    AudioStartReason, AudioStopReason, CtlEvent, SeqTracker, StreamId, mic_open_error_name,
};
pub use frame::{FrameAccumulator, FrameError};
pub use profile::{
    ButtonKey, CHROMECAST, DeviceProfile, GestureTiming, KeepaliveVariant, MicCloseVariant,
    MicOpenVariant, NibbleOrder, PROFILES, RC003, RemoteButton, find, find_by_ids,
};
pub use session::{Session, SessionAction, SessionState};
