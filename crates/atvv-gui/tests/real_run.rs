// SPDX-License-Identifier: GPL-3.0-or-later

//! Real-device GUI run against a live `atvvd`.
//!
//! Ignored by default. Start the workspace daemon on the session bus with the
//! RC003 connected and point `XDG_CONFIG_HOME` at a scratch directory, then:
//!
//! ```text
//! xvfb-run -a cargo test -p atvv-gui --test real_run -- --ignored --nocapture
//! ```

use std::time::{Duration, Instant};

use atvv_gui::app::AppWindow;
use atvv_gui::client::Backend;
use atvv_protocol::profile::RemoteButton;
use gtk::prelude::*;
use libadwaita::prelude::*;

fn pump(window: &AppWindow) {
    while gtk::glib::MainContext::default().iteration(false) {}
    window.poll_events();
}

fn wait_until(window: &AppWindow, timeout: Duration, what: &str, check: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        pump(window);
        if check() {
            return true;
        }
        if Instant::now() >= deadline {
            eprintln!("ATVV_GUI_REAL: timed out waiting for {what}");
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
#[ignore = "requires a live atvvd with the RC003 on the session bus"]
fn gui_against_real_daemon() {
    if gtk::init().is_err() {
        panic!("real GUI test needs a display; run it under xvfb-run");
    }
    let _ = libadwaita::init();
    atvv_gui::app::install_css();
    atvv_gui::i18n::Language::set(atvv_gui::i18n::Language::English);

    let window = AppWindow::build(None, Backend::start(None));
    window.window().present();
    window.begin();
    assert!(
        wait_until(&window, Duration::from_secs(20), "GetConfig", || {
            window.document().is_some()
        }),
        "the daemon did not answer GetConfig"
    );

    window
        .diagram()
        .button(RemoteButton::Up)
        .expect("up hotspot")
        .emit_clicked();
    window.trigger_row().set_selected(1);
    window.type_row().set_selected(1);
    window.value_row().set_text("KEY_F13");
    window.update_button().emit_clicked();
    assert!(window.apply_button().is_sensitive());
    window.apply_button().emit_clicked();
    assert!(
        wait_until(&window, Duration::from_secs(10), "SetConfig", || {
            !window.apply_button().is_sensitive()
        }),
        "the daemon did not apply the document"
    );
    let document = window.document().expect("document after save");
    assert_eq!(
        document.buttons["up"].double.as_deref(),
        Some("key:KEY_F13")
    );

    let client = atvv_daemon::dbus::Client::connect().expect("daemon client");
    let live = client.config().expect("GetConfig from the daemon");
    assert_eq!(live.buttons["up"].double.as_deref(), Some("key:KEY_F13"));

    window.test_button().set_active(true);
    assert!(
        wait_until(&window, Duration::from_secs(5), "capture", || {
            window.test_result_text().contains("Waiting")
        }),
        "capture did not start"
    );
    eprintln!("ATVV_GUI_REAL: press voice, Home, Menu, OK, and volume now");
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut last_text = window.test_result_text();
    while Instant::now() < deadline {
        pump(&window);
        let text = window.test_result_text();
        if text != last_text {
            eprintln!("ATVV_GUI_REAL: event -> {text:?}");
            last_text = text;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    eprintln!(
        "ATVV_GUI_REAL: test_active={} result={:?}",
        window.test_button().is_active(),
        window.test_result_text()
    );
    window.test_button().set_active(false);
    window.window().close();
}
