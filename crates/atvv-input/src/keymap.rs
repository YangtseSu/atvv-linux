// SPDX-License-Identifier: GPL-3.0-or-later

//! Key-name resolution and the profile key map.

use std::collections::BTreeMap;
use std::str::FromStr;

use atvv_protocol::profile::{DeviceProfile, RemoteButton};
use evdevil::event::Key;
use thiserror::Error;

use crate::action::{Action, ActionError, ActionMap};

/// One ordered key program: press modifiers first, release in reverse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyProgram {
    pub press: Vec<Key>,
    pub release: Vec<Key>,
}

impl KeyProgram {
    /// A single key held for as long as the caller keeps it pressed.
    #[must_use]
    pub fn single(code: Key) -> Self {
        Self {
            press: vec![code],
            release: vec![code],
        }
    }
}

/// Profile keys that cannot be resolved to one input code.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum KeyMapError {
    #[error("profile key {key:?} for {button} is not a known input name")]
    Unknown {
        button: RemoteButton,
        key: &'static str,
    },
    #[error("input key {key:?} is mapped to both {first} and {second}")]
    Duplicate {
        key: String,
        first: RemoteButton,
        second: RemoteButton,
    },
}

/// Physical key code to logical button, built from a device profile.
#[derive(Debug, Clone)]
pub struct KeyMap {
    codes: BTreeMap<Key, RemoteButton>,
}

impl KeyMap {
    /// Resolve every `button_keys` entry of a profile.
    pub fn from_profile(profile: &DeviceProfile) -> Result<Self, KeyMapError> {
        let mut codes = BTreeMap::new();
        for mapping in profile.button_keys {
            let code = resolve_key(mapping.key).ok_or(KeyMapError::Unknown {
                button: mapping.button,
                key: mapping.key,
            })?;
            if let Some(previous) = codes.insert(code, mapping.button)
                && previous != mapping.button
            {
                return Err(KeyMapError::Duplicate {
                    key: mapping.key.to_string(),
                    first: previous,
                    second: mapping.button,
                });
            }
        }
        Ok(Self { codes })
    }

    /// Logical button for a physical key, if the profile maps it.
    #[must_use]
    pub fn button(&self, code: Key) -> Option<RemoteButton> {
        self.codes.get(&code).copied()
    }

    /// Every mapped key code.
    pub fn codes(&self) -> impl Iterator<Item = Key> + '_ {
        self.codes.keys().copied()
    }
}

/// Key-name aliases. First match wins; names are normalized by
/// [`normalize`] (lower-case, `-`/`_`/spaces stripped).
const ALIASES: &[(&str, Key)] = &[
    ("logo", Key::KEY_LEFTMETA),
    ("leftmeta", Key::KEY_LEFTMETA),
    ("super", Key::KEY_LEFTMETA),
    ("meta", Key::KEY_LEFTMETA),
    ("win", Key::KEY_LEFTMETA),
    ("cmd", Key::KEY_LEFTMETA),
    ("rightmeta", Key::KEY_RIGHTMETA),
    ("alt", Key::KEY_LEFTALT),
    ("option", Key::KEY_LEFTALT),
    ("leftalt", Key::KEY_LEFTALT),
    ("altgr", Key::KEY_RIGHTALT),
    ("altr", Key::KEY_RIGHTALT),
    ("rightalt", Key::KEY_RIGHTALT),
    ("shift", Key::KEY_LEFTSHIFT),
    ("leftshift", Key::KEY_LEFTSHIFT),
    ("rightshift", Key::KEY_RIGHTSHIFT),
    ("ctrl", Key::KEY_LEFTCTRL),
    ("control", Key::KEY_LEFTCTRL),
    ("leftctrl", Key::KEY_LEFTCTRL),
    ("rightctrl", Key::KEY_RIGHTCTRL),
    ("return", Key::KEY_ENTER),
    ("enter", Key::KEY_ENTER),
    ("kpenter", Key::KEY_KPENTER),
    ("esc", Key::KEY_ESC),
    ("escape", Key::KEY_ESC),
    ("tab", Key::KEY_TAB),
    ("space", Key::KEY_SPACE),
    ("backspace", Key::KEY_BACKSPACE),
    ("bs", Key::KEY_BACKSPACE),
    ("delete", Key::KEY_DELETE),
    ("del", Key::KEY_DELETE),
    ("insert", Key::KEY_INSERT),
    ("ins", Key::KEY_INSERT),
    ("up", Key::KEY_UP),
    ("down", Key::KEY_DOWN),
    ("left", Key::KEY_LEFT),
    ("right", Key::KEY_RIGHT),
    ("home", Key::KEY_HOME),
    ("end", Key::KEY_END),
    ("pageup", Key::KEY_PAGEUP),
    ("pgup", Key::KEY_PAGEUP),
    ("pagedown", Key::KEY_PAGEDOWN),
    ("pgdn", Key::KEY_PAGEDOWN),
    ("menu", Key::KEY_MENU),
    ("contextmenu", Key::KEY_CONTEXT_MENU),
    ("menukey", Key::KEY_CONTEXT_MENU),
    ("grave", Key::KEY_GRAVE),
    ("backtick", Key::KEY_GRAVE),
    ("minus", Key::KEY_MINUS),
    ("dash", Key::KEY_MINUS),
    ("equal", Key::KEY_EQUAL),
    ("semicolon", Key::KEY_SEMICOLON),
    ("apostrophe", Key::KEY_APOSTROPHE),
    ("comma", Key::KEY_COMMA),
    ("dot", Key::KEY_DOT),
    ("period", Key::KEY_DOT),
    ("slash", Key::KEY_SLASH),
    ("backslash", Key::KEY_BACKSLASH),
    ("volumeup", Key::KEY_VOLUMEUP),
    ("volumedown", Key::KEY_VOLUMEDOWN),
    ("mute", Key::KEY_MUTE),
    ("micmute", Key::KEY_MICMUTE),
    ("playpause", Key::KEY_PLAYPAUSE),
    ("nextsong", Key::KEY_NEXTSONG),
    ("prevsong", Key::KEY_PREVIOUSSONG),
    ("power", Key::KEY_POWER),
    ("printscreen", Key::KEY_SYSRQ),
    ("prtsc", Key::KEY_SYSRQ),
    ("sysrq", Key::KEY_SYSRQ),
    ("homepage", Key::KEY_HOMEPAGE),
    ("brightnessup", Key::KEY_BRIGHTNESSUP),
    ("brightnessdown", Key::KEY_BRIGHTNESSDOWN),
];

/// Key names offered by pickers and key-name diagnostics.
///
/// Common bindable keys come first, then every `KEY_*` variant the input
/// backend resolves, sorted. Every entry resolves through [`resolve_key`]; a
/// test enforces it. Free-form input still accepts any kernel name.
pub const KEY_NAMES: &[&str] = &[
    "KEY_SYSRQ",
    "KEY_POWER",
    "KEY_MUTE",
    "KEY_VOLUMEUP",
    "KEY_VOLUMEDOWN",
    "KEY_MICMUTE",
    "KEY_PLAYPAUSE",
    "KEY_NEXTSONG",
    "KEY_PREVIOUSSONG",
    "KEY_BRIGHTNESSUP",
    "KEY_BRIGHTNESSDOWN",
    "KEY_HOMEPAGE",
    "KEY_COMPOSE",
    "KEY_F1",
    "KEY_F2",
    "KEY_F3",
    "KEY_F4",
    "KEY_F5",
    "KEY_F6",
    "KEY_F7",
    "KEY_F8",
    "KEY_F9",
    "KEY_F10",
    "KEY_F11",
    "KEY_F12",
    "KEY_F13",
    "KEY_F14",
    "KEY_F15",
    "KEY_F16",
    "KEY_F17",
    "KEY_F18",
    "KEY_F19",
    "KEY_F20",
    "KEY_F21",
    "KEY_F22",
    "KEY_F23",
    "KEY_F24",
    "KEY_ESC",
    "KEY_TAB",
    "KEY_SPACE",
    "KEY_ENTER",
    "KEY_BACKSPACE",
    "KEY_DELETE",
    "KEY_INSERT",
    "KEY_HOME",
    "KEY_END",
    "KEY_PAGEUP",
    "KEY_PAGEDOWN",
    "KEY_UP",
    "KEY_DOWN",
    "KEY_LEFT",
    "KEY_RIGHT",
    "KEY_GRAVE",
    "KEY_MINUS",
    "KEY_EQUAL",
    "KEY_LEFTBRACE",
    "KEY_RIGHTBRACE",
    "KEY_BACKSLASH",
    "KEY_SEMICOLON",
    "KEY_APOSTROPHE",
    "KEY_COMMA",
    "KEY_DOT",
    "KEY_SLASH",
    "KEY_A",
    "KEY_B",
    "KEY_C",
    "KEY_D",
    "KEY_E",
    "KEY_F",
    "KEY_G",
    "KEY_H",
    "KEY_I",
    "KEY_J",
    "KEY_K",
    "KEY_L",
    "KEY_M",
    "KEY_N",
    "KEY_O",
    "KEY_P",
    "KEY_Q",
    "KEY_R",
    "KEY_S",
    "KEY_T",
    "KEY_U",
    "KEY_V",
    "KEY_W",
    "KEY_X",
    "KEY_Y",
    "KEY_Z",
    "KEY_0",
    "KEY_1",
    "KEY_2",
    "KEY_3",
    "KEY_4",
    "KEY_5",
    "KEY_6",
    "KEY_7",
    "KEY_8",
    "KEY_9",
    "KEY_102ND",
    "KEY_10CHANNELSDOWN",
    "KEY_10CHANNELSUP",
    "KEY_3D_MODE",
    "KEY_AB",
    "KEY_ACCESSIBILITY",
    "KEY_ACTION_ON_SELECTION",
    "KEY_ADDRESSBOOK",
    "KEY_AGAIN",
    "KEY_ALL_APPLICATIONS",
    "KEY_ALS_TOGGLE",
    "KEY_ALTERASE",
    "KEY_ANGLE",
    "KEY_APPSELECT",
    "KEY_ARCHIVE",
    "KEY_ASPECT_RATIO",
    "KEY_ASSISTANT",
    "KEY_ATTENDANT_OFF",
    "KEY_ATTENDANT_ON",
    "KEY_ATTENDANT_TOGGLE",
    "KEY_AUDIO",
    "KEY_AUDIO_DESC",
    "KEY_AUTOPILOT_ENGAGE_TOGGLE",
    "KEY_AUX",
    "KEY_BACK",
    "KEY_BASSBOOST",
    "KEY_BATTERY",
    "KEY_BLUE",
    "KEY_BLUETOOTH",
    "KEY_BOOKMARKS",
    "KEY_BREAK",
    "KEY_BRIGHTNESS_AUTO",
    "KEY_BRIGHTNESS_CYCLE",
    "KEY_BRIGHTNESS_MAX",
    "KEY_BRIGHTNESS_MENU",
    "KEY_BRIGHTNESS_MIN",
    "KEY_BRIGHTNESS_TOGGLE",
    "KEY_BRIGHTNESS_ZERO",
    "KEY_BRL_DOT1",
    "KEY_BRL_DOT10",
    "KEY_BRL_DOT2",
    "KEY_BRL_DOT3",
    "KEY_BRL_DOT4",
    "KEY_BRL_DOT5",
    "KEY_BRL_DOT6",
    "KEY_BRL_DOT7",
    "KEY_BRL_DOT8",
    "KEY_BRL_DOT9",
    "KEY_BUTTONCONFIG",
    "KEY_CALC",
    "KEY_CALENDAR",
    "KEY_CAMERA",
    "KEY_CAMERA_ACCESS_DISABLE",
    "KEY_CAMERA_ACCESS_ENABLE",
    "KEY_CAMERA_ACCESS_TOGGLE",
    "KEY_CAMERA_DOWN",
    "KEY_CAMERA_FOCUS",
    "KEY_CAMERA_LEFT",
    "KEY_CAMERA_RIGHT",
    "KEY_CAMERA_UP",
    "KEY_CAMERA_ZOOMIN",
    "KEY_CAMERA_ZOOMOUT",
    "KEY_CANCEL",
    "KEY_CAPSLOCK",
    "KEY_CD",
    "KEY_CHANNEL",
    "KEY_CHANNELDOWN",
    "KEY_CHANNELUP",
    "KEY_CHAT",
    "KEY_CLEAR",
    "KEY_CLEARVU_SONAR",
    "KEY_CLOSE",
    "KEY_CLOSECD",
    "KEY_COFFEE",
    "KEY_COMPUTER",
    "KEY_CONFIG",
    "KEY_CONNECT",
    "KEY_CONTEXTUAL_INSERT",
    "KEY_CONTEXTUAL_QUERY",
    "KEY_CONTEXT_MENU",
    "KEY_CONTROLPANEL",
    "KEY_COPY",
    "KEY_CUT",
    "KEY_CYCLEWINDOWS",
    "KEY_DASHBOARD",
    "KEY_DATA",
    "KEY_DATABASE",
    "KEY_DELETEFILE",
    "KEY_DEL_EOL",
    "KEY_DEL_EOS",
    "KEY_DEL_LINE",
    "KEY_DICTATE",
    "KEY_DIGITS",
    "KEY_DIRECTION",
    "KEY_DIRECTORY",
    "KEY_DISPLAYTOGGLE",
    "KEY_DISPLAY_OFF",
    "KEY_DOCUMENTS",
    "KEY_DOLLAR",
    "KEY_DO_NOT_DISTURB",
    "KEY_DUAL_RANGE_RADAR",
    "KEY_DVD",
    "KEY_EDIT",
    "KEY_EDITOR",
    "KEY_EJECTCD",
    "KEY_EJECTCLOSECD",
    "KEY_EMAIL",
    "KEY_EMOJI_PICKER",
    "KEY_EPG",
    "KEY_EPRIVACY_SCREEN_OFF",
    "KEY_EPRIVACY_SCREEN_ON",
    "KEY_EURO",
    "KEY_EXIT",
    "KEY_FASTFORWARD",
    "KEY_FASTREVERSE",
    "KEY_FAVORITES",
    "KEY_FILE",
    "KEY_FINANCE",
    "KEY_FIND",
    "KEY_FIRST",
    "KEY_FISHING_CHART",
    "KEY_FN",
    "KEY_FN_1",
    "KEY_FN_2",
    "KEY_FN_B",
    "KEY_FN_D",
    "KEY_FN_E",
    "KEY_FN_ESC",
    "KEY_FN_F",
    "KEY_FN_F1",
    "KEY_FN_F10",
    "KEY_FN_F11",
    "KEY_FN_F12",
    "KEY_FN_F2",
    "KEY_FN_F3",
    "KEY_FN_F4",
    "KEY_FN_F5",
    "KEY_FN_F6",
    "KEY_FN_F7",
    "KEY_FN_F8",
    "KEY_FN_F9",
    "KEY_FN_RIGHT_SHIFT",
    "KEY_FN_S",
    "KEY_FORWARD",
    "KEY_FORWARDMAIL",
    "KEY_FRAMEBACK",
    "KEY_FRAMEFORWARD",
    "KEY_FRONT",
    "KEY_FULL_SCREEN",
    "KEY_GAMES",
    "KEY_GOTO",
    "KEY_GRAPHICSEDITOR",
    "KEY_GREEN",
    "KEY_HANGEUL",
    "KEY_HANGUEL",
    "KEY_HANGUP_PHONE",
    "KEY_HANJA",
    "KEY_HELP",
    "KEY_HENKAN",
    "KEY_HIRAGANA",
    "KEY_HP",
    "KEY_IMAGES",
    "KEY_INFO",
    "KEY_INS_LINE",
    "KEY_ISO",
    "KEY_JOURNAL",
    "KEY_KATAKANA",
    "KEY_KATAKANAHIRAGANA",
    "KEY_KBDILLUMDOWN",
    "KEY_KBDILLUMTOGGLE",
    "KEY_KBDILLUMUP",
    "KEY_KBDINPUTASSIST_ACCEPT",
    "KEY_KBDINPUTASSIST_CANCEL",
    "KEY_KBDINPUTASSIST_NEXT",
    "KEY_KBDINPUTASSIST_NEXTGROUP",
    "KEY_KBDINPUTASSIST_PREV",
    "KEY_KBDINPUTASSIST_PREVGROUP",
    "KEY_KBD_LAYOUT_NEXT",
    "KEY_KBD_LCD_MENU1",
    "KEY_KBD_LCD_MENU2",
    "KEY_KBD_LCD_MENU3",
    "KEY_KBD_LCD_MENU4",
    "KEY_KBD_LCD_MENU5",
    "KEY_KEYBOARD",
    "KEY_KP0",
    "KEY_KP1",
    "KEY_KP2",
    "KEY_KP3",
    "KEY_KP4",
    "KEY_KP5",
    "KEY_KP6",
    "KEY_KP7",
    "KEY_KP8",
    "KEY_KP9",
    "KEY_KPASTERISK",
    "KEY_KPCOMMA",
    "KEY_KPDOT",
    "KEY_KPENTER",
    "KEY_KPEQUAL",
    "KEY_KPJPCOMMA",
    "KEY_KPLEFTPAREN",
    "KEY_KPMINUS",
    "KEY_KPPLUS",
    "KEY_KPPLUSMINUS",
    "KEY_KPRIGHTPAREN",
    "KEY_KPSLASH",
    "KEY_LANGUAGE",
    "KEY_LAST",
    "KEY_LEFTALT",
    "KEY_LEFTCTRL",
    "KEY_LEFTMETA",
    "KEY_LEFTSHIFT",
    "KEY_LEFT_DOWN",
    "KEY_LEFT_UP",
    "KEY_LIGHTS_TOGGLE",
    "KEY_LINEFEED",
    "KEY_LINK_PHONE",
    "KEY_LIST",
    "KEY_LOGOFF",
    "KEY_MACRO",
    "KEY_MACRO1",
    "KEY_MACRO10",
    "KEY_MACRO11",
    "KEY_MACRO12",
    "KEY_MACRO13",
    "KEY_MACRO14",
    "KEY_MACRO15",
    "KEY_MACRO16",
    "KEY_MACRO17",
    "KEY_MACRO18",
    "KEY_MACRO19",
    "KEY_MACRO2",
    "KEY_MACRO20",
    "KEY_MACRO21",
    "KEY_MACRO22",
    "KEY_MACRO23",
    "KEY_MACRO24",
    "KEY_MACRO25",
    "KEY_MACRO26",
    "KEY_MACRO27",
    "KEY_MACRO28",
    "KEY_MACRO29",
    "KEY_MACRO3",
    "KEY_MACRO30",
    "KEY_MACRO4",
    "KEY_MACRO5",
    "KEY_MACRO6",
    "KEY_MACRO7",
    "KEY_MACRO8",
    "KEY_MACRO9",
    "KEY_MACRO_PRESET1",
    "KEY_MACRO_PRESET2",
    "KEY_MACRO_PRESET3",
    "KEY_MACRO_PRESET_CYCLE",
    "KEY_MACRO_RECORD_START",
    "KEY_MACRO_RECORD_STOP",
    "KEY_MAIL",
    "KEY_MARK_WAYPOINT",
    "KEY_MEDIA",
    "KEY_MEDIA_REPEAT",
    "KEY_MEDIA_TOP_MENU",
    "KEY_MEMO",
    "KEY_MENU",
    "KEY_MESSENGER",
    "KEY_MHP",
    "KEY_MODE",
    "KEY_MOVE",
    "KEY_MP3",
    "KEY_MSDOS",
    "KEY_MUHENKAN",
    "KEY_NAV_CHART",
    "KEY_NAV_INFO",
    "KEY_NEW",
    "KEY_NEWS",
    "KEY_NEXT",
    "KEY_NEXT_ELEMENT",
    "KEY_NEXT_FAVORITE",
    "KEY_NOTIFICATION_CENTER",
    "KEY_NUMERIC_0",
    "KEY_NUMERIC_1",
    "KEY_NUMERIC_11",
    "KEY_NUMERIC_12",
    "KEY_NUMERIC_2",
    "KEY_NUMERIC_3",
    "KEY_NUMERIC_4",
    "KEY_NUMERIC_5",
    "KEY_NUMERIC_6",
    "KEY_NUMERIC_7",
    "KEY_NUMERIC_8",
    "KEY_NUMERIC_9",
    "KEY_NUMERIC_A",
    "KEY_NUMERIC_B",
    "KEY_NUMERIC_C",
    "KEY_NUMERIC_D",
    "KEY_NUMERIC_POUND",
    "KEY_NUMERIC_STAR",
    "KEY_NUMLOCK",
    "KEY_OK",
    "KEY_ONSCREEN_KEYBOARD",
    "KEY_OPEN",
    "KEY_OPTION",
    "KEY_PASTE",
    "KEY_PAUSE",
    "KEY_PAUSECD",
    "KEY_PAUSE_RECORD",
    "KEY_PC",
    "KEY_PERFORMANCE",
    "KEY_PHONE",
    "KEY_PICKUP_PHONE",
    "KEY_PLAY",
    "KEY_PLAYCD",
    "KEY_PLAYER",
    "KEY_POWER2",
    "KEY_PRESENTATION",
    "KEY_PREVIOUS",
    "KEY_PREVIOUS_ELEMENT",
    "KEY_PRINT",
    "KEY_PRIVACY_SCREEN_TOGGLE",
    "KEY_PROG1",
    "KEY_PROG2",
    "KEY_PROG3",
    "KEY_PROG4",
    "KEY_PROGRAM",
    "KEY_PROPS",
    "KEY_PVR",
    "KEY_QUESTION",
    "KEY_RADAR_OVERLAY",
    "KEY_RADIO",
    "KEY_RECORD",
    "KEY_RED",
    "KEY_REDO",
    "KEY_REFRESH",
    "KEY_REFRESH_RATE_TOGGLE",
    "KEY_REPLY",
    "KEY_RESERVED",
    "KEY_RESTART",
    "KEY_REWIND",
    "KEY_RFKILL",
    "KEY_RIGHTALT",
    "KEY_RIGHTCTRL",
    "KEY_RIGHTMETA",
    "KEY_RIGHTSHIFT",
    "KEY_RIGHT_DOWN",
    "KEY_RIGHT_UP",
    "KEY_RO",
    "KEY_ROOT_MENU",
    "KEY_ROTATE_DISPLAY",
    "KEY_ROTATE_LOCK_TOGGLE",
    "KEY_SAT",
    "KEY_SAT2",
    "KEY_SAVE",
    "KEY_SCALE",
    "KEY_SCREEN",
    "KEY_SCREENLOCK",
    "KEY_SCREENSAVER",
    "KEY_SCROLLDOWN",
    "KEY_SCROLLLOCK",
    "KEY_SCROLLUP",
    "KEY_SEARCH",
    "KEY_SELECT",
    "KEY_SELECTIVE_SCREENSHOT",
    "KEY_SEND",
    "KEY_SENDFILE",
    "KEY_SETUP",
    "KEY_SHOP",
    "KEY_SHUFFLE",
    "KEY_SIDEVU_SONAR",
    "KEY_SINGLE_RANGE_RADAR",
    "KEY_SLEEP",
    "KEY_SLOW",
    "KEY_SLOWREVERSE",
    "KEY_SOS",
    "KEY_SOUND",
    "KEY_SPELLCHECK",
    "KEY_SPORT",
    "KEY_SPREADSHEET",
    "KEY_STOP",
    "KEY_STOPCD",
    "KEY_STOP_RECORD",
    "KEY_SUBTITLE",
    "KEY_SUSPEND",
    "KEY_SWITCHVIDEOMODE",
    "KEY_TAPE",
    "KEY_TASKMANAGER",
    "KEY_TEEN",
    "KEY_TEXT",
    "KEY_TIME",
    "KEY_TITLE",
    "KEY_TOUCHPAD_OFF",
    "KEY_TOUCHPAD_ON",
    "KEY_TOUCHPAD_TOGGLE",
    "KEY_TRADITIONAL_SONAR",
    "KEY_TUNER",
    "KEY_TV",
    "KEY_TV2",
    "KEY_TWEN",
    "KEY_UNDO",
    "KEY_UNKNOWN",
    "KEY_UNMUTE",
    "KEY_UWB",
    "KEY_VCR",
    "KEY_VCR2",
    "KEY_VENDOR",
    "KEY_VIDEO",
    "KEY_VIDEOPHONE",
    "KEY_VIDEO_NEXT",
    "KEY_VIDEO_PREV",
    "KEY_VOD",
    "KEY_VOICECOMMAND",
    "KEY_VOICEMAIL",
    "KEY_WAKEUP",
    "KEY_WIMAX",
    "KEY_WLAN",
    "KEY_WORDPROCESSOR",
    "KEY_WPS_BUTTON",
    "KEY_WWAN",
    "KEY_WWW",
    "KEY_XFER",
    "KEY_YELLOW",
    "KEY_YEN",
    "KEY_ZENKAKUHANKAKU",
    "KEY_ZOOM",
    "KEY_ZOOMIN",
    "KEY_ZOOMOUT",
    "KEY_ZOOMRESET",
];

fn normalize(name: &str) -> String {
    name.chars()
        .filter(|character| !matches!(character, '-' | '_' | ' '))
        .flat_map(char::to_lowercase)
        .collect()
}

/// Resolve a key name: verbatim kernel name, alias, or synthesized
/// `KEY_<NAME>` (`f5` → `KEY_F5`, `a` → `KEY_A`).
#[must_use]
pub fn resolve_key(name: &str) -> Option<Key> {
    let name = name.trim();
    if let Ok(code) = Key::from_str(name) {
        return Some(code);
    }
    let normalized = normalize(name);
    if let Some((_, code)) = ALIASES.iter().find(|(alias, _)| *alias == normalized) {
        return Some(*code);
    }
    let base = normalized
        .strip_prefix("key")
        .filter(|rest| !rest.is_empty())
        .unwrap_or(&normalized);
    Key::from_str(&format!("KEY_{}", base.to_uppercase())).ok()
}

/// Parse a synth expression: a single key (`DICTATE`, `F24`) or a chord
/// (`ctrl+alt+v`). Modifiers are pressed first and released in reverse.
///
/// Chords exist for compositors that refuse bare modifier or bare function
/// key bindings; the IME must be configured to the same chord.
#[must_use]
pub fn parse_key_program(input: &str) -> Option<KeyProgram> {
    let tokens: Vec<&str> = input.split('+').map(str::trim).collect();
    if tokens.is_empty() || tokens.iter().any(|token| token.is_empty()) {
        return None;
    }
    let (key, mods) = tokens.split_last()?;
    let action = if mods.is_empty() {
        Action::Key {
            name: (*key).to_string(),
        }
    } else {
        Action::Combo {
            mods: mods.iter().map(|token| (*token).to_string()).collect(),
            key: (*key).to_string(),
        }
    };
    KeyResolver.program(&action).ok()
}

/// Resolves actions into ordered key programs.
#[derive(Debug, Clone, Copy, Default)]
pub struct KeyResolver;

impl KeyResolver {
    /// Compile an action into a press/release program.
    pub fn program(&self, action: &Action) -> Result<KeyProgram, ActionError> {
        match action {
            Action::Key { name } => {
                let code =
                    resolve_key(name).ok_or_else(|| ActionError::UnknownKey(name.clone()))?;
                Ok(KeyProgram {
                    press: vec![code],
                    release: vec![code],
                })
            }
            Action::Combo { mods, key } => {
                let mut press = Vec::with_capacity(mods.len() + 1);
                for modifier in mods {
                    let code = resolve_key(modifier)
                        .ok_or_else(|| ActionError::UnknownKey(modifier.clone()))?;
                    if !press.contains(&code) {
                        press.push(code);
                    }
                }
                let code = resolve_key(key).ok_or_else(|| ActionError::UnknownKey(key.clone()))?;
                press.push(code);
                let release = press.iter().rev().copied().collect();
                Ok(KeyProgram { press, release })
            }
            Action::None | Action::Exec { .. } | Action::Dictation(_) | Action::Pointer(_) => {
                Err(ActionError::NotEmittable)
            }
        }
    }

    /// Every physical code the action map can emit, for uinput registration.
    pub fn codes_for(&self, map: &ActionMap) -> Result<Vec<Key>, ActionError> {
        let mut codes = Vec::new();
        for action in map.actions() {
            match self.program(action) {
                Ok(program) => codes.extend(program.press),
                Err(ActionError::NotEmittable) => {}
                Err(error) => return Err(error),
            }
        }
        codes.sort_unstable();
        codes.dedup();
        Ok(codes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{PointerAction, PointerButton};
    use atvv_protocol::RC003;

    #[test]
    fn profile_key_map_covers_the_remote() {
        let map = KeyMap::from_profile(&RC003).unwrap();
        assert_eq!(map.button(Key::KEY_F5), Some(RemoteButton::Voice));
        assert_eq!(map.button(Key::KEY_GRAVE), Some(RemoteButton::Tv));
        assert_eq!(map.button(Key::KEY_HOME), Some(RemoteButton::Home));
        assert_eq!(map.button(Key::KEY_COMPOSE), Some(RemoteButton::Menu));
        assert_eq!(map.button(Key::KEY_ENTER), Some(RemoteButton::Ok));
        assert_eq!(map.button(Key::KEY_KPENTER), Some(RemoteButton::Ok));
        assert_eq!(map.button(Key::KEY_BACK), Some(RemoteButton::Back));
        assert_eq!(map.button(Key::KEY_ESC), Some(RemoteButton::Back));
        assert_eq!(
            map.button(Key::KEY_VOLUMEDOWN),
            Some(RemoteButton::VolumeDown)
        );
        assert_eq!(map.button(Key::KEY_A), None);
        // Every physical button is reachable from the profile.
        for &button in RC003.buttons {
            assert!(
                map.codes().any(|code| map.button(code) == Some(button)),
                "{button}"
            );
        }
    }

    #[test]
    fn chromecast_key_map_covers_app_keys() {
        let map = KeyMap::from_profile(&atvv_protocol::CHROMECAST).unwrap();
        for (key, button) in [
            (Key::KEY_COFFEE, RemoteButton::Power),
            (Key::KEY_SELECT, RemoteButton::Ok),
            (Key::KEY_HOMEPAGE, RemoteButton::Home),
            (Key::KEY_TV, RemoteButton::Tv),
            (Key::KEY_MUTE, RemoteButton::Mute),
            (Key::KEY_CAMERA_ACCESS_DISABLE, RemoteButton::YouTube),
            (Key::KEY_CAMERA_ACCESS_TOGGLE, RemoteButton::Netflix),
        ] {
            assert_eq!(map.button(key), Some(button), "{key:?}");
        }
        assert_eq!(map.button(Key::KEY_PLAYPAUSE), None);
        for &button in atvv_protocol::CHROMECAST.buttons {
            assert!(
                map.codes().any(|code| map.button(code) == Some(button)),
                "{button}"
            );
        }
    }

    #[test]
    fn resolves_kernel_names_aliases_and_synthesis() {
        assert_eq!(resolve_key("KEY_F5"), Some(Key::KEY_F5));
        assert_eq!(resolve_key("f5"), Some(Key::KEY_F5));
        assert_eq!(resolve_key("key_f5"), Some(Key::KEY_F5));
        assert_eq!(resolve_key("F5"), Some(Key::KEY_F5));
        assert_eq!(resolve_key("F24"), Some(Key::KEY_F24));
        assert_eq!(resolve_key("ctrl"), Some(Key::KEY_LEFTCTRL));
        assert_eq!(resolve_key("super"), Some(Key::KEY_LEFTMETA));
        assert_eq!(resolve_key("alt_r"), Some(Key::KEY_RIGHTALT));
        assert_eq!(resolve_key("Alt_R"), Some(Key::KEY_RIGHTALT));
        assert_eq!(resolve_key("a"), Some(Key::KEY_A));
        assert_eq!(resolve_key("7"), Some(Key::KEY_7));
        assert_eq!(resolve_key("context-menu"), Some(Key::KEY_CONTEXT_MENU));
        assert_eq!(resolve_key("no-such-key"), None);
    }

    #[test]
    fn print_screen_names_resolve_to_sysrq() {
        assert_eq!(resolve_key("KEY_SYSRQ"), Some(Key::KEY_SYSRQ));
        assert_eq!(resolve_key("sysrq"), Some(Key::KEY_SYSRQ));
        assert_eq!(resolve_key("printscreen"), Some(Key::KEY_SYSRQ));
        assert_eq!(resolve_key("PrintScreen"), Some(Key::KEY_SYSRQ));
        assert_eq!(resolve_key("prtsc"), Some(Key::KEY_SYSRQ));
        // `print` stays the legacy kernel key, not the screenshot key.
        assert_eq!(resolve_key("print"), Some(Key::KEY_PRINT));
        assert_ne!(Key::KEY_PRINT, Key::KEY_SYSRQ);
    }

    #[test]
    fn key_names_cover_every_kernel_key() {
        let mut seen = std::collections::BTreeSet::new();
        for name in KEY_NAMES {
            assert!(name.starts_with("KEY_"), "{name}");
            assert!(resolve_key(name).is_some(), "{name}");
            assert!(seen.insert(*name), "duplicate {name}");
        }
        // Keys that the first curated list missed.
        for name in [
            "KEY_SYSRQ",
            "KEY_PRINT",
            "KEY_SCROLLLOCK",
            "KEY_PAUSE",
            "KEY_LEFTMETA",
            "KEY_KP0",
        ] {
            assert!(seen.contains(name), "{name} is missing");
        }
        assert!(KEY_NAMES.len() > 300, "only {} names", KEY_NAMES.len());
    }

    #[test]
    fn consumer_keys_resolve_to_the_kernel_codes() {
        assert_eq!(resolve_key("VOICECOMMAND"), Some(Key::KEY_VOICECOMMAND));
        assert_eq!(resolve_key("voicecommand"), Some(Key::KEY_VOICECOMMAND));
        assert_eq!(resolve_key("KEY_VOICECOMMAND"), Some(Key::KEY_VOICECOMMAND));
        assert_eq!(resolve_key("ASSISTANT"), Some(Key::KEY_ASSISTANT));
        assert_eq!(resolve_key("DICTATE"), Some(Key::KEY_DICTATE));
        assert_eq!(resolve_key("KEY_DICTATE"), Some(Key::KEY_DICTATE));
        assert_eq!(Key::KEY_VOICECOMMAND.raw(), 0x246);
        assert_eq!(Key::KEY_ASSISTANT.raw(), 0x247);
        assert_eq!(Key::KEY_DICTATE.raw(), 0x24a);

        let program = parse_key_program("DICTATE").unwrap();
        assert_eq!(program.press, vec![Key::KEY_DICTATE]);
        assert_eq!(program.release, vec![Key::KEY_DICTATE]);
    }

    #[test]
    fn single_key_program_presses_and_releases_the_same_key() {
        let program = KeyProgram::single(Key::KEY_RIGHTALT);
        assert_eq!(program.press, vec![Key::KEY_RIGHTALT]);
        assert_eq!(program.release, vec![Key::KEY_RIGHTALT]);
    }

    #[test]
    fn parses_single_keys_and_chords() {
        let single = parse_key_program("F24").unwrap();
        assert_eq!(single.press, vec![Key::KEY_F24]);
        assert_eq!(single.release, vec![Key::KEY_F24]);

        let chord = parse_key_program("ctrl+alt+v").unwrap();
        assert_eq!(
            chord.press,
            vec![Key::KEY_LEFTCTRL, Key::KEY_LEFTALT, Key::KEY_V]
        );
        assert_eq!(
            chord.release,
            vec![Key::KEY_V, Key::KEY_LEFTALT, Key::KEY_LEFTCTRL]
        );

        assert!(parse_key_program("ctrl+").is_none());
        assert!(parse_key_program("ctrl+nope").is_none());
    }

    #[test]
    fn compiles_taps_and_combos() {
        let resolver = KeyResolver;
        let tap = resolver
            .program(&Action::Key {
                name: "KEY_ENTER".into(),
            })
            .unwrap();
        assert_eq!(tap.press, vec![Key::KEY_ENTER]);
        assert_eq!(tap.release, vec![Key::KEY_ENTER]);

        let combo = resolver
            .program(&Action::Combo {
                mods: vec!["ctrl".into(), "alt".into()],
                key: "delete".into(),
            })
            .unwrap();
        assert_eq!(
            combo.press,
            vec![Key::KEY_LEFTCTRL, Key::KEY_LEFTALT, Key::KEY_DELETE]
        );
        assert_eq!(
            combo.release,
            vec![Key::KEY_DELETE, Key::KEY_LEFTALT, Key::KEY_LEFTCTRL]
        );
    }

    #[test]
    fn rejects_non_key_actions_and_unknown_names() {
        let resolver = KeyResolver;
        assert_eq!(
            resolver.program(&Action::None),
            Err(ActionError::NotEmittable)
        );
        assert_eq!(
            resolver.program(&Action::Pointer(PointerAction::Click(PointerButton::Left))),
            Err(ActionError::NotEmittable)
        );
        assert_eq!(
            resolver.program(&Action::Key {
                name: "nope".into()
            }),
            Err(ActionError::UnknownKey("nope".into()))
        );
    }

    #[test]
    fn default_map_codes_resolve() {
        let resolver = KeyResolver;
        let codes = resolver
            .codes_for(&ActionMap::defaults_for(&RC003))
            .unwrap();
        assert!(codes.contains(&Key::KEY_ENTER));
        assert!(codes.contains(&Key::KEY_GRAVE));
        assert!(codes.contains(&Key::KEY_HOME));
        assert!(codes.contains(&Key::KEY_COMPOSE));
        assert!(!codes.contains(&Key::KEY_POWER));

        let google = resolver
            .codes_for(&ActionMap::defaults_for(&atvv_protocol::CHROMECAST))
            .unwrap();
        assert!(google.contains(&Key::KEY_SELECT));
        assert!(google.contains(&Key::KEY_TV));
        assert!(google.contains(&Key::KEY_MUTE));
        assert!(google.contains(&Key::KEY_CAMERA_ACCESS_DISABLE));
        assert!(google.contains(&Key::KEY_CAMERA_ACCESS_TOGGLE));
        assert!(!google.contains(&Key::KEY_ENTER));
    }
}
