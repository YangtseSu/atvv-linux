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

/// Serialize tests that switch the process-wide language.
#[cfg(test)]
pub(crate) fn language_guard() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|error| error.into_inner())
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
        "Device information…" => "设备信息…",
        "Device information" => "设备信息",
        "Bluetooth and GATT facts" => "蓝牙与 GATT 信息",
        "Address" => "地址",
        "Name" => "名称",
        "Close" => "关闭",
        "No remote" => "无遥控器",
        "No remote connected" => "未连接遥控器",
        "Device: {name}" => "设备：{name}",
        "Passive facts reported by BlueZ and the ATVV GATT session." => {
            "由 BlueZ 和 ATVV GATT 会话上报的只读信息。"
        }
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
        "Hardware" => "硬件",
        "Firmware" => "固件",
        "Hardware revision" => "硬件版本",
        "Software revision" => "软件版本",
        "Serial number" => "序列号",
        "Manufacturer" => "制造商",
        "Model" => "型号",
        "{percent} % · last known" => "{percent} % · 上次值",

        // Action editor dialog.
        "Key" => "按键",
        "Type" => "类型",
        "Learn Key" => "学习按键",
        "Capture a keyboard key or chord" => "捕获键盘按键或组合键",
        "Command" => "命令",
        "Dictation" => "听写",
        "Pointer" => "指针",
        "Pointer action" => "指针操作",
        "Click" => "单击",
        "Move" => "移动",
        "Button" => "按键",
        "Direction" => "方向",
        "Units" => "单位",
        "Left click" => "左键单击",
        "Right click" => "右键单击",
        "Middle click" => "中键单击",
        "Side button" => "侧键",
        "Extra button" => "扩展键",
        "Forward button" => "前进键",
        "Back button" => "后退键",
        "Task button" => "任务键",
        "Pointer up" => "指针上移",
        "Pointer down" => "指针下移",
        "Pointer left" => "指针左移",
        "Pointer right" => "指针右移",
        "Emits a mouse click or moves the pointer. Movement is relative; the desktop may apply pointer acceleration." => {
            "发出鼠标单击或移动指针。移动是相对量，桌面可能应用指针加速。"
        }
        "Mapping" => "映射",
        "Custom mapping" => "自定义映射",
        "Profile default" => "配置文件默认值",
        "Unbound" => "未绑定",
        "Reset" => "重置",
        "Restore the profile default" => "恢复配置文件默认值",
        "Update" => "更新",
        "Chord" => "组合键",
        "Choose…" => "选择…",
        "Choose a key by name" => "按键名选择按键",
        "Choose key" => "选择按键",
        "Search keys" => "搜索按键",
        "The command runs as your user with a 5 s timeout and closed stdio." => {
            "命令以当前用户身份运行，超时 5 秒，标准输入输出已关闭。"
        }
        "Shell commands are disabled. Turn on “Allow shell commands” below first." => {
            "Shell 命令已禁用。请先开启“允许 shell 命令”。"
        }
        "Learn remote…" => "学习遥控器…",
        "Press a physical remote key to select its slot" => "按下遥控器实体按键以选择其槽位",
        "The button is swallowed; nothing is emitted." => "按键被吞掉，不发出任何操作。",
        "One key, emitted after the gesture." => "手势识别后发出单个按键。",
        "A chord, emitted after the gesture. Modifiers follow ctrl, alt, shift, super." => {
            "手势识别后发出按键组合。修饰键顺序为 ctrl、alt、shift、super。"
        }
        "Controls the ATVV microphone: start, stop, or toggle." => {
            "控制 ATVV 麦克风：开始、停止或切换录音。"
        }

        // Recording key.
        "Recording key" => "录音键",
        "Physical key" => "实体按键",
        "KEY_F5 is grabbed and fixed; it controls the ATVV microphone." => {
            "KEY_F5 被独占且固定，用于控制 ATVV 麦克风。"
        }
        "Output key" => "输出按键",
        "Capture a key or chord for the output" => "捕获用于输出的按键或组合键",
        "Restore DICTATE" => "恢复为 DICTATE",
        "Reset to DICTATE" => "重置为 DICTATE",
        "Output" => "输出",
        "Declared key…" => "声明键…",
        "Choose a declared recording key" => "选择契约声明的录音键",
        "Default" => "默认",
        "None (disable synthetic key)" => "无（禁用合成按键）",
        "Consumers must bind the same key. fcitx5-vinput uses Dictate, \
         VoiceCommand, Assistant." => {
            "消费者必须绑定相同的按键。fcitx5-vinput 使用 Dictate、\
             VoiceCommand、Assistant。"
        }
        "vinput is in D-Bus mode; the output key is not emitted." => {
            "vinput 处于 D-Bus 模式；不会发出输出按键。"
        }
        "vinput integration is off; the output key is not emitted." => {
            "vinput 集成已关闭；不会发出输出按键。"
        }

        // Test bar and advanced policy.
        "Test mode off" => "测试模式已关闭",
        "Test mode on" => "测试模式已开启",
        "Turn on Test to see what each key sends." => "开启“测试”以查看每个按键发送的内容。",
        "Press a remote key — shortcuts, volume, and dictation are suppressed." => {
            "按下遥控器按键——快捷键、音量和听写会被抑制。"
        }
        "Last resolved action" => "最近解析的操作",
        "{button} · {trigger}" => "{button} · {trigger}",
        "Run once" => "运行一次",
        "Run the displayed action once through atvvd" => "通过 atvvd 运行一次显示的操作",
        "Allow shell commands" => "允许 shell 命令",
        "Exec mappings run as your user with a 5 s timeout. Off by default." => {
            "Exec 映射以当前用户身份运行，超时 5 秒。默认关闭。"
        }

        // Pages and unavailable state.
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
        "Recording key selected" => "已选择录音键",
        "Microphone key (not remappable)" => "麦克风键（不可重映射）",
        "No action" => "无操作",
        "Waiting for a remote key…" => "等待遥控器按键…",
        "no configuration loaded" => "未加载配置",
        "Mapping updated" => "映射已更新",
        "Profile default restored" => "已恢复配置文件默认值",
        "Recording output key updated" => "录音输出按键已更新",
        "Recording output key reset" => "录音输出按键已重置",

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
        "None" => "无",
        "Command…" => "命令…",
        "Start dictation" => "开始听写",
        "Stop dictation" => "停止听写",
        "Toggle dictation" => "切换听写",
        "Return" => "回车",
        "Backspace" => "退格",
        "Delete" => "删除",
        "Escape" => "Esc",
        "Space" => "空格",
        "Tab" => "制表",
        "Play/Pause" => "播放/暂停",
        "Print Screen" => "打印屏幕",

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

        // Board and diagram text.
        "Remote diagram unavailable" => "遥控器示意图不可用",
        "Click a slot to edit it; click a key on the remote to edit its single action." => {
            "点击槽位进行编辑；点击遥控器上的按键编辑其单击操作。"
        }
        "Another key sends the same action in this gesture slot" => {
            "另一个按键在同一手势槽位发送相同的操作"
        }
        "Recording key settings" => "录音键设置",
        "Voice key" => "语音键",
        "Fixed KEY_F5 · ATVV microphone" => "固定 KEY_F5 · ATVV 麦克风",
        "{button} {trigger}: {action}" => "{button} {trigger}：{action}",
        "{button} · {trigger} · {raw}" => "{button} · {trigger} · {raw}",
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
        let _guard = language_guard();
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
