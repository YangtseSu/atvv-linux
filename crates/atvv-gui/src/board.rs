// SPDX-License-Identifier: GPL-3.0-or-later

//! Mapping board: one card per physical button around the profile diagram.
//!
//! The board is a view over [`GuiModel`]. It renders every action slot and
//! reports clicks through callbacks; selection and document state stay in the
//! model, so the hotspots, the cards, the editor dialog, and test mode cannot
//! drift apart.

use std::collections::BTreeMap;
use std::rc::Rc;

use atvv_input::Trigger;
use atvv_protocol::profile::RemoteButton;
use gtk::prelude::*;

use crate::diagram::RemoteDiagram;
use crate::i18n::{tr, tr_args};
use crate::model::{GuiModel, action_label, button_label, button_label_for, trigger_label};

/// Buttons shown left of the remote, top to bottom.
///
/// The two columns are the superset of every profile's physical buttons; a
/// card is hidden when the active profile has no such button.
pub const LEFT_COLUMN: [RemoteButton; 7] = [
    RemoteButton::Power,
    RemoteButton::Up,
    RemoteButton::Left,
    RemoteButton::Back,
    RemoteButton::Home,
    RemoteButton::Menu,
    RemoteButton::YouTube,
];

/// Buttons shown right of the remote, top to bottom.
pub const RIGHT_COLUMN: [RemoteButton; 8] = [
    RemoteButton::Right,
    RemoteButton::Ok,
    RemoteButton::Down,
    RemoteButton::VolumeUp,
    RemoteButton::VolumeDown,
    RemoteButton::Tv,
    RemoteButton::Mute,
    RemoteButton::Netflix,
];

/// Gesture slots every card shows, top to bottom.
pub const SLOTS: [Trigger; 3] = [Trigger::Single, Trigger::Double, Trigger::Long];

const COLUMN_WIDTH: i32 = 236;
const CENTER_MIN_WIDTH: i32 = 280;
const BOARD_MIN_WIDTH: i32 = 860;
const CARD_SPACING: i32 = 6;

fn remove_selection_classes(classed: &impl IsA<gtk::Widget>, classes: &[&str]) {
    for class in classes {
        classed.remove_css_class(class);
    }
}

/// One button card: the label plus the three gesture slots.
pub struct KeyCard {
    root: gtk::Box,
    name: gtk::Label,
    warn: gtk::Image,
    slots: [gtk::Button; 3],
    actions: [gtk::Label; 3],
}

impl KeyCard {
    /// The card container, used by tests to check section classes.
    #[must_use]
    pub fn root(&self) -> &gtk::Box {
        &self.root
    }

    /// Update the visible title after a profile change.
    pub fn set_label(&self, label: &str) {
        self.name.set_text(label);
    }

    /// Rendered card title.
    #[must_use]
    pub fn label_text(&self) -> String {
        self.name.text().to_string()
    }

    /// One gesture slot of this card.
    #[must_use]
    pub fn slot(&self, trigger: Trigger) -> Option<&gtk::Button> {
        let index = SLOTS.iter().position(|slot| *slot == trigger)?;
        self.slots.get(index)
    }

    /// The rendered action text of one gesture slot.
    #[must_use]
    pub fn action_text(&self, trigger: Trigger) -> Option<String> {
        let index = SLOTS.iter().position(|slot| *slot == trigger)?;
        self.actions
            .get(index)
            .map(|label| label.text().to_string())
    }

    fn new(button: RemoteButton) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 2);
        root.add_css_class("mapping-card");
        root.set_widget_name(&format!("card-{}", button.name()));

        let head = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        head.set_margin_start(4);
        head.set_margin_end(4);
        let name = gtk::Label::new(Some(button_label(button)));
        name.add_css_class("card-title");
        name.set_xalign(0.0);
        name.set_hexpand(true);
        let warn = gtk::Image::from_icon_name("dialog-warning-symbolic");
        warn.set_pixel_size(11);
        warn.add_css_class("card-warn");
        warn.set_tooltip_text(Some(tr(
            "Another key sends the same action in this gesture slot",
        )));
        warn.set_visible(false);
        head.append(&name);
        head.append(&warn);
        root.append(&head);

        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let mut slot_widgets = Vec::with_capacity(SLOTS.len());
        let mut action_labels = Vec::with_capacity(SLOTS.len());
        for trigger in SLOTS {
            let slot = gtk::Button::builder().focusable(true).build();
            slot.add_css_class("mapping-slot");
            slot.set_widget_name(&format!("slot-{}-{}", button.name(), trigger.name()));
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            row.set_margin_start(6);
            row.set_margin_end(6);
            let slot_name = gtk::Label::new(Some(trigger_label(trigger)));
            slot_name.add_css_class("slot-name");
            slot_name.set_size_request(48, -1);
            slot_name.set_xalign(0.0);
            let action = gtk::Label::new(Some(""));
            action.add_css_class("slot-action");
            action.set_xalign(0.0);
            action.set_hexpand(true);
            action.set_ellipsize(gtk::pango::EllipsizeMode::End);
            row.append(&slot_name);
            row.append(&action);
            slot.set_child(Some(&row));
            list.append(&slot);
            slot_widgets.push(slot);
            action_labels.push(action);
        }
        let slots: [gtk::Button; 3] = slot_widgets
            .try_into()
            .expect("SLOTS describes every card slot");
        let actions: [gtk::Label; 3] = action_labels
            .try_into()
            .expect("SLOTS describes every action label");
        root.append(&list);

        Self {
            root,
            name,
            warn,
            slots,
            actions,
        }
    }
}

/// The board widget plus the widgets the window needs to drive it.
pub struct MappingBoard {
    root: gtk::Box,
    cards: BTreeMap<RemoteButton, KeyCard>,
    diagram: RemoteDiagram,
    voice_card: gtk::Button,
    voice_output: gtk::Label,
    voice_sub: gtk::Label,
}

impl MappingBoard {
    #[must_use]
    pub fn new() -> Self {
        let root = gtk::Box::new(gtk::Orientation::Horizontal, 18);
        root.add_css_class("mapping-board");
        root.set_size_request(BOARD_MIN_WIDTH, -1);
        root.set_hexpand(true);

        let mut cards = BTreeMap::new();
        let left = gtk::Box::new(gtk::Orientation::Vertical, CARD_SPACING);
        left.set_width_request(COLUMN_WIDTH);
        left.set_valign(gtk::Align::Center);
        for button in LEFT_COLUMN {
            let card = KeyCard::new(button);
            left.append(&card.root);
            cards.insert(button, card);
        }
        let right = gtk::Box::new(gtk::Orientation::Vertical, CARD_SPACING);
        right.set_width_request(COLUMN_WIDTH);
        right.set_valign(gtk::Align::Center);
        for button in RIGHT_COLUMN {
            let card = KeyCard::new(button);
            right.append(&card.root);
            cards.insert(button, card);
        }

        let diagram = RemoteDiagram::new();
        let center = gtk::Box::new(gtk::Orientation::Vertical, 9);
        center.set_width_request(CENTER_MIN_WIDTH);
        center.set_hexpand(true);
        center.set_valign(gtk::Align::Fill);
        let hint = gtk::Label::new(Some(tr(
            "Click a slot to edit it; click a key on the remote to edit its single action.",
        )));
        hint.add_css_class("caption");
        hint.add_css_class("dim-label");
        hint.set_wrap(true);
        hint.set_justify(gtk::Justification::Center);
        center.append(&hint);
        center.append(diagram.root());

        let (voice_card, voice_output, voice_sub) = build_voice_card();
        center.append(&voice_card);

        root.append(&left);
        root.append(&center);
        root.append(&right);

        Self {
            root,
            cards,
            diagram,
            voice_card,
            voice_output,
            voice_sub,
        }
    }

    #[must_use]
    pub fn root(&self) -> &gtk::Box {
        &self.root
    }

    #[must_use]
    pub fn diagram(&self) -> &RemoteDiagram {
        &self.diagram
    }

    #[must_use]
    pub fn voice_card(&self) -> &gtk::Button {
        &self.voice_card
    }

    #[must_use]
    pub fn voice_output(&self) -> &gtk::Label {
        &self.voice_output
    }

    #[must_use]
    pub fn card(&self, button: RemoteButton) -> Option<&KeyCard> {
        self.cards.get(&button)
    }

    /// The focusable slot button for one gesture.
    #[must_use]
    pub fn slot(&self, button: RemoteButton, trigger: Trigger) -> Option<&gtk::Button> {
        self.cards.get(&button)?.slot(trigger)
    }

    /// The rendered action text for one gesture slot.
    #[must_use]
    pub fn slot_text(&self, button: RemoteButton, trigger: Trigger) -> Option<String> {
        self.cards.get(&button)?.action_text(trigger)
    }

    /// Connect every card slot; the handler runs on the GTK main thread.
    pub fn connect_slot<F: Fn(RemoteButton, Trigger) + 'static>(&self, handler: F) {
        let handler = Rc::new(handler);
        for (button, card) in &self.cards {
            for (index, slot) in card.slots.iter().enumerate() {
                let handler = Rc::clone(&handler);
                let button = *button;
                let trigger = SLOTS[index];
                slot.connect_clicked(move |_| handler(button, trigger));
            }
        }
    }

    /// Connect the recording-key card in the center column.
    pub fn connect_voice<F: Fn() + 'static>(&self, handler: F) {
        let handler = Rc::new(handler);
        self.voice_card.connect_clicked(move |_| handler());
    }

    /// Render the current selection, conflicts, and test-mode press.
    pub fn refresh(
        &self,
        model: &GuiModel,
        pressed: Option<(RemoteButton, Trigger)>,
        voice_selected: bool,
    ) {
        let conflicts = model.conflicts();
        let selected = (!voice_selected).then_some((model.selected, model.trigger));
        for (button, card) in &self.cards {
            let present = model.profile.has_button(*button);
            card.root.set_visible(present);
            card.set_label(button_label_for(&model.profile, *button));
            let selected_card = present && !voice_selected && *button == model.selected;
            let conflicted = present && conflicts.contains(button);
            remove_selection_classes(&card.root, &["selected", "conflict"]);
            if selected_card {
                card.root.add_css_class("selected");
            }
            if conflicted {
                card.root.add_css_class("conflict");
            }
            card.warn.set_visible(conflicted);
            for (index, trigger) in SLOTS.iter().enumerate() {
                let raw = model.effective_slot(*button, *trigger);
                let label = action_label(raw.as_deref());
                let slot = &card.slots[index];
                remove_selection_classes(slot, &["selected", "test", "conflict", "none"]);
                if selected == Some((*button, *trigger)) {
                    slot.add_css_class("selected");
                }
                if pressed == Some((*button, *trigger)) {
                    slot.add_css_class("test");
                }
                if conflicted && !label.none {
                    slot.add_css_class("conflict");
                }
                if label.none {
                    slot.add_css_class("none");
                }
                card.actions[index].set_text(&label.text);
                let accessible = tr_args(
                    "{button} {trigger}: {action}",
                    &[
                        ("button", button_label(*button)),
                        ("trigger", trigger_label(*trigger)),
                        ("action", &label.text),
                    ],
                );
                slot.update_property(&[gtk::accessible::Property::Label(&accessible)]);
                slot.set_tooltip_text(Some(&tr_args(
                    "{button} · {trigger} · {raw}",
                    &[
                        ("button", button_label(*button)),
                        ("trigger", trigger_label(*trigger)),
                        ("raw", raw.as_deref().unwrap_or("none")),
                    ],
                )));
            }
        }

        self.voice_output
            .set_text(model.document.trigger_key.as_deref().unwrap_or("none"));
        self.voice_sub
            .set_text(&match model.profile.primary_key(RemoteButton::Voice) {
                Some(key) => tr_args("Fixed {key} · ATVV microphone", &[("key", key)]),
                None => tr("Assistant key · ATVV microphone").to_string(),
            });
        remove_selection_classes(&self.voice_card, &["selected"]);
        if voice_selected {
            self.voice_card.add_css_class("selected");
        }
        self.diagram.set_profile(model.profile.id);
        self.diagram
            .refresh(model, pressed.map(|(button, _)| button), voice_selected);
    }
}

impl Default for MappingBoard {
    fn default() -> Self {
        Self::new()
    }
}

fn build_voice_card() -> (gtk::Button, gtk::Label, gtk::Label) {
    let button = gtk::Button::builder()
        .halign(gtk::Align::Fill)
        .tooltip_text(tr("Recording key settings"))
        .build();
    button.add_css_class("voice-card");
    button.set_widget_name("voice-card");
    button.update_property(&[gtk::accessible::Property::Label(tr(
        "Recording key settings",
    ))]);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let icon = gtk::Image::from_icon_name("audio-input-microphone-symbolic");
    icon.set_pixel_size(18);
    icon.add_css_class("voice-icon");
    let copy = gtk::Box::new(gtk::Orientation::Vertical, 0);
    copy.set_hexpand(true);
    let title = gtk::Label::new(Some(tr("Voice key")));
    title.add_css_class("voice-title");
    title.set_xalign(0.0);
    let sub = gtk::Label::new(Some(tr("Fixed KEY_F5 · ATVV microphone")));
    sub.add_css_class("voice-sub");
    sub.set_xalign(0.0);
    sub.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let output = gtk::Box::new(gtk::Orientation::Horizontal, 5);
    let output_label = gtk::Label::new(Some(tr("Output")));
    output_label.add_css_class("voice-sub");
    let chip = gtk::Label::new(Some("DICTATE"));
    chip.add_css_class("voice-output-chip");
    output.append(&output_label);
    output.append(&chip);
    copy.append(&title);
    copy.append(&sub);
    copy.append(&output);
    let chevron = gtk::Image::from_icon_name("go-next-symbolic");
    chevron.set_pixel_size(14);
    chevron.add_css_class("dim-label");
    row.append(&icon);
    row.append(&copy);
    row.append(&chevron);
    button.set_child(Some(&row));
    (button, chip, sub)
}

/// Scroll `target` into view inside `scroller` on both axes.
pub fn scroll_into_view(scroller: &gtk::ScrolledWindow, target: &impl IsA<gtk::Widget>) {
    let target = target.as_ref();
    let Some(bounds) = target.compute_bounds(scroller) else {
        return;
    };
    let margin = 12.0;
    for (adjustment, start, end) in [
        (
            scroller.hadjustment(),
            f64::from(bounds.x()),
            f64::from(bounds.x() + bounds.width()),
        ),
        (
            scroller.vadjustment(),
            f64::from(bounds.y()),
            f64::from(bounds.y() + bounds.height()),
        ),
    ] {
        let value = adjustment.value();
        if start < 0.0 {
            adjustment.set_value(value + start - margin);
        } else if end > adjustment.page_size() {
            adjustment.set_value(value + end - adjustment.page_size() + margin);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_logical_button_has_exactly_one_card() {
        let mut seen = BTreeSet::new();
        for button in LEFT_COLUMN.into_iter().chain(RIGHT_COLUMN) {
            assert_ne!(button, RemoteButton::Voice, "the voice key is not a card");
            assert!(seen.insert(button), "{button} has two cards");
        }
        for button in RemoteButton::LOGICAL {
            assert!(seen.contains(&button), "{button} has no card");
        }
        assert_eq!(seen.len(), RemoteButton::LOGICAL.len());
    }

    #[test]
    fn every_profile_button_has_a_card_in_a_column() {
        let mut seen = BTreeSet::new();
        for button in LEFT_COLUMN.into_iter().chain(RIGHT_COLUMN) {
            seen.insert(button);
        }
        for profile in [atvv_protocol::RC003, atvv_protocol::CHROMECAST] {
            for button in profile.buttons {
                assert!(seen.contains(button), "{} misses {button}", profile.id);
            }
            assert!(!seen.contains(&RemoteButton::Voice));
        }
    }

    #[test]
    fn every_card_has_three_gesture_slots() {
        assert_eq!(SLOTS, [Trigger::Single, Trigger::Double, Trigger::Long]);
        let mut names = BTreeSet::new();
        for trigger in SLOTS {
            assert!(names.insert(trigger.name()), "duplicate slot {trigger:?}");
        }
    }
}
