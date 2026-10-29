// SPDX-License-Identifier: GPL-3.0-or-later

//! `atvvctl` — hardware-free diagnostics for the ATVV stack.

use std::collections::BTreeSet;
use std::process::ExitCode;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use atvv_audio::{AudioSource, AudioSourceConfig};
use atvv_input::{ActionMap, FAKE_REMOTE, FakeRemote, Gesture, InputRuntime, RemoteEvent};
use atvv_protocol::caps::{Capabilities, InteractionModel, ProtocolVersion};
use atvv_protocol::events::{AudioStartReason, AudioStopReason, CtlEvent, StreamId};
use atvv_protocol::profile::{PROFILES, RemoteButton, find};
use atvv_protocol::session::Session;
use clap::{Parser, Subcommand};

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
    }
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

    if digits.len() % 2 != 0 {
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
