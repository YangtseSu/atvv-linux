// SPDX-License-Identifier: GPL-3.0-or-later

//! `org.atvv.Daemon` service and client.
//!
//! The service exposes a JSON view model and signals, not protocol structs.
//! The client is the only place that knows D-Bus member names.

use std::sync::mpsc::Sender;

use atvv_protocol::profile::DeviceProfile;
use serde_json::Value;
use zbus::blocking::Connection;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::ObjectPath;

use crate::daemon::{Command, DaemonEvent, EventSink};
use crate::status::{StatusCell, StatusView};

pub const SERVICE: &str = "org.atvv.Daemon";
pub const PATH: &str = "/org/atvv/Daemon";
pub const INTERFACE: &str = "org.atvv.Daemon";

/// Object served at [`PATH`].
pub struct DaemonInterface {
    status: StatusCell,
    commands: Sender<Command>,
    address: String,
}

impl DaemonInterface {
    #[must_use]
    pub fn new(status: StatusCell, commands: Sender<Command>, address: String) -> Self {
        Self {
            status,
            commands,
            address,
        }
    }

    fn send(&self, command: Command) -> zbus::fdo::Result<()> {
        self.commands
            .send(command)
            .map_err(|_| zbus::fdo::Error::Failed("the daemon is not running".to_string()))
    }
}

#[zbus::interface(name = "org.atvv.Daemon")]
impl DaemonInterface {
    /// Full view model as JSON.
    fn get_status(&self) -> String {
        let status = crate::status::lock(&self.status).clone();
        serde_json::to_string(&status).unwrap_or_else(|_| "{}".to_string())
    }

    /// Configured device addresses.
    fn list_devices(&self) -> Vec<String> {
        vec![self.address.clone()]
    }

    /// Open the remote microphone with `MIC_OPEN`.
    fn start_voice(&self) -> zbus::fdo::Result<()> {
        self.send(Command::StartVoice)
    }

    /// Close the remote microphone with `MIC_CLOSE`.
    fn stop_voice(&self) -> zbus::fdo::Result<()> {
        self.send(Command::StopVoice)
    }

    /// Start voice when idle, stop it when opening/streaming.
    fn toggle_voice(&self) -> zbus::fdo::Result<()> {
        self.send(Command::ToggleVoice)
    }

    /// Re-read `daemon.toml`.
    fn reload_config(&self) -> zbus::fdo::Result<()> {
        self.send(Command::ReloadConfig)
    }

    #[zbus(signal)]
    async fn state_changed(emitter: &SignalEmitter<'_>, state: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn voice_started(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn voice_stopped(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}

/// Emits daemon events on the session bus.
pub struct DbusSink {
    emitter: SignalEmitter<'static>,
}

impl DbusSink {
    pub fn new(connection: &Connection) -> zbus::Result<Self> {
        let path =
            ObjectPath::try_from(PATH).map_err(|error| zbus::Error::Failure(error.to_string()))?;
        Ok(Self {
            emitter: SignalEmitter::new(connection.inner(), path)?.into_owned(),
        })
    }
}

impl EventSink for DbusSink {
    fn publish(&self, event: DaemonEvent) {
        let result = match event {
            DaemonEvent::StateChanged(state) => {
                zbus::block_on(DaemonInterface::state_changed(&self.emitter, &state))
            }
            DaemonEvent::VoiceStarted => {
                zbus::block_on(DaemonInterface::voice_started(&self.emitter))
            }
            DaemonEvent::VoiceStopped => {
                zbus::block_on(DaemonInterface::voice_stopped(&self.emitter))
            }
        };
        if let Err(error) = result {
            eprintln!("atvvd: D-Bus signal failed: {error}");
        }
    }
}

/// Connect to the session bus.
pub fn connect_session() -> zbus::Result<Connection> {
    Connection::session()
}

/// Register the object and then claim `org.atvv.Daemon`.
///
/// The name is claimed after the object exists so a client that sees the name
/// can never send a method call into an empty object path.
pub fn serve(connection: &Connection, interface: DaemonInterface) -> zbus::Result<()> {
    connection.object_server().at(PATH, interface)?;
    connection.request_name(SERVICE)?;
    Ok(())
}

#[zbus::proxy(
    interface = "org.atvv.Daemon",
    default_service = "org.atvv.Daemon",
    default_path = "/org/atvv/Daemon"
)]
pub trait DaemonApi {
    fn get_status(&self) -> zbus::Result<String>;
    fn list_devices(&self) -> zbus::Result<Vec<String>>;
    fn start_voice(&self) -> zbus::Result<()>;
    fn stop_voice(&self) -> zbus::Result<()>;
    fn toggle_voice(&self) -> zbus::Result<()>;
    fn reload_config(&self) -> zbus::Result<()>;

    #[zbus(signal)]
    fn state_changed(&self, state: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn voice_started(&self) -> zbus::Result<()>;

    #[zbus(signal)]
    fn voice_stopped(&self) -> zbus::Result<()>;
}

/// Blocking client for the daemon API.
pub struct Client {
    connection: Connection,
}

impl Client {
    pub fn connect() -> Result<Self, ClientError> {
        let connection = zbus::blocking::connection::Builder::session()?
            .method_timeout(std::time::Duration::from_secs(10))
            .build()?;
        Ok(Self { connection })
    }

    #[must_use]
    pub fn with_connection(connection: Connection) -> Self {
        Self { connection }
    }

    #[must_use]
    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    pub fn proxy(&self) -> Result<DaemonApiProxyBlocking<'_>, ClientError> {
        Ok(DaemonApiProxyBlocking::new(&self.connection)?)
    }

    pub fn status(&self) -> Result<StatusView, ClientError> {
        let raw = self.proxy()?.get_status()?;
        Ok(serde_json::from_str(&raw)?)
    }

    pub fn status_json(&self) -> Result<Value, ClientError> {
        let raw = self.proxy()?.get_status()?;
        Ok(serde_json::from_str(&raw)?)
    }

    pub fn devices(&self) -> Result<Vec<String>, ClientError> {
        Ok(self.proxy()?.list_devices()?)
    }

    pub fn start_voice(&self) -> Result<(), ClientError> {
        Ok(self.proxy()?.start_voice()?)
    }

    pub fn stop_voice(&self) -> Result<(), ClientError> {
        Ok(self.proxy()?.stop_voice()?)
    }

    pub fn toggle_voice(&self) -> Result<(), ClientError> {
        Ok(self.proxy()?.toggle_voice()?)
    }

    pub fn reload(&self) -> Result<(), ClientError> {
        Ok(self.proxy()?.reload_config()?)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error(transparent)]
    DBus(#[from] zbus::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Used by the binary to build a PipeWire source name for a profile.
#[must_use]
pub fn source_name(profile: DeviceProfile) -> String {
    format!("atvv-{}", profile.id)
}
