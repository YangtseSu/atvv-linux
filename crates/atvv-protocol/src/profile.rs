// SPDX-License-Identifier: GPL-3.0-or-later

//! Device profiles: the facts that differ between ATVV remotes.

use std::fmt;

use crate::caps::{Codec, ProtocolVersion};
use crate::commands;
use crate::events::StreamId;

/// A logical button on an ATVV remote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RemoteButton {
    Power,
    Ok,
    Back,
    Up,
    Down,
    Left,
    Right,
    Home,
    Menu,
    Tv,
    VolumeUp,
    VolumeDown,
    /// The voice key drives ATVV; it is grabbed but not action-mapped.
    Voice,
}

impl RemoteButton {
    /// Every button a profile can describe.
    pub const ALL: [Self; 13] = [
        Self::Power,
        Self::Ok,
        Self::Back,
        Self::Up,
        Self::Down,
        Self::Left,
        Self::Right,
        Self::Home,
        Self::Menu,
        Self::Tv,
        Self::VolumeUp,
        Self::VolumeDown,
        Self::Voice,
    ];

    /// The twelve action-mapped buttons; the voice key is excluded.
    pub const LOGICAL: [Self; 12] = [
        Self::Power,
        Self::Ok,
        Self::Back,
        Self::Up,
        Self::Down,
        Self::Left,
        Self::Right,
        Self::Home,
        Self::Menu,
        Self::Tv,
        Self::VolumeUp,
        Self::VolumeDown,
    ];

    /// Stable configuration name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Power => "power",
            Self::Ok => "ok",
            Self::Back => "back",
            Self::Up => "up",
            Self::Down => "down",
            Self::Left => "left",
            Self::Right => "right",
            Self::Home => "home",
            Self::Menu => "menu",
            Self::Tv => "tv",
            Self::VolumeUp => "volume_up",
            Self::VolumeDown => "volume_down",
            Self::Voice => "voice",
        }
    }
}

impl fmt::Display for RemoteButton {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// One physical key code that produces a logical button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonKey {
    pub button: RemoteButton,
    /// Kernel input key name, e.g. `KEY_HOMEPAGE`.
    pub key: &'static str,
}

/// Gesture timing for one device family, in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GestureTiming {
    pub double_click_ms: u16,
    pub long_press_ms: u16,
    pub repeat_start_ms: u16,
    /// RC003: spurious release/press cycles while held arrive ~449 ms
    /// apart; a release is only accepted after this window.
    pub stable_release_ms: u16,
    /// Phantom presses after a resolved click are dropped for this long.
    pub suppression_ms: u16,
    pub repeat_interval_ms: u16,
    pub back_repeat_interval_ms: u16,
}

/// ADPCM nibble order used by the remote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NibbleOrder {
    HighFirst,
    LowFirst,
}

/// `MIC_OPEN` payload shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicOpenVariant {
    /// `[0C 00]`, used by the RC003.
    Short,
    /// `[0C 00 codec_hi codec_lo]`, used by older ATVV remotes.
    Codec,
}

/// `MIC_CLOSE` payload shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicCloseVariant {
    /// `[0D]`.
    Bare,
    /// `[0D stream_id]`.
    StreamId,
}

/// Keepalive payload shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeepaliveVariant {
    /// `[0E]`, used by the RC003.
    Bare,
    /// `[0E stream_id]`.
    StreamId,
}

/// One ATVV device profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceProfile {
    pub id: &'static str,
    pub display_name: &'static str,
    pub bluetooth_names: &'static [&'static str],
    pub vendor_id: u16,
    pub product_id: u16,
    pub model: &'static str,
    pub protocol: ProtocolVersion,
    pub codecs: &'static [Codec],
    pub frame_size: u16,
    pub nibble_order: NibbleOrder,
    pub mic_open: MicOpenVariant,
    pub mic_close: MicCloseVariant,
    pub keepalive: KeepaliveVariant,
    pub direct_audio_start: bool,
    pub stop_echo: bool,
    pub post_stop_discard_ms: u16,
    pub sample_rate: u32,
    pub gain_db: i8,
    /// Physical key names per logical button; aliases repeat a button.
    pub button_keys: &'static [ButtonKey],
    pub gesture_timing: GestureTiming,
}

impl DeviceProfile {
    /// Key names that produce `button`, in profile order.
    pub fn key_names(&self, button: RemoteButton) -> impl Iterator<Item = &'static str> + '_ {
        self.button_keys
            .iter()
            .filter(move |mapping| mapping.button == button)
            .map(|mapping| mapping.key)
    }

    /// First key name registered for `button`.
    #[must_use]
    pub fn primary_key(&self, button: RemoteButton) -> Option<&'static str> {
        self.key_names(button).next()
    }

    #[must_use]
    pub fn supports(&self, codec: Codec) -> bool {
        self.codecs.contains(&codec)
    }

    #[must_use]
    pub fn preferred_codec(&self) -> Codec {
        self.codecs.first().copied().unwrap_or(Codec::Adpcm16k)
    }

    #[must_use]
    pub fn mic_open(&self) -> Vec<u8> {
        match self.mic_open {
            MicOpenVariant::Short => commands::mic_open_short(),
            MicOpenVariant::Codec => commands::mic_open_codec(self.preferred_codec()),
        }
    }

    #[must_use]
    pub fn mic_close(&self, stream: StreamId) -> Vec<u8> {
        match self.mic_close {
            MicCloseVariant::Bare => commands::mic_close(),
            MicCloseVariant::StreamId => commands::mic_close_stream(stream),
        }
    }

    #[must_use]
    pub fn mic_extend(&self, stream: StreamId) -> Vec<u8> {
        match self.keepalive {
            KeepaliveVariant::Bare => commands::mic_extend(),
            KeepaliveVariant::StreamId => commands::mic_extend_stream(stream),
        }
    }

    #[must_use]
    pub fn matches_name(&self, name: &str) -> bool {
        self.bluetooth_names
            .iter()
            .any(|candidate| name == *candidate || name.starts_with(candidate))
    }
}

/// Xiaomi Bluetooth Remote 2 Pro / RC003.
pub const RC003: DeviceProfile = DeviceProfile {
    id: "miremote-rc003",
    display_name: "Xiaomi Bluetooth Remote 2 Pro (RC003)",
    bluetooth_names: &[
        "MI RC",
        "Xiaomi Bluetooth Remote 2 Pro",
        "小米蓝牙语音遥控器",
    ],
    vendor_id: 0x2717,
    product_id: 0x32B8,
    model: "RC003",
    protocol: ProtocolVersion::V1_0,
    codecs: &[Codec::Adpcm16k],
    frame_size: 120,
    nibble_order: NibbleOrder::HighFirst,
    mic_open: MicOpenVariant::Short,
    mic_close: MicCloseVariant::Bare,
    keepalive: KeepaliveVariant::Bare,
    direct_audio_start: true,
    stop_echo: true,
    post_stop_discard_ms: 300,
    sample_rate: 16_000,
    gain_db: 0,
    button_keys: &[
        ButtonKey {
            button: RemoteButton::Power,
            key: "KEY_POWER",
        },
        ButtonKey {
            button: RemoteButton::Ok,
            key: "KEY_ENTER",
        },
        ButtonKey {
            button: RemoteButton::Ok,
            key: "KEY_KPENTER",
        },
        ButtonKey {
            button: RemoteButton::Back,
            key: "KEY_BACK",
        },
        ButtonKey {
            button: RemoteButton::Back,
            key: "KEY_ESC",
        },
        ButtonKey {
            button: RemoteButton::Up,
            key: "KEY_UP",
        },
        ButtonKey {
            button: RemoteButton::Down,
            key: "KEY_DOWN",
        },
        ButtonKey {
            button: RemoteButton::Left,
            key: "KEY_LEFT",
        },
        ButtonKey {
            button: RemoteButton::Right,
            key: "KEY_RIGHT",
        },
        // Linux HID maps the RC003 home usage (0x4A) to KEY_HOME and the
        // application/menu usage (0x65) to KEY_COMPOSE; the evdev descriptor
        // advertises both and neither KEY_HOMEPAGE nor KEY_MENU.
        ButtonKey {
            button: RemoteButton::Home,
            key: "KEY_HOME",
        },
        ButtonKey {
            button: RemoteButton::Menu,
            key: "KEY_COMPOSE",
        },
        // Field-measured: the TV key arrives as grave/tilde.
        ButtonKey {
            button: RemoteButton::Tv,
            key: "KEY_GRAVE",
        },
        ButtonKey {
            button: RemoteButton::VolumeUp,
            key: "KEY_VOLUMEUP",
        },
        ButtonKey {
            button: RemoteButton::VolumeDown,
            key: "KEY_VOLUMEDOWN",
        },
        // Grabbed so F5 cannot reach the focused window; drives ATVV.
        ButtonKey {
            button: RemoteButton::Voice,
            key: "KEY_F5",
        },
    ],
    gesture_timing: GestureTiming {
        double_click_ms: 300,
        long_press_ms: 550,
        repeat_start_ms: 350,
        stable_release_ms: 600,
        suppression_ms: 180,
        repeat_interval_ms: 100,
        back_repeat_interval_ms: 50,
    },
};

/// Profiles compiled into this build.
pub static PROFILES: &[DeviceProfile] = &[RC003];

#[must_use]
pub fn find(id: &str) -> Option<&'static DeviceProfile> {
    PROFILES.iter().find(|profile| profile.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rc003_profile_carries_observed_facts() {
        assert_eq!(RC003.protocol, ProtocolVersion::V1_0);
        assert_eq!(RC003.frame_size, 120);
        assert_eq!(RC003.sample_rate, 16_000);
        assert_eq!(RC003.gain_db, 0);
        assert_eq!(RC003.mic_open, MicOpenVariant::Short);
        assert!(RC003.matches_name("MI RC 2 Pro"));
        assert!(RC003.matches_name("Xiaomi Bluetooth Remote 2 Pro"));
        assert_eq!(RC003.gesture_timing.stable_release_ms, 600);
        assert_eq!(RC003.gesture_timing.double_click_ms, 300);
        assert_eq!(RC003.gesture_timing.long_press_ms, 550);
        assert_eq!(RC003.gesture_timing.repeat_start_ms, 350);
        assert_eq!(RC003.gesture_timing.suppression_ms, 180);
    }

    #[test]
    fn every_logical_button_has_a_key() {
        for button in RemoteButton::LOGICAL {
            assert!(RC003.primary_key(button).is_some(), "{button} has no key");
        }
        assert_eq!(RC003.primary_key(RemoteButton::Voice), Some("KEY_F5"));
        assert_eq!(RC003.primary_key(RemoteButton::Tv), Some("KEY_GRAVE"));
        assert_eq!(RC003.primary_key(RemoteButton::Home), Some("KEY_HOME"));
        assert_eq!(
            RC003.key_names(RemoteButton::Ok).collect::<Vec<_>>(),
            vec!["KEY_ENTER", "KEY_KPENTER"]
        );
    }

    #[test]
    fn profile_builds_commands() {
        assert_eq!(RC003.mic_open(), vec![0x0C, 0x00]);
        assert_eq!(RC003.mic_close(StreamId(3)), vec![0x0D]);
        assert_eq!(RC003.mic_extend(StreamId(3)), vec![0x0E]);
    }

    #[test]
    fn lookup_is_by_stable_id() {
        assert_eq!(find("miremote-rc003"), Some(&RC003));
        assert_eq!(find("unknown"), None);
    }
}
