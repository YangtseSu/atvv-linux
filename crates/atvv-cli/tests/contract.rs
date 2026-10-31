// SPDX-License-Identifier: GPL-3.0-or-later

//! `atvvctl contract` and `atvvctl status` against an isolated-bus mock daemon.
//!
//! The test spawns the real `atvvctl` binary and the real `org.atvv.Daemon`
//! service with `MockTransport`; no hardware, no session bus, and no audio.

use std::fs;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use atvv_daemon::audio::PcmSink;
use atvv_daemon::daemon::{Daemon, DaemonConfig, EventSink, NullSink};
use atvv_daemon::dbus::{self, Client, DaemonInterface};
use atvv_daemon::status::StatusView;
use atvv_daemon::transport::{MockConfig, MockTransport, Transport};
use atvv_protocol::profile::RC003;
use serde_json::Value;
use zbus::Connection;
use zbus::connection::Builder;

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
        "atvv-miremote-rc003"
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

fn wait_ready(client: &Client) -> StatusView {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let status = client.status().expect("daemon status");
        if status.state == "ready" {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "daemon did not become ready; last status: {status:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn run_cli(bus: &TestBus, config_home: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_atvvctl"))
        .args(args)
        .env("DBUS_SESSION_BUS_ADDRESS", &bus.address)
        .env("XDG_CONFIG_HOME", config_home)
        .output()
        .expect("run atvvctl")
}

#[test]
fn contract_and_status_expose_the_provider_contract() {
    let bus = TestBus::start();
    let server = bus.connect();
    let (transport, _mock) = MockTransport::new(MockConfig::default());
    let address = transport.address().to_string();

    let mut config = DaemonConfig::new(RC003, address.clone());
    config.config.input = false;
    let signals: Arc<dyn EventSink> = Arc::new(NullSink);
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
        address.clone(),
    );
    dbus::serve(&server, interface).expect("serve org.atvv.Daemon");
    let daemon_thread = std::thread::spawn(move || daemon.run());

    let client = Client::with_connection(bus.connect());
    wait_ready(&client);

    let config_home = std::env::temp_dir().join(format!("atvv-contract-{}", std::process::id()));
    let _ = fs::remove_dir_all(&config_home);
    fs::create_dir_all(&config_home).expect("temp config home");

    let output = run_cli(&bus, &config_home, &["contract", "--json"]);
    assert!(
        output.status.success(),
        "contract --json failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let contract: Value = serde_json::from_slice(&output.stdout).expect("contract JSON");
    assert_eq!(contract["version"], "1");
    assert_eq!(contract["profile"], RC003.id);
    assert_eq!(contract["source"], "atvv-miremote-rc003");
    assert_eq!(contract["media_class"], "Audio/Source");
    assert_eq!(contract["sample_format"], "s16le");
    assert_eq!(contract["sample_rate"], 16_000);
    assert_eq!(contract["channels"], 1);
    assert_eq!(
        contract["recording_keys"],
        serde_json::json!(["DICTATE", "VOICECOMMAND", "ASSISTANT"])
    );
    assert_eq!(contract["active_key"], "DICTATE");
    assert_eq!(contract["input_ready"], false);
    assert_eq!(contract["dbus"]["name"], "org.atvv.Daemon");
    assert_eq!(contract["dbus"]["path"], "/org/atvv/Daemon");
    assert_eq!(contract["mcp"]["command"], "atvv-mcp");
    assert!(
        contract["recipe"]
            .as_str()
            .is_some_and(|recipe| recipe.contains("atvv-miremote-rc003"))
    );

    let output = run_cli(&bus, &config_home, &["contract"]);
    assert!(
        output.status.success(),
        "contract failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);
    for expected in [
        "profile:        miremote-rc003",
        "source:         atvv-miremote-rc003",
        "recording keys: DICTATE, VOICECOMMAND, ASSISTANT",
        "active key:     DICTATE",
        "dbus:           org.atvv.Daemon /org/atvv/Daemon org.atvv.Daemon",
        "mcp:            atvv-mcp (stdio)",
    ] {
        assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
    }

    let output = run_cli(&bus, &config_home, &["status"]);
    assert!(
        output.status.success(),
        "status failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let status: Value = serde_json::from_slice(&output.stdout).expect("status JSON");
    assert_eq!(status["contract"], contract);
    assert_eq!(status["contract"]["active_key"], "DICTATE");

    let entries: Vec<_> = fs::read_dir(&config_home)
        .expect("read temp config home")
        .collect();
    assert!(
        entries.is_empty(),
        "contract/status must not write configuration: {entries:?}"
    );

    let _ = fs::remove_dir_all(&config_home);
    drop(daemon_thread);
}
