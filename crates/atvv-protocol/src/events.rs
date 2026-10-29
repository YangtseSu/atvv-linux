// SPDX-License-Identifier: GPL-3.0-or-later

//! ATVV control-channel events.

use crate::caps::{Capabilities, Codec};

/// Stream identifier carried by v1.0 `AUDIO_START` and `MIC_CLOSE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamId(pub u8);

impl StreamId {
    pub const MIC_OPEN: Self = Self(0x00);
    pub const ANY: Self = Self(0xFF);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioStartReason {
    MicOpen,
    PressToTalk,
    HoldToTalk,
    Other(u8),
}

impl AudioStartReason {
    #[must_use]
    pub fn from_wire(value: u8) -> Self {
        match value {
            0x00 => Self::MicOpen,
            0x01 => Self::PressToTalk,
            0x03 => Self::HoldToTalk,
            other => Self::Other(other),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioStopReason {
    MicClose,
    HttButtonRelease,
    UpcomingAudioStart,
    TransferTimeout,
    NotificationsDisabled,
    Other(u8),
}

impl AudioStopReason {
    #[must_use]
    pub fn from_wire(value: u8) -> Self {
        match value {
            0x00 => Self::MicClose,
            0x02 => Self::HttButtonRelease,
            0x04 => Self::UpcomingAudioStart,
            0x08 => Self::TransferTimeout,
            0x10 => Self::NotificationsDisabled,
            other => Self::Other(other),
        }
    }
}

/// A parsed control-channel notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CtlEvent {
    AudioStart {
        reason: AudioStartReason,
        /// Raw codec byte. The capability response is authoritative.
        codec: Option<Codec>,
        stream_id: StreamId,
    },
    AudioStop {
        reason: AudioStopReason,
    },
    StartSearch,
    AudioSync {
        codec: Option<Codec>,
        seq: u16,
        predictor: i16,
        step_index: u8,
    },
    CapsResp(Capabilities),
    MicOpenError(u16),
    Unknown(Vec<u8>),
}

impl CtlEvent {
    /// Parse one v1.0 control notification.
    #[must_use]
    pub fn parse(data: &[u8]) -> Self {
        let Some(opcode) = data.first().copied() else {
            return Self::Unknown(Vec::new());
        };

        match opcode {
            0x00 => Self::AudioStop {
                reason: data.get(1).map_or(AudioStopReason::MicClose, |value| {
                    AudioStopReason::from_wire(*value)
                }),
            },
            0x04 => {
                if data.len() < 4 {
                    return Self::Unknown(data.to_vec());
                }
                Self::AudioStart {
                    reason: AudioStartReason::from_wire(data[1]),
                    codec: Codec::from_wire(data[2]),
                    stream_id: StreamId(data[3]),
                }
            }
            0x08 => Self::StartSearch,
            0x0A => {
                if data.len() < 7 {
                    return Self::Unknown(data.to_vec());
                }
                Self::AudioSync {
                    codec: Codec::from_wire(data[1]),
                    seq: u16::from_be_bytes([data[2], data[3]]),
                    predictor: i16::from_be_bytes([data[4], data[5]]),
                    step_index: data[6],
                }
            }
            0x0B => Capabilities::parse(data)
                .map_or_else(|_| Self::Unknown(data.to_vec()), Self::CapsResp),
            0x0C => {
                if data.len() < 3 {
                    return Self::Unknown(data.to_vec());
                }
                Self::MicOpenError(u16::from_be_bytes([data[1], data[2]]))
            }
            _ => Self::Unknown(data.to_vec()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::caps::{InteractionModel, ProtocolVersion};

    #[test]
    fn parses_audio_start_with_trusted_caps_codec() {
        let event = CtlEvent::parse(&[0x04, 0x03, 0x02, 0x01]);
        assert_eq!(
            event,
            CtlEvent::AudioStart {
                reason: AudioStartReason::HoldToTalk,
                codec: Some(Codec::Adpcm16k),
                stream_id: StreamId(1),
            }
        );
    }

    #[test]
    fn parses_audio_stop_reasons() {
        assert_eq!(
            CtlEvent::parse(&[0x00]),
            CtlEvent::AudioStop {
                reason: AudioStopReason::MicClose
            }
        );
        assert_eq!(
            CtlEvent::parse(&[0x00, 0x02]),
            CtlEvent::AudioStop {
                reason: AudioStopReason::HttButtonRelease
            }
        );
    }

    #[test]
    fn parses_start_search_sync_and_mic_open_error() {
        assert_eq!(CtlEvent::parse(&[0x08]), CtlEvent::StartSearch);
        assert_eq!(
            CtlEvent::parse(&[0x0A, 0x02, 0x01, 0x2C, 0xFF, 0xF0, 0x1E]),
            CtlEvent::AudioSync {
                codec: Some(Codec::Adpcm16k),
                seq: 300,
                predictor: -16,
                step_index: 30,
            }
        );
        assert_eq!(
            CtlEvent::parse(&[0x0C, 0x0F, 0x01]),
            CtlEvent::MicOpenError(0x0F01)
        );
    }

    #[test]
    fn parses_caps_and_keeps_invalid_payload_unknown() {
        let caps = [0x0B, 0x01, 0x00, 0x02, 0x03, 0x00, 0xA0];
        assert_eq!(
            CtlEvent::parse(&caps),
            CtlEvent::CapsResp(Capabilities {
                version: ProtocolVersion::V1_0,
                codec_mask: 0x02,
                interaction: InteractionModel::HoldToTalk,
                frame_size: 160,
            })
        );
        assert!(matches!(
            CtlEvent::parse(&[0x0B, 0x01, 0x00]),
            CtlEvent::Unknown(_)
        ));
    }

    #[test]
    fn empty_and_unknown_packets_are_preserved() {
        assert_eq!(CtlEvent::parse(&[]), CtlEvent::Unknown(Vec::new()));
        assert_eq!(
            CtlEvent::parse(&[0x7F, 0x01]),
            CtlEvent::Unknown(vec![0x7F, 0x01])
        );
    }
}
