// SPDX-License-Identifier: GPL-3.0-or-later

//! fcitx5-vinput D-Bus integration.
//!
//! `vinput = "dbus"` drives recording through `org.fcitx.Vinput` instead of
//! synthesizing a trigger key. The service is optional: a missing or failing
//! `vinput-daemon` degrades the trigger path and is reported in the status
//! view; it never stops the ATVV session.
//!
//! Interface snapshot: `xifan2333/fcitx5-vinput` `d9a409d`
//! `src/common/dbus/dbus_interface.h`, `src/daemon/runtime/dbus_service.cpp`.
//! `StartRecording()` takes no arguments and returns nothing; `StopRecording`
//! takes a scene id and returns the transcript. Start does not expose a scene
//! id, so the external path passes the empty id, matching `vinput`'s own CLI.

use std::time::Duration;

use thiserror::Error;
use zbus::Connection;

/// Well-known bus name of `vinput-daemon`.
pub const SERVICE: &str = "org.fcitx.Vinput";
/// Object path of the D-Bus service.
pub const PATH: &str = "/org/fcitx/Vinput";
/// Service interface.
pub const INTERFACE: &str = "org.fcitx.Vinput.Service";

/// Method timeout: a stuck vinput must not stall the ATVV event loop.
const METHOD_TIMEOUT: Duration = Duration::from_secs(2);

/// One recording control failure. Kept string-based so the daemon status can
/// carry it without exposing zbus types.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("vinput {operation} failed: {message}")]
pub struct VinputError {
    pub operation: &'static str,
    pub message: String,
}

/// Recording control for the fcitx5-vinput daemon.
pub trait VinputControl: Send {
    fn start(&mut self) -> Result<(), VinputError>;
    fn stop(&mut self) -> Result<(), VinputError>;
    /// Whether the last call reached the service.
    fn available(&self) -> bool;
    /// Last failure, kept for `GetStatus`.
    fn last_error(&self) -> Option<&str>;
}

#[zbus::proxy(
    interface = "org.fcitx.Vinput.Service",
    default_service = "org.fcitx.Vinput",
    default_path = "/org/fcitx/Vinput"
)]
trait VinputApi {
    fn start_recording(&self) -> zbus::Result<()>;
    fn stop_recording(&self, scene_id: &str) -> zbus::Result<String>;
}

/// Lazily connected client.
///
/// The session bus may not be reachable when the daemon starts; connecting on
/// the first voice-key edge lets the daemon start regardless and retry later.
pub struct VinputClient {
    connection: Option<Connection>,
    available: bool,
    last_error: Option<String>,
}

impl Default for VinputClient {
    fn default() -> Self {
        Self::new()
    }
}

impl VinputClient {
    #[must_use]
    pub fn new() -> Self {
        Self {
            connection: None,
            available: false,
            last_error: None,
        }
    }

    /// Use an existing connection; used by integration tests.
    #[must_use]
    pub fn with_connection(connection: Connection) -> Self {
        Self {
            connection: Some(connection),
            available: false,
            last_error: None,
        }
    }

    fn connection(&mut self) -> zbus::Result<&Connection> {
        if self.connection.is_none() {
            let connection = zbus::block_on(async {
                zbus::connection::Builder::session()
                    .method_timeout(METHOD_TIMEOUT)
                    .build()
                    .await
            })?;
            self.connection = Some(connection);
        }
        Ok(self.connection.as_ref().expect("connection was just set"))
    }

    fn call<T>(
        &mut self,
        operation: &'static str,
        f: impl FnOnce(&Connection) -> zbus::Result<T>,
    ) -> Result<T, VinputError> {
        let result = self.dispatch(f);
        match result {
            Ok(value) => {
                self.available = true;
                self.last_error = None;
                Ok(value)
            }
            Err(error) => {
                self.available = false;
                let message = error.to_string();
                self.last_error = Some(message.clone());
                Err(VinputError { operation, message })
            }
        }
    }

    /// Check name ownership first so an absent `vinput-daemon` fails at once
    /// instead of waiting for the method timeout.
    fn dispatch<T>(&mut self, f: impl FnOnce(&Connection) -> zbus::Result<T>) -> zbus::Result<T> {
        let connection = self.connection()?;
        let name = zbus::names::BusName::try_from(SERVICE)
            .map_err(|error| zbus::Error::Failure(error.to_string()))?;
        let present = zbus::block_on(async {
            let bus = zbus::fdo::DBusProxy::new(connection).await?;
            bus.name_has_owner(name).await
        })?;
        if !present {
            return Err(zbus::Error::Failure(format!("{SERVICE} is not running")));
        }
        f(connection)
    }
}

impl VinputControl for VinputClient {
    fn start(&mut self) -> Result<(), VinputError> {
        self.call("StartRecording", |connection| {
            zbus::block_on(async {
                let proxy = VinputApiProxy::new(connection).await?;
                proxy.start_recording().await
            })
        })
    }

    fn stop(&mut self) -> Result<(), VinputError> {
        self.call("StopRecording", |connection| {
            zbus::block_on(async {
                let proxy = VinputApiProxy::new(connection).await?;
                proxy.stop_recording("").await
            })
            .map(|_transcript| ())
        })
    }

    fn available(&self) -> bool {
        self.available
    }

    fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }
}
