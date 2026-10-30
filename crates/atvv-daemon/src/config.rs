// SPDX-License-Identifier: GPL-3.0-or-later

//! Daemon configuration file.
//!
//! The file is optional. Missing files load the defaults; an unreadable or
//! invalid file is an error, so `ReloadConfig` can report a typo instead of
//! silently keeping the old values.

use std::io;
use std::path::{Path, PathBuf};

use atvv_input::parse_key_program;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Gain clamp from the research notes: ±24 dB, no AGC.
pub const MAX_GAIN_DB: i8 = 24;

/// fcitx5-vinput trigger key used while the voice key is held.
///
/// The dictation consumer key (`KEY_DICTATE`, evdev `0x24a`). It is the
/// device function the remote's voice control represents, so the synthetic
/// trigger carries the meaning instead of a spare function key. fcitx5-vinput
/// should keep the user's own `[TriggerKey]` entries and add a `Dictate` entry
/// (for example `2=Dictate`); `F24` remains available as an alternative for
/// IMEs that do not know the consumer key. The target IME must be configured
/// to the same key.
pub const DEFAULT_TRIGGER_KEY: &str = "DICTATE";

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

/// Raw `daemon.toml`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FileConfig {
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
}

/// Validated configuration with defaults applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedConfig {
    pub path: PathBuf,
    pub existed: bool,
    pub device: Option<String>,
    pub adapter: Option<String>,
    pub gain_db: i8,
    pub input: bool,
    pub vinput: VinputMode,
    /// Key name; `None` disables synthetic trigger emission.
    pub trigger_key: Option<String>,
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

/// Load and validate a configuration file.
///
/// A missing file is not an error: it yields the defaults.
pub fn load(path: &Path, profile_gain_db: i8) -> Result<LoadedConfig, ConfigError> {
    if !path.exists() {
        return Ok(LoadedConfig {
            path: path.to_path_buf(),
            existed: false,
            device: None,
            adapter: None,
            gain_db: profile_gain_db,
            input: true,
            vinput: VinputMode::default(),
            trigger_key: Some(DEFAULT_TRIGGER_KEY.to_string()),
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

    let gain_db = file.gain_db.unwrap_or(profile_gain_db);
    if !(-MAX_GAIN_DB..=MAX_GAIN_DB).contains(&gain_db) {
        return Err(ConfigError::GainRange(gain_db));
    }

    let trigger_key =
        parse_trigger_key(file.trigger_key.as_deref().unwrap_or(DEFAULT_TRIGGER_KEY))?;

    Ok(LoadedConfig {
        path: path.to_path_buf(),
        existed: true,
        device: file.device,
        adapter: file.adapter,
        gain_db,
        input: file.input.unwrap_or(true),
        vinput: file.vinput.unwrap_or_default(),
        trigger_key,
    })
}

/// Validate a configured trigger key name.
///
/// `none` or an empty value disables synthetic trigger emission. Any other
/// value must resolve to a kernel key.
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
        toml::from_str(&text).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })?
    } else {
        FileConfig::default()
    };
    file.device = Some(device.to_string());
    if let Some(adapter) = adapter {
        file.adapter = Some(adapter.to_string());
    }
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
    std::fs::write(path, text).map_err(|source| ConfigError::Write {
        path: path.to_path_buf(),
        source,
    })
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
    #[error("gain_db {0} is outside -{MAX_GAIN_DB}..={MAX_GAIN_DB}")]
    GainRange(i8),
    #[error("trigger_key {0:?} is not a known input key name (use `none` to disable)")]
    TriggerKey(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_uses_defaults() {
        let path = std::env::temp_dir().join(format!(
            "atvv-config-missing-{}-{}.toml",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = std::fs::remove_file(&path);
        let config = load(&path, 0).unwrap();
        assert!(!config.existed);
        assert_eq!(config.gain_db, 0);
        assert!(config.input);
        assert_eq!(config.vinput, VinputMode::Trigger);
        assert_eq!(config.trigger_key.as_deref(), Some(DEFAULT_TRIGGER_KEY));
    }

    #[test]
    fn default_trigger_is_the_dictation_key() {
        assert_eq!(DEFAULT_TRIGGER_KEY, "DICTATE");
        assert!(parse_key_program(DEFAULT_TRIGGER_KEY).is_some());
    }

    #[test]
    fn parses_vinput_mode_and_trigger_key() {
        let path = std::env::temp_dir().join(format!(
            "atvv-config-vinput-{}-{}.toml",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::write(&path, "vinput = \"dbus\"\ntrigger_key = \"F18\"\n").unwrap();
        let config = load(&path, 0).unwrap();
        assert_eq!(config.vinput, VinputMode::Dbus);
        assert_eq!(config.trigger_key.as_deref(), Some("F18"));

        std::fs::write(&path, "trigger_key = \"none\"\n").unwrap();
        let config = load(&path, 0).unwrap();
        assert_eq!(config.vinput, VinputMode::Trigger);
        assert_eq!(config.trigger_key, None);

        std::fs::write(&path, "trigger_key = \"ctrl+alt+v\"\n").unwrap();
        let config = load(&path, 0).unwrap();
        assert_eq!(config.trigger_key.as_deref(), Some("ctrl+alt+v"));

        for key in ["DICTATE", "VOICECOMMAND", "ASSISTANT"] {
            std::fs::write(&path, format!("trigger_key = \"{key}\"\n")).unwrap();
            let config = load(&path, 0).unwrap();
            assert_eq!(config.trigger_key.as_deref(), Some(key));
        }

        std::fs::write(&path, "trigger_key = \"not-a-key\"\n").unwrap();
        assert!(matches!(
            load(&path, 0),
            Err(ConfigError::TriggerKey(key)) if key == "not-a-key"
        ));

        std::fs::write(&path, "vinput = \"carrier-pigeon\"\n").unwrap();
        assert!(matches!(load(&path, 0), Err(ConfigError::Parse { .. })));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn file_overrides_gain_and_input() {
        let path = std::env::temp_dir().join(format!(
            "atvv-config-{}-{}.toml",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::write(&path, "gain_db = 6\ninput = false\n").unwrap();
        let config = load(&path, 0).unwrap();
        assert!(config.existed);
        assert_eq!(config.gain_db, 6);
        assert!(!config.input);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn rejects_unknown_keys_and_out_of_range_gain() {
        let path = std::env::temp_dir().join(format!(
            "atvv-config-bad-{}-{}.toml",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::write(&path, "gaiN_db = 3\n").unwrap();
        assert!(matches!(load(&path, 0), Err(ConfigError::Parse { .. })));
        std::fs::write(&path, "gain_db = 40\n").unwrap();
        assert!(matches!(load(&path, 0), Err(ConfigError::GainRange(40))));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_device_round_trips_and_preserves_settings() {
        let path = std::env::temp_dir().join(format!(
            "atvv-config-save-{}-{}.toml",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::write(&path, "gain_db = 6\ninput = false\n").unwrap();
        save_device(&path, "C0:5D:39:F8:2D:4C", Some("hci0")).unwrap();

        let config = load(&path, 0).unwrap();
        assert_eq!(config.device.as_deref(), Some("C0:5D:39:F8:2D:4C"));
        assert_eq!(config.adapter.as_deref(), Some("hci0"));
        assert_eq!(config.gain_db, 6);
        assert!(!config.input);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_device_creates_the_file_and_keeps_an_unset_adapter() {
        let path = std::env::temp_dir().join(format!(
            "atvv-config-create-{}-{}.toml",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = std::fs::remove_file(&path);
        save_device(&path, "C0:5D:39:F8:2D:4C", None).unwrap();

        let config = load(&path, 0).unwrap();
        assert_eq!(config.device.as_deref(), Some("C0:5D:39:F8:2D:4C"));
        assert_eq!(config.adapter, None);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_device_refuses_a_malformed_existing_file() {
        let path = std::env::temp_dir().join(format!(
            "atvv-config-malformed-{}-{}.toml",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::write(&path, "gaiN_db = 3\n").unwrap();
        assert!(matches!(
            save_device(&path, "C0:5D:39:F8:2D:4C", None),
            Err(ConfigError::Parse { .. })
        ));
        let _ = std::fs::remove_file(&path);
    }
}
