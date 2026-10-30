// SPDX-License-Identifier: GPL-3.0-or-later

//! "Press a remote key" capture mode.
//!
//! Capture is a shared flag, not a daemon command: the D-Bus interface owns
//! the lifetime (including the disconnect watcher) and the daemon only emits
//! `ButtonEvent` while it is active.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// Capture mode state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CaptureState {
    pub active: bool,
    /// Unique bus name of the client that started the capture.
    pub owner: Option<String>,
}

pub type CaptureCell = Arc<Mutex<CaptureState>>;

#[must_use]
pub fn cell() -> CaptureCell {
    Arc::new(Mutex::new(CaptureState::default()))
}

pub fn lock(cell: &CaptureCell) -> MutexGuard<'_, CaptureState> {
    cell.lock().unwrap_or_else(PoisonError::into_inner)
}

#[must_use]
pub fn is_active(cell: &CaptureCell) -> bool {
    lock(cell).active
}

pub fn begin(cell: &CaptureCell, owner: Option<String>) {
    *lock(cell) = CaptureState {
        active: true,
        owner,
    };
}

pub fn end(cell: &CaptureCell) {
    *lock(cell) = CaptureState::default();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_and_end_are_shared_state() {
        let cell = cell();
        assert!(!is_active(&cell));
        begin(&cell, Some(":1.42".to_string()));
        assert!(is_active(&cell));
        assert_eq!(lock(&cell).owner.as_deref(), Some(":1.42"));
        end(&cell);
        assert!(!is_active(&cell));
        assert_eq!(lock(&cell).owner, None);
    }
}
