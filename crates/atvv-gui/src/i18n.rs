// SPDX-License-Identifier: GPL-3.0-or-later

//! GUI localization.
//!
//! English is the source language. [`tr`] takes the English string and returns
//! the translation for the active language, so a missing catalog entry falls
//! back to English instead of an empty label. The app resolves the language
//! once at startup from the desktop locale; tests may call [`Language::set`].
//!
//! Only interface text is translated. Protocol values (`single`, `none`), key
//! names (`KEY_F5`), addresses, and daemon diagnostics are data and stay
//! unchanged.

use std::sync::atomic::{AtomicU8, Ordering};

use gtk::glib;

/// Languages shipped by the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Language {
    /// Source language and fallback for every other locale.
    English = 0,
    /// Simplified Chinese.
    Chinese = 1,
}

static LANGUAGE: AtomicU8 = AtomicU8::new(Language::English as u8);

impl Language {
    /// Active language.
    #[must_use]
    pub fn current() -> Self {
        match LANGUAGE.load(Ordering::Relaxed) {
            value if value == Self::Chinese as u8 => Self::Chinese,
            _ => Self::English,
        }
    }

    /// Select the language for the rest of the process.
    pub fn set(language: Self) {
        LANGUAGE.store(language as u8, Ordering::Relaxed);
    }

    /// Resolve a language from GLib language names.
    ///
    /// The first `zh` locale wins. Only Simplified Chinese is shipped, so
    /// `zh_TW`/`zh-Hant` also select the Chinese catalog. Every other locale
    /// uses English.
    #[must_use]
    pub fn from_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Self {
        for name in names {
            let language = name.split(['.', '@', '_', '-']).next().unwrap_or_default();
            if language.eq_ignore_ascii_case("zh") {
                return Self::Chinese;
            }
        }
        Self::English
    }
}

/// Resolve the language from the desktop locale. Call once before building UI.
pub fn init() {
    Language::set(Language::from_names(
        glib::language_names().iter().map(|name| name.as_str()),
    ));
}

/// Translate an English source string.
#[must_use]
pub fn tr(text: &'static str) -> &'static str {
    match Language::current() {
        Language::English => text,
        Language::Chinese => zh(text).unwrap_or(text),
    }
}

/// Translate a template and replace `{name}` placeholders.
///
/// Templates keep the placeholders in both languages, so a translation may
/// reorder them without changing the call site.
#[must_use]
pub fn tr_args(template: &'static str, args: &[(&str, &str)]) -> String {
    let mut text = tr(template).to_string();
    for (name, value) in args {
        text = text.replace(&format!("{{{name}}}"), value);
    }
    text
}

/// Simplified Chinese catalog keyed by the English source string.
fn zh(text: &str) -> Option<&'static str> {
    Some(match text {
        // Window, header, and menu.
        "ATVV Remote" => "ATVV 遥控器",
        "Connecting…" => "正在连接…",
        "About ATVV" => "关于 ATVV",
        "Revert changes" => "还原更改",
        "Quit" => "退出",
        "Main menu" => "主菜单",
        "Test" => "测试",
        "Highlight remote presses and show the mapped action" => "高亮遥控器按键并显示映射的操作",
        "Revert" => "还原",
        "Apply" => "应用",
        "Daemon unavailable" => "守护进程不可用",
        "Retry" => "重试",
        "Unsaved changes" => "未保存的更改",
        "Press a remote key to test it" => "按下遥控器按键进行测试",
        "Press a remote key to select it" => "按下遥控器按键以选择",
        "Cancel" => "取消",

        // Sidebar and device status.
        "Battery" => "电池",
        "Unknown" => "未知",
        "Remote" => "遥控器",
        "Click a key to edit it. Dashed: no action. Amber: conflicting action. \
         The top-right voice key is fixed; its output key is configurable." => {
            "点击按键进行编辑。虚线：无操作。琥珀色：操作冲突。\
             右上角的语音键固定不变；其输出按键可配置。"
        }
        "Hardware" => "硬件",
        "Firmware" => "固件",
        "Hardware revision" => "硬件版本",
        "Software revision" => "软件版本",
        "Serial number" => "序列号",
        "Manufacturer" => "制造商",
        "Model" => "型号",
        "{percent} % · last known" => "{percent} % · 上次值",

        // Selected key and action editor.
        "Selected key" => "所选按键",
        "Key" => "按键",
        "Learn…" => "学习…",
        "Press a physical remote key to select it" => "按下遥控器上的实体按键以选择",
        "Gesture" => "手势",
        "Action" => "操作",
        "Type" => "类型",
        "Learn Key" => "学习按键",
        "Capture a keyboard key or chord" => "捕获键盘按键或组合键",
        "Command" => "命令",
        "Dictation" => "听写",
        "Mapping" => "映射",
        "Update this slot, then use Apply in the header to save." => {
            "更新此槽位，然后使用顶部的“应用”保存。"
        }
        "Reset" => "重置",
        "Restore the profile default" => "恢复配置文件默认值",
        "Update" => "更新",
        "Chord" => "组合键",
        "The command runs as your user with a 5 s timeout and closed stdio." => {
            "命令以当前用户身份运行，超时 5 秒，标准输入输出已关闭。"
        }
        "Shell commands are disabled. Turn on “Allow shell commands” below first." => {
            "Shell 命令已禁用。请先开启下方的“允许 shell 命令”。"
        }

        // Recording key.
        "Recording key" => "录音键",
        "Physical key" => "实体按键",
        "KEY_F5 is grabbed and fixed; it controls the ATVV microphone." => {
            "KEY_F5 被独占且固定，用于控制 ATVV 麦克风。"
        }
        "Output key" => "输出按键",
        "Capture a key or chord for the output" => "捕获用于输出的按键或组合键",
        "Emitted while the recording key is held; Apply saves." => "按住录音键时发出；应用后保存。",
        "Restore DICTATE" => "恢复为 DICTATE",

        // Test mode and advanced policy.
        "Test mode" => "测试模式",
        "Last test" => "上次测试",
        "Test mode is off" => "测试模式已关闭",
        "Run once" => "运行一次",
        "Run the displayed action once through atvvd" => "通过 atvvd 运行一次显示的操作",
        "{button} · {trigger} → {action}" => "{button} · {trigger} → {action}",
        "Advanced" => "高级",
        "Allow shell commands" => "允许 shell 命令",
        "Exec mappings run as your user with a 5 s timeout. Off by default." => {
            "Exec 映射以当前用户身份运行，超时 5 秒。默认关闭。"
        }

        // Pages and unavailable state.
        "Key mapping" => "按键映射",
        "Start the ATVV daemon with “systemctl --user start atvvd”, then retry." => {
            "使用“systemctl --user start atvvd”启动 ATVV 守护进程，然后重试。"
        }

        // Dialogs.
        "Allow shell commands?" => "允许 shell 命令？",
        "Exec mappings run shell commands as your user with a 5 s timeout. \
         Only enable this for commands you trust." => {
            "Exec 映射会以当前用户身份运行 shell 命令，超时 5 秒。\
             仅对信任的命令启用。"
        }
        "Enable" => "启用",
        "Learn key" => "学习按键",
        "Press a key or chord" => "按下按键或组合键",
        "Escape cancels. Held modifiers become part of the chord." => {
            "按 Escape 取消。按住的修饰键会成为组合键的一部分。"
        }
        "That key is not supported; try another." => "不支持该按键，请尝试其他按键。",
        "Run shell command?" => "运行 shell 命令？",
        "This runs “{command}” as your user with a 5 s timeout." => {
            "这将以当前用户身份运行“{command}”，超时 5 秒。"
        }
        "Run command" => "运行命令",

        // Toasts and error wrappers.
        "Cannot read the configuration: {error}" => "无法读取配置：{error}",
        "atvvd is not reachable: {error}" => "无法连接 atvvd：{error}",
        "Configuration applied" => "配置已应用",
        "Cannot apply the configuration: {error}" => "无法应用配置：{error}",
        "Cannot start capture mode: {error}" => "无法启动捕获模式：{error}",
        "Action sent" => "操作已发送",
        "Action failed: {error}" => "操作失败：{error}",
        "Selected {button} · {trigger}" => "已选择 {button} · {trigger}",
        "{button} · {trigger}{default_hint}" => "{button} · {trigger}{default_hint}",
        "Microphone key (not remappable)" => "麦克风键（不可重映射）",
        "No action" => "无操作",
        "Waiting for a remote key…" => "等待遥控器按键…",
        "no configuration loaded" => "未加载配置",
        "Mapping updated" => "映射已更新",
        "Profile default restored" => "已恢复配置文件默认值",
        "Recording output key updated" => "录音输出按键已更新",
        "Recording output key reset" => "录音输出按键已重置",
        " · profile default" => " · 配置文件默认值",
        "Voice key · output configurable" => "语音键 · 输出可配置",

        // Window subtitle.
        "{device} · recording" => "{device} · 录音中",
        "{device} · {state}" => "{device} · {state}",
        "Disconnected · {state}" => "未连接 · {state}",

        // Action grammar labels.
        "Do nothing" => "无操作",
        "Single key" => "单个按键",
        "Key combination" => "按键组合",
        "Run a command" => "运行命令",
        "Start recording" => "开始录音",
        "Stop recording" => "停止录音",
        "Toggle recording" => "切换录音",
        "Single" => "单击",
        "Double" => "双击",
        "Long" => "长按",

        // Remote buttons.
        "Power" => "电源",
        "OK" => "确定",
        "Back" => "返回",
        "Up" => "上",
        "Down" => "下",
        "Left" => "左",
        "Right" => "右",
        "Home" => "主页",
        "Menu" => "菜单",
        "TV" => "电视",
        "Volume up" => "音量加",
        "Volume down" => "音量减",
        "Voice" => "语音",

        // Diagram text.
        "Remote diagram unavailable" => "遥控器示意图不可用",
        "Voice key: drives the ATVV microphone and is not remappable" => {
            "语音键：控制 ATVV 麦克风，不可重映射"
        }
        "{label}: click to edit" => "{label}：点击编辑",
        "{label} key{suffix}" => "{label}键{suffix}",
        ", not remappable" => "，不可重映射",
        ", conflicting action" => "，操作冲突",

        // About dialog.
        "Configure ATVV remote button mappings and test them from the desktop." => {
            "在桌面上配置并测试 ATVV 遥控器按键映射。"
        }

        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_names_select_the_expected_catalog() {
        let cases = [
            (vec!["zh_CN.UTF-8", "zh_CN", "zh"], Language::Chinese),
            (vec!["zh-Hans", "en_US.UTF-8"], Language::Chinese),
            (vec!["zh_TW.UTF-8"], Language::Chinese),
            (vec!["en_US.UTF-8", "en"], Language::English),
            (vec!["de_DE.UTF-8", "de"], Language::English),
            (vec!["C", "POSIX"], Language::English),
            (Vec::new(), Language::English),
        ];
        for (names, expected) in cases {
            assert_eq!(Language::from_names(names), expected);
        }
    }

    #[test]
    fn catalog_translates_and_falls_back() {
        Language::set(Language::English);
        assert_eq!(tr("Apply"), "Apply");
        assert_eq!(zh("Apply"), Some("应用"));
        assert_eq!(zh("not a catalog entry"), None);

        Language::set(Language::Chinese);
        assert_eq!(tr("Apply"), "应用");
        assert_eq!(tr("not a catalog entry"), "not a catalog entry");
        assert_eq!(
            tr_args(
                "Cannot read the configuration: {error}",
                &[("error", "timed out")],
            ),
            "无法读取配置：timed out"
        );
        assert_eq!(
            tr_args("{device} · recording", &[("device", "RC003")],),
            "RC003 · 录音中"
        );
    }
}
