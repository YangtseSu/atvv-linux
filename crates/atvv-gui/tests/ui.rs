// SPDX-License-Identifier: GPL-3.0-or-later

//! Headless GTK smoke test: the board shows every card and slot, hotspots and
//! slots share one selection, the editor dialog validates and updates the
//! document, test mode highlights the pressed slot, and XTest pointer clicks
//! reach every slot and hotspot.
//!
//! GTK must be initialized and used from one thread, so the scenarios run in
//! one test. Run under `xvfb-run`.

use std::time::{Duration, Instant};

use atvv_config::{Config, ConfigDocument};
use atvv_gui::app::{AppWindow, HardwareField};
use atvv_gui::checklist::{CheckState, Layer};
use atvv_gui::client::{
    AudioInfo, Backend, ButtonSignal, ContractInfo, DaemonStatus, DeviceInfo, HardwareInfo,
    InputInfo, VinputInfo,
};
use atvv_gui::model::GuiModel;
use atvv_input::Trigger;
use atvv_protocol::profile::RemoteButton;
use atvv_protocol::{CHROMECAST, RC003};
use gtk::glib::variant::ToVariant;
use gtk::prelude::*;
use libadwaita::prelude::*;

const SLOTS: [Trigger; 3] = [Trigger::Single, Trigger::Double, Trigger::Long];

fn pump(millis: u64) {
    let deadline = Instant::now() + Duration::from_millis(millis);
    while Instant::now() < deadline {
        while gtk::glib::MainContext::default().iteration(false) {}
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn window() -> std::rc::Rc<AppWindow> {
    window_for(&RC003)
}

fn window_for(profile: &atvv_protocol::profile::DeviceProfile) -> std::rc::Rc<AppWindow> {
    let model = GuiModel::new(ConfigDocument::from_config(&Config::defaults(profile)))
        .expect("profile is known");
    let window = AppWindow::build(None, Backend::detached());
    window.set_model(model);
    window.window().present();
    for _ in 0..3 {
        while gtk::glib::MainContext::default().iteration(false) {}
    }
    gtk::test_widget_wait_for_draw(window.window());
    pump(40);
    window
}

fn status(connected: bool, percent: u8) -> DaemonStatus {
    DaemonStatus {
        state: if connected { "ready" } else { "disconnected" }.to_string(),
        connected,
        streaming: false,
        last_error: None,
        device: if connected {
            DeviceInfo {
                name: "Xiaomi Bluetooth Remote 2 Pro (RC003)".to_string(),
                address: "C0:5D:39:F8:2D:4C".to_string(),
            }
        } else {
            DeviceInfo::default()
        },
        hardware: HardwareInfo {
            battery_percent: Some(percent),
            manufacturer: Some("MIOM".to_string()),
            model: Some("RC003".to_string()),
            serial_number: Some("250519".to_string()),
            hardware_revision: Some("V2.0".to_string()),
            firmware_revision: Some("2671".to_string()),
            software_revision: Some("A.7.0.6".to_string()),
        },
        audio: AudioInfo {
            level: 0,
            gain_db: 0,
            available: true,
        },
        input: InputInfo {
            enabled: true,
            keyboard: true,
        },
        vinput: VinputInfo {
            mode: "trigger".to_string(),
        },
        contract: ContractInfo {
            profile: "miremote-rc003".to_string(),
            source: "atvv-miremote-rc003".to_string(),
            sample_format: "s16le".to_string(),
            sample_rate: 16_000,
            channels: 1,
            recording_keys: vec![
                "DICTATE".to_string(),
                "VOICECOMMAND".to_string(),
                "ASSISTANT".to_string(),
            ],
            active_key: Some("DICTATE".to_string()),
            recipe: "Select the source and bind DICTATE.".to_string(),
        },
    }
}

#[test]
fn window_smoke() {
    if gtk::init().is_err() {
        eprintln!("skipping GUI smoke test: no display");
        return;
    }
    let _ = libadwaita::init();
    atvv_gui::app::install_css();
    atvv_gui::i18n::Language::set(atvv_gui::i18n::Language::English);

    let window = window();
    window.set_status(status(true, 62));
    assert_eq!(window.battery_text(), "62 %");
    assert_eq!(
        window.hardware_text(HardwareField::Firmware).as_deref(),
        Some("2671")
    );
    assert_eq!(
        window.hardware_text(HardwareField::Serial).as_deref(),
        Some("250519")
    );
    assert_eq!(
        window.chip_device_text(),
        "Xiaomi Bluetooth Remote 2 Pro (RC003)"
    );
    assert_eq!(window.subtitle_text(), "RC003 · ready");
    assert_eq!(
        window.device_name_text(),
        "Xiaomi Bluetooth Remote 2 Pro (RC003)"
    );

    // Microphone dialog: live level, source summary, and gain through the
    // document's Apply path.
    let mut live = status(true, 62);
    live.audio.level = 640;
    window.set_status(live);
    window.open_microphone();
    pump(20);
    assert!(window.microphone().is_open());
    assert_eq!(window.microphone().level_text(), "64 %");
    assert!((window.microphone().level_bar().value() - 0.64).abs() < 1e-6);
    assert_eq!(window.microphone().source_text(), "atvv-miremote-rc003");
    assert_eq!(window.microphone().format_text(), "s16le · 16000 Hz · 1 ch");
    assert_eq!(window.microphone().gain_text(), "0 dB");
    assert_eq!(
        window.microphone().hint_text(),
        "Saved with Apply in the main window; staged slot edits on Default are saved as a new layout."
    );

    window.microphone().gain_scale().set_value(6.0);
    pump(20);
    assert_eq!(window.microphone().gain_text(), "+6 dB");
    assert_eq!(window.document().expect("document").gain_db, 6);
    assert_eq!(window.dirty_banner_text(), "Unsaved changes");

    let mut live = status(true, 62);
    live.audio.level = 250;
    window.set_status(live);
    assert_eq!(window.microphone().level_text(), "25 %");

    window.microphone().gain_scale().set_value(0.0);
    pump(20);
    assert_eq!(window.document().expect("document").gain_db, 0);
    window.mark_saved_for_test();
    window.microphone().close();
    pump(20);
    assert!(!window.microphone().is_open());

    // Getting started checklist: four independently computed layers, copyable
    // commands, and the informational consumer row. This document has no
    // active device.
    window.open_checklist();
    pump(20);
    assert!(window.checklist().is_open());
    assert_eq!(window.checklist().state(Layer::Daemon), CheckState::Ready);
    assert_eq!(window.checklist().state(Layer::Device), CheckState::Missing);
    assert_eq!(window.checklist().state(Layer::Source), CheckState::Ready);
    assert_eq!(window.checklist().state(Layer::Input), CheckState::Ready);
    assert_eq!(
        window.checklist().body_text(Layer::Device),
        "No remote is configured yet. Pair one with atvvctl."
    );
    assert!(
        window.checklist().commands(Layer::Daemon).is_empty(),
        "a ready layer hides its recovery commands"
    );
    assert_eq!(
        window.checklist().commands(Layer::Device),
        ["atvvctl pair <MAC>"]
    );
    assert!(window.checklist().copy(Layer::Device, 0));
    assert_eq!(
        window.checklist().last_copied().as_deref(),
        Some("atvvctl pair <MAC>")
    );
    assert_eq!(
        window.checklist().consumer_text(),
        "Select the source and bind DICTATE."
    );
    assert_eq!(
        window.checklist().link_url(),
        atvv_gui::checklist::INTEGRATIONS_URL
    );
    window.checklist().close();
    pump(20);
    assert!(!window.checklist().is_open());

    // Auto-open happens once, and only when the loaded document has no
    // device. A configured remote leaves the dialog closed.
    let auto = window_for(&RC003);
    auto.loaded_for_test(
        ConfigDocument::from_config(&Config::defaults(&RC003)),
        status(false, 0),
    );
    pump(20);
    assert!(auto.checklist().is_open());
    auto.checklist().close();
    pump(20);
    auto.loaded_for_test(
        ConfigDocument::from_config(&Config::defaults(&RC003)),
        status(false, 0),
    );
    assert!(
        !auto.checklist().is_open(),
        "auto-open must run once per process"
    );
    auto.window().close();

    let mut configured = ConfigDocument::from_config(&Config::defaults(&RC003));
    configured.active_device = Some("C0:5D:39:F8:2D:4C".to_string());
    let configured_window = window_for(&RC003);
    configured_window.loaded_for_test(configured, status(true, 62));
    pump(20);
    assert!(
        !configured_window.checklist().is_open(),
        "a configured device must not auto-open the checklist"
    );
    configured_window.window().close();

    window.set_status(status(false, 61));
    assert_eq!(window.battery_text(), "61 % · last known");

    // Twelve cards with three readable slots each.
    for &button in RC003.buttons {
        let card = window
            .board()
            .card(button)
            .unwrap_or_else(|| panic!("{button} has no card"));
        assert!(card.root().width() > 0, "{button} card has no allocation");
        for trigger in SLOTS {
            let slot = window
                .board()
                .slot(button, trigger)
                .unwrap_or_else(|| panic!("{button} {trigger:?} has no slot"));
            assert!(
                slot.width() > 0 && slot.height() > 0,
                "{button} {trigger:?} slot has no allocation"
            );
            assert!(slot.tooltip_text().is_some(), "no tooltip on {button}");
            assert!(
                window.board().slot_text(button, trigger).is_some(),
                "no rendered action on {button} {trigger:?}"
            );
        }
    }
    let (rc_left, rc_right) = atvv_gui::diagram::columns_for(RC003.id);
    assert_eq!(window.board().column_buttons(true), rc_left);
    assert_eq!(window.board().column_buttons(false), rc_right);
    assert_eq!(
        window.board().slot_text(RemoteButton::Ok, Trigger::Single),
        Some("Return".to_string())
    );
    assert_eq!(
        window.board().slot_text(RemoteButton::Up, Trigger::Single),
        Some("Up".to_string())
    );
    assert_eq!(
        window
            .board()
            .slot_text(RemoteButton::Power, Trigger::Single),
        Some("None".to_string())
    );

    // A card slot selects itself and opens the editor on that exact slot.
    window
        .board()
        .slot(RemoteButton::Up, Trigger::Double)
        .expect("up double")
        .emit_clicked();
    assert_eq!(window.selected(), Some(RemoteButton::Up));
    assert_eq!(window.selected_trigger(), Some(Trigger::Double));
    assert_eq!(window.editor().title(), "Up · Double");
    assert!(
        window.editor().is_open(),
        "the slot click must open the editor"
    );
    window.editor().type_row().set_selected(1);
    window.editor().value_row().set_text("KEY_F13");
    window.editor().update_button().emit_clicked();
    pump(30);
    assert_eq!(
        window
            .staged_slot(RemoteButton::Up, Trigger::Double)
            .as_deref(),
        Some("key:KEY_F13")
    );
    assert!(
        window.template_active(),
        "a fresh document edits the Default template"
    );
    assert!(window.apply_button().is_sensitive());
    assert_eq!(
        window.board().slot_text(RemoteButton::Up, Trigger::Double),
        Some("F13".to_string())
    );

    // A second button sending the same action marks both cards as conflicts.
    window
        .board()
        .slot(RemoteButton::Down, Trigger::Double)
        .expect("down double")
        .emit_clicked();
    window.editor().type_row().set_selected(1);
    window.editor().value_row().set_text("KEY_F13");
    window.editor().update_button().emit_clicked();
    pump(30);
    assert!(
        window
            .board()
            .card(RemoteButton::Up)
            .unwrap()
            .root()
            .has_css_class("conflict")
    );
    assert!(
        window
            .board()
            .slot(RemoteButton::Up, Trigger::Double)
            .unwrap()
            .has_css_class("conflict")
    );

    // Invalid input stays in the dialog, reports inline, and keeps the map.
    window
        .board()
        .slot(RemoteButton::Up, Trigger::Single)
        .expect("up single")
        .emit_clicked();
    window.editor().type_row().set_selected(1);
    window.editor().value_row().set_text("not-a-key");
    window.editor().update_button().emit_clicked();
    assert!(window.editor().error().is_visible());
    assert!(
        window.editor().error().text().contains("not-a-key"),
        "inline error: {}",
        window.editor().error().text()
    );
    assert_eq!(
        window
            .effective_slot(RemoteButton::Up, Trigger::Single)
            .as_deref(),
        Some("key:KEY_UP"),
        "an invalid edit must not change the document"
    );
    window.editor().cancel_button().emit_clicked();
    pump(30);

    // Pointer mappings are edited structurally: button for a click, direction
    // and units for a move, with no raw BTN_* typing.
    window
        .board()
        .slot(RemoteButton::Up, Trigger::Long)
        .expect("up long")
        .emit_clicked();
    window.editor().type_row().set_selected(5);
    window.editor().pointer_kind_row().set_selected(0);
    window.editor().pointer_button_row().set_selected(1);
    window.editor().update_button().emit_clicked();
    pump(30);
    assert_eq!(
        window
            .staged_slot(RemoteButton::Up, Trigger::Long)
            .as_deref(),
        Some("pointer:click:right")
    );
    assert_eq!(
        window.board().slot_text(RemoteButton::Up, Trigger::Long),
        Some("Right click".to_string())
    );

    window
        .board()
        .slot(RemoteButton::Up, Trigger::Long)
        .expect("up long")
        .emit_clicked();
    window.editor().type_row().set_selected(5);
    window.editor().pointer_kind_row().set_selected(1);
    window.editor().pointer_direction_row().set_selected(0);
    window.editor().pointer_units_row().set_value(7.0);
    window.editor().update_button().emit_clicked();
    pump(30);
    assert_eq!(
        window
            .staged_slot(RemoteButton::Up, Trigger::Long)
            .as_deref(),
        Some("pointer:move:up:7")
    );
    assert_eq!(
        window.board().slot_text(RemoteButton::Up, Trigger::Long),
        Some("Pointer up 7".to_string())
    );

    // A hotspot selects the card and its single slot.
    window
        .diagram()
        .button(RemoteButton::Back)
        .expect("back hotspot")
        .emit_clicked();
    assert_eq!(window.selected(), Some(RemoteButton::Back));
    assert_eq!(window.selected_trigger(), Some(Trigger::Single));
    assert_eq!(window.editor().title(), "Back · Single");
    window.editor().cancel_button().emit_clicked();
    pump(30);

    // The voice key opens the recording-key dialog, not an action slot.
    window
        .diagram()
        .button(RemoteButton::Voice)
        .expect("voice hotspot")
        .emit_clicked();
    assert!(window.voice_selected());
    assert_eq!(window.voice_value_row().text(), "DICTATE");
    assert!(window.voice_declared_button().is_visible());
    assert!(window.voice_hint_text().contains("Consumers must bind"));
    assert!(window.voice_hint_text().contains("Dictate"));

    gtk::prelude::WidgetExt::activate_action(
        window.window(),
        "win.voice-key",
        Some(&"VOICECOMMAND".to_variant()),
    )
    .expect("activate the declared-key action");
    assert_eq!(window.voice_value_row().text(), "VOICECOMMAND");
    window.voice_update_button().emit_clicked();
    pump(30);
    assert_eq!(
        window.document().expect("document").trigger_key.as_deref(),
        Some("VOICECOMMAND")
    );
    assert!(window.apply_button().is_sensitive());

    window.board().voice_card().emit_clicked();
    gtk::prelude::WidgetExt::activate_action(
        window.window(),
        "win.voice-key",
        Some(&"none".to_variant()),
    )
    .expect("activate the none choice");
    window.voice_update_button().emit_clicked();
    pump(30);
    assert_eq!(window.document().expect("document").trigger_key, None);

    window.board().voice_card().emit_clicked();
    window.voice_value_row().set_text("F23");
    window.voice_update_button().emit_clicked();
    pump(30);
    assert_eq!(
        window.document().expect("document").trigger_key.as_deref(),
        Some("F23")
    );
    assert_eq!(
        window.board().voice_output().text().as_str(),
        "F23",
        "the voice card shows the output key"
    );

    // The searchable picker lists canonical names and inserts the chosen key.
    window
        .board()
        .slot(RemoteButton::Menu, Trigger::Long)
        .expect("menu long")
        .emit_clicked();
    window.editor().type_row().set_selected(1);
    window.editor().choose_key_button().emit_clicked();
    pump(30);
    assert!(window.editor().picker().is_open());
    window.editor().picker().search().set_text("print");
    pump(30);
    let visible = window.editor().picker().visible_names();
    assert!(
        visible.contains(&"KEY_SYSRQ"),
        "Print Screen missing from the filtered list: {visible:?}"
    );
    assert!(
        !visible.contains(&"KEY_A"),
        "filter did not hide unrelated keys: {visible:?}"
    );
    assert!(window.editor().picker().select("KEY_SYSRQ"));
    pump(30);
    assert!(!window.editor().picker().is_open());
    assert_eq!(window.editor().value_row().text().as_str(), "KEY_SYSRQ");
    window.editor().update_button().emit_clicked();
    pump(30);
    assert_eq!(
        window
            .staged_slot(RemoteButton::Menu, Trigger::Long)
            .as_deref(),
        Some("key:KEY_SYSRQ")
    );
    assert_eq!(
        window.board().slot_text(RemoteButton::Menu, Trigger::Long),
        Some("Print Screen".to_string())
    );

    // A pick inside a chord keeps the held modifiers.
    window
        .board()
        .slot(RemoteButton::Menu, Trigger::Long)
        .expect("menu long")
        .emit_clicked();
    window.editor().type_row().set_selected(2);
    window.editor().value_row().set_text("ctrl+alt+v");
    window.editor().choose_key_button().emit_clicked();
    pump(30);
    assert!(window.editor().picker().select("KEY_SYSRQ"));
    assert_eq!(
        window.editor().value_row().text().as_str(),
        "ctrl+alt+KEY_SYSRQ"
    );
    window.editor().cancel_button().emit_clicked();
    pump(30);

    // exec stays opt-in: the dialog rejects it while the policy is off.
    window
        .board()
        .slot(RemoteButton::Tv, Trigger::Long)
        .expect("tv long")
        .emit_clicked();
    window.editor().type_row().set_selected(3);
    window.editor().exec_row().set_text("notify-send hi");
    window.editor().update_button().emit_clicked();
    assert!(
        window.editor().error().text().contains("allow_exec"),
        "exec error: {}",
        window.editor().error().text()
    );
    assert_eq!(window.staged_slot(RemoteButton::Tv, Trigger::Long), None);
    window.editor().cancel_button().emit_clicked();
    pump(30);

    // Test mode selects the pressed slot and exposes Run once.
    window.test_button().set_active(true);
    window.simulate_button_event(ButtonSignal {
        button: "ok".to_string(),
        trigger: "single".to_string(),
        pressed: true,
    });
    assert_eq!(window.selected(), Some(RemoteButton::Ok));
    assert_eq!(window.selected_trigger(), Some(Trigger::Single));
    assert!(
        window
            .board()
            .slot(RemoteButton::Ok, Trigger::Single)
            .unwrap()
            .has_css_class("test")
    );
    assert_eq!(window.test_result_text(), "OK · Single");
    assert_eq!(window.last_action_text(), "→ Return");
    window.simulate_button_event(ButtonSignal {
        button: "ok".to_string(),
        trigger: "single".to_string(),
        pressed: false,
    });
    assert!(
        !window
            .board()
            .slot(RemoteButton::Ok, Trigger::Single)
            .unwrap()
            .has_css_class("test")
    );
    window.test_button().set_active(false);
    pump(30);

    // Layout chip, template staging, save-as, and the manager.
    assert_eq!(window.layout_chip_text(), "Default");
    assert!(window.template_active());
    window
        .board()
        .slot(RemoteButton::Up, Trigger::Single)
        .expect("up single")
        .emit_clicked();
    window.editor().type_row().set_selected(1);
    window.editor().value_row().set_text("KEY_F13");
    window.editor().update_button().emit_clicked();
    pump(30);
    assert_eq!(
        window.dirty_banner_text(),
        "Unsaved changes on Default — Apply saves them as a new layout."
    );
    window.apply_button().emit_clicked();
    pump(30);
    assert!(window.layout_form_is_visible());
    assert!(window.layout_manager_is_open());
    assert_eq!(window.layout_name_row().text(), "My layout");
    window.layout_name_row().set_text("Coding");
    window.layout_form_submit().emit_clicked();
    pump(30);
    assert!(!window.layout_form_is_visible());
    assert_eq!(window.active_layout_id().as_deref(), Some("coding"));
    assert_eq!(window.layout_chip_text(), "Coding");
    assert!(!window.template_active());
    assert_eq!(
        window.document().expect("document").layouts["coding"]
            .slot(RemoteButton::Up, Trigger::Single),
        Some("key:KEY_F13")
    );
    assert_eq!(window.dirty_banner_text(), "Unsaved changes");
    window.mark_saved_for_test();

    // The manager tags the template and disables its destructive actions.
    window.present_layout_manager();
    pump(30);
    assert_eq!(
        window
            .layout_manager_row("default")
            .expect("default row")
            .title(),
        "Default"
    );
    assert!(window.layout_manager_row("coding").is_some());
    for action in ["rename", "delete", "reset"] {
        assert!(
            !window
                .layout_manager_action("default", action)
                .unwrap()
                .is_sensitive(),
            "Default must not offer {action}"
        );
    }
    assert!(
        window
            .layout_manager_action("default", "duplicate")
            .unwrap()
            .is_sensitive()
    );

    // A dirty document refuses to switch layouts.
    window
        .board()
        .slot(RemoteButton::Ok, Trigger::Single)
        .expect("ok single")
        .emit_clicked();
    window.editor().type_row().set_selected(1);
    window.editor().value_row().set_text("KEY_F13");
    window.editor().update_button().emit_clicked();
    pump(30);
    libadwaita::prelude::ActionRowExt::activate(&window.layout_manager_row("default").unwrap());
    pump(30);
    assert_eq!(window.active_layout_id().as_deref(), Some("coding"));
    window.mark_saved_for_test();

    // Rename, duplicate, delete, and reset through the manager.
    window.present_layout_manager();
    pump(30);
    window
        .layout_manager_action("coding", "rename")
        .unwrap()
        .emit_clicked();
    pump(30);
    assert!(window.layout_form_is_visible());
    window.layout_name_row().set_text("Editor");
    window.layout_form_submit().emit_clicked();
    pump(30);
    assert_eq!(
        window.layout_manager_row("coding").unwrap().title(),
        "Editor"
    );
    window
        .layout_manager_action("coding", "duplicate")
        .unwrap()
        .emit_clicked();
    pump(30);
    assert!(window.layout_manager_row("editor-copy").is_some());
    window
        .layout_manager_action("editor-copy", "delete")
        .unwrap()
        .emit_clicked();
    pump(30);
    assert!(window.layout_manager_row("editor-copy").is_none());
    assert!(
        !window
            .layout_manager_action("coding", "delete")
            .unwrap()
            .is_sensitive()
    );
    window
        .layout_manager_action("coding", "reset")
        .unwrap()
        .emit_clicked();
    pump(30);
    assert_eq!(window.staged_slot(RemoteButton::Up, Trigger::Single), None);
    assert_eq!(
        window
            .effective_slot(RemoteButton::Up, Trigger::Single)
            .as_deref(),
        Some("key:KEY_UP")
    );
    assert_eq!(window.layout_chip_text(), "Editor");

    // Preset picker: preview omp and create an inactive layout.
    window.open_preset_dialog_for_test(None);
    pump(30);
    assert!(window.preset_dialog_is_open());
    window.select_preset_for_test("omp");
    pump(20);
    assert_eq!(window.preset_name_row().text(), "omp");
    assert_eq!(
        window
            .preset_preview_row("power")
            .unwrap()
            .subtitle()
            .as_deref(),
        Some("Alt+R")
    );
    assert_eq!(
        window
            .preset_preview_row("tv")
            .unwrap()
            .subtitle()
            .as_deref(),
        Some("Ctrl+O")
    );
    window.preset_submit_button().emit_clicked();
    pump(400);
    assert_eq!(window.preset_selection(), Some("omp"));
    window.close_preset_dialog_for_test();
    pump(50);
    assert_eq!(window.active_layout_id().as_deref(), Some("coding"));
    let document = window.document().expect("document");
    let layout = document.layouts.get("omp-layout").expect("preset layout");
    assert_eq!(
        layout.slot(RemoteButton::Power, Trigger::Single),
        Some("combo:alt+r")
    );
    assert_eq!(
        layout.slot(RemoteButton::Tv, Trigger::Single),
        Some("combo:ctrl+o")
    );
    window.mark_saved_for_test();

    // Reset-to-preset shows the affected layout and the preset preview.
    window.open_preset_dialog_for_test(Some("coding"));
    pump(20);
    assert_eq!(
        window.preset_warning_text(),
        "This replaces every slot of “Editor”. The layout keeps its name and description."
    );
    window.select_preset_for_test("mpv");
    pump(10);
    assert_eq!(
        window
            .preset_preview_row("power")
            .unwrap()
            .subtitle()
            .as_deref(),
        Some("Q")
    );
    window.close_preset_dialog_for_test();
    pump(20);
    window.close_layout_manager();
    pump(400);

    // Real pointer clicks through XTEST: every card slot and every hotspot.
    if let Some(pointer) = XPointer::connect("ATVV Remote", window.window()) {
        if !pointer.covers() {
            eprintln!(
                "XTest pointer checks skipped: the X screen does not fit the {width}x{height} window",
                width = window.window().width(),
                height = window.window().height()
            );
            window.window().close();
            return;
        }
        for &button in RC003.buttons {
            for trigger in SLOTS {
                let slot = window.board().slot(button, trigger).expect("slot");
                window.scroll_slot_into_view(button, trigger);
                pump(40);
                pointer.click(slot);
                pump(120);
                assert_eq!(
                    window.selected(),
                    Some(button),
                    "{button} {trigger:?} slot did not receive the click"
                );
                assert_eq!(window.selected_trigger(), Some(trigger));
                assert!(window.editor().is_open());
                window.editor().close();
                pump(400);
            }
        }
        for button in RC003
            .buttons
            .iter()
            .copied()
            .chain(std::iter::once(RemoteButton::Voice))
        {
            let hotspot = window.diagram().button(button).expect("hotspot");
            window.scroll_board_to(hotspot);
            pump(40);
            pointer.click(hotspot);
            pump(120);
            if button == RemoteButton::Voice {
                assert!(window.voice_selected(), "voice hotspot missed");
                window.voice_editor().close();
            } else {
                assert_eq!(
                    window.selected(),
                    Some(button),
                    "{button} hotspot did not receive the click"
                );
                assert_eq!(window.selected_trigger(), Some(Trigger::Single));
                window.editor().close();
            }
            pump(400);
        }
    } else {
        eprintln!("XTest pointer checks skipped: no X11 window titled ATVV Remote");
    }

    window.window().close();

    // The Chromecast profile exposes its own button set and app keys.
    let google = window_for(&CHROMECAST);
    for &button in CHROMECAST.buttons {
        let card = google
            .board()
            .card(button)
            .unwrap_or_else(|| panic!("{button} has no card"));
        assert!(card.root().is_visible(), "{button} card is hidden");
    }
    let (google_left, google_right) = atvv_gui::diagram::columns_for(CHROMECAST.id);
    assert_eq!(google.board().column_buttons(true), google_left);
    assert_eq!(google.board().column_buttons(false), google_right);
    let menu = google.board().card(RemoteButton::Menu).expect("menu card");
    assert!(!menu.root().is_visible(), "Chromecast has no menu card");
    assert_eq!(
        google
            .board()
            .card(RemoteButton::Tv)
            .expect("input card")
            .label_text(),
        "Input"
    );
    assert_eq!(
        google
            .board()
            .card(RemoteButton::Mute)
            .expect("mute card")
            .label_text(),
        "Mute"
    );
    assert!(google.diagram().button(RemoteButton::Mute).is_some());
    assert!(
        google
            .diagram()
            .button(RemoteButton::Mute)
            .unwrap()
            .is_visible()
    );
    assert!(
        google
            .diagram()
            .button(RemoteButton::YouTube)
            .unwrap()
            .is_visible()
    );
    assert!(
        google
            .diagram()
            .button(RemoteButton::Netflix)
            .unwrap()
            .is_visible()
    );
    assert!(
        !google
            .diagram()
            .button(RemoteButton::Menu)
            .unwrap()
            .is_visible()
    );
    assert_eq!(
        google
            .board()
            .slot_text(RemoteButton::Mute, Trigger::Single),
        Some("Mute".to_string())
    );
    assert_eq!(
        google
            .board()
            .slot_text(RemoteButton::YouTube, Trigger::Single),
        Some("Camera Access Disable".to_string())
    );

    // Every new app key opens the same slot editor.
    for button in [
        RemoteButton::Mute,
        RemoteButton::YouTube,
        RemoteButton::Netflix,
        RemoteButton::Tv,
    ] {
        google
            .board()
            .slot(button, Trigger::Single)
            .unwrap_or_else(|| panic!("{button} slot"))
            .emit_clicked();
        assert_eq!(google.selected(), Some(button));
        assert!(
            google.editor().is_open(),
            "{button} did not open the editor"
        );
        google.editor().close();
        pump(20);
    }
    google
        .diagram()
        .button(RemoteButton::YouTube)
        .expect("youtube hotspot")
        .emit_clicked();
    assert_eq!(google.selected(), Some(RemoteButton::YouTube));
    assert!(google.editor().is_open());
    google.editor().close();
    pump(20);

    // Preset preview follows the active profile: no Menu row, and buttons the
    // preset does not define stay at their profile default.
    google.open_preset_dialog_for_test(None);
    pump(20);
    google.select_preset_for_test("mpv");
    pump(10);
    assert!(!google.preset_preview_row("menu").unwrap().is_visible());
    assert!(google.preset_preview_row("mute").unwrap().is_visible());
    assert_eq!(
        google
            .preset_preview_row("mute")
            .unwrap()
            .subtitle()
            .as_deref(),
        Some("")
    );
    assert_eq!(
        google
            .preset_preview_row("tv")
            .unwrap()
            .subtitle()
            .as_deref(),
        Some("F")
    );
    google.close_preset_dialog_for_test();
    pump(20);

    // The session-driven Assistant key reports through the same test bar.
    google.test_button().set_active(true);
    google.simulate_button_event(ButtonSignal {
        button: "voice".to_string(),
        trigger: "single".to_string(),
        pressed: true,
    });
    assert_eq!(google.test_result_text(), "Voice · Single");
    assert_eq!(
        google.last_action_text(),
        "→ Microphone key (not remappable)"
    );
    assert!(
        google
            .diagram()
            .button(RemoteButton::Voice)
            .unwrap()
            .has_css_class("pressed")
    );
    google.simulate_button_event(ButtonSignal {
        button: "voice".to_string(),
        trigger: "single".to_string(),
        pressed: false,
    });
    assert!(
        !google
            .diagram()
            .button(RemoteButton::Voice)
            .unwrap()
            .has_css_class("pressed")
    );
    google.test_button().set_active(false);
    pump(20);
    google.window().close();
}

/// XTEST pointer driver over the X11 window that GDK created.
struct XPointer {
    connection: x11rb::rust_connection::RustConnection,
    root: x11rb::protocol::xproto::Window,
    screen: (u16, u16),
    origin: (i16, i16),
    window: gtk::Window,
}

impl XPointer {
    fn connect(title: &str, window: &impl IsA<gtk::Window>) -> Option<Self> {
        use x11rb::connection::Connection;
        use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _};

        let (connection, screen_num) = x11rb::connect(None).ok()?;
        let root = connection.setup().roots[screen_num].root;
        let geometry = connection.get_geometry(root).ok()?.reply().ok()?;
        let children = connection.query_tree(root).ok()?.reply().ok()?.children;
        for child in children {
            let Ok(cookie) =
                connection.get_property(false, child, AtomEnum::WM_NAME, AtomEnum::ANY, 0, 1024)
            else {
                continue;
            };
            let Ok(reply) = cookie.reply() else {
                continue;
            };
            if reply.value != title.as_bytes() {
                continue;
            }
            let translated = connection
                .translate_coordinates(child, root, 0, 0)
                .ok()?
                .reply()
                .ok()?;
            return Some(Self {
                connection,
                root,
                screen: (geometry.width, geometry.height),
                origin: (translated.dst_x, translated.dst_y),
                window: window.clone().upcast(),
            });
        }
        None
    }

    /// Whether the whole window fits on the X screen, so every click lands.
    fn covers(&self) -> bool {
        let Ok(width) = u16::try_from(self.window.width()) else {
            return false;
        };
        let Ok(height) = u16::try_from(self.window.height()) else {
            return false;
        };
        self.origin.0 >= 0
            && self.origin.1 >= 0
            && u32::from(self.origin.0 as u16) + u32::from(width) <= u32::from(self.screen.0)
            && u32::from(self.origin.1 as u16) + u32::from(height) <= u32::from(self.screen.1)
    }

    /// Click the center of `widget` in root coordinates.
    fn click(&self, widget: &impl IsA<gtk::Widget>) {
        use x11rb::connection::Connection;
        use x11rb::protocol::xproto::{
            BUTTON_PRESS_EVENT, BUTTON_RELEASE_EVENT, MOTION_NOTIFY_EVENT,
        };
        use x11rb::protocol::xtest::ConnectionExt as _;

        let Some(bounds) = widget.compute_bounds(&self.window) else {
            return;
        };

        let x = self.origin.0
            + i16::try_from((bounds.x() + bounds.width() / 2.0).round() as i64).unwrap_or(0);
        let y = self.origin.1
            + i16::try_from((bounds.y() + bounds.height() / 2.0).round() as i64).unwrap_or(0);
        let _ = self.connection.xtest_fake_input(
            MOTION_NOTIFY_EVENT,
            0,
            x11rb::CURRENT_TIME,
            self.root,
            x,
            y,
            0,
        );
        let _ = self.connection.xtest_fake_input(
            BUTTON_PRESS_EVENT,
            1,
            x11rb::CURRENT_TIME,
            self.root,
            0,
            0,
            0,
        );
        let _ = self.connection.xtest_fake_input(
            BUTTON_RELEASE_EVENT,
            1,
            x11rb::CURRENT_TIME,
            self.root,
            0,
            0,
            0,
        );
        let _ = self.connection.flush();
    }
}
