// SPDX-License-Identifier: GPL-3.0-or-later

//! Fixed-size frame accumulation for ATVV v1.0 notifications.

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FrameError {
    #[error("frame size must be non-zero")]
    ZeroFrameSize,
}

/// Buffers notification fragments and emits complete codec frames.
#[derive(Debug, Clone)]
pub struct FrameAccumulator {
    frame_size: usize,
    pending: Vec<u8>,
}

impl FrameAccumulator {
    pub fn new(frame_size: u16) -> Result<Self, FrameError> {
        let frame_size = usize::from(frame_size);
        if frame_size == 0 {
            return Err(FrameError::ZeroFrameSize);
        }
        Ok(Self {
            frame_size,
            pending: Vec::with_capacity(frame_size * 2),
        })
    }

    #[must_use]
    pub fn frame_size(&self) -> usize {
        self.frame_size
    }

    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Append bytes and return every complete frame.
    pub fn append(&mut self, data: &[u8]) -> Vec<Vec<u8>> {
        self.pending.extend_from_slice(data);
        let mut frames = Vec::new();
        while self.pending.len() >= self.frame_size {
            frames.push(self.pending.drain(..self.frame_size).collect());
        }
        frames
    }

    /// Return the final partial frame, if any.
    pub fn flush(&mut self) -> Option<Vec<u8>> {
        if self.pending.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.pending))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_complete_frames_and_keeps_partial_data() {
        let mut accumulator = FrameAccumulator::new(4).unwrap();
        assert_eq!(
            accumulator.append(&[1, 2, 3, 4, 5, 6, 7]),
            vec![vec![1, 2, 3, 4]]
        );
        assert_eq!(accumulator.pending_len(), 3);
        assert_eq!(accumulator.append(&[8]), vec![vec![5, 6, 7, 8]]);
        assert_eq!(accumulator.pending_len(), 0);
    }

    #[test]
    fn flush_returns_remainder_once() {
        let mut accumulator = FrameAccumulator::new(4).unwrap();
        accumulator.append(&[1, 2, 3]);
        assert_eq!(accumulator.flush(), Some(vec![1, 2, 3]));
        assert_eq!(accumulator.flush(), None);
    }

    #[test]
    fn rejects_zero_frame_size() {
        assert_eq!(
            FrameAccumulator::new(0).unwrap_err(),
            FrameError::ZeroFrameSize
        );
    }
}
