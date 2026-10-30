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
use atvv_daemon::transport::{BluezTransport, MockConfig, MockHandle, MockTransport, Transport};
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

    let profile = RC003;
    let config_path = args
        .config
        .unwrap_or_else(atvv_daemon::config::default_path);
    let loaded = atvv_daemon::config::load(&config_path, profile.gain_db)?;
    let address = args.device.or_else(|| loaded.device.clone());
    let adapter = args.adapter.or_else(|| loaded.adapter.clone());

    // The mock handle must stay alive: dropping it closes the mock's command
    // channel and the transport reports a closed link.
    let (transport, _mock_handle): (Box<dyn Transport>, Option<MockHandle>) = if args.mock {
        let address = address.unwrap_or_else(|| "02:00:00:00:00:01".to_string());
        let (transport, handle) = MockTransport::new(MockConfig {
            address,
            ..MockConfig::default()
        });
        (Box::new(transport), Some(handle))
    } else {
        let address =
            address.ok_or("no device address: pass --device or run `atvvctl pair <MAC>` first")?;
        let transport = BluezTransport::new(
            atvv_bluez::BluezConfig {
                address,
                adapter,
                ..atvv_bluez::BluezConfig::default()
            },
            args.verbose,
        );
        (Box::new(transport), None)
    };

    let address = transport.address().to_string();
    let transport_label = format!("{} transport for {}", transport.kind(), transport.address());

    let mut config = DaemonConfig::new(profile, address.clone());
    config.source_name = dbus::source_name(profile);
    config.config_path = config_path;
    config.config_loaded = loaded.existed;
    config.gain_db = loaded.gain_db;
    config.input_enabled = loaded.input;
    config.vinput = loaded.vinput;
    config.trigger_key = loaded.trigger_key.clone();
    config.verbose = args.verbose;

    let audio = PipeWireSink::start(AudioSourceConfig {
        name: config.source_name.clone(),
        description: profile.display_name.to_string(),
        sample_rate: profile.sample_rate,
        channels: 1,
        queue_capacity: profile.sample_rate as usize,
        extra_properties: atvv_audio::contract_properties(profile.id),
    });

    let connection = dbus::connect_session()?;
    let signals: Arc<dyn EventSink> = Arc::new(DbusSink::new(&connection)?);
    // Only D-Bus mode needs a vinput client; trigger mode emits keys from the
    // input runtime and needs nothing here.
    let vinput: Option<Box<dyn atvv_daemon::VinputControl>> = (loaded.vinput
        == atvv_daemon::VinputMode::Dbus)
        .then(|| Box::new(atvv_daemon::VinputClient::new()) as Box<dyn atvv_daemon::VinputControl>);
    let (daemon, handle) =
        atvv_daemon::Daemon::new(config, transport, Box::new(audio), signals, vinput);
    let interface = DaemonInterface::new(handle.status(), handle.commands(), address);
    dbus::serve(&connection, interface)?;

    println!(
        "atvvd: {transport_label} on {} {}; vinput={}{}",
        dbus::SERVICE,
        dbus::PATH,
        loaded.vinput.name(),
        loaded
            .trigger_key
            .as_deref()
            .map_or_else(String::new, |key| format!(" trigger={key}"))
    );
    daemon.run();
    Ok(())
}
