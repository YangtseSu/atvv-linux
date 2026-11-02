// SPDX-License-Identifier: GPL-3.0-or-later

//! `atvvd` entry point.

use std::error::Error;
use std::process::ExitCode;
use std::sync::Arc;

use atvv_audio::AudioSourceConfig;
use atvv_daemon::audio::PipeWireSink;
use atvv_daemon::cli::Args;
use atvv_daemon::daemon::{DaemonConfig, EventSink};
use atvv_daemon::dbus::{self, DaemonInterface, DbusSink};
use atvv_daemon::transport::{
    BluezFactory, BluezTransport, MockConfig, MockFactory, MockHandle, MockTransport, Transport,
};
use atvv_protocol::profile::RC003;
use clap::Parser;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("atvvd: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let config_path = args.config.unwrap_or_else(atvv_config::default_path);
    let mut loaded = atvv_config::load(&config_path, &RC003)?;
    let address = args
        .device
        .clone()
        .or_else(|| loaded.config.active_device.clone())
        .or_else(|| args.mock.then(|| "02:00:00:00:00:01".to_string()))
        .ok_or("no device address: pass --device or run `atvvctl pair <MAC>` first")?;
    let profile = loaded
        .config
        .profile_for_address(&address)
        .or_else(|| loaded.config.active_profile())
        .copied()
        .unwrap_or(RC003);
    loaded.config.adopt_device(&address, &profile);
    let profile = loaded.config.select_device(&address)?;
    let adapter = args.adapter.or_else(|| loaded.config.adapter.clone());

    // The mock handle must stay alive: dropping it closes the mock's command
    // channel and the transport reports a closed link.
    let (transport, _mock_handle): (Box<dyn Transport>, Option<MockHandle>) = if args.mock {
        let (transport, handle) = MockTransport::new(MockConfig {
            address: address.clone(),
            ..MockConfig::default()
        });
        (Box::new(transport), Some(handle))
    } else {
        let transport = BluezTransport::new(
            atvv_bluez::BluezConfig {
                address: address.clone(),
                adapter: adapter.clone(),
                ..atvv_bluez::BluezConfig::default()
            },
            args.verbose,
        );
        (Box::new(transport), None)
    };

    let address = transport.address().to_string();
    let transport_label = format!("{} transport for {}", transport.kind(), transport.address());

    // The resident source runs at one rate for the daemon lifetime; the
    // negotiated `CAPS_RESP` rate is converted to it per frame (ADR 0009).
    // The sink configuration is the one authority for that rate.
    let source = AudioSourceConfig {
        name: dbus::source_name(profile),
        description: profile.display_name.to_string(),
        sample_rate: profile.sample_rate,
        channels: 1,
        queue_capacity: profile.sample_rate as usize,
        extra_properties: atvv_audio::contract_properties(profile.id),
    };
    let graph_rate = source.sample_rate;

    let mut config = DaemonConfig::new(profile, address.clone());
    config.source_name = source.name.clone();
    config.graph_rate = graph_rate;
    config.audio_config = Some(source.clone());
    let existed = loaded.existed;
    // The document mirrors `daemon.toml`, not CLI overrides; `endpoint` and
    // `adapter` below are only the transport's effective values.
    config.config = loaded.config;
    config.config_path = config_path;
    config.config_loaded = existed;
    config.verbose = args.verbose;

    let audio = PipeWireSink::start(source);

    let connection = dbus::connect_session()?;
    let signals: Arc<dyn EventSink> = Arc::new(DbusSink::new(&connection)?);
    // The client is lazy and only called in dbus mode, so a live reload can
    // switch modes without restarting the daemon.
    let vinput: Option<Box<dyn atvv_daemon::VinputControl>> =
        Some(Box::new(atvv_daemon::VinputClient::new()));
    let vinput_mode = config.config.vinput;
    let trigger_key = config.config.trigger_key.clone();
    let (mut daemon, handle) =
        atvv_daemon::Daemon::new(config, transport, Box::new(audio), signals, vinput);
    if args.mock {
        daemon.set_transport_factory(Box::new(MockFactory));
    } else {
        daemon.set_transport_factory(Box::new(BluezFactory {
            adapter,
            verbose: args.verbose,
        }));
    }
    let interface = DaemonInterface::new(
        handle.status(),
        handle.config(),
        handle.capture(),
        handle.commands(),
        address,
    );
    dbus::serve(&connection, interface)?;

    println!(
        "atvvd: {transport_label} on {} {}; vinput={}{}",
        dbus::SERVICE,
        dbus::PATH,
        vinput_mode.name(),
        trigger_key
            .as_deref()
            .map_or_else(String::new, |key| format!(" trigger={key}"))
    );
    daemon.run();
    Ok(())
}
