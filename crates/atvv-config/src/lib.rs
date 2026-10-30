// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared `daemon.toml` schema, validation, and editable configuration.
//!
//! This crate is the only parser for the configuration file. The daemon, the
//! CLI, and the GUI use [`FileConfig`] for TOML and [`ConfigDocument`] for the
//! JSON document exchanged over D-Bus.
//!
//! Action strings use the existing grammar from [`atvv_input`]. The file stores
//! per-button overrides; the profile supplies the defaults. `None` in a slot
//! means "use the profile default", not "unmapped": use the action `none` to
//! swallow a button without an action.
//!
//! `exec:` is disabled unless `allow_exec = true`. The daemon, never the input
//! runtime or a client, executes those actions.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use atvv_input::{
    Action, ActionError, ActionMap, KeyResolver, Trigger, parse_action, parse_key_program,
    resolve_key,
};
use atvv_protocol::profile::{DeviceProfile, GestureTiming, RemoteButton};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Current schema version written by this crate.
pub const SCHEMA_VERSION: u32 = 1;
/// Gain clamp from the research notes: ±24 dB, no AGC.
pub const MAX_GAIN_DB: i8 = 24;
/// fcitx5-vinput trigger key used while the voice key is held.
pub const DEFAULT_TRIGGER_KEY: &str = "DICTATE";
/// Upper bound for an `exec:` command line. The command stays opaque.
pub const MAX_EXEC_COMMAND: usize = 4096;

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
    fn is_empty(&self) -> bool {
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

    fn validate_effective(self) -> Result<Self, ConfigError> {
        self.with_overrides(Some(&GestureOverrides {
            double_click_ms: Some(self.double_click_ms),
            long_press_ms: Some(self.long_press_ms),
            repeat_start_ms: Some(self.repeat_start_ms),
            stable_release_ms: Some(self.stable_release_ms),
            suppression_ms: Some(self.suppression_ms),
            repeat_interval_ms: Some(self.repeat_interval_ms),
            back_repeat_interval_ms: Some(self.back_repeat_interval_ms),
        }))
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

/// Raw `daemon.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileConfig {
    pub schema_version: Option<u32>,
    /// Device profile id; must match the profile this build loaded.
    pub profile: Option<String>,
    /// Bluetooth address written by `atvvctl pair`.
    pub device: Option<String>,
    /// BlueZ adapter name such as `hci0`; the default adapter when absent.
    pub adapter: Option<String>,
    /// Override the profile gain in dB, clamped to ±24 dB.
    pub gain_db: Option<i8>,
    /// Start the evdev/uinput input path.
    pub input: Option<bool>,
    /// fcitx5-vinput integration mode.
    pub vinput: Option<VinputMode>,
    /// Synthetic key emitted while the voice key is held; `none` disables it.
    pub trigger_key: Option<String>,
    /// Allow `exec:` actions. Disabled by default.
    pub allow_exec: Option<bool>,
    /// Gesture timing overrides; absent fields keep the profile value.
    pub gesture: Option<GestureOverrides>,
    /// Per-button action overrides.
    #[serde(default)]
    pub buttons: BTreeMap<String, ButtonToml>,
}

/// Validated configuration with profile defaults applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub profile: String,
    pub device: Option<String>,
    pub adapter: Option<String>,
    pub gain_db: i8,
    pub input: bool,
    pub vinput: VinputMode,
    /// Key name; `None` disables synthetic trigger emission.
    pub trigger_key: Option<String>,
    pub allow_exec: bool,
    pub gesture: GestureTiming,
    pub map: ActionMap,
}

impl Config {
    /// Defaults for a profile with no configuration file.
    #[must_use]
    pub fn defaults(profile: &DeviceProfile) -> Self {
        Self {
            profile: profile.id.to_string(),
            device: None,
            adapter: None,
            gain_db: profile.gain_db,
            input: true,
            vinput: VinputMode::Trigger,
            trigger_key: Some(DEFAULT_TRIGGER_KEY.to_string()),
            allow_exec: false,
            gesture: profile.gesture_timing,
            map: ActionMap::defaults(),
        }
    }
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

fn check_schema(version: u32) -> Result<(), ConfigError> {
    if version == SCHEMA_VERSION {
        Ok(())
    } else {
        Err(ConfigError::SchemaVersion { found: version })
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
        Action::None | Action::Dictation(_) => {}
    }
    Ok(action)
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

impl FileConfig {
    /// Validate every field and merge the overrides into the profile defaults.
    pub fn validate(&self, profile: &DeviceProfile) -> Result<Config, ConfigError> {
        check_schema(self.schema_version.unwrap_or(SCHEMA_VERSION))?;
        if let Some(profile_id) = &self.profile
            && profile_id != profile.id
        {
            return Err(ConfigError::Profile {
                found: profile_id.clone(),
                expected: profile.id.to_string(),
            });
        }
        let gain_db = self.gain_db.unwrap_or(profile.gain_db);
        if !(-MAX_GAIN_DB..=MAX_GAIN_DB).contains(&gain_db) {
            return Err(ConfigError::GainRange(gain_db));
        }
        let trigger_key = match self.trigger_key.as_deref() {
            Some(name) => parse_trigger_key(name)?,
            None => Some(DEFAULT_TRIGGER_KEY.to_string()),
        };
        let allow_exec = self.allow_exec.unwrap_or(false);
        let gesture =
            GestureSettings::from(profile.gesture_timing).with_overrides(self.gesture.as_ref())?;

        let mut map = ActionMap::defaults();
        for (name, slots) in &self.buttons {
            let button =
                button_by_name(name).ok_or_else(|| ConfigError::UnknownButton(name.clone()))?;
            if button == RemoteButton::Voice {
                return Err(ConfigError::VoiceMapped);
            }
            for (trigger, value) in slots.slots() {
                let Some(raw) = value else {
                    continue;
                };
                let action = parse_checked_action(raw, allow_exec).map_err(|source| {
                    ConfigError::Action {
                        button: name.clone(),
                        trigger: trigger.name(),
                        value: raw.clone(),
                        source,
                    }
                })?;
                map.set(button, trigger, action)
                    .map_err(|error| ConfigError::Action {
                        button: name.clone(),
                        trigger: trigger.name(),
                        value: raw.clone(),
                        source: ActionCheckError::Grammar(error),
                    })?;
            }
        }

        Ok(Config {
            profile: profile.id.to_string(),
            device: self.device.clone(),
            adapter: self.adapter.clone(),
            gain_db,
            input: self.input.unwrap_or(true),
            vinput: self.vinput.unwrap_or_default(),
            trigger_key,
            allow_exec,
            gesture: gesture.into(),
            map,
        })
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

/// Record the paired device and adapter without discarding other settings.
///
/// A malformed existing file is an error: the daemon should not silently
/// replace a typo with defaults.
pub fn save_device(path: &Path, device: &str, adapter: Option<&str>) -> Result<(), ConfigError> {
    let mut file = if path.exists() {
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let file: FileConfig = toml::from_str(&text).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
        check_schema(file.schema_version.unwrap_or(SCHEMA_VERSION))?;
        file
    } else {
        FileConfig::default()
    };
    file.schema_version = Some(SCHEMA_VERSION);
    file.device = Some(device.to_string());
    if let Some(adapter) = adapter {
        file.adapter = Some(adapter.to_string());
    }
    write_file(path, &file)
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

/// Stable JSON view of the editable configuration.
///
/// `schema_version` versions this payload. A missing slot means "profile
/// default"; the action `none` means "swallow the button".
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigDocument {
    pub schema_version: u32,
    pub profile: String,
    pub device: Option<String>,
    pub adapter: Option<String>,
    pub gain_db: i8,
    pub input: bool,
    pub vinput: VinputMode,
    pub trigger_key: Option<String>,
    pub allow_exec: bool,
    pub gesture: GestureSettings,
    #[serde(default)]
    pub buttons: BTreeMap<String, ButtonDocument>,
}

impl ConfigDocument {
    /// Build the effective document from a validated configuration.
    #[must_use]
    pub fn from_config(config: &Config) -> Self {
        let mut buttons = BTreeMap::new();
        for button in RemoteButton::LOGICAL {
            buttons.insert(
                button.name().to_string(),
                ButtonDocument {
                    single: config
                        .map
                        .resolve(button, Trigger::Single)
                        .map(ToString::to_string),
                    double: config
                        .map
                        .resolve(button, Trigger::Double)
                        .map(ToString::to_string),
                    long: config
                        .map
                        .resolve(button, Trigger::Long)
                        .map(ToString::to_string),
                },
            );
        }
        Self {
            schema_version: SCHEMA_VERSION,
            profile: config.profile.clone(),
            device: config.device.clone(),
            adapter: config.adapter.clone(),
            gain_db: config.gain_db,
            input: config.input,
            vinput: config.vinput,
            trigger_key: config.trigger_key.clone(),
            allow_exec: config.allow_exec,
            gesture: config.gesture.into(),
            buttons,
        }
    }

    /// Validate the document and produce the effective configuration.
    pub fn validate(&self, profile: &DeviceProfile) -> Result<Config, ConfigError> {
        check_schema(self.schema_version)?;
        if self.profile != profile.id {
            return Err(ConfigError::Profile {
                found: self.profile.clone(),
                expected: profile.id.to_string(),
            });
        }
        if !(-MAX_GAIN_DB..=MAX_GAIN_DB).contains(&self.gain_db) {
            return Err(ConfigError::GainRange(self.gain_db));
        }
        let trigger_key = match self.trigger_key.as_deref() {
            Some(name) => parse_trigger_key(name)?,
            None => None,
        };
        let gesture = self.gesture.validate_effective()?;

        let mut map = ActionMap::defaults();
        for (name, slots) in &self.buttons {
            let button =
                button_by_name(name).ok_or_else(|| ConfigError::UnknownButton(name.clone()))?;
            if button == RemoteButton::Voice {
                return Err(ConfigError::VoiceMapped);
            }
            for (trigger, value) in slots.slots() {
                let Some(raw) = value else {
                    continue;
                };
                let action = parse_checked_action(raw, self.allow_exec).map_err(|source| {
                    ConfigError::Action {
                        button: name.clone(),
                        trigger: trigger.name(),
                        value: raw.clone(),
                        source,
                    }
                })?;
                map.set(button, trigger, action)
                    .map_err(|error| ConfigError::Action {
                        button: name.clone(),
                        trigger: trigger.name(),
                        value: raw.clone(),
                        source: ActionCheckError::Grammar(error),
                    })?;
            }
        }

        Ok(Config {
            profile: profile.id.to_string(),
            device: self.device.clone(),
            adapter: self.adapter.clone(),
            gain_db: self.gain_db,
            input: self.input,
            vinput: self.vinput,
            trigger_key,
            allow_exec: self.allow_exec,
            gesture: gesture.into(),
            map,
        })
    }

    /// Minimal TOML representation relative to the profile defaults.
    #[must_use]
    pub fn to_file(&self, profile: &DeviceProfile) -> FileConfig {
        let defaults = ActionMap::defaults();
        let mut buttons = BTreeMap::new();
        for (name, slots) in &self.buttons {
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

        let gesture = self.gesture.overrides_from(profile.gesture_timing);
        let trigger_key = match self.trigger_key.as_deref() {
            Some(DEFAULT_TRIGGER_KEY) => None,
            Some(key) => Some(key.to_string()),
            None => Some("none".to_string()),
        };
        FileConfig {
            schema_version: Some(SCHEMA_VERSION),
            profile: None,
            device: self.device.clone(),
            adapter: self.adapter.clone(),
            gain_db: (self.gain_db != profile.gain_db).then_some(self.gain_db),
            input: (!self.input).then_some(false),
            vinput: (self.vinput != VinputMode::Trigger).then_some(self.vinput),
            trigger_key,
            allow_exec: self.allow_exec.then_some(true),
            gesture: (!gesture.is_empty()).then_some(gesture),
            buttons,
        }
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
    let config = document.validate(profile)?;
    let normalized = ConfigDocument::from_config(&config);
    write_file(path, &normalized.to_file(profile))
}

fn write_file(path: &Path, file: &FileConfig) -> Result<(), ConfigError> {
    let text = toml::to_string_pretty(file).map_err(|source| ConfigError::Serialize {
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
    #[error("schema_version {found} is not supported (expected {SCHEMA_VERSION})")]
    SchemaVersion { found: u32 },
    #[error("gain_db {0} is outside -{MAX_GAIN_DB}..={MAX_GAIN_DB}")]
    GainRange(i8),
    #[error("trigger_key {0:?} is not a known input key name (use `none` to disable)")]
    TriggerKey(String),
    #[error("{field} {value} is outside {min}..={max}")]
    GestureRange {
        field: &'static str,
        value: u16,
        min: u16,
        max: u16,
    },
    #[error("profile {found:?} is not the active profile {expected:?}")]
    Profile { found: String, expected: String },
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use atvv_protocol::profile::RC003;

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
        assert_eq!(loaded.config.gesture, RC003.gesture_timing);
        assert_eq!(
            loaded.config.map.resolve(RemoteButton::Ok, Trigger::Single),
            Some(&Action::Key {
                name: "KEY_ENTER".into()
            })
        );
    }

    #[test]
    fn schema_version_round_trips_and_mismatches_are_rejected() {
        let mut document = defaults();
        assert_eq!(document.schema_version, SCHEMA_VERSION);
        let json = serde_json::to_string(&document).unwrap();
        assert_eq!(
            serde_json::from_str::<ConfigDocument>(&json).unwrap(),
            document
        );

        document.schema_version = 2;
        assert!(matches!(
            document.validate(&RC003),
            Err(ConfigError::SchemaVersion { found: 2 })
        ));

        let file = FileConfig {
            schema_version: Some(2),
            ..FileConfig::default()
        };
        assert!(matches!(
            file.validate(&RC003),
            Err(ConfigError::SchemaVersion { found: 2 })
        ));
    }

    #[test]
    fn button_overrides_merge_with_profile_defaults() {
        let path = path("buttons");
        std::fs::write(
            &path,
            "[buttons.up]\nsingle = \"key:KEY_F13\"\n[buttons.ok]\nlong = \"key:KEY_M\"\n",
        )
        .unwrap();
        let loaded = load(&path, &RC003).unwrap();
        assert_eq!(
            loaded.config.map.resolve(RemoteButton::Up, Trigger::Single),
            Some(&Action::Key {
                name: "KEY_F13".into()
            })
        );
        assert_eq!(
            loaded.config.map.resolve(RemoteButton::Ok, Trigger::Long),
            Some(&Action::Key {
                name: "KEY_M".into()
            })
        );
        // Untouched slots and buttons keep the profile defaults.
        assert_eq!(
            loaded.config.map.resolve(RemoteButton::Ok, Trigger::Single),
            Some(&Action::Key {
                name: "KEY_ENTER".into()
            })
        );
        assert_eq!(
            loaded
                .config
                .map
                .resolve(RemoteButton::Home, Trigger::Single),
            Some(&Action::Key {
                name: "KEY_HOMEPAGE".into()
            })
        );
        assert!(
            loaded
                .config
                .map
                .resolve(RemoteButton::Ok, Trigger::Double)
                .is_none()
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn invalid_actions_name_the_button_trigger_and_value() {
        let file = FileConfig {
            buttons: BTreeMap::from([(
                "up".to_string(),
                ButtonToml {
                    single: Some("key:not-a-key".to_string()),
                    ..ButtonToml::default()
                },
            )]),
            ..FileConfig::default()
        };
        let error = file.validate(&RC003).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("buttons.up.single"), "{message}");
        assert!(message.contains("not-a-key"), "{message}");
    }

    #[test]
    fn unknown_button_and_voice_key_are_rejected() {
        let unknown = FileConfig {
            buttons: BTreeMap::from([("sideways".to_string(), ButtonToml::default())]),
            ..FileConfig::default()
        };
        assert!(matches!(
            unknown.validate(&RC003),
            Err(ConfigError::UnknownButton(name)) if name == "sideways"
        ));

        let voice = FileConfig {
            buttons: BTreeMap::from([(
                "voice".to_string(),
                ButtonToml {
                    single: Some("key:KEY_F5".to_string()),
                    ..ButtonToml::default()
                },
            )]),
            ..FileConfig::default()
        };
        assert!(matches!(
            voice.validate(&RC003),
            Err(ConfigError::VoiceMapped)
        ));
    }

    #[test]
    fn duplicate_chord_keys_are_rejected() {
        for action in ["combo:ctrl+ctrl+a", "combo:ctrl+shift+leftctrl"] {
            let file = FileConfig {
                buttons: BTreeMap::from([(
                    "ok".to_string(),
                    ButtonToml {
                        single: Some(action.to_string()),
                        ..ButtonToml::default()
                    },
                )]),
                ..FileConfig::default()
            };
            let error = file.validate(&RC003).unwrap_err();
            assert!(
                matches!(
                    error,
                    ConfigError::Action {
                        source: ActionCheckError::DuplicateChordKey(_),
                        ..
                    }
                ),
                "{action}: {error}"
            );
        }
    }

    #[test]
    fn exec_is_disabled_until_enabled_and_bounded() {
        let exec = FileConfig {
            buttons: BTreeMap::from([(
                "ok".to_string(),
                ButtonToml {
                    single: Some("exec:notify-send hello".to_string()),
                    ..ButtonToml::default()
                },
            )]),
            ..FileConfig::default()
        };
        assert!(matches!(
            exec.validate(&RC003),
            Err(ConfigError::Action {
                source: ActionCheckError::ExecDisabled,
                ..
            })
        ));

        let allowed = FileConfig {
            allow_exec: Some(true),
            ..exec
        };
        let config = allowed.validate(&RC003).unwrap();
        assert_eq!(
            config.map.resolve(RemoteButton::Ok, Trigger::Single),
            Some(&Action::Exec {
                command: "notify-send hello".into()
            })
        );

        let long = format!("exec:{}", "x".repeat(MAX_EXEC_COMMAND + 1));
        let allowed = FileConfig {
            allow_exec: Some(true),
            buttons: BTreeMap::from([(
                "ok".to_string(),
                ButtonToml {
                    single: Some(long),
                    ..ButtonToml::default()
                },
            )]),
            ..FileConfig::default()
        };
        assert!(matches!(
            allowed.validate(&RC003),
            Err(ConfigError::Action {
                source: ActionCheckError::ExecTooLong,
                ..
            })
        ));
    }

    #[test]
    fn gesture_overrides_are_validated_and_merged() {
        let mut file = FileConfig {
            gesture: Some(GestureOverrides {
                double_click_ms: Some(350),
                ..GestureOverrides::default()
            }),
            ..FileConfig::default()
        };
        let config = file.validate(&RC003).unwrap();
        assert_eq!(config.gesture.double_click_ms, 350);
        assert_eq!(
            config.gesture.long_press_ms,
            RC003.gesture_timing.long_press_ms
        );

        file.gesture = Some(GestureOverrides {
            long_press_ms: Some(10),
            ..GestureOverrides::default()
        });
        assert!(matches!(
            file.validate(&RC003),
            Err(ConfigError::GestureRange {
                field: "long_press_ms",
                ..
            })
        ));
    }

    #[test]
    fn document_round_trips_through_json_and_file() {
        let mut document = defaults();
        document.buttons.get_mut("up").unwrap().single = Some("key:KEY_F13".into());
        document.buttons.get_mut("ok").unwrap().long = Some("key:KEY_M".into());
        document.gain_db = 6;
        document.allow_exec = true;
        document.buttons.get_mut("menu").unwrap().single = Some("exec:true".into());

        let json = serde_json::to_string(&document).unwrap();
        let decoded: ConfigDocument = serde_json::from_str(&json).unwrap();
        let config = decoded.validate(&RC003).unwrap();
        assert_eq!(config.gain_db, 6);
        assert_eq!(
            config.map.resolve(RemoteButton::Up, Trigger::Single),
            Some(&Action::Key {
                name: "KEY_F13".into()
            })
        );
        assert_eq!(
            config.map.resolve(RemoteButton::Menu, Trigger::Single),
            Some(&Action::Exec {
                command: "true".into()
            })
        );

        let path = path("document");
        save_document(&path, &document, &RC003).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("schema_version = 1"), "{text}");
        assert!(text.contains("gain_db = 6"), "{text}");
        assert!(text.contains("allow_exec = true"), "{text}");
        assert!(text.contains("[buttons.up]"), "{text}");
        assert!(text.contains("single = \"key:KEY_F13\""), "{text}");
        assert!(!text.contains("[buttons.home]"), "{text}");

        let loaded = load(&path, &RC003).unwrap();
        assert_eq!(loaded.config.gain_db, 6);
        assert_eq!(
            loaded.config.map.resolve(RemoteButton::Ok, Trigger::Long),
            Some(&Action::Key {
                name: "KEY_M".into()
            })
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn document_null_slot_means_profile_default() {
        let mut document = defaults();
        document.buttons.get_mut("ok").unwrap().single = None;
        document.buttons.get_mut("ok").unwrap().long = Some("none".into());
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
    }

    #[test]
    fn save_device_round_trips_and_preserves_settings() {
        let path = path("save");
        std::fs::write(&path, "gain_db = 6\ninput = false\n").unwrap();
        save_device(&path, "C0:5D:39:F8:2D:4C", Some("hci0")).unwrap();

        let loaded = load(&path, &RC003).unwrap();
        assert_eq!(loaded.config.device.as_deref(), Some("C0:5D:39:F8:2D:4C"));
        assert_eq!(loaded.config.adapter.as_deref(), Some("hci0"));
        assert_eq!(loaded.config.gain_db, 6);
        assert!(!loaded.config.input);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_device_refuses_a_malformed_or_future_file() {
        let path = path("malformed");
        std::fs::write(&path, "gaiN_db = 3\n").unwrap();
        assert!(matches!(
            save_device(&path, "C0:5D:39:F8:2D:4C", None),
            Err(ConfigError::Parse { .. })
        ));

        std::fs::write(&path, "schema_version = 2\n").unwrap();
        assert!(matches!(
            save_device(&path, "C0:5D:39:F8:2D:4C", None),
            Err(ConfigError::SchemaVersion { found: 2 })
        ));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn profile_mismatch_is_rejected() {
        let file = FileConfig {
            profile: Some("other-remote".to_string()),
            ..FileConfig::default()
        };
        assert!(matches!(
            file.validate(&RC003),
            Err(ConfigError::Profile { found, expected })
                if found == "other-remote" && expected == RC003.id
        ));

        let mut document = defaults();
        document.profile = "other-remote".to_string();
        assert!(matches!(
            document.validate(&RC003),
            Err(ConfigError::Profile { found, .. }) if found == "other-remote"
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
        assert_eq!(loaded.config.gesture, RC003.gesture_timing);
        let _ = std::fs::remove_file(&path);
    }
}
