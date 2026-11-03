// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared `daemon.toml` schema, validation, and editable configuration.
//!
//! This crate is the only parser for the configuration file. The daemon, the
//! CLI, and the GUI use [`FileConfig`] for TOML and [`ConfigDocument`] for the
//! JSON document exchanged over D-Bus.
//!
//! A **layout** is a named map of button actions plus gesture-timing
//! overrides. The device profile stays the hardware identity; layouts are the
//! user-facing key maps. `default` is synthesized from the profile, is never
//! stored, and acts as a template: editing it in the GUI stages a copy that a
//! save turns into a new user layout.
//!
//! Action strings use the existing grammar from [`atvv_input`]. A missing slot
//! means "use the profile default", not "unmapped": use the action `none` to
//! swallow a button without an action.
//!
//! `exec:` is disabled unless `allow_exec = true`. The daemon, never the input
//! runtime or a client, executes those actions.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

use atvv_input::{
    Action, ActionError, ActionMap, KeyResolver, Trigger, parse_action, parse_key_program,
    resolve_key,
};
use atvv_protocol::profile::{DeviceProfile, GestureTiming, RemoteButton, find};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Current schema version written by this crate.
pub const SCHEMA_VERSION: u32 = 3;
/// Flat button map schema; loaded once and migrated to [`SCHEMA_VERSION`].
pub const LEGACY_SCHEMA_VERSION: u32 = 1;
/// Layout schema with one active device; migrated to [`SCHEMA_VERSION`].
pub const V2_SCHEMA_VERSION: u32 = 2;
/// Gain clamp from the research notes: ±24 dB, no AGC.
pub const MAX_GAIN_DB: i8 = 24;
/// fcitx5-vinput trigger key used while the voice key is held.
pub const DEFAULT_TRIGGER_KEY: &str = "DICTATE";
/// Upper bound for an `exec:` command line. The command stays opaque.
pub const MAX_EXEC_COMMAND: usize = 4096;
/// Synthesized, read-only template layout derived from the device profile.
pub const DEFAULT_LAYOUT: &str = "default";
/// Display name of [`DEFAULT_LAYOUT`]; the GUI localizes it.
pub const DEFAULT_LAYOUT_NAME: &str = "Default";
/// Layout id created by the v1 migration.
pub const MIGRATED_LAYOUT: &str = "custom";
/// Display name of [`MIGRATED_LAYOUT`].
pub const MIGRATED_LAYOUT_NAME: &str = "Custom";
/// At most this many user layouts (the template is not counted).
pub const MAX_LAYOUTS: usize = 32;
/// Longest layout name in characters.
pub const MAX_LAYOUT_NAME: usize = 40;
/// Longest layout description in characters.
pub const MAX_LAYOUT_DESCRIPTION: usize = 120;
/// Longest device label in characters.
pub const MAX_DEVICE_LABEL: usize = 40;
/// Ids that user layouts may not take: the template and the built-in presets.
pub const RESERVED_LAYOUT_IDS: [&str; 4] = [DEFAULT_LAYOUT, "mpv", "slides", "omp"];

/// One built-in, read-only layout preset.
///
/// A preset is a template: applying it creates a user layout and never makes
/// the preset itself active. Every preset maps its declared buttons on a
/// single press; no slot is `none`. A layout built for a device profile drops
/// slots for buttons that profile does not have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutPreset {
    /// Stable id; also reserved against user layout ids.
    pub id: &'static str,
    /// Display name. App names such as `mpv` and `omp` stay untranslated.
    pub name: &'static str,
    /// One sentence shown in the picker.
    pub description: &'static str,
    /// Apps and versions the mappings were verified against.
    pub verified_with: &'static str,
    /// One action per preset button, keyed by configuration name.
    pub slots: [(&'static str, &'static str); 12],
    /// Optional long-press timing override.
    pub long_press_ms: Option<u16>,
}

/// Media playback with mpv's default key bindings.
///
/// Verified against mpv 0.41.0 default `input.conf`: Space pauses, `f` toggles
/// fullscreen, `m` mutes, the arrows seek, Enter advances the playlist, Esc
/// leaves fullscreen, and `q` quits.
pub const MPV_PRESET: LayoutPreset = LayoutPreset {
    id: "mpv",
    name: "mpv",
    description: "Media playback with mpv's default key bindings.",
    verified_with: "mpv 0.41.0",
    long_press_ms: None,
    slots: [
        ("power", "key:KEY_Q"),
        ("ok", "key:KEY_SPACE"),
        ("back", "key:KEY_ESC"),
        ("up", "key:KEY_UP"),
        ("down", "key:KEY_DOWN"),
        ("left", "key:KEY_LEFT"),
        ("right", "key:KEY_RIGHT"),
        ("home", "key:KEY_HOME"),
        ("menu", "key:KEY_M"),
        ("tv", "key:KEY_F"),
        ("volume_up", "key:KEY_VOLUMEUP"),
        ("volume_down", "key:KEY_VOLUMEDOWN"),
    ],
};

/// Presentation control shared by LibreOffice Impress and WPS Presentation.
///
/// PageUp/PageDown, B (black screen), F5 (start), Esc (end), and Home/End are
/// the standard presentation keys in both apps.
pub const SLIDES_PRESET: LayoutPreset = LayoutPreset {
    id: "slides",
    name: "Slides",
    description: "Presentation control for LibreOffice Impress and WPS Presentation.",
    verified_with: "LibreOffice Impress 26.8.1, WPS Office 12.1.2",
    long_press_ms: None,
    slots: [
        ("power", "key:KEY_END"),
        ("ok", "key:KEY_SPACE"),
        ("back", "key:KEY_ESC"),
        ("up", "key:KEY_PAGEUP"),
        ("down", "key:KEY_PAGEDOWN"),
        ("left", "key:KEY_LEFT"),
        ("right", "key:KEY_RIGHT"),
        ("home", "key:KEY_HOME"),
        ("menu", "key:KEY_B"),
        ("tv", "key:KEY_F5"),
        ("volume_up", "key:KEY_VOLUMEUP"),
        ("volume_down", "key:KEY_VOLUMEDOWN"),
    ],
};

/// Coding-agent session control with omp's default key bindings.
///
/// Verified against omp 18.14.2: Enter submits, Esc interrupts, `Ctrl+R`
/// searches prompt history, `Ctrl+O` expands tool output, `Shift+Tab` cycles
/// the thinking level, and `Alt+R` retries the last failed turn.
pub const OMP_PRESET: LayoutPreset = LayoutPreset {
    id: "omp",
    name: "omp",
    description: "Coding-agent session control with omp's default key bindings.",
    verified_with: "omp 18.14.2",
    long_press_ms: None,
    slots: [
        ("power", "combo:alt+r"),
        ("ok", "key:KEY_ENTER"),
        ("back", "key:KEY_ESC"),
        ("up", "key:KEY_UP"),
        ("down", "key:KEY_DOWN"),
        ("left", "key:KEY_LEFT"),
        ("right", "key:KEY_RIGHT"),
        ("home", "combo:ctrl+r"),
        ("menu", "combo:shift+tab"),
        ("tv", "combo:ctrl+o"),
        ("volume_up", "key:KEY_VOLUMEUP"),
        ("volume_down", "key:KEY_VOLUMEDOWN"),
    ],
};

/// The built-in preset catalog, in picker order.
pub const LAYOUT_PRESETS: [LayoutPreset; 3] = [MPV_PRESET, SLIDES_PRESET, OMP_PRESET];

/// Find a preset by its stable id.
#[must_use]
pub fn layout_preset(id: &str) -> Option<&'static LayoutPreset> {
    LAYOUT_PRESETS.iter().find(|preset| preset.id == id)
}

/// How the daemon reaches fcitx5-vinput.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VinputMode {
    /// Grab the voice key and synthesize `trigger_key` on press/release.
    #[default]
    Trigger,
    /// Call `org.fcitx.Vinput` `StartRecording`/`StopRecording` directly.
    Dbus,
    /// Do not integrate with vinput; the D-Bus/CLI session still works.
    Off,
}

impl VinputMode {
    /// Stable name used by status reports and diagnostics.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Trigger => "trigger",
            Self::Dbus => "dbus",
            Self::Off => "off",
        }
    }
}

/// Gesture timing overrides; absent fields keep the profile value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GestureOverrides {
    pub double_click_ms: Option<u16>,
    pub long_press_ms: Option<u16>,
    pub repeat_start_ms: Option<u16>,
    pub stable_release_ms: Option<u16>,
    pub suppression_ms: Option<u16>,
    pub repeat_interval_ms: Option<u16>,
    pub back_repeat_interval_ms: Option<u16>,
}

impl GestureOverrides {
    /// Whether every field keeps the profile value.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.double_click_ms.is_none()
            && self.long_press_ms.is_none()
            && self.repeat_start_ms.is_none()
            && self.stable_release_ms.is_none()
            && self.suppression_ms.is_none()
            && self.repeat_interval_ms.is_none()
            && self.back_repeat_interval_ms.is_none()
    }
}

/// Effective gesture timing as exchanged over D-Bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GestureSettings {
    pub double_click_ms: u16,
    pub long_press_ms: u16,
    pub repeat_start_ms: u16,
    pub stable_release_ms: u16,
    pub suppression_ms: u16,
    pub repeat_interval_ms: u16,
    pub back_repeat_interval_ms: u16,
}

impl From<GestureTiming> for GestureSettings {
    fn from(value: GestureTiming) -> Self {
        Self {
            double_click_ms: value.double_click_ms,
            long_press_ms: value.long_press_ms,
            repeat_start_ms: value.repeat_start_ms,
            stable_release_ms: value.stable_release_ms,
            suppression_ms: value.suppression_ms,
            repeat_interval_ms: value.repeat_interval_ms,
            back_repeat_interval_ms: value.back_repeat_interval_ms,
        }
    }
}

impl From<GestureSettings> for GestureTiming {
    fn from(value: GestureSettings) -> Self {
        Self {
            double_click_ms: value.double_click_ms,
            long_press_ms: value.long_press_ms,
            repeat_start_ms: value.repeat_start_ms,
            stable_release_ms: value.stable_release_ms,
            suppression_ms: value.suppression_ms,
            repeat_interval_ms: value.repeat_interval_ms,
            back_repeat_interval_ms: value.back_repeat_interval_ms,
        }
    }
}

impl GestureSettings {
    fn with_overrides(self, overrides: Option<&GestureOverrides>) -> Result<Self, ConfigError> {
        let Some(overrides) = overrides else {
            return Ok(self);
        };
        Ok(Self {
            double_click_ms: override_value(
                "double_click_ms",
                overrides.double_click_ms,
                50,
                2000,
                self.double_click_ms,
            )?,
            long_press_ms: override_value(
                "long_press_ms",
                overrides.long_press_ms,
                100,
                5000,
                self.long_press_ms,
            )?,
            repeat_start_ms: override_value(
                "repeat_start_ms",
                overrides.repeat_start_ms,
                50,
                2000,
                self.repeat_start_ms,
            )?,
            stable_release_ms: override_value(
                "stable_release_ms",
                overrides.stable_release_ms,
                0,
                2000,
                self.stable_release_ms,
            )?,
            suppression_ms: override_value(
                "suppression_ms",
                overrides.suppression_ms,
                0,
                2000,
                self.suppression_ms,
            )?,
            repeat_interval_ms: override_value(
                "repeat_interval_ms",
                overrides.repeat_interval_ms,
                10,
                1000,
                self.repeat_interval_ms,
            )?,
            back_repeat_interval_ms: override_value(
                "back_repeat_interval_ms",
                overrides.back_repeat_interval_ms,
                10,
                1000,
                self.back_repeat_interval_ms,
            )?,
        })
    }

    fn overrides_from(self, defaults: GestureTiming) -> GestureOverrides {
        GestureOverrides {
            double_click_ms: (self.double_click_ms != defaults.double_click_ms)
                .then_some(self.double_click_ms),
            long_press_ms: (self.long_press_ms != defaults.long_press_ms)
                .then_some(self.long_press_ms),
            repeat_start_ms: (self.repeat_start_ms != defaults.repeat_start_ms)
                .then_some(self.repeat_start_ms),
            stable_release_ms: (self.stable_release_ms != defaults.stable_release_ms)
                .then_some(self.stable_release_ms),
            suppression_ms: (self.suppression_ms != defaults.suppression_ms)
                .then_some(self.suppression_ms),
            repeat_interval_ms: (self.repeat_interval_ms != defaults.repeat_interval_ms)
                .then_some(self.repeat_interval_ms),
            back_repeat_interval_ms: (self.back_repeat_interval_ms
                != defaults.back_repeat_interval_ms)
                .then_some(self.back_repeat_interval_ms),
        }
    }
}

fn override_value(
    field: &'static str,
    value: Option<u16>,
    min: u16,
    max: u16,
    default: u16,
) -> Result<u16, ConfigError> {
    let Some(value) = value else {
        return Ok(default);
    };
    if !(min..=max).contains(&value) {
        return Err(ConfigError::GestureRange {
            field,
            value,
            min,
            max,
        });
    }
    Ok(value)
}

/// One optional action per gesture slot in the TOML file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ButtonToml {
    pub single: Option<String>,
    pub double: Option<String>,
    pub long: Option<String>,
}

impl ButtonToml {
    fn slots(&self) -> [(Trigger, Option<&String>); 3] {
        [
            (Trigger::Single, self.single.as_ref()),
            (Trigger::Double, self.double.as_ref()),
            (Trigger::Long, self.long.as_ref()),
        ]
    }

    fn is_empty(&self) -> bool {
        self.single.is_none() && self.double.is_none() && self.long.is_none()
    }
}

/// One optional action per gesture slot in the JSON document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ButtonDocument {
    pub single: Option<String>,
    pub double: Option<String>,
    pub long: Option<String>,
}

impl ButtonDocument {
    fn slots(&self) -> [(Trigger, Option<&String>); 3] {
        [
            (Trigger::Single, self.single.as_ref()),
            (Trigger::Double, self.double.as_ref()),
            (Trigger::Long, self.long.as_ref()),
        ]
    }

    /// One slot value; `None` means the profile default.
    #[must_use]
    pub fn slot(&self, trigger: Trigger) -> Option<&str> {
        match trigger {
            Trigger::Single => self.single.as_deref(),
            Trigger::Double => self.double.as_deref(),
            Trigger::Long => self.long.as_deref(),
        }
    }

    /// Replace one slot; `None` restores the profile default.
    pub fn set_slot(&mut self, trigger: Trigger, value: Option<String>) {
        match trigger {
            Trigger::Single => self.single = value,
            Trigger::Double => self.double = value,
            Trigger::Long => self.long = value,
        }
    }

    /// No slot carries an override.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.single.is_none() && self.double.is_none() && self.long.is_none()
    }
}

impl From<&ButtonToml> for ButtonDocument {
    fn from(value: &ButtonToml) -> Self {
        Self {
            single: value.single.clone(),
            double: value.double.clone(),
            long: value.long.clone(),
        }
    }
}

impl From<&ButtonDocument> for ButtonToml {
    fn from(value: &ButtonDocument) -> Self {
        Self {
            single: value.single.clone(),
            double: value.double.clone(),
            long: value.long.clone(),
        }
    }
}

/// One stored user layout in the TOML file.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutToml {
    pub name: String,
    pub description: Option<String>,
    pub gesture: Option<GestureOverrides>,
    #[serde(default)]
    pub buttons: BTreeMap<String, ButtonToml>,
}

impl LayoutToml {
    fn pointer(&self) -> LayoutDocument {
        LayoutDocument {
            name: self.name.clone(),
            description: self.description.clone(),
            gesture: self.gesture.clone(),
            buttons: self
                .buttons
                .iter()
                .filter(|(_, slots)| !slots.is_empty())
                .map(|(name, slots)| (name.clone(), slots.into()))
                .collect(),
        }
    }
}

/// A validated layout as exchanged over D-Bus and edited by the GUI.
///
/// Slots hold raw overrides: an absent slot means "profile default", and the
/// action `none` swallows the button. The `default` template is synthesized by
/// [`ConfigDocument::layout_summaries`] and never appears in `layouts`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutDocument {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub gesture: Option<GestureOverrides>,
    #[serde(default)]
    pub buttons: BTreeMap<String, ButtonDocument>,
}

impl LayoutDocument {
    /// One raw slot value; `None` means the profile default.
    #[must_use]
    pub fn slot(&self, button: RemoteButton, trigger: Trigger) -> Option<&str> {
        self.buttons.get(button.name())?.slot(trigger)
    }

    /// Replace one raw slot; `None` restores the profile default.
    pub fn set_slot(&mut self, button: RemoteButton, trigger: Trigger, value: Option<String>) {
        if value.is_none() {
            if let Some(slots) = self.buttons.get_mut(button.name()) {
                slots.set_slot(trigger, None);
                if slots.is_empty() {
                    self.buttons.remove(button.name());
                }
            }
            return;
        }
        self.buttons
            .entry(button.name().to_string())
            .or_default()
            .set_slot(trigger, value);
    }

    /// No slot or gesture override is stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buttons.values().all(ButtonDocument::is_empty)
            && self.gesture.as_ref().is_none_or(GestureOverrides::is_empty)
    }

    fn to_toml(&self) -> LayoutToml {
        LayoutToml {
            name: self.name.clone(),
            description: self.description.clone(),
            gesture: self.gesture.clone(),
            buttons: self
                .buttons
                .iter()
                .filter(|(_, slots)| !slots.is_empty())
                .map(|(name, slots)| (name.clone(), slots.into()))
                .collect(),
        }
    }
}

/// One known device in the TOML file.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceToml {
    /// Optional user label; BlueZ's alias is runtime-only.
    pub label: Option<String>,
    /// Device profile id.
    pub profile: String,
    /// Layout that drives this device while it is active.
    pub layout: Option<String>,
}

/// One known device in the JSON document.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceDocument {
    #[serde(default)]
    pub label: Option<String>,
    pub profile: String,
    /// Layout id; `default` is the synthesized template.
    #[serde(default = "default_layout_id")]
    pub layout: String,
}

fn default_layout_id() -> String {
    DEFAULT_LAYOUT.to_string()
}

/// Validated registry entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownDevice {
    pub label: Option<String>,
    pub profile: String,
    pub layout: String,
}

/// `[recording]` table: optional actions on the recording-key edges.
///
/// The values use the button grammar; `none` clears an edge.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecordingToml {
    pub press: Option<String>,
    pub release: Option<String>,
}

/// Raw `daemon.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileConfig {
    pub schema_version: Option<u32>,
    /// Active device address (schema 3).
    pub active_device: Option<String>,
    /// Known devices by Bluetooth address (schema 3).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub devices: BTreeMap<String, DeviceToml>,
    /// Device profile id; schema 1/2 only.
    pub profile: Option<String>,
    /// Bluetooth address written by `atvvctl pair`; schema 1/2 only.
    pub device: Option<String>,
    /// BlueZ adapter name such as `hci0`; the default adapter when absent.
    pub adapter: Option<String>,
    /// Override the profile gain in dB, clamped to ±24 dB.
    pub gain_db: Option<i8>,
    /// Start the evdev/uinput input path.
    pub input: Option<bool>,
    /// Open the microphone at ready and re-open it after a stop.
    pub preopen_mic: Option<bool>,
    /// fcitx5-vinput integration mode.
    pub vinput: Option<VinputMode>,
    /// Synthetic key emitted while the voice key is held; `none` disables it.
    pub trigger_key: Option<String>,
    /// Allow `exec:` actions. Disabled by default.
    pub allow_exec: Option<bool>,
    /// Actions on the recording-key edges.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recording: Option<RecordingToml>,
    /// The layout that drives the input runtime.
    pub active_layout: Option<String>,
    /// Named user layouts; `default` is synthesized and never stored.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub layouts: BTreeMap<String, LayoutToml>,
    /// Schema v1 gesture timing; migrated into [`MIGRATED_LAYOUT`].
    pub gesture: Option<GestureOverrides>,
    /// Schema v1 per-button overrides; migrated into [`MIGRATED_LAYOUT`].
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub buttons: BTreeMap<String, ButtonToml>,
}

/// Validated actions on the recording-key edges.
///
/// `None` means the edge runs nothing. Dictation is rejected at validation:
/// the physical recording key already owns the microphone.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecordingActions {
    pub press: Option<Action>,
    pub release: Option<Action>,
}

impl RecordingActions {
    /// Whether both edges are unset.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.press.is_none() && self.release.is_none()
    }
}

/// Validated configuration with the active layout resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub profile: String,
    /// Active device address; `None` before a device is paired.
    pub active_device: Option<String>,
    /// Known devices by normalized Bluetooth address.
    pub devices: BTreeMap<String, KnownDevice>,
    pub adapter: Option<String>,
    pub gain_db: i8,
    pub input: bool,
    /// Keep an ATVV microphone session open while not dictating.
    pub preopen_mic: bool,
    pub vinput: VinputMode,
    /// Key name; `None` disables synthetic trigger emission.
    pub trigger_key: Option<String>,
    pub allow_exec: bool,
    /// Actions on the recording-key press and release edges.
    pub recording: RecordingActions,
    /// Id of the layout that drives the runtime: `default` or a user layout.
    pub active_layout: String,
    /// Validated user layouts; the `default` template is not stored here.
    pub layouts: BTreeMap<String, LayoutDocument>,
    /// Effective gesture timing of [`Self::active_layout`].
    pub gesture: GestureTiming,
    /// Effective action map of [`Self::active_layout`].
    pub map: ActionMap,
}

impl Config {
    /// Defaults for a profile with no configuration file.
    #[must_use]
    pub fn defaults(profile: &DeviceProfile) -> Self {
        Self {
            profile: profile.id.to_string(),
            active_device: None,
            devices: BTreeMap::new(),
            adapter: None,
            gain_db: profile.gain_db,
            input: true,
            preopen_mic: false,
            vinput: VinputMode::Trigger,
            trigger_key: Some(DEFAULT_TRIGGER_KEY.to_string()),
            allow_exec: false,
            recording: RecordingActions::default(),
            active_layout: DEFAULT_LAYOUT.to_string(),
            layouts: BTreeMap::new(),
            gesture: profile.gesture_timing,
            map: ActionMap::defaults_for(profile),
        }
    }

    /// Display name of the active layout; the template uses its fixed name.
    #[must_use]
    pub fn active_layout_name(&self) -> &str {
        if self.active_layout == DEFAULT_LAYOUT {
            return DEFAULT_LAYOUT_NAME;
        }
        self.layouts
            .get(&self.active_layout)
            .map_or(self.active_layout.as_str(), |layout| layout.name.as_str())
    }

    /// Switch the active layout and resolve its map and gesture timing.
    pub fn select_layout(&mut self, id: &str, profile: &DeviceProfile) -> Result<(), ConfigError> {
        let (map, gesture) = resolve_layout(profile, id, &self.layouts, self.allow_exec)?;
        self.active_layout = id.to_string();
        if let Some(address) = &self.active_device
            && let Some(device) = self.devices.get_mut(address)
        {
            device.layout = id.to_string();
        }
        self.map = map;
        self.gesture = gesture;
        Ok(())
    }

    /// Make a known address active and load its profile and layout.
    pub fn select_device(&mut self, address: &str) -> Result<DeviceProfile, ConfigError> {
        let address = normalize_address(address)?;
        let device = self
            .devices
            .get(&address)
            .ok_or_else(|| ConfigError::UnknownDevice(address.clone()))?
            .clone();
        let profile = find(&device.profile)
            .ok_or_else(|| ConfigError::UnknownProfile(device.profile.clone()))?;
        self.active_device = Some(address);
        self.profile = profile.id.to_string();
        self.active_layout = device.layout;
        let (map, gesture) =
            resolve_layout(profile, &self.active_layout, &self.layouts, self.allow_exec)?;
        self.map = map;
        self.gesture = gesture;
        Ok(*profile)
    }

    /// Add an address to the registry if absent and make it active.
    ///
    /// An existing entry keeps its profile and layout: an explicit CLI
    /// `--device` override must not silently rewrite the saved device.
    pub fn adopt_device(&mut self, address: &str, profile: &DeviceProfile) {
        let Ok(address) = normalize_address(address) else {
            return;
        };
        self.devices
            .entry(address.clone())
            .or_insert_with(|| KnownDevice {
                label: None,
                profile: profile.id.to_string(),
                layout: DEFAULT_LAYOUT.to_string(),
            });
        self.active_device = Some(address);
        self.profile = profile.id.to_string();
    }

    /// Profile configured for an address, if the address is known.
    #[must_use]
    pub fn profile_for_address(&self, address: &str) -> Option<&'static DeviceProfile> {
        normalize_address(address)
            .ok()
            .and_then(|address| self.devices.get(&address))
            .and_then(|device| find(&device.profile))
    }

    /// Profile of the active device, falling back to the loaded profile.
    #[must_use]
    pub fn active_profile(&self) -> Option<&'static DeviceProfile> {
        self.active_device
            .as_deref()
            .and_then(|address| self.profile_for_address(address))
            .or_else(|| find(&self.profile))
    }
}

/// A layout entry for selectors and CLI listings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// The synthesized `default` template rather than a stored layout.
    pub template: bool,
}

/// A loaded configuration file.
#[derive(Debug, Clone)]
pub struct LoadedConfig {
    pub path: PathBuf,
    pub existed: bool,
    pub config: Config,
}

/// `$XDG_CONFIG_HOME/atvv/daemon.toml`, or `~/.config/atvv/daemon.toml`.
#[must_use]
pub fn default_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("atvv").join("daemon.toml")
}

/// Whether `text` is a colon-separated 48-bit Bluetooth address.
#[must_use]
pub fn is_bluetooth_address(text: &str) -> bool {
    let parts: Vec<&str> = text.split(':').collect();
    parts.len() == 6
        && parts
            .iter()
            .all(|part| part.len() == 2 && part.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

/// Normalize a Bluetooth address to uppercase BlueZ form.
pub fn normalize_address(text: &str) -> Result<String, ConfigError> {
    let text = text.trim();
    if !is_bluetooth_address(text) {
        return Err(ConfigError::Address(text.to_string()));
    }
    Ok(text.to_ascii_uppercase())
}

/// Whether `id` matches `[a-z0-9][a-z0-9_-]{0,31}`.
#[must_use]
pub fn layout_id_is_valid(id: &str) -> bool {
    if id.is_empty() || id.len() > 32 {
        return false;
    }
    let bytes = id.as_bytes();
    (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_' || *byte == b'-'
        })
}

/// Whether `id` is the template or a reserved preset id.
#[must_use]
pub fn layout_id_is_reserved(id: &str) -> bool {
    RESERVED_LAYOUT_IDS.contains(&id)
}

/// Derive a valid layout id from a display name.
#[must_use]
pub fn slug_layout_id(name: &str) -> String {
    let mut id = String::new();
    for ch in name.chars() {
        let ch = ch.to_ascii_lowercase();
        if ch.is_ascii_alphanumeric() {
            if id.len() == 32 {
                break;
            }
            id.push(ch);
        } else if !id.is_empty() && !id.ends_with('-') && id.len() < 32 {
            id.push('-');
        }
    }
    while id.ends_with('-') {
        id.pop();
    }
    if id.is_empty() {
        "layout".to_string()
    } else {
        id
    }
}

/// Return `base` or a suffixed id that no existing layout uses.
///
/// A reserved base gains `-layout`, so duplicating `Default` cannot produce
/// the template id.
#[must_use]
pub fn unique_layout_id<'a>(base: &str, existing: impl IntoIterator<Item = &'a str>) -> String {
    let existing: BTreeSet<&str> = existing.into_iter().collect();
    let mut base = base.to_string();
    if layout_id_is_reserved(&base) {
        base.push_str("-layout");
    }
    if base.len() > 32 {
        base.truncate(32);
        while base.ends_with('-') {
            base.pop();
        }
    }
    if !existing.contains(base.as_str()) {
        return base;
    }
    let mut suffix = 2;
    loop {
        let tail = format!("-{suffix}");
        let keep = 32usize.saturating_sub(tail.len());
        let mut candidate = base[..base.len().min(keep)].to_string();
        while candidate.ends_with('-') {
            candidate.pop();
        }
        candidate.push_str(&tail);
        if !existing.contains(candidate.as_str()) {
            return candidate;
        }
        suffix += 1;
    }
}

/// Return `base` or a suffixed name that no existing layout uses.
///
/// `Default` is reserved as a display name because it denotes the template.
#[must_use]
pub fn unique_layout_name<'a>(base: &str, existing: impl IntoIterator<Item = &'a str>) -> String {
    let existing: BTreeSet<String> = existing.into_iter().map(str::to_ascii_lowercase).collect();
    let reserved = base.eq_ignore_ascii_case(DEFAULT_LAYOUT_NAME);
    if !reserved && !existing.contains(&base.to_ascii_lowercase()) {
        return base.to_string();
    }
    let mut suffix = 2;
    loop {
        let candidate = format!("{base} {suffix}");
        if !existing.contains(&candidate.to_ascii_lowercase()) {
            return candidate;
        }
        suffix += 1;
    }
}

fn parse_trigger_key(name: &str) -> Result<Option<String>, ConfigError> {
    let name = name.trim();
    if name.is_empty() || name.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    if parse_key_program(name).is_none() {
        return Err(ConfigError::TriggerKey(name.to_string()));
    }
    Ok(Some(name.to_string()))
}

fn button_by_name(name: &str) -> Option<RemoteButton> {
    RemoteButton::ALL
        .iter()
        .copied()
        .find(|button| button.name() == name)
}

/// Validate one action string for configuration use.
///
/// Checks the grammar, key names, duplicate chord keys, the `exec` policy, and
/// the command bound. It does not execute anything.
pub fn parse_checked_action(raw: &str, allow_exec: bool) -> Result<Action, ActionCheckError> {
    let action = parse_action(raw)?;
    match &action {
        Action::Exec { command } => {
            if !allow_exec {
                return Err(ActionCheckError::ExecDisabled);
            }
            if command.len() > MAX_EXEC_COMMAND {
                return Err(ActionCheckError::ExecTooLong);
            }
        }
        Action::Key { .. } | Action::Combo { .. } => {
            if let Some(key) = duplicate_chord_key(&action) {
                return Err(ActionCheckError::DuplicateChordKey(key));
            }
            KeyResolver.program(&action)?;
        }
        Action::None | Action::Dictation(_) | Action::Pointer(_) => {}
    }
    Ok(action)
}

/// Validate one action string for a recording-key edge.
///
/// `none` disables the edge. The rest of the button grammar applies, except
/// `dictation:*`: the physical key already starts and stops the ATVV
/// microphone, so a second dictation source would race it.
pub fn parse_recording_action(
    raw: &str,
    allow_exec: bool,
) -> Result<Option<Action>, ActionCheckError> {
    let action = parse_checked_action(raw, allow_exec)?;
    match action {
        Action::None => Ok(None),
        Action::Dictation(_) => Err(ActionCheckError::RecordingDictation),
        action => Ok(Some(action)),
    }
}

/// Validate the optional `[recording]` table.
fn validate_recording(
    recording: Option<&RecordingToml>,
    allow_exec: bool,
) -> Result<RecordingActions, ConfigError> {
    let Some(recording) = recording else {
        return Ok(RecordingActions::default());
    };
    let edge = |edge: &'static str, raw: &Option<String>| {
        let Some(raw) = raw.as_deref() else {
            return Ok(None);
        };
        parse_recording_action(raw, allow_exec).map_err(|source| ConfigError::RecordingAction {
            edge,
            value: raw.to_string(),
            source,
        })
    };
    Ok(RecordingActions {
        press: edge("press", &recording.press)?,
        release: edge("release", &recording.release)?,
    })
}

/// Return the first key that appears twice in a chord.
fn duplicate_chord_key(action: &Action) -> Option<String> {
    let Action::Combo { mods, key } = action else {
        return None;
    };
    let mut seen = BTreeMap::new();
    for name in mods.iter().chain(std::iter::once(key)) {
        let code = resolve_key(name)?;
        if seen.insert(code, name).is_some() {
            return Some(name.clone());
        }
    }
    None
}

fn validate_layout(
    id: &str,
    layout: &LayoutToml,
    allow_exec: bool,
    profile: &DeviceProfile,
    names: &mut BTreeSet<String>,
) -> Result<LayoutDocument, ConfigError> {
    if !layout_id_is_valid(id) {
        return Err(ConfigError::LayoutId(id.to_string()));
    }
    if layout_id_is_reserved(id) {
        return Err(ConfigError::LayoutReserved(id.to_string()));
    }
    let name = layout.name.trim();
    if name.is_empty() || name.chars().count() > MAX_LAYOUT_NAME {
        return Err(ConfigError::LayoutName(id.to_string()));
    }
    if name.eq_ignore_ascii_case(DEFAULT_LAYOUT_NAME) {
        return Err(ConfigError::LayoutNameReserved(id.to_string()));
    }
    if !names.insert(name.to_ascii_lowercase()) {
        return Err(ConfigError::LayoutNameTaken {
            id: id.to_string(),
            name: name.to_string(),
        });
    }
    let description = layout
        .description
        .as_deref()
        .map(str::trim)
        .filter(|description| !description.is_empty());
    if description.is_some_and(|description| description.chars().count() > MAX_LAYOUT_DESCRIPTION) {
        return Err(ConfigError::LayoutDescription(id.to_string()));
    }

    let mut document = layout.pointer();
    document.name = name.to_string();
    document.description = description.map(str::to_string);
    for (button_name, slots) in &layout.buttons {
        let button = button_by_name(button_name)
            .ok_or_else(|| ConfigError::UnknownButton(button_name.clone()))?;
        if button == RemoteButton::Voice {
            return Err(ConfigError::VoiceMapped);
        }
        for (trigger, value) in slots.slots() {
            let Some(raw) = value else {
                continue;
            };
            parse_checked_action(raw, allow_exec).map_err(|source| ConfigError::LayoutAction {
                layout: id.to_string(),
                button: button_name.clone(),
                trigger: trigger.name(),
                value: raw.clone(),
                source,
            })?;
        }
    }
    GestureSettings::from(profile.gesture_timing)
        .with_overrides(layout.gesture.as_ref())
        .map_err(|error| ConfigError::LayoutGesture {
            layout: id.to_string(),
            source: Box::new(error),
        })?;
    Ok(document)
}

fn resolve_layout(
    profile: &DeviceProfile,
    id: &str,
    layouts: &BTreeMap<String, LayoutDocument>,
    allow_exec: bool,
) -> Result<(ActionMap, GestureTiming), ConfigError> {
    let mut map = ActionMap::defaults_for(profile);
    let mut gesture = profile.gesture_timing;
    if id == DEFAULT_LAYOUT {
        return Ok((map, gesture));
    }
    let layout = layouts
        .get(id)
        .ok_or_else(|| ConfigError::UnknownLayout(id.to_string()))?;
    for (button_name, slots) in &layout.buttons {
        let button = button_by_name(button_name)
            .ok_or_else(|| ConfigError::UnknownButton(button_name.clone()))?;
        if button == RemoteButton::Voice {
            return Err(ConfigError::VoiceMapped);
        }
        for (trigger, value) in slots.slots() {
            let Some(raw) = value else {
                continue;
            };
            let action = parse_checked_action(raw, allow_exec).map_err(|source| {
                ConfigError::LayoutAction {
                    layout: id.to_string(),
                    button: button_name.clone(),
                    trigger: trigger.name(),
                    value: raw.clone(),
                    source,
                }
            })?;
            map.set(button, trigger, action)
                .map_err(|error| ConfigError::LayoutAction {
                    layout: id.to_string(),
                    button: button_name.clone(),
                    trigger: trigger.name(),
                    value: raw.clone(),
                    source: ActionCheckError::Grammar(error),
                })?;
        }
    }
    gesture = GestureSettings::from(profile.gesture_timing)
        .with_overrides(layout.gesture.as_ref())
        .map_err(|error| ConfigError::LayoutGesture {
            layout: id.to_string(),
            source: Box::new(error),
        })?
        .into();
    Ok((map, gesture))
}

impl FileConfig {
    /// Validate every field, migrate older schemas, and resolve the active
    /// device's layout against the profile defaults.
    pub fn validate(&self, fallback: &DeviceProfile) -> Result<Config, ConfigError> {
        let mut file = self.migrated(fallback)?;
        let active_device = match file.active_device.take() {
            Some(address) => Some(normalize_address(&address)?),
            None => None,
        };
        if file.layouts.len() > MAX_LAYOUTS {
            return Err(ConfigError::TooManyLayouts);
        }
        let mut names = BTreeSet::new();
        let mut layouts = BTreeMap::new();
        for (id, layout) in &file.layouts {
            layouts.insert(
                id.clone(),
                validate_layout(
                    id,
                    layout,
                    file.allow_exec.unwrap_or(false),
                    fallback,
                    &mut names,
                )?,
            );
        }
        let mut devices = BTreeMap::new();
        for (raw_address, device) in &file.devices {
            let address = normalize_address(raw_address)?;
            if let Some(label) = &device.label {
                let label = label.trim();
                if label.is_empty() || label.chars().count() > MAX_DEVICE_LABEL {
                    return Err(ConfigError::DeviceLabel(address));
                }
            }
            if atvv_protocol::profile::find(&device.profile).is_none() {
                return Err(ConfigError::UnknownProfile(device.profile.clone()));
            }
            let layout = device
                .layout
                .clone()
                .unwrap_or_else(|| DEFAULT_LAYOUT.to_string());
            if layout != DEFAULT_LAYOUT && !layouts.contains_key(&layout) {
                return Err(ConfigError::ActiveLayout(layout));
            }
            if devices
                .insert(
                    address.clone(),
                    KnownDevice {
                        label: device.label.clone(),
                        profile: device.profile.clone(),
                        layout,
                    },
                )
                .is_some()
            {
                return Err(ConfigError::DuplicateDevice(address));
            }
        }
        let active_profile = match &active_device {
            Some(address) => {
                let device = devices
                    .get(address)
                    .ok_or_else(|| ConfigError::UnknownDevice(address.clone()))?;
                atvv_protocol::profile::find(&device.profile)
                    .ok_or_else(|| ConfigError::UnknownProfile(device.profile.clone()))?
            }
            None => fallback,
        };
        let active_layout = if let Some(address) = &active_device {
            devices.get(address).map_or_else(
                || DEFAULT_LAYOUT.to_string(),
                |device| device.layout.clone(),
            )
        } else {
            file.active_layout
                .clone()
                .unwrap_or_else(|| DEFAULT_LAYOUT.to_string())
        };
        if active_layout != DEFAULT_LAYOUT && !layouts.contains_key(&active_layout) {
            return Err(ConfigError::ActiveLayout(active_layout));
        }
        let gain_db = file.gain_db.unwrap_or(active_profile.gain_db);
        if !(-MAX_GAIN_DB..=MAX_GAIN_DB).contains(&gain_db) {
            return Err(ConfigError::GainRange(gain_db));
        }
        let trigger_key = match file.trigger_key.as_deref() {
            Some(name) => parse_trigger_key(name)?,
            None => Some(DEFAULT_TRIGGER_KEY.to_string()),
        };
        let allow_exec = file.allow_exec.unwrap_or(false);
        let recording = validate_recording(file.recording.as_ref(), allow_exec)?;
        let (map, gesture) = resolve_layout(active_profile, &active_layout, &layouts, allow_exec)?;

        Ok(Config {
            profile: active_profile.id.to_string(),
            active_device,
            devices,
            adapter: file.adapter.clone(),
            gain_db,
            input: file.input.unwrap_or(true),
            preopen_mic: file.preopen_mic.unwrap_or(false),
            vinput: file.vinput.unwrap_or_default(),
            trigger_key,
            allow_exec,
            recording,
            active_layout,
            layouts,
            gesture,
            map,
        })
    }

    /// Move a flat v1 button map into the `custom` layout, then convert the
    /// single `device` field into the schema 3 registry.
    ///
    /// A v2 file must not carry v3 fields, and a v3 file must not carry the
    /// legacy tables. The returned file is always schema version 3.
    fn migrated(&self, fallback: &DeviceProfile) -> Result<FileConfig, ConfigError> {
        let mut file = self.clone();
        if file.schema_version == Some(SCHEMA_VERSION) {
            if file.profile.is_some()
                || file.device.is_some()
                || !file.buttons.is_empty()
                || file.gesture.is_some()
            {
                return Err(ConfigError::LegacyTables);
            }
            return Ok(file);
        }
        if file.schema_version == Some(V2_SCHEMA_VERSION) {
            if file.active_device.is_some() || !file.devices.is_empty() {
                return Err(ConfigError::MixedSchema);
            }
            if !file.buttons.is_empty() || file.gesture.is_some() {
                return Err(ConfigError::LegacyTables);
            }
        }
        if file.schema_version.is_none() || file.schema_version == Some(LEGACY_SCHEMA_VERSION) {
            if !file.layouts.is_empty() || file.active_layout.is_some() {
                return Err(ConfigError::MixedSchema);
            }
            let has_overrides = file.buttons.values().any(|slots| !slots.is_empty())
                || file
                    .gesture
                    .as_ref()
                    .is_some_and(|gesture| !gesture.is_empty());
            if has_overrides {
                let mut buttons = BTreeMap::new();
                for (name, slots) in std::mem::take(&mut file.buttons) {
                    if !slots.is_empty() {
                        buttons.insert(name, slots);
                    }
                }
                file.layouts.insert(
                    MIGRATED_LAYOUT.to_string(),
                    LayoutToml {
                        name: MIGRATED_LAYOUT_NAME.to_string(),
                        description: None,
                        gesture: file.gesture.take(),
                        buttons,
                    },
                );
                file.active_layout = Some(MIGRATED_LAYOUT.to_string());
            } else {
                file.buttons.clear();
                file.gesture = None;
                file.active_layout = Some(DEFAULT_LAYOUT.to_string());
            }
            file.schema_version = Some(V2_SCHEMA_VERSION);
        }
        if file.schema_version != Some(V2_SCHEMA_VERSION) {
            return Err(ConfigError::SchemaVersion {
                found: file.schema_version.unwrap_or(0),
            });
        }

        if let Some(address) = file.device.take() {
            let profile_id = file
                .profile
                .take()
                .unwrap_or_else(|| fallback.id.to_string());
            let layout = file
                .active_layout
                .take()
                .unwrap_or_else(|| DEFAULT_LAYOUT.to_string());
            let address = normalize_address(&address)?;
            file.devices.insert(
                address.clone(),
                DeviceToml {
                    label: None,
                    profile: profile_id,
                    layout: Some(layout),
                },
            );
            file.active_device = Some(address);
        } else {
            file.profile = None;
        }
        file.schema_version = Some(SCHEMA_VERSION);
        Ok(file)
    }
}

/// Load and validate a configuration file.
///
/// A missing file is not an error: it yields the profile defaults.
pub fn load(path: &Path, profile: &DeviceProfile) -> Result<LoadedConfig, ConfigError> {
    if !path.exists() {
        return Ok(LoadedConfig {
            path: path.to_path_buf(),
            existed: false,
            config: Config::defaults(profile),
        });
    }
    let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let file: FileConfig = toml::from_str(&text).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(LoadedConfig {
        path: path.to_path_buf(),
        existed: true,
        config: file.validate(profile)?,
    })
}

/// Record a paired device and adapter without discarding layouts.
///
/// The device becomes active. An existing entry keeps its label and layout;
/// only the profile is refreshed. The file is migrated to the current schema
/// in the same write; `legacy_profile` is the profile of the single device a
/// schema-1/2 file stores, and is ignored for schema 3. A malformed existing
/// file is an error: the daemon should not silently replace a typo with
/// defaults.
pub fn save_device(
    path: &Path,
    device: &str,
    profile: &DeviceProfile,
    legacy_profile: &DeviceProfile,
    adapter: Option<&str>,
) -> Result<(), ConfigError> {
    let address = normalize_address(device)?;
    let mut file = if path.exists() {
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let file: FileConfig = toml::from_str(&text).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
        // A schema-1/2 file stores one device without a profile. Migration
        // must use that device's profile, not the one being paired: pairing a
        // second device must not relabel the first.
        file.migrated(legacy_profile)?
    } else {
        FileConfig {
            schema_version: Some(SCHEMA_VERSION),
            ..FileConfig::default()
        }
    };
    let fallback_layout = file
        .active_layout
        .clone()
        .unwrap_or_else(|| DEFAULT_LAYOUT.to_string());
    file.devices
        .entry(address.clone())
        .and_modify(|device| device.profile = profile.id.to_string())
        .or_insert_with(|| DeviceToml {
            label: None,
            profile: profile.id.to_string(),
            layout: Some(fallback_layout.clone()),
        });
    if let Some(device) = file.devices.get_mut(&address)
        && device.layout.is_none()
    {
        device.layout = Some(fallback_layout);
    }
    file.active_device = Some(address);
    file.active_layout = None;
    if let Some(adapter) = adapter {
        file.adapter = Some(adapter.to_string());
    }
    write_file(path, &file)
}

/// Raw action strings for the recording-key edges in [`ConfigDocument`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecordingDocument {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub press: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<String>,
}

impl RecordingDocument {
    /// Whether both edges are unset.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.press.is_none() && self.release.is_none()
    }
}

/// Stable JSON view of the editable configuration.
///
/// `schema_version` versions this payload. `layouts` holds raw overrides for
/// every user layout; the `default` template is synthesized and never stored.
/// A missing slot means "profile default"; the action `none` means "swallow
/// the button".
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigDocument {
    pub schema_version: u32,
    /// Active profile id, derived from the active device entry.
    pub profile: String,
    pub active_device: Option<String>,
    #[serde(default)]
    pub devices: BTreeMap<String, DeviceDocument>,
    pub adapter: Option<String>,
    pub gain_db: i8,
    pub input: bool,
    #[serde(default)]
    pub preopen_mic: bool,
    pub vinput: VinputMode,
    pub trigger_key: Option<String>,
    pub allow_exec: bool,
    /// Actions on the recording-key edges; absent means both are unset.
    #[serde(default)]
    pub recording: RecordingDocument,
    /// Id of the active layout: `default` or a key in [`Self::layouts`].
    pub active_layout: String,
    #[serde(default)]
    pub layouts: BTreeMap<String, LayoutDocument>,
}

impl ConfigDocument {
    /// Build the editable document from a validated configuration.
    #[must_use]
    pub fn from_config(config: &Config) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            profile: config.profile.clone(),
            active_device: config.active_device.clone(),
            devices: config
                .devices
                .iter()
                .map(|(address, device)| {
                    (
                        address.clone(),
                        DeviceDocument {
                            label: device.label.clone(),
                            profile: device.profile.clone(),
                            layout: device.layout.clone(),
                        },
                    )
                })
                .collect(),
            adapter: config.adapter.clone(),
            gain_db: config.gain_db,
            input: config.input,
            preopen_mic: config.preopen_mic,
            vinput: config.vinput,
            trigger_key: config.trigger_key.clone(),
            allow_exec: config.allow_exec,
            recording: RecordingDocument {
                press: config.recording.press.as_ref().map(ToString::to_string),
                release: config.recording.release.as_ref().map(ToString::to_string),
            },
            active_layout: config.active_layout.clone(),
            layouts: config.layouts.clone(),
        }
    }

    /// Profile of the active device, falling back to the document profile.
    #[must_use]
    pub fn active_profile(&self) -> Option<&'static DeviceProfile> {
        self.active_device
            .as_ref()
            .and_then(|address| self.devices.get(address))
            .and_then(|device| find(&device.profile))
            .or_else(|| find(&self.profile))
    }

    /// Layouts for a selector: the template first, then user layouts by id.
    #[must_use]
    pub fn layout_summaries(&self) -> Vec<LayoutSummary> {
        let mut summaries = vec![LayoutSummary {
            id: DEFAULT_LAYOUT.to_string(),
            name: DEFAULT_LAYOUT_NAME.to_string(),
            description: None,
            template: true,
        }];
        summaries.extend(self.layouts.iter().map(|(id, layout)| LayoutSummary {
            id: id.clone(),
            name: layout.name.clone(),
            description: layout.description.clone(),
            template: false,
        }));
        summaries
    }

    /// Resolve any layout id to its effective map and gesture timing.
    pub fn resolved_layout(
        &self,
        id: &str,
        profile: &DeviceProfile,
    ) -> Result<(ActionMap, GestureTiming), ConfigError> {
        let mut document = self.clone();
        document.active_layout = id.to_string();
        let config = document.validate(profile)?;
        Ok((config.map, config.gesture))
    }

    /// Validate the document and produce the effective configuration.
    pub fn validate(&self, profile: &DeviceProfile) -> Result<Config, ConfigError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(ConfigError::SchemaVersion {
                found: self.schema_version,
            });
        }
        self.to_file_config().validate(profile)
    }

    /// Minimal TOML representation relative to the profile defaults.
    #[must_use]
    pub fn to_file(&self, profile: &DeviceProfile) -> FileConfig {
        let mut layouts = BTreeMap::new();
        for (id, layout) in &self.layouts {
            layouts.insert(id.clone(), layout_to_toml(layout, profile));
        }
        let trigger_key = match self.trigger_key.as_deref() {
            Some(DEFAULT_TRIGGER_KEY) => None,
            Some(key) => Some(key.to_string()),
            None => Some("none".to_string()),
        };
        let devices = self
            .devices
            .iter()
            .map(|(address, device)| {
                (
                    address.clone(),
                    DeviceToml {
                        label: device.label.clone(),
                        profile: device.profile.clone(),
                        layout: Some(if self.active_device.as_deref() == Some(address.as_str()) {
                            self.active_layout.clone()
                        } else {
                            device.layout.clone()
                        }),
                    },
                )
            })
            .collect();
        FileConfig {
            schema_version: Some(SCHEMA_VERSION),
            active_device: self.active_device.clone(),
            devices,
            profile: None,
            device: None,
            adapter: self.adapter.clone(),
            gain_db: (self.gain_db != profile.gain_db).then_some(self.gain_db),
            input: (!self.input).then_some(false),
            preopen_mic: self.preopen_mic.then_some(true),
            vinput: (self.vinput != VinputMode::Trigger).then_some(self.vinput),
            trigger_key,
            allow_exec: self.allow_exec.then_some(true),
            recording: (!self.recording.is_empty()).then(|| RecordingToml {
                press: self.recording.press.clone(),
                release: self.recording.release.clone(),
            }),
            active_layout: self
                .active_device
                .is_none()
                .then(|| self.active_layout.clone()),
            layouts,
            gesture: None,
            buttons: BTreeMap::new(),
        }
    }

    fn to_file_config(&self) -> FileConfig {
        let devices = self
            .devices
            .iter()
            .map(|(address, device)| {
                (
                    address.clone(),
                    DeviceToml {
                        label: device.label.clone(),
                        profile: device.profile.clone(),
                        layout: Some(if self.active_device.as_deref() == Some(address.as_str()) {
                            self.active_layout.clone()
                        } else {
                            device.layout.clone()
                        }),
                    },
                )
            })
            .collect();
        FileConfig {
            schema_version: Some(self.schema_version),
            active_device: self.active_device.clone(),
            devices,
            profile: None,
            device: None,
            adapter: self.adapter.clone(),
            gain_db: Some(self.gain_db),
            input: Some(self.input),
            preopen_mic: Some(self.preopen_mic),
            vinput: Some(self.vinput),
            trigger_key: self.trigger_key.clone(),
            allow_exec: Some(self.allow_exec),
            recording: (!self.recording.is_empty()).then(|| RecordingToml {
                press: self.recording.press.clone(),
                release: self.recording.release.clone(),
            }),
            active_layout: self
                .active_device
                .is_none()
                .then(|| self.active_layout.clone()),
            layouts: self
                .layouts
                .iter()
                .map(|(id, layout)| (id.clone(), layout.to_toml()))
                .collect(),
            gesture: None,
            buttons: BTreeMap::new(),
        }
    }
}

impl LayoutPreset {
    /// Build a user layout from this preset for one device profile.
    ///
    /// Slots for buttons the profile does not have are dropped, so applying a
    /// preset to a Chromecast never creates a `menu` slot. The layout carries
    /// every remaining action on its single-press slot. Double and long
    /// presses keep the profile defaults so the preset does not add gesture
    /// latency.
    #[must_use]
    pub fn layout_for(&self, profile: &DeviceProfile) -> LayoutDocument {
        let buttons = self
            .slots
            .iter()
            .filter_map(|(name, action)| {
                let button = button_by_name(name)?;
                profile.has_button(button).then(|| {
                    (
                        (*name).to_string(),
                        ButtonDocument {
                            single: Some((*action).to_string()),
                            double: None,
                            long: None,
                        },
                    )
                })
            })
            .collect();
        LayoutDocument {
            name: self.name.to_string(),
            description: Some(self.description.to_string()),
            gesture: self.long_press_ms.map(|long_press_ms| GestureOverrides {
                long_press_ms: Some(long_press_ms),
                ..GestureOverrides::default()
            }),
            buttons,
        }
    }

    /// Create a user layout from this preset without switching to it.
    ///
    /// `name` defaults to the preset display name. The document is only
    /// changed after the result validates against `profile`.
    pub fn apply(
        &self,
        document: &mut ConfigDocument,
        id: &str,
        name: Option<&str>,
        profile: &DeviceProfile,
    ) -> Result<(), ConfigError> {
        if !layout_id_is_valid(id) {
            return Err(ConfigError::LayoutId(id.to_string()));
        }
        if layout_id_is_reserved(id) {
            return Err(ConfigError::LayoutReserved(id.to_string()));
        }
        if document.layouts.contains_key(id) {
            return Err(ConfigError::LayoutExists(id.to_string()));
        }
        if document.layouts.len() >= MAX_LAYOUTS {
            return Err(ConfigError::TooManyLayouts);
        }
        let mut next = document.clone();
        let mut layout = self.layout_for(profile);
        if let Some(name) = name {
            let name = name.trim();
            if name.is_empty() {
                return Err(ConfigError::LayoutName(id.to_string()));
            }
            layout.name = name.to_string();
        }
        next.layouts.insert(id.to_string(), layout);
        next.validate(profile)?;
        *document = next;
        Ok(())
    }

    /// Replace one user layout's slots with this preset.
    ///
    /// The layout keeps its name and description; the active layout does not
    /// change. `default` and unknown ids are rejected.
    pub fn reset_to(
        &self,
        document: &mut ConfigDocument,
        id: &str,
        profile: &DeviceProfile,
    ) -> Result<(), ConfigError> {
        if id == DEFAULT_LAYOUT {
            return Err(ConfigError::LayoutReserved(id.to_string()));
        }
        let mut next = document.clone();
        let existing = next
            .layouts
            .get(id)
            .ok_or_else(|| ConfigError::UnknownLayout(id.to_string()))?;
        let mut replacement = self.layout_for(profile);
        replacement.name = existing.name.clone();
        replacement.description = existing.description.clone();
        next.layouts.insert(id.to_string(), replacement);
        next.validate(profile)?;
        *document = next;
        Ok(())
    }
}

fn layout_to_toml(layout: &LayoutDocument, profile: &DeviceProfile) -> LayoutToml {
    let defaults = ActionMap::defaults_for(profile);
    let mut buttons = BTreeMap::new();
    for (name, slots) in &layout.buttons {
        let Some(button) = button_by_name(name) else {
            continue;
        };
        if button == RemoteButton::Voice {
            continue;
        }
        let table = ButtonToml {
            single: override_slot(slots.single.as_ref(), &defaults, button, Trigger::Single),
            double: override_slot(slots.double.as_ref(), &defaults, button, Trigger::Double),
            long: override_slot(slots.long.as_ref(), &defaults, button, Trigger::Long),
        };
        if !table.is_empty() {
            buttons.insert(name.clone(), table);
        }
    }
    let gesture = layout.gesture.as_ref().and_then(|overrides| {
        let effective = GestureSettings::from(profile.gesture_timing)
            .with_overrides(Some(overrides))
            .ok()?;
        let normalized = effective.overrides_from(profile.gesture_timing);
        (!normalized.is_empty()).then_some(normalized)
    });
    LayoutToml {
        name: layout.name.clone(),
        description: layout
            .description
            .clone()
            .filter(|description| !description.trim().is_empty()),
        gesture,
        buttons,
    }
}

fn override_slot(
    value: Option<&String>,
    defaults: &ActionMap,
    button: RemoteButton,
    trigger: Trigger,
) -> Option<String> {
    let value = value?;
    let default = defaults.resolve(button, trigger).map(ToString::to_string);
    (default.as_deref() != Some(value.as_str())).then(|| value.clone())
}

/// Validate and persist an editable document.
///
/// The document is normalized against profile defaults, so slots that match a
/// default are omitted from the file.
pub fn save_document(
    path: &Path,
    document: &ConfigDocument,
    profile: &DeviceProfile,
) -> Result<(), ConfigError> {
    document.validate(profile)?;
    write_file(path, &document.to_file(profile))
}

fn write_file(path: &Path, file: &FileConfig) -> Result<(), ConfigError> {
    let mut file = file.clone();
    file.schema_version = Some(SCHEMA_VERSION);
    let text = toml::to_string_pretty(&file).map_err(|source| ConfigError::Serialize {
        path: path.to_path_buf(),
        source,
    })?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| ConfigError::Write {
            path: path.to_path_buf(),
            source,
        })?;
    }
    let temp = path.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(&temp, text).map_err(|source| ConfigError::Write {
        path: temp.clone(),
        source,
    })?;
    std::fs::rename(&temp, path).map_err(|source| {
        let _ = std::fs::remove_file(&temp);
        ConfigError::Write {
            path: path.to_path_buf(),
            source,
        }
    })
}

/// One action that failed configuration validation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ActionCheckError {
    #[error(transparent)]
    Grammar(#[from] ActionError),
    #[error("exec actions are disabled; set allow_exec = true")]
    ExecDisabled,
    #[error("exec command is longer than {MAX_EXEC_COMMAND} bytes")]
    ExecTooLong,
    #[error("duplicate key {0:?} in chord")]
    DuplicateChordKey(String),
    #[error(
        "the recording key already starts and stops dictation; use key, combo, pointer, or exec"
    )]
    RecordingDictation,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("cannot read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot parse {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("cannot serialize {path}: {source}")]
    Serialize {
        path: PathBuf,
        #[source]
        source: toml::ser::Error,
    },
    #[error("cannot write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(
        "schema_version {found} is not supported (expected {LEGACY_SCHEMA_VERSION} or {SCHEMA_VERSION})"
    )]
    SchemaVersion { found: u32 },
    #[error("a v1 button map and v2 layouts cannot share one document")]
    MixedSchema,
    #[error("the top-level [buttons] and [gesture] tables moved into a layout")]
    LegacyTables,
    #[error("gain_db {0} is outside -{MAX_GAIN_DB}..={MAX_GAIN_DB}")]
    GainRange(i8),
    #[error("trigger_key {0:?} is not a known input key name (use `none` to disable)")]
    TriggerKey(String),
    #[error("recording.{edge} = {value:?}: {source}")]
    RecordingAction {
        edge: &'static str,
        value: String,
        #[source]
        source: ActionCheckError,
    },
    #[error("{field} {value} is outside {min}..={max}")]
    GestureRange {
        field: &'static str,
        value: u16,
        min: u16,
        max: u16,
    },
    #[error("bluetooth address {0:?} must be six colon-separated hex octets")]
    Address(String),
    #[error("unknown device {0:?}; run `atvvctl pair <MAC>` first")]
    UnknownDevice(String),
    #[error("unknown device profile {0:?}")]
    UnknownProfile(String),
    #[error("device {0:?}: the label must be 1-{MAX_DEVICE_LABEL} characters")]
    DeviceLabel(String),
    #[error("device address {0:?} appears more than once after normalization")]
    DuplicateDevice(String),
    #[error("buttons: unknown button {0:?}")]
    UnknownButton(String),
    #[error("buttons.voice: the voice key drives ATVV and cannot be action-mapped")]
    VoiceMapped,
    #[error("buttons.{button}.{trigger} = {value:?}: {source}")]
    Action {
        button: String,
        trigger: &'static str,
        value: String,
        #[source]
        source: ActionCheckError,
    },
    #[error("layout id {0:?} must match [a-z0-9][a-z0-9_-]{{0,31}}")]
    LayoutId(String),
    #[error("layout id {0:?} is reserved")]
    LayoutReserved(String),
    #[error("layout {0:?} already exists")]
    LayoutExists(String),
    #[error("layout {0:?}: the name must be 1-{MAX_LAYOUT_NAME} characters")]
    LayoutName(String),
    #[error("layout {0:?}: the name Default is reserved for the template")]
    LayoutNameReserved(String),
    #[error("layout {id:?}: the name {name:?} is already used")]
    LayoutNameTaken { id: String, name: String },
    #[error("layout {0:?}: the description is longer than {MAX_LAYOUT_DESCRIPTION} characters")]
    LayoutDescription(String),
    #[error("at most {MAX_LAYOUTS} layouts are allowed")]
    TooManyLayouts,
    #[error("active_layout {0:?} does not exist")]
    ActiveLayout(String),
    #[error("layout {0:?} does not exist")]
    UnknownLayout(String),
    #[error("layouts.{layout}.buttons.{button}.{trigger} = {value:?}: {source}")]
    LayoutAction {
        layout: String,
        button: String,
        trigger: &'static str,
        value: String,
        #[source]
        source: ActionCheckError,
    },
    #[error("layouts.{layout}.gesture: {source}")]
    LayoutGesture {
        layout: String,
        #[source]
        source: Box<ConfigError>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use atvv_protocol::profile::{CHROMECAST, RC003};

    fn path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "atvv-config-{name}-{}-{}.toml",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ))
    }

    fn defaults() -> ConfigDocument {
        ConfigDocument::from_config(&Config::defaults(&RC003))
    }

    fn layout_mut<'a>(document: &'a mut ConfigDocument, id: &str) -> &'a mut LayoutDocument {
        document.layouts.get_mut(id).expect("test layout")
    }

    fn custom_document() -> ConfigDocument {
        let mut document = defaults();
        let mut layout = LayoutDocument {
            name: "Coding".to_string(),
            description: Some("Editor shortcuts".to_string()),
            ..LayoutDocument::default()
        };
        layout.set_slot(
            RemoteButton::Up,
            Trigger::Single,
            Some("key:KEY_F13".to_string()),
        );
        layout.gesture = Some(GestureOverrides {
            long_press_ms: Some(700),
            ..GestureOverrides::default()
        });
        document.layouts.insert("editor".to_string(), layout);
        document.active_layout = "editor".to_string();
        document
    }

    #[test]
    fn missing_file_uses_profile_defaults() {
        let path = path("missing");
        let _ = std::fs::remove_file(&path);
        let loaded = load(&path, &RC003).unwrap();
        assert!(!loaded.existed);
        assert_eq!(loaded.config.gain_db, 0);
        assert!(loaded.config.input);
        assert_eq!(loaded.config.vinput, VinputMode::Trigger);
        assert_eq!(
            loaded.config.trigger_key.as_deref(),
            Some(DEFAULT_TRIGGER_KEY)
        );
        assert!(!loaded.config.allow_exec);
        assert_eq!(loaded.config.active_layout, DEFAULT_LAYOUT);
        assert!(loaded.config.layouts.is_empty());
        assert_eq!(loaded.config.gesture, RC003.gesture_timing);
        assert_eq!(
            loaded.config.map.resolve(RemoteButton::Ok, Trigger::Single),
            Some(&Action::Key {
                name: "KEY_ENTER".into()
            })
        );
    }

    #[test]
    fn v1_button_map_migrates_to_the_custom_layout() {
        let path = path("migrate");
        std::fs::write(
            &path,
            "schema_version = 1\ngain_db = 6\n\n[gesture]\nlong_press_ms = 700\n\n[buttons.up]\nsingle = \"key:KEY_F13\"\n",
        )
        .unwrap();
        let loaded = load(&path, &RC003).unwrap();
        assert_eq!(loaded.config.active_layout, MIGRATED_LAYOUT);
        assert_eq!(loaded.config.active_layout_name(), MIGRATED_LAYOUT_NAME);
        let layout = &loaded.config.layouts[MIGRATED_LAYOUT];
        assert_eq!(layout.name, MIGRATED_LAYOUT_NAME);
        assert_eq!(
            layout.slot(RemoteButton::Up, Trigger::Single),
            Some("key:KEY_F13")
        );
        assert_eq!(
            loaded.config.map.resolve(RemoteButton::Up, Trigger::Single),
            Some(&Action::Key {
                name: "KEY_F13".into()
            })
        );
        assert_eq!(loaded.config.gesture.long_press_ms, 700);

        let document = ConfigDocument::from_config(&loaded.config);
        save_document(&path, &document, &RC003).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("schema_version = 3"), "{text}");
        assert!(text.contains("active_layout = \"custom\""), "{text}");
        assert!(text.contains("[layouts.custom]"), "{text}");
        assert!(text.contains("[layouts.custom.buttons.up]"), "{text}");
        assert!(!text.contains("\n[buttons.up]"), "{text}");
        assert!(text.contains("gain_db = 6"), "{text}");

        let reloaded = load(&path, &RC003).unwrap().config;
        assert_eq!(reloaded, loaded.config);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn v1_without_overrides_migrates_to_the_default_template() {
        let path = path("empty-migrate");
        std::fs::write(&path, "schema_version = 1\ninput = false\n").unwrap();
        let loaded = load(&path, &RC003).unwrap();
        assert_eq!(loaded.config.active_layout, DEFAULT_LAYOUT);
        assert!(loaded.config.layouts.is_empty());
        assert!(!loaded.config.input);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn schema_versions_and_mixed_tables_are_rejected() {
        for text in [
            "schema_version = 4\n",
            "schema_version = 1\n[layouts.editor]\nname = \"Editor\"\n",
            "schema_version = 2\n[buttons.up]\nsingle = \"key:KEY_F13\"\n",
            "schema_version = 2\n[gesture]\nlong_press_ms = 700\n",
        ] {
            let file: FileConfig = toml::from_str(text).unwrap();
            assert!(file.validate(&RC003).is_err(), "{text}");
        }
        let mut document = defaults();
        document.schema_version = 1;
        assert!(matches!(
            document.validate(&RC003),
            Err(ConfigError::SchemaVersion { found: 1 })
        ));
    }

    #[test]
    fn layout_ids_names_and_limits_are_validated() {
        let invalid = |mutate: &dyn Fn(&mut ConfigDocument)| {
            let mut document = custom_document();
            mutate(&mut document);
            document.validate(&RC003).map(|_| ())
        };
        assert!(matches!(
            invalid(&|document| {
                document.layouts.insert(
                    "default".to_string(),
                    LayoutDocument {
                        name: "Mine".to_string(),
                        ..LayoutDocument::default()
                    },
                );
            }),
            Err(ConfigError::LayoutReserved(_))
        ));
        assert!(matches!(
            invalid(&|document| {
                document.layouts.insert(
                    "Coding".to_string(),
                    LayoutDocument {
                        name: "Other".to_string(),
                        ..LayoutDocument::default()
                    },
                );
            }),
            Err(ConfigError::LayoutId(_))
        ));
        assert!(matches!(
            invalid(&|document| layout_mut(document, "editor").name = String::new()),
            Err(ConfigError::LayoutName(_))
        ));
        assert!(matches!(
            invalid(&|document| layout_mut(document, "editor").name = "default".to_string()),
            Err(ConfigError::LayoutNameReserved(_))
        ));
        assert!(matches!(
            invalid(&|document| {
                document.layouts.insert(
                    "mpv".to_string(),
                    LayoutDocument {
                        name: "MPV".to_string(),
                        ..LayoutDocument::default()
                    },
                );
            }),
            Err(ConfigError::LayoutReserved(_))
        ));
        assert!(matches!(
            invalid(&|document| {
                document.layouts.insert(
                    "other".to_string(),
                    LayoutDocument {
                        name: "CODING".to_string(),
                        ..LayoutDocument::default()
                    },
                );
            }),
            Err(ConfigError::LayoutNameTaken { .. })
        ));
        assert!(matches!(
            invalid(&|document| layout_mut(document, "editor").name = "x".repeat(41)),
            Err(ConfigError::LayoutName(_))
        ));
        assert!(matches!(
            invalid(&|document| {
                layout_mut(document, "editor").description = Some("x".repeat(121));
            }),
            Err(ConfigError::LayoutDescription(_))
        ));
        assert!(matches!(
            invalid(&|document| document.active_layout = "missing".to_string()),
            Err(ConfigError::ActiveLayout(id)) if id == "missing"
        ));
        assert!(matches!(
            invalid(&|document| {
                for index in 0..MAX_LAYOUTS {
                    document.layouts.insert(
                        format!("layout-{index}"),
                        LayoutDocument {
                            name: format!("Layout {index}"),
                            ..LayoutDocument::default()
                        },
                    );
                }
            }),
            Err(ConfigError::TooManyLayouts)
        ));
    }

    #[test]
    fn malformed_actions_are_rejected_in_every_layout() {
        let mut document = custom_document();
        document.layouts.insert(
            "broken".to_string(),
            LayoutDocument {
                name: "Broken".to_string(),
                ..LayoutDocument::default()
            },
        );
        layout_mut(&mut document, "broken").set_slot(
            RemoteButton::Ok,
            Trigger::Long,
            Some("key:not-a-key".to_string()),
        );
        let error = document.validate(&RC003).unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains("layouts.broken.buttons.ok.long"),
            "{message}"
        );
        assert!(message.contains("not-a-key"), "{message}");
    }

    #[test]
    fn layout_gesture_overrides_are_validated() {
        let mut document = custom_document();
        layout_mut(&mut document, "editor").gesture = Some(GestureOverrides {
            long_press_ms: Some(10),
            ..GestureOverrides::default()
        });
        assert!(matches!(
            document.validate(&RC003),
            Err(ConfigError::LayoutGesture { .. })
        ));
    }

    #[test]
    fn pointer_and_exec_policy_still_apply_per_layout() {
        let mut document = custom_document();
        layout_mut(&mut document, "editor").set_slot(
            RemoteButton::Ok,
            Trigger::Single,
            Some("exec:true".to_string()),
        );
        assert!(matches!(
            document.validate(&RC003),
            Err(ConfigError::LayoutAction {
                source: ActionCheckError::ExecDisabled,
                ..
            })
        ));
        document.allow_exec = true;
        assert!(document.validate(&RC003).is_ok());
    }

    #[test]
    fn recording_edges_default_to_none_and_round_trip() {
        let path = path("recording");
        let mut document = defaults();
        assert!(document.recording.is_empty());
        save_document(&path, &document, &RC003).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("[recording]"), "{text}");
        let config = document.validate(&RC003).unwrap();
        assert!(config.recording.is_empty());
        let _ = std::fs::remove_file(&path);

        document.allow_exec = true;
        document.recording.press = Some("combo:ctrl+alt+v".to_string());
        document.recording.release = Some("exec:printf done".to_string());
        let json = serde_json::to_string(&document).unwrap();
        assert_eq!(
            serde_json::from_str::<ConfigDocument>(&json).unwrap(),
            document
        );
        let config = document.validate(&RC003).unwrap();
        assert_eq!(
            config.recording.press,
            Some(Action::Combo {
                mods: vec!["ctrl".into(), "alt".into()],
                key: "v".into()
            })
        );
        assert_eq!(
            config.recording.release,
            Some(Action::Exec {
                command: "printf done".into()
            })
        );

        save_document(&path, &document, &RC003).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("[recording]"), "{text}");
        assert!(text.contains("press = \"combo:ctrl+alt+v\""), "{text}");
        assert!(text.contains("release = \"exec:printf done\""), "{text}");
        let loaded = load(&path, &RC003).unwrap().config;
        assert_eq!(loaded, config);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn recording_edges_reject_dictation_and_unpermitted_exec() {
        let mut document = defaults();
        document.recording.press = Some("dictation:start".to_string());
        assert!(matches!(
            document.validate(&RC003),
            Err(ConfigError::RecordingAction {
                edge: "press",
                source: ActionCheckError::RecordingDictation,
                ..
            })
        ));

        document.recording.press = Some("key:KEY_F13".to_string());
        document.recording.release = Some("exec:true".to_string());
        assert!(matches!(
            document.validate(&RC003),
            Err(ConfigError::RecordingAction {
                edge: "release",
                source: ActionCheckError::ExecDisabled,
                ..
            })
        ));

        document.allow_exec = true;
        assert!(document.validate(&RC003).is_ok());

        document.recording.press = Some("key:not-a-key".to_string());
        assert!(matches!(
            document.validate(&RC003),
            Err(ConfigError::RecordingAction {
                edge: "press",
                source: ActionCheckError::Grammar(_),
                ..
            })
        ));
    }

    #[test]
    fn document_round_trips_through_json_and_file() {
        let mut document = custom_document();
        document.preopen_mic = true;
        let json = serde_json::to_string(&document).unwrap();
        assert_eq!(
            serde_json::from_str::<ConfigDocument>(&json).unwrap(),
            document
        );
        let config = document.validate(&RC003).unwrap();
        assert_eq!(config.active_layout, "editor");
        assert_eq!(config.active_layout_name(), "Coding");
        assert_eq!(
            config.map.resolve(RemoteButton::Up, Trigger::Single),
            Some(&Action::Key {
                name: "KEY_F13".into()
            })
        );
        assert_eq!(
            config.map.resolve(RemoteButton::Ok, Trigger::Single),
            Some(&Action::Key {
                name: "KEY_ENTER".into()
            })
        );
        assert_eq!(config.gesture.long_press_ms, 700);
        assert_eq!(
            config.gesture.double_click_ms,
            RC003.gesture_timing.double_click_ms
        );

        let path = path("document");
        save_document(&path, &document, &RC003).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("schema_version = 3"), "{text}");
        assert!(text.contains("active_layout = \"editor\""), "{text}");
        assert!(text.contains("[layouts.editor]"), "{text}");
        assert!(text.contains("name = \"Coding\""), "{text}");
        assert!(text.contains("[layouts.editor.buttons.up]"), "{text}");
        assert!(text.contains("single = \"key:KEY_F13\""), "{text}");
        assert!(text.contains("[layouts.editor.gesture]"), "{text}");
        assert!(text.contains("preopen_mic = true"), "{text}");
        assert!(!text.contains("[buttons]"), "{text}");
        let loaded = load(&path, &RC003).unwrap().config;
        assert_eq!(loaded, config);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn default_template_is_synthesized_and_not_stored() {
        let mut document = defaults();
        document.layouts.insert(
            "editor".to_string(),
            LayoutDocument {
                name: "Editor".to_string(),
                ..LayoutDocument::default()
            },
        );
        let summaries = document.layout_summaries();
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].id, DEFAULT_LAYOUT);
        assert!(summaries[0].template);
        assert_eq!(summaries[1].id, "editor");

        let path = path("template");
        save_document(&path, &document, &RC003).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("layouts.default"), "{text}");
        assert!(text.contains("active_layout = \"default\""), "{text}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn document_null_slot_means_profile_default() {
        let mut document = defaults();
        document
            .layouts
            .entry("editor".to_string())
            .or_insert_with(|| LayoutDocument {
                name: "Editor".to_string(),
                ..LayoutDocument::default()
            });
        document.active_layout = "editor".to_string();
        layout_mut(&mut document, "editor").set_slot(
            RemoteButton::Ok,
            Trigger::Long,
            Some("none".to_string()),
        );
        let config = document.validate(&RC003).unwrap();
        assert_eq!(
            config.map.resolve(RemoteButton::Ok, Trigger::Single),
            Some(&Action::Key {
                name: "KEY_ENTER".into()
            })
        );
        assert_eq!(
            config.map.resolve(RemoteButton::Ok, Trigger::Long),
            Some(&Action::None)
        );
        layout_mut(&mut document, "editor").set_slot(RemoteButton::Ok, Trigger::Long, None);
        assert!(
            !layout_mut(&mut document, "editor")
                .buttons
                .contains_key("ok"),
            "resetting a slot drops the empty entry"
        );
    }

    #[test]
    fn resolved_layout_reads_any_layout_without_switching() {
        let document = custom_document();
        let (map, gesture) = document.resolved_layout("editor", &RC003).unwrap();
        assert_eq!(
            map.resolve(RemoteButton::Up, Trigger::Single),
            Some(&Action::Key {
                name: "KEY_F13".into()
            })
        );
        assert_eq!(gesture.long_press_ms, 700);
        let (map, gesture) = document.resolved_layout(DEFAULT_LAYOUT, &RC003).unwrap();
        assert_eq!(
            map.resolve(RemoteButton::Up, Trigger::Single),
            Some(&Action::Key {
                name: "KEY_UP".into()
            })
        );
        assert_eq!(gesture, RC003.gesture_timing);
        assert!(matches!(
            document.resolved_layout("missing", &RC003),
            Err(ConfigError::ActiveLayout(id)) if id == "missing"
        ));
    }

    #[test]
    fn duplicate_chord_keys_are_rejected_in_layouts() {
        let mut document = custom_document();
        layout_mut(&mut document, "editor").set_slot(
            RemoteButton::Ok,
            Trigger::Double,
            Some("combo:ctrl+ctrl+a".to_string()),
        );
        let error = document.validate(&RC003).unwrap_err();
        assert!(
            matches!(
                error,
                ConfigError::LayoutAction {
                    source: ActionCheckError::DuplicateChordKey(_),
                    ..
                }
            ),
            "{error}"
        );
    }

    #[test]
    fn unknown_button_and_voice_key_are_rejected_in_layouts() {
        let mut document = custom_document();
        layout_mut(&mut document, "editor").buttons.insert(
            "sideways".to_string(),
            ButtonDocument {
                single: Some("key:KEY_A".to_string()),
                ..ButtonDocument::default()
            },
        );
        assert!(matches!(
            document.validate(&RC003),
            Err(ConfigError::UnknownButton(name)) if name == "sideways"
        ));
        document
            .layouts
            .get_mut("editor")
            .unwrap()
            .buttons
            .remove("sideways");
        layout_mut(&mut document, "editor").buttons.insert(
            "voice".to_string(),
            ButtonDocument {
                single: Some("key:KEY_F5".to_string()),
                ..ButtonDocument::default()
            },
        );
        assert!(matches!(
            document.validate(&RC003),
            Err(ConfigError::VoiceMapped)
        ));
    }

    #[test]
    fn v2_device_migrates_to_the_registry() {
        let path = path("v2-device");
        std::fs::write(
            &path,
            "schema_version = 2\nprofile = \"miremote-rc003\"\ndevice = \"c0:5d:39:f8:2d:4c\"\nactive_layout = \"editor\"\n\n[layouts.editor]\nname = \"Editor\"\n",
        )
        .unwrap();
        let loaded = load(&path, &RC003).unwrap();
        assert_eq!(
            loaded.config.active_device.as_deref(),
            Some("C0:5D:39:F8:2D:4C")
        );
        assert_eq!(loaded.config.active_layout, "editor");
        let device = &loaded.config.devices["C0:5D:39:F8:2D:4C"];
        assert_eq!(device.profile, "miremote-rc003");
        assert_eq!(device.layout, "editor");

        save_document(&path, &ConfigDocument::from_config(&loaded.config), &RC003).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("schema_version = 3"), "{text}");
        assert!(text.contains("[devices.\"C0:5D:39:F8:2D:4C\"]"), "{text}");
        assert!(text.contains("layout = \"editor\""), "{text}");
        let parsed: FileConfig = toml::from_str(&text).unwrap();
        assert!(parsed.profile.is_none());
        assert!(parsed.device.is_none());
        assert!(parsed.active_layout.is_none());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn select_device_loads_the_device_layout_and_profile() {
        let first = "C0:5D:39:F8:2D:4C";
        let second = "74:46:B3:22:2D:D5";
        let mut layout = LayoutDocument {
            name: "Second".to_string(),
            ..LayoutDocument::default()
        };
        layout.set_slot(
            RemoteButton::Up,
            Trigger::Single,
            Some("key:KEY_F13".to_string()),
        );
        let file = FileConfig {
            schema_version: Some(SCHEMA_VERSION),
            active_device: Some(first.to_string()),
            devices: BTreeMap::from([
                (
                    first.to_string(),
                    DeviceToml {
                        label: None,
                        profile: RC003.id.to_string(),
                        layout: Some(DEFAULT_LAYOUT.to_string()),
                    },
                ),
                (
                    second.to_string(),
                    DeviceToml {
                        label: Some("Chromecast".to_string()),
                        profile: CHROMECAST.id.to_string(),
                        layout: Some("second".to_string()),
                    },
                ),
            ]),
            layouts: BTreeMap::from([("second".to_string(), layout.to_toml())]),
            ..FileConfig::default()
        };
        let mut config = file.validate(&RC003).unwrap();
        assert_eq!(config.profile, RC003.id);
        let profile = config.select_device(second).unwrap();
        assert_eq!(profile.id, CHROMECAST.id);
        assert_eq!(config.profile, CHROMECAST.id);
        assert_eq!(config.active_device.as_deref(), Some(second));
        assert_eq!(config.active_layout, "second");
        assert_eq!(
            config.map.resolve(RemoteButton::Up, Trigger::Single),
            Some(&Action::Key {
                name: "KEY_F13".into()
            })
        );
        assert_eq!(config.devices[first].layout, DEFAULT_LAYOUT);
    }

    #[test]
    fn save_device_round_trips_and_preserves_layouts() {
        let path = path("save");
        std::fs::write(
            &path,
            "schema_version = 1\ngain_db = 6\ninput = false\n[buttons.up]\nsingle = \"key:KEY_F13\"\n",
        )
        .unwrap();
        save_device(&path, "C0:5D:39:F8:2D:4C", &RC003, &RC003, Some("hci0")).unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("schema_version = 3"), "{text}");
        assert!(text.contains("[layouts.custom.buttons.up]"), "{text}");
        let loaded = load(&path, &RC003).unwrap();
        assert_eq!(
            loaded.config.active_device.as_deref(),
            Some("C0:5D:39:F8:2D:4C")
        );
        assert_eq!(loaded.config.adapter.as_deref(), Some("hci0"));
        assert_eq!(loaded.config.gain_db, 6);
        assert!(!loaded.config.input);
        assert_eq!(loaded.config.active_layout, MIGRATED_LAYOUT);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_device_keeps_the_legacy_profile_when_pairing_another_device() {
        let path = path("second-device");
        std::fs::write(
            &path,
            "schema_version = 2\n\
             device = \"C0:5D:39:F8:2D:4C\"\n\
             active_layout = \"custom\"\n\n\
             [layouts.custom]\n\
             name = \"Custom\"\n\n\
             [layouts.custom.buttons.up]\n\
             single = \"key:KEY_F13\"\n",
        )
        .unwrap();
        save_device(&path, "74:46:B3:22:2D:D5", &CHROMECAST, &RC003, None).unwrap();

        let loaded = load(&path, &CHROMECAST).unwrap();
        assert_eq!(
            loaded.config.active_device.as_deref(),
            Some("74:46:B3:22:2D:D5")
        );
        assert_eq!(loaded.config.devices["C0:5D:39:F8:2D:4C"].profile, RC003.id);
        assert_eq!(
            loaded.config.devices["74:46:B3:22:2D:D5"].profile,
            CHROMECAST.id
        );
        assert_eq!(loaded.config.devices["C0:5D:39:F8:2D:4C"].layout, "custom");
        assert_eq!(loaded.config.active_profile().unwrap().id, CHROMECAST.id);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_device_refuses_a_malformed_or_future_file() {
        let path = path("malformed");
        std::fs::write(&path, "gaiN_db = 3\n").unwrap();
        assert!(matches!(
            save_device(&path, "C0:5D:39:F8:2D:4C", &RC003, &RC003, None),
            Err(ConfigError::Parse { .. })
        ));

        std::fs::write(&path, "schema_version = 4\n").unwrap();
        assert!(matches!(
            save_device(&path, "C0:5D:39:F8:2D:4C", &RC003, &RC003, None),
            Err(ConfigError::SchemaVersion { found: 4 })
        ));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn unknown_device_profile_is_rejected() {
        let file = FileConfig {
            schema_version: Some(SCHEMA_VERSION),
            active_device: Some("C0:5D:39:F8:2D:4C".to_string()),
            devices: BTreeMap::from([(
                "C0:5D:39:F8:2D:4C".to_string(),
                DeviceToml {
                    label: None,
                    profile: "other-remote".to_string(),
                    layout: None,
                },
            )]),
            ..FileConfig::default()
        };
        assert!(matches!(
            file.validate(&RC003),
            Err(ConfigError::UnknownProfile(profile)) if profile == "other-remote"
        ));
    }

    #[test]
    fn legacy_file_without_schema_or_buttons_loads() {
        let path = path("legacy");
        std::fs::write(
            &path,
            "vinput = \"dbus\"\ntrigger_key = \"F18\"\ninput = false\n",
        )
        .unwrap();
        let loaded = load(&path, &RC003).unwrap();
        assert_eq!(loaded.config.vinput, VinputMode::Dbus);
        assert_eq!(loaded.config.trigger_key.as_deref(), Some("F18"));
        assert!(!loaded.config.input);
        assert!(!loaded.config.preopen_mic);
        assert_eq!(loaded.config.active_layout, DEFAULT_LAYOUT);
        assert_eq!(loaded.config.gesture, RC003.gesture_timing);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn layout_naming_helpers_generate_valid_unique_ids() {
        assert!(layout_id_is_valid("editor"));
        assert!(layout_id_is_valid("editor-2"));
        assert!(layout_id_is_valid("2fast"));
        assert!(!layout_id_is_valid("Editor"));
        assert!(!layout_id_is_valid("-editor"));
        assert!(!layout_id_is_valid("x".repeat(33).as_str()));
        assert!(layout_id_is_reserved("default"));
        assert!(layout_id_is_reserved("mpv"));
        assert!(layout_id_is_reserved("slides"));
        assert!(layout_id_is_reserved("omp"));
        assert!(!layout_id_is_reserved("media"));
        assert!(!layout_id_is_reserved("coding"));
        assert_eq!(slug_layout_id("My Layout!"), "my-layout");
        assert_eq!(slug_layout_id("默认"), "layout");
        assert_eq!(
            unique_layout_id("editor", ["editor", "editor-2"]),
            "editor-3"
        );
        assert_eq!(
            unique_layout_id("default", Vec::<&str>::new().iter().copied()),
            "default-layout"
        );
        assert_eq!(
            unique_layout_name("Coding", ["coding", "Coding 2"]),
            "Coding 3"
        );
        assert_eq!(
            unique_layout_name("Default", Vec::<&str>::new().iter().copied()),
            "Default 2"
        );
    }

    #[test]
    fn preset_catalog_covers_every_action_button_without_none() {
        for preset in &LAYOUT_PRESETS {
            assert!(layout_id_is_valid(preset.id), "{}", preset.id);
            assert!(layout_id_is_reserved(preset.id), "{}", preset.id);
            assert!(!preset.name.trim().is_empty());
            assert!(preset.name.chars().count() <= MAX_LAYOUT_NAME);
            assert!(!preset.description.trim().is_empty());
            assert!(preset.description.chars().count() <= MAX_LAYOUT_DESCRIPTION);
            assert!(!preset.verified_with.trim().is_empty());
            let mut seen = BTreeSet::new();
            for (button, raw) in preset.slots {
                let button = button_by_name(button).unwrap_or_else(|| panic!("unknown {button}"));
                assert_ne!(button, RemoteButton::Voice);
                assert!(seen.insert(button), "{}: {button} appears twice", preset.id);
                let action = parse_checked_action(raw, false)
                    .unwrap_or_else(|error| panic!("{} {button}: {error}", preset.id));
                assert_ne!(action, Action::None, "{} {button} maps none", preset.id);
                assert!(
                    matches!(action, Action::Key { .. } | Action::Combo { .. }),
                    "{} {button} must be a key action",
                    preset.id
                );
            }
            assert_eq!(seen.len(), RC003.buttons.len());
            for &button in RC003.buttons {
                assert!(seen.contains(&button), "{} misses {button}", preset.id);
            }

            let google = preset.layout_for(&CHROMECAST);
            assert!(!google.buttons.contains_key("menu"));
            assert!(google.buttons.contains_key("tv"));
            assert_eq!(google.buttons.len(), CHROMECAST.buttons.len() - 3);
            for name in google.buttons.keys() {
                let button = button_by_name(name).expect("known button");
                assert!(CHROMECAST.has_button(button), "{name} is not on the device");
            }
        }
    }

    #[test]
    fn preset_catalog_matches_the_reviewed_snapshot() {
        let expected = [
            "[mpv] mpv | Media playback with mpv's default key bindings. | mpv 0.41.0",
            "  power = key:KEY_Q",
            "  ok = key:KEY_SPACE",
            "  back = key:KEY_ESC",
            "  up = key:KEY_UP",
            "  down = key:KEY_DOWN",
            "  left = key:KEY_LEFT",
            "  right = key:KEY_RIGHT",
            "  home = key:KEY_HOME",
            "  menu = key:KEY_M",
            "  tv = key:KEY_F",
            "  volume_up = key:KEY_VOLUMEUP",
            "  volume_down = key:KEY_VOLUMEDOWN",
            "[slides] Slides | Presentation control for LibreOffice Impress and WPS Presentation. | LibreOffice Impress 26.8.1, WPS Office 12.1.2",
            "  power = key:KEY_END",
            "  ok = key:KEY_SPACE",
            "  back = key:KEY_ESC",
            "  up = key:KEY_PAGEUP",
            "  down = key:KEY_PAGEDOWN",
            "  left = key:KEY_LEFT",
            "  right = key:KEY_RIGHT",
            "  home = key:KEY_HOME",
            "  menu = key:KEY_B",
            "  tv = key:KEY_F5",
            "  volume_up = key:KEY_VOLUMEUP",
            "  volume_down = key:KEY_VOLUMEDOWN",
            "[omp] omp | Coding-agent session control with omp's default key bindings. | omp 18.14.2",
            "  power = combo:alt+r",
            "  ok = key:KEY_ENTER",
            "  back = key:KEY_ESC",
            "  up = key:KEY_UP",
            "  down = key:KEY_DOWN",
            "  left = key:KEY_LEFT",
            "  right = key:KEY_RIGHT",
            "  home = combo:ctrl+r",
            "  menu = combo:shift+tab",
            "  tv = combo:ctrl+o",
            "  volume_up = key:KEY_VOLUMEUP",
            "  volume_down = key:KEY_VOLUMEDOWN",
        ]
        .join("\n")
            + "\n";
        let mut actual = String::new();
        for preset in &LAYOUT_PRESETS {
            actual.push_str(&format!(
                "[{}] {} | {} | {}\n",
                preset.id, preset.name, preset.description, preset.verified_with
            ));
            for (button, action) in preset.slots {
                actual.push_str(&format!("  {button} = {action}\n"));
            }
        }
        assert_eq!(actual, expected);
    }

    #[test]
    fn applying_a_preset_creates_an_inactive_valid_layout() {
        let mut document = defaults();
        MPV_PRESET
            .apply(&mut document, "movie", Some("Movie"), &RC003)
            .unwrap();
        assert_eq!(document.active_layout, DEFAULT_LAYOUT);
        let layout = &document.layouts["movie"];
        assert_eq!(layout.name, "Movie");
        assert_eq!(layout.description.as_deref(), Some(MPV_PRESET.description));
        for &button in RC003.buttons {
            assert!(layout.slot(button, Trigger::Single).is_some(), "{button}");
        }
        assert_eq!(
            layout.slot(RemoteButton::Power, Trigger::Single),
            Some("key:KEY_Q")
        );
        assert_eq!(
            layout.slot(RemoteButton::Ok, Trigger::Single),
            Some("key:KEY_SPACE")
        );
        let config = document.validate(&RC003).unwrap();
        assert_eq!(config.active_layout, DEFAULT_LAYOUT);
        assert_eq!(
            config.layouts["movie"].slot(RemoteButton::Tv, Trigger::Single),
            Some("key:KEY_F")
        );
    }

    #[test]
    fn preset_apply_rejects_reserved_and_existing_ids() {
        let mut document = defaults();
        assert!(matches!(
            MPV_PRESET.apply(&mut document, "mpv", None, &RC003),
            Err(ConfigError::LayoutReserved(_))
        ));
        assert!(matches!(
            MPV_PRESET.apply(&mut document, "Default", None, &RC003),
            Err(ConfigError::LayoutId(_))
        ));
        OMP_PRESET
            .apply(&mut document, "agent", None, &RC003)
            .unwrap();
        assert!(matches!(
            OMP_PRESET.apply(&mut document, "agent", None, &RC003),
            Err(ConfigError::LayoutExists(_))
        ));
        assert_eq!(document.layouts["agent"].name, "omp");
    }

    #[test]
    fn chromecast_app_keys_round_trip_through_the_config() {
        let mut document = ConfigDocument::from_config(&Config::defaults(&CHROMECAST));
        let mut layout = LayoutDocument {
            name: "Media".to_string(),
            ..LayoutDocument::default()
        };
        layout.set_slot(
            RemoteButton::Mute,
            Trigger::Single,
            Some("key:KEY_F13".to_string()),
        );
        layout.set_slot(
            RemoteButton::YouTube,
            Trigger::Single,
            Some("combo:ctrl+KEY_Y".to_string()),
        );
        layout.set_slot(
            RemoteButton::Netflix,
            Trigger::Single,
            Some("key:KEY_F15".to_string()),
        );
        document.layouts.insert("media".to_string(), layout);
        document.active_layout = "media".to_string();
        let config = document.validate(&CHROMECAST).unwrap();
        assert_eq!(
            config.map.resolve(RemoteButton::Mute, Trigger::Single),
            Some(&Action::Key {
                name: "KEY_F13".to_string()
            })
        );
        assert_eq!(
            config.map.resolve(RemoteButton::YouTube, Trigger::Single),
            Some(&Action::Combo {
                mods: vec!["ctrl".to_string()],
                key: "KEY_Y".to_string()
            })
        );

        let path = path("chromecast-app-keys");
        save_document(&path, &document, &CHROMECAST).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("[layouts.media.buttons.mute]"), "{text}");
        assert!(text.contains("[layouts.media.buttons.youtube]"), "{text}");
        assert!(text.contains("[layouts.media.buttons.netflix]"), "{text}");
        let loaded = load(&path, &CHROMECAST).unwrap().config;
        assert_eq!(
            loaded.map.resolve(RemoteButton::Netflix, Trigger::Single),
            Some(&Action::Key {
                name: "KEY_F15".to_string()
            })
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn preset_reset_replaces_slots_and_keeps_identity() {
        let mut document = defaults();
        let mut layout = LayoutDocument {
            name: "Mine".to_string(),
            description: Some("keep me".to_string()),
            ..LayoutDocument::default()
        };
        layout.set_slot(
            RemoteButton::Ok,
            Trigger::Single,
            Some("key:KEY_F13".to_string()),
        );
        document.layouts.insert("mine".to_string(), layout);
        document.active_layout = "mine".to_string();
        SLIDES_PRESET
            .reset_to(&mut document, "mine", &RC003)
            .unwrap();
        let layout = &document.layouts["mine"];
        assert_eq!(layout.name, "Mine");
        assert_eq!(layout.description.as_deref(), Some("keep me"));
        assert_eq!(
            layout.slot(RemoteButton::Up, Trigger::Single),
            Some("key:KEY_PAGEUP")
        );
        assert_eq!(
            layout.slot(RemoteButton::Ok, Trigger::Single),
            Some("key:KEY_SPACE")
        );
        assert_eq!(document.active_layout, "mine");
        document.validate(&RC003).unwrap();
    }

    #[test]
    fn preset_reset_rejects_template_and_unknown_layouts() {
        let mut document = defaults();
        assert!(matches!(
            MPV_PRESET.reset_to(&mut document, DEFAULT_LAYOUT, &RC003),
            Err(ConfigError::LayoutReserved(_))
        ));
        assert!(matches!(
            MPV_PRESET.reset_to(&mut document, "missing", &RC003),
            Err(ConfigError::UnknownLayout(_))
        ));
    }
}
