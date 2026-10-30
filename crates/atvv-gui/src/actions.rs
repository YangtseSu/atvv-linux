// SPDX-License-Identifier: GPL-3.0-or-later

//! Action editor state: parse a grammar string into editable fields and back.

use atvv_config::parse_checked_action;
use atvv_input::{Action, DictationMode, parse_action};

/// Editable action kind, in the order shown by the type selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    None,
    Key,
    Combo,
    Exec,
    Dictation,
}

impl ActionKind {
    pub const ALL: [Self; 5] = [
        Self::None,
        Self::Key,
        Self::Combo,
        Self::Exec,
        Self::Dictation,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::None => "Do nothing",
            Self::Key => "Single key",
            Self::Combo => "Key combination",
            Self::Exec => "Run a command",
            Self::Dictation => "Dictation",
        }
    }

    #[must_use]
    pub const fn from_index(index: u32) -> Self {
        match index {
            1 => Self::Key,
            2 => Self::Combo,
            3 => Self::Exec,
            4 => Self::Dictation,
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
    pub const fn label(self) -> &'static str {
        match self {
            Self::Start => "Start recording",
            Self::Stop => "Stop recording",
            Self::Toggle => "Toggle recording",
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
}

impl Default for ActionDraft {
    fn default() -> Self {
        Self {
            kind: ActionKind::None,
            key: String::new(),
            chord: String::new(),
            command: String::new(),
            dictation: DictationChoice::Toggle,
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
        ] {
            assert_eq!(ActionDraft::from_raw(raw).to_raw(), raw);
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
