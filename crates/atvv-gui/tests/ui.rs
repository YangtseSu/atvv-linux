// SPDX-License-Identifier: GPL-3.0-or-later

//! Headless GTK smoke test: the window builds and maps, hotspots select keys,
//! and the action editor writes into the shared document.
//!
//! GTK must be initialized and used from one thread, so the scenarios run in
//! one test. Run under `xvfb-run`.

use atvv_config::{Config, ConfigDocument};
use atvv_gui::app::{AppWindow, HardwareField};
use atvv_gui::client::{Backend, DaemonStatus, DeviceInfo, HardwareInfo};
use atvv_gui::model::GuiModel;
use atvv_protocol::RC003;
use atvv_protocol::profile::RemoteButton;
use gtk::prelude::*;
use libadwaita::prelude::*;

fn window() -> std::rc::Rc<AppWindow> {
    let model = GuiModel::new(ConfigDocument::from_config(&Config::defaults(&RC003)))
        .expect("profile is known");
    let window = AppWindow::build(None, Backend::start(None));
    window.set_model(model);
    window.window().present();
    for _ in 0..3 {
        while gtk::glib::MainContext::default().iteration(false) {}
    }
    gtk::test_widget_wait_for_draw(window.window());
    window
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
    window.set_status(DaemonStatus {
        state: "ready".to_string(),
        connected: true,
        streaming: false,
        last_error: None,
        device: DeviceInfo {
            name: "Xiaomi Bluetooth Remote 2 Pro (RC003)".to_string(),
            address: "C0:5D:39:F8:2D:4C".to_string(),
        },
        hardware: HardwareInfo {
            battery_percent: Some(62),
            manufacturer: Some("MIOM".to_string()),
            model: Some("RC003".to_string()),
            serial_number: Some("250519".to_string()),
            hardware_revision: Some("V2.0".to_string()),
            firmware_revision: Some("2671".to_string()),
            software_revision: Some("A.7.0.6".to_string()),
        },
    });
    assert_eq!(window.battery_text(), "62 %");
    assert_eq!(
        window.hardware_text(HardwareField::Firmware).as_deref(),
        Some("2671")
    );
    assert_eq!(
        window.hardware_text(HardwareField::Serial).as_deref(),
        Some("250519")
    );

    window.set_status(DaemonStatus {
        state: "disconnected".to_string(),
        connected: false,
        streaming: false,
        last_error: None,
        device: DeviceInfo::default(),
        hardware: HardwareInfo {
            battery_percent: Some(61),
            ..HardwareInfo::default()
        },
    });
    assert_eq!(window.battery_text(), "61 % · last known");

    assert!(window.window().get_visible());
    assert!(
        window.diagram().fixed().width() > 0,
        "the hotspot overlay was allocated no space"
    );

    for button in RemoteButton::LOGICAL {
        let hotspot = window
            .diagram()
            .button(button)
            .unwrap_or_else(|| panic!("{button} has no hotspot"));
        assert_eq!(hotspot.widget_name(), format!("hotspot-{}", button.name()));
        assert!(
            hotspot.width() > 0 && hotspot.height() > 0,
            "{button} hotspot has no allocation"
        );
        let bounds = hotspot
            .compute_bounds(window.diagram().fixed())
            .expect("hotspot bound");
        assert!(bounds.width() > 0.0 && bounds.height() > 0.0);
    }
    let voice = window
        .diagram()
        .button(RemoteButton::Voice)
        .expect("voice hotspot");
    assert!(voice.width() > 0 && voice.height() > 0);
    let selected_before = window.selected();
    voice.emit_clicked();
    assert_eq!(
        window.selected(),
        selected_before,
        "the voice key must not become the selected mapping"
    );
    assert!(window.voice_selected());
    assert_eq!(window.voice_value_row().text(), "DICTATE");
    assert!(!window.apply_button().is_sensitive());

    window.voice_value_row().set_text("F23");
    window.voice_update_button().emit_clicked();
    assert_eq!(
        window.document().expect("document").trigger_key.as_deref(),
        Some("F23")
    );

    let model = GuiModel::new(window.document().expect("document")).expect("model");
    window
        .diagram()
        .refresh(&model, Some(RemoteButton::Power), false);
    assert!(
        window
            .diagram()
            .button(RemoteButton::Power)
            .unwrap()
            .has_css_class("pressed")
    );
    window.diagram().refresh(&model, None, false);
    assert!(
        !window
            .diagram()
            .button(RemoteButton::Power)
            .unwrap()
            .has_css_class("pressed")
    );

    window
        .diagram()
        .button(RemoteButton::Up)
        .unwrap()
        .emit_clicked();
    assert_eq!(window.selected(), Some(RemoteButton::Up));
    assert!(!window.voice_selected());
    assert_eq!(window.value_text(), "KEY_UP");
    assert!(
        window.apply_button().is_sensitive(),
        "the output-key edit must mark the document dirty"
    );

    window.trigger_row().set_selected(1);
    assert_eq!(window.value_text(), "");
    window.type_row().set_selected(1);
    window.value_row().set_text("KEY_F13");
    window.update_button().emit_clicked();
    let document = window.document().expect("document");
    assert_eq!(
        document.buttons["up"].double.as_deref(),
        Some("key:KEY_F13")
    );
    assert!(window.apply_button().is_sensitive());

    // A second button with the same action marks both hotspots as conflicts.
    window
        .diagram()
        .button(RemoteButton::Down)
        .unwrap()
        .emit_clicked();
    window.trigger_row().set_selected(1);
    window.type_row().set_selected(1);
    window.value_row().set_text("KEY_F13");
    window.update_button().emit_clicked();
    assert!(
        window
            .diagram()
            .button(RemoteButton::Up)
            .unwrap()
            .has_css_class("conflict")
    );
    assert!(
        window
            .diagram()
            .button(RemoteButton::Down)
            .unwrap()
            .has_css_class("conflict")
    );

    // Invalid input stays in the editor, reports the error, and keeps the old map.
    window
        .diagram()
        .button(RemoteButton::Up)
        .unwrap()
        .emit_clicked();
    window.trigger_row().set_selected(0);
    window.value_row().set_text("not-a-key");
    window.update_button().emit_clicked();
    assert!(window.action_error().get_visible());
    assert!(window.action_error().text().contains("not-a-key"));
    let document = window.document().expect("document");
    assert_eq!(document.buttons["up"].single.as_deref(), Some("key:KEY_UP"));
    assert!(window.apply_button().is_sensitive());

    window.window().close();
}
