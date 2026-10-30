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
use atvv_config::{ConfigDocument, VinputMode, parse_checked_action};
use atvv_daemon::{Client, ContractView};
use atvv_input::{
    ActionMap, FAKE_REMOTE, FakeRemote, Gesture, InputRuntime, KeyResolver, RemoteEvent, Trigger,
};
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
    /// Print the provider contract consumers discover on the source.
    Contract {
        /// Emit the contract as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Control the remote microphone through the daemon.
    Mic {
        #[command(subcommand)]
        command: MicCommand,
    },
    /// Re-read the daemon configuration.
    Reload,
    /// Read or edit the daemon configuration.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Wait for one physical remote button press (capture mode).
    Capture {
        /// Stop after this many seconds.
        #[arg(long, default_value_t = 30)]
        timeout: u64,
        /// Emit one JSON object per event.
        #[arg(long)]
        json: bool,
    },
    /// Resolve or validate an action mapping without executing it.
    Map {
        #[command(subcommand)]
        command: MapCommand,
    },
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
enum ConfigCommand {
    /// Print the active configuration.
    Get {
        /// Emit the JSON document used by `SetConfig`.
        #[arg(long)]
        json: bool,
    },
    /// Change values, apply them live, and persist them.
    Set(ConfigSet),
}

#[derive(Debug, clap::Args)]
struct ConfigSet {
    /// Output gain in dB.
    #[arg(long)]
    gain_db: Option<i8>,
    /// Enable or disable the evdev/uinput input path.
    #[arg(long, num_args = 0..=1, default_missing_value = "true",
          value_parser = clap::value_parser!(bool))]
    input: Option<bool>,
    /// Allow `exec:` actions.
    #[arg(long, num_args = 0..=1, default_missing_value = "true",
          value_parser = clap::value_parser!(bool))]
    allow_exec: Option<bool>,
    /// fcitx5-vinput integration mode.
    #[arg(long, value_parser = ["trigger", "dbus", "off"])]
    vinput: Option<String>,
    /// Synthetic key or chord; `none` disables it.
    #[arg(long)]
    trigger_key: Option<String>,
    /// Bluetooth address saved for the next start.
    #[arg(long)]
    device: Option<String>,
    /// BlueZ adapter name saved for the next start.
    #[arg(long)]
    adapter: Option<String>,
    /// Logical button to edit: power, ok, back, up, down, left, right, home,
    /// menu, tv, volume_up, volume_down.
    #[arg(long)]
    button: Option<String>,
    /// Gesture slot to edit.
    #[arg(long, value_parser = ["single", "double", "long"])]
    trigger: Option<String>,
    /// Action string, `none`, or `default` to restore the profile value.
    #[arg(long)]
    action: Option<String>,
}

#[derive(Debug, Subcommand)]
enum MapCommand {
    /// Print the effective action for a button and trigger slot.
    Test {
        /// Logical button; defaults to every button when omitted.
        #[arg(long)]
        button: Option<String>,
        #[arg(long, default_value = "single",
              value_parser = ["single", "double", "long"])]
        trigger: String,
        /// Validate this action string instead of reading the configuration.
        #[arg(long)]
        action: Option<String>,
        /// Emit JSON.
        #[arg(long)]
        json: bool,
    },
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
        Command::Contract { json } => contract(json),
        Command::Mic { command } => mic(command),
        Command::Reload => reload(),
        Command::Config { command } => config(command),
        Command::Capture { timeout, json } => capture(timeout, json),
        Command::Map { command } => map(command),
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

fn contract(json: bool) -> Result<(), String> {
    let client = daemon_client()?;
    let status = client
        .status()
        .map_err(|error| format!("cannot read daemon status: {error}"))?;
    if json {
        let rendered = serde_json::to_string_pretty(&status.contract)
            .map_err(|error| format!("cannot render contract: {error}"))?;
        println!("{rendered}");
    } else {
        print!("{}", render_contract(&status.contract));
    }
    Ok(())
}

fn render_contract(contract: &ContractView) -> String {
    let layout = if contract.channels == 1 {
        "mono".to_string()
    } else {
        format!("{} channels", contract.channels)
    };
    let active = contract.active_key.as_deref().unwrap_or("none");
    format!(
        "profile:        {}\nsource:         {}\nmedia class:    {}\nformat:         {} Hz {} {}\nrecording keys: {}\nactive key:     {}\ninput:          {}\nkeyboard:       {}\ndbus:           {} {} {}\nmcp:            {} ({})\nrecipe:         {}\n",
        contract.profile,
        contract.source,
        contract.media_class,
        contract.sample_rate,
        layout,
        contract.sample_format,
        contract.recording_keys.join(", "),
        active,
        if contract.input_ready {
            "ready"
        } else {
            "unavailable"
        },
        if contract.keyboard_ready {
            "ready"
        } else {
            "unavailable"
        },
        contract.dbus.name,
        contract.dbus.path,
        contract.dbus.interface,
        contract.mcp.command,
        contract.mcp.transport,
        contract.recipe,
    )
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

fn config(command: ConfigCommand) -> Result<(), String> {
    let client = daemon_client()?;
    match command {
        ConfigCommand::Get { json } => {
            let document = client
                .config()
                .map_err(|error| format!("cannot read the configuration: {error}"))?;
            if json {
                let rendered = serde_json::to_string_pretty(&document)
                    .map_err(|error| format!("cannot render the configuration: {error}"))?;
                println!("{rendered}");
            } else {
                print!("{}", render_config(&document));
            }
            Ok(())
        }
        ConfigCommand::Set(args) => set_config(&client, args),
    }
}

fn set_config(client: &Client, args: ConfigSet) -> Result<(), String> {
    let mut document = client
        .config()
        .map_err(|error| format!("cannot read the configuration: {error}"))?;
    let status = client.status().ok();
    let mut changed = false;

    if let Some(gain_db) = args.gain_db {
        document.gain_db = gain_db;
        changed = true;
    }
    if let Some(input) = args.input {
        document.input = input;
        changed = true;
    }
    if let Some(allow_exec) = args.allow_exec {
        document.allow_exec = allow_exec;
        changed = true;
    }
    if let Some(vinput) = args.vinput.as_deref() {
        document.vinput = parse_vinput(vinput)?;
        changed = true;
    }
    if let Some(trigger_key) = args.trigger_key {
        let trigger_key = trigger_key.trim();
        document.trigger_key = if trigger_key.is_empty() || trigger_key.eq_ignore_ascii_case("none")
        {
            None
        } else {
            Some(trigger_key.to_string())
        };
        changed = true;
    }
    if let Some(device) = args.device {
        document.device = Some(device);
        changed = true;
    }
    if let Some(adapter) = args.adapter {
        document.adapter = Some(adapter);
        changed = true;
    }

    match (args.button, args.trigger, args.action) {
        (Some(button), Some(trigger), Some(action)) => {
            let button = find_button(&button)?;
            let trigger = parse_trigger(&trigger)?;
            let value = if action.trim().eq_ignore_ascii_case("default") {
                default_action(status.as_ref(), button, trigger)
            } else {
                Some(action.trim().to_string())
            };
            let slots = document
                .buttons
                .entry(button.name().to_string())
                .or_default();
            set_slot(slots, trigger, value);
            changed = true;
        }
        (None, None, None) => {}
        _ => return Err("--button requires --trigger and --action".to_string()),
    }

    if !changed {
        return Err("no changes requested".to_string());
    }
    client
        .set_config(&document)
        .map_err(|error| format!("cannot apply the configuration: {error}"))?;
    println!("applied");
    Ok(())
}

fn parse_vinput(value: &str) -> Result<VinputMode, String> {
    match value {
        "trigger" => Ok(VinputMode::Trigger),
        "dbus" => Ok(VinputMode::Dbus),
        "off" => Ok(VinputMode::Off),
        other => Err(format!("unknown vinput mode {other:?}")),
    }
}

fn parse_trigger(value: &str) -> Result<Trigger, String> {
    match value {
        "single" => Ok(Trigger::Single),
        "double" => Ok(Trigger::Double),
        "long" => Ok(Trigger::Long),
        other => Err(format!("unknown trigger slot {other:?}")),
    }
}

fn find_button(name: &str) -> Result<RemoteButton, String> {
    RemoteButton::LOGICAL
        .iter()
        .copied()
        .find(|button| button.name() == name)
        .ok_or_else(|| {
            let available = RemoteButton::LOGICAL
                .iter()
                .map(|button| button.name())
                .collect::<Vec<_>>()
                .join(", ");
            format!("unknown button {name:?}; available: {available}")
        })
}

fn default_action(
    _status: Option<&atvv_daemon::StatusView>,
    button: RemoteButton,
    trigger: Trigger,
) -> Option<String> {
    ActionMap::defaults()
        .resolve(button, trigger)
        .map(ToString::to_string)
}

fn set_slot(slots: &mut atvv_config::ButtonDocument, trigger: Trigger, value: Option<String>) {
    match trigger {
        Trigger::Single => slots.single = value,
        Trigger::Double => slots.double = value,
        Trigger::Long => slots.long = value,
    }
}

fn render_config(document: &ConfigDocument) -> String {
    let trigger_key = document.trigger_key.as_deref().unwrap_or("none");
    let mut text = format!(
        "schema:      {}\nprofile:     {}\ndevice:      {}\nadapter:     {}\ngain_db:     {}\ninput:       {}\nvinput:      {}\ntrigger_key: {}\nallow_exec:  {}\ngesture:     double={} long={} repeat_start={} latch={} suppression={} repeat={} back_repeat={}\n",
        document.schema_version,
        document.profile,
        document.device.as_deref().unwrap_or("-"),
        document.adapter.as_deref().unwrap_or("-"),
        document.gain_db,
        document.input,
        document.vinput.name(),
        trigger_key,
        document.allow_exec,
        document.gesture.double_click_ms,
        document.gesture.long_press_ms,
        document.gesture.repeat_start_ms,
        document.gesture.stable_release_ms,
        document.gesture.suppression_ms,
        document.gesture.repeat_interval_ms,
        document.gesture.back_repeat_interval_ms,
    );
    text.push_str("buttons:\n");
    for button in RemoteButton::LOGICAL {
        let Some(slots) = document.buttons.get(button.name()) else {
            continue;
        };
        text.push_str(&format!(
            "  {:<12} single={:<22} double={:<22} long={}\n",
            button.name(),
            slots.single.as_deref().unwrap_or("-"),
            slots.double.as_deref().unwrap_or("-"),
            slots.long.as_deref().unwrap_or("-"),
        ));
    }
    if document.allow_exec {
        text.push_str("note: exec actions run shell commands as your user\n");
    }
    text
}

fn capture(timeout: u64, json: bool) -> Result<(), String> {
    let client = daemon_client()?;
    let connection = client.connection().clone();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<()>();
    let (tx, rx) = std::sync::mpsc::channel::<(String, String, bool)>();
    std::thread::spawn(move || {
        let Ok(proxy) = atvv_daemon::dbus::DaemonApiProxyBlocking::new(&connection) else {
            return;
        };
        let Ok(mut events) = proxy.receive_button_event() else {
            return;
        };
        let _ = ready_tx.send(());
        for event in events.by_ref() {
            let Ok(args) = event.args() else {
                continue;
            };
            if tx
                .send((
                    args.button().to_string(),
                    args.trigger().to_string(),
                    *args.pressed(),
                ))
                .is_err()
            {
                break;
            }
        }
    });
    ready_rx
        .recv_timeout(Duration::from_secs(5))
        .map_err(|_| "cannot subscribe to ButtonEvent".to_string())?;
    client
        .begin_capture()
        .map_err(|error| format!("BeginCapture failed: {error}"))?;

    let deadline = Instant::now() + Duration::from_secs(timeout.max(1));
    let result = loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break Err(format!("no button pressed within {timeout}s"));
        }
        match rx.recv_timeout(remaining) {
            Ok((button, trigger, pressed)) => {
                if json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "button": button,
                            "trigger": trigger,
                            "pressed": pressed,
                        })
                    );
                } else {
                    println!("button: {button:<12} trigger: {trigger:<8} pressed: {pressed}");
                }
                if pressed {
                    break Ok(());
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                break Err(format!("no button pressed within {timeout}s"));
            }
            Err(RecvTimeoutError::Disconnected) => {
                break Err("the ButtonEvent stream ended".to_string());
            }
        }
    };
    let _ = client.end_capture();
    result
}

fn map(command: MapCommand) -> Result<(), String> {
    let MapCommand::Test {
        button,
        trigger,
        action,
        json,
    } = command;
    let client = daemon_client()?;
    let document = client
        .config()
        .map_err(|error| format!("cannot read the configuration: {error}"))?;
    let trigger = parse_trigger(&trigger)?;

    if let Some(action) = action {
        let checked = parse_checked_action(&action, document.allow_exec)
            .map_err(|error| format!("{action:?}: {error}"))?;
        let program = KeyResolver.program(&checked).ok();
        if json {
            let value = serde_json::json!({
                "action": checked.to_string(),
                "kind": action_kind(&checked),
                "allowed": true,
                "press": program.as_ref().map(|program| {
                    program.press.iter().map(|key| format!("{key:?}")).collect::<Vec<_>>()
                }),
                "release": program.as_ref().map(|program| {
                    program.release.iter().map(|key| format!("{key:?}")).collect::<Vec<_>>()
                }),
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?
            );
        } else {
            println!("action:  {}", checked);
            println!("kind:    {}", action_kind(&checked));
            println!("allowed: true");
            match program {
                Some(program) => {
                    println!("press:   {}", key_names(&program.press));
                    println!("release: {}", key_names(&program.release));
                }
                None => println!("keys:    none"),
            }
        }
        return Ok(());
    }

    let buttons: Vec<RemoteButton> = button
        .as_deref()
        .map(find_button)
        .transpose()?
        .map_or_else(|| RemoteButton::LOGICAL.to_vec(), |button| vec![button]);
    if json {
        if let Some(button) = button.as_deref() {
            let button = find_button(button)?;
            let value = serde_json::json!({
                "button": button.name(),
                "trigger": trigger.name(),
                "action": resolved_action(&document, button, trigger),
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?
            );
        } else {
            let mut map = serde_json::Map::new();
            for button in buttons {
                map.insert(
                    button.name().to_string(),
                    serde_json::json!(resolved_action(&document, button, trigger)),
                );
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&map).map_err(|error| error.to_string())?
            );
        }
    } else {
        for button in buttons {
            println!(
                "{:<12} {:<7} {}",
                button.name(),
                trigger.name(),
                resolved_action(&document, button, trigger).unwrap_or_else(|| "-".to_string())
            );
        }
    }
    Ok(())
}

fn resolved_action(
    document: &ConfigDocument,
    button: RemoteButton,
    trigger: Trigger,
) -> Option<String> {
    let slots = document.buttons.get(button.name())?;
    match trigger {
        Trigger::Single => slots.single.clone(),
        Trigger::Double => slots.double.clone(),
        Trigger::Long => slots.long.clone(),
    }
}

fn action_kind(action: &atvv_input::Action) -> &'static str {
    match action {
        atvv_input::Action::None => "none",
        atvv_input::Action::Key { .. } => "key",
        atvv_input::Action::Combo { .. } => "combo",
        atvv_input::Action::Exec { .. } => "exec",
        atvv_input::Action::Dictation(_) => "dictation",
    }
}

fn key_names<T: std::fmt::Debug>(keys: &[T]) -> String {
    keys.iter()
        .map(|key| format!("{key:?}"))
        .collect::<Vec<_>>()
        .join(", ")
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
    let path = atvv_config::default_path();
    atvv_config::save_device(&path, &report.address, adapter.as_deref())
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
    let config_path = atvv_config::default_path();
    let config = atvv_config::load(&config_path, &RC003);
    let configured_device = config
        .as_ref()
        .ok()
        .and_then(|config| config.config.device.clone());
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
            Ok(output) if output.status.success() => {
                checks.push(Check::new(
                    "pipewire",
                    CheckState::Ok,
                    format!("node {} found", status.audio.source),
                ));
                let advertised = String::from_utf8_lossy(&output.stdout).contains("atvv.");
                checks.push(Check::new(
                    "properties",
                    if advertised {
                        CheckState::Ok
                    } else {
                        CheckState::Warn
                    },
                    if advertised {
                        "source advertises the atvv.* contract".to_string()
                    } else {
                        "source advertises no atvv.* properties; rebuild or restart atvvd"
                            .to_string()
                    },
                ));
            }
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

        let contract = &status.contract;
        let declared = if contract.recording_keys.is_empty() {
            "none".to_string()
        } else {
            contract.recording_keys.join(",")
        };
        checks.push(Check::new(
            "contract",
            if contract.recording_keys.is_empty() {
                CheckState::Warn
            } else {
                CheckState::Ok
            },
            format!(
                "v{} {} {} Hz {}; keys {}; active {}",
                contract.version,
                contract.media_class,
                contract.sample_rate,
                contract.sample_format,
                declared,
                contract.active_key.as_deref().unwrap_or("none")
            ),
        ));
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
            let trigger = config.config.trigger_key.as_deref().unwrap_or("none");
            match config.config.vinput {
                atvv_config::VinputMode::Trigger => {
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
                atvv_config::VinputMode::Dbus => {
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
                atvv_config::VinputMode::Off => checks.push(Check::new(
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
                config.config.gain_db,
                config.config.input,
                config.config.vinput.name()
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
        extra_properties: atvv_audio::contract_properties("test"),
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
    use super::{parse_hex, render_contract};
    use atvv_daemon::{ContractView, DbusView, McpView};

    fn contract() -> ContractView {
        ContractView {
            version: "1".to_string(),
            profile: "miremote-rc003".to_string(),
            source: "atvv-miremote-rc003".to_string(),
            media_class: "Audio/Source".to_string(),
            sample_format: "s16le".to_string(),
            sample_rate: 16_000,
            channels: 1,
            recording_keys: vec![
                "DICTATE".to_string(),
                "VOICECOMMAND".to_string(),
                "ASSISTANT".to_string(),
            ],
            active_key: Some("DICTATE".to_string()),
            input_ready: true,
            keyboard_ready: true,
            dbus: DbusView {
                name: "org.atvv.Daemon".to_string(),
                path: "/org/atvv/Daemon".to_string(),
                interface: "org.atvv.Daemon".to_string(),
            },
            mcp: McpView {
                command: "atvv-mcp".to_string(),
                transport: "stdio".to_string(),
            },
            recipe: "Select atvv-miremote-rc003; bind the declared keys.".to_string(),
        }
    }

    #[test]
    fn renders_the_contract_for_consumers() {
        let text = render_contract(&contract());
        for expected in [
            "profile:        miremote-rc003",
            "source:         atvv-miremote-rc003",
            "media class:    Audio/Source",
            "format:         16000 Hz mono s16le",
            "recording keys: DICTATE, VOICECOMMAND, ASSISTANT",
            "active key:     DICTATE",
            "input:          ready",
            "keyboard:       ready",
            "dbus:           org.atvv.Daemon /org/atvv/Daemon org.atvv.Daemon",
            "mcp:            atvv-mcp (stdio)",
        ] {
            assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
        }
    }

    #[test]
    fn renders_missing_active_key_and_multichannel_layout() {
        let mut contract = contract();
        contract.active_key = None;
        contract.input_ready = false;
        contract.keyboard_ready = false;
        contract.channels = 2;
        let text = render_contract(&contract);
        assert!(text.contains("format:         16000 Hz 2 channels s16le"));
        assert!(text.contains("active key:     none"));
        assert!(text.contains("input:          unavailable"));
        assert!(text.contains("keyboard:       unavailable"));
    }

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
