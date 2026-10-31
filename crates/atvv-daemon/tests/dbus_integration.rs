// SPDX-License-Identifier: GPL-3.0-or-later

//! Isolated session-bus integration test for `org.atvv.Daemon`.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use atvv_daemon::audio::PcmSink;
use atvv_daemon::daemon::{Command as DaemonCommand, Daemon, DaemonConfig, EventSink};
use atvv_daemon::dbus::{self, Client, DaemonInterface, DbusSink};
use atvv_daemon::status::StatusView;
use atvv_daemon::transport::{MockConfig, MockTransport, Transport};
use atvv_protocol::profile::RC003;
use futures_util::StreamExt;
use zbus::Connection;
use zbus::MatchRule;

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

fn temp_config(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("atvv-dbus-{name}-{}.toml", std::process::id()))
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

fn collect_signals(connection: Connection) -> Receiver<(String, Option<String>)> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        zbus::block_on(async {
            let Ok(interface) = zbus::names::InterfaceName::try_from(dbus::INTERFACE) else {
                return;
            };
            let Ok(rule) = MatchRule::builder()
                .msg_type(zbus::message::Type::Signal)
                .interface(interface)
                .build()
            else {
                return;
            };
            let queue = std::num::NonZeroUsize::new(32).expect("non-zero queue");
            let Ok(mut signals) =
                zbus::MessageStream::for_match_rule(rule, &connection, Some(queue)).await
            else {
                return;
            };
            if sender.send(("__ready__".to_string(), None)).is_err() {
                return;
            }
            while let Some(message) = signals.next().await {
                let Ok(message) = message else {
                    break;
                };
                let name = message
                    .header()
                    .member()
                    .map(ToString::to_string)
                    .unwrap_or_default();
                let state = message
                    .body()
                    .deserialize::<(String,)>()
                    .ok()
                    .map(|(state,)| state);
                if sender.send((name, state)).is_err() {
                    break;
                }
            }
        });
    });
    receiver
}

fn wait_for_signal(
    signals: &Receiver<(String, Option<String>)>,
    wanted: &str,
    deadline: Instant,
) -> Option<String> {
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match signals.recv_timeout(remaining.min(Duration::from_millis(200))) {
            Ok((name, state)) if name == wanted => return Some(state.unwrap_or_default()),
            Ok(_) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return None,
        }
    }
    None
}

#[test]
fn mock_daemon_dbus_contract() {
    let bus = TestBus::start();
    let server = zbus::block_on(zbus::connection::Builder::address(bus.address.as_str()).build())
        .expect("server connection");

    let (transport, mock) = MockTransport::new(MockConfig {
        address: ADDRESS.to_string(),
        hardware: atvv_bluez::HardwareInfo {
            battery_percent: Some(62),
            manufacturer: Some("MIOM".to_string()),
            model: Some("RC003".to_string()),
            serial_number: Some("250519".to_string()),
            hardware_revision: Some("V2.0".to_string()),
            firmware_revision: Some("2671".to_string()),
            software_revision: Some("A.7.0.6".to_string()),
        },
        ..MockConfig::default()
    });
    let config_path = temp_config("reload");
    let _ = std::fs::remove_file(&config_path);

    let mut config = DaemonConfig::new(RC003, ADDRESS);
    config.config.input = false;
    config.config_path = config_path.clone();
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
        address.clone(),
    );
    dbus::serve(&server, interface).expect("serve org.atvv.Daemon");
    let daemon_thread = std::thread::spawn(move || daemon.run());

    let client_connection =
        zbus::block_on(zbus::connection::Builder::address(bus.address.as_str()).build())
            .expect("client connection");
    let client = Client::with_connection(client_connection.clone());
    let status = wait_status(&client, |status| status.state == "ready");
    assert_eq!(status.device.id, RC003.id);
    assert_eq!(status.device.transport, "mock");
    assert_eq!(status.caps.as_ref().map(|caps| caps.frame_size), Some(120));
    assert_eq!(status.hardware.battery_percent, Some(62));
    assert_eq!(status.hardware.firmware_revision.as_deref(), Some("2671"));
    assert_eq!(status.hardware.hardware_revision.as_deref(), Some("V2.0"));
    assert_eq!(status.hardware.serial_number.as_deref(), Some("250519"));

    assert_eq!(client.devices().expect("devices"), vec![address]);

    let reply = zbus::block_on(client_connection.call_method(
        Some(dbus::SERVICE),
        dbus::PATH,
        Some("org.freedesktop.DBus.Introspectable"),
        "Introspect",
        &(),
    ))
    .expect("introspection");
    let xml: String = reply.body().deserialize().expect("introspection XML");
    for member in [
        "GetStatus",
        "ListDevices",
        "StartVoice",
        "StopVoice",
        "ReloadConfig",
        "StateChanged",
        "VoiceStarted",
        "VoiceStopped",
    ] {
        assert!(xml.contains(member), "{member} missing from introspection");
    }

    let signals = collect_signals(client_connection.clone());
    assert_eq!(
        signals
            .recv_timeout(Duration::from_secs(1))
            .expect("signal collector"),
        ("__ready__".to_string(), None)
    );

    // Mock direct AUDIO_START: no START_SEARCH, exactly the RC003 path.
    mock.start_audio();
    let status = wait_status(&client, |status| {
        status.state == "streaming" && status.audio.frames > 0 && status.audio.samples > 0
    });
    assert!(status.streaming);
    assert!(status.audio.available);

    // Device AUDIO_STOP -> one MIC_CLOSE; the mock echo must be ignored.
    mock.stop_audio(0x00);
    wait_status(&client, |status| status.state == "ready");
    assert_eq!(mock.command_count(0x0D), 1);
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(
        mock.command_count(0x0D),
        1,
        "stop echo caused a second close"
    );

    let deadline = Instant::now() + Duration::from_secs(2);
    assert!(
        wait_for_signal(&signals, "VoiceStarted", deadline).is_some(),
        "VoiceStarted signal missing"
    );
    assert!(
        wait_for_signal(&signals, "VoiceStopped", deadline).is_some(),
        "VoiceStopped signal missing"
    );

    // D-Bus StartVoice opens with MIC_OPEN; StopVoice closes once.
    mock.clear_commands();
    client.start_voice().expect("StartVoice");
    wait_status(&client, |status| status.state == "streaming");
    client.stop_voice().expect("StopVoice");
    wait_status(&client, |status| status.state == "ready");
    assert_eq!(mock.command_count(0x0D), 1);
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(mock.command_count(0x0D), 1);

    // ReloadConfig applies a real file change.
    std::fs::write(&config_path, "gain_db = 6\n").expect("write config");
    client.reload().expect("ReloadConfig");
    let status = wait_status(&client, |status| status.config.gain_db == 6);
    assert!(status.config.loaded);
    assert_eq!(status.audio.gain_db, 6);

    handle
        .send(DaemonCommand::Shutdown)
        .expect("shutdown command");
    daemon_thread.join().expect("daemon thread");
    let _ = std::fs::remove_file(&config_path);
}
