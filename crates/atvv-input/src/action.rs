// SPDX-License-Identifier: GPL-3.0-or-later

//! Action grammar and the per-button action map.
//!
//! Grammar:
//!
//! ```text
//! none
//! key:<name>
//! combo:<mod>+<mod>+<name>
//! exec:<command>
//! dictation:start|stop|toggle
//! ```
//!
//! Key names are resolved by [`crate::keymap::KeyResolver`]. The map never
//! emits anything itself; the runtime compiles key actions into uinput frames
//! and forwards `exec`/`dictation` actions to the daemon.

use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;

use atvv_protocol::profile::{GestureTiming, RemoteButton};
use thiserror::Error;

use crate::gesture::Policy;

/// Gesture slot an action is bound to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    Single,
    Double,
    Long,
}

impl Trigger {
    /// Stable configuration name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Double => "double",
            Self::Long => "long",
        }
    }
}

/// Dictation control carried to the daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DictationMode {
    Start,
    Stop,
    Toggle,
}

/// One mapped action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Swallow the button.
    None,
    /// Tap one key.
    Key { name: String },
    /// Hold modifiers while tapping a key.
    Combo { mods: Vec<String>, key: String },
    /// Run a command through the daemon, never inside the input thread.
    Exec { command: String },
    /// Drive the ATVV dictation path.
    Dictation(DictationMode),
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => f.write_str("none"),
            Self::Key { name } => write!(f, "key:{name}"),
            Self::Combo { mods, key } => {
                write!(f, "combo:")?;
                for modifier in mods {
                    write!(f, "{modifier}+")?;
                }
                f.write_str(key)
            }
            Self::Exec { command } => write!(f, "exec:{command}"),
            Self::Dictation(DictationMode::Start) => f.write_str("dictation:start"),
            Self::Dictation(DictationMode::Stop) => f.write_str("dictation:stop"),
            Self::Dictation(DictationMode::Toggle) => f.write_str("dictation:toggle"),
        }
    }
}

/// Parse errors for action strings and key names.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ActionError {
    #[error("empty action")]
    Empty,
    #[error("unknown action kind: {0}")]
    UnknownKind(String),
    #[error("missing key name")]
    NoKey,
    #[error("empty modifier in combo")]
    EmptyModifier,
    #[error("bad dictation mode: {0}")]
    BadDictation(String),
    #[error("voice key is not action-mappable")]
    VoiceNotMappable,
    #[error("unknown key name: {0}")]
    UnknownKey(String),
    #[error("action carries no key program")]
    NotEmittable,
}

fn key_or_combo(rest: &str) -> Result<Action, ActionError> {
    let tokens: Vec<&str> = rest.split('+').collect();
    if tokens.iter().all(|token| token.trim().is_empty()) {
        return Err(ActionError::NoKey);
    }
    if tokens.iter().any(|token| token.trim().is_empty()) {
        return Err(ActionError::EmptyModifier);
    }
    let mut mods: Vec<String> = tokens
        .iter()
        .map(|token| token.trim().to_string())
        .collect();
    let key = mods.pop().ok_or(ActionError::NoKey)?;
    if mods.is_empty() {
        Ok(Action::Key { name: key })
    } else {
        Ok(Action::Combo { mods, key })
    }
}

/// Parse one action string.
pub fn parse_action(input: &str) -> Result<Action, ActionError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(ActionError::Empty);
    }
    if input.eq_ignore_ascii_case("none") {
        return Ok(Action::None);
    }
    let Some((kind, rest)) = input.split_once(':') else {
        return Err(ActionError::UnknownKind(input.to_string()));
    };
    if kind.eq_ignore_ascii_case("exec") {
        let command = rest.trim_start();
        if command.is_empty() {
            return Err(ActionError::Empty);
        }
        return Ok(Action::Exec {
            command: command.to_string(),
        });
    }
    if kind.eq_ignore_ascii_case("dictation") {
        return match rest.trim().to_ascii_lowercase().as_str() {
            "start" => Ok(Action::Dictation(DictationMode::Start)),
            "stop" => Ok(Action::Dictation(DictationMode::Stop)),
            "toggle" => Ok(Action::Dictation(DictationMode::Toggle)),
            other => Err(ActionError::BadDictation(other.to_string())),
        };
    }
    if kind.eq_ignore_ascii_case("key") || kind.eq_ignore_ascii_case("combo") {
        return key_or_combo(rest);
    }
    Err(ActionError::UnknownKind(kind.to_string()))
}

/// Slots bound for one button.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ButtonActions {
    pub single: Option<Action>,
    pub double: Option<Action>,
    pub long: Option<Action>,
}

impl ButtonActions {
    fn get(&self, trigger: Trigger) -> Option<&Action> {
        match trigger {
            Trigger::Single => self.single.as_ref(),
            Trigger::Double => self.double.as_ref(),
            Trigger::Long => self.long.as_ref(),
        }
    }

    fn set(&mut self, trigger: Trigger, action: Action) {
        match trigger {
            Trigger::Single => self.single = Some(action),
            Trigger::Double => self.double = Some(action),
            Trigger::Long => self.long = Some(action),
        }
    }
}

/// Effective action slots for every logical button.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionMap {
    slots: BTreeMap<RemoteButton, ButtonActions>,
}

impl Default for ActionMap {
    fn default() -> Self {
        Self::defaults()
    }
}

impl ActionMap {
    /// Pass-through defaults: each key is re-emitted unchanged so a grabbed
    /// button still reaches the compositor.
    ///
    /// Power defaults to `none`: emitting `KEY_POWER` would race logind's
    /// `HandlePowerKey`, and the desktop policy belongs to the daemon config
    /// (step 06). No desktop-specific shortcuts are baked in.
    #[must_use]
    pub fn defaults() -> Self {
        let mut map = Self {
            slots: BTreeMap::new(),
        };
        map.insert(RemoteButton::Power, Trigger::Single, Action::None);
        for (button, key) in [
            (RemoteButton::Ok, "KEY_ENTER"),
            (RemoteButton::Back, "KEY_BACK"),
            (RemoteButton::Up, "KEY_UP"),
            (RemoteButton::Down, "KEY_DOWN"),
            (RemoteButton::Left, "KEY_LEFT"),
            (RemoteButton::Right, "KEY_RIGHT"),
            (RemoteButton::Home, "KEY_HOMEPAGE"),
            (RemoteButton::Menu, "KEY_MENU"),
            (RemoteButton::Tv, "KEY_GRAVE"),
            (RemoteButton::VolumeUp, "KEY_VOLUMEUP"),
            (RemoteButton::VolumeDown, "KEY_VOLUMEDOWN"),
        ] {
            map.insert(
                button,
                Trigger::Single,
                Action::Key {
                    name: key.to_string(),
                },
            );
        }
        map
    }

    fn insert(&mut self, button: RemoteButton, trigger: Trigger, action: Action) {
        self.slots.entry(button).or_default().set(trigger, action);
    }

    /// Bind an action. The voice key is rejected: it drives ATVV.
    pub fn set(
        &mut self,
        button: RemoteButton,
        trigger: Trigger,
        action: Action,
    ) -> Result<(), ActionError> {
        if button == RemoteButton::Voice {
            return Err(ActionError::VoiceNotMappable);
        }
        self.insert(button, trigger, action);
        Ok(())
    }

    /// Resolve `(button, trigger)`. `Some(Action::None)` means swallowed.
    #[must_use]
    pub fn resolve(&self, button: RemoteButton, trigger: Trigger) -> Option<&Action> {
        self.slots.get(&button).and_then(|slots| slots.get(trigger))
    }

    /// Every action in the map, for keycode validation.
    pub fn actions(&self) -> impl Iterator<Item = &Action> {
        self.slots
            .values()
            .flat_map(|slots| [&slots.single, &slots.double, &slots.long])
            .flatten()
    }

    fn configured(&self, button: RemoteButton, trigger: Trigger) -> bool {
        self.resolve(button, trigger)
            .is_some_and(|action| *action != Action::None)
    }

    /// Gesture policy implied by the configured slots.
    #[must_use]
    pub fn policy(&self, button: RemoteButton, timing: &GestureTiming) -> Policy {
        let double_click = self.configured(button, Trigger::Double);
        let long_press = self.configured(button, Trigger::Long);
        let plain_key = match self.resolve(button, Trigger::Single) {
            Some(Action::Key { .. }) => true,
            Some(Action::Combo { mods, .. }) => mods.is_empty(),
            _ => false,
        };
        let repeatable = matches!(
            button,
            RemoteButton::Back
                | RemoteButton::Up
                | RemoteButton::Down
                | RemoteButton::Left
                | RemoteButton::Right
                | RemoteButton::VolumeUp
                | RemoteButton::VolumeDown
        );
        let repeat = if repeatable && !double_click && !long_press && plain_key {
            let interval = if button == RemoteButton::Back {
                timing.back_repeat_interval_ms
            } else {
                timing.repeat_interval_ms
            };
            Some(Duration::from_millis(u64::from(interval)))
        } else {
            None
        };
        Policy {
            double_click,
            long_press,
            repeat,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(input: &str) -> Action {
        parse_action(input).unwrap()
    }

    #[test]
    fn parses_the_documented_grammar() {
        assert_eq!(action("none"), Action::None);
        assert_eq!(
            action(" KEY:KEY_F5 "),
            Action::Key {
                name: "KEY_F5".into()
            }
        );
        assert_eq!(
            action("combo:ctrl+alt+KEY_DELETE"),
            Action::Combo {
                mods: vec!["ctrl".into(), "alt".into()],
                key: "KEY_DELETE".into(),
            }
        );
        assert_eq!(
            action("exec:pactl a+b"),
            Action::Exec {
                command: "pactl a+b".into()
            }
        );
        assert_eq!(
            action("dictation:TOGGLE"),
            Action::Dictation(DictationMode::Toggle)
        );
    }

    #[test]
    fn rejects_bad_actions() {
        assert_eq!(parse_action("  "), Err(ActionError::Empty));
        assert_eq!(
            parse_action("foo:bar"),
            Err(ActionError::UnknownKind("foo".into()))
        );
        assert_eq!(parse_action("combo:"), Err(ActionError::NoKey));
        assert_eq!(
            parse_action("combo:ctrl++x"),
            Err(ActionError::EmptyModifier)
        );
        assert_eq!(
            parse_action("dictation:sideways"),
            Err(ActionError::BadDictation("sideways".into()))
        );
        assert!(parse_action("return").is_err());
    }

    #[test]
    fn display_round_trips() {
        for input in [
            "none",
            "key:KEY_F5",
            "combo:ctrl+alt+KEY_DELETE",
            "exec:pactl a+b",
            "dictation:start",
            "dictation:stop",
            "dictation:toggle",
        ] {
            assert_eq!(parse_action(input).unwrap().to_string(), input);
        }
    }

    #[test]
    fn defaults_cover_all_twelve_buttons() {
        let map = ActionMap::defaults();
        for button in RemoteButton::LOGICAL {
            assert!(
                map.resolve(button, Trigger::Single).is_some(),
                "{button} missing"
            );
            assert!(!map.configured(button, Trigger::Double));
            assert!(!map.configured(button, Trigger::Long));
        }
        assert_eq!(
            map.resolve(RemoteButton::Power, Trigger::Single),
            Some(&Action::None)
        );
        assert_eq!(map.resolve(RemoteButton::Voice, Trigger::Single), None);
    }

    #[test]
    fn repeat_policy_matches_the_remote() {
        let map = ActionMap::defaults();
        let timing = atvv_protocol::RC003.gesture_timing;
        assert_eq!(
            map.policy(RemoteButton::Back, &timing).repeat,
            Some(Duration::from_millis(50))
        );
        assert_eq!(
            map.policy(RemoteButton::Up, &timing).repeat,
            Some(Duration::from_millis(100))
        );
        assert_eq!(
            map.policy(RemoteButton::VolumeDown, &timing).repeat,
            Some(Duration::from_millis(100))
        );
        assert_eq!(map.policy(RemoteButton::Ok, &timing).repeat, None);
        assert_eq!(map.policy(RemoteButton::Power, &timing).repeat, None);
    }

    #[test]
    fn secondary_slots_disable_repeat() {
        let timing = atvv_protocol::RC003.gesture_timing;
        let mut map = ActionMap::defaults();
        map.set(
            RemoteButton::VolumeUp,
            Trigger::Long,
            Action::Key {
                name: "KEY_MUTE".into(),
            },
        )
        .unwrap();
        let policy = map.policy(RemoteButton::VolumeUp, &timing);
        assert!(policy.long_press);
        assert_eq!(policy.repeat, None);

        map.set(
            RemoteButton::Down,
            Trigger::Double,
            Action::Key {
                name: "KEY_END".into(),
            },
        )
        .unwrap();
        let policy = map.policy(RemoteButton::Down, &timing);
        assert!(policy.double_click);
        assert_eq!(policy.repeat, None);
    }

    #[test]
    fn voice_and_combos_do_not_repeat() {
        let timing = atvv_protocol::RC003.gesture_timing;
        let mut map = ActionMap::defaults();
        assert_eq!(
            map.set(
                RemoteButton::Voice,
                Trigger::Single,
                Action::Key {
                    name: "KEY_F5".into()
                }
            ),
            Err(ActionError::VoiceNotMappable)
        );
        map.set(
            RemoteButton::Up,
            Trigger::Single,
            Action::Combo {
                mods: vec!["ctrl".into()],
                key: "KEY_UP".into(),
            },
        )
        .unwrap();
        assert_eq!(map.policy(RemoteButton::Up, &timing).repeat, None);
    }
}
