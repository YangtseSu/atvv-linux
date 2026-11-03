// SPDX-License-Identifier: GPL-3.0-or-later

//! UI-neutral configuration state.
//!
//! The model owns the editable document and the selected slot. It never talks
//! to GTK or D-Bus; the window renders it and sends it through the worker.

use std::collections::{BTreeMap, BTreeSet};

use atvv_config::{
    ConfigDocument, DEFAULT_LAYOUT, DEFAULT_LAYOUT_NAME, LayoutDocument, LayoutPreset,
    LayoutSummary, MAX_LAYOUT_NAME, parse_checked_action, slug_layout_id, unique_layout_id,
    unique_layout_name,
};
use atvv_input::{Action, ActionMap, Trigger, parse_action};
use atvv_protocol::profile::{DeviceProfile, RemoteButton};

use crate::actions::pointer_action_label;
use crate::i18n::{tr, tr_args};

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
        RemoteButton::Mute => tr("Mute"),
        RemoteButton::YouTube => tr("YouTube"),
        RemoteButton::Netflix => tr("Netflix"),
        RemoteButton::Voice => tr("Voice"),
    }
}

/// Localized button label for a device profile. A profile may name a physical
/// key differently from its logical slot (Chromecast: the `tv` slot is the
/// Input key).
#[must_use]
pub fn button_label_for(profile: &DeviceProfile, button: RemoteButton) -> &'static str {
    profile
        .label(button)
        .map_or_else(|| button_label(button), tr)
}

/// Human label for a gesture slot.
#[must_use]
pub fn trigger_label(trigger: Trigger) -> &'static str {
    match trigger {
        Trigger::Single => tr("Single"),
        Trigger::Double => tr("Double"),
        Trigger::Long => tr("Long"),
    }
}

/// Normalize an optional raw document value: empty or `none` clears it.
fn normalise_value(raw: Option<String>) -> Option<String> {
    match raw.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(value) if value.eq_ignore_ascii_case("none") => None,
        Some(value) => Some(value.to_string()),
    }
}

/// Parse the protocol name used by `ButtonEvent`.
#[must_use]
pub fn trigger_from_name(name: &str) -> Option<Trigger> {
    match name {
        "single" => Some(Trigger::Single),
        "double" => Some(Trigger::Double),
        "long" => Some(Trigger::Long),
        _ => None,
    }
}

/// Compact form of one slot value, as shown by the board.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionLabel {
    pub text: String,
    /// The slot swallows the button; the card renders it dimmed.
    pub none: bool,
}

fn none_label() -> ActionLabel {
    ActionLabel {
        text: tr("None").to_string(),
        none: true,
    }
}

/// Render `raw` the way the board shows it: `Return`, `F13`,
/// `Ctrl+Alt+V`, `Command…`, `Start dictation`, or `None`.
///
/// An unparseable value is returned unchanged so a hand-edited document stays
/// visible instead of silently reading as unbound.
#[must_use]
pub fn action_label(raw: Option<&str>) -> ActionLabel {
    let Some(raw) = raw else {
        return none_label();
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return none_label();
    }
    match parse_action(raw) {
        Ok(Action::None) => none_label(),
        Ok(Action::Key { name }) => ActionLabel {
            text: friendly_key(&name),
            none: false,
        },
        Ok(Action::Combo { mods, key }) => {
            let mut text = mods
                .iter()
                .map(|modifier| modifier_label(modifier))
                .collect::<Vec<_>>()
                .join("+");
            text.push('+');
            text.push_str(&friendly_key(&key));
            ActionLabel { text, none: false }
        }
        Ok(Action::Exec { .. }) => ActionLabel {
            text: tr("Command…").to_string(),
            none: false,
        },
        Ok(Action::Dictation(mode)) => ActionLabel {
            text: match mode {
                atvv_input::DictationMode::Start => tr("Start dictation"),
                atvv_input::DictationMode::Stop => tr("Stop dictation"),
                atvv_input::DictationMode::Toggle => tr("Toggle dictation"),
            }
            .to_string(),
            none: false,
        },
        Ok(Action::Pointer(pointer)) => ActionLabel {
            text: pointer_action_label(pointer),
            none: false,
        },
        Err(_) => ActionLabel {
            text: raw.to_string(),
            none: false,
        },
    }
}

/// Present one modifier token in the board's chord form.
fn modifier_label(token: &str) -> String {
    match token.trim().to_ascii_lowercase().as_str() {
        "ctrl" | "control" => "Ctrl".to_string(),
        "alt" => "Alt".to_string(),
        "shift" => "Shift".to_string(),
        "super" | "meta" => "Super".to_string(),
        other => title_case(other),
    }
}

/// Display name for one evdev key name.
///
/// Known keys use their desktop label; anything else falls back to a
/// title-cased bare name, so `KEY_F13` reads `F13` and `KEY_COMMA` reads
/// `Comma`.
#[must_use]
pub fn friendly_key(name: &str) -> String {
    known_key_label(name)
        .map(|label| tr(label).to_string())
        .unwrap_or_else(|| derived_key_label(name))
}

/// English source label for a key, independent of the active UI language.
///
/// Used by the key picker so an English query works in every locale.
#[must_use]
pub fn friendly_key_english(name: &str) -> String {
    known_key_label(name)
        .map(str::to_string)
        .unwrap_or_else(|| derived_key_label(name))
}

/// English catalog label for a known key name.
fn known_key_label(name: &str) -> Option<&'static str> {
    let upper = name.trim().to_ascii_uppercase();
    match upper.as_str() {
        "KEY_ENTER" | "KEY_KPENTER" => Some("Return"),
        "KEY_BACK" => Some("Back"),
        "KEY_BACKSPACE" => Some("Backspace"),
        "KEY_DELETE" => Some("Delete"),
        "KEY_UP" => Some("Up"),
        "KEY_DOWN" => Some("Down"),
        "KEY_LEFT" => Some("Left"),
        "KEY_RIGHT" => Some("Right"),
        "KEY_HOME" | "KEY_HOMEPAGE" => Some("Home"),
        "KEY_COMPOSE" | "KEY_MENU" => Some("Menu"),
        "KEY_GRAVE" => Some("TV"),
        "KEY_VOLUMEUP" => Some("Volume up"),
        "KEY_VOLUMEDOWN" => Some("Volume down"),
        "KEY_POWER" => Some("Power"),
        "KEY_SYSRQ" => Some("Print Screen"),
        "KEY_ESC" => Some("Escape"),
        "KEY_SPACE" => Some("Space"),
        "KEY_TAB" => Some("Tab"),
        "KEY_PLAYPAUSE" => Some("Play/Pause"),
        _ => None,
    }
}

/// Label derived from the kernel name for keys without a catalog entry.
fn derived_key_label(name: &str) -> String {
    let upper = name.trim().to_ascii_uppercase();
    let bare = upper.strip_prefix("KEY_").unwrap_or(&upper);
    if bare.len() > 1 && bare.starts_with('F') && bare[1..].chars().all(|c| c.is_ascii_digit()) {
        return bare.to_string();
    }
    title_case(bare)
}

fn title_case(value: &str) -> String {
    value
        .split(['_', '-', ' '])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first
                    .to_uppercase()
                    .chain(chars.flat_map(char::to_lowercase))
                    .collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

fn working_layout(document: &ConfigDocument) -> Result<LayoutDocument, String> {
    if document.active_layout == DEFAULT_LAYOUT {
        return Ok(LayoutDocument::default());
    }
    document
        .layouts
        .get(&document.active_layout)
        .cloned()
        .ok_or_else(|| format!("unknown layout {:?}", document.active_layout))
}

fn clean_description(description: Option<&str>) -> Option<String> {
    description
        .map(str::trim)
        .filter(|description| !description.is_empty())
        .map(str::to_string)
}

/// Keep the active device's registry entry in step with `active_layout`.
fn sync_device_layout(document: &mut ConfigDocument) {
    if let Some(address) = document.active_device.clone()
        && let Some(device) = document.devices.get_mut(&address)
    {
        device.layout = document.active_layout.clone();
    }
}

/// Editable configuration and the current selection.
///
/// `working` holds the raw slots of the active layout. While the `default`
/// template is active it is a staged copy: the document keeps no `default`
/// layout, and saving replaces it with a new user layout.
#[derive(Debug, Clone)]
pub struct GuiModel {
    pub profile: DeviceProfile,
    pub document: ConfigDocument,
    pub working: LayoutDocument,
    pub selected: RemoteButton,
    pub trigger: Trigger,
    pub dirty: bool,
    defaults: ActionMap,
}

impl GuiModel {
    pub fn new(document: ConfigDocument) -> Result<Self, String> {
        let profile = document
            .active_profile()
            .copied()
            .ok_or_else(|| format!("unknown device profile {:?}", document.profile))?;
        let working = working_layout(&document)?;
        Ok(Self {
            profile,
            document,
            working,
            selected: RemoteButton::Ok,
            trigger: Trigger::Single,
            dirty: false,
            defaults: ActionMap::defaults_for(&profile),
        })
    }

    /// The active layout is the synthesized, read-only `default` template.
    #[must_use]
    pub fn template(&self) -> bool {
        self.document.active_layout == DEFAULT_LAYOUT
    }

    /// Active layout id.
    #[must_use]
    pub fn active_layout(&self) -> &str {
        &self.document.active_layout
    }

    /// Localized display name for a layout id.
    #[must_use]
    pub fn layout_name(&self, id: &str) -> String {
        if id == DEFAULT_LAYOUT {
            return tr("Default").to_string();
        }
        self.document
            .layouts
            .get(id)
            .map_or_else(|| id.to_string(), |layout| layout.name.clone())
    }

    /// Selector entries: the template first, then user layouts by id.
    #[must_use]
    pub fn summaries(&self) -> Vec<LayoutSummary> {
        self.document.layout_summaries()
    }

    /// The template has staged slot edits that must be saved as a layout.
    #[must_use]
    pub fn template_changed(&self) -> bool {
        self.template() && !self.working.is_empty()
    }

    /// Raw override stored for one slot; `None` means the profile default.
    #[must_use]
    pub fn slot(&self, button: RemoteButton, trigger: Trigger) -> Option<&str> {
        self.working.slot(button, trigger)
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
    /// While the template is active this stages the change in `working`.
    /// Returns whether the document changed.
    pub fn set_slot_raw(&mut self, raw: Option<String>) -> Result<bool, String> {
        if let Some(value) = raw.as_deref() {
            parse_checked_action(value, self.document.allow_exec)
                .map_err(|error| error.to_string())?;
        }
        let before = self
            .working
            .slot(self.selected, self.trigger)
            .map(str::to_string);
        self.working
            .set_slot(self.selected, self.trigger, raw.clone());
        let changed = before != raw;
        if changed {
            self.sync_active();
            self.dirty = true;
        }
        Ok(changed)
    }

    /// Mirror the active user layout into the document.
    fn sync_active(&mut self) {
        if !self.template() {
            self.document
                .layouts
                .insert(self.document.active_layout.clone(), self.working.clone());
        }
    }

    /// The document was persisted; clear the dirty flag.
    pub fn mark_saved(&mut self) {
        self.dirty = false;
    }

    /// Switch the active layout. Dirty documents must be applied or reverted
    /// first so a staged template edit is never dropped.
    pub fn switch_layout(&mut self, id: &str) -> Result<bool, String> {
        if self.dirty {
            return Err(tr("Apply or revert before switching layouts.").to_string());
        }
        if id == self.document.active_layout {
            return Ok(false);
        }
        if id != DEFAULT_LAYOUT && !self.document.layouts.contains_key(id) {
            return Err(format!("unknown layout {id:?}"));
        }
        self.document.active_layout = id.to_string();
        sync_device_layout(&mut self.document);
        self.working = working_layout(&self.document)?;
        Ok(true)
    }

    /// Create a user layout from `default` or another layout without switching.
    pub fn create_layout(
        &mut self,
        name: &str,
        description: Option<&str>,
        from: Option<&str>,
    ) -> Result<String, String> {
        let name = name.trim();
        self.check_name(name, None)?;
        let mut next = self.document.clone();
        let source = match from {
            None | Some(DEFAULT_LAYOUT) => LayoutDocument::default(),
            Some(from) => next
                .layouts
                .get(from)
                .cloned()
                .ok_or_else(|| format!("unknown layout {from:?}"))?,
        };
        let id = unique_layout_id(
            &slug_layout_id(name),
            next.layouts.keys().map(String::as_str),
        );
        next.layouts.insert(
            id.clone(),
            LayoutDocument {
                name: name.to_string(),
                description: clean_description(description),
                gesture: source.gesture,
                buttons: source.buttons,
            },
        );
        next.validate(&self.profile)
            .map_err(|error| error.to_string())?;
        self.document = next;
        self.dirty = true;
        Ok(id)
    }

    /// Create a user layout from a built-in preset without switching to it.
    pub fn create_layout_from_preset(
        &mut self,
        preset: &LayoutPreset,
        name: &str,
    ) -> Result<String, String> {
        let name = name.trim();
        self.check_name(name, None)?;
        let mut next = self.document.clone();
        let mut layout = preset.layout_for(&self.profile);
        layout.name = name.to_string();
        let id = unique_layout_id(
            &slug_layout_id(name),
            next.layouts.keys().map(String::as_str),
        );
        next.layouts.insert(id.clone(), layout);
        next.validate(&self.profile)
            .map_err(|error| error.to_string())?;
        self.document = next;
        self.dirty = true;
        Ok(id)
    }

    /// Persist the staged template slots as a new active layout.
    pub fn save_as_layout(
        &mut self,
        name: &str,
        description: Option<&str>,
    ) -> Result<String, String> {
        let name = name.trim();
        self.check_name(name, None)?;
        let mut next = self.document.clone();
        let id = unique_layout_id(
            &slug_layout_id(name),
            next.layouts.keys().map(String::as_str),
        );
        let mut layout = self.working.clone();
        layout.name = name.to_string();
        layout.description = clean_description(description);
        next.layouts.insert(id.clone(), layout);
        next.active_layout = id.clone();
        sync_device_layout(&mut next);
        next.validate(&self.profile)
            .map_err(|error| error.to_string())?;
        self.working = working_layout(&next)?;
        self.document = next;
        self.dirty = true;
        Ok(id)
    }

    /// Rename a user layout; `None` keeps the current description.
    pub fn rename_layout(
        &mut self,
        id: &str,
        name: &str,
        description: Option<&str>,
    ) -> Result<(), String> {
        if id == DEFAULT_LAYOUT {
            return Err(tr("Default is a template.").to_string());
        }
        let name = name.trim();
        self.check_name(name, Some(id))?;
        let mut next = self.document.clone();
        {
            let layout = next
                .layouts
                .get_mut(id)
                .ok_or_else(|| format!("unknown layout {id:?}"))?;
            layout.name = name.to_string();
            if let Some(description) = description {
                layout.description = clean_description(Some(description));
            }
        }
        next.validate(&self.profile)
            .map_err(|error| error.to_string())?;
        self.document = next;
        if self.document.active_layout == id {
            self.working = working_layout(&self.document)?;
        }
        self.dirty = true;
        Ok(())
    }

    /// Delete a user layout; the template and the active layout stay.
    pub fn delete_layout(&mut self, id: &str) -> Result<(), String> {
        if id == DEFAULT_LAYOUT {
            return Err(tr("Default is a template.").to_string());
        }
        if id == self.document.active_layout {
            return Err(tr("Cannot delete the active layout.").to_string());
        }
        let mut next = self.document.clone();
        if next.layouts.remove(id).is_none() {
            return Err(format!("unknown layout {id:?}"));
        }
        next.validate(&self.profile)
            .map_err(|error| error.to_string())?;
        self.document = next;
        self.dirty = true;
        Ok(())
    }

    /// Clear a user layout's slots and gesture timing.
    pub fn reset_layout(&mut self, id: &str) -> Result<(), String> {
        if id == DEFAULT_LAYOUT {
            return Err(tr("Default is a template.").to_string());
        }
        let mut next = self.document.clone();
        {
            let layout = next
                .layouts
                .get_mut(id)
                .ok_or_else(|| format!("unknown layout {id:?}"))?;
            layout.buttons.clear();
            layout.gesture = None;
        }
        next.validate(&self.profile)
            .map_err(|error| error.to_string())?;
        self.document = next;
        if self.document.active_layout == id {
            self.working = working_layout(&self.document)?;
        }
        self.dirty = true;
        Ok(())
    }

    /// Replace one user layout's slots with a built-in preset.
    ///
    /// The layout keeps its name and description; the active layout does not
    /// change.
    pub fn reset_layout_to_preset(
        &mut self,
        id: &str,
        preset: &LayoutPreset,
    ) -> Result<(), String> {
        if id == DEFAULT_LAYOUT {
            return Err(tr("Default is a template.").to_string());
        }
        let mut next = self.document.clone();
        {
            let layout = next
                .layouts
                .get_mut(id)
                .ok_or_else(|| format!("unknown layout {id:?}"))?;
            let name = layout.name.clone();
            let description = layout.description.clone();
            let mut replacement = preset.layout_for(&self.profile);
            replacement.name = name;
            replacement.description = description;
            *layout = replacement;
        }
        next.validate(&self.profile)
            .map_err(|error| error.to_string())?;
        self.document = next;
        if self.document.active_layout == id {
            self.working = working_layout(&self.document)?;
        }
        self.dirty = true;
        Ok(())
    }

    /// Copy a layout under a new name and id; the copy stays inactive.
    ///
    /// Duplicating the template copies the profile baseline, not staged edits.
    pub fn duplicate_layout(&mut self, id: &str) -> Result<String, String> {
        let source = if id == DEFAULT_LAYOUT {
            LayoutDocument::default()
        } else {
            self.document
                .layouts
                .get(id)
                .cloned()
                .ok_or_else(|| format!("unknown layout {id:?}"))?
        };
        let base = if id == DEFAULT_LAYOUT {
            DEFAULT_LAYOUT_NAME.to_string()
        } else {
            source.name.clone()
        };
        let existing: Vec<String> = self
            .document
            .layouts
            .values()
            .map(|layout| layout.name.clone())
            .collect();
        let name = unique_layout_name(&format!("{base} copy"), existing.iter().map(String::as_str));
        let mut next = self.document.clone();
        let new_id = unique_layout_id(
            &slug_layout_id(&name),
            next.layouts.keys().map(String::as_str),
        );
        next.layouts.insert(
            new_id.clone(),
            LayoutDocument {
                name,
                description: source.description,
                gesture: source.gesture,
                buttons: source.buttons,
            },
        );
        next.validate(&self.profile)
            .map_err(|error| error.to_string())?;
        self.document = next;
        self.dirty = true;
        Ok(new_id)
    }

    /// Reject empty, overlong, reserved, and duplicate names before saving.
    fn check_name(&self, name: &str, exclude: Option<&str>) -> Result<(), String> {
        if name.is_empty() {
            return Err(tr("Enter a layout name.").to_string());
        }
        if name.chars().count() > MAX_LAYOUT_NAME {
            return Err(tr("Use 40 characters or fewer.").to_string());
        }
        if name.eq_ignore_ascii_case(DEFAULT_LAYOUT_NAME) {
            return Err(tr("That name is reserved or already used.").to_string());
        }
        if self.document.layouts.iter().any(|(id, layout)| {
            Some(id.as_str()) != exclude && layout.name.eq_ignore_ascii_case(name)
        }) {
            return Err(tr("That name is reserved or already used.").to_string());
        }
        Ok(())
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

    /// Apply the recording-key page atomically: output key and edge actions.
    ///
    /// `None`, an empty string, or `none` clears a field. Edge actions use the
    /// button grammar without dictation. Validation runs on a copy, so a
    /// rejected value leaves the document untouched.
    pub fn set_recording_edges(
        &mut self,
        trigger: Option<String>,
        press: Option<String>,
        release: Option<String>,
    ) -> Result<bool, String> {
        let trigger = normalise_value(trigger);
        let press = normalise_value(press);
        let release = normalise_value(release);
        if self.document.trigger_key == trigger
            && self.document.recording.press == press
            && self.document.recording.release == release
        {
            return Ok(false);
        }
        let mut next = self.document.clone();
        next.trigger_key = trigger;
        next.recording.press = press;
        next.recording.release = release;
        next.validate(&self.profile)
            .map_err(|error| error.to_string())?;
        self.document = next;
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

    /// Set the post-decode gain in dB.
    ///
    /// The existing Apply path persists the whole document; this never touches
    /// the active layout.
    pub fn set_gain_db(&mut self, gain_db: i8) -> Result<bool, String> {
        if !(-atvv_config::MAX_GAIN_DB..=atvv_config::MAX_GAIN_DB).contains(&gain_db) {
            let max = atvv_config::MAX_GAIN_DB.to_string();
            return Err(tr_args(
                "Gain must be between -{max} and {max} dB.",
                &[("max", &max)],
            ));
        }
        if self.document.gain_db == gain_db {
            return Ok(false);
        }
        self.document.gain_db = gain_db;
        self.dirty = true;
        Ok(true)
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
            for &button in self.profile.buttons {
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
    fn chromecast_model_uses_its_own_buttons_and_defaults() {
        let mut model = GuiModel::new(ConfigDocument::from_config(&Config::defaults(
            &atvv_protocol::CHROMECAST,
        )))
        .unwrap();
        for button in [
            RemoteButton::Mute,
            RemoteButton::YouTube,
            RemoteButton::Netflix,
        ] {
            assert!(model.profile.has_button(button), "{button}");
        }
        assert!(!model.profile.has_button(RemoteButton::Menu));
        assert_eq!(model.profile.label(RemoteButton::Tv), Some("Input"));
        assert_eq!(
            model
                .effective_slot(RemoteButton::Mute, Trigger::Single)
                .as_deref(),
            Some("key:KEY_MUTE")
        );
        assert_eq!(
            model
                .effective_slot(RemoteButton::YouTube, Trigger::Single)
                .as_deref(),
            Some("key:KEY_CAMERA_ACCESS_DISABLE")
        );
        assert_eq!(
            model
                .effective_slot(RemoteButton::Netflix, Trigger::Single)
                .as_deref(),
            Some("key:KEY_CAMERA_ACCESS_TOGGLE")
        );
        assert_eq!(
            model
                .effective_slot(RemoteButton::Tv, Trigger::Single)
                .as_deref(),
            Some("key:KEY_TV")
        );
        assert_eq!(
            model.effective_slot(RemoteButton::Menu, Trigger::Single),
            None
        );

        let preset = atvv_config::layout_preset("mpv").expect("mpv preset");
        let id = model
            .create_layout_from_preset(preset, "movie")
            .expect("create");
        let layout = &model.document.layouts[&id];
        assert!(layout.slot(RemoteButton::Menu, Trigger::Single).is_none());
        assert!(layout.slot(RemoteButton::Mute, Trigger::Single).is_none());
        assert_eq!(
            layout.slot(RemoteButton::Tv, Trigger::Single),
            Some("key:KEY_F")
        );
    }

    #[test]
    fn editing_a_slot_marks_dirty_and_keeps_the_document_valid() {
        let mut model = model();
        model.selected = RemoteButton::Up;
        assert!(model.set_slot_raw(Some("combo:ctrl+alt+v".into())).unwrap());
        assert!(model.dirty);
        assert!(model.template(), "a fresh document starts on Default");
        assert!(model.template_changed());
        assert!(
            model.document.layouts.is_empty(),
            "the template is never written into the document"
        );
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
    fn gain_is_validated_and_marks_the_document_dirty() {
        let mut model = model();
        assert_eq!(model.document.gain_db, 0);
        assert!(model.set_gain_db(6).unwrap());
        assert_eq!(model.document.gain_db, 6);
        assert!(model.dirty);
        assert!(!model.set_gain_db(6).unwrap());
        assert!(model.validate().is_ok());

        let error = model.set_gain_db(25).unwrap_err();
        assert!(error.contains("24"), "{error}");
        assert_eq!(model.document.gain_db, 6);
        let error = model.set_gain_db(-25).unwrap_err();
        assert!(error.contains("24"), "{error}");
        assert_eq!(model.document.gain_db, 6);

        assert!(model.set_gain_db(-24).unwrap());
        assert!(model.validate().is_ok());
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
    fn recording_edges_validate_and_clear() {
        let mut model = model();
        assert!(!model.dirty);
        assert!(
            !model
                .set_recording_edges(Some("DICTATE".into()), None, None)
                .unwrap(),
            "an unchanged page is not dirty"
        );

        assert!(
            model
                .set_recording_edges(
                    Some("F23".into()),
                    Some("key:KEY_F13".into()),
                    Some("combo:ctrl+alt+v".into()),
                )
                .unwrap()
        );
        assert_eq!(model.document.trigger_key.as_deref(), Some("F23"));
        assert_eq!(
            model.document.recording.press.as_deref(),
            Some("key:KEY_F13")
        );
        assert_eq!(
            model.document.recording.release.as_deref(),
            Some("combo:ctrl+alt+v")
        );
        assert!(model.validate().is_ok());

        let error = model
            .set_recording_edges(None, Some("dictation:start".into()), None)
            .unwrap_err();
        assert!(error.contains("dictation"), "{error}");
        assert_eq!(
            model.document.recording.press.as_deref(),
            Some("key:KEY_F13"),
            "a rejected edge leaves the whole page untouched"
        );

        let error = model
            .set_recording_edges(None, Some("exec:true".into()), None)
            .unwrap_err();
        assert!(error.contains("allow_exec"), "{error}");
        assert_eq!(
            model.document.recording.press.as_deref(),
            Some("key:KEY_F13")
        );

        assert!(
            model
                .set_recording_edges(None, Some("none".into()), Some(String::new()))
                .unwrap()
        );
        assert_eq!(model.document.trigger_key, None);
        assert!(model.document.recording.is_empty());
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
        let mut layout = LayoutDocument {
            name: "Coding".to_string(),
            ..LayoutDocument::default()
        };
        layout.set_slot(
            RemoteButton::Menu,
            Trigger::Single,
            Some("key:KEY_F13".into()),
        );
        saved.layouts.insert("coding-layout".to_string(), layout);
        saved.active_layout = "coding-layout".to_string();
        model.replace_saved(saved).unwrap();
        assert_eq!(model.selected, RemoteButton::Menu);
        assert!(!model.dirty);
        assert_eq!(model.active_layout(), "coding-layout");
        assert_eq!(
            model.slot(RemoteButton::Menu, Trigger::Single),
            Some("key:KEY_F13")
        );
        assert_eq!(model.document.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn template_edits_are_saved_as_a_new_layout() {
        let mut model = model();
        model.selected = RemoteButton::Up;
        model.set_slot_raw(Some("key:KEY_F13".into())).unwrap();
        assert!(model.template_changed());
        let id = model
            .save_as_layout("Coding", Some("Editor shortcuts"))
            .unwrap();
        assert_eq!(id, "coding", "the layout id is derived from the name");
        assert!(!model.template());
        assert!(!model.template_changed());
        assert_eq!(model.active_layout(), id);
        assert_eq!(model.document.layouts[&id].name, "Coding");
        assert_eq!(
            model.document.layouts[&id].slot(RemoteButton::Up, Trigger::Single),
            Some("key:KEY_F13")
        );
        assert_eq!(
            model.effective_slot(RemoteButton::Up, Trigger::Single),
            Some("key:KEY_F13".to_string())
        );
        assert!(model.dirty, "the new layout still needs Apply");
        assert!(model.validate().is_ok());
    }

    #[test]
    fn switching_layouts_is_blocked_while_dirty() {
        let _guard = crate::i18n::language_guard();
        crate::i18n::Language::set(crate::i18n::Language::English);
        let mut model = model();
        let id = model.create_layout("mpv", None, None).unwrap();
        assert_eq!(id, "mpv-layout");
        model.mark_saved();
        assert!(model.switch_layout(&id).unwrap());
        assert_eq!(model.active_layout(), id);
        assert!(!model.template());

        model.selected = RemoteButton::Ok;
        model.set_slot_raw(Some("key:KEY_F13".into())).unwrap();
        let error = model.switch_layout(DEFAULT_LAYOUT).unwrap_err();
        assert!(error.contains("Apply or revert"), "{error}");
        assert_eq!(model.active_layout(), id);

        model.mark_saved();
        assert!(model.switch_layout(DEFAULT_LAYOUT).unwrap());
        assert!(model.template());
        assert!(!model.switch_layout(DEFAULT_LAYOUT).unwrap());
    }

    #[test]
    fn manager_creates_renames_duplicates_deletes_and_resets() {
        let mut model = model();
        let id = model.create_layout("Media", None, None).unwrap();
        assert!(model.document.layouts.contains_key(&id));
        assert!(model.dirty);

        model
            .rename_layout(&id, "Playback", Some("Player shortcuts"))
            .unwrap();
        assert_eq!(model.document.layouts[&id].name, "Playback");
        assert_eq!(
            model.document.layouts[&id].description.as_deref(),
            Some("Player shortcuts")
        );

        let copy = model.duplicate_layout(&id).unwrap();
        assert_eq!(model.document.layouts[&copy].name, "Playback copy");
        assert!(model.document.layouts[&copy].is_empty());

        let template_copy = model.duplicate_layout(DEFAULT_LAYOUT).unwrap();
        assert_eq!(model.document.layouts[&template_copy].name, "Default copy");

        assert!(model.delete_layout(&id).is_ok());
        assert!(!model.document.layouts.contains_key(&id));

        model.mark_saved();
        model.switch_layout(&copy).unwrap();
        model.selected = RemoteButton::Up;
        model.set_slot_raw(Some("key:KEY_F13".into())).unwrap();
        model.mark_saved();
        model.reset_layout(&copy).unwrap();
        assert!(model.working.is_empty());
        assert_eq!(
            model.effective_slot(RemoteButton::Up, Trigger::Single),
            Some("key:KEY_UP".to_string())
        );
        assert!(model.dirty);
    }

    #[test]
    fn the_template_cannot_be_renamed_deleted_or_reset() {
        let mut model = model();
        assert!(model.rename_layout(DEFAULT_LAYOUT, "Mine", None).is_err());
        assert!(model.delete_layout(DEFAULT_LAYOUT).is_err());
        assert!(model.reset_layout(DEFAULT_LAYOUT).is_err());
        assert!(model.switch_layout("missing").is_err());
    }

    #[test]
    fn duplicate_names_and_reserved_names_are_rejected() {
        let _guard = crate::i18n::language_guard();
        crate::i18n::Language::set(crate::i18n::Language::English);
        let mut model = model();
        model.create_layout("Media", None, None).unwrap();
        let error = model.create_layout("media", None, None).unwrap_err();
        assert!(error.contains("reserved or already used"), "{error}");
        let error = model.create_layout("Default", None, None).unwrap_err();
        assert!(error.contains("reserved or already used"), "{error}");
        let error = model
            .create_layout("x".repeat(41).as_str(), None, None)
            .unwrap_err();
        assert!(error.contains("40"), "{error}");
    }

    #[test]
    fn slot_labels_are_compact_and_readable() {
        let _guard = crate::i18n::language_guard();
        crate::i18n::Language::set(crate::i18n::Language::English);
        assert_eq!(action_label(None).text, "None");
        assert!(action_label(None).none);
        assert_eq!(action_label(Some("none")).text, "None");
        assert_eq!(action_label(Some("key:KEY_ENTER")).text, "Return");
        assert_eq!(action_label(Some("key:KEY_HOMEPAGE")).text, "Home");
        assert_eq!(action_label(Some("key:KEY_COMPOSE")).text, "Menu");
        assert_eq!(action_label(Some("key:KEY_F13")).text, "F13");
        assert_eq!(action_label(Some("key:KEY_COMMA")).text, "Comma");
        assert_eq!(action_label(Some("combo:ctrl+alt+v")).text, "Ctrl+Alt+V");
        assert_eq!(action_label(Some("combo:shift+F11")).text, "Shift+F11");
        assert_eq!(action_label(Some("exec:notify-send hi")).text, "Command…");
        assert_eq!(
            action_label(Some("dictation:start")).text,
            "Start dictation"
        );
        assert_eq!(action_label(Some("dictation:stop")).text, "Stop dictation");
        assert_eq!(
            action_label(Some("dictation:toggle")).text,
            "Toggle dictation"
        );
        assert_eq!(action_label(Some("pointer:click:left")).text, "Left click");
        assert_eq!(action_label(Some("pointer:click:back")).text, "Back button");
        assert_eq!(action_label(Some("pointer:move:up:1")).text, "Pointer up 1");
        assert_eq!(
            action_label(Some("pointer:move:right:24")).text,
            "Pointer right 24"
        );
        let unknown = action_label(Some("bogus-value"));
        assert_eq!(unknown.text, "bogus-value");
        assert!(!unknown.none, "invalid values stay visible, not dimmed");
    }

    #[test]
    fn english_key_labels_ignore_the_active_language() {
        let _guard = crate::i18n::language_guard();
        crate::i18n::Language::set(crate::i18n::Language::Chinese);
        assert_eq!(friendly_key("KEY_SYSRQ"), "打印屏幕");
        assert_eq!(friendly_key_english("KEY_SYSRQ"), "Print Screen");
        crate::i18n::Language::set(crate::i18n::Language::English);
        assert_eq!(friendly_key("KEY_SYSRQ"), "Print Screen");
        crate::i18n::Language::set(crate::i18n::Language::English);
    }

    #[test]
    fn trigger_names_round_trip() {
        let _guard = crate::i18n::language_guard();
        crate::i18n::Language::set(crate::i18n::Language::English);
        for trigger in [Trigger::Single, Trigger::Double, Trigger::Long] {
            assert_eq!(trigger_from_name(trigger.name()), Some(trigger));
        }
        assert_eq!(trigger_from_name("release"), None);
        assert_eq!(trigger_label(Trigger::Double), "Double");
    }

    #[test]
    fn preset_creation_stages_an_inactive_layout() {
        let mut model = model();
        let preset = atvv_config::layout_preset("omp").expect("omp preset");
        let id = model
            .create_layout_from_preset(preset, "omp")
            .expect("create");
        assert_eq!(id, "omp-layout");
        assert_eq!(model.active_layout(), DEFAULT_LAYOUT);
        assert!(model.dirty);
        assert_eq!(
            model.document.layouts[&id].slot(RemoteButton::Power, Trigger::Single),
            Some("combo:alt+r")
        );
        assert_eq!(
            model.document.layouts[&id].slot(RemoteButton::Tv, Trigger::Single),
            Some("combo:ctrl+o")
        );
        assert_eq!(
            model.document.layouts[&id].slot(RemoteButton::VolumeUp, Trigger::Single),
            Some("key:KEY_VOLUMEUP")
        );
    }

    #[test]
    fn preset_reset_keeps_the_layout_name_and_active_state() {
        let mut model = model();
        let preset = atvv_config::layout_preset("mpv").expect("mpv preset");
        let id = model
            .create_layout_from_preset(preset, "movie")
            .expect("create");
        model.document.active_layout = id.clone();
        model.working = working_layout(&model.document).expect("working");
        model.dirty = false;
        let slides = atvv_config::layout_preset("slides").expect("slides preset");
        model
            .reset_layout_to_preset(&id, slides)
            .expect("reset to preset");
        assert_eq!(model.active_layout(), id);
        assert_eq!(model.document.layouts[&id].name, "movie");
        assert_eq!(
            model.working.slot(RemoteButton::Up, Trigger::Single),
            Some("key:KEY_PAGEUP")
        );
        assert!(model.dirty);
        assert!(
            model
                .reset_layout_to_preset(DEFAULT_LAYOUT, slides)
                .is_err()
        );
    }

    #[test]
    fn active_device_layout_is_kept_in_the_registry() {
        let address = "C0:5D:39:F8:2D:4C";
        let mut document = document();
        document.active_device = Some(address.to_string());
        document.devices.insert(
            address.to_string(),
            atvv_config::DeviceDocument {
                label: None,
                profile: atvv_protocol::RC003.id.to_string(),
                layout: "editor".to_string(),
            },
        );
        document.layouts.insert(
            "editor".to_string(),
            LayoutDocument {
                name: "Editor".to_string(),
                ..LayoutDocument::default()
            },
        );
        document.active_layout = "editor".to_string();
        let mut model = GuiModel::new(document).unwrap();
        assert_eq!(model.active_layout(), "editor");
        assert_eq!(model.profile.id, atvv_protocol::RC003.id);
        assert!(model.switch_layout(DEFAULT_LAYOUT).unwrap());
        assert_eq!(
            model.document.devices[address].layout, DEFAULT_LAYOUT,
            "the active device entry must follow the layout switch"
        );
    }
}
