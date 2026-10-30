// SPDX-License-Identifier: GPL-3.0-or-later

//! Stable D-Bus view model.
//!
//! Protocol types never cross the D-Bus boundary. `GetStatus` returns this
//! struct as JSON and the signals carry the same state names.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use atvv_audio::{CHANNELS, CONTRACT_VERSION, MEDIA_CLASS, RECORDING_KEYS, SAMPLE_FORMAT};
use atvv_protocol::profile::DeviceProfile;
use serde::{Deserialize, Serialize};

pub type StatusCell = Arc<Mutex<StatusView>>;

#[must_use]
pub fn cell(view: StatusView) -> StatusCell {
    Arc::new(Mutex::new(view))
}

pub fn lock(cell: &StatusCell) -> MutexGuard<'_, StatusView> {
    cell.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusView {
    pub state: String,
    pub connected: bool,
    pub streaming: bool,
    pub device: DeviceView,
    pub caps: Option<CapsView>,
    pub audio: AudioView,
    pub input: InputView,
    pub vinput: VinputView,
    /// Provider contract: what consumers may bind and where the control
    /// endpoints are. Stable across sessions; `active_key` reflects the
    /// current input state.
    pub contract: ContractView,
    pub config: ConfigView,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceView {
    pub id: String,
    pub name: String,
    pub model: String,
    pub address: String,
    pub transport: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapsView {
    pub version: String,
    pub codec: String,
    pub codec_mask: u8,
    pub interaction: String,
    pub frame_size: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioView {
    pub source: String,
    pub available: bool,
    pub streaming: bool,
    pub queued_samples: usize,
    pub gain_db: i8,
    pub frames: u64,
    pub samples: u64,
    pub dropped_frames: u64,
    pub degraded: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputView {
    pub enabled: bool,
    pub available: bool,
    pub keyboard: bool,
    pub grabbed: Vec<String>,
    pub degraded: Vec<String>,
    pub last_button: Option<String>,
    pub voice_key: Option<bool>,
}

/// fcitx5-vinput integration state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VinputView {
    /// `trigger`, `dbus`, or `off`.
    pub mode: String,
    /// Synthetic key name in trigger mode.
    pub trigger: Option<String>,
    /// Whether the last `org.fcitx.Vinput` call reached the service.
    pub available: bool,
    pub last_error: Option<String>,
}

/// Provider contract as seen by consumers.
///
/// The declared keys and the source format are stable for a profile.
/// `active_key` is the key the provider will emit in trigger mode, even while
/// the input runtime is temporarily unavailable; `input_ready` and
/// `keyboard_ready` report whether it can be emitted right now. Consumers
/// should bind the declared set rather than only the active key, because
/// `trigger_key` may change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractView {
    pub version: String,
    pub profile: String,
    pub source: String,
    pub media_class: String,
    pub sample_format: String,
    pub sample_rate: u32,
    pub channels: u32,
    pub recording_keys: Vec<String>,
    pub active_key: Option<String>,
    pub input_ready: bool,
    pub keyboard_ready: bool,
    pub dbus: DbusView,
    pub mcp: McpView,
    pub recipe: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DbusView {
    pub name: String,
    pub path: String,
    pub interface: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpView {
    pub command: String,
    pub transport: String,
}

/// Build the provider contract for the current input and integration state.
#[must_use]
pub fn contract_view(
    profile: &DeviceProfile,
    source: &str,
    input: &InputView,
    vinput: &VinputView,
) -> ContractView {
    let input_ready = input.enabled && input.available;
    let keyboard_ready = input.keyboard;
    let active_key = (vinput.mode == crate::config::VinputMode::Trigger.name())
        .then(|| vinput.trigger.clone())
        .flatten();
    ContractView {
        version: CONTRACT_VERSION.to_string(),
        profile: profile.id.to_string(),
        source: source.to_string(),
        media_class: MEDIA_CLASS.to_string(),
        sample_format: SAMPLE_FORMAT.to_string(),
        sample_rate: profile.sample_rate,
        channels: CHANNELS,
        recording_keys: RECORDING_KEYS
            .iter()
            .map(|key| (*key).to_string())
            .collect(),
        active_key,
        input_ready,
        keyboard_ready,
        dbus: DbusView {
            name: crate::dbus::SERVICE.to_string(),
            path: crate::dbus::PATH.to_string(),
            interface: crate::dbus::INTERFACE.to_string(),
        },
        mcp: McpView {
            command: "atvv-mcp".to_string(),
            transport: "stdio".to_string(),
        },
        recipe: format!(
            "Select {source} as the capture source; bind {} or set the consumer shortcut to the active key.",
            RECORDING_KEYS.join(", ")
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigView {
    pub path: Option<String>,
    pub loaded: bool,
    pub gain_db: i8,
    pub input: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use atvv_protocol::profile::RC003;

    fn input_view(enabled: bool, available: bool, keyboard: bool) -> InputView {
        InputView {
            enabled,
            available,
            keyboard,
            grabbed: Vec::new(),
            degraded: Vec::new(),
            last_button: None,
            voice_key: None,
        }
    }

    fn vinput_view(mode: &str, trigger: Option<&str>) -> VinputView {
        VinputView {
            mode: mode.to_string(),
            trigger: trigger.map(str::to_string),
            available: false,
            last_error: None,
        }
    }

    #[test]
    fn contract_reports_the_declared_set_and_active_key() {
        let input = input_view(true, true, true);
        let vinput = vinput_view("trigger", Some("DICTATE"));
        let contract = contract_view(&RC003, "atvv-miremote-rc003", &input, &vinput);

        assert_eq!(contract.version, "1");
        assert_eq!(contract.profile, "miremote-rc003");
        assert_eq!(contract.source, "atvv-miremote-rc003");
        assert_eq!(contract.media_class, "Audio/Source");
        assert_eq!(contract.sample_format, "s16le");
        assert_eq!(contract.sample_rate, 16_000);
        assert_eq!(contract.channels, 1);
        assert_eq!(
            contract.recording_keys.join(","),
            "DICTATE,VOICECOMMAND,ASSISTANT"
        );
        assert_eq!(contract.active_key.as_deref(), Some("DICTATE"));
        assert!(contract.input_ready);
        assert!(contract.keyboard_ready);
        assert_eq!(contract.dbus.name, "org.atvv.Daemon");
        assert_eq!(contract.dbus.path, "/org/atvv/Daemon");
        assert_eq!(contract.dbus.interface, "org.atvv.Daemon");
        assert_eq!(contract.mcp.command, "atvv-mcp");
        assert_eq!(contract.mcp.transport, "stdio");
        assert!(contract.recipe.contains("atvv-miremote-rc003"));
        assert!(contract.recipe.contains("DICTATE"));
    }

    #[test]
    fn contract_reports_the_configured_key_and_current_input_state() {
        let ready_input = input_view(true, true, true);
        let trigger = vinput_view("trigger", Some("DICTATE"));

        let disabled = contract_view(&RC003, "atvv-x", &input_view(false, true, true), &trigger);
        assert_eq!(disabled.active_key.as_deref(), Some("DICTATE"));
        assert!(!disabled.input_ready);

        let absent = contract_view(&RC003, "atvv-x", &input_view(true, false, true), &trigger);
        assert_eq!(absent.active_key.as_deref(), Some("DICTATE"));
        assert!(!absent.input_ready);

        let no_keyboard = contract_view(&RC003, "atvv-x", &input_view(true, true, false), &trigger);
        assert_eq!(no_keyboard.active_key.as_deref(), Some("DICTATE"));
        assert!(no_keyboard.input_ready);
        assert!(!no_keyboard.keyboard_ready);

        let dbus = contract_view(
            &RC003,
            "atvv-x",
            &ready_input,
            &vinput_view("dbus", Some("DICTATE")),
        );
        assert_eq!(dbus.active_key, None);

        let no_trigger = contract_view(
            &RC003,
            "atvv-x",
            &ready_input,
            &vinput_view("trigger", None),
        );
        assert_eq!(no_trigger.active_key, None);
    }

    #[test]
    fn contract_round_trips_through_json() {
        let contract = contract_view(
            &RC003,
            "atvv-miremote-rc003",
            &input_view(true, true, true),
            &vinput_view("trigger", Some("DICTATE")),
        );
        let encoded = serde_json::to_string(&contract).expect("serialize contract");
        let decoded: ContractView = serde_json::from_str(&encoded).expect("deserialize contract");
        assert_eq!(decoded, contract);
    }
}
