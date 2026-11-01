// SPDX-License-Identifier: GPL-3.0-or-later

//! Sample-rate conversion for decoded ATVV PCM.
//!
//! The resident PipeWire source runs at one rate for the daemon lifetime
//! (ADR 0009), while `CAPS_RESP` decides the rate of the incoming stream. A
//! remote that negotiates ADPCM 8 kHz into a 16 kHz graph would otherwise play
//! at double speed and underrun, so the daemon converts each decoded frame
//! instead of rebuilding the node mid-session.
//!
//! The converter is linear interpolation over `i16`. ATVV v1.0 defines only
//! 8 kHz and 16 kHz narrowband voice, where the difference from a windowed-sinc
//! design is not audible for dictation, and a stateful integer path is trivial
//! to reason about and test.

/// A stateful linear-interpolation resampler.
///
/// One instance belongs to one audio stream: the interpolation phase and the
/// previous sample carry across calls, so callers must feed consecutive
/// frames from the same stream.
#[derive(Debug, Clone)]
pub struct Resampler {
    /// Source rate in Hz.
    from: u32,
    /// Destination rate in Hz.
    to: u32,
    /// Fractional position in `[0, 1)` between `prev` and the next input.
    phase: f64,
    /// Last input sample, used as the left edge of the next interpolation.
    prev: i16,
    /// Whether `prev` holds a real sample.
    primed: bool,
}

impl Resampler {
    /// Create a converter from `from` Hz to `to` Hz.
    ///
    /// # Panics
    ///
    /// Panics if either rate is zero. A zero rate cannot appear in a validated
    /// [`AudioSourceConfig`](crate::AudioSourceConfig), and a zero step would
    /// never advance the interpolation phase, so accepting one would hang the
    /// caller rather than hide the defect.
    #[must_use]
    pub fn new(from: u32, to: u32) -> Self {
        assert!(
            from > 0 && to > 0,
            "resampler rates must be non-zero, got {from} -> {to}"
        );
        Self {
            from,
            to,
            phase: 0.0,
            prev: 0,
            primed: false,
        }
    }

    /// Whether the converter is an exact passthrough.
    #[must_use]
    pub fn is_identity(&self) -> bool {
        self.from == self.to
    }

    /// Source and destination rates.
    #[must_use]
    pub fn rates(&self) -> (u32, u32) {
        (self.from, self.to)
    }

    /// Forget the previous sample and phase. Call between streams.
    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.prev = 0;
        self.primed = false;
    }

    /// Convert `input`, appending to `out`.
    ///
    /// Returns the number of samples appended. When the rates match the input
    /// is copied verbatim and the state is left untouched.
    ///
    /// Input samples are consumed one at a time: each one emits every output
    /// position whose interpolation phase falls before it, then becomes the
    /// left edge for the following samples. The last input sample has no right
    /// edge yet, so a stream ends short by at most one input interval — two
    /// output samples when upsampling 2:1. That tail is constant: the output
    /// length is `input_len * to / from` minus the tail, whatever the length
    /// or chunking.
    pub fn process(&mut self, input: &[i16], out: &mut Vec<i16>) -> usize {
        if self.is_identity() {
            out.extend_from_slice(input);
            return input.len();
        }
        if input.is_empty() {
            return 0;
        }

        let before = out.len();
        let step = f64::from(self.from) / f64::from(self.to);

        for &current in input {
            if !self.primed {
                // The stream's first sample has no left edge; it is emitted
                // once and becomes the left edge for the rest of the stream.
                out.push(current);
                self.prev = current;
                self.primed = true;
                self.phase += step;
                continue;
            }
            let left = f64::from(self.prev);
            let right = f64::from(current);
            while self.phase < 1.0 {
                let value = left + (right - left) * self.phase;
                out.push(
                    value
                        .round()
                        .clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16,
                );
                self.phase += step;
            }
            self.phase -= 1.0;
            self.prev = current;
        }

        out.len() - before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(from: u32, to: u32, input: &[i16]) -> Vec<i16> {
        let mut resampler = Resampler::new(from, to);
        let mut out = Vec::new();
        resampler.process(input, &mut out);
        out
    }

    #[test]
    fn identity_copies_input_verbatim() {
        let input: Vec<i16> = (0..100).map(|n| (n * 300 - 15_000) as i16).collect();
        let resampler = Resampler::new(16_000, 16_000);
        assert!(resampler.is_identity());
        assert_eq!(run(16_000, 16_000, &input), input);
    }

    #[test]
    fn matches_the_integer_ratio_in_both_directions() {
        // 8 kHz device into a 16 kHz graph: the count doubles, less the
        // constant tail that waits for a right edge. 16 kHz into an 8 kHz
        // graph: the count halves.
        let input = vec![1000_i16; 8];
        let out = run(8000, 16_000, &input);
        assert_eq!(out.len(), 14);
        let input = vec![1000_i16; 16];
        assert_eq!(run(16_000, 8000, &input).len(), 8);
    }

    #[test]
    fn keeps_the_output_length_stable_for_a_long_stream() {
        // A rate error would show up as a growing deficit: 10 s of 8 kHz into
        // a 16 kHz graph must stay within one interpolation step of 160 000
        // output samples, whatever the chunking.
        let step = 0.5;
        let expected = (80_000.0 / step) as usize;
        for chunk in [80_000usize, 8_000, 800, 8] {
            let input = vec![1000_i16; chunk];
            let mut resampler = Resampler::new(8000, 16_000);
            let mut out = Vec::new();
            let mut fed = 0;
            while fed < 80_000 {
                let take = chunk.min(80_000 - fed);
                resampler.process(&input[..take], &mut out);
                fed += take;
            }
            let deficit = expected - out.len();
            assert!(
                deficit <= 2,
                "chunk {chunk}: {} samples, expected {expected}, deficit {deficit}",
                out.len()
            );
        }
    }

    #[test]
    fn upsampling_preserves_a_constant_signal() {
        let input = vec![1234_i16; 64];
        let out = run(8000, 16_000, &input);
        assert!(out.iter().all(|sample| *sample == 1234), "got {out:?}");
    }

    #[test]
    fn interpolates_between_neighbours() {
        // A ramp doubles under 2x interpolation: every step repeats.
        let input: Vec<i16> = (0..8).map(|n| (n * 1000) as i16).collect();
        let out = run(8000, 16_000, &input);
        assert_eq!(&out[..6], &[0, 500, 1000, 1500, 2000, 2500]);
    }

    #[test]
    fn carries_state_across_calls() {
        let whole = run(8000, 16_000, &[0, 1000, 2000, 3000]);

        let mut resampler = Resampler::new(8000, 16_000);
        let mut out = Vec::new();
        resampler.process(&[0, 1000], &mut out);
        resampler.process(&[2000, 3000], &mut out);
        assert_eq!(out, whole, "chunked input must match one-shot input");
    }

    #[test]
    fn reset_restarts_the_stream() {
        let mut resampler = Resampler::new(8000, 16_000);
        let mut out = Vec::new();
        resampler.process(&[500, 500, 500], &mut out);
        resampler.reset();
        let mut second = Vec::new();
        let fresh = run(8000, 16_000, &[500, 500, 500]);
        resampler.process(&[500, 500, 500], &mut second);
        assert_eq!(second, fresh);
    }

    #[test]
    fn empty_input_is_a_no_op() {
        let mut resampler = Resampler::new(8000, 16_000);
        let mut out = Vec::new();
        assert_eq!(resampler.process(&[], &mut out), 0);
        assert!(out.is_empty());
    }

    #[test]
    fn interpolates_full_scale_samples_without_wrapping() {
        // The interpolation of alternating full-scale samples stays inside the
        // i16 range instead of wrapping. The midpoint is -0.5, which rounds
        // away from zero.
        let out = run(8000, 16_000, &[i16::MIN, i16::MAX, i16::MIN, i16::MAX]);
        assert_eq!(out, [i16::MIN, -1, i16::MAX, -1, i16::MIN, -1]);
    }

    #[test]
    #[should_panic(expected = "non-zero")]
    fn rejects_a_zero_source_rate() {
        let _ = Resampler::new(0, 16_000);
    }

    #[test]
    #[should_panic(expected = "non-zero")]
    fn rejects_a_zero_destination_rate() {
        let _ = Resampler::new(16_000, 0);
    }
}
