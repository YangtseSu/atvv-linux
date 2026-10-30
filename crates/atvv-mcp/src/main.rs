// SPDX-License-Identifier: GPL-3.0-or-later

//! `atvv-mcp`: stdio MCP server for `org.atvv.Daemon`.
//!
//! Protocol messages use stdout; diagnostics must stay on stderr.

use std::io::{self, BufRead, Write};
use std::process::ExitCode;

use atvv_mcp::{DaemonBackend, Server};

fn main() -> ExitCode {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut server = Server::new(DaemonBackend::new());
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(error) => {
                eprintln!("atvv-mcp: stdin: {error}");
                break;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let Some(response) = server.handle_line(&line) else {
            continue;
        };
        if writeln!(out, "{response}")
            .and_then(|()| out.flush())
            .is_err()
        {
            break;
        }
    }
    ExitCode::SUCCESS
}
