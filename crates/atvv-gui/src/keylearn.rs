// SPDX-License-Identifier: GPL-3.0-or-later

//! Keyboard key learning for the GUI.
//!
//! GTK reports keyval names (`V`, `Control_L`, `Page_Up`); the action grammar
//! uses kernel/alias names (`v`, `ctrl`, `pageup`). The mapping is pure so it
//! can be tested without a display.

use atvv_input::resolve_key;

/// Names GTK uses for modifier keys. Pressing one alone does not produce an
/// action; the next non-modifier key carries the modifier state.
pub fn is_modifier_name(name: &str) -> bool {
    matches!(
        name,
        "Control_L"
            | "Control_R"
            | "Shift_L"
            | "Shift_R"
            | "Alt_L"
            | "Alt_R"
            | "Meta_L"
            | "Meta_R"
            | "Super_L"
            | "Super_R"
            | "Hyper_L"
            | "Hyper_R"
            | "Caps_Lock"
            | "Num_Lock"
    )
}

/// Normalize a GTK key name to a name the action grammar resolves.
fn canonical_key(name: &str) -> Option<String> {
    let name = name.trim();
    if name.is_empty() || is_modifier_name(name) {
        return None;
    }
    let candidate = if name.chars().count() == 1
        && name.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
    {
        name.to_ascii_lowercase()
    } else {
        name.to_string()
    };
    resolve_key(&candidate)?;
    Some(candidate)
}

/// Build a `key:`/`combo:` action from one GTK key event.
///
/// Modifiers are recorded in a stable order (`ctrl`, `alt`, `shift`, `super`)
/// so the same chord always produces the same string.
#[must_use]
pub fn learn_action(
    key_name: &str,
    ctrl: bool,
    alt: bool,
    shift: bool,
    super_key: bool,
) -> Option<String> {
    let key = canonical_key(key_name)?;
    let mut mods = Vec::new();
    if ctrl {
        mods.push("ctrl");
    }
    if alt {
        mods.push("alt");
    }
    if shift {
        mods.push("shift");
    }
    if super_key {
        mods.push("super");
    }
    if mods.is_empty() {
        Some(format!("key:{key}"))
    } else {
        Some(format!("combo:{}+{key}", mods.join("+")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learns_single_keys_and_chords() {
        assert_eq!(
            learn_action("v", false, false, false, false).as_deref(),
            Some("key:v")
        );
        assert_eq!(
            learn_action("F13", true, true, false, false).as_deref(),
            Some("combo:ctrl+alt+F13")
        );
        assert_eq!(
            learn_action("Return", false, false, false, true).as_deref(),
            Some("combo:super+Return")
        );
        assert_eq!(
            learn_action("Page_Up", false, false, false, false).as_deref(),
            Some("key:Page_Up")
        );
        assert_eq!(
            learn_action("slash", false, false, false, false).as_deref(),
            Some("key:slash")
        );
    }

    #[test]
    fn shifted_letters_become_explicit_chords() {
        assert_eq!(
            learn_action("V", false, false, true, false).as_deref(),
            Some("combo:shift+v")
        );
    }

    #[test]
    fn modifier_only_and_unknown_keys_are_ignored() {
        assert_eq!(learn_action("Control_L", true, false, false, false), None);
        assert_eq!(learn_action("Shift_R", false, false, true, false), None);
        assert_eq!(learn_action("Dead_acute", false, false, false, false), None);
        assert_eq!(learn_action("", false, false, false, false), None);
    }
}
