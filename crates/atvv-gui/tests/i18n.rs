// SPDX-License-Identifier: GPL-3.0-or-later

//! The Simplified Chinese window: labels, status text, and page titles come
//! from the catalog while protocol values stay unchanged.
//!
//! GTK must be initialized and used from one thread, so the scenario runs in
//! one test. Run under `xvfb-run`.

use atvv_config::{Config, ConfigDocument};
use atvv_gui::app::{AppWindow, HardwareField};
use atvv_gui::client::{Backend, DaemonStatus, DeviceInfo, HardwareInfo};
use atvv_gui::i18n::{self, Language};
use atvv_gui::model::GuiModel;
use atvv_protocol::RC003;
use gtk::prelude::*;
use libadwaita::prelude::*;

fn combo_text(row: &libadwaita::ComboRow, index: u32) -> Option<String> {
    let item = row.model()?.item(index)?;
    item.downcast::<gtk::StringObject>()
        .ok()
        .map(|object| object.string().to_string())
}

#[test]
fn chinese_window() {
    if gtk::init().is_err() {
        eprintln!("skipping GUI i18n test: no display");
        return;
    }
    let _ = libadwaita::init();
    Language::set(Language::Chinese);

    let model = GuiModel::new(ConfigDocument::from_config(&Config::defaults(&RC003)))
        .expect("profile is known");
    let window = AppWindow::build(None, Backend::start(None));
    window.set_model(model);
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
    window.window().present();
    gtk::test_widget_wait_for_draw(window.window());

    assert_eq!(window.window().title().as_deref(), Some("ATVV 遥控器"));
    assert_eq!(window.battery_text(), "62 %");
    assert_eq!(
        window.hardware_text(HardwareField::Firmware).as_deref(),
        Some("2671")
    );
    assert_eq!(
        window.hardware_text(HardwareField::Serial).as_deref(),
        Some("250519")
    );
    assert_eq!(window.apply_button().label().as_deref(), Some("应用"));
    assert_eq!(window.test_button().label().as_deref(), Some("测试"));
    assert_eq!(window.update_button().label().as_deref(), Some("更新"));
    assert_eq!(
        window
            .diagram()
            .button(atvv_protocol::profile::RemoteButton::Power)
            .and_then(|button| button.tooltip_text())
            .as_deref(),
        Some("电源：点击编辑")
    );

    assert_eq!(combo_text(window.trigger_row(), 0).as_deref(), Some("单击"));
    assert_eq!(combo_text(window.trigger_row(), 1).as_deref(), Some("双击"));
    assert_eq!(combo_text(window.trigger_row(), 2).as_deref(), Some("长按"));
    assert_eq!(combo_text(window.type_row(), 0).as_deref(), Some("无操作"));
    assert_eq!(combo_text(window.type_row(), 4).as_deref(), Some("听写"));

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
    assert_eq!(window.battery_text(), "61 % · 上次值");

    window.window().close();
}

/// Run with a Chinese desktop locale to prove that GLib locale detection
/// reaches the catalog:
///
/// ```text
/// LANG=zh_CN.UTF-8 LANGUAGE=zh_CN xvfb-run -a \
///   cargo test -p atvv-gui --test i18n -- --ignored
/// ```
#[test]
#[ignore = "reads the process locale; run with LANG=zh_CN.UTF-8"]
fn locale_selects_chinese() {
    if gtk::init().is_err() {
        panic!("locale test needs a display; run it under xvfb-run");
    }
    i18n::init();
    assert_eq!(Language::current(), Language::Chinese);
}
