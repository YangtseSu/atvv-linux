// SPDX-License-Identifier: GPL-3.0-or-later

//! UI-neutral configuration state.
//!
//! The model owns the editable document and the selected slot. It never talks
//! to GTK or D-Bus; the window renders it and sends it through the worker.

use std::collections::{BTreeMap, BTreeSet};

use atvv_config::{ConfigDocument, parse_checked_action};
use atvv_input::{ActionMap, Trigger};
use atvv_protocol::profile::{DeviceProfile, PROFILES, RemoteButton};

use crate::i18n::tr;

/// Resolve a profile id from the daemon document.
#[must_use]
pub fn profile_by_id(id: &str) -> Option<DeviceProfile> {
    PROFILES.iter().copied().find(|profile| profile.id == id)
}

/// Human label for a hotspot and the selected-key row.
#[must_use]
pub fn button_label(button: RemoteButton) -> &'static str {
    match button {
        RemoteButton::Power => tr("Power"),
        RemoteButton::Ok => tr("OK"),
        RemoteButton::Back => tr("Back"),
        RemoteButton::Up => tr("Up"),
        RemoteButton::Down => tr("Down"),
        RemoteButton::Left => tr("Left"),
        RemoteButton::Right => tr("Right"),
        RemoteButton::Home => tr("Home"),
        RemoteButton::Menu => tr("Menu"),
        RemoteButton::Tv => tr("TV"),
        RemoteButton::VolumeUp => tr("Volume up"),
        RemoteButton::VolumeDown => tr("Volume down"),
        RemoteButton::Voice => tr("Voice"),
    }
}

/// One slot in the document for a logical button.
fn slot_ref(document: &ConfigDocument, button: RemoteButton, trigger: Trigger) -> Option<&str> {
    let slots = document.buttons.get(button.name())?;
    match trigger {
        Trigger::Single => slots.single.as_deref(),
        Trigger::Double => slots.double.as_deref(),
        Trigger::Long => slots.long.as_deref(),
    }
}

fn slot_mut(
    document: &mut ConfigDocument,
    button: RemoteButton,
    trigger: Trigger,
) -> &mut Option<String> {
    let slots = document
        .buttons
        .entry(button.name().to_string())
        .or_default();
    match trigger {
        Trigger::Single => &mut slots.single,
        Trigger::Double => &mut slots.double,
        Trigger::Long => &mut slots.long,
    }
}

/// Editable configuration and the current selection.
#[derive(Debug, Clone)]
pub struct GuiModel {
    pub profile: DeviceProfile,
    pub document: ConfigDocument,
    pub selected: RemoteButton,
    pub trigger: Trigger,
    pub dirty: bool,
    defaults: ActionMap,
}

impl GuiModel {
    pub fn new(document: ConfigDocument) -> Result<Self, String> {
        let profile = profile_by_id(&document.profile)
            .ok_or_else(|| format!("unknown device profile {:?}", document.profile))?;
        Ok(Self {
            profile,
            document,
            selected: RemoteButton::Ok,
            trigger: Trigger::Single,
            dirty: false,
            defaults: ActionMap::defaults(),
        })
    }

    /// Raw override stored for one slot; `None` means the profile default.
    #[must_use]
    pub fn slot(&self, button: RemoteButton, trigger: Trigger) -> Option<&str> {
        slot_ref(&self.document, button, trigger)
    }

    /// Profile default for one slot, if the profile defines one.
    #[must_use]
    pub fn profile_default(&self, button: RemoteButton, trigger: Trigger) -> Option<String> {
        self.defaults
            .resolve(button, trigger)
            .map(ToString::to_string)
    }

    /// Raw slot or profile default, as the runtime would resolve it.
    #[must_use]
    pub fn effective_slot(&self, button: RemoteButton, trigger: Trigger) -> Option<String> {
        self.slot(button, trigger)
            .map(str::to_string)
            .or_else(|| self.profile_default(button, trigger))
    }

    /// Selected slot value for the editor.
    #[must_use]
    pub fn selected_raw(&self) -> Option<&str> {
        self.slot(self.selected, self.trigger)
    }

    /// Selected effective value; used for the "profile default" hint.
    #[must_use]
    pub fn selected_effective(&self) -> Option<String> {
        self.effective_slot(self.selected, self.trigger)
    }

    /// Replace the selected slot. `None` restores the profile default.
    ///
    /// Returns whether the document changed.
    pub fn set_slot_raw(&mut self, raw: Option<String>) -> Result<bool, String> {
        if let Some(value) = raw.as_deref() {
            parse_checked_action(value, self.document.allow_exec)
                .map_err(|error| error.to_string())?;
        }
        let before = self.selected_raw().map(str::to_string);
        *slot_mut(&mut self.document, self.selected, self.trigger) = raw.clone();
        let changed = before != raw;
        if changed {
            self.dirty = true;
        }
        Ok(changed)
    }

    /// The document was persisted; clear the dirty flag.
    pub fn mark_saved(&mut self) {
        self.dirty = false;
    }

    /// Change the synthetic key emitted while the recording key is held.
    ///
    /// `None`, an empty string, or `none` disables the trigger. The physical
    /// recording key stays `KEY_F5` and is never part of the action map.
    pub fn set_trigger_key(&mut self, raw: Option<String>) -> Result<bool, String> {
        let value = match raw.as_deref().map(str::trim) {
            None | Some("") => None,
            Some(name) if name.eq_ignore_ascii_case("none") => None,
            Some(name) => Some(name.to_string()),
        };
        if self.document.trigger_key == value {
            return Ok(false);
        }
        let mut next = self.document.clone();
        next.trigger_key = value;
        next.validate(&self.profile)
            .map_err(|error| error.to_string())?;
        self.document.trigger_key = next.trigger_key;
        self.dirty = true;
        Ok(true)
    }

    /// Set the global `allow_exec` policy.
    pub fn set_allow_exec(&mut self, allow: bool) {
        if self.document.allow_exec != allow {
            self.document.allow_exec = allow;
            self.dirty = true;
        }
    }

    /// Validate the complete document before `SetConfig`.
    pub fn validate(&self) -> Result<(), String> {
        self.document
            .validate(&self.profile)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// A saved document arrived: replace it and clear the dirty flag.
    pub fn replace_saved(&mut self, document: ConfigDocument) -> Result<(), String> {
        let selected = self.selected;
        let trigger = self.trigger;
        *self = Self::new(document)?;
        self.selected = selected;
        self.trigger = trigger;
        Ok(())
    }

    /// Any of the three slots emits an action.
    #[must_use]
    pub fn is_bound(&self, button: RemoteButton) -> bool {
        [Trigger::Single, Trigger::Double, Trigger::Long]
            .into_iter()
            .filter_map(|trigger| self.effective_slot(button, trigger))
            .any(|value| value != "none")
    }

    /// Buttons whose effective action collides with another button's in the
    /// same gesture slot. Two remotes keys sending the same shortcut are
    /// usually a mistake.
    #[must_use]
    pub fn conflicts(&self) -> BTreeSet<RemoteButton> {
        let mut conflicts = BTreeSet::new();
        for trigger in [Trigger::Single, Trigger::Double, Trigger::Long] {
            let mut actions: BTreeMap<String, Vec<RemoteButton>> = BTreeMap::new();
            for button in RemoteButton::LOGICAL {
                if let Some(action) = self.effective_slot(button, trigger)
                    && action != "none"
                {
                    actions.entry(action).or_default().push(button);
                }
            }
            for buttons in actions.values() {
                if buttons.len() > 1 {
                    conflicts.extend(buttons.iter().copied());
                }
            }
        }
        conflicts
    }

    /// The action test mode would run for a `ButtonEvent`.
    #[must_use]
    pub fn action_for_event(&self, button: RemoteButton, trigger: &str) -> Option<String> {
        let trigger = match trigger {
            "single" => Trigger::Single,
            "double" => Trigger::Double,
            "long" => Trigger::Long,
            _ => return None,
        };
        self.effective_slot(button, trigger)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atvv_config::{Config, SCHEMA_VERSION};

    fn document() -> ConfigDocument {
        ConfigDocument::from_config(&Config::defaults(&atvv_protocol::RC003))
    }

    fn model() -> GuiModel {
        GuiModel::new(document()).unwrap()
    }

    #[test]
    fn defaults_are_bound_except_power() {
        let model = model();
        assert_eq!(model.selected, RemoteButton::Ok);
        assert_eq!(model.trigger, Trigger::Single);
        assert!(model.is_bound(RemoteButton::Ok));
        assert!(!model.is_bound(RemoteButton::Power));
        assert!(model.conflicts().is_empty());
        assert_eq!(
            model
                .effective_slot(RemoteButton::Ok, Trigger::Single)
                .as_deref(),
            Some("key:KEY_ENTER")
        );
        assert_eq!(
            model.effective_slot(RemoteButton::Up, Trigger::Double),
            None
        );
    }

    #[test]
    fn editing_a_slot_marks_dirty_and_keeps_the_document_valid() {
        let mut model = model();
        model.selected = RemoteButton::Up;
        assert!(model.set_slot_raw(Some("combo:ctrl+alt+v".into())).unwrap());
        assert!(model.dirty);
        assert_eq!(model.selected_raw(), Some("combo:ctrl+alt+v"));
        assert!(model.validate().is_ok());

        assert!(!model.set_slot_raw(Some("combo:ctrl+alt+v".into())).unwrap());
        assert_eq!(
            model.slot(RemoteButton::Up, Trigger::Double),
            None,
            "the change lands in the selected slot only"
        );
    }

    #[test]
    fn invalid_actions_are_rejected_without_touching_the_document() {
        let mut model = model();
        model.selected = RemoteButton::Up;
        let before = model.document.clone();
        let error = model
            .set_slot_raw(Some("key:not-a-key".into()))
            .unwrap_err();
        assert!(error.contains("not-a-key"));
        assert_eq!(model.document, before);
        assert!(!model.dirty);
    }

    #[test]
    fn resetting_a_slot_falls_back_to_the_profile_default() {
        let mut model = model();
        model.selected = RemoteButton::Up;
        model.set_slot_raw(Some("key:KEY_F13".into())).unwrap();
        assert_eq!(
            model
                .effective_slot(RemoteButton::Up, Trigger::Single)
                .as_deref(),
            Some("key:KEY_F13")
        );
        model.set_slot_raw(None).unwrap();
        assert_eq!(model.selected_raw(), None);
        assert_eq!(
            model
                .effective_slot(RemoteButton::Up, Trigger::Single)
                .as_deref(),
            Some("key:KEY_UP")
        );
        assert!(model.validate().is_ok());
    }

    #[test]
    fn colliding_actions_are_reported_per_trigger() {
        let mut model = model();
        model.selected = RemoteButton::Up;
        model.set_slot_raw(Some("key:KEY_F13".into())).unwrap();
        model.selected = RemoteButton::Down;
        model.set_slot_raw(Some("key:KEY_F13".into())).unwrap();
        let conflicts = model.conflicts();
        assert!(conflicts.contains(&RemoteButton::Up));
        assert!(conflicts.contains(&RemoteButton::Down));
        assert!(!conflicts.contains(&RemoteButton::Ok));
    }

    #[test]
    fn trigger_key_can_change_and_reset() {
        let mut model = model();
        assert_eq!(model.document.trigger_key.as_deref(), Some("DICTATE"));
        assert!(model.set_trigger_key(Some("F23".into())).unwrap());
        assert_eq!(model.document.trigger_key.as_deref(), Some("F23"));
        assert!(model.dirty);
        assert!(model.validate().is_ok());

        let error = model.set_trigger_key(Some("not-a-key".into())).unwrap_err();
        assert!(error.contains("not-a-key"));
        assert_eq!(model.document.trigger_key.as_deref(), Some("F23"));

        assert!(model.set_trigger_key(Some("none".into())).unwrap());
        assert_eq!(model.document.trigger_key, None);
        assert!(model.validate().is_ok());
    }

    #[test]
    fn exec_needs_the_global_policy() {
        let mut model = model();
        assert!(model.set_slot_raw(Some("exec:true".into())).is_err());
        model.set_allow_exec(true);
        assert!(model.set_slot_raw(Some("exec:true".into())).is_ok());
        assert!(model.validate().is_ok());
    }

    #[test]
    fn replace_saved_keeps_the_selection() {
        let mut model = model();
        model.selected = RemoteButton::Menu;
        model.set_slot_raw(Some("key:KEY_F13".into())).unwrap();
        let mut saved = document();
        saved.buttons.get_mut("menu").unwrap().single = Some("key:KEY_F13".into());
        model.replace_saved(saved).unwrap();
        assert_eq!(model.selected, RemoteButton::Menu);
        assert!(!model.dirty);
        assert_eq!(model.document.schema_version, SCHEMA_VERSION);
    }
}
