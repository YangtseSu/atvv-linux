// SPDX-License-Identifier: GPL-3.0-or-later

//! Daemon configuration file.
//!
//! The file is optional. Missing files load the defaults; an unreadable or
//! invalid file is an error, so `ReloadConfig` can report a typo instead of
//! silently keeping the old values.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Gain clamp from the research notes: ±24 dB, no AGC.
pub const MAX_GAIN_DB: i8 = 24;

/// Raw `daemon.toml`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FileConfig {
    /// Override the profile gain in dB, clamped to ±24 dB.
    pub gain_db: Option<i8>,
    /// Start the evdev/uinput input path.
    pub input: Option<bool>,
}

/// Validated configuration with defaults applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedConfig {
    pub path: PathBuf,
    pub existed: bool,
    pub gain_db: i8,
    pub input: bool,
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
            gain_db: profile_gain_db,
            input: true,
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

    Ok(LoadedConfig {
        path: path.to_path_buf(),
        existed: true,
        gain_db,
        input: file.input.unwrap_or(true),
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
    #[error("gain_db {0} is outside -{MAX_GAIN_DB}..={MAX_GAIN_DB}")]
    GainRange(i8),
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
}
