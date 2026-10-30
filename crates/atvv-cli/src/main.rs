// SPDX-License-Identifier: GPL-3.0-or-later

//! `atvvctl` — hardware-free diagnostics for the ATVV stack.

use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, ExitCode};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use atvv_audio::{AudioSource, AudioSourceConfig};
use atvv_daemon::Client;
use atvv_input::{ActionMap, FAKE_REMOTE, FakeRemote, Gesture, InputRuntime, RemoteEvent};
use atvv_protocol::caps::{Capabilities, InteractionModel, ProtocolVersion};
use atvv_protocol::events::{AudioStartReason, AudioStopReason, CtlEvent, StreamId};
use atvv_protocol::profile::{PROFILES, RC003, RemoteButton, find};
use atvv_protocol::session::Session;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

#[derive(Debug, Parser)]
#[command(
    name = "atvvctl",
    version,
    about = "ATVV diagnostics and profile tools"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Inspect compiled device profiles.
    Profile {
        #[command(subcommand)]
        command: ProfileCommand,
    },
    /// Decode an ATVV CAPS_RESP payload.
    Caps {
        #[command(subcommand)]
        command: CapsCommand,
    },
    /// Drive the RC003 session state machine without hardware.
    Simulate {
        #[arg(default_value = "miremote-rc003")]
        profile: String,
    },
    /// Exercise the native PipeWire source with a synthetic tone.
    Audio {
        #[command(subcommand)]
        command: AudioCommand,
    },
    /// Exercise the evdev/uinput input path with a fake remote.
    Input {
        #[command(subcommand)]
        command: InputCommand,
    },
    /// Show the running daemon's status from `org.atvv.Daemon`.
    Status,
    /// Control the remote microphone through the daemon.
    Mic {
        #[command(subcommand)]
        command: MicCommand,
    },
    /// Re-read the daemon configuration.
    Reload,
    /// Pair and trust a remote, then save its address.
    Pair {
        /// Bluetooth address, e.g. C0:5D:39:F8:2D:4C.
        address: String,
        /// BlueZ adapter name such as `hci0`.
        #[arg(long)]
        adapter: Option<String>,
        /// Do not write the address to daemon.toml.
        #[arg(long)]
        no_save: bool,
    },
    /// Capture the resident ATVV source to a WAV file.
    Record {
        /// Output WAV path.
        #[arg(long)]
        wav: PathBuf,
        /// Capture length in seconds.
        #[arg(long, default_value_t = 10)]
        seconds: u64,
        /// Open the remote microphone through the daemon while capturing.
        #[arg(long)]
        trigger: bool,
    },
    /// Print daemon state changes and voice signals.
    Monitor,
    /// Check D-Bus, BlueZ, PipeWire, uinput, and config.
    Doctor {
        /// Bluetooth address to probe (default: `device` in daemon.toml).
        #[arg(long)]
        device: Option<String>,
        /// BlueZ adapter name such as `hci0`.
        #[arg(long)]
        adapter: Option<String>,
    },
    /// Print shell completions to stdout.
    Completions {
        /// Shell to generate completions for.
        shell: Shell,
    },
    /// Write man pages into a directory (used by packaging).
    #[command(hide = true)]
    Manpages {
        /// Destination directory.
        dir: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum MicCommand {
    Start,
    Stop,
    Toggle,
}

#[derive(Debug, Subcommand)]
enum InputCommand {
    /// Create and grab a fake uinput remote, then exercise all 12 buttons.
    FakeRemote {
        /// Keep both fake devices open so evtest/libinput can inspect them.
        #[arg(long, default_value_t = 0)]
        hold: u64,
    },
}

#[derive(Debug, Subcommand)]
enum AudioCommand {
    /// Create a 16 kHz source and feed a 440 Hz tone.
    Test {
        #[arg(long, default_value_t = 3)]
        seconds: u64,
    },
}

#[derive(Debug, Subcommand)]
enum ProfileCommand {
    /// List compiled profiles.
    List,
    /// Show one profile by id.
    Show { id: String },
}

#[derive(Debug, Subcommand)]
enum CapsCommand {
    /// Decode a hex payload, e.g. 0B010002000078.
    Decode { hex: String },
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("atvvctl: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    match Cli::parse().command {
        Command::Profile { command } => match command {
            ProfileCommand::List => {
                list_profiles();
                Ok(())
            }
            ProfileCommand::Show { id } => show_profile(&id),
        },
        Command::Caps { command } => match command {
            CapsCommand::Decode { hex } => decode_caps(&hex),
        },
        Command::Simulate { profile } => simulate(&profile),
        Command::Audio { command } => match command {
            AudioCommand::Test { seconds } => audio_test(seconds),
        },
        Command::Input { command } => match command {
            InputCommand::FakeRemote { hold } => input_fake_remote(hold),
        },
        Command::Status => status(),
        Command::Mic { command } => mic(command),
        Command::Reload => reload(),
        Command::Pair {
            address,
            adapter,
            no_save,
        } => pair(&address, adapter, no_save),
        Command::Record {
            wav,
            seconds,
            trigger,
        } => record(&wav, seconds, trigger),
        Command::Monitor => monitor(),
        Command::Doctor { device, adapter } => doctor(device, adapter),
        Command::Completions { shell } => completions(shell),
        Command::Manpages { dir } => manpages(&dir),
    }
}

fn completions(shell: Shell) -> Result<(), String> {
    let mut command = Cli::command();
    let mut output = Vec::new();
    clap_complete::generate(shell, &mut command, "atvvctl", &mut output);
    std::io::stdout()
        .write_all(&output)
        .map_err(|error| format!("cannot write completions: {error}"))
}

fn manpages(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir)
        .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
    clap_mangen::generate_to(Cli::command(), dir)
        .map_err(|error| format!("cannot write atvvctl man pages: {error}"))?;
    clap_mangen::generate_to(atvv_daemon::cli::Args::command(), dir)
        .map_err(|error| format!("cannot write atvvd man pages: {error}"))?;
    Ok(())
}

fn daemon_client() -> Result<Client, String> {
    Client::connect()
        .map_err(|error| format!("cannot connect to the session D-Bus: {error}\nis atvvd running?"))
}

/// Whether `vinput-daemon` owns its well-known name on the session bus.
fn vinput_service_present() -> bool {
    let Ok(connection) = zbus::blocking::Connection::session() else {
        return false;
    };
    let Ok(name) = zbus::names::BusName::try_from(atvv_daemon::vinput::SERVICE) else {
        return false;
    };
    let Ok(bus) = zbus::blocking::fdo::DBusProxy::new(&connection) else {
        return false;
    };
    bus.name_has_owner(name).unwrap_or(false)
}

fn status() -> Result<(), String> {
    let client = daemon_client()?;
    let value = client.status_json().map_err(|error| error.to_string())?;
    let rendered = serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?;
    println!("{rendered}");
    Ok(())
}

fn mic(command: MicCommand) -> Result<(), String> {
    let client = daemon_client()?;
    match command {
        MicCommand::Start => client.start_voice(),
        MicCommand::Stop => client.stop_voice(),
        MicCommand::Toggle => client.toggle_voice(),
    }
    .map_err(|error| error.to_string())?;
    println!("ok");
    Ok(())
}

fn reload() -> Result<(), String> {
    let client = daemon_client()?;
    client.reload().map_err(|error| error.to_string())?;
    println!("reloaded");
    Ok(())
}

fn pair(address: &str, adapter: Option<String>, no_save: bool) -> Result<(), String> {
    let report = atvv_bluez::pair(atvv_bluez::PairConfig {
        address: address.to_string(),
        adapter: adapter.clone(),
        ..atvv_bluez::PairConfig::default()
    })
    .map_err(|error| format!("pairing {address} failed: {error}"))?;

    println!("address: {}", report.address);
    if let Some(name) = &report.name {
        println!("name: {name}");
    }
    println!("already_paired: {}", report.already_paired);
    println!("paired: {}", report.paired);
    println!("trusted: {}", report.trusted);
    for request in &report.agent_requests {
        println!("agent: {request}");
    }
    if !report.paired {
        return Err("the device is not bonded".to_string());
    }
    if no_save {
        println!("config: not saved (--no-save)");
        return Ok(());
    }
    let path = atvv_daemon::config::default_path();
    atvv_daemon::config::save_device(&path, &report.address, adapter.as_deref())
        .map_err(|error| error.to_string())?;
    println!("config: {} device={}", path.display(), report.address);
    Ok(())
}

fn record(wav: &PathBuf, seconds: u64, trigger: bool) -> Result<(), String> {
    let client = daemon_client()?;
    let status = client
        .status()
        .map_err(|error| format!("cannot read daemon status: {error}"))?;
    if !status.audio.available {
        return Err(format!(
            "source {} is unavailable: {}",
            status.audio.source,
            status.audio.degraded.as_deref().unwrap_or("degraded")
        ));
    }

    let rate = RC003.sample_rate;
    let seconds = seconds.max(1);
    let samples = u64::from(rate) * seconds;
    let mut child = ProcessCommand::new("pw-cat")
        .arg("--record")
        .args([
            "--target",
            &status.audio.source,
            "--rate",
            &rate.to_string(),
            "--channels",
            "1",
            "--format",
            "s16",
            "--container",
            "wav",
            "--sample-count",
            &samples.to_string(),
        ])
        .arg(wav)
        .spawn()
        .map_err(|error| format!("cannot start pw-cat: {error}"))?;

    if trigger {
        // Let the capture stream link before asking the remote for audio.
        std::thread::sleep(Duration::from_millis(300));
        client
            .start_voice()
            .map_err(|error| format!("StartVoice failed: {error}"))?;
    }
    println!(
        "recording {seconds}s at {rate} Hz from {} to {}",
        status.audio.source,
        wav.display()
    );
    let outcome = child.wait();
    if trigger {
        let _ = client.stop_voice();
    }
    let outcome = outcome.map_err(|error| format!("pw-cat failed: {error}"))?;
    // pw-cat may exit non-zero even after writing a complete
    // `--sample-count` capture, so the file is the source of truth.
    let bytes = std::fs::metadata(wav).map_or(0, |metadata| metadata.len());
    if bytes <= 44 {
        return Err(format!(
            "pw-cat produced no audio (exit {outcome}): {}",
            wav.display()
        ));
    }
    if !outcome.success() {
        eprintln!("atvvctl: pw-cat exited with {outcome}; {bytes} bytes were written");
    }
    println!("captured {}", wav.display());
    Ok(())
}

fn monitor() -> Result<(), String> {
    let client = daemon_client()?;
    match client.status() {
        Ok(status) => println!(
            "state={} transport={} caps={}",
            status.state,
            status.device.transport,
            status.caps.as_ref().map_or_else(
                || "none".to_string(),
                |caps| format!("v{} {} {} B", caps.version, caps.codec, caps.frame_size)
            )
        ),
        Err(error) => eprintln!("status: {error}"),
    }

    let proxy = zbus::blocking::Proxy::new(
        client.connection(),
        atvv_daemon::dbus::SERVICE,
        atvv_daemon::dbus::PATH,
        atvv_daemon::dbus::INTERFACE,
    )
    .map_err(|error| error.to_string())?;
    let signals = proxy
        .receive_all_signals()
        .map_err(|error| error.to_string())?;
    println!("monitoring {} (Ctrl-C to stop)", atvv_daemon::dbus::SERVICE);
    for message in signals {
        let member = message
            .header()
            .member()
            .map_or_else(|| "<unknown>".to_string(), ToString::to_string);
        match member.as_str() {
            "StateChanged" => {
                let state: String = message
                    .body()
                    .deserialize()
                    .map_err(|error| error.to_string())?;
                println!("state: {state}");
            }
            "VoiceStarted" => println!("voice: started"),
            "VoiceStopped" => println!("voice: stopped"),
            other => println!("{other}"),
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckState {
    Ok,
    Warn,
    Fail,
}

impl CheckState {
    fn label(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warn => "warn",
            Self::Fail => "fail",
        }
    }
}

struct Check {
    name: &'static str,
    state: CheckState,
    detail: String,
}

impl Check {
    fn new(name: &'static str, state: CheckState, detail: impl Into<String>) -> Self {
        Self {
            name,
            state,
            detail: detail.into(),
        }
    }
}

fn doctor(device_arg: Option<String>, adapter: Option<String>) -> Result<(), String> {
    let mut checks = Vec::new();
    let config_path = atvv_daemon::config::default_path();
    let config = atvv_daemon::config::load(&config_path, RC003.gain_db);
    let configured_device = config
        .as_ref()
        .ok()
        .and_then(|config| config.device.clone());
    let address = device_arg.or(configured_device);
    let status = match Client::connect() {
        Ok(client) => match client.status() {
            Ok(status) => {
                checks.push(Check::new(
                    "daemon",
                    CheckState::Ok,
                    format!(
                        "state={} transport={}",
                        status.state, status.device.transport
                    ),
                ));
                Some(status)
            }
            Err(error) => {
                checks.push(Check::new(
                    "daemon",
                    CheckState::Fail,
                    format!("GetStatus failed: {error}"),
                ));
                None
            }
        },
        Err(error) => {
            checks.push(Check::new(
                "daemon",
                CheckState::Fail,
                format!("session D-Bus unavailable: {error}"),
            ));
            None
        }
    };

    if let Some(status) = &status {
        if status.audio.available {
            checks.push(Check::new(
                "source",
                CheckState::Ok,
                format!(
                    "{} available (queued {} samples)",
                    status.audio.source, status.audio.queued_samples
                ),
            ));
        } else {
            checks.push(Check::new(
                "source",
                CheckState::Warn,
                format!(
                    "{} unavailable: {}",
                    status.audio.source,
                    status.audio.degraded.as_deref().unwrap_or("degraded")
                ),
            ));
        }

        match ProcessCommand::new("pw-cli")
            .args(["info", &status.audio.source])
            .output()
        {
            Ok(output) if output.status.success() => checks.push(Check::new(
                "pipewire",
                CheckState::Ok,
                format!("node {} found", status.audio.source),
            )),
            Ok(_) => checks.push(Check::new(
                "pipewire",
                CheckState::Warn,
                format!("node {} not found", status.audio.source),
            )),
            Err(error) => checks.push(Check::new(
                "pipewire",
                CheckState::Warn,
                format!("pw-cli unavailable: {error}"),
            )),
        }
    } else {
        checks.push(Check::new(
            "source",
            CheckState::Warn,
            "skipped: daemon unavailable",
        ));
        checks.push(Check::new(
            "pipewire",
            CheckState::Warn,
            "skipped: daemon unavailable",
        ));
    }

    match address {
        Some(address) => {
            let probe = atvv_bluez::BluezConfig {
                address: address.clone(),
                adapter: adapter.clone(),
                ..atvv_bluez::BluezConfig::default()
            };
            match atvv_bluez::probe(&probe) {
                Ok(report) => {
                    checks.push(Check::new(
                        "bluez",
                        if report.powered {
                            CheckState::Ok
                        } else {
                            CheckState::Fail
                        },
                        format!(
                            "adapter {} is {}",
                            report.adapter,
                            if report.powered { "powered" } else { "off" }
                        ),
                    ));
                    checks.push(Check::new(
                        "device",
                        if report.connected && report.paired && report.trusted {
                            CheckState::Ok
                        } else {
                            CheckState::Warn
                        },
                        format!(
                            "{} {}{}{}{}",
                            report.address,
                            report.name.as_deref().unwrap_or("(unnamed)"),
                            if report.paired {
                                " paired"
                            } else {
                                " not-paired"
                            },
                            if report.trusted {
                                " trusted"
                            } else {
                                " untrusted"
                            },
                            if report.connected {
                                " connected"
                            } else {
                                " disconnected"
                            },
                        ),
                    ));
                    match &report.atvv {
                        Some(info) => checks.push(Check::new(
                            "atvv",
                            CheckState::Ok,
                            format!(
                                "service {}; tx mtu {} [{}]; audio mtu {}; control mtu {}",
                                info.service_uuid,
                                info.transmit.mtu,
                                info.transmit.flags.join(","),
                                info.audio.mtu,
                                info.control.mtu,
                            ),
                        )),
                        None => checks.push(Check::new(
                            "atvv",
                            CheckState::Warn,
                            if report.connected {
                                "service is not resolved yet"
                            } else {
                                "connect the remote to resolve the service"
                            },
                        )),
                    }
                }
                Err(error) => {
                    checks.push(Check::new("bluez", CheckState::Fail, error.to_string()));
                }
            }
        }
        None => checks.push(Check::new(
            "bluez",
            CheckState::Warn,
            "no device configured; run `atvvctl pair <MAC>`",
        )),
    }

    match OpenOptions::new().write(true).open("/dev/uinput") {
        Ok(_) => checks.push(Check::new(
            "uinput",
            CheckState::Ok,
            "/dev/uinput is writable",
        )),
        Err(error) => checks.push(Check::new(
            "uinput",
            CheckState::Warn,
            format!("/dev/uinput is not writable: {error}"),
        )),
    }

    match &config {
        Ok(config) => {
            let trigger = config.trigger_key.as_deref().unwrap_or("none");
            match config.vinput {
                atvv_daemon::VinputMode::Trigger => {
                    let keyboard = status.as_ref().is_some_and(|status| status.input.keyboard);
                    checks.push(Check::new(
                        "vinput",
                        if keyboard {
                            CheckState::Ok
                        } else {
                            CheckState::Warn
                        },
                        format!(
                            "trigger mode: {trigger} ({})",
                            if keyboard {
                                "virtual keyboard ready"
                            } else {
                                "virtual keyboard unavailable"
                            }
                        ),
                    ));
                }
                atvv_daemon::VinputMode::Dbus => {
                    let present = vinput_service_present();
                    checks.push(Check::new(
                        "vinput",
                        if present {
                            CheckState::Ok
                        } else {
                            CheckState::Warn
                        },
                        if present {
                            "dbus mode: org.fcitx.Vinput is running"
                        } else {
                            "dbus mode: org.fcitx.Vinput is not running; trigger degrades"
                        },
                    ));
                }
                atvv_daemon::VinputMode::Off => checks.push(Check::new(
                    "vinput",
                    CheckState::Warn,
                    "disabled; the voice key does not trigger fcitx",
                )),
            }
        }
        Err(_) => checks.push(Check::new(
            "vinput",
            CheckState::Warn,
            "skipped: configuration is invalid",
        )),
    }

    match config {
        Ok(config) if config.existed => checks.push(Check::new(
            "config",
            CheckState::Ok,
            format!(
                "{} (gain {} dB, input {}, vinput {})",
                config_path.display(),
                config.gain_db,
                config.input,
                config.vinput.name()
            ),
        )),
        Ok(_) => checks.push(Check::new(
            "config",
            CheckState::Warn,
            format!("{} not found; defaults in use", config_path.display()),
        )),
        Err(error) => checks.push(Check::new("config", CheckState::Fail, error.to_string())),
    }

    for check in &checks {
        println!(
            "{:<10} {:<4} {}",
            check.name,
            check.state.label(),
            check.detail
        );
    }
    if checks.iter().any(|check| check.state == CheckState::Fail) {
        return Err("one or more checks failed".to_string());
    }
    Ok(())
}

fn audio_test(seconds: u64) -> Result<(), String> {
    let config = AudioSourceConfig {
        name: "atvv-test".to_string(),
        description: "ATVV test source".to_string(),
        sample_rate: 16_000,
        channels: 1,
        queue_capacity: 16_000,
    };
    let mut source = AudioSource::start(config).map_err(|error| error.to_string())?;
    println!(
        "source: {} (16000 Hz mono s16le); recording for {seconds}s",
        source.name()
    );

    let mut phase = 0.0_f64;
    let step = std::f64::consts::TAU * 440.0 / 16_000.0;
    let chunks = seconds.saturating_mul(100).max(1);
    let start = Instant::now();

    for _ in 0..chunks {
        let mut buffer = [0_i16; 160];
        for sample in &mut buffer {
            phase += step;
            if phase >= std::f64::consts::TAU {
                phase -= std::f64::consts::TAU;
            }
            *sample = (phase.sin() * 0.2 * f64::from(i16::MAX)) as i16;
        }
        source.push(&buffer);
        std::thread::sleep(Duration::from_millis(10));
    }

    source.stop();
    println!("stopped after {:?}", start.elapsed());
    Ok(())
}

fn input_fake_remote(hold_seconds: u64) -> Result<(), String> {
    let mut fake = FakeRemote::open().map_err(|error| {
        format!(
            "cannot create the fake remote: {error}\n\
             /dev/uinput must exist and be writable; the udev rule is step 09"
        )
    })?;
    let fake_path = fake.node_path().to_path_buf();
    println!(
        "fake remote: {} ({})",
        fake_path.display(),
        FAKE_REMOTE.bluetooth_names[0]
    );

    let runtime = InputRuntime::start(FAKE_REMOTE, ActionMap::defaults())
        .map_err(|error| error.to_string())?;
    for message in runtime.degraded() {
        println!("degraded: {message}");
    }
    for skipped in runtime.skipped() {
        println!(
            "skipped: {} ({}) — {}",
            skipped.path.display(),
            skipped.name,
            skipped.reason
        );
    }
    println!("grabbed:");
    for path in runtime.grabbed() {
        println!("  {}", path.display());
        if path != &fake_path {
            return Err(format!(
                "refusing to continue: grabbed {}, not the fake remote",
                path.display()
            ));
        }
    }
    if runtime.grabbed().len() != 1 {
        return Err(format!(
            "expected exactly one grabbed node, got {}",
            runtime.grabbed().len()
        ));
    }
    match runtime.keyboard_path() {
        Some(path) => println!("virtual keyboard: {}", path.display()),
        None => println!("virtual keyboard: unavailable"),
    }

    for button in RemoteButton::LOGICAL {
        fake.tap(button)
            .map_err(|error| format!("fake remote {button}: {error}"))?;
    }
    fake.press(RemoteButton::Voice)
        .map_err(|error| format!("fake remote voice: {error}"))?;
    fake.release(RemoteButton::Voice)
        .map_err(|error| format!("fake remote voice: {error}"))?;

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut seen = BTreeSet::new();
    let mut voice = Vec::new();
    let mut dispatch_errors = BTreeSet::new();
    while (seen.len() < RemoteButton::LOGICAL.len() || voice.len() < 2) && Instant::now() < deadline
    {
        let timeout = deadline.saturating_duration_since(Instant::now());
        match runtime.recv_timeout(timeout) {
            Ok(RemoteEvent::Button {
                button,
                gesture,
                action,
                error,
            }) => {
                if matches!(gesture, Gesture::SingleClick(_) | Gesture::RepeatTick(_))
                    && seen.insert(button)
                {
                    let action = action
                        .as_ref()
                        .map_or_else(|| "unmapped".to_string(), ToString::to_string);
                    println!("button: {button:<12} {gesture:?} -> {action}");
                }
                if let Some(error) = error {
                    dispatch_errors.insert(format!("{button}: {error}"));
                }
            }
            Ok(RemoteEvent::Voice { pressed }) => {
                println!("voice: {}", if pressed { "press" } else { "release" });
                voice.push(pressed);
            }
            Ok(RemoteEvent::Degraded { message }) => println!("degraded: {message}"),
            Ok(RemoteEvent::DeviceLost { path, error }) => {
                return Err(format!("lost {}: {error}", path.display()));
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    println!(
        "recognized {}/{} logical buttons",
        seen.len(),
        RemoteButton::LOGICAL.len()
    );
    let missing: Vec<_> = RemoteButton::LOGICAL
        .into_iter()
        .filter(|button| !seen.contains(button))
        .collect();
    for button in &missing {
        println!("missing: {button}");
    }
    for error in &dispatch_errors {
        println!("dispatch error: {error}");
    }
    if voice != [true, false] {
        println!("voice edges: {voice:?} (expected [true, false])");
    }
    if !missing.is_empty() {
        return Err(format!(
            "{} logical buttons were not recognized",
            missing.len()
        ));
    }
    if voice != [true, false] {
        return Err("the voice key did not produce a clean press/release pair".to_string());
    }
    if !dispatch_errors.is_empty() {
        return Err("one or more mapped keys could not be emitted".to_string());
    }

    if hold_seconds > 0 {
        println!(
            "holding both devices open for {hold_seconds}s; tapping Up every second\n  \
             evtest {}\n  evtest {}\n  libinput debug-events",
            fake_path.display(),
            runtime.keyboard_path().map_or_else(
                || "<unavailable>".to_string(),
                |path| path.display().to_string()
            )
        );
        let deadline = Instant::now() + Duration::from_secs(hold_seconds);
        while Instant::now() < deadline {
            fake.tap(RemoteButton::Up)
                .map_err(|error| format!("fake remote up: {error}"))?;
            std::thread::sleep(Duration::from_millis(1000));
            while runtime.try_recv().is_ok() {}
        }
    }

    Ok(())
}

fn list_profiles() {
    println!(
        "{:<20} {:<44} {:<6} {:>6} {:>8}",
        "ID", "NAME", "ATVV", "FRAME", "RATE"
    );
    for profile in PROFILES {
        println!(
            "{:<20} {:<44} {:<6} {:>6} {:>8}",
            profile.id,
            profile.display_name,
            profile.protocol.to_string(),
            format!("{} B", profile.frame_size),
            format!("{} Hz", profile.sample_rate),
        );
    }
}

fn show_profile(id: &str) -> Result<(), String> {
    let profile = find(id).ok_or_else(|| {
        let available = PROFILES
            .iter()
            .map(|profile| profile.id)
            .collect::<Vec<_>>()
            .join(", ");
        format!("unknown profile {id:?}; available: {available}")
    })?;

    println!("id: {}", profile.id);
    println!("name: {}", profile.display_name);
    println!("bluetooth_names: {}", profile.bluetooth_names.join(", "));
    println!("vendor_id: 0x{:04x}", profile.vendor_id);
    println!("product_id: 0x{:04x}", profile.product_id);
    println!("model: {}", profile.model);
    println!("atvv: {}", profile.protocol);
    println!(
        "codecs: {}",
        profile
            .codecs
            .iter()
            .map(|codec| format!("{codec:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("frame_size: {} B", profile.frame_size);
    println!("nibble_order: {:?}", profile.nibble_order);
    println!("mic_open: {:?}", profile.mic_open);
    println!("mic_close: {:?}", profile.mic_close);
    println!("keepalive: {:?}", profile.keepalive);
    println!("direct_audio_start: {}", profile.direct_audio_start);
    println!("stop_echo: {}", profile.stop_echo);
    println!("post_stop_discard_ms: {}", profile.post_stop_discard_ms);
    println!("sample_rate: {} Hz", profile.sample_rate);
    println!("gain_db: {} dB", profile.gain_db);
    println!("buttons:");
    for mapping in profile.button_keys {
        println!("  {:<12} {}", mapping.button, mapping.key);
    }
    let timing = profile.gesture_timing;
    println!(
        "gesture_ms: double={} long={} repeat_start={} latch={} suppression={} repeat={} back_repeat={}",
        timing.double_click_ms,
        timing.long_press_ms,
        timing.repeat_start_ms,
        timing.stable_release_ms,
        timing.suppression_ms,
        timing.repeat_interval_ms,
        timing.back_repeat_interval_ms,
    );
    Ok(())
}

fn decode_caps(hex: &str) -> Result<(), String> {
    let payload = parse_hex(hex)?;
    let caps = Capabilities::parse(&payload).map_err(|error| error.to_string())?;

    println!("version: {}", caps.version);
    println!("codec_mask: 0x{:02x}", caps.codec_mask);
    let codecs = [
        (atvv_protocol::caps::Codec::Adpcm8k, "adpcm8k"),
        (atvv_protocol::caps::Codec::Adpcm16k, "adpcm16k"),
    ]
    .into_iter()
    .filter(|(codec, _)| caps.supports(*codec))
    .map(|(_, name)| name)
    .collect::<Vec<_>>()
    .join(", ");
    println!(
        "codecs: {}",
        if codecs.is_empty() { "none" } else { &codecs }
    );
    println!("preferred_codec: {:?}", caps.best_codec());
    println!("interaction: {:?}", caps.interaction);
    println!("frame_size: {} B", caps.frame_size);
    Ok(())
}

fn simulate(profile_id: &str) -> Result<(), String> {
    let profile = find(profile_id)
        .copied()
        .ok_or_else(|| format!("unknown profile {profile_id:?}"))?;
    let mut session = Session::new(profile);

    println!("profile: {} ({})", profile.display_name, profile.id);
    println!("state: {:?}", session.state());

    let caps = Capabilities {
        version: ProtocolVersion::V1_0,
        codec_mask: 0x02,
        interaction: InteractionModel::HoldToTalk,
        frame_size: profile.frame_size,
    };

    emit(&mut session, "CAPS_RESP", CtlEvent::CapsResp(caps));
    emit(
        &mut session,
        "AUDIO_START (direct)",
        CtlEvent::AudioStart {
            reason: AudioStartReason::HoldToTalk,
            codec: Some(profile.preferred_codec()),
            stream_id: StreamId(1),
        },
    );
    if let Some(keepalive) = session.keepalive() {
        println!("keepalive: {keepalive:02x?}");
    }
    emit(
        &mut session,
        "AUDIO_STOP",
        CtlEvent::AudioStop {
            reason: AudioStopReason::MicClose,
        },
    );
    emit(
        &mut session,
        "AUDIO_STOP echo",
        CtlEvent::AudioStop {
            reason: AudioStopReason::MicClose,
        },
    );

    Ok(())
}

fn emit(session: &mut Session, label: &str, event: CtlEvent) {
    let actions = session.on_event(event);
    println!("{label}: state {:?}", session.state());
    for action in actions {
        println!("  {action}");
    }
}

fn parse_hex(input: &str) -> Result<Vec<u8>, String> {
    let trimmed = input.trim();
    let trimmed = trimmed.strip_prefix("0x").unwrap_or(trimmed);
    let digits: Vec<char> = trimmed
        .chars()
        .filter(|character| !character.is_whitespace() && *character != ':' && *character != '-')
        .collect();

    if !digits.len().is_multiple_of(2) {
        return Err("hex payload must contain an even number of digits".to_string());
    }

    digits
        .chunks_exact(2)
        .map(|pair| {
            let high = pair[0]
                .to_digit(16)
                .ok_or_else(|| format!("invalid hex digit {:?}", pair[0]))?;
            let low = pair[1]
                .to_digit(16)
                .ok_or_else(|| format!("invalid hex digit {:?}", pair[1]))?;
            Ok(((high << 4) | low) as u8)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::parse_hex;

    #[test]
    fn parses_spaced_prefixed_and_plain_hex() {
        assert_eq!(
            parse_hex("0B 01 00 02 00 00 78").unwrap(),
            vec![0x0B, 0x01, 0x00, 0x02, 0x00, 0x00, 0x78]
        );
        assert_eq!(
            parse_hex("0x0b010002000078").unwrap(),
            vec![0x0B, 0x01, 0x00, 0x02, 0x00, 0x00, 0x78]
        );
        assert_eq!(parse_hex("0B:01-00").unwrap(), vec![0x0B, 0x01, 0x00]);
    }

    #[test]
    fn rejects_odd_and_invalid_hex() {
        assert!(parse_hex("0B 0").is_err());
        assert!(parse_hex("0B 0G").is_err());
    }
}
