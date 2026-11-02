// SPDX-License-Identifier: GPL-3.0-or-later

//! Modal editors: one action slot and the recording-key output.
//!
//! Both dialogs are views: they read a [`GuiModel`] when opened, compose a
//! draft, and hand commands back to the window. Validation and persistence
//! stay in `GuiModel`/`SetConfig`, so the dialogs cannot bypass the atomic
//! apply path.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::gio;
use gtk::glib::variant::ToVariant;
use gtk::prelude::*;
use libadwaita as adw;

use adw::prelude::*;

use atvv_input::{PointerAction, PointerAxis, PointerButton};

use crate::actions::{
    ActionDraft, ActionKind, DictationChoice, POINTER_AXES, POINTER_BUTTONS, PointerKind,
    pointer_axis_index, pointer_axis_label, pointer_button_index, pointer_button_label,
};
use crate::i18n::{tr, tr_args};
use crate::keypicker::KeyPicker;
use crate::model::{GuiModel, button_label_for, trigger_label};

/// Hint shown under the action fields.
fn action_hint(kind: ActionKind, allow_exec: bool) -> &'static str {
    match kind {
        ActionKind::None => tr("The button is swallowed; nothing is emitted."),
        ActionKind::Key => tr("One key, emitted after the gesture."),
        ActionKind::Combo => {
            tr("A chord, emitted after the gesture. Modifiers follow ctrl, alt, shift, super.")
        }
        ActionKind::Dictation => tr("Controls the ATVV microphone: start, stop, or toggle."),
        ActionKind::Pointer => tr(
            "Emits a mouse click or moves the pointer. Movement is relative; \
             the desktop may apply pointer acceleration.",
        ),
        ActionKind::Exec if allow_exec => {
            tr("The command runs as your user with a 5 s timeout and closed stdio.")
        }
        ActionKind::Exec => {
            tr("Shell commands are disabled. Turn on “Allow shell commands” first.")
        }
    }
}

/// Consumer hint shown under the recording-key fields.
#[must_use]
pub fn voice_mode_hint(mode: &str) -> String {
    let bind = tr(
        "Consumers must bind the same key. fcitx5-vinput uses Dictate, \
         VoiceCommand, Assistant.",
    );
    match mode {
        "dbus" => format!(
            "{}\n{}",
            tr("vinput is in D-Bus mode; the output key is not emitted."),
            bind
        ),
        "off" => format!(
            "{}\n{}",
            tr("vinput integration is off; the output key is not emitted."),
            bind
        ),
        _ => bind.to_string(),
    }
}

fn footer_bar() -> gtk::Box {
    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    footer.set_margin_top(10);
    footer.set_margin_bottom(10);
    footer.set_margin_start(12);
    footer.set_margin_end(12);
    footer
}

fn spacer() -> gtk::Box {
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    spacer
}

fn dim_label(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("caption");
    label.add_css_class("dim-label");
    label.set_wrap(true);
    label.set_xalign(0.0);
    label
}

fn error_label() -> gtk::Label {
    let label = gtk::Label::new(None);
    label.add_css_class("caption");
    label.add_css_class("error");
    label.set_wrap(true);
    label.set_xalign(0.0);
    label.set_visible(false);
    label
}

/// The action fields, shared with the type-row handler.
///
/// GTK widgets are reference counted, so the handler and the editor observe
/// the same rows; the `Cell`s are shared through `Rc`.
struct EditorRows {
    type_row: adw::ComboRow,
    value_row: adw::EntryRow,
    choose_key: gtk::Button,
    learn_key: gtk::Button,
    pointer_kind: adw::ComboRow,
    pointer_button: adw::ComboRow,
    pointer_direction: adw::ComboRow,
    pointer_units: adw::SpinRow,
    exec_row: adw::EntryRow,
    allow_row: adw::SwitchRow,
    dictation_row: adw::ComboRow,
    hint: gtk::Label,
    update: gtk::Button,
    allow_exec: Cell<bool>,
    loading: Cell<bool>,
}

impl EditorRows {
    fn refresh(&self) {
        let kind = ActionKind::from_index(self.type_row.selected());
        let allow = self.allow_exec.get();
        let key_value = matches!(kind, ActionKind::Key | ActionKind::Combo);
        self.value_row.set_visible(key_value);
        self.choose_key.set_visible(key_value);
        self.learn_key.set_visible(key_value);
        self.value_row.set_title(if kind == ActionKind::Combo {
            tr("Chord")
        } else {
            tr("Key")
        });
        self.exec_row.set_visible(kind == ActionKind::Exec);
        self.allow_row.set_visible(kind == ActionKind::Exec);
        self.dictation_row
            .set_visible(kind == ActionKind::Dictation);
        self.refresh_pointer(kind == ActionKind::Pointer);
        self.hint.set_text(action_hint(kind, allow));
        self.update.set_sensitive(kind != ActionKind::Exec || allow);
    }

    fn refresh_pointer(&self, visible: bool) {
        self.pointer_kind.set_visible(visible);
        if !visible {
            self.pointer_button.set_visible(false);
            self.pointer_direction.set_visible(false);
            self.pointer_units.set_visible(false);
            return;
        }
        let click = PointerKind::from_index(self.pointer_kind.selected()) == PointerKind::Click;
        self.pointer_button.set_visible(click);
        self.pointer_direction.set_visible(!click);
        self.pointer_units.set_visible(!click);
    }

    /// Insert a picked key, preserving chord modifiers.
    fn set_chosen_key(&self, name: &str) {
        match ActionKind::from_index(self.type_row.selected()) {
            ActionKind::Key => self.value_row.set_text(name),
            ActionKind::Combo => {
                let current = self.value_row.text().to_string();
                let prefix = current
                    .rsplit_once('+')
                    .map(|(prefix, _)| format!("{prefix}+"))
                    .unwrap_or_default();
                self.value_row.set_text(&format!("{prefix}{name}"));
            }
            _ => {}
        }
    }
}

/// Editor for one button/gesture slot.
pub struct ActionEditor {
    dialog: adw::Dialog,
    title: adw::WindowTitle,
    group: adw::PreferencesGroup,
    rows: Rc<EditorRows>,
    error: gtk::Label,
    reset: gtk::Button,
    cancel: gtk::Button,
    learn_remote: gtk::Button,
    picker: Rc<KeyPicker>,
    open: Rc<Cell<bool>>,
}

impl ActionEditor {
    #[must_use]
    pub fn new() -> Self {
        let type_list = gtk::StringList::new(&ActionKind::ALL.map(ActionKind::label));
        let dictation_list =
            gtk::StringList::new(&DictationChoice::ALL.map(DictationChoice::label));

        let group = adw::PreferencesGroup::new();
        let type_row = adw::ComboRow::builder()
            .title(tr("Type"))
            .model(&type_list)
            .selected(0)
            .build();
        let value_row = adw::EntryRow::builder().title(tr("Key")).build();
        let choose_key = gtk::Button::with_label(tr("Choose…"));
        choose_key.add_css_class("flat");
        choose_key.set_tooltip_text(Some(tr("Choose a key by name")));
        let learn_key = gtk::Button::with_label(tr("Learn Key"));
        learn_key.add_css_class("flat");
        learn_key.set_tooltip_text(Some(tr("Capture a keyboard key or chord")));
        value_row.add_suffix(&choose_key);
        value_row.add_suffix(&learn_key);
        let exec_row = adw::EntryRow::builder().title(tr("Command")).build();
        let allow_row = adw::SwitchRow::builder()
            .title(tr("Allow shell commands"))
            .subtitle(tr(
                "Exec mappings run as your user with a 5 s timeout. Off by default.",
            ))
            .build();
        let dictation_row = adw::ComboRow::builder()
            .title(tr("Dictation"))
            .model(&dictation_list)
            .selected(0)
            .build();
        let pointer_kind_list = gtk::StringList::new(&PointerKind::ALL.map(PointerKind::label));
        let pointer_button_list = gtk::StringList::new(&POINTER_BUTTONS.map(pointer_button_label));
        let pointer_direction_list = gtk::StringList::new(&POINTER_AXES.map(pointer_axis_label));
        let pointer_kind = adw::ComboRow::builder()
            .title(tr("Pointer action"))
            .model(&pointer_kind_list)
            .selected(0)
            .build();
        let pointer_button = adw::ComboRow::builder()
            .title(tr("Button"))
            .model(&pointer_button_list)
            .selected(0)
            .build();
        let pointer_direction = adw::ComboRow::builder()
            .title(tr("Direction"))
            .model(&pointer_direction_list)
            .selected(0)
            .build();
        let pointer_units = adw::SpinRow::with_range(1.0, 255.0, 1.0);
        pointer_units.set_title(tr("Units"));
        pointer_units.set_value(1.0);
        let hint = dim_label("");
        let update = gtk::Button::with_label(tr("Update"));
        update.add_css_class("suggested-action");
        group.add(&type_row);
        group.add(&value_row);
        group.add(&pointer_kind);
        group.add(&pointer_button);
        group.add(&pointer_direction);
        group.add(&pointer_units);
        group.add(&exec_row);
        group.add(&allow_row);
        group.add(&dictation_row);

        let error = error_label();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.append(&group);
        content.append(&hint);
        content.append(&error);

        let footer = footer_bar();
        let learn_remote = gtk::Button::with_label(tr("Learn remote…"));
        learn_remote.add_css_class("flat");
        learn_remote.set_tooltip_text(Some(tr("Press a physical remote key to select its slot")));
        let reset = gtk::Button::with_label(tr("Reset"));
        reset.add_css_class("flat");
        reset.set_tooltip_text(Some(tr("Restore the profile default")));
        let cancel = gtk::Button::with_label(tr("Cancel"));
        cancel.add_css_class("flat");
        footer.append(&learn_remote);
        footer.append(&spacer());
        footer.append(&reset);
        footer.append(&cancel);
        footer.append(&update);

        let title = adw::WindowTitle::new(tr("Mapping"), "");
        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&title));

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&content));
        toolbar.add_bottom_bar(&footer);
        let dialog = adw::Dialog::builder()
            .title(tr("Mapping"))
            .content_width(440)
            .child(&toolbar)
            .build();
        dialog.set_default_widget(Some(&update));
        dialog.set_focus(Some(&type_row));

        let rows = Rc::new(EditorRows {
            type_row,
            value_row,
            choose_key,
            learn_key,
            pointer_kind,
            pointer_button,
            pointer_direction,
            pointer_units,
            exec_row,
            allow_row,
            dictation_row,
            hint,
            update: update.clone(),
            allow_exec: Cell::new(false),
            loading: Cell::new(false),
        });
        let refresh = Rc::clone(&rows);
        rows.type_row
            .connect_selected_notify(move |_| refresh.refresh());
        let pointer_refresh = Rc::clone(&rows);
        rows.pointer_kind
            .connect_selected_notify(move |_| pointer_refresh.refresh());

        let picker_rows = Rc::clone(&rows);
        let picker_error = error.clone();
        let picker = Rc::new(KeyPicker::new(move |name| {
            picker_rows.set_chosen_key(name);
            picker_error.set_text("");
            picker_error.set_visible(false);
        }));
        let picker_for_button = Rc::clone(&picker);
        let dialog_for_button = dialog.clone();
        rows.choose_key
            .connect_clicked(move |_| picker_for_button.present(&dialog_for_button));

        let editor = Self {
            dialog,
            title,
            group,
            rows,
            error,
            reset,
            cancel,
            learn_remote,
            picker,
            open: Rc::new(Cell::new(false)),
        };
        let open = Rc::clone(&editor.open);
        editor.dialog.connect_closed(move |_| open.set(false));
        editor
    }

    #[must_use]
    pub fn dialog(&self) -> &adw::Dialog {
        &self.dialog
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open.get()
    }

    pub fn present(&self, parent: &impl IsA<gtk::Widget>) {
        self.open.set(true);
        self.dialog.present(Some(parent));
    }

    pub fn close(&self) {
        self.open.set(false);
        self.dialog.close();
    }

    /// Fill the fields from the model's current slot.
    pub fn load(&self, model: &GuiModel) {
        self.title.set_title(&tr_args(
            "{button} · {trigger}",
            &[
                ("button", button_label_for(&model.profile, model.selected)),
                ("trigger", trigger_label(model.trigger)),
            ],
        ));
        let custom = model.selected_raw().is_some();
        let has_default = model
            .profile_default(model.selected, model.trigger)
            .is_some();
        self.group.set_description(Some(if custom {
            tr("Custom mapping")
        } else if has_default {
            tr("Profile default")
        } else {
            tr("Unbound")
        }));
        let draft = ActionDraft::from_raw(model.selected_effective().as_deref().unwrap_or("none"));
        self.rows.loading.set(true);
        self.rows.type_row.set_selected(draft.kind.index());
        self.rows.value_row.set_text(match draft.kind {
            ActionKind::Key => &draft.key,
            ActionKind::Combo => &draft.chord,
            _ => "",
        });
        self.rows.exec_row.set_text(&draft.command);
        self.rows
            .dictation_row
            .set_selected(draft.dictation.index());
        match draft.pointer {
            PointerAction::Click(button) => {
                self.rows
                    .pointer_kind
                    .set_selected(PointerKind::Click.index());
                self.rows
                    .pointer_button
                    .set_selected(pointer_button_index(button));
            }
            PointerAction::Move { direction, units } => {
                self.rows
                    .pointer_kind
                    .set_selected(PointerKind::Move.index());
                self.rows
                    .pointer_direction
                    .set_selected(pointer_axis_index(direction));
                self.rows.pointer_units.set_value(f64::from(units));
            }
        }
        self.rows.allow_row.set_active(model.document.allow_exec);
        self.rows.allow_exec.set(model.document.allow_exec);
        self.reset.set_sensitive(custom);
        self.rows.loading.set(false);
        self.rows.refresh();
        self.show_error("");
    }

    /// Store the global policy and mirror it in the switch.
    pub fn set_allow_exec(&self, allow: bool) {
        self.rows.allow_exec.set(allow);
        self.rows.loading.set(true);
        self.rows.allow_row.set_active(allow);
        self.rows.loading.set(false);
        self.rows.refresh();
    }

    /// The editable value composed from the dialog fields.
    #[must_use]
    pub fn draft(&self) -> ActionDraft {
        let kind = ActionKind::from_index(self.rows.type_row.selected());
        let text = self.rows.value_row.text().to_string();
        let pointer =
            if PointerKind::from_index(self.rows.pointer_kind.selected()) == PointerKind::Move {
                let index = usize::try_from(self.rows.pointer_direction.selected()).unwrap_or(0);
                let direction = POINTER_AXES.get(index).copied().unwrap_or(PointerAxis::Up);
                let units = self.rows.pointer_units.value().round() as u16;
                PointerAction::Move { direction, units }
            } else {
                let index = usize::try_from(self.rows.pointer_button.selected()).unwrap_or(0);
                let button = POINTER_BUTTONS
                    .get(index)
                    .copied()
                    .unwrap_or(PointerButton::Left);
                PointerAction::Click(button)
            };
        ActionDraft {
            kind,
            key: text.clone(),
            chord: text,
            command: self.rows.exec_row.text().to_string(),
            dictation: DictationChoice::from_index(self.rows.dictation_row.selected()),
            pointer,
        }
    }

    /// Fill the value from a learned keyboard key or chord.
    pub fn set_learned(&self, learned: &str) {
        let draft = ActionDraft::from_raw(learned);
        self.rows.loading.set(true);
        self.rows.type_row.set_selected(draft.kind.index());
        if draft.kind == ActionKind::Key {
            self.rows.value_row.set_text(&draft.key);
        } else if draft.kind == ActionKind::Combo {
            self.rows.value_row.set_text(&draft.chord);
        }
        self.rows.loading.set(false);
        self.rows.refresh();
        self.show_error("");
    }

    pub fn show_error(&self, message: &str) {
        self.error.set_text(message);
        self.error.set_visible(!message.is_empty());
    }

    // Callbacks for the window.

    pub fn connect_update<F: Fn() + 'static>(&self, handler: F) {
        self.rows.update.connect_clicked(move |_| handler());
    }

    pub fn connect_reset<F: Fn() + 'static>(&self, handler: F) {
        self.reset.connect_clicked(move |_| handler());
    }

    pub fn connect_cancel<F: Fn() + 'static>(&self, handler: F) {
        self.cancel.connect_clicked(move |_| handler());
    }

    pub fn connect_learn_key<F: Fn() + 'static>(&self, handler: F) {
        self.rows.learn_key.connect_clicked(move |_| handler());
    }

    pub fn connect_learn_remote<F: Fn() + 'static>(&self, handler: F) {
        self.learn_remote.connect_clicked(move |_| handler());
    }

    /// The user toggled the policy switch; the window confirms and applies it.
    pub fn connect_allow_exec<F: Fn(bool) + 'static>(&self, handler: F) {
        let rows = Rc::clone(&self.rows);
        self.rows.allow_row.connect_active_notify(move |row| {
            if rows.loading.get() {
                return;
            }
            handler(row.is_active());
        });
    }

    // Widgets for the window and tests.

    #[must_use]
    pub fn update_button(&self) -> &gtk::Button {
        &self.rows.update
    }

    #[must_use]
    pub fn reset_button(&self) -> &gtk::Button {
        &self.reset
    }

    #[must_use]
    pub fn cancel_button(&self) -> &gtk::Button {
        &self.cancel
    }

    #[must_use]
    pub fn learn_key_button(&self) -> &gtk::Button {
        &self.rows.learn_key
    }

    #[must_use]
    pub fn choose_key_button(&self) -> &gtk::Button {
        &self.rows.choose_key
    }

    /// The searchable key picker, exposed for tests.
    #[must_use]
    pub fn picker(&self) -> &KeyPicker {
        &self.picker
    }

    #[must_use]
    pub fn learn_remote_button(&self) -> &gtk::Button {
        &self.learn_remote
    }

    #[must_use]
    pub fn allow_exec_row(&self) -> &adw::SwitchRow {
        &self.rows.allow_row
    }

    #[must_use]
    pub fn type_row(&self) -> &adw::ComboRow {
        &self.rows.type_row
    }

    #[must_use]
    pub fn value_row(&self) -> &adw::EntryRow {
        &self.rows.value_row
    }

    #[must_use]
    pub fn exec_row(&self) -> &adw::EntryRow {
        &self.rows.exec_row
    }

    #[must_use]
    pub fn dictation_row(&self) -> &adw::ComboRow {
        &self.rows.dictation_row
    }

    #[must_use]
    pub fn pointer_kind_row(&self) -> &adw::ComboRow {
        &self.rows.pointer_kind
    }

    #[must_use]
    pub fn pointer_button_row(&self) -> &adw::ComboRow {
        &self.rows.pointer_button
    }

    #[must_use]
    pub fn pointer_direction_row(&self) -> &adw::ComboRow {
        &self.rows.pointer_direction
    }

    #[must_use]
    pub fn pointer_units_row(&self) -> &adw::SpinRow {
        &self.rows.pointer_units
    }

    #[must_use]
    pub fn error(&self) -> &gtk::Label {
        &self.error
    }

    #[must_use]
    pub fn hint_text(&self) -> String {
        self.rows.hint.text().to_string()
    }

    #[must_use]
    pub fn title(&self) -> String {
        self.title.title().to_string()
    }
}

impl Default for ActionEditor {
    fn default() -> Self {
        Self::new()
    }
}

/// Editor for the recording-key output.
pub struct VoiceEditor {
    dialog: adw::Dialog,
    value_row: adw::EntryRow,
    declared_button: gtk::MenuButton,
    learn_key: gtk::Button,
    hint: gtk::Label,
    error: gtk::Label,
    reset: gtk::Button,
    cancel: gtk::Button,
    update: gtk::Button,
    keys: RefCell<Vec<String>>,
    open: Rc<Cell<bool>>,
}

impl VoiceEditor {
    #[must_use]
    pub fn new() -> Self {
        let group = adw::PreferencesGroup::new();
        let info = adw::ActionRow::builder()
            .title(tr("Physical key"))
            .subtitle(tr(
                "KEY_F5 is grabbed and fixed; it controls the ATVV microphone.",
            ))
            .build();
        let value_row = adw::EntryRow::builder().title(tr("Output key")).build();
        let declared_button = gtk::MenuButton::builder()
            .label(tr("Declared key…"))
            .always_show_arrow(true)
            .tooltip_text(tr("Choose a declared recording key"))
            .build();
        declared_button.add_css_class("flat");
        declared_button.set_visible(false);
        let learn_key = gtk::Button::with_label(tr("Learn Key"));
        learn_key.add_css_class("flat");
        learn_key.set_tooltip_text(Some(tr("Capture a key or chord for the output")));
        value_row.add_suffix(&declared_button);
        value_row.add_suffix(&learn_key);
        group.add(&info);
        group.add(&value_row);

        let hint = dim_label("");
        let error = error_label();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.append(&group);
        content.append(&hint);
        content.append(&error);

        let footer = footer_bar();
        let reset = gtk::Button::with_label(tr("Reset to DICTATE"));
        reset.add_css_class("flat");
        let cancel = gtk::Button::with_label(tr("Cancel"));
        cancel.add_css_class("flat");
        let update = gtk::Button::with_label(tr("Update"));
        update.add_css_class("suggested-action");
        footer.append(&reset);
        footer.append(&spacer());
        footer.append(&cancel);
        footer.append(&update);

        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&adw::WindowTitle::new(tr("Recording key"), "")));
        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&content));
        toolbar.add_bottom_bar(&footer);
        let dialog = adw::Dialog::builder()
            .title(tr("Recording key"))
            .content_width(440)
            .child(&toolbar)
            .build();
        dialog.set_default_widget(Some(&update));
        dialog.set_focus(Some(&value_row));

        let editor = Self {
            dialog,
            value_row,
            declared_button,
            learn_key,
            hint,
            error,
            reset,
            cancel,
            update,
            keys: RefCell::new(Vec::new()),
            open: Rc::new(Cell::new(false)),
        };
        let open = Rc::clone(&editor.open);
        editor.dialog.connect_closed(move |_| open.set(false));
        editor
    }

    #[must_use]
    pub fn dialog(&self) -> &adw::Dialog {
        &self.dialog
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open.get()
    }

    pub fn present(&self, parent: &impl IsA<gtk::Widget>) {
        self.open.set(true);
        self.dialog.present(Some(parent));
    }

    pub fn close(&self) {
        self.open.set(false);
        self.dialog.close();
    }

    pub fn load(&self, model: &GuiModel) {
        self.value_row
            .set_text(model.document.trigger_key.as_deref().unwrap_or("none"));
        self.reset.set_sensitive(
            model.document.trigger_key.as_deref() != Some(atvv_config::DEFAULT_TRIGGER_KEY),
        );
        self.show_error("");
    }

    /// Fill the output field without saving it.
    pub fn set_value(&self, key: &str) {
        self.value_row.set_text(key);
        self.show_error("");
    }

    #[must_use]
    pub fn value(&self) -> String {
        self.value_row.text().to_string()
    }

    /// Rebuild the declared-key menu only when the contract list changes.
    pub fn refresh_declared_keys(&self, keys: &[String]) {
        if self.keys.borrow().as_slice() == keys {
            return;
        }
        self.keys.borrow_mut().clear();
        self.keys.borrow_mut().extend_from_slice(keys);
        if keys.is_empty() {
            self.declared_button.set_visible(false);
        } else {
            self.declared_button
                .set_menu_model(Some(&declared_key_menu(keys)));
            self.declared_button.set_visible(true);
        }
    }

    /// Consumer hint for the current vinput mode.
    pub fn set_mode(&self, mode: &str) {
        let hint = voice_mode_hint(mode);
        if self.hint.text().as_str() != hint {
            self.hint.set_text(&hint);
        }
    }

    pub fn show_error(&self, message: &str) {
        self.error.set_text(message);
        self.error.set_visible(!message.is_empty());
    }

    pub fn connect_update<F: Fn() + 'static>(&self, handler: F) {
        self.update.connect_clicked(move |_| handler());
    }

    pub fn connect_reset<F: Fn() + 'static>(&self, handler: F) {
        self.reset.connect_clicked(move |_| handler());
    }

    pub fn connect_cancel<F: Fn() + 'static>(&self, handler: F) {
        self.cancel.connect_clicked(move |_| handler());
    }

    pub fn connect_learn_key<F: Fn() + 'static>(&self, handler: F) {
        self.learn_key.connect_clicked(move |_| handler());
    }

    #[must_use]
    pub fn value_row(&self) -> &adw::EntryRow {
        &self.value_row
    }

    #[must_use]
    pub fn declared_button(&self) -> &gtk::MenuButton {
        &self.declared_button
    }

    #[must_use]
    pub fn update_button(&self) -> &gtk::Button {
        &self.update
    }

    #[must_use]
    pub fn reset_button(&self) -> &gtk::Button {
        &self.reset
    }

    #[must_use]
    pub fn hint_text(&self) -> String {
        self.hint.text().to_string()
    }

    #[must_use]
    pub fn error(&self) -> &gtk::Label {
        &self.error
    }
}

impl Default for VoiceEditor {
    fn default() -> Self {
        Self::new()
    }
}

/// Menu model for the declared recording keys.
///
/// The key names come from the daemon contract; `win.voice-key` fills the
/// output entry without saving, so the normal Update/Apply path still owns
/// validation and persistence.
fn declared_key_menu(keys: &[String]) -> gio::Menu {
    let menu = gio::Menu::new();
    let declared = gio::Menu::new();
    for (index, key) in keys.iter().enumerate() {
        let label = if index == 0 {
            format!("{key} ({})", tr("Default"))
        } else {
            key.clone()
        };
        let item = gio::MenuItem::new(Some(&label), Some("win.voice-key"));
        item.set_attribute_value("target", Some(&key.to_variant()));
        declared.append_item(&item);
    }
    menu.append_section(None, &declared);

    let extra = gio::Menu::new();
    let none = gio::MenuItem::new(
        Some(tr("None (disable synthetic key)")),
        Some("win.voice-key"),
    );
    none.set_attribute_value("target", Some(&"none".to_variant()));
    extra.append_item(&none);
    menu.append_section(None, &extra);
    menu
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hints_explain_each_action_kind() {
        let _guard = crate::i18n::language_guard();
        crate::i18n::Language::set(crate::i18n::Language::English);
        assert!(action_hint(ActionKind::None, false).contains("swallowed"));
        assert!(action_hint(ActionKind::Key, false).contains("One key"));
        assert!(action_hint(ActionKind::Combo, false).contains("ctrl, alt, shift, super"));
        assert!(action_hint(ActionKind::Dictation, false).contains("microphone"));
        assert!(action_hint(ActionKind::Exec, false).contains("disabled"));
        assert!(!action_hint(ActionKind::Exec, true).contains("disabled"));
        assert!(action_hint(ActionKind::Exec, true).contains("5 s"));
    }

    #[test]
    fn voice_hint_tracks_the_vinput_mode() {
        let _guard = crate::i18n::language_guard();
        crate::i18n::Language::set(crate::i18n::Language::English);
        let trigger = voice_mode_hint("trigger");
        assert!(trigger.contains("Consumers must bind"));
        assert!(!trigger.contains("not emitted"));

        let dbus = voice_mode_hint("dbus");
        assert!(dbus.contains("D-Bus"));
        assert!(dbus.contains("not emitted"));

        let off = voice_mode_hint("off");
        assert!(off.contains("integration is off"));
        assert!(off.contains("not emitted"));
    }
}
