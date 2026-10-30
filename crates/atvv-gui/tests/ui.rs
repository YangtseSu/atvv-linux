// SPDX-License-Identifier: GPL-3.0-or-later

//! Headless GTK smoke test: the window builds and maps, hotspots select keys,
//! and the action editor writes into the shared document.
//!
//! GTK must be initialized and used from one thread, so the scenarios run in
//! one test. Run under `xvfb-run`.

use atvv_config::{Config, ConfigDocument};
use atvv_gui::app::AppWindow;
use atvv_gui::client::Backend;
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

    let window = window();
    assert!(window.window().get_visible());

    for button in RemoteButton::LOGICAL {
        let hotspot = window
            .diagram()
            .button(button)
            .unwrap_or_else(|| panic!("{button} has no hotspot"));
        assert_eq!(hotspot.widget_name(), format!("hotspot-{}", button.name()));
    }
    assert!(window.diagram().button(RemoteButton::Voice).is_none());

    window
        .diagram()
        .button(RemoteButton::Up)
        .unwrap()
        .emit_clicked();
    assert_eq!(window.selected(), Some(RemoteButton::Up));
    assert_eq!(window.value_text(), "KEY_UP");
    assert!(!window.apply_button().is_sensitive());

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
