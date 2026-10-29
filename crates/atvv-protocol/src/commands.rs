// SPDX-License-Identifier: GPL-3.0-or-later

//! ATVV command encoding.

use crate::caps::Codec;
use crate::events::StreamId;

/// `GET_CAPS` for v1.0: version 1.0, ADPCM 8/16 kHz, PTT + HTT.
pub const GET_CAPS: [u8; 6] = [0x0A, 0x01, 0x00, 0x00, 0x03, 0x03];

/// RC003-style short `MIC_OPEN`.
pub const MIC_OPEN_SHORT: [u8; 2] = [0x0C, 0x00];

/// Bare `MIC_CLOSE`.
pub const MIC_CLOSE: [u8; 1] = [0x0D];

/// Bare `MIC_EXTEND`.
pub const MIC_EXTEND: [u8; 1] = [0x0E];

#[must_use]
pub fn get_caps() -> Vec<u8> {
    GET_CAPS.to_vec()
}

#[must_use]
pub fn mic_open_short() -> Vec<u8> {
    MIC_OPEN_SHORT.to_vec()
}

#[must_use]
pub fn mic_open_codec(codec: Codec) -> Vec<u8> {
    vec![0x0C, 0x00, 0x00, codec.wire()]
}

#[must_use]
pub fn mic_close() -> Vec<u8> {
    MIC_CLOSE.to_vec()
}

#[must_use]
pub fn mic_close_stream(stream: StreamId) -> Vec<u8> {
    vec![0x0D, stream.0]
}

#[must_use]
pub fn mic_extend() -> Vec<u8> {
    MIC_EXTEND.to_vec()
}

#[must_use]
pub fn mic_extend_stream(stream: StreamId) -> Vec<u8> {
    vec![0x0E, stream.0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_match_the_v1_wire_shape() {
        assert_eq!(get_caps(), vec![0x0A, 0x01, 0x00, 0x00, 0x03, 0x03]);
        assert_eq!(mic_open_short(), vec![0x0C, 0x00]);
        assert_eq!(
            mic_open_codec(Codec::Adpcm16k),
            vec![0x0C, 0x00, 0x00, 0x02]
        );
        assert_eq!(mic_close(), vec![0x0D]);
        assert_eq!(mic_close_stream(StreamId(7)), vec![0x0D, 0x07]);
        assert_eq!(mic_extend(), vec![0x0E]);
        assert_eq!(mic_extend_stream(StreamId(7)), vec![0x0E, 0x07]);
    }
}
