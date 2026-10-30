// SPDX-License-Identifier: GPL-3.0-or-later

//! End-to-end stdio MCP test against an isolated-bus mock daemon.
//!
//! The test spawns the real `atvv-mcp` binary and the real `org.atvv.Daemon`
//! service with `MockTransport`; no hardware, no session bus, and no audio.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use atvv_daemon::audio::PcmSink;
use atvv_daemon::daemon::{Daemon, DaemonConfig, EventSink, NullSink};
use atvv_daemon::dbus::{self, Client, DaemonInterface};
use atvv_daemon::status::StatusView;
use atvv_daemon::transport::{MockConfig, MockHandle, MockTransport, Transport};
use atvv_protocol::profile::RC003;
use serde_json::{Value, json};
use zbus::blocking::Connection;
use zbus::blocking::connection::Builder;

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

fn wait_for_command(mock: &MockHandle, opcode: u8) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while mock.command_count(opcode) == 0 {
        assert!(
            Instant::now() < deadline,
            "daemon never sent command 0x{opcode:02x}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

struct Mcp {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Mcp {
    fn spawn(address: &str) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_atvv-mcp"))
            .env("DBUS_SESSION_BUS_ADDRESS", address)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawn atvv-mcp");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));
        Self {
            child,
            stdin,
            stdout,
        }
    }

    fn send(&mut self, message: Value) {
        writeln!(self.stdin, "{message}").expect("write request");
        self.stdin.flush().expect("flush request");
    }

    fn exchange(&mut self, message: Value) -> Value {
        self.send(message);
        let mut line = String::new();
        self.stdout.read_line(&mut line).expect("read response");
        serde_json::from_str(&line).expect("valid JSON response")
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn request(id: u64, method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
}

#[test]
fn stdio_mcp_contract_against_the_mock_daemon() {
    let bus = TestBus::start();
    let server = bus.connect();
    let (transport, mock) = MockTransport::new(MockConfig::default());
    let address = transport.address().to_string();

    let mut config = DaemonConfig::new(RC003, address.clone());
    config.input_enabled = false;
    let signals: std::sync::Arc<dyn EventSink> = std::sync::Arc::new(NullSink);
    let (daemon, handle) = Daemon::new(
        config,
        Box::new(transport),
        Box::new(CountingSink),
        signals,
        None,
    );
    let interface = DaemonInterface::new(handle.status(), handle.commands(), address.clone());
    dbus::serve(&server, interface).expect("serve org.atvv.Daemon");
    let daemon_thread = std::thread::spawn(move || daemon.run());

    let client = Client::with_connection(bus.connect());
    wait_ready(&client);

    let mut mcp = Mcp::spawn(&bus.address);
    let initialized = mcp.exchange(request(
        1,
        "initialize",
        json!({"protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": {"name": "test"}}),
    ));
    assert_eq!(initialized["id"], 1);
    assert_eq!(initialized["result"]["protocolVersion"], "2025-11-25");
    assert!(initialized["result"]["capabilities"]["tools"].is_object());
    assert_eq!(initialized["result"]["serverInfo"]["name"], "atvv-mcp");

    mcp.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));

    let listed = mcp.exchange(request(2, "tools/list", json!({})));
    let mut names: Vec<&str> = listed["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .map(|tool| tool["name"].as_str().expect("name"))
        .collect();
    names.sort_unstable();
    let mut expected = atvv_mcp::TOOLS.to_vec();
    expected.sort_unstable();
    assert_eq!(names, expected);

    let status = mcp.exchange(request(
        3,
        "tools/call",
        json!({"name": "status", "arguments": {}}),
    ));
    assert_eq!(status["result"]["isError"], false);
    let text = status["result"]["content"][0]["text"]
        .as_str()
        .expect("status text");
    let parsed: Value = serde_json::from_str(text).expect("status JSON");
    assert_eq!(parsed["device"]["id"], RC003.id);
    assert_eq!(parsed["device"]["transport"], "mock");

    let start = mcp.exchange(request(
        4,
        "tools/call",
        json!({"name": "voice_start", "arguments": {}}),
    ));
    assert_eq!(start["result"]["isError"], false, "{start}");
    wait_for_command(&mock, 0x0C);

    let stop = mcp.exchange(request(
        5,
        "tools/call",
        json!({"name": "voice_stop", "arguments": {}}),
    ));
    assert_eq!(stop["result"]["isError"], false, "{stop}");
    wait_for_command(&mock, 0x0D);

    let devices = mcp.exchange(request(
        6,
        "tools/call",
        json!({"name": "list_devices", "arguments": {}}),
    ));
    let text = devices["result"]["content"][0]["text"]
        .as_str()
        .expect("devices text");
    let parsed: Value = serde_json::from_str(text).expect("devices JSON");
    assert_eq!(parsed["devices"], json!([address]));

    drop(mcp);
    let _ = client;
    drop(daemon_thread);
}
