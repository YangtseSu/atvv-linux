// SPDX-License-Identifier: GPL-3.0-or-later

//! `org.atvv.Daemon` service and client.
//!
//! The service exposes JSON view models and signals, not protocol structs.
//! The client is the only place that knows D-Bus member names.

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use atvv_config::ConfigDocument;
use atvv_protocol::profile::DeviceProfile;
use serde_json::Value;
use zbus::blocking::Connection;
use zbus::message::Header;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::ObjectPath;

use crate::capture::{self, CaptureCell, CaptureState};
use crate::daemon::{Command, DaemonEvent, EventSink};
use crate::status::{ConfigCell, StatusCell, StatusView, lock_config};

pub const SERVICE: &str = "org.atvv.Daemon";
pub const PATH: &str = "/org/atvv/Daemon";
pub const INTERFACE: &str = "org.atvv.Daemon";

/// How long a `SetConfig` caller waits for the daemon loop to apply it.
const CONFIG_TIMEOUT: Duration = Duration::from_secs(10);

/// Object served at [`PATH`].
pub struct DaemonInterface {
    status: StatusCell,
    config: ConfigCell,
    capture: CaptureCell,
    commands: Sender<Command>,
    address: String,
}

impl DaemonInterface {
    #[must_use]
    pub fn new(
        status: StatusCell,
        config: ConfigCell,
        capture: CaptureCell,
        commands: Sender<Command>,
        address: String,
    ) -> Self {
        Self {
            status,
            config,
            capture,
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

    /// Effective editable configuration as JSON.
    fn get_config(&self) -> String {
        let config = lock_config(&self.config).clone();
        serde_json::to_string(&config).unwrap_or_else(|_| "{}".to_string())
    }

    /// Validate, apply, and persist a configuration document.
    fn set_config(&self, json: &str) -> zbus::fdo::Result<()> {
        let (reply, result) = mpsc::channel();
        self.commands
            .send(Command::SetConfig {
                json: json.to_string(),
                reply,
            })
            .map_err(|_| zbus::fdo::Error::Failed("the daemon is not running".to_string()))?;
        wait(result)
    }

    /// Start reporting logical button events to the caller.
    fn begin_capture(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> zbus::fdo::Result<()> {
        let owner = header.sender().map(ToString::to_string);
        capture::begin(&self.capture, owner.clone());
        if let Some(owner) = owner {
            let cell = Arc::clone(&self.capture);
            let connection = connection.clone();
            std::thread::spawn(move || watch_owner(connection, owner, cell));
        }
        Ok(())
    }

    /// Stop capture mode.
    fn end_capture(&self) -> zbus::fdo::Result<()> {
        capture::end(&self.capture);
        Ok(())
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

    /// Re-read `daemon.toml`; returns the load or apply error.
    fn reload_config(&self) -> zbus::fdo::Result<()> {
        let (reply, result) = mpsc::channel();
        self.commands
            .send(Command::ReloadConfig { reply })
            .map_err(|_| zbus::fdo::Error::Failed("the daemon is not running".to_string()))?;
        wait(result)
    }

    #[zbus(signal)]
    async fn state_changed(emitter: &SignalEmitter<'_>, state: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn voice_started(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn voice_stopped(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn button_event(
        emitter: &SignalEmitter<'_>,
        button: &str,
        trigger: &str,
        pressed: bool,
    ) -> zbus::Result<()>;
}

/// Wait for a daemon reply to a configuration command.
fn wait(result: Receiver<Result<(), String>>) -> zbus::fdo::Result<()> {
    match result.recv_timeout(CONFIG_TIMEOUT) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(message)) => Err(zbus::fdo::Error::InvalidArgs(message)),
        Err(_) => Err(zbus::fdo::Error::Failed(
            "the daemon did not answer the configuration request".to_string(),
        )),
    }
}

/// Clear capture mode when the client that started it leaves the bus.
///
/// The owner is a unique bus name, so `NameHasOwner` is a direct check and
/// avoids depending on match-rule argument filtering.
fn watch_owner(connection: zbus::Connection, owner: String, cell: CaptureCell) {
    let connection = zbus::blocking::Connection::from(connection);
    let Ok(proxy) = zbus::blocking::fdo::DBusProxy::new(&connection) else {
        return;
    };
    loop {
        {
            let state = capture::lock(&cell);
            if !state.active || state.owner.as_deref() != Some(owner.as_str()) {
                return;
            }
        }
        let Ok(name) = zbus::names::BusName::try_from(owner.as_str()) else {
            return;
        };
        match proxy.name_has_owner(name) {
            Ok(true) => {}
            Ok(false) => break,
            Err(_) => return,
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let mut state = capture::lock(&cell);
    if state.active && state.owner.as_deref() == Some(owner.as_str()) {
        *state = CaptureState::default();
    }
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
            DaemonEvent::ButtonEvent {
                button,
                trigger,
                pressed,
            } => zbus::block_on(DaemonInterface::button_event(
                &self.emitter,
                &button,
                &trigger,
                pressed,
            )),
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
    fn get_config(&self) -> zbus::Result<String>;
    fn set_config(&self, json: &str) -> zbus::Result<()>;
    fn begin_capture(&self) -> zbus::Result<()>;
    fn end_capture(&self) -> zbus::Result<()>;
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

    #[zbus(signal)]
    fn button_event(&self, button: &str, trigger: &str, pressed: bool) -> zbus::Result<()>;
}

/// Blocking client for the daemon API.
pub struct Client {
    connection: Connection,
}

impl Client {
    pub fn connect() -> Result<Self, ClientError> {
        let connection = zbus::blocking::connection::Builder::session()?
            .method_timeout(Duration::from_secs(10))
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

    pub fn config(&self) -> Result<ConfigDocument, ClientError> {
        let raw = self.proxy()?.get_config()?;
        Ok(serde_json::from_str(&raw)?)
    }

    pub fn set_config(&self, document: &ConfigDocument) -> Result<(), ClientError> {
        let json = serde_json::to_string(document)?;
        Ok(self.proxy()?.set_config(&json)?)
    }

    pub fn begin_capture(&self) -> Result<(), ClientError> {
        Ok(self.proxy()?.begin_capture()?)
    }

    pub fn end_capture(&self) -> Result<(), ClientError> {
        Ok(self.proxy()?.end_capture()?)
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
