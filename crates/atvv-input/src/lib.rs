// SPDX-License-Identifier: GPL-3.0-or-later

//! evdev input path for ATVV remotes: discovery, `EVIOCGRAB`, gesture
//! recognition, and a uinput virtual keyboard.
//!
//! The crate is synchronous by design: each device has a reader thread, the
//! gesture engine has its own clock-driven thread, and the uinput writer is a
//! blocking thread fed by an unbounded channel. A daemon wraps
//! [`InputRuntime`] and consumes [`RemoteEvent`]s.

#![forbid(unsafe_code)]

pub mod action;
pub mod device;
pub mod fake;
pub mod gesture;
pub mod keymap;
pub mod runtime;
pub mod vkbd;

pub use action::{
    Action, ActionError, ActionMap, ButtonActions, DictationMode, Trigger, parse_action,
};
pub use device::{
    Discovery, MatchQuality, MatchedDevice, SkipReason, SkippedDevice, discover, match_quality,
};
pub use fake::{FAKE_REMOTE, FakeRemote, FakeRemoteError};
pub use gesture::{Gesture, GestureEngine, Policy, Timing};
pub use keymap::{KeyMap, KeyMapError, KeyProgram, KeyResolver, resolve_key};
pub use runtime::{InputError, InputRuntime, RemoteEvent};
pub use vkbd::{
    KeyStep, KeyboardError, VirtualKeyboard, hold_frame, key_events, release_frame, tap_frames,
};
