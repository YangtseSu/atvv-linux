// SPDX-License-Identifier: GPL-3.0-or-later

//! `atvvd`: ATVV session daemon and `org.atvv.Daemon` D-Bus boundary.

#![forbid(unsafe_code)]

pub mod audio;
pub mod cli;
pub mod config;
pub mod daemon;
pub mod dbus;
pub mod status;
pub mod transport;
pub mod vinput;

pub use audio::{PcmSink, PipeWireSink};
pub use config::VinputMode;
pub use daemon::{Command, Daemon, DaemonConfig, DaemonEvent, DaemonHandle, EventSink, NullSink};
pub use dbus::{Client, ClientError};
pub use status::{
    AudioView, CapsView, ConfigView, DeviceView, InputView, StatusCell, StatusView, VinputView,
};
pub use transport::{
    BluezTransport, MockConfig, MockHandle, MockTransport, Transport, TransportEvent,
};
pub use vinput::{VinputClient, VinputControl, VinputError};
