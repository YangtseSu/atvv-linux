// SPDX-License-Identifier: GPL-3.0-or-later

//! Capability negotiation (`GET_CAPS` / `CAPS_RESP`).
//!
//! Only ATVV v1.0 is accepted. The v0.4 layout is intentionally out of scope.

use thiserror::Error;

/// ATVV wire protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolVersion {
    V1_0,
}

impl ProtocolVersion {
    pub const V1_0_WIRE: u16 = 0x0100;

    #[must_use]
    pub fn wire(self) -> u16 {
        match self {
            Self::V1_0 => Self::V1_0_WIRE,
        }
    }

    #[must_use]
    pub fn from_wire(value: u16) -> Option<Self> {
        match value {
            Self::V1_0_WIRE => Some(Self::V1_0),
            _ => None,
        }
    }
}

impl std::fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::V1_0 => f.write_str("1.0"),
        }
    }
}

/// IMA/DVI ADPCM codec advertised by the remote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    Adpcm8k,
    Adpcm16k,
}

impl Codec {
    #[must_use]
    pub fn wire(self) -> u8 {
        match self {
            Self::Adpcm8k => 0x01,
            Self::Adpcm16k => 0x02,
        }
    }

    #[must_use]
    pub fn from_wire(value: u8) -> Option<Self> {
        match value {
            0x01 => Some(Self::Adpcm8k),
            0x02 => Some(Self::Adpcm16k),
            _ => None,
        }
    }

    #[must_use]
    pub fn bit(self) -> u8 {
        self.wire()
    }

    #[must_use]
    pub fn sample_rate(self) -> u32 {
        match self {
            Self::Adpcm8k => 8_000,
            Self::Adpcm16k => 16_000,
        }
    }
}

/// Interaction model reported in `CAPS_RESP`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionModel {
    OnRequest,
    PressToTalk,
    HoldToTalk,
    Unknown(u8),
}

impl InteractionModel {
    #[must_use]
    pub fn from_wire(value: u8) -> Self {
        match value {
            0x00 => Self::OnRequest,
            0x01 => Self::PressToTalk,
            0x03 => Self::HoldToTalk,
            other => Self::Unknown(other),
        }
    }

    #[must_use]
    pub fn wire(self) -> u8 {
        match self {
            Self::OnRequest => 0x00,
            Self::PressToTalk => 0x01,
            Self::HoldToTalk => 0x03,
            Self::Unknown(value) => value,
        }
    }
}

/// Parsed capabilities from a v1.0 `CAPS_RESP` notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    pub version: ProtocolVersion,
    /// Raw codec bitmask. Use [`Self::supports`] or [`Self::best_codec`].
    pub codec_mask: u8,
    pub interaction: InteractionModel,
    pub frame_size: u16,
}

impl Capabilities {
    /// Fixed v1.0 layout: `0B version(2) codecs(1) interaction(1) frame(2)`.
    pub const LEN: usize = 7;

    #[must_use]
    pub fn supports(self, codec: Codec) -> bool {
        self.codec_mask & codec.bit() != 0
    }

    /// Best codec we support, preferring 16 kHz.
    #[must_use]
    pub fn best_codec(self) -> Option<Codec> {
        if self.supports(Codec::Adpcm16k) {
            Some(Codec::Adpcm16k)
        } else if self.supports(Codec::Adpcm8k) {
            Some(Codec::Adpcm8k)
        } else {
            None
        }
    }

    /// Parse a v1.0 `CAPS_RESP` payload.
    ///
    /// Some firmware reports `codecs = 0` and puts the codec bits in the
    /// interaction byte. The parser promotes that byte, matching observed
    /// ATVV remotes.
    pub fn parse(data: &[u8]) -> Result<Self, CapsError> {
        if data.len() < Self::LEN {
            return Err(CapsError::TooShort { len: data.len() });
        }
        if data[0] != 0x0B {
            return Err(CapsError::BadOpcode(data[0]));
        }

        let version_wire = u16::from_be_bytes([data[1], data[2]]);
        let version = ProtocolVersion::from_wire(version_wire)
            .ok_or(CapsError::UnknownVersion(version_wire))?;

        let interaction_raw = data[4];
        let codec_mask = if data[3] == 0 {
            interaction_raw
        } else {
            data[3]
        };
        let frame_size = u16::from_be_bytes([data[5], data[6]]);
        if frame_size == 0 {
            return Err(CapsError::ZeroFrameSize);
        }

        Ok(Self {
            version,
            codec_mask,
            interaction: InteractionModel::from_wire(interaction_raw),
            frame_size,
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CapsError {
    #[error("CAPS_RESP too short: {len} bytes, need at least 7")]
    TooShort { len: usize },
    #[error("expected CAPS_RESP opcode 0x0b, got 0x{0:02x}")]
    BadOpcode(u8),
    #[error("unsupported ATVV version 0x{0:04x}")]
    UnknownVersion(u16),
    #[error("CAPS_RESP frame size is zero")]
    ZeroFrameSize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rc003_payload() -> [u8; 7] {
        [0x0B, 0x01, 0x00, 0x02, 0x00, 0x00, 0x78]
    }

    #[test]
    fn parses_rc003_capabilities() {
        let caps = Capabilities::parse(&rc003_payload()).unwrap();
        assert_eq!(caps.version, ProtocolVersion::V1_0);
        assert_eq!(caps.codec_mask, 0x02);
        assert_eq!(caps.best_codec(), Some(Codec::Adpcm16k));
        assert_eq!(caps.interaction, InteractionModel::OnRequest);
        assert_eq!(caps.frame_size, 120);
    }

    #[test]
    fn parses_hold_to_talk_and_160_byte_frames() {
        let payload = [0x0B, 0x01, 0x00, 0x02, 0x03, 0x00, 0xA0];
        let caps = Capabilities::parse(&payload).unwrap();
        assert_eq!(caps.interaction, InteractionModel::HoldToTalk);
        assert_eq!(caps.frame_size, 160);
    }

    #[test]
    fn promotes_codec_bits_from_interaction_byte() {
        let payload = [0x0B, 0x01, 0x00, 0x00, 0x02, 0x00, 0x78];
        let caps = Capabilities::parse(&payload).unwrap();
        assert_eq!(caps.codec_mask, 0x02);
        assert_eq!(caps.best_codec(), Some(Codec::Adpcm16k));
    }

    #[test]
    fn rejects_short_bad_opcode_unknown_version_and_zero_frame() {
        assert_eq!(
            Capabilities::parse(&[0x0B, 0x01, 0x00]),
            Err(CapsError::TooShort { len: 3 })
        );
        let mut bad_opcode = rc003_payload();
        bad_opcode[0] = 0x04;
        assert_eq!(
            Capabilities::parse(&bad_opcode),
            Err(CapsError::BadOpcode(0x04))
        );
        let mut bad_version = rc003_payload();
        bad_version[1] = 0x00;
        bad_version[2] = 0x04;
        assert_eq!(
            Capabilities::parse(&bad_version),
            Err(CapsError::UnknownVersion(0x0004))
        );
        let mut zero_frame = rc003_payload();
        zero_frame[5] = 0;
        zero_frame[6] = 0;
        assert_eq!(
            Capabilities::parse(&zero_frame),
            Err(CapsError::ZeroFrameSize)
        );
    }

    #[test]
    fn prefers_16k_when_both_codecs_are_offered() {
        let payload = [0x0B, 0x01, 0x00, 0x03, 0x00, 0x00, 0x78];
        let caps = Capabilities::parse(&payload).unwrap();
        assert_eq!(caps.best_codec(), Some(Codec::Adpcm16k));
    }
}
