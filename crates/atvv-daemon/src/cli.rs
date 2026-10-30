// SPDX-License-Identifier: GPL-3.0-or-later

//! `atvvd` command line.
//!
//! The definition lives in the library so the man-page generator in
//! `atvvctl` describes the same flags the binary accepts.

use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "atvvd", version, about = "ATVV remote session daemon")]
pub struct Args {
    /// Bluetooth address of the remote (default: `device` in daemon.toml).
    #[arg(long)]
    pub device: Option<String>,
    /// BlueZ adapter name such as `hci0` (default: BlueZ's default adapter).
    #[arg(long)]
    pub adapter: Option<String>,
    /// Use the hardware-free mock transport.
    #[arg(long)]
    pub mock: bool,
    /// Log protocol and lifecycle activity to stderr.
    #[arg(long)]
    pub verbose: bool,
    /// Configuration file (default: $XDG_CONFIG_HOME/atvv/daemon.toml).
    #[arg(long)]
    pub config: Option<PathBuf>,
}
