// SPDX-License-Identifier: GPL-3.0-or-later

//! Errors surfaced by the BlueZ transport.

use std::time::Duration;

use thiserror::Error;

pub type Result<T, E = BluezError> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum BluezError {
    #[error("BlueZ: {0}")]
    BlueZ(#[from] bluer::Error),
    #[error("invalid Bluetooth address {0:?}")]
    Address(String),
    #[error("no Bluetooth adapter is available")]
    NoAdapter,
    #[error("ATVV service {uuid} was not found; is the remote connected?")]
    MissingService { uuid: &'static str },
    #[error("ATVV characteristic {uuid} was not found")]
    MissingCharacteristic { uuid: &'static str },
    #[error("ATVV transmit characteristic {uuid} is not writable")]
    NotWritable { uuid: &'static str },
    #[error("{operation} timed out after {timeout:?}")]
    Timeout {
        operation: &'static str,
        timeout: Duration,
    },
    #[error("the BlueZ link is closed")]
    Closed,
    #[error("Bluetooth runtime: {0}")]
    Runtime(#[from] std::io::Error),
}
