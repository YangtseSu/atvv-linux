// SPDX-License-Identifier: GPL-3.0-or-later

//! Board proportions: the illustration never dictates the board height.
//!
//! The SVG's intrinsic 320x950 size used to reach layout through
//! `GtkPicture`, so the center column grew to 950 px of natural height, the
//! board scrolled at its own default window size, and the card columns were
//! centred inside the surplus. The diagram now takes the height the columns
//! leave.
//!
//! GTK must be initialized and used from one thread, so the scenario runs in
//! one test. Run under `xvfb-run`.

use std::time::{Duration, Instant};

use atvv_config::{Config, ConfigDocument};
use atvv_gui::app::AppWindow;
use atvv_gui::client::Backend;
use atvv_gui::model::GuiModel;
use atvv_protocol::{CHROMECAST, RC003, profile::DeviceProfile};
use gtk::prelude::*;

fn pump(millis: u64) {
    let deadline = Instant::now() + Duration::from_millis(millis);
    while Instant::now() < deadline {
        while gtk::glib::MainContext::default().iteration(false) {}
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn column_height(window: &AppWindow, left: bool) -> i32 {
    let button = window
        .board()
        .column_buttons(left)
        .first()
        .copied()
        .expect("column has a card");
    let card = window.board().card(button).expect("card");
    let column = card
        .root()
        .parent()
        .and_then(|parent| parent.downcast::<gtk::Box>().ok())
        .expect("column box");
    column.height()
}

fn check(profile: &DeviceProfile) {
    let model =
        GuiModel::new(ConfigDocument::from_config(&Config::defaults(profile))).expect("known");
    let window = AppWindow::build(None, Backend::detached());
    window.set_model(model);
    window.window().set_default_size(1040, 760);
    window.window().present();
    gtk::test_widget_wait_for_draw(window.window());
    pump(120);

    let columns = column_height(&window, true).max(column_height(&window, false));
    let diagram = window.diagram().root();
    assert!(
        diagram.height() <= columns,
        "{}: the diagram is {} px tall against {} px of card columns",
        profile.id,
        diagram.height(),
        columns
    );
    assert!(
        diagram.width() < columns,
        "{}: the diagram is {} px wide, wider than the columns are tall; it is \
         no longer a remote-shaped reference view",
        profile.id,
        diagram.width()
    );

    // Spare height belongs below the board, not in bands around the columns.
    let board = window.board().root();
    for left in [true, false] {
        let button = window.board().column_buttons(left)[0];
        let card = window.board().card(button).expect("card");
        let bounds = card.root().compute_bounds(board).expect("card bounds");
        assert_eq!(
            bounds.y().round() as i32,
            0,
            "{}: a card column starts {} px below the board",
            profile.id,
            bounds.y()
        );
    }

    window.window().close();
    pump(40);
}

#[test]
fn diagram_fits_the_card_columns() {
    if gtk::init().is_err() {
        eprintln!("skipping board proportion test: no display");
        return;
    }
    let _ = libadwaita::init();
    atvv_gui::app::install_css();
    atvv_gui::i18n::Language::set(atvv_gui::i18n::Language::English);

    for profile in [&RC003, &CHROMECAST] {
        check(profile);
    }
}
