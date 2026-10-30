// SPDX-License-Identifier: GPL-3.0-or-later

//! `atvvd`: ATVV session daemon and `org.atvv.Daemon` D-Bus boundary.

#![forbid(unsafe_code)]

pub mod audio;
pub mod capture;
pub mod cli;
pub mod daemon;
pub mod dbus;
pub mod exec;
pub mod status;
pub mod transport;
pub mod vinput;

pub use audio::{PcmSink, PipeWireSink};
pub use capture::{CaptureCell, CaptureState};
pub use daemon::{Command, Daemon, DaemonConfig, DaemonEvent, DaemonHandle, EventSink, NullSink};
pub use dbus::{Client, ClientError};
pub use exec::{ExecEngine, ExecResult};
pub use status::{
    AudioView, CapsView, ConfigCell, ConfigView, ContractView, DbusView, DeviceView, InputView,
    McpView, StatusCell, StatusView, VinputView,
};
pub use transport::{
    BluezTransport, MockConfig, MockHandle, MockTransport, Transport, TransportEvent,
};
pub use vinput::{VinputClient, VinputControl, VinputError};

pub use atvv_config::{Config, ConfigDocument, LoadedConfig, VinputMode};
