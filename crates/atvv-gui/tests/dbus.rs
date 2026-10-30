// SPDX-License-Identifier: GPL-3.0-or-later

//! The GUI worker against the real `org.atvv.Daemon` service on a private bus.
//!
//! The daemon runs with the mock transport and a counting PCM sink, so the
//! test needs no BlueZ, PipeWire, or input device.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use atvv_config::ConfigDocument;
use atvv_daemon::Transport;
use atvv_daemon::audio::PcmSink;
use atvv_daemon::daemon::{Command as DaemonCommand, Daemon, DaemonConfig, EventSink};
use atvv_daemon::dbus::{self, Client, DaemonInterface, DbusSink};
use atvv_daemon::status::StatusView;
use atvv_daemon::transport::{MockConfig, MockTransport};
use atvv_gui::client::{Backend, Event};
use atvv_protocol::profile::RC003;
use zbus::blocking::connection::Builder;

const ADDRESS: &str = "02:00:00:00:00:01";
const TIMEOUT: Duration = Duration::from_secs(5);

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

    fn connect(&self) -> zbus::blocking::Connection {
        Builder::address(self.address.as_str())
            .expect("connection builder")
            .build()
            .expect("connection")
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
    std::env::temp_dir().join(format!("atvv-gui-dbus-{name}-{}.toml", std::process::id()))
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
    server: zbus::blocking::Connection,
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

fn next_event(backend: &Backend) -> Event {
    backend
        .recv_timeout(TIMEOUT)
        .unwrap_or_else(|error| panic!("worker did not answer: {error}"))
}

#[test]
fn worker_loads_saves_captures_and_uses_run_action() {
    let bus = TestBus::start();
    let fixture = Fixture::start(&bus, "worker");
    let backend = Backend::start(Some(bus.address.clone()));

    backend.load();
    let (document, status) = loop {
        match next_event(&backend) {
            Event::Loaded { document, status } => break (document, status),
            Event::LoadFailed(error) => panic!("GetConfig failed: {error}"),
            _ => {}
        }
    };
    assert_eq!(document.profile, "miremote-rc003");
    assert_eq!(document.schema_version, atvv_config::SCHEMA_VERSION);
    assert!(status.connected);
    assert_eq!(
        document.buttons["ok"].single.as_deref(),
        Some("key:KEY_ENTER")
    );

    let mut changed = document.clone();
    changed.gain_db = 3;
    changed.buttons.get_mut("up").unwrap().single = Some("key:KEY_F13".into());
    backend.save(changed.clone());
    match next_event(&backend) {
        Event::Saved(Ok(())) => {}
        other => panic!("SaveConfig failed: {other:?}"),
    }
    let text = std::fs::read_to_string(&fixture.path).expect("config file");
    assert!(text.contains("gain_db = 3"), "{text}");
    assert!(text.contains("KEY_F13"), "{text}");

    // The service-side sink emits while the GUI's capture listener is active.
    let sink = DbusSink::new(&fixture.server).expect("signal sink");
    backend.begin_capture();
    match next_event(&backend) {
        Event::CaptureStarted(Ok(())) => {}
        other => panic!("BeginCapture failed: {other:?}"),
    }
    sink.publish(atvv_daemon::DaemonEvent::ButtonEvent {
        button: "up".to_string(),
        trigger: "single".to_string(),
        pressed: true,
    });
    match next_event(&backend) {
        Event::Button(event) => {
            assert_eq!(event.button, "up");
            assert_eq!(event.trigger, "single");
            assert!(event.pressed);
        }
        other => panic!("ButtonEvent not forwarded: {other:?}"),
    }
    backend.end_capture();
    match next_event(&backend) {
        Event::CaptureEnded(Ok(())) => {}
        other => panic!("EndCapture failed: {other:?}"),
    }

    // The fixture disables the input runtime and exec policy.
    backend.run_action("key:KEY_F13".into());
    match next_event(&backend) {
        Event::ActionRun(Err(error)) => assert!(error.contains("input runtime"), "{error}"),
        other => panic!("key RunAction should fail without input: {other:?}"),
    }
    backend.run_action("exec:true".into());
    match next_event(&backend) {
        Event::ActionRun(Err(error)) => assert!(error.contains("allow_exec"), "{error}"),
        other => panic!("exec RunAction should need allow_exec: {other:?}"),
    }

    changed.allow_exec = true;
    backend.save(changed);
    match next_event(&backend) {
        Event::Saved(Ok(())) => {}
        other => panic!("enabling exec failed: {other:?}"),
    }
    backend.run_action("exec:true".into());
    match next_event(&backend) {
        Event::ActionRun(Ok(())) => {}
        other => panic!("exec RunAction failed: {other:?}"),
    }
}

#[test]
fn worker_reports_an_unavailable_daemon_without_hanging() {
    let bus = TestBus::start();
    let backend = Backend::start(Some(bus.address.clone()));
    backend.load();
    match next_event(&backend) {
        Event::LoadFailed(error) => assert!(!error.is_empty()),
        other => panic!("expected LoadFailed, got {other:?}"),
    }
}

#[test]
fn document_types_are_shared_with_the_daemon() {
    // Both sides parse the same JSON; this catches a schema drift early.
    let document = ConfigDocument::from_config(&atvv_config::Config::defaults(&RC003));
    let json = serde_json::to_string(&document).expect("serialize");
    let parsed: ConfigDocument = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed, document);
}
