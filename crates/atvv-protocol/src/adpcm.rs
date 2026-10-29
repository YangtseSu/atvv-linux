// SPDX-License-Identifier: GPL-3.0-or-later

//! IMA/DVI ADPCM decoder for ATVV v1.0 headerless frames.

/// IMA/DVI step-size table.
const STEP_TABLE: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449,
    494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272,
    2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493,
    10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767,
];

/// IMA/DVI step-index adjustment table.
const INDEX_TABLE: [i32; 8] = [-1, -1, -1, -1, 2, 4, 6, 8];

const MAX_STEP_INDEX: u8 = 88;

/// Stateful IMA/DVI ADPCM decoder.
///
/// v1.0 frames are headerless: the decoder state persists across frames and is
/// refreshed by a `0x0A` sync packet.
#[derive(Debug, Clone, Copy, Default)]
pub struct Decoder {
    predictor: i16,
    step_index: u8,
}

impl Decoder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self, predictor: i16, step_index: u8) {
        self.predictor = predictor;
        self.step_index = step_index.min(MAX_STEP_INDEX);
    }

    #[must_use]
    pub fn predictor(self) -> i16 {
        self.predictor
    }

    #[must_use]
    pub fn step_index(self) -> u8 {
        self.step_index
    }

    /// Decode one 4-bit nibble and update the predictor/index.
    pub fn decode_nibble(&mut self, nibble: u8) -> i16 {
        debug_assert!(nibble <= 0x0F);
        let step = STEP_TABLE[self.step_index as usize];
        let mut diff = step >> 3;
        if nibble & 0x1 != 0 {
            diff += step >> 2;
        }
        if nibble & 0x2 != 0 {
            diff += step >> 1;
        }
        if nibble & 0x4 != 0 {
            diff += step;
        }

        let mut predictor = i32::from(self.predictor);
        if nibble & 0x8 != 0 {
            predictor -= diff;
        } else {
            predictor += diff;
        }
        self.predictor = predictor.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;

        let next_index = i32::from(self.step_index) + INDEX_TABLE[usize::from(nibble & 0x7)];
        self.step_index = next_index.clamp(0, i32::from(MAX_STEP_INDEX)) as u8;
        self.predictor
    }

    /// Decode bytes into `out`, high nibble first. Returns the number of samples.
    pub fn decode_into(&mut self, data: &[u8], out: &mut [i16]) -> usize {
        let mut written = 0;
        for &byte in data {
            if written >= out.len() {
                break;
            }
            out[written] = self.decode_nibble(byte >> 4);
            written += 1;
            if written >= out.len() {
                break;
            }
            out[written] = self.decode_nibble(byte & 0x0F);
            written += 1;
        }
        written
    }

    /// Decode a v1.0 headerless frame.
    #[must_use]
    pub fn decode(&mut self, data: &[u8]) -> Vec<i16> {
        let mut samples = vec![0_i16; data.len() * 2];
        let written = self.decode_into(data, &mut samples);
        samples.truncate(written);
        samples
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_input_decodes_to_silence() {
        let mut decoder = Decoder::new();
        assert_eq!(decoder.decode(&[0x00, 0x00, 0x00]), vec![0, 0, 0, 0, 0, 0]);
        assert_eq!(decoder.predictor(), 0);
        assert_eq!(decoder.step_index(), 0);
    }

    #[test]
    fn nibbles_are_high_first_and_signed() {
        let mut decoder = Decoder::new();
        assert_eq!(decoder.decode(&[0x07]), vec![0, 11]);
        let mut decoder = Decoder::new();
        assert_eq!(decoder.decode(&[0x0F]), vec![0, -11]);
    }

    #[test]
    fn reset_clamps_step_index() {
        let mut decoder = Decoder::new();
        decoder.reset(123, 200);
        assert_eq!(decoder.predictor(), 123);
        assert_eq!(decoder.step_index(), MAX_STEP_INDEX);
    }

    #[test]
    fn step_index_stays_in_range() {
        let mut decoder = Decoder::new();
        for _ in 0..10_000 {
            decoder.decode_nibble(0x0F);
            assert!(decoder.step_index() <= MAX_STEP_INDEX);
        }
    }

    #[test]
    fn decode_into_respects_output_capacity() {
        let mut decoder = Decoder::new();
        let mut out = [0_i16; 2];
        assert_eq!(decoder.decode_into(&[0x00, 0x00], &mut out), 2);
    }
}
