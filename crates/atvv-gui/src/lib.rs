// SPDX-License-Identifier: GPL-3.0-or-later

#![forbid(unsafe_code)]

//! `atvv-config`: GTK4/libadwaita configuration client for `atvvd`.
//!
//! The GUI is a separate D-Bus client. It never opens evdev/uinput, BlueZ, or
//! PipeWire, and it never writes `daemon.toml`; `atvv-gui`'s worker calls
//! `GetConfig`/`SetConfig` and the capture API.

pub mod actions;
pub mod app;
pub mod client;
pub mod diagram;
pub mod i18n;
pub mod keylearn;
pub mod model;
