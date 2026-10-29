// SPDX-License-Identifier: GPL-3.0-or-later

//! `atvvd` entry point.

use std::error::Error;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use atvv_audio::AudioSourceConfig;
use atvv_daemon::audio::PipeWireSink;
use atvv_daemon::daemon::{DaemonConfig, EventSink};
use atvv_daemon::dbus::{self, DaemonInterface, DbusSink};
use atvv_daemon::transport::{MockConfig, MockTransport, Transport};
use atvv_protocol::profile::RC003;
use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "atvvd", version, about = "ATVV remote session daemon")]
struct Args {
    /// Bluetooth address of the remote.
    #[arg(long)]
    device: Option<String>,
    /// Use the hardware-free mock transport.
    #[arg(long)]
    mock: bool,
    /// Log protocol and lifecycle activity to stderr.
    #[arg(long)]
    verbose: bool,
    /// Configuration file (default: $XDG_CONFIG_HOME/atvv/daemon.toml).
    #[arg(long)]
    config: Option<PathBuf>,
}

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

    if !args.mock {
        return Err("the BlueZ transport is step 07; start the daemon with --mock for now".into());
    }

    let profile = RC003;
    let config_path = args
        .config
        .unwrap_or_else(atvv_daemon::config::default_path);
    let loaded = atvv_daemon::config::load(&config_path, profile.gain_db)?;
    let address = args
        .device
        .unwrap_or_else(|| "02:00:00:00:00:01".to_string());

    let (transport, _mock_handle) = MockTransport::new(MockConfig {
        address: address.clone(),
        ..MockConfig::default()
    });
    let address = transport.address().to_string();

    let mut config = DaemonConfig::new(profile, address.clone());
    config.source_name = dbus::source_name(profile);
    config.config_path = config_path;
    config.config_loaded = loaded.existed;
    config.gain_db = loaded.gain_db;
    config.input_enabled = loaded.input;
    config.verbose = args.verbose;

    let audio = PipeWireSink::start(AudioSourceConfig {
        name: config.source_name.clone(),
        description: profile.display_name.to_string(),
        sample_rate: profile.sample_rate,
        channels: 1,
        queue_capacity: profile.sample_rate as usize,
    });

    let connection = dbus::connect_session()?;
    let signals: Arc<dyn EventSink> = Arc::new(DbusSink::new(&connection)?);
    let (daemon, handle) =
        atvv_daemon::Daemon::new(config, Box::new(transport), Box::new(audio), signals);
    let interface = DaemonInterface::new(handle.status(), handle.commands(), address);
    dbus::serve(&connection, interface)?;

    println!("atvvd: mock transport on {} {}", dbus::SERVICE, dbus::PATH);
    daemon.run();
    Ok(())
}
