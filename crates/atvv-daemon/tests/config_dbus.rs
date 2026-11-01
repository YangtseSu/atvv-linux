// SPDX-License-Identifier: GPL-3.0-or-later

//! Isolated-bus tests for the configuration and capture D-Bus API.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use atvv_config::{ConfigDocument, LayoutDocument};
use atvv_daemon::Transport;
use atvv_daemon::audio::PcmSink;
use atvv_daemon::capture;
use atvv_daemon::daemon::{Command as DaemonCommand, Daemon, DaemonConfig, EventSink};
use atvv_daemon::dbus::{self, Client, DaemonInterface, DbusSink};
use atvv_daemon::status::StatusView;
use atvv_daemon::transport::{MockConfig, MockTransport};
use atvv_protocol::profile::{RC003, RemoteButton};
use futures_util::StreamExt;
use zbus::Connection;
use zbus::connection::Builder;

const ADDRESS: &str = "02:00:00:00:00:01";

struct TestBus {
    child: Child,
    address: String,
}

impl TestBus {
    fn start() -> Self {
        let mut child = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("dbus-daemon is required for the integration test");
        let stdout = child.stdout.take().expect("dbus-daemon stdout");
        let mut line = String::new();
        BufReader::new(stdout)
            .read_line(&mut line)
            .expect("dbus-daemon address");
        let address = line.trim().to_string();
        assert!(
            address.starts_with("unix:"),
            "unexpected bus address: {address}"
        );
        Self { child, address }
    }

    fn connect(&self) -> Connection {
        zbus::block_on(Builder::address(self.address.as_str()).build()).expect("connection")
    }
}

impl Drop for TestBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct CountingSink;

impl PcmSink for CountingSink {
    fn push(&mut self, _samples: &[i16]) {}

    fn name(&self) -> &str {
        "counting"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn queued_samples(&self) -> usize {
        0
    }

    fn degraded(&self) -> Option<&str> {
        None
    }
}

fn config_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "atvv-config-dbus-{name}-{}.toml",
        std::process::id()
    ))
}

fn wait_status(client: &Client, predicate: impl Fn(&StatusView) -> bool) -> StatusView {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let status = client.status().expect("daemon status");
        if predicate(&status) {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for the daemon; last status: {status:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

struct Fixture {
    server: zbus::Connection,
    client: Client,
    handle: atvv_daemon::DaemonHandle,
    thread: Option<std::thread::JoinHandle<()>>,
    path: PathBuf,
}

impl Fixture {
    fn start(bus: &TestBus, name: &str) -> Self {
        let server = bus.connect();
        let (transport, _mock) = MockTransport::new(MockConfig {
            address: ADDRESS.to_string(),
            ..MockConfig::default()
        });
        let path = config_path(name);
        let _ = std::fs::remove_file(&path);
        let mut config = DaemonConfig::new(RC003, ADDRESS);
        config.config.input = false;
        config.config_path = path.clone();
        let address = transport.address().to_string();

        let signals: Arc<dyn EventSink> = Arc::new(DbusSink::new(&server).expect("signal sink"));
        let (daemon, handle) = Daemon::new(
            config,
            Box::new(transport),
            Box::new(CountingSink),
            signals,
            None,
        );
        let interface = DaemonInterface::new(
            handle.status(),
            handle.config(),
            handle.capture(),
            handle.commands(),
            address,
        );
        dbus::serve(&server, interface).expect("serve org.atvv.Daemon");
        let thread = std::thread::spawn(move || daemon.run());
        let client = Client::with_connection(bus.connect());
        wait_status(&client, |status| status.state == "ready");
        Self {
            server,
            client,
            handle,
            thread: Some(thread),
            path,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.handle.send(DaemonCommand::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let _ = std::fs::remove_file(&self.path);
    }
}

#[test]
fn config_round_trip_persists_and_invalid_documents_do_not_apply() {
    let bus = TestBus::start();
    let fixture = Fixture::start(&bus, "round-trip");

    let connection = fixture.client.connection();
    let reply = zbus::block_on(connection.call_method(
        Some(dbus::SERVICE),
        dbus::PATH,
        Some("org.freedesktop.DBus.Introspectable"),
        "Introspect",
        &(),
    ))
    .expect("introspection");
    let xml: String = reply.body().deserialize().expect("introspection XML");
    for member in [
        "GetConfig",
        "SetConfig",
        "SetLayout",
        "LayoutChanged",
        "BeginCapture",
        "EndCapture",
        "ButtonEvent",
        "RunAction",
    ] {
        assert!(xml.contains(member), "{member} missing from introspection");
    }

    let mut document = fixture.client.config().expect("GetConfig");
    assert_eq!(document.schema_version, atvv_config::SCHEMA_VERSION);
    assert_eq!(document.active_layout, atvv_config::DEFAULT_LAYOUT);
    assert!(document.layouts.is_empty());
    document.gain_db = 6;
    let mut layout = LayoutDocument {
        name: "Editor".to_string(),
        ..LayoutDocument::default()
    };
    layout.set_slot(
        RemoteButton::Up,
        atvv_input::Trigger::Single,
        Some("key:KEY_F13".into()),
    );
    document.layouts.insert("editor".to_string(), layout);
    document.active_layout = "editor".to_string();
    fixture.client.set_config(&document).expect("SetConfig");

    let status = wait_status(&fixture.client, |status| status.config.gain_db == 6);
    assert!(status.config.loaded);
    assert_eq!(status.layout.id, "editor");
    assert_eq!(status.layout.name, "Editor");
    let stored = fixture.client.config().expect("GetConfig after SetConfig");
    assert_eq!(stored.gain_db, 6);
    assert_eq!(
        stored.layouts["editor"].slot(RemoteButton::Up, atvv_input::Trigger::Single),
        Some("key:KEY_F13")
    );

    let text = std::fs::read_to_string(&fixture.path).expect("config file");
    assert!(text.contains("gain_db = 6"), "{text}");
    assert!(text.contains("[layouts.editor.buttons.up]"), "{text}");

    let mut invalid = stored.clone();
    invalid.layouts.get_mut("editor").unwrap().set_slot(
        RemoteButton::Up,
        atvv_input::Trigger::Single,
        Some("key:not-a-key".into()),
    );
    let error = fixture
        .client
        .set_config(&invalid)
        .expect_err("invalid action must fail");
    assert!(error.to_string().contains("not-a-key"), "{error}");

    let after = fixture.client.config().expect("GetConfig after failure");
    assert_eq!(after, stored);
    assert_eq!(std::fs::read_to_string(&fixture.path).unwrap(), text);
}

#[test]
fn capture_ends_when_the_owner_disconnects() {
    let bus = TestBus::start();
    let fixture = Fixture::start(&bus, "capture");

    let owner = bus.connect();
    let owner_name = owner.unique_name().expect("unique name").to_string();
    let capture_client = Client::with_connection(owner.clone());
    capture_client.begin_capture().expect("BeginCapture");
    let capture_cell = fixture.handle.capture();
    {
        let state = capture::lock(&capture_cell);
        assert!(state.active);
        assert_eq!(state.owner.as_deref(), Some(owner_name.as_str()));
    }

    drop(capture_client);
    drop(owner);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if !capture::is_active(&capture_cell) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "capture stayed active after the owner disconnected"
        );
        std::thread::sleep(Duration::from_millis(10));
    }

    fixture.client.begin_capture().expect("BeginCapture");
    fixture.client.end_capture().expect("EndCapture");
    assert!(!capture::is_active(&capture_cell));
}

#[test]
fn button_event_signal_carries_the_capture_payload() {
    let bus = TestBus::start();
    let fixture = Fixture::start(&bus, "signal");
    let sink = DbusSink::new(&fixture.server).expect("signal sink");
    let proxy = fixture.client.proxy().expect("proxy");
    let mut events = zbus::block_on(proxy.receive_button_event()).expect("receive ButtonEvent");
    std::thread::spawn(move || {
        sink.publish(atvv_daemon::DaemonEvent::ButtonEvent {
            button: "ok".to_string(),
            trigger: "single".to_string(),
            pressed: true,
        });
    });
    let event = zbus::block_on(events.next()).expect("ButtonEvent signal");
    let args = event.args().expect("signal args");
    assert_eq!(*args.button(), "ok");
    assert_eq!(*args.trigger(), "single");
    assert!(*args.pressed());
}

#[test]
fn get_config_returns_the_document_shape() {
    let bus = TestBus::start();
    let fixture = Fixture::start(&bus, "shape");
    let document: ConfigDocument = fixture.client.config().expect("GetConfig");
    assert_eq!(document.schema_version, atvv_config::SCHEMA_VERSION);
    assert_eq!(document.active_layout, atvv_config::DEFAULT_LAYOUT);
    assert!(document.layouts.is_empty());
    assert!(!document.allow_exec);
    assert_eq!(document.trigger_key.as_deref(), Some("DICTATE"));
}

#[test]
fn set_layout_switches_persists_and_rejects_unknown_ids() {
    let bus = TestBus::start();
    let fixture = Fixture::start(&bus, "layout");

    let mut document = fixture.client.config().expect("GetConfig");
    for (id, name, action) in [
        ("editor", "Editor", "key:KEY_F13"),
        ("player", "Player", "key:KEY_F14"),
    ] {
        let mut layout = LayoutDocument {
            name: name.to_string(),
            ..LayoutDocument::default()
        };
        layout.set_slot(
            RemoteButton::Up,
            atvv_input::Trigger::Single,
            Some(action.to_string()),
        );
        document.layouts.insert(id.to_string(), layout);
    }
    fixture.client.set_config(&document).expect("SetConfig");

    let proxy = fixture.client.proxy().expect("proxy");
    let mut events = zbus::block_on(proxy.receive_layout_changed()).expect("receive LayoutChanged");
    fixture.client.set_layout("editor").expect("SetLayout");
    let event = zbus::block_on(events.next()).expect("LayoutChanged signal");
    assert_eq!(*event.args().expect("args").id(), "editor");

    let status = wait_status(&fixture.client, |status| status.layout.id == "editor");
    assert_eq!(status.layout.name, "Editor");
    let stored = fixture.client.config().expect("GetConfig after SetLayout");
    assert_eq!(stored.active_layout, "editor");
    let loaded = atvv_config::load(&fixture.path, &RC003).expect("persisted file");
    assert_eq!(loaded.config.active_layout, "editor");

    let error = fixture
        .client
        .set_layout("missing")
        .expect_err("unknown layout must fail");
    assert!(error.to_string().contains("missing"), "{error}");
    assert_eq!(fixture.client.config().unwrap().active_layout, "editor");
    assert_eq!(
        atvv_config::load(&fixture.path, &RC003)
            .unwrap()
            .config
            .active_layout,
        "editor"
    );

    fixture
        .client
        .set_layout("default")
        .expect("SetLayout default");
    let status = wait_status(&fixture.client, |status| status.layout.id == "default");
    assert_eq!(status.layout.name, atvv_config::DEFAULT_LAYOUT_NAME);
    assert_eq!(
        atvv_config::load(&fixture.path, &RC003)
            .unwrap()
            .config
            .active_layout,
        "default"
    );
}

#[test]
fn run_action_checks_the_grammar_and_exec_policy() {
    let bus = TestBus::start();
    let fixture = Fixture::start(&bus, "run-action");
    let path = std::env::temp_dir().join(format!("atvv-run-action-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&path);

    let error = fixture
        .client
        .run_action("key:not-a-key")
        .expect_err("invalid keys must fail");
    assert!(error.to_string().contains("not-a-key"), "{error}");

    let error = fixture
        .client
        .run_action(&format!("exec:touch {}", path.display()))
        .expect_err("exec must need allow_exec");
    assert!(error.to_string().contains("exec"), "{error}");
    assert!(!path.exists());

    let error = fixture
        .client
        .run_action("key:KEY_F13")
        .expect_err("the test fixture has no input runtime");
    assert!(error.to_string().contains("input runtime"), "{error}");

    let mut document = fixture.client.config().expect("GetConfig");
    document.allow_exec = true;
    fixture.client.set_config(&document).expect("enable exec");
    fixture
        .client
        .run_action(&format!("exec:printf run-ok > {}", path.display()))
        .expect("exec RunAction");

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Ok(text) = std::fs::read_to_string(&path) {
            assert_eq!(text, "run-ok");
            break;
        }
        assert!(Instant::now() < deadline, "exec did not run");
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = std::fs::remove_file(&path);
}
