// SPDX-License-Identifier: GPL-3.0-or-later

//! Key-name resolution and the profile key map.

use std::collections::BTreeMap;
use std::str::FromStr;

use atvv_protocol::profile::{DeviceProfile, RemoteButton};
use evdev::KeyCode;
use thiserror::Error;

use crate::action::{Action, ActionError, ActionMap};

/// One ordered key program: press modifiers first, release in reverse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyProgram {
    pub press: Vec<KeyCode>,
    pub release: Vec<KeyCode>,
}

impl KeyProgram {
    /// A single key held for as long as the caller keeps it pressed.
    #[must_use]
    pub fn single(code: KeyCode) -> Self {
        Self {
            press: vec![code],
            release: vec![code],
        }
    }
}

/// Profile keys that cannot be resolved to one input code.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum KeyMapError {
    #[error("profile key {key:?} for {button} is not a known input name")]
    Unknown {
        button: RemoteButton,
        key: &'static str,
    },
    #[error("input key {key:?} is mapped to both {first} and {second}")]
    Duplicate {
        key: String,
        first: RemoteButton,
        second: RemoteButton,
    },
}

/// Physical key code to logical button, built from a device profile.
#[derive(Debug, Clone)]
pub struct KeyMap {
    codes: BTreeMap<KeyCode, RemoteButton>,
}

impl KeyMap {
    /// Resolve every `button_keys` entry of a profile.
    pub fn from_profile(profile: &DeviceProfile) -> Result<Self, KeyMapError> {
        let mut codes = BTreeMap::new();
        for mapping in profile.button_keys {
            let code = resolve_key(mapping.key).ok_or(KeyMapError::Unknown {
                button: mapping.button,
                key: mapping.key,
            })?;
            if let Some(previous) = codes.insert(code, mapping.button) {
                if previous != mapping.button {
                    return Err(KeyMapError::Duplicate {
                        key: mapping.key.to_string(),
                        first: previous,
                        second: mapping.button,
                    });
                }
            }
        }
        Ok(Self { codes })
    }

    /// Logical button for a physical key, if the profile maps it.
    #[must_use]
    pub fn button(&self, code: KeyCode) -> Option<RemoteButton> {
        self.codes.get(&code).copied()
    }

    /// Every mapped key code.
    pub fn codes(&self) -> impl Iterator<Item = KeyCode> + '_ {
        self.codes.keys().copied()
    }
}

/// Key-name aliases. First match wins; names are normalized by
/// [`normalize`] (lower-case, `-`/`_`/spaces stripped).
const ALIASES: &[(&str, KeyCode)] = &[
    ("logo", KeyCode::KEY_LEFTMETA),
    ("leftmeta", KeyCode::KEY_LEFTMETA),
    ("super", KeyCode::KEY_LEFTMETA),
    ("meta", KeyCode::KEY_LEFTMETA),
    ("win", KeyCode::KEY_LEFTMETA),
    ("cmd", KeyCode::KEY_LEFTMETA),
    ("rightmeta", KeyCode::KEY_RIGHTMETA),
    ("alt", KeyCode::KEY_LEFTALT),
    ("option", KeyCode::KEY_LEFTALT),
    ("leftalt", KeyCode::KEY_LEFTALT),
    ("altgr", KeyCode::KEY_RIGHTALT),
    ("altr", KeyCode::KEY_RIGHTALT),
    ("rightalt", KeyCode::KEY_RIGHTALT),
    ("shift", KeyCode::KEY_LEFTSHIFT),
    ("leftshift", KeyCode::KEY_LEFTSHIFT),
    ("rightshift", KeyCode::KEY_RIGHTSHIFT),
    ("ctrl", KeyCode::KEY_LEFTCTRL),
    ("control", KeyCode::KEY_LEFTCTRL),
    ("leftctrl", KeyCode::KEY_LEFTCTRL),
    ("rightctrl", KeyCode::KEY_RIGHTCTRL),
    ("return", KeyCode::KEY_ENTER),
    ("enter", KeyCode::KEY_ENTER),
    ("kpenter", KeyCode::KEY_KPENTER),
    ("esc", KeyCode::KEY_ESC),
    ("escape", KeyCode::KEY_ESC),
    ("tab", KeyCode::KEY_TAB),
    ("space", KeyCode::KEY_SPACE),
    ("backspace", KeyCode::KEY_BACKSPACE),
    ("bs", KeyCode::KEY_BACKSPACE),
    ("delete", KeyCode::KEY_DELETE),
    ("del", KeyCode::KEY_DELETE),
    ("insert", KeyCode::KEY_INSERT),
    ("ins", KeyCode::KEY_INSERT),
    ("up", KeyCode::KEY_UP),
    ("down", KeyCode::KEY_DOWN),
    ("left", KeyCode::KEY_LEFT),
    ("right", KeyCode::KEY_RIGHT),
    ("home", KeyCode::KEY_HOME),
    ("end", KeyCode::KEY_END),
    ("pageup", KeyCode::KEY_PAGEUP),
    ("pgup", KeyCode::KEY_PAGEUP),
    ("pagedown", KeyCode::KEY_PAGEDOWN),
    ("pgdn", KeyCode::KEY_PAGEDOWN),
    ("menu", KeyCode::KEY_MENU),
    ("contextmenu", KeyCode::KEY_CONTEXT_MENU),
    ("menukey", KeyCode::KEY_CONTEXT_MENU),
    ("grave", KeyCode::KEY_GRAVE),
    ("backtick", KeyCode::KEY_GRAVE),
    ("minus", KeyCode::KEY_MINUS),
    ("dash", KeyCode::KEY_MINUS),
    ("equal", KeyCode::KEY_EQUAL),
    ("semicolon", KeyCode::KEY_SEMICOLON),
    ("apostrophe", KeyCode::KEY_APOSTROPHE),
    ("comma", KeyCode::KEY_COMMA),
    ("dot", KeyCode::KEY_DOT),
    ("period", KeyCode::KEY_DOT),
    ("slash", KeyCode::KEY_SLASH),
    ("backslash", KeyCode::KEY_BACKSLASH),
    ("volumeup", KeyCode::KEY_VOLUMEUP),
    ("volumedown", KeyCode::KEY_VOLUMEDOWN),
    ("mute", KeyCode::KEY_MUTE),
    ("micmute", KeyCode::KEY_MICMUTE),
    ("playpause", KeyCode::KEY_PLAYPAUSE),
    ("nextsong", KeyCode::KEY_NEXTSONG),
    ("prevsong", KeyCode::KEY_PREVIOUSSONG),
    ("power", KeyCode::KEY_POWER),
    ("homepage", KeyCode::KEY_HOMEPAGE),
    ("brightnessup", KeyCode::KEY_BRIGHTNESSUP),
    ("brightnessdown", KeyCode::KEY_BRIGHTNESSDOWN),
];

fn normalize(name: &str) -> String {
    name.chars()
        .filter(|character| !matches!(character, '-' | '_' | ' '))
        .flat_map(char::to_lowercase)
        .collect()
}

/// Resolve a key name: verbatim kernel name, alias, or synthesized
/// `KEY_<NAME>` (`f5` → `KEY_F5`, `a` → `KEY_A`).
#[must_use]
pub fn resolve_key(name: &str) -> Option<KeyCode> {
    let name = name.trim();
    if let Ok(code) = KeyCode::from_str(name) {
        return Some(code);
    }
    let normalized = normalize(name);
    if let Some((_, code)) = ALIASES.iter().find(|(alias, _)| *alias == normalized) {
        return Some(*code);
    }
    let base = normalized
        .strip_prefix("key")
        .filter(|rest| !rest.is_empty())
        .unwrap_or(&normalized);
    KeyCode::from_str(&format!("KEY_{}", base.to_uppercase())).ok()
}

/// Parse a synth expression: a single key (`F24`, `F18`) or a chord
/// (`ctrl+alt+v`). Modifiers are pressed first and released in reverse.
///
/// Chords exist for compositors that refuse bare modifier or bare function
/// key bindings; the IME must be configured to the same chord.
#[must_use]
pub fn parse_key_program(input: &str) -> Option<KeyProgram> {
    let tokens: Vec<&str> = input.split('+').map(str::trim).collect();
    if tokens.is_empty() || tokens.iter().any(|token| token.is_empty()) {
        return None;
    }
    let (key, mods) = tokens.split_last()?;
    let action = if mods.is_empty() {
        Action::Key {
            name: (*key).to_string(),
        }
    } else {
        Action::Combo {
            mods: mods.iter().map(|token| (*token).to_string()).collect(),
            key: (*key).to_string(),
        }
    };
    KeyResolver.program(&action).ok()
}

/// Resolves actions into ordered key programs.
#[derive(Debug, Clone, Copy, Default)]
pub struct KeyResolver;

impl KeyResolver {
    /// Compile an action into a press/release program.
    pub fn program(&self, action: &Action) -> Result<KeyProgram, ActionError> {
        match action {
            Action::Key { name } => {
                let code =
                    resolve_key(name).ok_or_else(|| ActionError::UnknownKey(name.clone()))?;
                Ok(KeyProgram {
                    press: vec![code],
                    release: vec![code],
                })
            }
            Action::Combo { mods, key } => {
                let mut press = Vec::with_capacity(mods.len() + 1);
                for modifier in mods {
                    let code = resolve_key(modifier)
                        .ok_or_else(|| ActionError::UnknownKey(modifier.clone()))?;
                    if !press.contains(&code) {
                        press.push(code);
                    }
                }
                let code = resolve_key(key).ok_or_else(|| ActionError::UnknownKey(key.clone()))?;
                press.push(code);
                let release = press.iter().rev().copied().collect();
                Ok(KeyProgram { press, release })
            }
            Action::None | Action::Exec { .. } | Action::Dictation(_) => {
                Err(ActionError::NotEmittable)
            }
        }
    }

    /// Every physical code the action map can emit, for uinput registration.
    pub fn codes_for(&self, map: &ActionMap) -> Result<Vec<KeyCode>, ActionError> {
        let mut codes = Vec::new();
        for action in map.actions() {
            match self.program(action) {
                Ok(program) => codes.extend(program.press),
                Err(ActionError::NotEmittable) => {}
                Err(error) => return Err(error),
            }
        }
        codes.sort_unstable();
        codes.dedup();
        Ok(codes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atvv_protocol::RC003;

    #[test]
    fn profile_key_map_covers_the_remote() {
        let map = KeyMap::from_profile(&RC003).unwrap();
        assert_eq!(map.button(KeyCode::KEY_F5), Some(RemoteButton::Voice));
        assert_eq!(map.button(KeyCode::KEY_GRAVE), Some(RemoteButton::Tv));
        assert_eq!(map.button(KeyCode::KEY_HOMEPAGE), Some(RemoteButton::Home));
        assert_eq!(map.button(KeyCode::KEY_ENTER), Some(RemoteButton::Ok));
        assert_eq!(map.button(KeyCode::KEY_KPENTER), Some(RemoteButton::Ok));
        assert_eq!(map.button(KeyCode::KEY_BACK), Some(RemoteButton::Back));
        assert_eq!(map.button(KeyCode::KEY_ESC), Some(RemoteButton::Back));
        assert_eq!(
            map.button(KeyCode::KEY_VOLUMEDOWN),
            Some(RemoteButton::VolumeDown)
        );
        assert_eq!(map.button(KeyCode::KEY_A), None);
        // Every logical button is reachable from the profile.
        for button in RemoteButton::LOGICAL {
            assert!(
                map.codes().any(|code| map.button(code) == Some(button)),
                "{button}"
            );
        }
    }

    #[test]
    fn resolves_kernel_names_aliases_and_synthesis() {
        assert_eq!(resolve_key("KEY_F5"), Some(KeyCode::KEY_F5));
        assert_eq!(resolve_key("f5"), Some(KeyCode::KEY_F5));
        assert_eq!(resolve_key("key_f5"), Some(KeyCode::KEY_F5));
        assert_eq!(resolve_key("F5"), Some(KeyCode::KEY_F5));
        assert_eq!(resolve_key("F24"), Some(KeyCode::KEY_F24));
        assert_eq!(resolve_key("ctrl"), Some(KeyCode::KEY_LEFTCTRL));
        assert_eq!(resolve_key("super"), Some(KeyCode::KEY_LEFTMETA));
        assert_eq!(resolve_key("alt_r"), Some(KeyCode::KEY_RIGHTALT));
        assert_eq!(resolve_key("Alt_R"), Some(KeyCode::KEY_RIGHTALT));
        assert_eq!(resolve_key("a"), Some(KeyCode::KEY_A));
        assert_eq!(resolve_key("7"), Some(KeyCode::KEY_7));
        assert_eq!(resolve_key("context-menu"), Some(KeyCode::KEY_CONTEXT_MENU));
        assert_eq!(resolve_key("no-such-key"), None);
    }

    #[test]
    fn single_key_program_presses_and_releases_the_same_key() {
        let program = KeyProgram::single(KeyCode::KEY_RIGHTALT);
        assert_eq!(program.press, vec![KeyCode::KEY_RIGHTALT]);
        assert_eq!(program.release, vec![KeyCode::KEY_RIGHTALT]);
    }

    #[test]
    fn parses_single_keys_and_chords() {
        let single = parse_key_program("F24").unwrap();
        assert_eq!(single.press, vec![KeyCode::KEY_F24]);
        assert_eq!(single.release, vec![KeyCode::KEY_F24]);

        let chord = parse_key_program("ctrl+alt+v").unwrap();
        assert_eq!(
            chord.press,
            vec![KeyCode::KEY_LEFTCTRL, KeyCode::KEY_LEFTALT, KeyCode::KEY_V]
        );
        assert_eq!(
            chord.release,
            vec![KeyCode::KEY_V, KeyCode::KEY_LEFTALT, KeyCode::KEY_LEFTCTRL]
        );

        assert!(parse_key_program("ctrl+").is_none());
        assert!(parse_key_program("ctrl+nope").is_none());
    }

    #[test]
    fn compiles_taps_and_combos() {
        let resolver = KeyResolver;
        let tap = resolver
            .program(&Action::Key {
                name: "KEY_ENTER".into(),
            })
            .unwrap();
        assert_eq!(tap.press, vec![KeyCode::KEY_ENTER]);
        assert_eq!(tap.release, vec![KeyCode::KEY_ENTER]);

        let combo = resolver
            .program(&Action::Combo {
                mods: vec!["ctrl".into(), "alt".into()],
                key: "delete".into(),
            })
            .unwrap();
        assert_eq!(
            combo.press,
            vec![
                KeyCode::KEY_LEFTCTRL,
                KeyCode::KEY_LEFTALT,
                KeyCode::KEY_DELETE
            ]
        );
        assert_eq!(
            combo.release,
            vec![
                KeyCode::KEY_DELETE,
                KeyCode::KEY_LEFTALT,
                KeyCode::KEY_LEFTCTRL
            ]
        );
    }

    #[test]
    fn rejects_non_key_actions_and_unknown_names() {
        let resolver = KeyResolver;
        assert_eq!(
            resolver.program(&Action::None),
            Err(ActionError::NotEmittable)
        );
        assert_eq!(
            resolver.program(&Action::Key {
                name: "nope".into()
            }),
            Err(ActionError::UnknownKey("nope".into()))
        );
    }

    #[test]
    fn default_map_codes_resolve() {
        let resolver = KeyResolver;
        let codes = resolver.codes_for(&ActionMap::defaults()).unwrap();
        assert!(codes.contains(&KeyCode::KEY_ENTER));
        assert!(codes.contains(&KeyCode::KEY_GRAVE));
        assert!(codes.contains(&KeyCode::KEY_HOMEPAGE));
        assert!(!codes.contains(&KeyCode::KEY_POWER));
    }
}
