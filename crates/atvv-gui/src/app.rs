// SPDX-License-Identifier: GPL-3.0-or-later

//! GTK4/libadwaita window: mapping board, modal editors, capture, test bar.
//!
//! The window owns the [`GuiModel`] and renders it through the board. Every
//! edit goes through the model and the daemon's `SetConfig`; the window never
//! writes the configuration file, grabs input devices, or talks to BlueZ.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::{Rc, Weak};
use std::time::Duration;

use atvv_config::{
    ConfigDocument, LAYOUT_PRESETS, LayoutPreset, LayoutSummary, unique_layout_name,
};
use atvv_input::Trigger;
use atvv_protocol::profile::RemoteButton;
use gtk::glib::variant::ToVariant;
use gtk::{gio, glib};
use libadwaita as adw;

use adw::prelude::*;

use crate::board::{MappingBoard, scroll_into_view};
use crate::client::{Backend, ButtonSignal, DaemonStatus, Event, HardwareInfo};
use crate::editor::{ActionEditor, MicrophoneDialog, VoiceEditor, voice_mode_hint};
use crate::i18n::{self, tr, tr_args};
use crate::keylearn;
use crate::model::{
    GuiModel, action_label, button_label, button_label_for, trigger_from_name, trigger_label,
};

pub const APP_ID: &str = "org.atvv.Config";

const CSS: &str = r#"
.remote-canvas {
  background: alpha(@card_bg_color, 0.55);
  border-radius: 24px;
}
.remote-hotspot {
  background: transparent;
  border: 1.5px solid transparent;
  border-radius: 14px;
  box-shadow: none;
  padding: 0;
  min-width: 0;
  min-height: 0;
}
.remote-hotspot:hover {
  background: alpha(@accent_bg_color, 0.16);
  border-color: alpha(@accent_color, 0.45);
}
.remote-hotspot:focus-visible {
  outline: 2px solid @accent_color;
  outline-offset: 2px;
}
.remote-hotspot.selected {
  background: alpha(@accent_bg_color, 0.38);
  border: 2px solid @accent_color;
  box-shadow: 0 0 0 2px alpha(@accent_color, 0.28);
}
.remote-hotspot.conflict {
  background: alpha(@warning_color, 0.12);
  border-color: @warning_color;
}
.remote-hotspot.pressed {
  background: alpha(@accent_bg_color, 0.62);
  border: 2px solid @accent_color;
  box-shadow: 0 0 0 3px alpha(@accent_color, 0.32);
}
.remote-hotspot.voice.pressed {
  background: alpha(@accent_bg_color, 0.62);
  border: 2px solid @accent_color;
}

.mapping-board {
  padding: 16px 20px;
}
.mapping-card {
  background: alpha(@card_bg_color, 0.92);
  border: 1px solid @borders;
  border-radius: 12px;
  padding: 6px;
}
.mapping-card.selected {
  border-color: @accent_color;
  box-shadow: 0 0 0 1px alpha(@accent_color, 0.35);
}
.mapping-card.conflict {
  border-color: @warning_color;
}
.card-title {
  font-weight: 700;
}
.card-warn {
  color: @warning_color;
}
.mapping-slot {
  min-height: 0;
  min-width: 0;
  padding: 1px 2px;
  background: transparent;
  border: 0 solid transparent;
  box-shadow: none;
  border-radius: 6px;
}
.mapping-slot:hover {
  background: alpha(currentColor, 0.07);
}
.mapping-slot:focus-visible {
  outline: 2px solid @accent_color;
  outline-offset: -1px;
}
.mapping-slot.selected {
  background: alpha(@accent_bg_color, 0.32);
  border-left: 2px solid @accent_color;
}
.mapping-slot.conflict {
  background: alpha(@warning_bg_color, 0.30);
  border-left: 2px solid @warning_color;
}
.mapping-slot.test {
  background: @accent_bg_color;
}
.mapping-slot.test label {
  color: @accent_fg_color;
}
.slot-name {
  font-size: 10px;
  font-weight: 700;
  color: alpha(currentColor, 0.55);
}
.slot-action {
  font-size: 12px;
}
.mapping-slot.none .slot-action {
  color: alpha(currentColor, 0.45);
}

.voice-card {
  padding: 8px 10px;
  border: 1px solid @borders;
  border-radius: 12px;
  background: alpha(@card_bg_color, 0.90);
}
.voice-card:hover {
  border-color: alpha(@accent_color, 0.50);
}
.voice-card.selected {
  border-color: @accent_color;
  box-shadow: 0 0 0 1px alpha(@accent_color, 0.35);
}
.voice-icon {
  color: @accent_color;
}
.voice-title {
  font-weight: 700;
}
.voice-sub {
  font-size: 11px;
  color: alpha(currentColor, 0.60);
}
.voice-output-chip {
  font-family: monospace;
  font-size: 10px;
  padding: 0 5px;
  border-radius: 4px;
  background: alpha(currentColor, 0.08);
}

.device-chip {
  padding: 2px 10px;
}
.layout-chip {
  padding: 2px 10px;
}
.layout-row {
  padding: 0;
}
.layout-row-name {
  font-weight: 600;
}
.layout-tag {
  font-size: 10px;
  padding: 0 5px;
  border-radius: 4px;
  background: alpha(currentColor, 0.10);
}
.layout-tag-template {
  color: alpha(currentColor, 0.55);
}
.layout-tag-active {
  color: @accent_color;
}
.layout-manage {
  margin-top: 2px;
}
.test-bar {
  padding: 8px 16px;
  border-top: 1px solid @borders;
}
.test-dot {
  min-width: 8px;
  min-height: 8px;
  border-radius: 999px;
  background: alpha(currentColor, 0.35);
}
.test-bar.on .test-dot {
  background: @accent_color;
  box-shadow: 0 0 0 3px alpha(@accent_color, 0.25);
}
.test-title {
  font-weight: 700;
}
.test-sub {
  font-size: 12px;
  color: alpha(currentColor, 0.65);
}
.last-action {
  font-weight: 600;
  color: @accent_color;
}
label.error {
  color: @error_color;
}
.apply:disabled {
  opacity: 0.45;
}
"#;

/// Install the key-state CSS once per display.
pub fn install_css() {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();
    provider.load_from_data(CSS);
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

/// Start the application.
pub fn run() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    let quit = gio::SimpleAction::new("quit", None);
    let weak = app.downgrade();
    quit.connect_activate(move |_, _| {
        if let Some(app) = weak.upgrade() {
            app.quit();
        }
    });
    app.add_action(&quit);
    app.set_accels_for_action("app.quit", &["<primary>q"]);
    app.connect_activate(|app| {
        i18n::init();
        // GTK/GDK must be initialized before a display exists; installing the
        // stylesheet here is what makes the board states render.
        install_css();
        apply_color_scheme_override();
        let window = build_window(Some(app), Backend::start(None));
        window.window().present();
        window.begin();
    });
    app.run()
}

/// Deterministic theme selection for screenshots and headless checks.
///
/// Normal use follows the system color scheme through libadwaita.
fn apply_color_scheme_override() {
    let Ok(scheme) = std::env::var("ATVV_GUI_COLOR_SCHEME") else {
        return;
    };
    let manager = adw::StyleManager::default();
    match scheme.as_str() {
        "dark" => manager.set_color_scheme(adw::ColorScheme::ForceDark),
        "light" => manager.set_color_scheme(adw::ColorScheme::ForceLight),
        _ => {}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureMode {
    Off,
    Test,
    Learn,
}

/// One Device Information Service fact shown in the device dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareField {
    Firmware,
    Hardware,
    Software,
    Serial,
    Manufacturer,
    Model,
}

impl HardwareField {
    const ALL: [Self; 6] = [
        Self::Firmware,
        Self::Hardware,
        Self::Software,
        Self::Serial,
        Self::Manufacturer,
        Self::Model,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Firmware => tr("Firmware"),
            Self::Hardware => tr("Hardware revision"),
            Self::Software => tr("Software revision"),
            Self::Serial => tr("Serial number"),
            Self::Manufacturer => tr("Manufacturer"),
            Self::Model => tr("Model"),
        }
    }

    fn value(self, hardware: &HardwareInfo) -> Option<&str> {
        match self {
            Self::Firmware => hardware.firmware_revision.as_deref(),
            Self::Hardware => hardware.hardware_revision.as_deref(),
            Self::Software => hardware.software_revision.as_deref(),
            Self::Serial => hardware.serial_number.as_deref(),
            Self::Manufacturer => hardware.manufacturer.as_deref(),
            Self::Model => hardware.model.as_deref(),
        }
    }
}

/// Battery icon name for a percentage.
fn battery_icon(percent: u8) -> &'static str {
    match percent {
        80..=100 => "battery-full-symbolic",
        40..=79 => "battery-good-symbolic",
        20..=39 => "battery-low-symbolic",
        1..=19 => "battery-caution-symbolic",
        _ => "battery-empty-symbolic",
    }
}

/// Battery text for the chip and the device dialog.
fn battery_text(status: Option<&DaemonStatus>) -> String {
    let Some(status) = status else {
        return tr("Unknown").to_string();
    };
    match status.hardware.battery_percent {
        Some(percent) if status.connected => format!("{percent} %"),
        Some(percent) => {
            let text = percent.to_string();
            tr_args("{percent} % · last known", &[("percent", &text)])
        }
        None => tr("Unknown").to_string(),
    }
}

/// Passive device facts, shown from the menu and the header chip.
struct DeviceDialog {
    dialog: adw::Dialog,
    name_row: adw::ActionRow,
    battery_row: adw::ActionRow,
    address_row: adw::ActionRow,
    hardware_group: adw::PreferencesGroup,
    hardware_rows: Vec<(HardwareField, adw::ActionRow)>,
}

impl DeviceDialog {
    fn new() -> Self {
        let remote_group = adw::PreferencesGroup::builder().title(tr("Remote")).build();
        let name_row = adw::ActionRow::builder()
            .title(tr("Name"))
            .subtitle("—")
            .build();
        let battery_row = adw::ActionRow::builder().title(tr("Battery")).build();
        let address_row = adw::ActionRow::builder()
            .title(tr("Address"))
            .subtitle("—")
            .build();
        remote_group.add(&name_row);
        remote_group.add(&battery_row);
        remote_group.add(&address_row);

        let hardware_group = adw::PreferencesGroup::builder()
            .title(tr("Hardware"))
            .build();
        let mut hardware_rows = Vec::new();
        for field in HardwareField::ALL {
            let row = adw::ActionRow::builder().title(field.label()).build();
            row.set_visible(false);
            hardware_group.add(&row);
            hardware_rows.push((field, row));
        }

        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.append(&remote_group);
        content.append(&hardware_group);
        let hint = gtk::Label::new(Some(tr(
            "Passive facts reported by BlueZ and the ATVV GATT session.",
        )));
        hint.add_css_class("caption");
        hint.add_css_class("dim-label");
        hint.set_wrap(true);
        hint.set_xalign(0.0);
        content.append(&hint);

        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        footer.set_margin_top(10);
        footer.set_margin_bottom(10);
        footer.set_margin_start(12);
        footer.set_margin_end(12);
        let close = gtk::Button::with_label(tr("Close"));
        close.add_css_class("flat");
        footer.append(&spacer());
        footer.append(&close);

        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&adw::WindowTitle::new(
            tr("Device information"),
            tr("Bluetooth and GATT facts"),
        )));
        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&content));
        toolbar.add_bottom_bar(&footer);
        let dialog = adw::Dialog::builder()
            .title(tr("Device information"))
            .content_width(440)
            .child(&toolbar)
            .build();
        let weak = dialog.downgrade();
        close.connect_clicked(move |_| {
            if let Some(dialog) = weak.upgrade() {
                dialog.close();
            }
        });

        Self {
            dialog,
            name_row,
            battery_row,
            address_row,
            hardware_group,
            hardware_rows,
        }
    }

    fn present(&self, parent: &impl IsA<gtk::Widget>) {
        self.dialog.present(Some(parent));
    }

    fn refresh(&self, status: Option<&DaemonStatus>) {
        let name = status
            .map(|status| status.device.name.as_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("—");
        self.name_row.set_subtitle(name);
        self.battery_row.set_subtitle(&battery_text(status));
        let address = status
            .map(|status| status.device.address.as_str())
            .filter(|address| !address.is_empty())
            .unwrap_or("—");
        self.address_row.set_subtitle(address);

        let hardware = status.map(|status| &status.hardware);
        let mut any = false;
        for (field, row) in &self.hardware_rows {
            let value = hardware.and_then(|hardware| field.value(hardware));
            row.set_subtitle(value.unwrap_or(""));
            row.set_visible(value.is_some());
            any |= value.is_some();
        }
        self.hardware_group.set_visible(any);
    }

    fn name_text(&self) -> String {
        self.name_row
            .subtitle()
            .map(|subtitle| subtitle.to_string())
            .unwrap_or_default()
    }

    fn battery_text(&self) -> String {
        self.battery_row
            .subtitle()
            .map(|subtitle| subtitle.to_string())
            .unwrap_or_default()
    }

    fn hardware_text(&self, field: HardwareField) -> Option<String> {
        self.hardware_rows
            .iter()
            .find(|(candidate, _)| *candidate == field)
            .and_then(|(_, row)| row.subtitle().map(|subtitle| subtitle.to_string()))
    }
}

fn layout_tag(text: &str, css: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("caption");
    label.add_css_class("layout-tag");
    label.add_css_class(css);
    label
}

fn layout_display_name(summary: &LayoutSummary) -> &str {
    if summary.template {
        tr("Default")
    } else {
        summary.name.as_str()
    }
}

fn layout_popover_row(summary: &LayoutSummary, active: &str) -> gtk::Button {
    let row = gtk::Button::new();
    row.add_css_class("flat");
    row.add_css_class("layout-row");
    row.set_widget_name(&format!("layout-row-{}", summary.id));
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    content.set_margin_top(6);
    content.set_margin_bottom(6);
    content.set_margin_start(8);
    content.set_margin_end(8);
    let check = gtk::Image::from_icon_name("object-select-symbolic");
    check.set_pixel_size(14);
    check.set_visible(summary.id == active);
    let copy = gtk::Box::new(gtk::Orientation::Vertical, 1);
    copy.set_hexpand(true);
    let name = gtk::Label::new(Some(layout_display_name(summary)));
    name.set_xalign(0.0);
    name.add_css_class("layout-row-name");
    copy.append(&name);
    if let Some(description) = summary
        .description
        .as_deref()
        .filter(|description| !description.is_empty())
    {
        let label = gtk::Label::new(Some(description));
        label.set_xalign(0.0);
        label.add_css_class("caption");
        label.add_css_class("dim-label");
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        copy.append(&label);
    }
    content.append(&check);
    content.append(&copy);
    if summary.template {
        content.append(&layout_tag(tr("Template"), "layout-tag-template"));
    }
    row.set_child(Some(&content));
    row.update_property(&[gtk::accessible::Property::Label(layout_display_name(
        summary,
    ))]);
    row
}

fn layout_manager_row(
    summary: &LayoutSummary,
    active: &str,
    weak: Weak<AppWindow>,
) -> (adw::ActionRow, Vec<(String, gtk::Button)>) {
    let row = adw::ActionRow::builder()
        .title(layout_display_name(summary))
        .subtitle(summary.description.as_deref().unwrap_or(""))
        .activatable(true)
        .build();
    row.set_widget_name(&format!("layout-manager-row-{}", summary.id));
    let check = gtk::Image::from_icon_name("object-select-symbolic");
    check.set_pixel_size(14);
    check.set_visible(summary.id == active);
    row.add_prefix(&check);
    let tags = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    if summary.id == active {
        tags.append(&layout_tag(tr("Active"), "layout-tag-active"));
    }
    if summary.template {
        tags.append(&layout_tag(tr("Template"), "layout-tag-template"));
    }
    row.add_suffix(&tags);
    let more = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .tooltip_text(tr_args(
            "Actions for {name}",
            &[("name", layout_display_name(summary))],
        ))
        .build();
    more.add_css_class("flat");
    let popover = gtk::Popover::new();
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let mut buttons = Vec::new();
    for (action, label) in [
        ("duplicate", "Duplicate"),
        ("rename", "Rename"),
        ("delete", "Delete"),
        ("reset", "Reset"),
        ("preset-reset", "Reset to preset…"),
    ] {
        let button = gtk::Button::with_label(tr(label));
        button.add_css_class("flat");
        let disabled = (summary.template && action != "duplicate")
            || (action == "delete" && summary.id == active);
        if !disabled {
            button.set_action_name(Some(&format!("win.layout-{action}")));
            button.set_action_target_value(Some(&summary.id.to_variant()));
        }
        button.set_sensitive(!disabled);
        buttons.push((format!("{}:{action}", summary.id), button.clone()));
        menu.append(&button);
    }
    popover.set_child(Some(&menu));
    more.set_popover(Some(&popover));
    row.add_suffix(&more);
    let is_active = summary.id == active;
    let id = summary.id.clone();
    row.connect_activated(move |_| {
        if !is_active && let Some(this) = weak.upgrade() {
            this.switch_layout(&id);
        }
    });
    (row, buttons)
}

fn spacer() -> gtk::Box {
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    spacer
}

/// Which inline layout form the manager shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayoutFormMode {
    New,
    Rename,
    Save,
}

/// Which action the preset dialog performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PresetMode {
    New,
    Reset,
}

/// Resolve a stable configuration name to a logical button.
fn remote_button(name: &str) -> Option<RemoteButton> {
    RemoteButton::ALL
        .into_iter()
        .find(|button| button.name() == name)
}

#[derive(Debug, Clone)]
struct TestResult {
    button: String,
    trigger: String,
    shown: String,
    action: Option<String>,
}

pub struct AppWindow {
    window: adw::ApplicationWindow,
    backend: Backend,
    model: RefCell<Option<GuiModel>>,
    status: RefCell<Option<DaemonStatus>>,
    title: adw::WindowTitle,
    device_chip: gtk::MenuButton,
    device_chip_name: gtk::Label,
    device_chip_icon: gtk::Image,
    device_chip_battery: gtk::Label,
    device_chip_battery_icon: gtk::Image,
    device_list: gtk::ListBox,
    device_info_button: gtk::Button,
    layout_chip: gtk::MenuButton,
    layout_chip_name: gtk::Label,
    layout_popover: gtk::Popover,
    layout_list: gtk::Box,
    layout_manage: gtk::Button,
    layout_rows: RefCell<BTreeMap<String, gtk::Button>>,
    layout_dialog: adw::Dialog,
    layout_dialog_title: adw::WindowTitle,
    layout_manager_list: gtk::ListBox,
    layout_manager_rows: RefCell<BTreeMap<String, adw::ActionRow>>,
    layout_actions: RefCell<BTreeMap<String, gtk::Button>>,
    layout_manager_hint: gtk::Label,
    layout_form: gtk::Box,
    layout_form_hint: gtk::Label,
    layout_name_row: adw::EntryRow,
    layout_desc_row: adw::EntryRow,
    layout_form_error: gtk::Label,
    layout_form_submit: gtk::Button,
    layout_form_cancel: gtk::Button,
    layout_manager_new: gtk::Button,
    layout_manager_preset_new: gtk::Button,
    layout_manager_apply: gtk::Button,
    layout_manager_revert: gtk::Button,
    layout_manager_close: gtk::Button,
    layout_form_mode: Cell<LayoutFormMode>,
    layout_form_target: RefCell<Option<String>>,
    preset_dialog: adw::Dialog,
    preset_dialog_title: adw::WindowTitle,
    preset_list: gtk::ListBox,
    preset_rows: RefCell<BTreeMap<&'static str, adw::ActionRow>>,
    preset_preview_rows: RefCell<BTreeMap<RemoteButton, adw::ActionRow>>,
    preset_name_row: adw::EntryRow,
    preset_name_list: gtk::ListBox,
    preset_warning: gtk::Label,
    preset_error: gtk::Label,
    preset_submit: gtk::Button,
    preset_cancel: gtk::Button,
    preset_mode: Cell<PresetMode>,
    preset_target: RefCell<Option<String>>,
    preset_selected: Cell<Option<&'static LayoutPreset>>,
    this: RefCell<Weak<AppWindow>>,
    board: MappingBoard,
    board_scroll: gtk::ScrolledWindow,
    test_bar: gtk::Box,
    test_title: gtk::Label,
    test_sub: gtk::Label,
    last_action_label: gtk::Label,
    run_once_button: gtk::Button,
    retry_button: gtk::Button,
    apply_button: gtk::Button,
    revert_button: gtk::Button,
    test_button: gtk::ToggleButton,
    error_banner: adw::Banner,
    dirty_banner: adw::Banner,
    capture_banner: adw::Banner,
    editor: ActionEditor,
    voice_editor: VoiceEditor,
    microphone: MicrophoneDialog,
    device_dialog: DeviceDialog,
    stack: gtk::Stack,
    toast_overlay: adw::ToastOverlay,
    capture_mode: Cell<CaptureMode>,
    suppress_test_toggle: Cell<bool>,
    /// One daemon status request at a time; the fast poll checks this.
    status_in_flight: Cell<bool>,
    /// Set while the 200 ms microphone poll timer runs.
    microphone_polling: Cell<bool>,
    voice_selected: Cell<bool>,
    pressed: Cell<Option<(RemoteButton, Trigger)>>,
    last_test: RefCell<Option<TestResult>>,
}

impl AppWindow {
    /// Build the window. `application` is `None` in display-only tests.
    #[must_use]
    pub fn build(application: Option<&adw::Application>, backend: Backend) -> Rc<Self> {
        let title = adw::WindowTitle::new(tr("ATVV Remote"), tr("Connecting…"));

        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&title));

        let menu = gio::Menu::new();
        menu.append(Some(tr("Microphone…")), Some("win.microphone"));
        menu.append(Some(tr("Device information…")), Some("win.device-info"));
        menu.append(Some(tr("Revert changes")), Some("win.revert"));
        menu.append(Some(tr("About ATVV")), Some("win.about"));
        menu.append(Some(tr("Quit")), Some("app.quit"));
        let menu_button = gtk::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .menu_model(&menu)
            .tooltip_text(tr("Main menu"))
            .build();
        header.pack_start(&menu_button);

        let device_popover = gtk::Popover::new();
        device_popover.set_size_request(340, -1);
        let device_pop_content = gtk::Box::new(gtk::Orientation::Vertical, 6);
        device_pop_content.set_margin_top(10);
        device_pop_content.set_margin_bottom(10);
        device_pop_content.set_margin_start(10);
        device_pop_content.set_margin_end(10);
        let device_pop_title = gtk::Label::new(Some(tr("Devices")));
        device_pop_title.add_css_class("heading");
        device_pop_title.set_halign(gtk::Align::Start);
        device_pop_content.append(&device_pop_title);
        let device_list = gtk::ListBox::new();
        device_list.set_selection_mode(gtk::SelectionMode::None);
        device_list.add_css_class("boxed-list");
        device_pop_content.append(&device_list);
        device_pop_content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        let device_info_button = gtk::Button::with_label(tr("Device information…"));
        device_info_button.add_css_class("flat");
        device_pop_content.append(&device_info_button);
        device_popover.set_child(Some(&device_pop_content));

        let device_chip = gtk::MenuButton::builder()
            .popover(&device_popover)
            .tooltip_text(tr("Devices"))
            .build();
        device_chip.add_css_class("flat");
        device_chip.add_css_class("device-chip");
        let chip_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let device_chip_icon = gtk::Image::from_icon_name("bluetooth-symbolic");
        device_chip_icon.set_pixel_size(13);
        let device_chip_name = gtk::Label::new(Some(tr("No remote")));
        device_chip_name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        device_chip_name.set_max_width_chars(22);
        let device_chip_battery_icon = gtk::Image::from_icon_name("battery-empty-symbolic");
        device_chip_battery_icon.set_pixel_size(16);
        device_chip_battery_icon.set_visible(false);
        let device_chip_battery = gtk::Label::new(None);
        device_chip_battery.add_css_class("caption");
        device_chip_battery.add_css_class("dim-label");
        chip_row.append(&device_chip_icon);
        chip_row.append(&device_chip_name);
        chip_row.append(&device_chip_battery_icon);
        chip_row.append(&device_chip_battery);
        device_chip.set_child(Some(&chip_row));
        header.pack_start(&device_chip);

        // Layout chip: the active layout, a direct switcher, and the manager.
        let layout_pop_content = gtk::Box::new(gtk::Orientation::Vertical, 4);
        layout_pop_content.set_margin_top(10);
        layout_pop_content.set_margin_bottom(10);
        layout_pop_content.set_margin_start(10);
        layout_pop_content.set_margin_end(10);
        let layout_pop_title = gtk::Label::new(Some(tr("Layouts")));
        layout_pop_title.add_css_class("heading");
        layout_pop_title.set_xalign(0.0);
        let layout_list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let layout_manage = gtk::Button::with_label(tr("Manage layouts…"));
        layout_manage.add_css_class("flat");
        layout_manage.add_css_class("layout-manage");
        layout_pop_content.append(&layout_pop_title);
        layout_pop_content.append(&layout_list);
        layout_pop_content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        layout_pop_content.append(&layout_manage);
        let layout_popover = gtk::Popover::new();
        layout_popover.set_has_arrow(true);
        layout_popover.set_child(Some(&layout_pop_content));
        let layout_chip = gtk::MenuButton::builder()
            .tooltip_text(tr("Switch layout"))
            .always_show_arrow(false)
            .popover(&layout_popover)
            .build();
        layout_chip.add_css_class("flat");
        layout_chip.add_css_class("layout-chip");
        let layout_chip_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let layout_chip_icon = gtk::Image::from_icon_name("view-list-symbolic");
        layout_chip_icon.set_pixel_size(13);
        let layout_chip_name = gtk::Label::new(Some(tr("Default")));
        layout_chip_name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        layout_chip_name.set_max_width_chars(16);
        layout_chip_row.append(&layout_chip_icon);
        layout_chip_row.append(&layout_chip_name);
        layout_chip.set_child(Some(&layout_chip_row));
        header.pack_start(&layout_chip);

        let test_button = gtk::ToggleButton::builder()
            .label(tr("Test"))
            .tooltip_text(tr("Highlight remote presses and show the mapped action"))
            .build();
        test_button.add_css_class("flat");
        let revert_button = gtk::Button::with_label(tr("Revert"));
        revert_button.add_css_class("flat");
        revert_button.set_sensitive(false);
        let apply_button = gtk::Button::with_label(tr("Apply"));
        apply_button.add_css_class("suggested-action");
        apply_button.add_css_class("apply");
        apply_button.set_sensitive(false);
        header.pack_end(&apply_button);
        header.pack_end(&revert_button);
        header.pack_end(&test_button);

        let error_banner = adw::Banner::new(tr("Daemon unavailable"));
        error_banner.set_button_label(Some(tr("Retry")));
        error_banner.set_revealed(false);
        let dirty_banner = adw::Banner::new(tr("Unsaved changes"));
        dirty_banner.set_button_label(Some(tr("Revert")));
        dirty_banner.set_revealed(false);
        let capture_banner = adw::Banner::new(tr("Press a remote key to test it"));
        capture_banner.set_button_label(Some(tr("Cancel")));
        capture_banner.set_revealed(false);
        let banners = gtk::Box::new(gtk::Orientation::Vertical, 0);
        banners.append(&error_banner);
        banners.append(&dirty_banner);
        banners.append(&capture_banner);

        let board = MappingBoard::new();
        let board_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .child(board.root())
            .build();

        // Bottom bar: capture state, the last resolved action, and Run once.
        let test_bar = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        test_bar.add_css_class("test-bar");
        let test_dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        test_dot.add_css_class("test-dot");
        test_dot.set_size_request(8, 8);
        test_dot.set_valign(gtk::Align::Center);
        let test_copy = gtk::Box::new(gtk::Orientation::Vertical, 0);
        test_copy.set_hexpand(true);
        let test_title = gtk::Label::new(Some(tr("Test mode off")));
        test_title.add_css_class("test-title");
        test_title.set_xalign(0.0);
        let test_sub = gtk::Label::new(Some(tr("Turn on Test to see what each key sends.")));
        test_sub.add_css_class("test-sub");
        test_sub.set_xalign(0.0);
        test_sub.set_ellipsize(gtk::pango::EllipsizeMode::End);
        test_copy.append(&test_title);
        test_copy.append(&test_sub);
        let last_action_label = gtk::Label::new(None);
        last_action_label.add_css_class("last-action");
        last_action_label.set_visible(false);
        last_action_label.set_tooltip_text(Some(tr("Last resolved action")));
        let run_once_button = gtk::Button::with_label(tr("Run once"));
        run_once_button.add_css_class("flat");
        run_once_button.set_sensitive(false);
        run_once_button.set_tooltip_text(Some(tr("Run the displayed action once through atvvd")));
        test_bar.append(&test_dot);
        test_bar.append(&test_copy);
        test_bar.append(&last_action_label);
        test_bar.append(&run_once_button);
        test_bar.set_visible(false);

        // Layout manager dialog.
        let layout_dialog_title = adw::WindowTitle::new(tr("Layouts"), "");
        let layout_dialog_header = adw::HeaderBar::new();
        layout_dialog_header.set_title_widget(Some(&layout_dialog_title));
        let layout_dialog_toolbar = adw::ToolbarView::new();
        layout_dialog_toolbar.add_top_bar(&layout_dialog_header);
        let layout_manager_list = gtk::ListBox::new();
        layout_manager_list.add_css_class("boxed-list");
        layout_manager_list.set_selection_mode(gtk::SelectionMode::None);
        let layout_name_row = adw::EntryRow::builder().title(tr("Name")).build();
        let layout_desc_row = adw::EntryRow::builder().title(tr("Description")).build();
        let layout_form_error = gtk::Label::new(None);
        layout_form_error.add_css_class("error");
        layout_form_error.set_xalign(0.0);
        layout_form_error.set_wrap(true);
        layout_form_error.set_visible(false);
        let layout_form_hint = gtk::Label::new(None);
        layout_form_hint.add_css_class("dim-label");
        layout_form_hint.add_css_class("caption");
        layout_form_hint.set_xalign(0.0);
        layout_form_hint.set_wrap(true);
        layout_form_hint.set_visible(false);
        let layout_form_buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        layout_form_buttons.set_halign(gtk::Align::End);
        let layout_form_cancel = gtk::Button::with_label(tr("Cancel"));
        layout_form_cancel.add_css_class("flat");
        let layout_form_submit = gtk::Button::with_label(tr("Create"));
        layout_form_submit.add_css_class("suggested-action");
        layout_form_buttons.append(&layout_form_cancel);
        layout_form_buttons.append(&layout_form_submit);
        let layout_form_fields = gtk::ListBox::new();
        layout_form_fields.add_css_class("boxed-list");
        layout_form_fields.set_selection_mode(gtk::SelectionMode::None);
        layout_form_fields.append(&layout_name_row);
        layout_form_fields.append(&layout_desc_row);
        let layout_form = gtk::Box::new(gtk::Orientation::Vertical, 6);
        layout_form.set_visible(false);
        layout_form.append(&layout_form_fields);
        layout_form.append(&layout_form_hint);
        layout_form.append(&layout_form_error);
        layout_form.append(&layout_form_buttons);
        let layout_manager_hint = gtk::Label::new(Some(tr(
            "The active layout drives the board and the input runtime. Default is a template: edit it and Apply saves the changes as a new layout.",
        )));
        layout_manager_hint.add_css_class("dim-label");
        layout_manager_hint.add_css_class("caption");
        layout_manager_hint.set_xalign(0.0);
        layout_manager_hint.set_wrap(true);
        let layout_dialog_body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        layout_dialog_body.set_margin_top(18);
        layout_dialog_body.set_margin_bottom(18);
        layout_dialog_body.set_margin_start(18);
        layout_dialog_body.set_margin_end(18);
        layout_dialog_body.append(&layout_manager_list);
        layout_dialog_body.append(&layout_form);
        layout_dialog_body.append(&layout_manager_hint);
        layout_dialog_toolbar.set_content(Some(&layout_dialog_body));
        let layout_manager_footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        layout_manager_footer.set_margin_top(8);
        layout_manager_footer.set_margin_bottom(8);
        layout_manager_footer.set_margin_start(12);
        layout_manager_footer.set_margin_end(12);
        let layout_manager_new = gtk::Button::with_label(tr("New layout…"));
        layout_manager_new.add_css_class("flat");
        let layout_manager_preset_new = gtk::Button::with_label(tr("New from preset…"));
        layout_manager_preset_new.add_css_class("flat");
        let footer_spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        footer_spacer.set_hexpand(true);
        let layout_manager_revert = gtk::Button::with_label(tr("Revert"));
        layout_manager_revert.add_css_class("flat");
        layout_manager_revert.set_visible(false);
        let layout_manager_apply = gtk::Button::with_label(tr("Apply"));
        layout_manager_apply.add_css_class("suggested-action");
        layout_manager_apply.set_visible(false);
        let layout_manager_close = gtk::Button::with_label(tr("Close"));
        layout_manager_close.add_css_class("flat");
        layout_manager_footer.append(&layout_manager_new);
        layout_manager_footer.append(&layout_manager_preset_new);
        layout_manager_footer.append(&footer_spacer);
        layout_manager_footer.append(&layout_manager_revert);
        layout_manager_footer.append(&layout_manager_apply);
        layout_manager_footer.append(&layout_manager_close);
        layout_dialog_toolbar.add_bottom_bar(&layout_manager_footer);
        let layout_dialog = adw::Dialog::new();
        layout_dialog.set_content_width(520);
        layout_dialog.set_content_height(560);
        layout_dialog.set_child(Some(&layout_dialog_toolbar));

        // Preset dialog: pick a built-in template, preview every slot, then
        // create a user layout or replace one.
        let preset_dialog_title = adw::WindowTitle::new(tr("New from preset"), "");
        let preset_header = adw::HeaderBar::new();
        preset_header.set_title_widget(Some(&preset_dialog_title));
        let preset_list = gtk::ListBox::new();
        preset_list.add_css_class("boxed-list");
        preset_list.set_selection_mode(gtk::SelectionMode::Single);
        let mut preset_rows = BTreeMap::new();
        for preset in &LAYOUT_PRESETS {
            let row = adw::ActionRow::builder()
                .title(tr(preset.name))
                .subtitle(format!(
                    "{} · {}",
                    tr(preset.description),
                    preset.verified_with
                ))
                .activatable(true)
                .build();
            row.set_widget_name(&format!("preset-row-{}", preset.id));
            preset_list.append(&row);
            preset_rows.insert(preset.id, row);
        }
        let preset_preview = gtk::ListBox::new();
        preset_preview.add_css_class("boxed-list");
        preset_preview.set_selection_mode(gtk::SelectionMode::None);
        let mut preset_preview_rows = BTreeMap::new();
        for button in RemoteButton::LOGICAL {
            let row = adw::ActionRow::builder()
                .title(button_label(button))
                .build();
            row.set_widget_name(&format!("preset-preview-{}", button.name()));
            preset_preview.append(&row);
            preset_preview_rows.insert(button, row);
        }
        let preset_preview_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .min_content_height(220)
            .vexpand(true)
            .child(&preset_preview)
            .build();
        let preset_preview_title = gtk::Label::new(Some(tr("Slots")));
        preset_preview_title.add_css_class("heading");
        preset_preview_title.set_xalign(0.0);
        let preset_name_row = adw::EntryRow::builder().title(tr("Name")).build();
        let preset_name_list = gtk::ListBox::new();
        preset_name_list.add_css_class("boxed-list");
        preset_name_list.set_selection_mode(gtk::SelectionMode::None);
        preset_name_list.append(&preset_name_row);
        let preset_warning = gtk::Label::new(None);
        preset_warning.add_css_class("dim-label");
        preset_warning.add_css_class("caption");
        preset_warning.set_xalign(0.0);
        preset_warning.set_wrap(true);
        preset_warning.set_visible(false);
        let preset_error = gtk::Label::new(None);
        preset_error.add_css_class("error");
        preset_error.set_xalign(0.0);
        preset_error.set_wrap(true);
        preset_error.set_visible(false);
        let preset_body = gtk::Box::new(gtk::Orientation::Vertical, 10);
        preset_body.set_margin_top(18);
        preset_body.set_margin_bottom(18);
        preset_body.set_margin_start(18);
        preset_body.set_margin_end(18);
        preset_body.append(&preset_list);
        preset_body.append(&preset_preview_title);
        preset_body.append(&preset_preview_scroll);
        preset_body.append(&preset_name_list);
        preset_body.append(&preset_warning);
        preset_body.append(&preset_error);
        let preset_toolbar = adw::ToolbarView::new();
        preset_toolbar.add_top_bar(&preset_header);
        preset_toolbar.set_content(Some(&preset_body));
        let preset_cancel = gtk::Button::with_label(tr("Cancel"));
        preset_cancel.add_css_class("flat");
        let preset_submit = gtk::Button::with_label(tr("Create"));
        preset_submit.add_css_class("suggested-action");
        let preset_footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        preset_footer.set_margin_top(8);
        preset_footer.set_margin_bottom(8);
        preset_footer.set_margin_start(12);
        preset_footer.set_margin_end(12);
        let preset_footer_spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        preset_footer_spacer.set_hexpand(true);
        preset_footer.append(&preset_cancel);
        preset_footer.append(&preset_footer_spacer);
        preset_footer.append(&preset_submit);
        preset_toolbar.add_bottom_bar(&preset_footer);
        let preset_dialog = adw::Dialog::new();
        preset_dialog.set_content_width(560);
        preset_dialog.set_content_height(620);
        preset_dialog.set_child(Some(&preset_toolbar));

        let unavailable = adw::StatusPage::builder()
            .icon_name("network-offline-symbolic")
            .title(tr("Daemon unavailable"))
            .description(tr(
                "Start the ATVV daemon with “systemctl --user start atvvd”, then retry.",
            ))
            .build();
        let retry = gtk::Button::with_label(tr("Retry"));
        retry.add_css_class("suggested-action");
        retry.set_halign(gtk::Align::Center);
        unavailable.set_child(Some(&retry));

        let stack = gtk::Stack::new();
        stack.set_vexpand(true);
        stack.add_named(&unavailable, Some("unavailable"));
        stack.add_named(&board_scroll, Some("config"));
        stack.set_visible_child_name("unavailable");

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.add_top_bar(&banners);
        toolbar.set_content(Some(&stack));
        toolbar.add_bottom_bar(&test_bar);
        toolbar.set_vexpand(true);

        let toast_overlay = adw::ToastOverlay::new();
        toast_overlay.set_child(Some(&toolbar));

        let mut builder = adw::ApplicationWindow::builder()
            .title(tr("ATVV Remote"))
            .default_width(1040)
            .default_height(760)
            .width_request(360)
            .height_request(420);
        if let Some(application) = application {
            builder = builder.application(application);
        }
        let window = builder.build();
        window.set_icon_name(Some(APP_ID));
        window.set_content(Some(&toast_overlay));

        let ui = Rc::new(Self {
            window,
            backend,
            model: RefCell::new(None),
            status: RefCell::new(None),
            title,
            device_chip,
            device_chip_name,
            device_chip_icon,
            device_chip_battery,
            device_chip_battery_icon,
            device_list,
            device_info_button,
            layout_chip,
            layout_chip_name,
            layout_popover,
            layout_list,
            layout_manage,
            layout_rows: RefCell::new(BTreeMap::new()),
            layout_dialog,
            layout_dialog_title,
            layout_manager_list,
            layout_manager_rows: RefCell::new(BTreeMap::new()),
            layout_actions: RefCell::new(BTreeMap::new()),
            layout_manager_hint,
            layout_form,
            layout_form_hint,
            layout_name_row,
            layout_desc_row,
            layout_form_error,
            layout_form_submit,
            layout_form_cancel,
            layout_manager_new,
            layout_manager_preset_new,
            layout_manager_apply,
            layout_manager_revert,
            layout_manager_close,
            layout_form_mode: Cell::new(LayoutFormMode::New),
            layout_form_target: RefCell::new(None),
            preset_dialog,
            preset_dialog_title,
            preset_list,
            preset_rows: RefCell::new(preset_rows),
            preset_preview_rows: RefCell::new(preset_preview_rows),
            preset_name_row,
            preset_name_list,
            preset_warning,
            preset_error,
            preset_submit,
            preset_cancel,
            preset_mode: Cell::new(PresetMode::New),
            preset_target: RefCell::new(None),
            preset_selected: Cell::new(None),
            this: RefCell::new(Weak::new()),
            board,
            board_scroll,
            test_bar,
            test_title,
            test_sub,
            last_action_label,
            run_once_button,
            retry_button: retry,
            apply_button,
            revert_button,
            test_button,
            error_banner,
            dirty_banner,
            capture_banner,
            editor: ActionEditor::new(),
            voice_editor: VoiceEditor::new(),
            microphone: MicrophoneDialog::new(),
            device_dialog: DeviceDialog::new(),
            stack,
            toast_overlay,
            capture_mode: Cell::new(CaptureMode::Off),
            suppress_test_toggle: Cell::new(false),
            status_in_flight: Cell::new(false),
            microphone_polling: Cell::new(false),
            voice_selected: Cell::new(false),
            pressed: Cell::new(None),
            last_test: RefCell::new(None),
        });
        *ui.this.borrow_mut() = Rc::downgrade(&ui);
        Self::connect_events(&ui);
        connect_timers(&ui);
        ui.refresh_all();
        ui
    }

    /// Load the current document and start polling the daemon.
    pub fn begin(&self) {
        self.backend.load();
    }

    /// Drain worker results; the main-loop timer calls this.
    pub fn poll_events(&self) {
        while let Some(event) = self.backend.try_recv() {
            self.handle_event(event);
        }
    }

    fn connect_events(ui: &Rc<Self>) {
        let this = Rc::clone(ui);
        ui.board
            .connect_slot(move |button, trigger| this.open_slot(button, trigger));

        let this = Rc::clone(ui);
        ui.board.diagram().connect_selected(move |button| {
            if button == RemoteButton::Voice {
                this.open_voice();
            } else {
                this.open_slot(button, Trigger::Single);
            }
        });
        let this = Rc::clone(ui);
        ui.board.connect_voice(move || this.open_voice());

        let this = Rc::clone(ui);
        ui.device_info_button.connect_clicked(move |_| {
            this.device_chip.popdown();
            this.device_dialog.present(&this.window);
        });

        let this = Rc::clone(ui);
        ui.device_list.connect_row_activated(move |_, row| {
            let address = row.widget_name().to_string();
            let address = address.strip_prefix("device-row-").unwrap_or_default();
            if address.is_empty() {
                return;
            }
            let dirty = this
                .model
                .borrow()
                .as_ref()
                .is_some_and(|model| model.dirty);
            if dirty {
                this.toast(tr("Apply or revert before switching devices."));
                return;
            }
            let active = this
                .model
                .borrow()
                .as_ref()
                .and_then(|model| model.document.active_device.as_deref())
                == Some(address);
            this.device_chip.popdown();
            if !active {
                this.backend.select_device(address.to_string());
            }
        });

        let this = Rc::clone(ui);
        ui.layout_manage.connect_clicked(move |_| {
            this.layout_popover.popdown();
            this.open_layout_manager();
        });
        let this = Rc::clone(ui);
        ui.layout_manager_new
            .connect_clicked(move |_| this.show_layout_form(LayoutFormMode::New, None));
        let this = Rc::clone(ui);
        ui.layout_manager_preset_new
            .connect_clicked(move |_| this.open_preset_dialog(PresetMode::New, None));
        let this = Rc::clone(ui);
        ui.preset_cancel.connect_clicked(move |_| {
            this.preset_dialog.force_close();
        });
        let this = Rc::clone(ui);
        ui.preset_submit
            .connect_clicked(move |_| this.submit_preset());
        let this = Rc::clone(ui);
        ui.preset_list.connect_row_selected(move |_, row| {
            let Some(row) = row else {
                return;
            };
            let name = row.widget_name();
            let Some(id) = name.strip_prefix("preset-row-") else {
                return;
            };
            if let Some(preset) = atvv_config::layout_preset(id) {
                this.select_preset(preset);
            }
        });
        let this = Rc::clone(ui);
        ui.layout_form_cancel
            .connect_clicked(move |_| this.hide_layout_form());
        let this = Rc::clone(ui);
        ui.layout_form_submit
            .connect_clicked(move |_| this.submit_layout_form());
        let this = Rc::clone(ui);
        ui.layout_manager_apply
            .connect_clicked(move |_| this.apply());
        let this = Rc::clone(ui);
        ui.layout_manager_revert
            .connect_clicked(move |_| this.revert());
        let this = Rc::clone(ui);
        ui.layout_manager_close.connect_clicked(move |_| {
            this.layout_dialog.close();
        });

        for name in [
            "layout-switch",
            "layout-duplicate",
            "layout-rename",
            "layout-delete",
            "layout-reset",
            "layout-preset-reset",
        ] {
            let action = gio::SimpleAction::new(name, Some(glib::VariantTy::STRING));
            let this = Rc::clone(ui);
            action.connect_activate(move |_, parameter| {
                let Some(id) = parameter.and_then(|value| value.str()) else {
                    return;
                };
                match name {
                    "layout-switch" => this.switch_layout(id),
                    "layout-duplicate" => this.duplicate_layout(id),
                    "layout-rename" => {
                        this.open_layout_manager();
                        this.show_layout_form(LayoutFormMode::Rename, Some(id.to_string()));
                    }
                    "layout-delete" => this.delete_layout(id),
                    "layout-reset" => this.reset_layout(id),
                    "layout-preset-reset" => {
                        this.open_preset_dialog(PresetMode::Reset, Some(id.to_string()));
                    }
                    _ => {}
                }
            });
            ui.window.add_action(&action);
        }

        let this = Rc::clone(ui);
        ui.apply_button.connect_clicked(move |_| this.apply());
        let this = Rc::clone(ui);
        ui.revert_button.connect_clicked(move |_| this.revert());
        let this = Rc::clone(ui);
        ui.test_button.connect_toggled(move |button| {
            if this.suppress_test_toggle.get() {
                return;
            }
            if button.is_active() {
                this.start_capture(CaptureMode::Test);
            } else if this.capture_mode.get() == CaptureMode::Test {
                this.stop_capture();
            }
        });

        let this = Rc::clone(ui);
        ui.error_banner.connect_button_clicked(move |_| {
            if this.model.borrow().is_some() {
                this.backend.refresh_status();
            } else {
                this.backend.load();
            }
        });
        let this = Rc::clone(ui);
        ui.dirty_banner
            .connect_button_clicked(move |_| this.revert());
        let this = Rc::clone(ui);
        ui.capture_banner
            .connect_button_clicked(move |_| this.stop_capture());

        let this = Rc::clone(ui);
        ui.editor.connect_update(move || this.editor_update());
        let this = Rc::clone(ui);
        ui.editor.connect_reset(move || this.editor_reset());
        let this = Rc::clone(ui);
        ui.editor.connect_cancel(move || this.editor.close());
        let this = Rc::clone(ui);
        ui.editor
            .connect_learn_key(move || this.learn_key_for(false));
        let this = Rc::clone(ui);
        ui.editor.connect_learn_remote(move || this.learn_remote());
        let this = Rc::clone(ui);
        ui.editor
            .connect_allow_exec(move |allow| this.editor_allow_exec(allow));

        let this = Rc::clone(ui);
        ui.voice_editor.connect_update(move || this.voice_update());
        let this = Rc::clone(ui);
        ui.voice_editor.connect_reset(move || this.voice_reset());
        let this = Rc::clone(ui);
        ui.voice_editor
            .connect_cancel(move || this.voice_editor.close());
        let this = Rc::clone(ui);
        ui.voice_editor
            .connect_learn_key(move || this.learn_key_for(true));

        let this = Rc::clone(ui);
        ui.microphone
            .connect_gain_changed(move |gain| this.set_gain(gain));

        let this = Rc::clone(ui);
        ui.run_once_button.connect_clicked(move |_| this.run_once());

        let this = Rc::clone(ui);
        ui.window.connect_close_request(move |_| {
            this.stop_capture();
            glib::Propagation::Proceed
        });

        let this = Rc::clone(ui);
        let revert = gio::SimpleAction::new("revert", None);
        revert.connect_activate(move |_, _| this.revert());
        ui.window.add_action(&revert);

        let this = Rc::clone(ui);
        let voice_key = gio::SimpleAction::new("voice-key", Some(glib::VariantTy::STRING));
        voice_key.connect_activate(move |_, parameter| {
            if let Some(key) = parameter.and_then(|value| value.str()) {
                this.voice_editor.set_value(key);
            }
        });
        ui.window.add_action(&voice_key);

        let this = Rc::clone(ui);
        let device_info = gio::SimpleAction::new("device-info", None);
        device_info.connect_activate(move |_, _| {
            this.device_dialog.present(&this.window);
        });
        ui.window.add_action(&device_info);

        let this = Rc::clone(ui);
        let microphone = gio::SimpleAction::new("microphone", None);
        microphone.connect_activate(move |_, _| this.open_microphone());
        ui.window.add_action(&microphone);

        let this = Rc::clone(ui);
        let about = gio::SimpleAction::new("about", None);
        about.connect_activate(move |_, _| {
            let dialog = this.about_dialog();
            dialog.present(Some(&this.window));
        });
        ui.window.add_action(&about);

        let this = Rc::clone(ui);
        ui.retry_button
            .connect_clicked(move |_| this.backend.load());
    }

    fn open_slot(&self, button: RemoteButton, trigger: Trigger) {
        self.voice_selected.set(false);
        {
            let mut model = self.model.borrow_mut();
            let Some(model) = model.as_mut() else {
                return;
            };
            model.selected = button;
            model.trigger = trigger;
        }
        self.refresh_all();
        self.scroll_to_selection();
        self.show_editor();
    }

    fn show_editor(&self) {
        let model = self.model.borrow();
        let Some(model) = model.as_ref() else {
            return;
        };
        self.editor.load(model);
        self.editor.present(&self.window);
    }

    fn reload_editor(&self) {
        if !self.editor.is_open() {
            return;
        }
        let model = self.model.borrow();
        let Some(model) = model.as_ref() else {
            return;
        };
        self.editor.load(model);
    }

    fn open_voice(&self) {
        self.voice_selected.set(true);
        self.refresh_all();
        self.scroll_to_selection();
        let model = self.model.borrow();
        let Some(model) = model.as_ref() else {
            return;
        };
        self.voice_editor.load(model);
        if self.voice_editor.is_open() {
            return;
        }
        self.voice_editor.present(&self.window);
    }

    /// Open the level/gain dialog and start its 200 ms status poll.
    #[doc(hidden)]
    pub fn open_microphone(self: &Rc<Self>) {
        {
            let model = self.model.borrow();
            let Some(model) = model.as_ref() else {
                return;
            };
            self.microphone.load(model);
        }
        self.microphone.present(&self.window);
        self.refresh_microphone();
        start_microphone_poll(self);
    }

    /// Apply a gain value from the microphone dialog to the editable document.
    fn set_gain(&self, gain_db: i8) {
        let result = self
            .model
            .borrow_mut()
            .as_mut()
            .map(|model| model.set_gain_db(gain_db));
        match result {
            Some(Ok(true)) => self.refresh_all(),
            Some(Err(error)) => self.toast(&error),
            Some(Ok(false)) | None => {}
        }
    }

    fn scroll_to_selection(&self) {
        if self.voice_selected.get() {
            scroll_into_view(&self.board_scroll, self.board.voice_card());
            return;
        }
        let model = self.model.borrow();
        let Some(model) = model.as_ref() else {
            return;
        };
        if let Some(slot) = self.board.slot(model.selected, model.trigger) {
            scroll_into_view(&self.board_scroll, slot);
        }
    }

    fn handle_event(&self, event: Event) {
        match event {
            Event::Loaded { document, status } => match GuiModel::new(document) {
                Ok(model) => {
                    *self.model.borrow_mut() = Some(model);
                    *self.status.borrow_mut() = Some(status);
                    self.stack.set_visible_child_name("config");
                    self.error_banner.set_revealed(false);
                    self.refresh_all();
                    self.reload_editor();
                }
                Err(error) => self.show_error(&error),
            },
            Event::LoadFailed(error) => {
                if self.model.borrow().is_some() {
                    self.show_error(&tr_args(
                        "Cannot read the configuration: {error}",
                        &[("error", &error)],
                    ));
                } else {
                    self.stack.set_visible_child_name("unavailable");
                    self.title.set_subtitle(tr("Daemon unavailable"));
                }
            }
            Event::Status(Ok(status)) => {
                self.status_in_flight.set(false);
                *self.status.borrow_mut() = Some(status);
                self.refresh_title();
                self.refresh_device_chip();
                self.refresh_device_dialog();
                self.refresh_voice_controls();
                self.refresh_microphone();
            }
            Event::Status(Err(error)) => {
                self.status_in_flight.set(false);
                if self.model.borrow().is_some() {
                    self.show_error(&tr_args(
                        "atvvd is not reachable: {error}",
                        &[("error", &error)],
                    ));
                }
            }
            Event::Saved(Ok(())) => {
                if let Some(model) = self.model.borrow_mut().as_mut() {
                    model.mark_saved();
                }
                self.toast(tr("Configuration applied"));
                self.backend.load();
                self.refresh_all();
            }
            Event::Saved(Err(error)) => {
                self.show_error(&tr_args(
                    "Cannot apply the configuration: {error}",
                    &[("error", &error)],
                ));
                self.refresh_all();
            }
            Event::LayoutSwitched(Ok(())) => {
                *self.last_test.borrow_mut() = None;
                self.pressed.set(None);
                self.toast(tr("Layout switched"));
                self.backend.load();
            }
            Event::LayoutChanged(id) => {
                let reload = self
                    .model
                    .borrow()
                    .as_ref()
                    .is_some_and(|model| !model.dirty && model.active_layout() != id);
                if reload {
                    self.backend.load();
                }
            }
            Event::DeviceSwitched(Ok(())) => {
                self.force_capture_off();
                self.toast(tr("Device switched"));
                self.backend.load();
            }
            Event::DeviceSwitched(Err(error)) => {
                self.show_error(&tr_args(
                    "Cannot switch devices: {error}",
                    &[("error", &error)],
                ));
                self.backend.load();
            }
            Event::DeviceChanged(_) => {
                self.force_capture_off();
                self.backend.load();
            }
            Event::LayoutSwitched(Err(error)) => {
                self.show_error(&tr_args(
                    "Cannot switch layouts: {error}",
                    &[("error", &error)],
                ));
                self.backend.load();
            }
            Event::CaptureStarted(Ok(())) => {
                self.error_banner.set_revealed(false);
            }
            Event::CaptureStarted(Err(error)) => {
                self.force_capture_off();
                self.show_error(&tr_args(
                    "Cannot start capture mode: {error}",
                    &[("error", &error)],
                ));
            }
            Event::CaptureEnded(_) => self.force_capture_off(),
            Event::ActionRun(Ok(())) => self.toast(tr("Action sent")),
            Event::ActionRun(Err(error)) => {
                self.show_error(&tr_args("Action failed: {error}", &[("error", &error)]))
            }
            Event::Button(event) => self.on_button_event(event),
            Event::SignalStreamEnded => {}
        }
    }

    fn on_button_event(&self, event: ButtonSignal) {
        let Some(button) = RemoteButton::ALL
            .iter()
            .copied()
            .find(|button| button.name() == event.button)
        else {
            return;
        };
        match self.capture_mode.get() {
            CaptureMode::Learn => {
                if event.pressed {
                    let trigger = trigger_from_name(&event.trigger);
                    self.stop_capture();
                    if button == RemoteButton::Voice {
                        self.voice_selected.set(true);
                        self.refresh_all();
                        self.reload_voice_editor();
                        self.scroll_to_selection();
                        self.toast(tr("Recording key selected"));
                        return;
                    }
                    let trigger = trigger.unwrap_or(Trigger::Single);
                    self.voice_selected.set(false);
                    if let Some(model) = self.model.borrow_mut().as_mut() {
                        model.selected = button;
                        model.trigger = trigger;
                    }
                    self.refresh_all();
                    self.reload_editor();
                    self.scroll_to_selection();
                    let label = self.model.borrow().as_ref().map_or_else(
                        || button_label(button),
                        |model| button_label_for(&model.profile, button),
                    );
                    self.toast(&tr_args(
                        "Selected {button} · {trigger}",
                        &[("button", label), ("trigger", trigger_label(trigger))],
                    ));
                }
            }
            CaptureMode::Test => {
                if event.pressed {
                    let trigger = trigger_from_name(&event.trigger);
                    if let Some(trigger) = trigger
                        && button != RemoteButton::Voice
                        && let Some(model) = self.model.borrow_mut().as_mut()
                    {
                        model.selected = button;
                        model.trigger = trigger;
                    }
                    self.pressed.set(trigger.map(|trigger| (button, trigger)));
                    let action = if button == RemoteButton::Voice {
                        None
                    } else {
                        self.model
                            .borrow()
                            .as_ref()
                            .and_then(|model| model.action_for_event(button, &event.trigger))
                    };
                    let shown = if button == RemoteButton::Voice {
                        tr("Microphone key (not remappable)").to_string()
                    } else {
                        action_label(action.as_deref()).text
                    };
                    let trigger = trigger.map_or_else(
                        || event.trigger.clone(),
                        |trigger| trigger_label(trigger).to_string(),
                    );
                    let label = self.model.borrow().as_ref().map_or_else(
                        || button_label(button),
                        |model| button_label_for(&model.profile, button),
                    );
                    *self.last_test.borrow_mut() = Some(TestResult {
                        button: label.to_string(),
                        trigger,
                        shown,
                        action,
                    });
                    self.refresh_board();
                    self.refresh_test_bar();
                    self.scroll_to_selection();
                } else {
                    self.pressed.set(None);
                    self.refresh_board();
                }
            }
            CaptureMode::Off => {}
        }
    }

    fn reload_voice_editor(&self) {
        let model = self.model.borrow();
        let Some(model) = model.as_ref() else {
            return;
        };
        self.voice_editor.load(model);
        if !self.voice_editor.is_open() {
            self.voice_editor.present(&self.window);
        }
    }

    fn start_capture(&self, mode: CaptureMode) {
        if self.capture_mode.get() != CaptureMode::Off {
            self.backend.end_capture();
        }
        self.capture_mode.set(mode);
        self.backend.begin_capture();
        self.capture_banner.set_title(match mode {
            CaptureMode::Test => tr("Press a remote key to test it"),
            CaptureMode::Learn => tr("Press a remote key to select it"),
            CaptureMode::Off => "",
        });
        self.capture_banner.set_revealed(true);
        if mode == CaptureMode::Test {
            *self.last_test.borrow_mut() = None;
            self.pressed.set(None);
            self.refresh_board();
        }
        self.refresh_test_bar();
    }

    fn stop_capture(&self) {
        if self.capture_mode.get() == CaptureMode::Off {
            return;
        }
        self.backend.end_capture();
        self.force_capture_off();
    }

    fn force_capture_off(&self) {
        self.capture_mode.set(CaptureMode::Off);
        self.capture_banner.set_revealed(false);
        self.pressed.set(None);
        self.suppress_test_toggle.set(true);
        self.test_button.set_active(false);
        self.suppress_test_toggle.set(false);
        self.refresh_board();
        self.refresh_test_bar();
    }

    fn apply(&self) {
        let (template_changed, document) = {
            let model = self.model.borrow();
            let Some(model) = model.as_ref() else {
                return;
            };
            if let Err(error) = model.validate() {
                self.show_error(&error);
                return;
            }
            (model.template_changed(), model.document.clone())
        };
        if template_changed {
            // The template's changes become a new layout instead of being
            // written into the document.
            self.open_layout_manager();
            self.show_layout_form(LayoutFormMode::Save, None);
            return;
        }
        self.apply_button.set_sensitive(false);
        self.error_banner.set_revealed(false);
        self.backend.save(document);
    }

    /// Switch the active layout; a dirty document is refused by the model.
    fn switch_layout(&self, id: &str) {
        let result = self
            .model
            .borrow_mut()
            .as_mut()
            .map(|model| model.switch_layout(id));
        match result {
            None => {}
            Some(Err(error)) => self.toast(&error),
            Some(Ok(false)) => self.layout_popover.popdown(),
            Some(Ok(true)) => {
                self.layout_popover.popdown();
                *self.last_test.borrow_mut() = None;
                self.pressed.set(None);
                self.refresh_all();
                self.reload_editor();
                self.backend.set_layout(id.to_string());
            }
        }
    }

    fn duplicate_layout(&self, id: &str) {
        let result = self
            .model
            .borrow_mut()
            .as_mut()
            .map(|model| model.duplicate_layout(id));
        match result {
            Some(Ok(_)) => {
                self.toast(tr("Layout duplicated"));
                self.refresh_all();
            }
            Some(Err(error)) => self.toast(&error),
            None => {}
        }
    }

    fn delete_layout(&self, id: &str) {
        let result = self
            .model
            .borrow_mut()
            .as_mut()
            .map(|model| model.delete_layout(id));
        match result {
            Some(Ok(())) => {
                self.toast(tr("Layout deleted"));
                self.refresh_all();
            }
            Some(Err(error)) => self.toast(&error),
            None => {}
        }
    }

    fn reset_layout(&self, id: &str) {
        let result = self
            .model
            .borrow_mut()
            .as_mut()
            .map(|model| model.reset_layout(id));
        match result {
            Some(Ok(())) => {
                self.toast(tr("Layout reset"));
                self.refresh_all();
            }
            Some(Err(error)) => self.toast(&error),
            None => {}
        }
    }

    fn open_layout_manager(&self) {
        self.layout_popover.popdown();
        self.layout_dialog.present(Some(&self.window));
    }

    fn open_preset_dialog(&self, mode: PresetMode, target: Option<String>) {
        self.preset_mode.set(mode);
        *self.preset_target.borrow_mut() = target.clone();
        self.preset_dialog_title.set_title(match mode {
            PresetMode::New => tr("New from preset"),
            PresetMode::Reset => tr("Reset to preset"),
        });
        self.preset_submit.set_label(match mode {
            PresetMode::New => tr("Create"),
            PresetMode::Reset => tr("Reset"),
        });
        self.preset_name_row.set_visible(mode == PresetMode::New);
        self.preset_name_list.set_visible(mode == PresetMode::New);
        self.preset_warning.set_visible(mode == PresetMode::Reset);
        self.preset_error.set_visible(false);
        self.preset_error.set_text("");
        if mode == PresetMode::Reset {
            let name = target.as_deref().map_or_else(String::new, |id| {
                self.model
                    .borrow()
                    .as_ref()
                    .map_or_else(|| id.to_string(), |model| model.layout_name(id))
            });
            self.preset_warning.set_text(&tr_args(
                "This replaces every slot of “{name}”. The layout keeps its name and description.",
                &[("name", name.as_str())],
            ));
        }
        self.refresh_preset_preview();
        if let Some(preset) = LAYOUT_PRESETS.first() {
            self.select_preset(preset);
            if let Some(row) = self.preset_rows.borrow().get(preset.id) {
                self.preset_list.select_row(Some(row));
            }
        }
        self.preset_dialog.present(Some(&self.window));
    }

    /// Show only the active device's buttons in the preset preview. Slots the
    /// preset does not define stay blank: the profile default still applies.
    fn refresh_preset_preview(&self) {
        let profile = self.model.borrow().as_ref().map(|model| model.profile);
        for (button, row) in self.preset_preview_rows.borrow().iter() {
            row.set_visible(profile.is_some_and(|profile| profile.has_button(*button)));
            row.set_title(profile.map_or_else(
                || button_label(*button),
                |profile| button_label_for(&profile, *button),
            ));
            row.set_subtitle("");
        }
    }

    fn select_preset(&self, preset: &'static LayoutPreset) {
        self.preset_selected.set(Some(preset));
        let profile = self.model.borrow().as_ref().map(|model| model.profile);
        for row in self.preset_preview_rows.borrow().values() {
            row.set_subtitle("");
        }
        for (name, action) in preset.slots {
            let Some(button) = remote_button(name) else {
                continue;
            };
            if profile.is_some_and(|profile| !profile.has_button(button)) {
                continue;
            }
            if let Some(row) = self.preset_preview_rows.borrow().get(&button) {
                row.set_subtitle(&action_label(Some(action)).text);
            }
        }
        if self.preset_mode.get() == PresetMode::New {
            let existing: Vec<String> =
                self.model.borrow().as_ref().map_or_else(Vec::new, |model| {
                    model
                        .document
                        .layouts
                        .values()
                        .map(|layout| layout.name.clone())
                        .collect()
                });
            let name = unique_layout_name(tr(preset.name), existing.iter().map(String::as_str));
            self.preset_name_row.set_text(&name);
        }
    }

    fn submit_preset(&self) {
        let Some(preset) = self.preset_selected.get() else {
            return;
        };
        match self.preset_mode.get() {
            PresetMode::New => {
                let name = self.preset_name_row.text().to_string();
                let result = self
                    .model
                    .borrow_mut()
                    .as_mut()
                    .ok_or_else(|| tr("no configuration loaded").to_string())
                    .and_then(|model| model.create_layout_from_preset(preset, &name));
                match result {
                    Ok(_) => {
                        self.preset_dialog.force_close();
                        self.refresh_all();
                        self.toast(tr("Layout created"));
                    }
                    Err(error) => {
                        self.preset_error.set_text(&error);
                        self.preset_error.set_visible(true);
                    }
                }
            }
            PresetMode::Reset => {
                let Some(target) = self.preset_target.borrow().clone() else {
                    return;
                };
                let name = self
                    .model
                    .borrow()
                    .as_ref()
                    .map_or_else(|| target.clone(), |model| model.layout_name(&target));
                let body = tr_args(
                    "This replaces every slot of “{name}”. The layout keeps its name and description.",
                    &[("name", name.as_str())],
                );
                let dialog =
                    adw::AlertDialog::new(Some(tr("Reset to preset?")), Some(body.as_str()));
                dialog.add_response("cancel", tr("Cancel"));
                dialog.add_response("reset", tr("Reset"));
                dialog.set_response_appearance("reset", adw::ResponseAppearance::Destructive);
                dialog.set_default_response(Some("cancel"));
                dialog.set_close_response("cancel");
                let this = self.this.borrow().clone();
                let preset_id = preset.id;
                dialog.connect_response(None, move |_, response| {
                    if response != "reset" {
                        return;
                    }
                    let Some(this) = this.upgrade() else {
                        return;
                    };
                    let Some(preset) = atvv_config::layout_preset(preset_id) else {
                        return;
                    };
                    let result = this
                        .model
                        .borrow_mut()
                        .as_mut()
                        .ok_or_else(|| tr("no configuration loaded").to_string())
                        .and_then(|model| model.reset_layout_to_preset(&target, preset));
                    match result {
                        Ok(()) => {
                            this.preset_dialog.force_close();
                            this.refresh_all();
                            this.toast(tr("Layout reset"));
                        }
                        Err(error) => this.toast(&error),
                    }
                });
                dialog.present(Some(&self.window));
            }
        }
    }

    fn hide_layout_form(&self) {
        self.layout_form.set_visible(false);
        self.layout_manager_list.set_visible(true);
        self.layout_manager_new.set_visible(true);
        self.layout_manager_hint.set_visible(true);
        self.layout_form_error.set_visible(false);
        self.layout_form_hint.set_visible(false);
        self.layout_dialog_title.set_title(tr("Layouts"));
        if let Some(model) = self.model.borrow().as_ref() {
            self.refresh_layout_footer(model);
        }
    }

    fn show_layout_form(&self, mode: LayoutFormMode, target: Option<String>) {
        let model = self.model.borrow();
        let Some(model) = model.as_ref() else {
            return;
        };
        self.layout_form_mode.set(mode);
        *self.layout_form_target.borrow_mut() = target.clone();
        self.layout_manager_list.set_visible(false);
        self.layout_form.set_visible(true);
        self.layout_manager_new.set_visible(false);
        self.layout_manager_hint.set_visible(false);
        self.layout_form_error.set_visible(false);
        self.layout_form_error.set_text("");
        self.layout_form_hint
            .set_visible(mode == LayoutFormMode::Save);
        self.layout_dialog_title.set_title(match mode {
            LayoutFormMode::New => tr("New layout"),
            LayoutFormMode::Rename => tr("Rename layout"),
            LayoutFormMode::Save => tr("Save as layout"),
        });
        self.layout_form_submit.set_label(match mode {
            LayoutFormMode::New => tr("Create"),
            LayoutFormMode::Rename | LayoutFormMode::Save => tr("Save"),
        });
        let (name, description) = match mode {
            LayoutFormMode::New => (String::new(), None),
            LayoutFormMode::Rename => target
                .as_deref()
                .and_then(|id| model.document.layouts.get(id))
                .map_or_else(Default::default, |layout| {
                    (layout.name.clone(), layout.description.clone())
                }),
            LayoutFormMode::Save => {
                let existing: Vec<String> = model
                    .document
                    .layouts
                    .values()
                    .map(|layout| layout.name.clone())
                    .collect();
                (
                    unique_layout_name(tr("My layout"), existing.iter().map(String::as_str)),
                    None,
                )
            }
        };
        self.layout_name_row.set_text(&name);
        self.layout_desc_row
            .set_text(description.as_deref().unwrap_or(""));
        self.layout_form_hint
            .set_text(if mode == LayoutFormMode::Save {
                tr("Default is a template. These edits become a new layout.")
            } else {
                ""
            });
        self.refresh_layout_footer(model);
    }

    fn submit_layout_form(&self) {
        let name = self.layout_name_row.text().to_string();
        let description = self.layout_desc_row.text().to_string();
        let description = (!description.trim().is_empty()).then_some(description.as_str());
        let mode = self.layout_form_mode.get();
        let target = self.layout_form_target.borrow().clone();
        let result = {
            let mut model = self.model.borrow_mut();
            let Some(model) = model.as_mut() else {
                return;
            };
            match mode {
                LayoutFormMode::New => model.create_layout(&name, description, None).map(|_| false),
                LayoutFormMode::Rename => target
                    .as_deref()
                    .ok_or_else(|| tr("no layout selected").to_string())
                    .and_then(|id| model.rename_layout(id, &name, description).map(|()| false)),
                LayoutFormMode::Save => model.save_as_layout(&name, description).map(|_| true),
            }
        };
        match result {
            Err(error) => {
                self.layout_form_error.set_text(&error);
                self.layout_form_error.set_visible(true);
            }
            Ok(saved) => {
                self.hide_layout_form();
                self.refresh_all();
                if saved {
                    let document = self
                        .model
                        .borrow()
                        .as_ref()
                        .map(|model| model.document.clone());
                    if let Some(document) = document {
                        self.toast(tr("Saved as a new layout"));
                        self.backend.save(document);
                    }
                } else {
                    self.toast(match mode {
                        LayoutFormMode::New => tr("Layout created"),
                        _ => tr("Layout renamed"),
                    });
                }
            }
        }
    }

    fn refresh_layout_ui(&self, model: &GuiModel) {
        let active = model.active_layout().to_string();
        let active_name = model.layout_name(&active);
        self.layout_chip_name.set_text(&active_name);
        self.layout_chip
            .update_property(&[gtk::accessible::Property::Label(&tr_args(
                "Layout: {name}",
                &[("name", &active_name)],
            ))]);
        self.layout_chip.set_tooltip_text(Some(&tr_args(
            "Switch layout · {name}",
            &[("name", &active_name)],
        )));

        while let Some(child) = self.layout_list.first_child() {
            self.layout_list.remove(&child);
        }
        self.layout_rows.borrow_mut().clear();
        for summary in model.summaries() {
            let row = layout_popover_row(&summary, &active);
            let weak = self.this.borrow().clone();
            let id = summary.id.clone();
            row.connect_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    this.switch_layout(&id);
                }
            });
            self.layout_rows
                .borrow_mut()
                .insert(summary.id.clone(), row.clone());
            self.layout_list.append(&row);
        }

        while let Some(child) = self.layout_manager_list.first_child() {
            self.layout_manager_list.remove(&child);
        }
        self.layout_manager_rows.borrow_mut().clear();
        self.layout_actions.borrow_mut().clear();
        for summary in model.summaries() {
            let (row, actions) = layout_manager_row(&summary, &active, self.this.borrow().clone());
            for (key, button) in actions {
                self.layout_actions.borrow_mut().insert(key, button);
            }
            self.layout_manager_rows
                .borrow_mut()
                .insert(summary.id.clone(), row.clone());
            self.layout_manager_list.append(&row);
        }
        self.layout_dialog_title
            .set_subtitle(&format!("{} · {}", active_name, model.profile.id));
        self.refresh_layout_footer(model);
    }

    fn refresh_layout_footer(&self, model: &GuiModel) {
        let visible = model.dirty && !self.layout_form.is_visible();
        self.layout_manager_apply.set_visible(visible);
        self.layout_manager_revert.set_visible(visible);
    }

    /// Test hook: open the manager without a pointer press.
    #[doc(hidden)]
    pub fn present_layout_manager(&self) {
        self.open_layout_manager();
    }

    /// Test hook: scroll a board slot into the visible viewport.
    #[doc(hidden)]
    pub fn scroll_slot_into_view(&self, button: RemoteButton, trigger: Trigger) {
        if let Some(slot) = self.board.slot(button, trigger) {
            scroll_into_view(&self.board_scroll, slot);
        }
    }

    /// Test hook: scroll any board child into the visible viewport.
    #[doc(hidden)]
    pub fn scroll_board_to(&self, widget: &impl IsA<gtk::Widget>) {
        scroll_into_view(&self.board_scroll, widget);
    }

    /// Test hook: treat the model as persisted without a daemon.
    #[doc(hidden)]
    pub fn mark_saved_for_test(&self) {
        if let Some(model) = self.model.borrow_mut().as_mut() {
            model.mark_saved();
        }
        self.refresh_all();
    }

    /// Test hook: close the layout manager dialog.
    #[doc(hidden)]
    pub fn close_layout_manager(&self) {
        self.layout_dialog.close();
    }

    /// Test hook: open one inline form.
    #[doc(hidden)]
    pub fn present_layout_form(&self, mode: &str, target: Option<&str>) {
        let mode = match mode {
            "new" => LayoutFormMode::New,
            "rename" => LayoutFormMode::Rename,
            "save" => LayoutFormMode::Save,
            _ => return,
        };
        self.show_layout_form(mode, target.map(str::to_string));
    }

    fn revert(&self) {
        if self.model.borrow().is_some() {
            self.backend.load();
        }
    }

    fn editor_update(&self) {
        let draft = self.editor.draft();
        let allow_exec = self
            .model
            .borrow()
            .as_ref()
            .is_some_and(|model| model.document.allow_exec);
        if let Err(error) = draft.validate(allow_exec) {
            self.editor.show_error(&error);
            return;
        }
        let raw = draft.to_raw();
        let result = self
            .model
            .borrow_mut()
            .as_mut()
            .ok_or_else(|| tr("no configuration loaded").to_string())
            .and_then(|model| model.set_slot_raw(Some(raw)));
        match result {
            Ok(_) => {
                self.editor.close();
                self.toast(tr("Mapping updated"));
                self.refresh_all();
            }
            Err(error) => self.editor.show_error(&error),
        }
    }

    fn editor_reset(&self) {
        let result = self
            .model
            .borrow_mut()
            .as_mut()
            .ok_or_else(|| tr("no configuration loaded").to_string())
            .and_then(|model| model.set_slot_raw(None));
        match result {
            Ok(_) => {
                self.editor.close();
                self.toast(tr("Profile default restored"));
                self.refresh_all();
            }
            Err(error) => self.editor.show_error(&error),
        }
    }

    fn editor_allow_exec(self: &Rc<Self>, allow: bool) {
        let already = self
            .model
            .borrow()
            .as_ref()
            .is_some_and(|model| model.document.allow_exec);
        if allow == already {
            self.editor.set_allow_exec(allow);
            return;
        }
        if !allow {
            if let Some(model) = self.model.borrow_mut().as_mut() {
                model.set_allow_exec(false);
            }
            self.editor.set_allow_exec(false);
            self.refresh_all();
            return;
        }
        let dialog = adw::AlertDialog::builder()
            .heading(tr("Allow shell commands?"))
            .body(tr(
                "Exec mappings run shell commands as your user with a 5 s timeout. \
                 Only enable this for commands you trust.",
            ))
            .build();
        dialog.add_response("cancel", tr("Cancel"));
        dialog.add_response("enable", tr("Enable"));
        dialog.set_response_appearance("enable", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let this = Rc::clone(self);
        dialog.connect_response(None, move |_, response| {
            let enabled = response == "enable";
            this.editor.set_allow_exec(enabled);
            if enabled && let Some(model) = this.model.borrow_mut().as_mut() {
                model.set_allow_exec(true);
            }
            this.refresh_all();
        });
        dialog.present(Some(&self.window));
    }

    fn voice_update(&self) {
        let raw = self.voice_editor.value();
        let result = self
            .model
            .borrow_mut()
            .as_mut()
            .ok_or_else(|| tr("no configuration loaded").to_string())
            .and_then(|model| model.set_trigger_key(Some(raw)));
        match result {
            Ok(_) => {
                self.voice_editor.close();
                self.toast(tr("Recording output key updated"));
                self.refresh_all();
            }
            Err(error) => self.voice_editor.show_error(&error),
        }
    }

    fn voice_reset(&self) {
        let result = self
            .model
            .borrow_mut()
            .as_mut()
            .ok_or_else(|| tr("no configuration loaded").to_string())
            .and_then(|model| {
                model.set_trigger_key(Some(atvv_config::DEFAULT_TRIGGER_KEY.to_string()))
            });
        match result {
            Ok(_) => {
                self.voice_editor.close();
                self.toast(tr("Recording output key reset"));
                self.refresh_all();
            }
            Err(error) => self.voice_editor.show_error(&error),
        }
    }

    fn learn_key_for(self: &Rc<Self>, voice: bool) {
        let dialog = gtk::Window::builder()
            .title(tr("Learn key"))
            .transient_for(&self.window)
            .modal(true)
            .resizable(false)
            .default_width(380)
            .build();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        content.set_margin_top(24);
        content.set_margin_bottom(24);
        content.set_margin_start(24);
        content.set_margin_end(24);
        let heading = gtk::Label::new(Some(tr("Press a key or chord")));
        heading.add_css_class("title-2");
        let hint = gtk::Label::new(Some(tr(
            "Escape cancels. Held modifiers become part of the chord.",
        )));
        hint.add_css_class("dim-label");
        hint.set_wrap(true);
        content.append(&heading);
        content.append(&hint);
        dialog.set_child(Some(&content));

        let controller = gtk::EventControllerKey::new();
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        let this = Rc::clone(self);
        let dialog_weak = dialog.downgrade();
        let hint_weak = hint.downgrade();
        controller.connect_key_pressed(move |_, keyval, _, state| {
            if keyval == gtk::gdk::Key::Escape {
                if let Some(dialog) = dialog_weak.upgrade() {
                    dialog.close();
                }
                return glib::Propagation::Stop;
            }
            let name = keyval
                .name()
                .map(|name| name.to_string())
                .unwrap_or_default();
            if keylearn::is_modifier_name(&name) {
                return glib::Propagation::Stop;
            }
            let learned = keylearn::learn_action(
                &name,
                state.contains(gtk::gdk::ModifierType::CONTROL_MASK),
                state.contains(gtk::gdk::ModifierType::ALT_MASK),
                state.contains(gtk::gdk::ModifierType::SHIFT_MASK),
                state.contains(gtk::gdk::ModifierType::SUPER_MASK)
                    | state.contains(gtk::gdk::ModifierType::META_MASK),
            );
            match learned {
                Some(learned) => {
                    if voice {
                        this.voice_editor.set_value(&learned);
                    } else {
                        this.editor.set_learned(&learned);
                    }
                    if let Some(dialog) = dialog_weak.upgrade() {
                        dialog.close();
                    }
                }
                None => {
                    if let Some(hint) = hint_weak.upgrade() {
                        hint.set_text(tr("That key is not supported; try another."));
                    }
                }
            }
            glib::Propagation::Stop
        });
        dialog.add_controller(controller);
        dialog.present();
    }

    fn learn_remote(&self) {
        if self.test_button.is_active() {
            self.suppress_test_toggle.set(true);
            self.test_button.set_active(false);
            self.suppress_test_toggle.set(false);
        }
        self.start_capture(CaptureMode::Learn);
    }

    fn run_once(self: &Rc<Self>) {
        let action = self
            .last_test
            .borrow()
            .as_ref()
            .and_then(|result| result.action.clone());
        let Some(action) = action else {
            return;
        };
        if action == "none" {
            return;
        }
        if let Some(command) = action.strip_prefix("exec:") {
            let dialog = adw::AlertDialog::builder()
                .heading(tr("Run shell command?"))
                .body(tr_args(
                    "This runs “{command}” as your user with a 5 s timeout.",
                    &[("command", command)],
                ))
                .build();
            dialog.add_response("cancel", tr("Cancel"));
            dialog.add_response("run", tr("Run command"));
            dialog.set_response_appearance("run", adw::ResponseAppearance::Destructive);
            dialog.set_default_response(Some("cancel"));
            dialog.set_close_response("cancel");
            let this = Rc::clone(self);
            dialog.connect_response(None, move |_, response| {
                if response == "run" {
                    this.backend.run_action(action.clone());
                }
            });
            dialog.present(Some(&self.window));
        } else {
            self.backend.run_action(action);
        }
    }

    fn refresh_all(&self) {
        self.refresh_title();
        self.refresh_device_chip();
        self.refresh_device_dialog();
        self.refresh_voice_controls();
        self.refresh_microphone();
        let model = self.model.borrow();
        let Some(model) = model.as_ref() else {
            self.apply_button.set_sensitive(false);
            self.revert_button.set_sensitive(false);
            self.dirty_banner.set_revealed(false);
            self.test_button.set_sensitive(false);
            self.test_bar.set_visible(false);
            self.stack.set_visible_child_name("unavailable");
            return;
        };
        self.test_bar.set_visible(true);
        self.stack.set_visible_child_name("config");
        self.board
            .refresh(model, self.pressed.get(), self.voice_selected.get());
        self.refresh_layout_ui(model);
        self.apply_button.set_sensitive(model.dirty);
        self.revert_button.set_sensitive(model.dirty);
        self.dirty_banner.set_title(if model.template_changed() {
            tr("Unsaved changes on Default — Apply saves them as a new layout.")
        } else {
            tr("Unsaved changes")
        });
        self.dirty_banner.set_revealed(model.dirty);
        self.test_button.set_sensitive(true);
        self.refresh_test_bar();
    }

    fn refresh_board(&self) {
        let model = self.model.borrow();
        if let Some(model) = model.as_ref() {
            self.board
                .refresh(model, self.pressed.get(), self.voice_selected.get());
        }
    }

    fn refresh_test_bar(&self) {
        let testing = self.capture_mode.get() == CaptureMode::Test;
        if testing {
            self.test_bar.add_css_class("on");
        } else {
            self.test_bar.remove_css_class("on");
        }
        if !testing {
            self.test_title.set_text(tr("Test mode off"));
            self.test_sub
                .set_text(tr("Turn on Test to see what each key sends."));
            self.last_action_label.set_visible(false);
            self.run_once_button.set_sensitive(false);
            return;
        }
        self.test_title.set_text(tr("Test mode on"));
        let last_test = self.last_test.borrow();
        let Some(result) = last_test.as_ref() else {
            self.test_sub.set_text(tr(
                "Press a remote key — shortcuts, volume, and dictation are suppressed.",
            ));
            self.last_action_label.set_visible(false);
            self.run_once_button.set_sensitive(false);
            return;
        };
        self.test_sub.set_text(&tr_args(
            "{button} · {trigger}",
            &[("button", &result.button), ("trigger", &result.trigger)],
        ));
        self.last_action_label
            .set_text(&format!("→ {}", result.shown));
        self.last_action_label.set_visible(true);
        let runnable = result
            .action
            .as_deref()
            .is_some_and(|action| action != "none");
        self.run_once_button.set_sensitive(runnable);
    }

    fn refresh_title(&self) {
        let status = self.status.borrow();
        let model = self
            .model
            .borrow()
            .as_ref()
            .map(|model| model.profile.model.to_string())
            .filter(|model| !model.is_empty());
        let subtitle = match status.as_ref() {
            None => tr("Connecting…").to_string(),
            Some(status) => {
                let identity = model.clone().or_else(|| {
                    if !status.device.name.is_empty() {
                        Some(status.device.name.clone())
                    } else if !status.device.address.is_empty() {
                        Some(status.device.address.clone())
                    } else {
                        None
                    }
                });
                match identity {
                    Some(identity) if status.streaming => {
                        tr_args("{device} · recording", &[("device", &identity)])
                    }
                    Some(identity) => tr_args(
                        "{device} · {state}",
                        &[("device", &identity), ("state", &status.state)],
                    ),
                    None if status.connected => status.state.clone(),
                    None => tr("No remote connected").to_string(),
                }
            }
        };
        self.title.set_subtitle(&subtitle);
    }

    fn refresh_device_chip(&self) {
        let status = self.status.borrow();
        let status = status.as_ref();
        let connected = status.is_some_and(|status| status.connected);
        let device_name = status
            .map(|status| status.device.name.trim())
            .filter(|name| !name.is_empty());
        let address = status
            .map(|status| status.device.address.as_str())
            .unwrap_or_default();
        // BlueZ's alias is the name the user recognizes and can edit; the
        // profile's display name and model are the static fallbacks.
        let profile = self
            .model
            .borrow()
            .as_ref()
            .map(|model| (model.profile.display_name, model.profile.model));
        let label = device_name
            .map(str::to_string)
            .or_else(|| {
                profile
                    .map(|(display, _)| display.to_string())
                    .filter(|display| !display.is_empty())
            })
            .or_else(|| {
                profile
                    .map(|(_, model)| model.to_string())
                    .filter(|model| !model.is_empty())
            })
            .unwrap_or_else(|| {
                if address.is_empty() {
                    tr("No remote").to_string()
                } else {
                    address.to_string()
                }
            });
        self.device_chip_name.set_text(&label);
        let tooltip = if address.is_empty() {
            label.clone()
        } else {
            address.to_string()
        };
        self.device_chip.set_tooltip_text(Some(&tooltip));
        if connected {
            self.device_chip_icon.remove_css_class("dim-label");
        } else {
            self.device_chip_icon.add_css_class("dim-label");
        }
        let percent = status.and_then(|status| status.hardware.battery_percent);
        self.device_chip_battery.set_text(&if percent.is_some() {
            battery_text(status)
        } else {
            String::new()
        });
        self.device_chip_battery_icon.set_visible(percent.is_some());
        if let Some(percent) = percent {
            self.device_chip_battery_icon
                .set_icon_name(Some(battery_icon(percent)));
        }
        let accessible = tr_args("Device: {name}", &[("name", &label)]);
        self.device_chip
            .update_property(&[gtk::accessible::Property::Label(&accessible)]);
        self.refresh_device_list();
    }

    fn refresh_device_dialog(&self) {
        let status = self.status.borrow();
        self.device_dialog.refresh(status.as_ref());
    }

    fn refresh_device_list(&self) {
        while let Some(row) = self.device_list.first_child() {
            self.device_list.remove(&row);
        }
        let model = self.model.borrow();
        let Some(model) = model.as_ref() else {
            return;
        };
        let status = self.status.borrow();
        for (address, device) in &model.document.devices {
            let active = model.document.active_device.as_deref() == Some(address.as_str());
            let connected = active && status.as_ref().is_some_and(|status| status.connected);
            let name = device
                .label
                .clone()
                .or_else(|| {
                    active
                        .then(|| {
                            status
                                .as_ref()
                                .map(|status| status.device.name.trim().to_string())
                        })
                        .flatten()
                })
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| address.clone());
            let subtitle = format!(
                "{address} · {} · {}{}",
                device.profile,
                model.layout_name(&device.layout),
                if connected {
                    format!(" · {}", tr("Connected"))
                } else {
                    String::new()
                },
            );
            let row = adw::ActionRow::builder()
                .title(name)
                .subtitle(subtitle)
                .activatable(true)
                .build();
            row.set_widget_name(&format!("device-row-{address}"));
            if active {
                let check = gtk::Image::from_icon_name("object-select-symbolic");
                check.add_css_class("accent");
                row.add_suffix(&check);
            }
            self.device_list.append(&row);
        }
        if model.document.devices.is_empty() {
            let row = adw::ActionRow::builder()
                .title(tr("No known devices"))
                .subtitle(tr("Run atvvctl pair to add one."))
                .build();
            self.device_list.append(&row);
        }
    }

    fn refresh_voice_controls(&self) {
        let status = self.status.borrow();
        let keys = status
            .as_ref()
            .map(|status| status.contract.recording_keys.as_slice())
            .unwrap_or_default();
        let mode = status
            .as_ref()
            .map(|status| status.vinput.mode.as_str())
            .unwrap_or_default();
        self.voice_editor.refresh_declared_keys(keys);
        self.voice_editor.set_mode(mode);
    }

    fn refresh_microphone(&self) {
        {
            let status = self.status.borrow();
            if let Some(status) = status.as_ref() {
                self.microphone.set_level(status.audio.level);
                self.microphone.set_source(
                    &status.contract.source,
                    &status.contract.sample_format,
                    status.contract.sample_rate,
                    status.contract.channels,
                );
            }
        }
        if let Some(model) = self.model.borrow().as_ref() {
            self.microphone.load(model);
        }
    }

    fn show_error(&self, message: &str) {
        self.error_banner.set_title(message);
        self.error_banner.set_revealed(true);
    }

    fn toast(&self, message: &str) {
        let toast = adw::Toast::new(message);
        toast.set_timeout(2);
        self.toast_overlay.add_toast(toast);
    }

    fn about_dialog(&self) -> adw::AboutDialog {
        adw::AboutDialog::builder()
            .application_name(tr("ATVV Remote"))
            .application_icon(APP_ID)
            .developer_name("Yangtse Su")
            .version(env!("CARGO_PKG_VERSION"))
            .comments(tr(
                "Configure ATVV remote button mappings and test them from the desktop.",
            ))
            .website("https://github.com/YangtseSu/atvv-linux")
            .license_type(gtk::License::Gpl30)
            .build()
    }

    // Test-only accessors: integration tests drive the real widgets.

    #[doc(hidden)]
    #[must_use]
    pub fn window(&self) -> &adw::ApplicationWindow {
        &self.window
    }

    #[doc(hidden)]
    #[must_use]
    pub fn layout_chip_text(&self) -> String {
        self.layout_chip_name.text().to_string()
    }

    #[doc(hidden)]
    #[must_use]
    pub fn layout_row(&self, id: &str) -> Option<gtk::Button> {
        self.layout_rows.borrow().get(id).cloned()
    }

    #[doc(hidden)]
    #[must_use]
    pub fn layout_manager_row(&self, id: &str) -> Option<adw::ActionRow> {
        self.layout_manager_rows.borrow().get(id).cloned()
    }

    #[doc(hidden)]
    #[must_use]
    pub fn layout_manager_action(&self, id: &str, action: &str) -> Option<gtk::Button> {
        self.layout_actions
            .borrow()
            .get(&format!("{id}:{action}"))
            .cloned()
    }

    #[doc(hidden)]
    #[must_use]
    pub fn layout_manager_is_open(&self) -> bool {
        self.layout_dialog.is_visible()
    }

    #[doc(hidden)]
    #[must_use]
    pub fn layout_name_row(&self) -> &adw::EntryRow {
        &self.layout_name_row
    }

    #[doc(hidden)]
    #[must_use]
    pub fn layout_desc_row(&self) -> &adw::EntryRow {
        &self.layout_desc_row
    }

    #[doc(hidden)]
    #[must_use]
    pub fn layout_form_submit(&self) -> &gtk::Button {
        &self.layout_form_submit
    }

    #[doc(hidden)]
    #[must_use]
    pub fn layout_form_is_visible(&self) -> bool {
        self.layout_form.is_visible()
    }

    #[doc(hidden)]
    #[must_use]
    pub fn layout_form_error_text(&self) -> String {
        self.layout_form_error.text().to_string()
    }

    #[doc(hidden)]
    #[must_use]
    pub fn layout_manager_new_button(&self) -> &gtk::Button {
        &self.layout_manager_new
    }

    #[doc(hidden)]
    #[must_use]
    pub fn layout_manager_preset_new_button(&self) -> &gtk::Button {
        &self.layout_manager_preset_new
    }

    #[doc(hidden)]
    pub fn open_preset_dialog_for_test(&self, target: Option<&str>) {
        let mode = if target.is_some() {
            PresetMode::Reset
        } else {
            PresetMode::New
        };
        self.open_preset_dialog(mode, target.map(str::to_string));
    }

    #[doc(hidden)]
    #[must_use]
    pub fn preset_dialog_is_open(&self) -> bool {
        self.preset_dialog.is_visible()
    }

    #[doc(hidden)]
    pub fn close_preset_dialog_for_test(&self) {
        self.preset_dialog.force_close();
    }

    #[doc(hidden)]
    #[must_use]
    pub fn preset_row(&self, id: &str) -> Option<adw::ActionRow> {
        self.preset_rows.borrow().get(id).cloned()
    }

    #[doc(hidden)]
    #[must_use]
    pub fn preset_name_row(&self) -> &adw::EntryRow {
        &self.preset_name_row
    }

    #[doc(hidden)]
    #[must_use]
    pub fn preset_preview_row(&self, button: &str) -> Option<adw::ActionRow> {
        let button = remote_button(button)?;
        self.preset_preview_rows.borrow().get(&button).cloned()
    }

    #[doc(hidden)]
    #[must_use]
    pub fn preset_warning_text(&self) -> String {
        self.preset_warning.text().to_string()
    }

    #[doc(hidden)]
    #[must_use]
    pub fn preset_error_text(&self) -> String {
        self.preset_error.text().to_string()
    }

    #[doc(hidden)]
    #[must_use]
    pub fn preset_selection(&self) -> Option<&'static str> {
        self.preset_selected.get().map(|preset| preset.id)
    }

    #[doc(hidden)]
    pub fn select_preset_for_test(&self, id: &str) {
        if let Some(preset) = atvv_config::layout_preset(id) {
            self.select_preset(preset);
        }
    }

    #[doc(hidden)]
    #[must_use]
    pub fn preset_submit_button(&self) -> &gtk::Button {
        &self.preset_submit
    }

    #[doc(hidden)]
    #[must_use]
    pub fn layout_manager_apply_button(&self) -> &gtk::Button {
        &self.layout_manager_apply
    }

    #[doc(hidden)]
    #[must_use]
    pub fn dirty_banner_text(&self) -> String {
        self.dirty_banner.title().to_string()
    }

    #[doc(hidden)]
    pub fn set_status(&self, status: DaemonStatus) {
        *self.status.borrow_mut() = Some(status);
        self.refresh_title();
        self.refresh_device_chip();
        self.refresh_device_dialog();
        self.refresh_voice_controls();
        self.refresh_microphone();
    }

    #[doc(hidden)]
    #[must_use]
    pub fn battery_text(&self) -> String {
        self.device_dialog.battery_text()
    }

    #[doc(hidden)]
    #[must_use]
    pub fn device_name_text(&self) -> String {
        self.device_dialog.name_text()
    }

    #[doc(hidden)]
    #[must_use]
    pub fn subtitle_text(&self) -> String {
        self.title.subtitle().to_string()
    }

    #[doc(hidden)]
    #[must_use]
    pub fn hardware_text(&self, field: HardwareField) -> Option<String> {
        self.device_dialog.hardware_text(field)
    }

    #[doc(hidden)]
    #[must_use]
    pub fn chip_device_text(&self) -> String {
        self.device_chip_name.text().to_string()
    }

    #[doc(hidden)]
    #[must_use]
    pub fn diagram(&self) -> &crate::diagram::RemoteDiagram {
        self.board.diagram()
    }

    #[doc(hidden)]
    #[must_use]
    pub fn board(&self) -> &MappingBoard {
        &self.board
    }

    #[doc(hidden)]
    #[must_use]
    pub fn board_scroll(&self) -> &gtk::ScrolledWindow {
        &self.board_scroll
    }

    #[doc(hidden)]
    #[must_use]
    pub fn editor(&self) -> &ActionEditor {
        &self.editor
    }

    #[doc(hidden)]
    #[must_use]
    pub fn voice_editor(&self) -> &VoiceEditor {
        &self.voice_editor
    }

    #[doc(hidden)]
    #[must_use]
    pub fn microphone(&self) -> &MicrophoneDialog {
        &self.microphone
    }

    #[doc(hidden)]
    pub fn set_model(&self, model: GuiModel) {
        *self.model.borrow_mut() = Some(model);
        self.stack.set_visible_child_name("config");
        self.refresh_all();
    }

    #[doc(hidden)]
    #[must_use]
    pub fn active_layout_id(&self) -> Option<String> {
        self.model
            .borrow()
            .as_ref()
            .map(|model| model.active_layout().to_string())
    }

    #[doc(hidden)]
    #[must_use]
    pub fn template_active(&self) -> bool {
        self.model.borrow().as_ref().is_some_and(GuiModel::template)
    }

    #[doc(hidden)]
    #[must_use]
    pub fn staged_slot(&self, button: RemoteButton, trigger: Trigger) -> Option<String> {
        self.model
            .borrow()
            .as_ref()
            .and_then(|model| model.slot(button, trigger).map(str::to_string))
    }

    #[doc(hidden)]
    #[must_use]
    pub fn effective_slot(&self, button: RemoteButton, trigger: Trigger) -> Option<String> {
        self.model
            .borrow()
            .as_ref()
            .and_then(|model| model.effective_slot(button, trigger))
    }

    #[doc(hidden)]
    #[must_use]
    pub fn selected(&self) -> Option<RemoteButton> {
        self.model.borrow().as_ref().map(|model| model.selected)
    }

    #[doc(hidden)]
    #[must_use]
    pub fn selected_trigger(&self) -> Option<Trigger> {
        self.model.borrow().as_ref().map(|model| model.trigger)
    }

    #[doc(hidden)]
    #[must_use]
    pub fn apply_button(&self) -> &gtk::Button {
        &self.apply_button
    }

    #[doc(hidden)]
    #[must_use]
    pub fn document(&self) -> Option<ConfigDocument> {
        self.model
            .borrow()
            .as_ref()
            .map(|model| model.document.clone())
    }

    #[doc(hidden)]
    #[must_use]
    pub fn voice_selected(&self) -> bool {
        self.voice_selected.get()
    }

    #[doc(hidden)]
    #[must_use]
    pub fn voice_value_row(&self) -> &adw::EntryRow {
        self.voice_editor.value_row()
    }

    #[doc(hidden)]
    #[must_use]
    pub fn voice_declared_button(&self) -> &gtk::MenuButton {
        self.voice_editor.declared_button()
    }

    #[doc(hidden)]
    #[must_use]
    pub fn voice_hint_text(&self) -> String {
        self.voice_editor.hint_text()
    }

    #[doc(hidden)]
    #[must_use]
    pub fn voice_update_button(&self) -> &gtk::Button {
        self.voice_editor.update_button()
    }

    #[doc(hidden)]
    #[must_use]
    pub fn test_button(&self) -> &gtk::ToggleButton {
        &self.test_button
    }

    #[doc(hidden)]
    #[must_use]
    pub fn test_result_text(&self) -> String {
        self.test_sub.text().to_string()
    }

    #[doc(hidden)]
    #[must_use]
    pub fn last_action_text(&self) -> String {
        self.last_action_label.text().to_string()
    }

    /// Feed a `ButtonEvent` the way the daemon signal listener would.
    #[doc(hidden)]
    pub fn simulate_button_event(&self, event: ButtonSignal) {
        self.on_button_event(event);
    }
}

impl Drop for AppWindow {
    fn drop(&mut self) {
        self.backend.stop();
    }
}

fn connect_timers(ui: &Rc<AppWindow>) {
    let weak = Rc::downgrade(ui);
    glib::timeout_add_local(Duration::from_millis(50), move || {
        let Some(ui) = weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        ui.poll_events();
        glib::ControlFlow::Continue
    });
    let weak = Rc::downgrade(ui);
    glib::timeout_add_local(Duration::from_secs(2), move || {
        let Some(ui) = weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        if ui.model.borrow().is_some() && !ui.status_in_flight.get() {
            ui.status_in_flight.set(true);
            ui.backend.refresh_status();
        }
        glib::ControlFlow::Continue
    });
}

/// Poll the level at 200 ms while the microphone dialog is open.
///
/// The timer stops when the dialog closes; at most one status request is in
/// flight, so the global 2 s poll and this one never queue up.
fn start_microphone_poll(ui: &Rc<AppWindow>) {
    if ui.microphone_polling.get() {
        return;
    }
    ui.microphone_polling.set(true);
    let weak = Rc::downgrade(ui);
    glib::timeout_add_local(Duration::from_millis(200), move || {
        let Some(ui) = weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        if !ui.microphone.is_open() {
            ui.microphone_polling.set(false);
            return glib::ControlFlow::Break;
        }
        if !ui.status_in_flight.get() {
            ui.status_in_flight.set(true);
            ui.backend.refresh_status();
        }
        glib::ControlFlow::Continue
    });
}

/// Recording-key hint for a vinput mode, kept for the window tests.
#[must_use]
pub fn voice_controls_hint(mode: &str) -> String {
    voice_mode_hint(mode)
}

/// Build a window for display tests.
#[must_use]
pub fn build_window(application: Option<&adw::Application>, backend: Backend) -> Rc<AppWindow> {
    AppWindow::build(application, backend)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_hint_warns_when_vinput_cannot_emit() {
        let _guard = crate::i18n::language_guard();
        crate::i18n::Language::set(crate::i18n::Language::English);
        let trigger = voice_controls_hint("trigger");
        assert!(trigger.contains("Consumers must bind"));
        assert!(!trigger.contains("not emitted"));

        let dbus = voice_controls_hint("dbus");
        assert!(dbus.contains("D-Bus"));
        assert!(dbus.contains("not emitted"));

        let off = voice_controls_hint("off");
        assert!(off.contains("integration is off"));
        assert!(off.contains("not emitted"));
    }

    #[test]
    fn battery_text_marks_a_stale_value() {
        let _guard = crate::i18n::language_guard();
        crate::i18n::Language::set(crate::i18n::Language::English);
        let mut status = DaemonStatus {
            connected: true,
            ..DaemonStatus::default()
        };
        status.hardware.battery_percent = Some(62);
        assert_eq!(battery_text(Some(&status)), "62 %");
        status.connected = false;
        assert_eq!(battery_text(Some(&status)), "62 % · last known");
        status.hardware.battery_percent = None;
        assert_eq!(battery_text(Some(&status)), "Unknown");
        assert_eq!(battery_text(None), "Unknown");
    }
}
