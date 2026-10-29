// SPDX-License-Identifier: GPL-3.0-or-later

//! `atvvctl` — hardware-free diagnostics for the ATVV stack.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use atvv_audio::{AudioSource, AudioSourceConfig};
use atvv_protocol::caps::{Capabilities, InteractionModel, ProtocolVersion};
use atvv_protocol::events::{AudioStartReason, AudioStopReason, CtlEvent, StreamId};
use atvv_protocol::profile::{PROFILES, find};
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
    println!("voice_key: {}", profile.voice_key);
    println!("tv_key: {}", profile.tv_key);
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
