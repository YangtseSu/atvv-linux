// SPDX-License-Identifier: GPL-3.0-or-later

//! Daemon access for the MCP tools.

use serde_json::Value;

use atvv_daemon::Client;

/// Control surface behind the MCP tools. Tests inject a fake.
pub trait Backend: Send {
    fn status(&mut self) -> Result<Value, String>;
    fn voice_start(&mut self) -> Result<(), String>;
    fn voice_stop(&mut self) -> Result<(), String>;
    fn voice_toggle(&mut self) -> Result<(), String>;
    fn list_devices(&mut self) -> Result<Vec<String>, String>;
    fn reload(&mut self) -> Result<(), String>;
}

/// `org.atvv.Daemon` client, connected lazily.
///
/// The daemon may start after the MCP server; each call retries the session
/// bus when no connection exists yet.
#[derive(Default)]
pub struct DaemonBackend {
    client: Option<Client>,
}

impl DaemonBackend {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn client(&mut self) -> Result<&Client, String> {
        if self.client.is_none() {
            self.client = Some(
                Client::connect()
                    .map_err(|error| format!("cannot connect to the session D-Bus: {error}"))?,
            );
        }
        Ok(self.client.as_ref().expect("client was just set"))
    }
}

impl Backend for DaemonBackend {
    fn status(&mut self) -> Result<Value, String> {
        self.client()?
            .status_json()
            .map_err(|error| format!("GetStatus failed: {error}"))
    }

    fn voice_start(&mut self) -> Result<(), String> {
        self.client()?
            .start_voice()
            .map_err(|error| format!("StartVoice failed: {error}"))
    }

    fn voice_stop(&mut self) -> Result<(), String> {
        self.client()?
            .stop_voice()
            .map_err(|error| format!("StopVoice failed: {error}"))
    }

    fn voice_toggle(&mut self) -> Result<(), String> {
        self.client()?
            .toggle_voice()
            .map_err(|error| format!("ToggleVoice failed: {error}"))
    }

    fn list_devices(&mut self) -> Result<Vec<String>, String> {
        self.client()?
            .devices()
            .map_err(|error| format!("ListDevices failed: {error}"))
    }

    fn reload(&mut self) -> Result<(), String> {
        self.client()?
            .reload()
            .map_err(|error| format!("ReloadConfig failed: {error}"))
    }
}
