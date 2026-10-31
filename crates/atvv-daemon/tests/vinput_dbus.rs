// SPDX-License-Identifier: GPL-3.0-or-later

//! Isolated-bus tests for the `org.fcitx.Vinput` client.
//!
//! `vinput` is never linked or copied; the mock reproduces only the method
//! signatures recorded in `docs/research/protocol-and-device.md` §8.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use atvv_daemon::vinput::{SERVICE, VinputClient, VinputControl};
use zbus::interface;

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

    fn connect(&self) -> zbus::Connection {
        zbus::block_on(
            zbus::connection::Builder::address(self.address.as_str())
                .method_timeout(Duration::from_secs(2))
                .build(),
        )
        .expect("connection")
    }
}

impl Drop for TestBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct MockVinput {
    calls: Arc<Mutex<Vec<String>>>,
}

#[interface(name = "org.fcitx.Vinput.Service")]
impl MockVinput {
    fn start_recording(&self) {
        self.calls
            .lock()
            .unwrap()
            .push("StartRecording".to_string());
    }

    fn stop_recording(&self, scene_id: &str) -> String {
        self.calls
            .lock()
            .unwrap()
            .push(format!("StopRecording:{scene_id}"));
        "transcript".to_string()
    }
}

#[test]
fn start_and_stop_reach_the_service() {
    let bus = TestBus::start();
    let server = bus.connect();
    let calls = Arc::new(Mutex::new(Vec::new()));
    zbus::block_on(server.object_server().at(
        atvv_daemon::vinput::PATH,
        MockVinput {
            calls: Arc::clone(&calls),
        },
    ))
    .expect("register mock vinput");
    zbus::block_on(server.request_name(SERVICE)).expect("claim vinput name");

    let mut client = VinputClient::with_connection(bus.connect());
    client.start().expect("StartRecording");
    client.stop().expect("StopRecording");

    assert_eq!(
        *calls.lock().unwrap(),
        vec!["StartRecording".to_string(), "StopRecording:".to_string()]
    );
    assert!(client.available());
    assert_eq!(client.last_error(), None);
}

#[test]
fn a_missing_service_degrades_instead_of_stalling() {
    let bus = TestBus::start();
    let mut client = VinputClient::with_connection(bus.connect());

    let error = client.start().expect_err("no vinput service on this bus");
    assert!(
        error.message.contains("org.fcitx.Vinput") || !error.message.is_empty(),
        "unexpected error: {error}"
    );
    assert!(!client.available());
    assert!(client.last_error().is_some());
}
