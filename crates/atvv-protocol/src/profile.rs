// SPDX-License-Identifier: GPL-3.0-or-later

//! Device profiles: the facts that differ between ATVV remotes.

use crate::caps::{Codec, ProtocolVersion};
use crate::commands;
use crate::events::StreamId;

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
    pub voice_key: &'static str,
    pub tv_key: &'static str,
}

impl DeviceProfile {
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
    voice_key: "KEY_F5",
    tv_key: "KEY_GRAVE",
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
