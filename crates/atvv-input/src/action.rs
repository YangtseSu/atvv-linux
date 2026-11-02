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
//! pointer:click:left|right|middle|side|extra|forward|back|task
//! pointer:move:up|down|left|right:<units>
//! ```
//!
//! Key names are resolved by [`crate::keymap::KeyResolver`]. The map never
//! emits anything itself; the runtime compiles key actions into uinput frames,
//! pointer actions into events on a dedicated pointer device, and forwards
//! `exec`/`dictation` actions to the daemon.

use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;

use atvv_protocol::profile::{DeviceProfile, GestureTiming, RemoteButton};
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

/// Pointer button emitted by `pointer:click:<button>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
    Side,
    Extra,
    Forward,
    Back,
    Task,
}

impl PointerButton {
    /// Every supported button, in configuration and UI order.
    pub const ALL: [Self; 8] = [
        Self::Left,
        Self::Right,
        Self::Middle,
        Self::Side,
        Self::Extra,
        Self::Forward,
        Self::Back,
        Self::Task,
    ];

    /// Stable configuration name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Middle => "middle",
            Self::Side => "side",
            Self::Extra => "extra",
            Self::Forward => "forward",
            Self::Back => "back",
            Self::Task => "task",
        }
    }

    /// Parse a configuration name, case-insensitively.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        let name = name.trim();
        Self::ALL
            .into_iter()
            .find(|button| button.name().eq_ignore_ascii_case(name))
    }
}

/// Relative pointer direction for `pointer:move:<direction>:<units>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerAxis {
    Up,
    Down,
    Left,
    Right,
}

impl PointerAxis {
    /// Every direction, in configuration and UI order.
    pub const ALL: [Self; 4] = [Self::Up, Self::Down, Self::Left, Self::Right];

    /// Stable configuration name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::Left => "left",
            Self::Right => "right",
        }
    }

    /// Parse a configuration name, case-insensitively.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        let name = name.trim();
        Self::ALL
            .into_iter()
            .find(|axis| axis.name().eq_ignore_ascii_case(name))
    }
}

/// One pointer action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerAction {
    /// Press and release one pointer button.
    Click(PointerButton),
    /// Move the pointer by `units` relative counts in `direction`.
    Move { direction: PointerAxis, units: u16 },
}

impl PointerAction {
    /// Smallest accepted relative movement.
    pub const MIN_UNITS: u16 = 1;
    /// Largest accepted relative movement.
    pub const MAX_UNITS: u16 = 255;
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
    /// Emit a pointer button or relative motion.
    Pointer(PointerAction),
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
            Self::Pointer(pointer) => pointer.fmt(f),
        }
    }
}

impl fmt::Display for PointerAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Click(button) => write!(f, "pointer:click:{}", button.name()),
            Self::Move { direction, units } => {
                write!(f, "pointer:move:{}:{units}", direction.name())
            }
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
    #[error(
        "{0} is a pointer button; use pointer:click:<left|right|middle|side|extra|forward|back|task>"
    )]
    PointerKey(String),
    #[error("bad pointer action: {0}")]
    BadPointer(String),
    #[error("unknown pointer button: {0}")]
    UnknownPointerButton(String),
    #[error("unknown pointer direction: {0}")]
    UnknownPointerDirection(String),
    #[error("pointer move units must be 1-255: {0}")]
    PointerUnits(String),
}

fn key_or_combo(rest: &str) -> Result<Action, ActionError> {
    let tokens: Vec<&str> = rest.split('+').collect();
    if tokens.iter().all(|token| token.trim().is_empty()) {
        return Err(ActionError::NoKey);
    }
    if tokens.iter().any(|token| token.trim().is_empty()) {
        return Err(ActionError::EmptyModifier);
    }
    if let Some(token) = tokens
        .iter()
        .map(|token| token.trim())
        .find(|token| token.to_ascii_uppercase().starts_with("BTN_"))
    {
        return Err(ActionError::PointerKey(token.to_string()));
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

fn pointer_action(rest: &str) -> Result<PointerAction, ActionError> {
    let mut parts = rest.split(':');
    let command = parts.next().unwrap_or_default().trim();
    match command.to_ascii_lowercase().as_str() {
        "click" => {
            let Some(name) = parts.next() else {
                return Err(ActionError::BadPointer("missing click button".to_string()));
            };
            if parts.next().is_some() {
                return Err(ActionError::BadPointer(rest.to_string()));
            }
            let name = name.trim();
            PointerButton::parse(name)
                .map(PointerAction::Click)
                .ok_or_else(|| ActionError::UnknownPointerButton(name.to_string()))
        }
        "move" => {
            let Some(direction) = parts.next() else {
                return Err(ActionError::BadPointer(
                    "missing move direction".to_string(),
                ));
            };
            let Some(units) = parts.next() else {
                return Err(ActionError::BadPointer("missing move units".to_string()));
            };
            if parts.next().is_some() {
                return Err(ActionError::BadPointer(rest.to_string()));
            }
            let direction = direction.trim();
            let axis = PointerAxis::parse(direction)
                .ok_or_else(|| ActionError::UnknownPointerDirection(direction.to_string()))?;
            let units = units.trim();
            let value: u16 = units
                .parse()
                .map_err(|_| ActionError::PointerUnits(units.to_string()))?;
            if !(PointerAction::MIN_UNITS..=PointerAction::MAX_UNITS).contains(&value) {
                return Err(ActionError::PointerUnits(units.to_string()));
            }
            Ok(PointerAction::Move {
                direction: axis,
                units: value,
            })
        }
        _ => Err(ActionError::BadPointer(rest.to_string())),
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
    if kind.eq_ignore_ascii_case("pointer") {
        return pointer_action(rest).map(Action::Pointer);
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

impl ActionMap {
    /// Pass-through defaults derived from a device profile: each physical
    /// button re-emits its own primary key so a grabbed button still reaches
    /// the compositor.
    ///
    /// Power defaults to `none`: emitting `KEY_POWER` would race logind's
    /// `HandlePowerKey`, and the desktop policy belongs to the daemon config
    /// (step 06). No desktop-specific shortcuts are baked in. Buttons the
    /// profile does not have get no action.
    #[must_use]
    pub fn defaults_for(profile: &DeviceProfile) -> Self {
        let mut map = Self {
            slots: BTreeMap::new(),
        };
        for button in profile.buttons {
            let action = if *button == RemoteButton::Power {
                Action::None
            } else if let Some(key) = profile.primary_key(*button) {
                Action::Key {
                    name: key.to_string(),
                }
            } else {
                continue;
            };
            map.insert(*button, Trigger::Single, action);
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
        assert_eq!(
            action("pointer:click:LEFT"),
            Action::Pointer(PointerAction::Click(PointerButton::Left))
        );
        assert_eq!(
            action("pointer:move:up:1"),
            Action::Pointer(PointerAction::Move {
                direction: PointerAxis::Up,
                units: 1,
            })
        );
        assert_eq!(
            action("pointer:move:right:255"),
            Action::Pointer(PointerAction::Move {
                direction: PointerAxis::Right,
                units: 255,
            })
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
        assert_eq!(
            parse_action("key:BTN_LEFT"),
            Err(ActionError::PointerKey("BTN_LEFT".into()))
        );
        assert!(matches!(
            parse_action("pointer:click:sideways"),
            Err(ActionError::UnknownPointerButton(_))
        ));
        assert!(matches!(
            parse_action("pointer:move:diagonal:1"),
            Err(ActionError::UnknownPointerDirection(_))
        ));
        assert_eq!(
            parse_action("pointer:move:up:0"),
            Err(ActionError::PointerUnits("0".into()))
        );
        assert_eq!(
            parse_action("pointer:move:up:256"),
            Err(ActionError::PointerUnits("256".into()))
        );
        assert!(matches!(
            parse_action("pointer:move:up"),
            Err(ActionError::BadPointer(_))
        ));
        assert!(matches!(
            parse_action("pointer:click"),
            Err(ActionError::BadPointer(_))
        ));
        assert!(matches!(
            parse_action("pointer:wheel:up"),
            Err(ActionError::BadPointer(_))
        ));
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
            "pointer:click:left",
            "pointer:click:task",
            "pointer:move:up:1",
            "pointer:move:down:10",
            "pointer:move:left:255",
            "pointer:move:right:3",
        ] {
            assert_eq!(parse_action(input).unwrap().to_string(), input);
        }
    }

    #[test]
    fn defaults_cover_the_profile_buttons() {
        let map = ActionMap::defaults_for(&atvv_protocol::RC003);
        for &button in atvv_protocol::RC003.buttons {
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
        assert_eq!(map.resolve(RemoteButton::Mute, Trigger::Single), None);

        let google = ActionMap::defaults_for(&atvv_protocol::CHROMECAST);
        for &button in atvv_protocol::CHROMECAST.buttons {
            assert!(
                google.resolve(button, Trigger::Single).is_some(),
                "{button} missing"
            );
        }
        assert_eq!(
            google.resolve(RemoteButton::Mute, Trigger::Single),
            Some(&Action::Key {
                name: "KEY_MUTE".to_string()
            })
        );
        assert_eq!(
            google.resolve(RemoteButton::YouTube, Trigger::Single),
            Some(&Action::Key {
                name: "KEY_CAMERA_ACCESS_DISABLE".to_string()
            })
        );
        assert_eq!(
            google.resolve(RemoteButton::Netflix, Trigger::Single),
            Some(&Action::Key {
                name: "KEY_CAMERA_ACCESS_TOGGLE".to_string()
            })
        );
        assert_eq!(
            google.resolve(RemoteButton::Tv, Trigger::Single),
            Some(&Action::Key {
                name: "KEY_TV".to_string()
            })
        );
        assert_eq!(google.resolve(RemoteButton::Menu, Trigger::Single), None);
    }

    #[test]
    fn repeat_policy_matches_the_remote() {
        let map = ActionMap::defaults_for(&atvv_protocol::RC003);
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
        let mut map = ActionMap::defaults_for(&atvv_protocol::RC003);
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
    fn pointer_actions_use_gesture_slots_without_keyboard_repeat() {
        let timing = atvv_protocol::RC003.gesture_timing;
        let mut map = ActionMap::defaults_for(&atvv_protocol::RC003);
        map.set(
            RemoteButton::Up,
            Trigger::Single,
            action("pointer:move:up:1"),
        )
        .unwrap();
        map.set(
            RemoteButton::Up,
            Trigger::Double,
            action("pointer:move:up:10"),
        )
        .unwrap();
        map.set(
            RemoteButton::Up,
            Trigger::Long,
            action("pointer:move:up:50"),
        )
        .unwrap();
        let policy = map.policy(RemoteButton::Up, &timing);
        assert!(policy.double_click);
        assert!(policy.long_press);
        assert_eq!(policy.repeat, None);

        let mut single = ActionMap::defaults_for(&atvv_protocol::RC003);
        single
            .set(
                RemoteButton::Up,
                Trigger::Single,
                action("pointer:move:up:1"),
            )
            .unwrap();
        assert_eq!(single.policy(RemoteButton::Up, &timing).repeat, None);
    }

    #[test]
    fn voice_and_combos_do_not_repeat() {
        let timing = atvv_protocol::RC003.gesture_timing;
        let mut map = ActionMap::defaults_for(&atvv_protocol::RC003);
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
