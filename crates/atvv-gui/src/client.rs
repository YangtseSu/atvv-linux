// SPDX-License-Identifier: GPL-3.0-or-later

//! D-Bus worker for the GUI.
//!
//! All calls run on one worker thread so the GTK main loop never blocks on the
//! daemon. Results and `ButtonSignal` signals are delivered over a channel the
//! window drains with a short main-loop timer.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use atvv_config::ConfigDocument;
use futures_util::StreamExt;
use serde::Deserialize;
use zbus::{Connection, connection};

pub const SERVICE: &str = "org.atvv.Daemon";
pub const PATH: &str = "/org/atvv/Daemon";
pub const INTERFACE: &str = "org.atvv.Daemon";

const METHOD_TIMEOUT: Duration = Duration::from_secs(10);

/// Subset of `GetStatus` the window shows.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct DaemonStatus {
    pub state: String,
    pub connected: bool,
    pub streaming: bool,
    pub last_error: Option<String>,
    pub device: DeviceInfo,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct DeviceInfo {
    pub name: String,
    pub address: String,
}

/// One logical button event forwarded by the daemon during capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonSignal {
    pub button: String,
    pub trigger: String,
    pub pressed: bool,
}

/// Work sent to the worker thread.
#[derive(Debug)]
pub enum Request {
    Load,
    Status,
    Save(ConfigDocument),
    BeginCapture,
    EndCapture,
    RunAction(String),
    Shutdown,
}

/// Result sent back to the main loop.
#[derive(Debug)]
pub enum Event {
    Loaded {
        document: ConfigDocument,
        status: DaemonStatus,
    },
    LoadFailed(String),
    Status(Result<DaemonStatus, String>),
    Saved(Result<(), String>),
    CaptureStarted(Result<(), String>),
    CaptureEnded(Result<(), String>),
    ActionRun(Result<(), String>),
    Button(ButtonSignal),
    SignalStreamEnded,
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
    fn run_action(&self, action: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn button_event(&self, button: &str, trigger: &str, pressed: bool) -> zbus::Result<()>;
}

/// Connection to `org.atvv.Daemon`; created on the worker thread.
struct DaemonClient {
    connection: Connection,
}

impl DaemonClient {
    fn connect(address: Option<&str>) -> Result<Self, String> {
        let connection = zbus::block_on(async {
            let builder = match address {
                Some(address) => connection::Builder::address(address),
                None => connection::Builder::session(),
            };
            builder.method_timeout(METHOD_TIMEOUT).build().await
        })
        .map_err(|error| format!("cannot connect to the session bus: {error}"))?;
        Ok(Self { connection })
    }

    fn proxy(&self) -> Result<DaemonApiProxy<'_>, String> {
        zbus::block_on(DaemonApiProxy::new(&self.connection)).map_err(|error| error.to_string())
    }

    fn load(&self) -> Result<(ConfigDocument, DaemonStatus), String> {
        let proxy = self.proxy()?;
        let raw = zbus::block_on(proxy.get_config()).map_err(|error| error.to_string())?;
        let document: ConfigDocument = serde_json::from_str(&raw)
            .map_err(|error| format!("invalid configuration document: {error}"))?;
        let status = self.status()?;
        Ok((document, status))
    }

    fn status(&self) -> Result<DaemonStatus, String> {
        let raw = zbus::block_on(self.proxy()?.get_status()).map_err(|error| error.to_string())?;
        serde_json::from_str(&raw).map_err(|error| format!("invalid status document: {error}"))
    }

    fn save(&self, document: &ConfigDocument) -> Result<(), String> {
        let json = serde_json::to_string(document).map_err(|error| error.to_string())?;
        zbus::block_on(self.proxy()?.set_config(&json)).map_err(|error| error.to_string())
    }

    fn begin_capture(&self) -> Result<(), String> {
        zbus::block_on(self.proxy()?.begin_capture()).map_err(|error| error.to_string())
    }

    fn end_capture(&self) -> Result<(), String> {
        zbus::block_on(self.proxy()?.end_capture()).map_err(|error| error.to_string())
    }

    fn run_action(&self, action: &str) -> Result<(), String> {
        zbus::block_on(self.proxy()?.run_action(action)).map_err(|error| error.to_string())
    }
}

/// Handle to the worker; commands are queued and never block the caller.
pub struct Backend {
    requests: Sender<Request>,
    events: Receiver<Event>,
}

impl Backend {
    /// Start the worker. `address` overrides the session bus (tests).
    #[must_use]
    pub fn start(address: Option<String>) -> Self {
        let (requests, request_rx) = mpsc::channel();
        let (event_tx, events) = mpsc::channel();
        thread::Builder::new()
            .name("atvv-gui-dbus".to_string())
            .spawn(move || worker(address.as_deref(), &request_rx, &event_tx))
            .expect("spawn the D-Bus worker");
        Self { requests, events }
    }

    pub fn load(&self) {
        let _ = self.requests.send(Request::Load);
    }

    pub fn refresh_status(&self) {
        let _ = self.requests.send(Request::Status);
    }

    pub fn save(&self, document: ConfigDocument) {
        let _ = self.requests.send(Request::Save(document));
    }

    pub fn begin_capture(&self) {
        let _ = self.requests.send(Request::BeginCapture);
    }

    pub fn end_capture(&self) {
        let _ = self.requests.send(Request::EndCapture);
    }

    pub fn run_action(&self, action: String) {
        let _ = self.requests.send(Request::RunAction(action));
    }

    pub fn stop(&self) {
        let _ = self.requests.send(Request::Shutdown);
    }

    /// Drain one result, if any. Called from the GTK main loop timer.
    pub fn try_recv(&self) -> Option<Event> {
        self.events.try_recv().ok()
    }

    /// Blocking receive for tests.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<Event, mpsc::RecvTimeoutError> {
        self.events.recv_timeout(timeout)
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        self.stop();
    }
}

fn worker(address: Option<&str>, requests: &Receiver<Request>, events: &Sender<Event>) {
    let mut client: Option<DaemonClient> = None;
    let mut listener_started = false;
    while let Ok(request) = requests.recv() {
        if matches!(request, Request::Shutdown) {
            break;
        }
        if client.is_none() {
            match DaemonClient::connect(address) {
                Ok(connected) => {
                    if !listener_started {
                        spawn_signal_listener(&connected, events);
                        listener_started = true;
                    }
                    client = Some(connected);
                }
                Err(error) => {
                    send_failure(&request, events, &error);
                    continue;
                }
            }
        }
        let Some(active) = client.as_ref() else {
            continue;
        };
        match request {
            Request::Load => match active.load() {
                Ok((document, status)) => {
                    let _ = events.send(Event::Loaded { document, status });
                }
                Err(error) => {
                    let _ = events.send(Event::LoadFailed(error));
                    client = None;
                }
            },
            Request::Status => {
                let result = active.status();
                let failed = result.is_err();
                let _ = events.send(Event::Status(result));
                if failed {
                    client = None;
                }
            }
            Request::Save(document) => {
                let result = active.save(&document);
                let failed = result.is_err();
                let _ = events.send(Event::Saved(result));
                if failed {
                    client = None;
                }
            }
            Request::BeginCapture => {
                let result = active.begin_capture();
                let failed = result.is_err();
                let _ = events.send(Event::CaptureStarted(result));
                if failed {
                    client = None;
                }
            }
            Request::EndCapture => {
                let result = active.end_capture();
                let failed = result.is_err();
                let _ = events.send(Event::CaptureEnded(result));
                if failed {
                    client = None;
                }
            }
            Request::RunAction(action) => {
                let result = active.run_action(&action);
                let failed = result.is_err();
                let _ = events.send(Event::ActionRun(result));
                if failed {
                    client = None;
                }
            }
            Request::Shutdown => unreachable!(),
        }
    }
}

fn send_failure(request: &Request, events: &Sender<Event>, error: &str) {
    match request {
        Request::Load => {
            let _ = events.send(Event::LoadFailed(error.to_string()));
        }
        Request::Status => {
            let _ = events.send(Event::Status(Err(error.to_string())));
        }
        Request::Save(_) => {
            let _ = events.send(Event::Saved(Err(error.to_string())));
        }
        Request::BeginCapture => {
            let _ = events.send(Event::CaptureStarted(Err(error.to_string())));
        }
        Request::EndCapture => {
            let _ = events.send(Event::CaptureEnded(Err(error.to_string())));
        }
        Request::RunAction(_) => {
            let _ = events.send(Event::ActionRun(Err(error.to_string())));
        }
        Request::Shutdown => {}
    }
}

fn spawn_signal_listener(client: &DaemonClient, events: &Sender<Event>) {
    let connection = client.connection.clone();
    let events = events.clone();
    let _ = thread::Builder::new()
        .name("atvv-gui-signals".to_string())
        .spawn(move || {
            zbus::block_on(async {
                let Ok(proxy) = DaemonApiProxy::new(&connection).await else {
                    let _ = events.send(Event::SignalStreamEnded);
                    return;
                };
                let Ok(mut stream) = proxy.receive_button_event().await else {
                    let _ = events.send(Event::SignalStreamEnded);
                    return;
                };
                while let Some(event) = stream.next().await {
                    let Ok(args) = event.args() else {
                        continue;
                    };
                    let button = args.button().to_string();
                    let trigger = args.trigger().to_string();
                    let pressed = *args.pressed();
                    if events
                        .send(Event::Button(ButtonSignal {
                            button,
                            trigger,
                            pressed,
                        }))
                        .is_err()
                    {
                        return;
                    }
                }
                let _ = events.send(Event::SignalStreamEnded);
            });
        });
}
