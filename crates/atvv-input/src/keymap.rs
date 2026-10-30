// SPDX-License-Identifier: GPL-3.0-or-later

//! Key-name resolution and the profile key map.

use std::collections::BTreeMap;
use std::str::FromStr;

use atvv_protocol::profile::{DeviceProfile, RemoteButton};
use evdevil::event::Key;
use thiserror::Error;

use crate::action::{Action, ActionError, ActionMap};

/// One ordered key program: press modifiers first, release in reverse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyProgram {
    pub press: Vec<Key>,
    pub release: Vec<Key>,
}

impl KeyProgram {
    /// A single key held for as long as the caller keeps it pressed.
    #[must_use]
    pub fn single(code: Key) -> Self {
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
    codes: BTreeMap<Key, RemoteButton>,
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
    pub fn button(&self, code: Key) -> Option<RemoteButton> {
        self.codes.get(&code).copied()
    }

    /// Every mapped key code.
    pub fn codes(&self) -> impl Iterator<Item = Key> + '_ {
        self.codes.keys().copied()
    }
}

/// Key-name aliases. First match wins; names are normalized by
/// [`normalize`] (lower-case, `-`/`_`/spaces stripped).
const ALIASES: &[(&str, Key)] = &[
    ("logo", Key::KEY_LEFTMETA),
    ("leftmeta", Key::KEY_LEFTMETA),
    ("super", Key::KEY_LEFTMETA),
    ("meta", Key::KEY_LEFTMETA),
    ("win", Key::KEY_LEFTMETA),
    ("cmd", Key::KEY_LEFTMETA),
    ("rightmeta", Key::KEY_RIGHTMETA),
    ("alt", Key::KEY_LEFTALT),
    ("option", Key::KEY_LEFTALT),
    ("leftalt", Key::KEY_LEFTALT),
    ("altgr", Key::KEY_RIGHTALT),
    ("altr", Key::KEY_RIGHTALT),
    ("rightalt", Key::KEY_RIGHTALT),
    ("shift", Key::KEY_LEFTSHIFT),
    ("leftshift", Key::KEY_LEFTSHIFT),
    ("rightshift", Key::KEY_RIGHTSHIFT),
    ("ctrl", Key::KEY_LEFTCTRL),
    ("control", Key::KEY_LEFTCTRL),
    ("leftctrl", Key::KEY_LEFTCTRL),
    ("rightctrl", Key::KEY_RIGHTCTRL),
    ("return", Key::KEY_ENTER),
    ("enter", Key::KEY_ENTER),
    ("kpenter", Key::KEY_KPENTER),
    ("esc", Key::KEY_ESC),
    ("escape", Key::KEY_ESC),
    ("tab", Key::KEY_TAB),
    ("space", Key::KEY_SPACE),
    ("backspace", Key::KEY_BACKSPACE),
    ("bs", Key::KEY_BACKSPACE),
    ("delete", Key::KEY_DELETE),
    ("del", Key::KEY_DELETE),
    ("insert", Key::KEY_INSERT),
    ("ins", Key::KEY_INSERT),
    ("up", Key::KEY_UP),
    ("down", Key::KEY_DOWN),
    ("left", Key::KEY_LEFT),
    ("right", Key::KEY_RIGHT),
    ("home", Key::KEY_HOME),
    ("end", Key::KEY_END),
    ("pageup", Key::KEY_PAGEUP),
    ("pgup", Key::KEY_PAGEUP),
    ("pagedown", Key::KEY_PAGEDOWN),
    ("pgdn", Key::KEY_PAGEDOWN),
    ("menu", Key::KEY_MENU),
    ("contextmenu", Key::KEY_CONTEXT_MENU),
    ("menukey", Key::KEY_CONTEXT_MENU),
    ("grave", Key::KEY_GRAVE),
    ("backtick", Key::KEY_GRAVE),
    ("minus", Key::KEY_MINUS),
    ("dash", Key::KEY_MINUS),
    ("equal", Key::KEY_EQUAL),
    ("semicolon", Key::KEY_SEMICOLON),
    ("apostrophe", Key::KEY_APOSTROPHE),
    ("comma", Key::KEY_COMMA),
    ("dot", Key::KEY_DOT),
    ("period", Key::KEY_DOT),
    ("slash", Key::KEY_SLASH),
    ("backslash", Key::KEY_BACKSLASH),
    ("volumeup", Key::KEY_VOLUMEUP),
    ("volumedown", Key::KEY_VOLUMEDOWN),
    ("mute", Key::KEY_MUTE),
    ("micmute", Key::KEY_MICMUTE),
    ("playpause", Key::KEY_PLAYPAUSE),
    ("nextsong", Key::KEY_NEXTSONG),
    ("prevsong", Key::KEY_PREVIOUSSONG),
    ("power", Key::KEY_POWER),
    ("homepage", Key::KEY_HOMEPAGE),
    ("brightnessup", Key::KEY_BRIGHTNESSUP),
    ("brightnessdown", Key::KEY_BRIGHTNESSDOWN),
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
pub fn resolve_key(name: &str) -> Option<Key> {
    let name = name.trim();
    if let Ok(code) = Key::from_str(name) {
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
    Key::from_str(&format!("KEY_{}", base.to_uppercase())).ok()
}

/// Parse a synth expression: a single key (`DICTATE`, `F24`) or a chord
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
    pub fn codes_for(&self, map: &ActionMap) -> Result<Vec<Key>, ActionError> {
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
        assert_eq!(map.button(Key::KEY_F5), Some(RemoteButton::Voice));
        assert_eq!(map.button(Key::KEY_GRAVE), Some(RemoteButton::Tv));
        assert_eq!(map.button(Key::KEY_HOMEPAGE), Some(RemoteButton::Home));
        assert_eq!(map.button(Key::KEY_ENTER), Some(RemoteButton::Ok));
        assert_eq!(map.button(Key::KEY_KPENTER), Some(RemoteButton::Ok));
        assert_eq!(map.button(Key::KEY_BACK), Some(RemoteButton::Back));
        assert_eq!(map.button(Key::KEY_ESC), Some(RemoteButton::Back));
        assert_eq!(
            map.button(Key::KEY_VOLUMEDOWN),
            Some(RemoteButton::VolumeDown)
        );
        assert_eq!(map.button(Key::KEY_A), None);
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
        assert_eq!(resolve_key("KEY_F5"), Some(Key::KEY_F5));
        assert_eq!(resolve_key("f5"), Some(Key::KEY_F5));
        assert_eq!(resolve_key("key_f5"), Some(Key::KEY_F5));
        assert_eq!(resolve_key("F5"), Some(Key::KEY_F5));
        assert_eq!(resolve_key("F24"), Some(Key::KEY_F24));
        assert_eq!(resolve_key("ctrl"), Some(Key::KEY_LEFTCTRL));
        assert_eq!(resolve_key("super"), Some(Key::KEY_LEFTMETA));
        assert_eq!(resolve_key("alt_r"), Some(Key::KEY_RIGHTALT));
        assert_eq!(resolve_key("Alt_R"), Some(Key::KEY_RIGHTALT));
        assert_eq!(resolve_key("a"), Some(Key::KEY_A));
        assert_eq!(resolve_key("7"), Some(Key::KEY_7));
        assert_eq!(resolve_key("context-menu"), Some(Key::KEY_CONTEXT_MENU));
        assert_eq!(resolve_key("no-such-key"), None);
    }

    #[test]
    fn consumer_keys_resolve_to_the_kernel_codes() {
        assert_eq!(resolve_key("VOICECOMMAND"), Some(Key::KEY_VOICECOMMAND));
        assert_eq!(resolve_key("voicecommand"), Some(Key::KEY_VOICECOMMAND));
        assert_eq!(resolve_key("KEY_VOICECOMMAND"), Some(Key::KEY_VOICECOMMAND));
        assert_eq!(resolve_key("ASSISTANT"), Some(Key::KEY_ASSISTANT));
        assert_eq!(resolve_key("DICTATE"), Some(Key::KEY_DICTATE));
        assert_eq!(resolve_key("KEY_DICTATE"), Some(Key::KEY_DICTATE));
        assert_eq!(Key::KEY_VOICECOMMAND.raw(), 0x246);
        assert_eq!(Key::KEY_ASSISTANT.raw(), 0x247);
        assert_eq!(Key::KEY_DICTATE.raw(), 0x24a);

        let program = parse_key_program("DICTATE").unwrap();
        assert_eq!(program.press, vec![Key::KEY_DICTATE]);
        assert_eq!(program.release, vec![Key::KEY_DICTATE]);
    }

    #[test]
    fn single_key_program_presses_and_releases_the_same_key() {
        let program = KeyProgram::single(Key::KEY_RIGHTALT);
        assert_eq!(program.press, vec![Key::KEY_RIGHTALT]);
        assert_eq!(program.release, vec![Key::KEY_RIGHTALT]);
    }

    #[test]
    fn parses_single_keys_and_chords() {
        let single = parse_key_program("F24").unwrap();
        assert_eq!(single.press, vec![Key::KEY_F24]);
        assert_eq!(single.release, vec![Key::KEY_F24]);

        let chord = parse_key_program("ctrl+alt+v").unwrap();
        assert_eq!(
            chord.press,
            vec![Key::KEY_LEFTCTRL, Key::KEY_LEFTALT, Key::KEY_V]
        );
        assert_eq!(
            chord.release,
            vec![Key::KEY_V, Key::KEY_LEFTALT, Key::KEY_LEFTCTRL]
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
        assert_eq!(tap.press, vec![Key::KEY_ENTER]);
        assert_eq!(tap.release, vec![Key::KEY_ENTER]);

        let combo = resolver
            .program(&Action::Combo {
                mods: vec!["ctrl".into(), "alt".into()],
                key: "delete".into(),
            })
            .unwrap();
        assert_eq!(
            combo.press,
            vec![Key::KEY_LEFTCTRL, Key::KEY_LEFTALT, Key::KEY_DELETE]
        );
        assert_eq!(
            combo.release,
            vec![Key::KEY_DELETE, Key::KEY_LEFTALT, Key::KEY_LEFTCTRL]
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
        assert!(codes.contains(&Key::KEY_ENTER));
        assert!(codes.contains(&Key::KEY_GRAVE));
        assert!(codes.contains(&Key::KEY_HOMEPAGE));
        assert!(!codes.contains(&Key::KEY_POWER));
    }
}
