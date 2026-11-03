// SPDX-License-Identifier: GPL-3.0-or-later

//! The window opens tall enough for the board it shows.
//!
//! A constant default height scrolled on the profiles and locales that need
//! more room: the Chromecast has seven cards per column, and a Chinese card
//! title can wrap where English fits one line, so the board needed 854 px while
//! the window opened at 760 px. The window now grows to its measured height
//! after a document loads, and a size the user chose is left alone.
//!
//! GTK must be initialized and used from one thread, so the scenario runs in
//! one test.

use std::time::{Duration, Instant};

use atvv_config::{Config, ConfigDocument};
use atvv_gui::app::AppWindow;
use atvv_gui::client::Backend;
use atvv_gui::i18n::Language;
use atvv_gui::model::GuiModel;
use atvv_protocol::profile::DeviceProfile;
use atvv_protocol::{CHROMECAST, RC003};
use gtk::prelude::*;

/// Height the window opens at before a document is loaded.
const OPENING_HEIGHT: i32 = 760;

fn pump(millis: u64) {
    let deadline = Instant::now() + Duration::from_millis(millis);
    while Instant::now() < deadline {
        while gtk::glib::MainContext::default().iteration(false) {}
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn model_for(profile: &DeviceProfile) -> GuiModel {
    GuiModel::new(ConfigDocument::from_config(&Config::defaults(profile))).expect("profile known")
}

fn open(profile: &DeviceProfile) -> std::rc::Rc<AppWindow> {
    let window = AppWindow::build(None, Backend::detached());
    window.window().present();
    gtk::test_widget_wait_for_draw(window.window());
    pump(60);
    window.set_model(model_for(profile));
    pump(200);
    window
}

/// A document that arrives before the window is mapped — the first daemon
/// reply on a slow start — must fit too.
fn open_before_present(profile: &DeviceProfile) -> std::rc::Rc<AppWindow> {
    let window = AppWindow::build(None, Backend::detached());
    window.set_model(model_for(profile));
    pump(60);
    window.window().present();
    gtk::test_widget_wait_for_draw(window.window());
    pump(200);
    window
}

/// Vertical range the board still hides beyond its viewport.
fn vertical_overflow(window: &AppWindow) -> f64 {
    let adjustment = window.board_scroll().vadjustment();
    (adjustment.upper() - adjustment.page_size()).max(0.0)
}

#[test]
fn window_grows_to_the_board() {
    if gtk::init().is_err() {
        eprintln!("skipping window fit test: no display");
        return;
    }
    let _ = libadwaita::init();
    atvv_gui::app::install_css();

    let Some(monitor) = gtk::gdk::Display::default().and_then(|display| {
        display
            .monitors()
            .item(0)
            .and_downcast::<gtk::gdk::Monitor>()
    }) else {
        eprintln!("skipping window fit test: no monitor");
        return;
    };
    let screen_height = monitor.geometry().height();

    for language in [Language::English, Language::Chinese] {
        Language::set(language);
        for profile in [&RC003, &CHROMECAST] {
            for open_window in [
                open as fn(&DeviceProfile) -> std::rc::Rc<AppWindow>,
                open_before_present,
            ] {
                let window = open_window(profile);
                let height = window.window().height();
                let board = window.board().root();
                let needed = board.measure(gtk::Orientation::Vertical, board.width()).1
                    + (height - window.board_scroll().height());

                // A board taller than the screen cannot avoid scrolling; the
                // compositor clamps the window there.
                if needed > screen_height {
                    eprintln!(
                        "{language:?} {}: the board needs {needed} px on a \
                         {screen_height} px screen",
                        profile.id
                    );
                    window.window().close();
                    pump(30);
                    continue;
                }

                let overflow = vertical_overflow(&window);
                assert_eq!(
                    overflow, 0.0,
                    "{language:?} {}: the board still hides {overflow} px below \
                     its {height} px window, though {needed} px would show it",
                    profile.id
                );
                assert!(
                    height >= OPENING_HEIGHT,
                    "{language:?} {}: the window shrank to {height} px",
                    profile.id
                );

                // A window the user shrank keeps its size: the board scrolls
                // instead of the window growing back.
                window.window().set_default_size(1040, 600);
                pump(120);
                window.set_model(model_for(profile));
                pump(200);
                assert_eq!(
                    window.window().height(),
                    600,
                    "{language:?} {}: a refresh grew the window back past the \
                     user's resize",
                    profile.id
                );

                window.window().close();
                pump(30);
            }
        }
    }
}
