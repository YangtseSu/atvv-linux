// SPDX-License-Identifier: GPL-3.0-or-later

//! GTK4/libadwaita window, editor, capture-mode, and test-mode wiring.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use atvv_config::ConfigDocument;
use atvv_input::Trigger;
use atvv_protocol::profile::RemoteButton;
use gtk::{gio, glib};
use libadwaita as adw;

use adw::prelude::*;

use crate::actions::{ActionDraft, ActionKind, DictationChoice};
use crate::client::{Backend, ButtonSignal, DaemonStatus, Event, HardwareInfo};
use crate::diagram::RemoteDiagram;
use crate::i18n::{self, tr, tr_args};
use crate::keylearn;
use crate::model::{GuiModel, button_label};

pub const APP_ID: &str = "org.atvv.Config";

const CSS: &str = r#"
.remote-canvas {
  background: alpha(@card_bg_color, 0.55);
  border-radius: 24px;
}
.remote-hotspot {
  background: transparent;
  border: 1.5px solid transparent;
  border-radius: 999px;
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
        // stylesheet here is what makes the hotspot states render.
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

/// One Device Information Service fact shown in the Hardware group.
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

pub struct AppWindow {
    window: adw::ApplicationWindow,
    backend: Backend,
    model: RefCell<Option<GuiModel>>,
    status: RefCell<Option<DaemonStatus>>,
    title: adw::WindowTitle,
    battery_row: adw::ActionRow,
    battery_icon: gtk::Image,
    hardware_group: adw::PreferencesGroup,
    hardware_rows: Vec<(HardwareField, adw::ActionRow)>,
    stack: gtk::Stack,
    split: adw::NavigationSplitView,
    diagram: RemoteDiagram,
    apply_button: gtk::Button,
    revert_button: gtk::Button,
    test_button: gtk::ToggleButton,
    error_banner: adw::Banner,
    dirty_banner: adw::Banner,
    capture_banner: adw::Banner,
    key_group: adw::PreferencesGroup,
    action_group: adw::PreferencesGroup,
    voice_group: adw::PreferencesGroup,
    selected_row: adw::ActionRow,
    trigger_row: adw::ComboRow,
    type_row: adw::ComboRow,
    value_row: adw::EntryRow,
    learn_key_button: gtk::Button,
    learn_remote_button: gtk::Button,
    retry_button: gtk::Button,
    exec_row: adw::EntryRow,
    exec_warning: gtk::Label,
    dictation_row: adw::ComboRow,
    action_error: gtk::Label,
    apply_action: gtk::Button,
    reset_action: gtk::Button,
    voice_value_row: adw::EntryRow,
    voice_learn_button: gtk::Button,
    voice_apply_button: gtk::Button,
    voice_reset_button: gtk::Button,
    voice_error: gtk::Label,
    allow_exec_row: adw::SwitchRow,
    test_result_row: adw::ActionRow,
    run_once_button: gtk::Button,
    toast_overlay: adw::ToastOverlay,
    loading: Cell<bool>,
    capture_mode: Cell<CaptureMode>,
    suppress_test_toggle: Cell<bool>,
    voice_selected: Cell<bool>,
    pressed: Cell<Option<RemoteButton>>,
    last_action: RefCell<Option<String>>,
}

impl AppWindow {
    /// Build the window. `application` is `None` in display-only tests.
    #[must_use]
    pub fn build(application: Option<&adw::Application>, backend: Backend) -> Rc<Self> {
        let diagram = RemoteDiagram::new();
        let title = adw::WindowTitle::new(tr("ATVV Remote"), tr("Connecting…"));

        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&title));

        let menu = gio::Menu::new();
        menu.append(Some(tr("About ATVV")), Some("win.about"));
        menu.append(Some(tr("Revert changes")), Some("win.revert"));
        menu.append(Some(tr("Quit")), Some("app.quit"));
        let menu_button = gtk::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .menu_model(&menu)
            .tooltip_text(tr("Main menu"))
            .build();
        header.pack_start(&menu_button);

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

        let sidebar_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .build();
        let sidebar_content = gtk::Box::new(gtk::Orientation::Vertical, 10);
        sidebar_content.set_margin_top(18);
        sidebar_content.set_margin_bottom(18);
        sidebar_content.set_margin_start(18);
        sidebar_content.set_margin_end(18);

        let battery_row = adw::ActionRow::builder()
            .title(tr("Battery"))
            .subtitle(tr("Unknown"))
            .build();
        let battery_icon = gtk::Image::from_icon_name("battery-empty-symbolic");
        battery_icon.set_pixel_size(18);
        battery_icon.set_visible(false);
        battery_row.add_suffix(&battery_icon);
        let device_group = adw::PreferencesGroup::builder().title(tr("Remote")).build();
        device_group.add(&battery_row);
        sidebar_content.append(&device_group);

        sidebar_content.append(diagram.root());
        let legend = gtk::Label::new(Some(tr(
            "Click a key to edit it. Dashed: no action. Amber: conflicting action. \
             The top-right voice key is fixed; its output key is configurable.",
        )));
        legend.add_css_class("caption");
        legend.add_css_class("dim-label");
        legend.set_wrap(true);
        legend.set_justify(gtk::Justification::Center);
        sidebar_content.append(&legend);

        let hardware_group = adw::PreferencesGroup::builder()
            .title(tr("Hardware"))
            .build();
        hardware_group.set_visible(false);
        let mut hardware_rows = Vec::new();
        for field in HardwareField::ALL {
            let row = adw::ActionRow::builder().title(field.label()).build();
            row.set_visible(false);
            hardware_group.add(&row);
            hardware_rows.push((field, row));
        }
        sidebar_content.append(&hardware_group);
        sidebar_scroll.set_child(Some(&sidebar_content));
        let sidebar_page = adw::NavigationPage::new(&sidebar_scroll, tr("Remote"));

        let trigger_list = gtk::StringList::new(&[tr("Single"), tr("Double"), tr("Long")]);
        let dictation_list =
            gtk::StringList::new(&DictationChoice::ALL.map(DictationChoice::label));
        let type_list = gtk::StringList::new(&ActionKind::ALL.map(ActionKind::label));

        let key_group = adw::PreferencesGroup::builder()
            .title(tr("Selected key"))
            .build();
        let selected_row = adw::ActionRow::builder().title(tr("Key")).build();
        let learn_remote = gtk::Button::with_label(tr("Learn…"));
        learn_remote.add_css_class("flat");
        learn_remote.set_tooltip_text(Some(tr("Press a physical remote key to select it")));
        selected_row.add_suffix(&learn_remote);
        let trigger_row = adw::ComboRow::builder()
            .title(tr("Gesture"))
            .model(&trigger_list)
            .selected(0)
            .build();
        key_group.add(&selected_row);
        key_group.add(&trigger_row);

        let action_group = adw::PreferencesGroup::builder().title(tr("Action")).build();
        let type_row = adw::ComboRow::builder()
            .title(tr("Type"))
            .model(&type_list)
            .selected(0)
            .build();
        let value_row = adw::EntryRow::builder().title(tr("Key")).build();
        let learn_key_button = gtk::Button::with_label(tr("Learn Key"));
        learn_key_button.add_css_class("flat");
        learn_key_button.set_tooltip_text(Some(tr("Capture a keyboard key or chord")));
        value_row.add_suffix(&learn_key_button);
        let exec_row = adw::EntryRow::builder().title(tr("Command")).build();
        let exec_warning = gtk::Label::new(None);
        exec_warning.add_css_class("caption");
        exec_warning.add_css_class("dim-label");
        exec_warning.set_wrap(true);
        exec_warning.set_xalign(0.0);
        exec_warning.set_margin_start(12);
        exec_warning.set_margin_end(12);
        exec_warning.set_margin_bottom(6);
        let dictation_row = adw::ComboRow::builder()
            .title(tr("Dictation"))
            .model(&dictation_list)
            .selected(0)
            .build();
        let apply_row = adw::ActionRow::builder()
            .title(tr("Mapping"))
            .subtitle(tr(
                "Update this slot, then use Apply in the header to save.",
            ))
            .build();
        let reset_action = gtk::Button::with_label(tr("Reset"));
        reset_action.add_css_class("flat");
        reset_action.set_tooltip_text(Some(tr("Restore the profile default")));
        let apply_action = gtk::Button::with_label(tr("Update"));
        apply_action.add_css_class("suggested-action");
        apply_row.add_suffix(&reset_action);
        apply_row.add_suffix(&apply_action);
        let action_error = gtk::Label::new(None);
        action_error.add_css_class("error");
        action_error.add_css_class("caption");
        action_error.set_wrap(true);
        action_error.set_xalign(0.0);
        action_error.set_margin_start(12);
        action_error.set_margin_end(12);
        action_error.set_margin_bottom(6);
        action_error.set_visible(false);
        action_group.add(&type_row);
        action_group.add(&value_row);
        action_group.add(&exec_row);
        action_group.add(&exec_warning);
        action_group.add(&dictation_row);
        action_group.add(&apply_row);
        action_group.add(&action_error);

        let voice_group = adw::PreferencesGroup::builder()
            .title(tr("Recording key"))
            .build();
        let voice_info = adw::ActionRow::builder()
            .title(tr("Physical key"))
            .subtitle(tr(
                "KEY_F5 is grabbed and fixed; it controls the ATVV microphone.",
            ))
            .build();
        let voice_value_row = adw::EntryRow::builder().title(tr("Output key")).build();
        let voice_learn_button = gtk::Button::with_label(tr("Learn Key"));
        voice_learn_button.add_css_class("flat");
        voice_learn_button.set_tooltip_text(Some(tr("Capture a key or chord for the output")));
        voice_value_row.add_suffix(&voice_learn_button);
        let voice_apply_row = adw::ActionRow::builder()
            .title(tr("Output key"))
            .subtitle(tr("Emitted while the recording key is held; Apply saves."))
            .build();
        let voice_reset_button = gtk::Button::with_label(tr("Reset"));
        voice_reset_button.add_css_class("flat");
        voice_reset_button.set_tooltip_text(Some(tr("Restore DICTATE")));
        let voice_apply_button = gtk::Button::with_label(tr("Update"));
        voice_apply_button.add_css_class("suggested-action");
        voice_apply_row.add_suffix(&voice_reset_button);
        voice_apply_row.add_suffix(&voice_apply_button);
        let voice_error = gtk::Label::new(None);
        voice_error.add_css_class("error");
        voice_error.add_css_class("caption");
        voice_error.set_wrap(true);
        voice_error.set_xalign(0.0);
        voice_error.set_margin_start(12);
        voice_error.set_margin_end(12);
        voice_error.set_margin_bottom(6);
        voice_error.set_visible(false);
        voice_group.add(&voice_info);
        voice_group.add(&voice_value_row);
        voice_group.add(&voice_apply_row);
        voice_group.add(&voice_error);
        voice_group.set_visible(false);

        let test_group = adw::PreferencesGroup::builder()
            .title(tr("Test mode"))
            .build();
        let test_result_row = adw::ActionRow::builder()
            .title(tr("Last test"))
            .subtitle(tr("Test mode is off"))
            .build();
        let run_once_button = gtk::Button::with_label(tr("Run once"));
        run_once_button.add_css_class("flat");
        run_once_button.set_sensitive(false);
        run_once_button.set_tooltip_text(Some(tr("Run the displayed action once through atvvd")));
        test_result_row.add_suffix(&run_once_button);
        test_group.add(&test_result_row);

        let advanced_group = adw::PreferencesGroup::builder()
            .title(tr("Advanced"))
            .build();
        let allow_exec_row = adw::SwitchRow::builder()
            .title(tr("Allow shell commands"))
            .subtitle(tr(
                "Exec mappings run as your user with a 5 s timeout. Off by default.",
            ))
            .build();
        advanced_group.add(&allow_exec_row);

        let prefs = adw::PreferencesPage::new();
        prefs.add(&key_group);
        prefs.add(&action_group);
        prefs.add(&voice_group);
        prefs.add(&test_group);
        prefs.add(&advanced_group);
        let content_page = adw::NavigationPage::new(&prefs, tr("Key mapping"));

        let split = adw::NavigationSplitView::new();
        split.set_sidebar(Some(&sidebar_page));
        split.set_content(Some(&content_page));
        split.set_show_content(false);
        split.set_vexpand(true);

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
        stack.add_named(&split, Some("config"));
        stack.set_visible_child_name("unavailable");

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(&banners);
        content.append(&stack);

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&content));
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
            battery_row,
            battery_icon,
            hardware_group,
            hardware_rows,
            stack,
            split,
            diagram,
            apply_button,
            revert_button,
            test_button,
            error_banner,
            dirty_banner,
            capture_banner,
            key_group,
            action_group,
            voice_group,
            selected_row,
            trigger_row,
            type_row,
            value_row,
            learn_key_button,
            learn_remote_button: learn_remote,
            retry_button: retry,
            exec_row,
            exec_warning,
            dictation_row,
            action_error,
            apply_action,
            reset_action,
            voice_value_row,
            voice_learn_button,
            voice_apply_button,
            voice_reset_button,
            voice_error,
            allow_exec_row,
            test_result_row,
            run_once_button,
            toast_overlay,
            loading: Cell::new(false),
            capture_mode: Cell::new(CaptureMode::Off),
            suppress_test_toggle: Cell::new(false),
            voice_selected: Cell::new(false),
            pressed: Cell::new(None),
            last_action: RefCell::new(None),
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
        ui.diagram
            .connect_selected(move |button| this.select_button(button));

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
        ui.trigger_row.connect_selected_notify(move |row| {
            if this.loading.get() {
                return;
            }
            let trigger = match row.selected() {
                1 => Trigger::Double,
                2 => Trigger::Long,
                _ => Trigger::Single,
            };
            if let Some(model) = this.model.borrow_mut().as_mut() {
                model.trigger = trigger;
            }
            this.refresh_all();
        });
        let this = Rc::clone(ui);
        ui.type_row.connect_selected_notify(move |row| {
            if this.loading.get() {
                return;
            }
            let kind = ActionKind::from_index(row.selected());
            let allow_exec = this
                .model
                .borrow()
                .as_ref()
                .is_some_and(|model| model.document.allow_exec);
            this.update_editor_visibility(kind, allow_exec);
        });
        let this = Rc::clone(ui);
        ui.allow_exec_row.connect_active_notify(move |row| {
            if this.loading.get() {
                return;
            }
            let allow = row.is_active();
            let already = this
                .model
                .borrow()
                .as_ref()
                .is_some_and(|model| model.document.allow_exec);
            if allow == already {
                return;
            }
            if !allow {
                if let Some(model) = this.model.borrow_mut().as_mut() {
                    model.set_allow_exec(false);
                }
                this.refresh_all();
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
            let dialog_this = Rc::clone(&this);
            dialog.connect_response(None, move |_, response| {
                if response == "enable" {
                    if let Some(model) = dialog_this.model.borrow_mut().as_mut() {
                        model.set_allow_exec(true);
                    }
                    dialog_this.refresh_all();
                } else {
                    dialog_this.loading.set(true);
                    dialog_this.allow_exec_row.set_active(false);
                    dialog_this.loading.set(false);
                }
            });
            dialog.present(Some(&this.window));
        });

        let this = Rc::clone(ui);
        ui.apply_action
            .connect_clicked(move |_| this.apply_action());
        let this = Rc::clone(ui);
        ui.reset_action
            .connect_clicked(move |_| this.reset_action());
        let this = Rc::clone(ui);
        ui.voice_apply_button
            .connect_clicked(move |_| this.apply_voice_key());
        let this = Rc::clone(ui);
        ui.voice_reset_button
            .connect_clicked(move |_| this.reset_voice_key());
        let this = Rc::clone(ui);
        ui.voice_learn_button
            .connect_clicked(move |_| this.learn_voice_key());
        let this = Rc::clone(ui);
        ui.learn_key_button
            .connect_clicked(move |_| this.learn_key());
        let this = Rc::clone(ui);
        ui.learn_remote_button
            .connect_clicked(move |_| this.learn_remote());

        let this = Rc::clone(ui);
        ui.run_once_button.connect_clicked(move |_| this.run_once());

        let this = Rc::clone(ui);
        ui.retry_button
            .connect_clicked(move |_| this.backend.load());

        let this = Rc::clone(ui);
        let revert = gio::SimpleAction::new("revert", None);
        revert.connect_activate(move |_, _| this.revert());
        ui.window.add_action(&revert);

        let this = Rc::clone(ui);
        let about = gio::SimpleAction::new("about", None);
        about.connect_activate(move |_, _| {
            let dialog = this.about_dialog();
            dialog.present(Some(&this.window));
        });
        ui.window.add_action(&about);
    }

    fn select_button(&self, button: RemoteButton) {
        if button == RemoteButton::Voice {
            self.voice_selected.set(true);
            self.split.set_show_content(true);
            self.refresh_all();
            return;
        }
        self.voice_selected.set(false);
        if let Some(model) = self.model.borrow_mut().as_mut() {
            model.selected = button;
        }
        self.split.set_show_content(true);
        self.refresh_all();
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
                self.refresh_hardware();
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
                    if button == RemoteButton::Voice {
                        self.voice_selected.set(true);
                    } else {
                        self.voice_selected.set(false);
                        if let Some(model) = self.model.borrow_mut().as_mut() {
                            model.selected = button;
                        }
                    }
                    self.force_capture_off();
                    self.split.set_show_content(true);
                    let trigger = match trigger_from_name(&event.trigger) {
                        Some(trigger) => trigger_label(trigger),
                        None => event.trigger.as_str(),
                    };
                    self.toast(&tr_args(
                        "Selected {button} · {trigger}",
                        &[("button", button_label(button)), ("trigger", trigger)],
                    ));
                    self.refresh_all();
                }
            }
            CaptureMode::Test => {
                if event.pressed {
                    self.pressed.set(Some(button));
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
                        action
                            .clone()
                            .unwrap_or_else(|| tr("No action").to_string())
                    };
                    let trigger = match trigger_from_name(&event.trigger) {
                        Some(trigger) => trigger_label(trigger),
                        None => event.trigger.as_str(),
                    };
                    self.test_result_row.set_subtitle(&tr_args(
                        "{button} · {trigger} → {action}",
                        &[
                            ("button", button_label(button)),
                            ("trigger", trigger),
                            ("action", &shown),
                        ],
                    ));
                    let runnable = button != RemoteButton::Voice
                        && action.as_deref().is_some_and(|value| value != "none");
                    *self.last_action.borrow_mut() = action;
                    self.run_once_button.set_sensitive(runnable);
                } else {
                    self.pressed.set(None);
                }
                if let Some(model) = self.model.borrow().as_ref() {
                    self.diagram
                        .refresh(model, self.pressed.get(), self.voice_selected.get());
                }
            }
            CaptureMode::Off => {}
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
            self.test_result_row
                .set_subtitle(tr("Waiting for a remote key…"));
            *self.last_action.borrow_mut() = None;
            self.run_once_button.set_sensitive(false);
        }
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
        if let Some(model) = self.model.borrow().as_ref() {
            self.diagram.refresh(model, None, self.voice_selected.get());
        }
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

    fn apply_action(&self) {
        let draft = self.draft_from_widgets();
        let allow_exec = self
            .model
            .borrow()
            .as_ref()
            .is_some_and(|model| model.document.allow_exec);
        if let Err(error) = draft.validate(allow_exec) {
            self.show_action_error(&error);
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
                self.show_action_error("");
                self.toast(tr("Mapping updated"));
                self.refresh_all();
            }
            Err(error) => self.show_action_error(&error),
        }
    }

    fn reset_action(&self) {
        let result = self
            .model
            .borrow_mut()
            .as_mut()
            .ok_or_else(|| tr("no configuration loaded").to_string())
            .and_then(|model| model.set_slot_raw(None));
        match result {
            Ok(_) => {
                self.show_action_error("");
                self.toast(tr("Profile default restored"));
                self.refresh_all();
            }
            Err(error) => self.show_action_error(&error),
        }
    }

    fn apply_voice_key(&self) {
        let raw = self.voice_value_row.text().to_string();
        let result = self
            .model
            .borrow_mut()
            .as_mut()
            .ok_or_else(|| tr("no configuration loaded").to_string())
            .and_then(|model| model.set_trigger_key(Some(raw)));
        match result {
            Ok(_) => {
                self.show_voice_error("");
                self.toast(tr("Recording output key updated"));
                self.refresh_all();
            }
            Err(error) => self.show_voice_error(&error),
        }
    }

    fn reset_voice_key(&self) {
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
                self.show_voice_error("");
                self.toast(tr("Recording output key reset"));
                self.refresh_all();
            }
            Err(error) => self.show_voice_error(&error),
        }
    }

    fn show_voice_error(&self, message: &str) {
        self.voice_error.set_text(message);
        self.voice_error.set_visible(!message.is_empty());
    }

    fn draft_from_widgets(&self) -> ActionDraft {
        let kind = ActionKind::from_index(self.type_row.selected());
        let text = self.value_row.text().to_string();
        ActionDraft {
            kind,
            key: text.clone(),
            chord: text,
            command: self.exec_row.text().to_string(),
            dictation: DictationChoice::from_index(self.dictation_row.selected()),
        }
    }

    fn set_learned_key(&self, learned: &str) {
        let draft = ActionDraft::from_raw(learned);
        self.loading.set(true);
        self.type_row.set_selected(draft.kind.index());
        if draft.kind == ActionKind::Key {
            self.value_row.set_text(&draft.key);
        } else if draft.kind == ActionKind::Combo {
            self.value_row.set_text(&draft.chord);
        }
        self.loading.set(false);
        let allow_exec = self
            .model
            .borrow()
            .as_ref()
            .is_some_and(|model| model.document.allow_exec);
        self.update_editor_visibility(draft.kind, allow_exec);
        self.show_action_error("");
    }

    fn set_learned_voice_key(&self, learned: &str) {
        let draft = ActionDraft::from_raw(learned);
        let value = match draft.kind {
            ActionKind::Key => draft.key,
            ActionKind::Combo => draft.chord,
            _ => return,
        };
        self.voice_value_row.set_text(&value);
        self.show_voice_error("");
    }

    fn learn_key(self: &Rc<Self>) {
        self.learn_key_for(false);
    }

    fn learn_voice_key(self: &Rc<Self>) {
        self.learn_key_for(true);
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
                        this.set_learned_voice_key(&learned);
                    } else {
                        this.set_learned_key(&learned);
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
        let action = self.last_action.borrow().clone();
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

    fn update_editor_visibility(&self, kind: ActionKind, allow_exec: bool) {
        let key_value = matches!(kind, ActionKind::Key | ActionKind::Combo);
        self.value_row.set_visible(key_value);
        self.learn_key_button.set_visible(key_value);
        self.value_row.set_title(if kind == ActionKind::Combo {
            tr("Chord")
        } else {
            tr("Key")
        });
        self.exec_row.set_visible(kind == ActionKind::Exec);
        self.exec_warning.set_visible(kind == ActionKind::Exec);
        self.exec_row.set_sensitive(allow_exec);
        self.dictation_row
            .set_visible(kind == ActionKind::Dictation);
        self.exec_warning.set_text(if allow_exec {
            tr("The command runs as your user with a 5 s timeout and closed stdio.")
        } else {
            tr("Shell commands are disabled. Turn on “Allow shell commands” below first.")
        });
        self.apply_action
            .set_sensitive(kind != ActionKind::Exec || allow_exec);
    }

    fn refresh_all(&self) {
        self.refresh_title();
        self.refresh_hardware();
        let model = self.model.borrow();
        let Some(model) = model.as_ref() else {
            self.apply_button.set_sensitive(false);
            self.revert_button.set_sensitive(false);
            self.dirty_banner.set_revealed(false);
            self.test_button.set_sensitive(false);
            return;
        };
        self.diagram
            .refresh(model, self.pressed.get(), self.voice_selected.get());
        self.apply_button.set_sensitive(model.dirty);
        self.revert_button.set_sensitive(model.dirty);
        self.dirty_banner.set_revealed(model.dirty);
        self.test_button.set_sensitive(true);
        let default_hint = if model.selected_raw().is_none() {
            tr(" · profile default")
        } else {
            ""
        };
        if self.voice_selected.get() {
            self.selected_row
                .set_subtitle(tr("Voice key · output configurable"));
        } else {
            self.selected_row.set_subtitle(&tr_args(
                "{button} · {trigger}{default_hint}",
                &[
                    ("button", button_label(model.selected)),
                    ("trigger", trigger_label(model.trigger)),
                    ("default_hint", default_hint),
                ],
            ));
        }
        self.refresh_editor(model);
    }

    fn refresh_title(&self) {
        let status = self.status.borrow();
        let subtitle = match status.as_ref() {
            None => tr("Connecting…").to_string(),
            Some(status) if status.connected => {
                let device = if status.device.name.is_empty() {
                    status.device.address.clone()
                } else {
                    status.device.name.clone()
                };
                if status.streaming {
                    tr_args("{device} · recording", &[("device", &device)])
                } else if device.is_empty() {
                    status.state.clone()
                } else {
                    tr_args(
                        "{device} · {state}",
                        &[("device", &device), ("state", &status.state)],
                    )
                }
            }
            Some(status) => tr_args("Disconnected · {state}", &[("state", &status.state)]),
        };
        self.title.set_subtitle(&subtitle);
    }

    fn refresh_hardware(&self) {
        let status = self.status.borrow();
        let hardware = status.as_ref().map(|status| &status.hardware);
        let connected = status.as_ref().is_some_and(|status| status.connected);
        let percent = hardware.and_then(|hardware| hardware.battery_percent);
        let (subtitle, icon) = match percent {
            Some(percent) if connected => (format!("{percent} %"), battery_icon(percent)),
            Some(percent) => {
                let text = percent.to_string();
                (
                    tr_args("{percent} % · last known", &[("percent", &text)]),
                    battery_icon(percent),
                )
            }
            None => (tr("Unknown").to_string(), "battery-empty-symbolic"),
        };
        self.battery_row.set_subtitle(&subtitle);
        self.battery_icon.set_icon_name(Some(icon));
        self.battery_icon.set_visible(percent.is_some());

        let mut any = false;
        for (field, row) in &self.hardware_rows {
            let value = hardware.and_then(|hardware| field.value(hardware));
            row.set_subtitle(value.unwrap_or(""));
            row.set_visible(value.is_some());
            any |= value.is_some();
        }
        self.hardware_group.set_visible(any);
    }

    fn refresh_editor(&self, model: &GuiModel) {
        if self.voice_selected.get() {
            self.key_group.set_visible(false);
            self.action_group.set_visible(false);
            self.voice_group.set_visible(true);
            self.loading.set(true);
            self.voice_value_row
                .set_text(model.document.trigger_key.as_deref().unwrap_or("none"));
            self.voice_reset_button.set_sensitive(
                model.document.trigger_key.as_deref() != Some(atvv_config::DEFAULT_TRIGGER_KEY),
            );
            self.loading.set(false);
            self.show_voice_error("");
            return;
        }
        self.key_group.set_visible(true);
        self.action_group.set_visible(true);
        self.voice_group.set_visible(false);
        self.loading.set(true);
        self.trigger_row.set_selected(match model.trigger {
            Trigger::Single => 0,
            Trigger::Double => 1,
            Trigger::Long => 2,
        });
        let draft = ActionDraft::from_raw(model.selected_effective().as_deref().unwrap_or("none"));
        self.type_row.set_selected(draft.kind.index());
        self.value_row.set_text(match draft.kind {
            ActionKind::Key => &draft.key,
            ActionKind::Combo => &draft.chord,
            _ => "",
        });
        self.exec_row.set_text(&draft.command);
        self.dictation_row.set_selected(draft.dictation.index());
        self.allow_exec_row.set_active(model.document.allow_exec);
        self.reset_action
            .set_sensitive(model.selected_raw().is_some());
        self.update_editor_visibility(draft.kind, model.document.allow_exec);
        self.show_action_error("");
        self.loading.set(false);
    }

    fn show_action_error(&self, message: &str) {
        self.action_error.set_text(message);
        self.action_error.set_visible(!message.is_empty());
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
        self.refresh_hardware();
    }

    #[doc(hidden)]
    #[must_use]
    pub fn battery_text(&self) -> String {
        self.battery_row
            .subtitle()
            .map(|subtitle| subtitle.to_string())
            .unwrap_or_default()
    }

    #[doc(hidden)]
    #[must_use]
    pub fn hardware_text(&self, field: HardwareField) -> Option<String> {
        self.hardware_rows
            .iter()
            .find(|(candidate, _)| *candidate == field)
            .and_then(|(_, row)| row.subtitle().map(|subtitle| subtitle.to_string()))
    }

    #[doc(hidden)]
    #[must_use]
    pub fn diagram(&self) -> &RemoteDiagram {
        &self.diagram
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
    pub fn value_text(&self) -> String {
        self.value_row.text().to_string()
    }

    #[doc(hidden)]
    #[must_use]
    pub fn value_row(&self) -> &adw::EntryRow {
        &self.value_row
    }

    #[doc(hidden)]
    #[must_use]
    pub fn apply_button(&self) -> &gtk::Button {
        &self.apply_button
    }

    #[doc(hidden)]
    #[must_use]
    pub fn action_error(&self) -> &gtk::Label {
        &self.action_error
    }

    #[doc(hidden)]
    #[must_use]
    pub fn trigger_row(&self) -> &adw::ComboRow {
        &self.trigger_row
    }

    #[doc(hidden)]
    #[must_use]
    pub fn type_row(&self) -> &adw::ComboRow {
        &self.type_row
    }

    #[doc(hidden)]
    #[must_use]
    pub fn update_button(&self) -> &gtk::Button {
        &self.apply_action
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
        &self.voice_value_row
    }

    #[doc(hidden)]
    #[must_use]
    pub fn voice_update_button(&self) -> &gtk::Button {
        &self.voice_apply_button
    }

    #[doc(hidden)]
    #[must_use]
    pub fn test_button(&self) -> &gtk::ToggleButton {
        &self.test_button
    }

    #[doc(hidden)]
    #[must_use]
    pub fn test_result_text(&self) -> String {
        self.test_result_row
            .subtitle()
            .map(|subtitle| subtitle.to_string())
            .unwrap_or_default()
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

#[must_use]
fn trigger_label(trigger: Trigger) -> &'static str {
    match trigger {
        Trigger::Single => tr("Single"),
        Trigger::Double => tr("Double"),
        Trigger::Long => tr("Long"),
    }
}

/// Parse the protocol name used by `ButtonEvent`.
#[must_use]
fn trigger_from_name(name: &str) -> Option<Trigger> {
    match name {
        "single" => Some(Trigger::Single),
        "double" => Some(Trigger::Double),
        "long" => Some(Trigger::Long),
        _ => None,
    }
}

/// Build a window for display tests.
#[must_use]
pub fn build_window(application: Option<&adw::Application>, backend: Backend) -> Rc<AppWindow> {
    AppWindow::build(application, backend)
}
