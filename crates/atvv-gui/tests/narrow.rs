// SPDX-License-Identifier: GPL-3.0-or-later

//! Narrow-window smoke test: the board overflows horizontally, and selecting
//! a card scrolls it into view instead of leaving it off screen.
//!
//! GTK must be initialized and used from one thread, so the scenario runs in
//! one test and in its own process. Run under `xvfb-run`.

use std::time::{Duration, Instant};

use atvv_config::{Config, ConfigDocument};
use atvv_gui::app::AppWindow;
use atvv_gui::client::Backend;
use atvv_gui::model::GuiModel;
use atvv_input::Trigger;
use atvv_protocol::RC003;
use atvv_protocol::profile::RemoteButton;
use gtk::prelude::*;

fn pump(millis: u64) {
    let deadline = Instant::now() + Duration::from_millis(millis);
    while Instant::now() < deadline {
        while gtk::glib::MainContext::default().iteration(false) {}
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Whether `slot` lies inside the scrolled viewport.
fn visible(window: &AppWindow, slot: &gtk::Button) -> bool {
    let Some(bounds) = slot.compute_bounds(window.board_scroll()) else {
        return false;
    };
    let scroller = window.board_scroll();
    let width = scroller.width() as f32;
    let height = scroller.height() as f32;
    bounds.x() + bounds.width() > 0.0
        && bounds.y() + bounds.height() > 0.0
        && bounds.x() < width
        && bounds.y() < height
}

#[test]
fn narrow_window_scrolls_to_the_selected_slot() {
    if gtk::init().is_err() {
        eprintln!("skipping narrow-window test: no display");
        return;
    }
    let _ = libadwaita::init();
    atvv_gui::app::install_css();
    atvv_gui::i18n::Language::set(atvv_gui::i18n::Language::English);

    let model = GuiModel::new(ConfigDocument::from_config(&Config::defaults(&RC003)))
        .expect("profile is known");
    let window = AppWindow::build(None, Backend::start(None));
    window.set_model(model);
    window.window().set_default_size(700, 560);
    window.window().present();
    gtk::test_widget_wait_for_draw(window.window());
    pump(50);

    let horizontal = window.board_scroll().hadjustment();
    let vertical = window.board_scroll().vadjustment();
    assert!(
        horizontal.upper() > horizontal.page_size(),
        "the board must overflow horizontally at 700 px: upper {} page {}",
        horizontal.upper(),
        horizontal.page_size()
    );
    assert!(
        vertical.upper() > vertical.page_size(),
        "the board must overflow vertically at 560 px: upper {} page {}",
        vertical.upper(),
        vertical.page_size()
    );

    // The rightmost, bottom card must scroll into view.
    window
        .board()
        .slot(RemoteButton::Tv, Trigger::Single)
        .expect("tv single")
        .emit_clicked();
    window.editor().close();
    pump(80);
    assert!(
        horizontal.value() > 0.0,
        "selecting TV must scroll the board right: {}",
        horizontal.value()
    );
    assert!(
        visible(
            &window,
            window
                .board()
                .slot(RemoteButton::Tv, Trigger::Single)
                .unwrap()
        ),
        "the TV slot must be visible after selection"
    );

    // A left-column card scrolls back.
    window
        .board()
        .slot(RemoteButton::Power, Trigger::Single)
        .expect("power single")
        .emit_clicked();
    window.editor().close();
    pump(80);
    assert!(
        visible(
            &window,
            window
                .board()
                .slot(RemoteButton::Power, Trigger::Single)
                .unwrap()
        ),
        "the Power slot must be visible after selection"
    );

    window.window().close();
}
