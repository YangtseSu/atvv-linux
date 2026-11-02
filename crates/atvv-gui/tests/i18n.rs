// SPDX-License-Identifier: GPL-3.0-or-later

//! The Simplified Chinese window: board labels, dialog controls, status text,
//! and test-bar copy come from the catalog while protocol values stay
//! unchanged.
//!
//! GTK must be initialized and used from one thread, so the scenario runs in
//! one test. Run under `xvfb-run`.

use atvv_config::{Config, ConfigDocument};
use atvv_gui::app::{AppWindow, HardwareField};
use atvv_gui::client::{
    AudioInfo, Backend, ContractInfo, DaemonStatus, DeviceInfo, HardwareInfo, InputInfo, VinputInfo,
};
use atvv_gui::i18n::{self, Language};
use atvv_gui::model::{GuiModel, action_label, trigger_label};
use atvv_input::Trigger;
use atvv_protocol::RC003;
use atvv_protocol::profile::RemoteButton;
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
    let window = AppWindow::build(None, Backend::detached());
    window.set_model(model);
    window.set_status(DaemonStatus {
        state: "ready".to_string(),
        connected: true,
        streaming: false,
        last_error: None,
        device: DeviceInfo {
            name: "小米蓝牙语音遥控器".to_string(),
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
        audio: AudioInfo {
            available: true,
            ..AudioInfo::default()
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
            recipe: "Select the source.".to_string(),
        },
    });
    window.window().present();
    gtk::test_widget_wait_for_draw(window.window());

    assert_eq!(window.window().title().as_deref(), Some("ATVV 遥控器"));
    assert_eq!(window.chip_device_text(), "小米蓝牙语音遥控器");
    assert_eq!(window.subtitle_text(), "RC003 · ready");
    assert_eq!(window.device_name_text(), "小米蓝牙语音遥控器");
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
    assert_eq!(
        window
            .diagram()
            .button(RemoteButton::Power)
            .and_then(|button| button.tooltip_text())
            .as_deref(),
        Some("电源：点击编辑")
    );

    // Board labels: gesture names and the compact action form.
    assert_eq!(trigger_label(Trigger::Single), "单击");
    assert_eq!(trigger_label(Trigger::Double), "双击");
    assert_eq!(trigger_label(Trigger::Long), "长按");
    assert_eq!(
        window.board().slot_text(RemoteButton::Ok, Trigger::Single),
        Some("回车".to_string())
    );
    assert_eq!(
        window
            .board()
            .slot_text(RemoteButton::Power, Trigger::Single),
        Some("无".to_string())
    );
    assert_eq!(action_label(Some("pointer:click:left")).text, "左键单击");
    assert_eq!(action_label(Some("pointer:move:up:3")).text, "指针上移 3");

    // The action dialog keeps the translated controls.
    assert_eq!(
        combo_text(window.editor().type_row(), 0).as_deref(),
        Some("无操作")
    );
    assert_eq!(
        combo_text(window.editor().type_row(), 4).as_deref(),
        Some("听写")
    );
    assert_eq!(
        combo_text(window.editor().type_row(), 5).as_deref(),
        Some("指针")
    );
    assert_eq!(
        window.editor().update_button().label().as_deref(),
        Some("更新")
    );
    assert!(window.voice_hint_text().contains("消费者必须绑定"));

    window
        .diagram()
        .button(RemoteButton::Up)
        .expect("up hotspot")
        .emit_clicked();
    assert_eq!(window.editor().title(), "上 · 单击");
    assert_eq!(window.editor().value_row().text().as_str(), "KEY_UP");
    assert_eq!(
        window.editor().choose_key_button().label().as_deref(),
        Some("选择…")
    );
    window.editor().choose_key_button().emit_clicked();
    assert!(window.editor().picker().is_open());
    assert_eq!(
        window.editor().picker().dialog().title().as_str(),
        "选择按键"
    );
    assert_eq!(
        window
            .editor()
            .picker()
            .search()
            .placeholder_text()
            .as_deref(),
        Some("搜索按键")
    );
    // The filter matches English labels and canonical names as well as the
    // localized label, regardless of the UI language.
    for query in ["print", "printscreen", "打印", "sysrq", "key_sysrq"] {
        window.editor().picker().search().set_text(query);
        let visible = window.editor().picker().visible_names();
        assert!(
            visible.contains(&"KEY_SYSRQ"),
            "{query:?} did not find Print Screen: {visible:?}"
        );
        assert!(
            !visible.contains(&"KEY_A"),
            "{query:?} did not filter unrelated keys: {visible:?}"
        );
    }
    assert!(window.editor().picker().select("KEY_SYSRQ"));
    assert_eq!(window.editor().value_row().text().as_str(), "KEY_SYSRQ");
    window.editor().cancel_button().emit_clicked();

    window
        .diagram()
        .button(RemoteButton::Voice)
        .expect("voice hotspot")
        .emit_clicked();
    assert_eq!(window.voice_value_row().text(), "DICTATE");
    assert_eq!(
        window.voice_declared_button().label().as_deref(),
        Some("声明键…")
    );
    assert!(window.voice_hint_text().contains("消费者必须绑定"));
    window.voice_editor().close();

    window.open_microphone();
    assert!(window.microphone().is_open());
    assert_eq!(
        window.microphone().hint_text(),
        "在主窗口中点击“应用”后保存；默认布局中暂存的槽位编辑会另存为新布局。"
    );
    assert_eq!(window.microphone().gain_text(), "0 dB");
    window.microphone().close();

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
        ..DaemonStatus::default()
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
