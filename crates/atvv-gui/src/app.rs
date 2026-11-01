// SPDX-License-Identifier: GPL-3.0-or-later

//! GTK4/libadwaita window: mapping board, modal editors, capture, test bar.
//!
//! The window owns the [`GuiModel`] and renders it through the board. Every
//! edit goes through the model and the daemon's `SetConfig`; the window never
//! writes the configuration file, grabs input devices, or talks to BlueZ.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use atvv_config::ConfigDocument;
use atvv_input::Trigger;
use atvv_protocol::profile::RemoteButton;
use gtk::{gio, glib};
use libadwaita as adw;

use adw::prelude::*;

use crate::board::{MappingBoard, scroll_into_view};
use crate::client::{Backend, ButtonSignal, DaemonStatus, Event, HardwareInfo};
use crate::editor::{ActionEditor, VoiceEditor, voice_mode_hint};
use crate::i18n::{self, tr, tr_args};
use crate::keylearn;
use crate::model::{GuiModel, action_label, button_label, trigger_from_name, trigger_label};

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
.remote-hotspot.bound {
  border-color: alpha(@accent_color, 0.30);
}
.remote-hotspot.unbound {
  border-style: dashed;
  border-color: alpha(currentColor, 0.32);
}
.remote-hotspot.voice {
  border-style: dotted;
  border-color: alpha(currentColor, 0.30);
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

fn spacer() -> gtk::Box {
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    spacer
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
    device_chip: gtk::Button,
    device_chip_name: gtk::Label,
    device_chip_icon: gtk::Image,
    device_chip_battery: gtk::Label,
    device_chip_battery_icon: gtk::Image,
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
    device_dialog: DeviceDialog,
    stack: gtk::Stack,
    toast_overlay: adw::ToastOverlay,
    capture_mode: Cell<CaptureMode>,
    suppress_test_toggle: Cell<bool>,
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

        let device_chip = gtk::Button::builder()
            .tooltip_text(tr("Device information"))
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
            device_dialog: DeviceDialog::new(),
            stack,
            toast_overlay,
            capture_mode: Cell::new(CaptureMode::Off),
            suppress_test_toggle: Cell::new(false),
            voice_selected: Cell::new(false),
            pressed: Cell::new(None),
            last_test: RefCell::new(None),
        });
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
        ui.device_chip
            .connect_clicked(move |_| this.device_dialog.present(&this.window));

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
                *self.status.borrow_mut() = Some(status);
                self.refresh_title();
                self.refresh_device_chip();
                self.refresh_device_dialog();
                self.refresh_voice_controls();
            }
            Event::Status(Err(error)) => {
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
                    self.toast(&tr_args(
                        "Selected {button} · {trigger}",
                        &[
                            ("button", button_label(button)),
                            ("trigger", trigger_label(trigger)),
                        ],
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
                    *self.last_test.borrow_mut() = Some(TestResult {
                        button: button_label(button).to_string(),
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
        let document = {
            let model = self.model.borrow();
            let Some(model) = model.as_ref() else {
                return;
            };
            if let Err(error) = model.validate() {
                self.show_error(&error);
                return;
            }
            model.document.clone()
        };
        self.apply_button.set_sensitive(false);
        self.error_banner.set_revealed(false);
        self.backend.save(document);
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
        self.apply_button.set_sensitive(model.dirty);
        self.revert_button.set_sensitive(model.dirty);
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
    }

    fn refresh_device_dialog(&self) {
        let status = self.status.borrow();
        self.device_dialog.refresh(status.as_ref());
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
    pub fn set_status(&self, status: DaemonStatus) {
        *self.status.borrow_mut() = Some(status);
        self.refresh_title();
        self.refresh_device_chip();
        self.refresh_device_dialog();
        self.refresh_voice_controls();
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
    pub fn set_model(&self, model: GuiModel) {
        *self.model.borrow_mut() = Some(model);
        self.stack.set_visible_child_name("config");
        self.refresh_all();
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
        if ui.model.borrow().is_some() {
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
