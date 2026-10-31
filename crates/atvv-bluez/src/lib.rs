// SPDX-License-Identifier: GPL-3.0-or-later

//! BlueZ GATT transport for ATVV v1.0.
//!
//! BlueZ's Rust bindings are asynchronous while the daemon's session loop is
//! synchronous. This crate owns a small Tokio runtime: notification tasks push
//! events into a channel, and the synchronous [`BluezLink::poll`] drains it
//! without touching the runtime.
//!
//! Wakefulness of the remote is the user's: this crate never sends HID
//! reports and never opens raw BLE sockets.

#![forbid(unsafe_code)]

mod error;
mod hardware;
mod link;
mod pair;
mod probe;

pub use error::{BluezError, Result};
pub use hardware::HardwareInfo;
pub use link::{
    BluezConfig, BluezEvent, BluezLink, DEFAULT_CONNECT_TIMEOUT, DEFAULT_SERVICES_TIMEOUT,
    DEFAULT_WRITE_TIMEOUT, NotificationMode,
};
pub use pair::{DEFAULT_PAIR_TIMEOUT, PairConfig, PairReport, pair};
pub use probe::{AtvvInfo, CharacteristicInfo, ProbeReport, probe};

use bluer::{Address, Uuid};
use tokio::runtime::{Builder, Runtime};

/// Parse a Bluetooth address into BlueZ's form.
pub(crate) fn parse_address(text: &str) -> Result<Address> {
    text.parse::<Address>()
        .map_err(|_| BluezError::Address(text.to_string()))
}

/// ATVV UUID constants are fixed strings; a parse failure is a programming
/// error rather than a runtime condition.
pub(crate) fn atvv_uuid(text: &str) -> Uuid {
    Uuid::parse_str(text).expect("ATVV UUID constants are valid")
}

/// Build the private runtime used by the synchronous BlueZ wrappers.
///
/// One worker thread is enough: the runtime only services D-Bus calls and
/// notification sockets.
pub(crate) fn build_runtime() -> Result<Runtime> {
    Builder::new_multi_thread()
        .worker_threads(1)
        .thread_name("atvv-bluez")
        .enable_all()
        .build()
        .map_err(BluezError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_case_insensitive_addresses() {
        let upper = parse_address("C0:5D:39:F8:2D:4C").unwrap();
        let lower = parse_address("c0:5d:39:f8:2d:4c").unwrap();
        assert_eq!(upper, lower);
        assert_eq!(upper.to_string(), "C0:5D:39:F8:2D:4C");
    }

    #[test]
    fn rejects_malformed_addresses() {
        for text in [
            "",
            "C0:5D:39:F8:2D",
            "C0:5D:39:F8:2D:4C:00",
            "zz:00:00:00:00:00",
        ] {
            assert!(matches!(parse_address(text), Err(BluezError::Address(_))));
        }
    }
}
