// SPDX-License-Identifier: GPL-3.0-or-later

//! Action editor state: parse a grammar string into editable fields and back.

use atvv_config::parse_checked_action;
use atvv_input::{Action, DictationMode, PointerAction, PointerAxis, PointerButton, parse_action};

use crate::i18n::tr;

/// Editable action kind, in the order shown by the type selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    None,
    Key,
    Combo,
    Exec,
    Dictation,
    Pointer,
}

impl ActionKind {
    pub const ALL: [Self; 6] = [
        Self::None,
        Self::Key,
        Self::Combo,
        Self::Exec,
        Self::Dictation,
        Self::Pointer,
    ];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::None => tr("Do nothing"),
            Self::Key => tr("Single key"),
            Self::Combo => tr("Key combination"),
            Self::Exec => tr("Run a command"),
            Self::Dictation => tr("Dictation"),
            Self::Pointer => tr("Pointer"),
        }
    }

    #[must_use]
    pub const fn from_index(index: u32) -> Self {
        match index {
            1 => Self::Key,
            2 => Self::Combo,
            3 => Self::Exec,
            4 => Self::Dictation,
            5 => Self::Pointer,
            _ => Self::None,
        }
    }

    #[must_use]
    pub const fn index(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Key => 1,
            Self::Combo => 2,
            Self::Exec => 3,
            Self::Dictation => 4,
            Self::Pointer => 5,
        }
    }

    #[must_use]
    pub const fn default_value(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Key => "F13",
            Self::Combo => "ctrl+alt+v",
            Self::Exec => "",
            Self::Dictation => "",
            Self::Pointer => "pointer:click:left",
        }
    }
}

/// Pointer sub-type shown by the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerKind {
    Click,
    Move,
}

impl PointerKind {
    pub const ALL: [Self; 2] = [Self::Click, Self::Move];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Click => tr("Click"),
            Self::Move => tr("Move"),
        }
    }

    #[must_use]
    pub const fn from_index(index: u32) -> Self {
        match index {
            1 => Self::Move,
            _ => Self::Click,
        }
    }

    #[must_use]
    pub const fn index(self) -> u32 {
        match self {
            Self::Click => 0,
            Self::Move => 1,
        }
    }
}

/// Every pointer button, in selector order.
pub const POINTER_BUTTONS: [PointerButton; 8] = PointerButton::ALL;
/// Every pointer direction, in selector order.
pub const POINTER_AXES: [PointerAxis; 4] = PointerAxis::ALL;

/// Localized label for one pointer button.
#[must_use]
pub fn pointer_button_label(button: PointerButton) -> &'static str {
    match button {
        PointerButton::Left => tr("Left click"),
        PointerButton::Right => tr("Right click"),
        PointerButton::Middle => tr("Middle click"),
        PointerButton::Side => tr("Side button"),
        PointerButton::Extra => tr("Extra button"),
        PointerButton::Forward => tr("Forward button"),
        PointerButton::Back => tr("Back button"),
        PointerButton::Task => tr("Task button"),
    }
}

/// Localized label for one pointer direction.
#[must_use]
pub fn pointer_axis_label(axis: PointerAxis) -> &'static str {
    match axis {
        PointerAxis::Up => tr("Up"),
        PointerAxis::Down => tr("Down"),
        PointerAxis::Left => tr("Left"),
        PointerAxis::Right => tr("Right"),
    }
}

/// Selector index of a pointer button.
#[must_use]
pub fn pointer_button_index(button: PointerButton) -> u32 {
    POINTER_BUTTONS
        .iter()
        .position(|candidate| *candidate == button)
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or(0)
}

/// Selector index of a pointer direction.
#[must_use]
pub fn pointer_axis_index(axis: PointerAxis) -> u32 {
    POINTER_AXES
        .iter()
        .position(|candidate| *candidate == axis)
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or(0)
}

/// Board label for a pointer action, without the grammar prefix.
#[must_use]
pub fn pointer_action_label(action: PointerAction) -> String {
    match action {
        PointerAction::Click(button) => pointer_button_label(button).to_string(),
        PointerAction::Move { direction, units } => {
            let label = match direction {
                PointerAxis::Up => tr("Pointer up"),
                PointerAxis::Down => tr("Pointer down"),
                PointerAxis::Left => tr("Pointer left"),
                PointerAxis::Right => tr("Pointer right"),
            };
            format!("{label} {units}")
        }
    }
}

/// Dictation direction shown by the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DictationChoice {
    Start,
    Stop,
    Toggle,
}

impl DictationChoice {
    pub const ALL: [Self; 3] = [Self::Start, Self::Stop, Self::Toggle];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Start => tr("Start recording"),
            Self::Stop => tr("Stop recording"),
            Self::Toggle => tr("Toggle recording"),
        }
    }

    #[must_use]
    pub const fn from_index(index: u32) -> Self {
        match index {
            1 => Self::Stop,
            2 => Self::Toggle,
            _ => Self::Start,
        }
    }

    #[must_use]
    pub const fn index(self) -> u32 {
        match self {
            Self::Start => 0,
            Self::Stop => 1,
            Self::Toggle => 2,
        }
    }

    #[must_use]
    pub const fn to_raw(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Toggle => "toggle",
        }
    }
}

/// One editable slot value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionDraft {
    pub kind: ActionKind,
    pub key: String,
    pub chord: String,
    pub command: String,
    pub dictation: DictationChoice,
    pub pointer: PointerAction,
}

impl Default for ActionDraft {
    fn default() -> Self {
        Self {
            kind: ActionKind::None,
            key: String::new(),
            chord: String::new(),
            command: String::new(),
            dictation: DictationChoice::Toggle,
            pointer: PointerAction::Click(PointerButton::Left),
        }
    }
}

impl ActionDraft {
    /// Build editor fields from a grammar string.
    ///
    /// Configuration documents are validated before they reach the GUI; an
    /// unknown string is kept in the key field so the editor can show it and
    /// the validation error instead of silently replacing it.
    #[must_use]
    pub fn from_raw(raw: &str) -> Self {
        let mut draft = Self::default();
        match parse_action(raw) {
            Ok(Action::None) => draft.kind = ActionKind::None,
            Ok(Action::Key { name }) => {
                draft.kind = ActionKind::Key;
                draft.key = name;
            }
            Ok(Action::Combo { mods, key }) => {
                draft.kind = ActionKind::Combo;
                draft.chord = format!("{}+{key}", mods.join("+"));
            }
            Ok(Action::Exec { command }) => {
                draft.kind = ActionKind::Exec;
                draft.command = command;
            }
            Ok(Action::Dictation(mode)) => {
                draft.kind = ActionKind::Dictation;
                draft.dictation = match mode {
                    DictationMode::Start => DictationChoice::Start,
                    DictationMode::Stop => DictationChoice::Stop,
                    DictationMode::Toggle => DictationChoice::Toggle,
                };
            }
            Ok(Action::Pointer(pointer)) => {
                draft.kind = ActionKind::Pointer;
                draft.pointer = pointer;
            }
            Err(_) => {
                draft.kind = ActionKind::Key;
                draft.key = raw.trim().to_string();
            }
        }
        draft
    }

    #[must_use]
    pub fn to_raw(&self) -> String {
        match self.kind {
            ActionKind::None => "none".to_string(),
            ActionKind::Key => format!("key:{}", self.key.trim()),
            ActionKind::Combo => format!("combo:{}", self.chord.trim()),
            ActionKind::Exec => format!("exec:{}", self.command),
            ActionKind::Dictation => {
                format!("dictation:{}", self.dictation.to_raw())
            }
            ActionKind::Pointer => self.pointer.to_string(),
        }
    }

    /// Validate the composed grammar string, including the `exec` policy.
    pub fn validate(&self, allow_exec: bool) -> Result<(), String> {
        let raw = self.to_raw();
        parse_checked_action(&raw, allow_exec)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Fill the value for the active kind from a learned action string.
    pub fn apply_learned(&mut self, learned: &str) -> bool {
        let parsed = Self::from_raw(learned);
        match parsed.kind {
            ActionKind::Key => {
                self.kind = ActionKind::Key;
                self.key = parsed.key;
                true
            }
            ActionKind::Combo => {
                self.kind = ActionKind::Combo;
                self.chord = parsed.chord;
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_action_kind() {
        for raw in [
            "none",
            "key:KEY_F13",
            "combo:ctrl+alt+v",
            "exec:notify-send hello",
            "dictation:start",
            "dictation:stop",
            "dictation:toggle",
            "pointer:click:left",
            "pointer:click:task",
            "pointer:move:up:1",
            "pointer:move:left:255",
        ] {
            assert_eq!(ActionDraft::from_raw(raw).to_raw(), raw);
        }
    }

    #[test]
    fn pointer_draft_keeps_the_button_and_units() {
        let click = ActionDraft::from_raw("pointer:click:back");
        assert_eq!(click.kind, ActionKind::Pointer);
        assert_eq!(click.pointer, PointerAction::Click(PointerButton::Back));

        let move_draft = ActionDraft::from_raw("pointer:move:down:42");
        assert_eq!(
            move_draft.pointer,
            PointerAction::Move {
                direction: PointerAxis::Down,
                units: 42,
            }
        );
        assert!(move_draft.validate(false).is_ok());
        assert!(
            ActionDraft::from_raw("pointer:move:down:256")
                .validate(false)
                .is_err()
        );
    }

    #[test]
    fn kind_indices_round_trip() {
        for kind in ActionKind::ALL {
            assert_eq!(ActionKind::from_index(kind.index()), kind);
        }
        for kind in PointerKind::ALL {
            assert_eq!(PointerKind::from_index(kind.index()), kind);
        }
    }

    #[test]
    fn keeps_an_unknown_value_visible_for_validation() {
        let draft = ActionDraft::from_raw("key:not-a-key");
        assert_eq!(draft.kind, ActionKind::Key);
        assert_eq!(draft.key, "not-a-key");
        assert!(draft.validate(false).is_err());
    }

    #[test]
    fn validation_reports_the_action_problem() {
        let mut draft = ActionDraft {
            kind: ActionKind::Key,
            key: String::new(),
            ..ActionDraft::default()
        };
        assert!(draft.validate(false).is_err());

        draft.key = "KEY_F13".into();
        assert!(draft.validate(false).is_ok());

        let exec = ActionDraft {
            kind: ActionKind::Exec,
            command: "true".into(),
            ..ActionDraft::default()
        };
        assert!(exec.validate(false).unwrap_err().contains("allow_exec"));
        assert!(exec.validate(true).is_ok());

        let combo = ActionDraft {
            kind: ActionKind::Combo,
            chord: "ctrl+ctrl+v".into(),
            ..ActionDraft::default()
        };
        assert!(combo.validate(false).unwrap_err().contains("duplicate"));
    }

    #[test]
    fn learned_actions_replace_the_matching_value() {
        let mut draft = ActionDraft::default();
        assert!(draft.apply_learned("key:KEY_F13"));
        assert_eq!(draft.kind, ActionKind::Key);
        assert_eq!(draft.key, "KEY_F13");
        assert!(draft.apply_learned("combo:ctrl+alt+v"));
        assert_eq!(draft.kind, ActionKind::Combo);
        assert_eq!(draft.chord, "ctrl+alt+v");
        assert!(!draft.apply_learned("exec:true"));
    }
}
