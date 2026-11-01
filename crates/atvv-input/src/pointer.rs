// SPDX-License-Identifier: GPL-3.0-or-later

//! Virtual pointer backed by `/dev/uinput`.
//!
//! A relative pointer device is separate from the virtual keyboard because
//! `BTN_LEFT` on a keyboard-only device is not a pointer click: compositors
//! route pointer buttons and relative motion through devices that advertise
//! `REL_X`/`REL_Y` and the button codes. The device is opened only when the
//! action map contains pointer actions, so a keyboard-only configuration does
//! not add a mouse to the session.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;

use evdevil::event::{InputEvent, Key, KeyEvent, KeyState, Rel, RelEvent};
use evdevil::uinput::UinputDevice;
use evdevil::{Bus, InputId, InputProp};
use thiserror::Error;

use crate::action::{PointerAction, PointerAxis, PointerButton};
use crate::vkbd::{VIRTUAL_VENDOR_ID, wait_for_node};

/// uinput name; distinct from the keyboard and every device profile.
pub const VIRTUAL_POINTER_NAME: &str = "atvv-virtual-pointer";
/// Distinct from the virtual keyboard (`0x0001`) and the fake remote (`0x0002`).
pub const VIRTUAL_POINTER_PRODUCT_ID: u16 = 0x0003;

/// Pointer emission failed or the writer thread exited.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PointerError {
    #[error("virtual pointer is unavailable (uinput open failed or the writer exited)")]
    Unavailable,
}

/// Kernel key code for one pointer button.
#[must_use]
pub const fn button_key(button: PointerButton) -> Key {
    match button {
        PointerButton::Left => Key::BTN_LEFT,
        PointerButton::Right => Key::BTN_RIGHT,
        PointerButton::Middle => Key::BTN_MIDDLE,
        PointerButton::Side => Key::BTN_SIDE,
        PointerButton::Extra => Key::BTN_EXTRA,
        PointerButton::Forward => Key::BTN_FORWARD,
        PointerButton::Back => Key::BTN_BACK,
        PointerButton::Task => Key::BTN_TASK,
    }
}

/// Relative axis and signed direction for a pointer move.
#[must_use]
pub const fn axis_motion(direction: PointerAxis) -> (Rel, i32) {
    match direction {
        PointerAxis::Up => (Rel::Y, -1),
        PointerAxis::Down => (Rel::Y, 1),
        PointerAxis::Left => (Rel::X, -1),
        PointerAxis::Right => (Rel::X, 1),
    }
}

/// Press and release frames for a click. Each frame is one SYN_REPORT batch
/// because `UinputDevice::write_events` appends the report.
#[must_use]
pub fn click_frames(button: PointerButton) -> [Vec<InputEvent>; 2] {
    let key = button_key(button);
    [
        vec![KeyEvent::new(key, KeyState::PRESSED).into()],
        vec![KeyEvent::new(key, KeyState::RELEASED).into()],
    ]
}

/// One frame of relative motion.
#[must_use]
pub fn move_events(direction: PointerAxis, units: u16) -> Vec<InputEvent> {
    let (axis, sign) = axis_motion(direction);
    vec![RelEvent::new(axis, sign * i32::from(units)).into()]
}

enum Command {
    /// Frame batches; each inner slice is terminated by `SYN_REPORT`.
    Frames(Vec<Vec<InputEvent>>),
}

/// Handle to the virtual pointer writer.
#[derive(Debug, Clone)]
pub struct VirtualPointer {
    tx: Sender<Command>,
    alive: Arc<AtomicBool>,
    path: Option<PathBuf>,
}

impl VirtualPointer {
    /// Create the virtual pointer with every supported button and both axes.
    ///
    /// Fails when `/dev/uinput` is missing or not writable.
    pub fn open() -> std::io::Result<Self> {
        let keys = PointerButton::ALL.map(button_key);
        let device = UinputDevice::builder()?
            .with_input_id(InputId::new(
                Bus::VIRTUAL,
                VIRTUAL_VENDOR_ID,
                VIRTUAL_POINTER_PRODUCT_ID,
                0x0001,
            ))?
            .with_keys(keys)?
            .with_rel_axes([Rel::X, Rel::Y])?
            .with_props([InputProp::POINTER])?
            .build(VIRTUAL_POINTER_NAME)?;
        let path = wait_for_node(&device);

        let alive = Arc::new(AtomicBool::new(true));
        let (tx, rx) = mpsc::channel::<Command>();
        let thread_alive = Arc::clone(&alive);
        let _handle: JoinHandle<()> = std::thread::Builder::new()
            .name("atvv-pointer".into())
            .spawn(move || {
                while let Ok(Command::Frames(frames)) = rx.recv() {
                    for events in frames {
                        if events.is_empty() {
                            continue;
                        }
                        if device.write_events(&events).is_err() {
                            thread_alive.store(false, Ordering::Relaxed);
                            return;
                        }
                    }
                }
            })?;
        Ok(Self { tx, alive, path })
    }

    /// Whether the writer is still accepting frames.
    #[must_use]
    pub fn is_available(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }

    /// `/dev/input/eventN` of the virtual pointer, when known.
    #[must_use]
    pub fn node_path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    fn send(&self, frames: Vec<Vec<InputEvent>>) -> Result<(), PointerError> {
        if !self.is_available() {
            return Err(PointerError::Unavailable);
        }
        self.tx
            .send(Command::Frames(frames))
            .map_err(|_| PointerError::Unavailable)
    }

    /// Press and release one pointer button.
    pub fn click(&self, button: PointerButton) -> Result<(), PointerError> {
        self.send(click_frames(button).to_vec())
    }

    /// Move by `units` relative counts in `direction`.
    pub fn move_by(&self, direction: PointerAxis, units: u16) -> Result<(), PointerError> {
        self.send(vec![move_events(direction, units)])
    }

    /// Emit one parsed pointer action.
    pub fn apply(&self, action: &PointerAction) -> Result<(), PointerError> {
        match action {
            PointerAction::Click(button) => self.click(*button),
            PointerAction::Move { direction, units } => self.move_by(*direction, *units),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_frames_press_then_release_the_button() {
        let [press, release] = click_frames(PointerButton::Left);
        assert_eq!(press.len(), 1);
        assert_eq!(release.len(), 1);
        assert_eq!(press[0].raw_code(), Key::BTN_LEFT.raw());
        assert_eq!(press[0].raw_value(), 1);
        assert_eq!(release[0].raw_code(), Key::BTN_LEFT.raw());
        assert_eq!(release[0].raw_value(), 0);
    }

    #[test]
    fn every_function_button_has_a_distinct_kernel_code() {
        let mut codes = std::collections::BTreeSet::new();
        for button in PointerButton::ALL {
            let key = button_key(button);
            assert!(matches!(
                key,
                Key::BTN_LEFT
                    | Key::BTN_RIGHT
                    | Key::BTN_MIDDLE
                    | Key::BTN_SIDE
                    | Key::BTN_EXTRA
                    | Key::BTN_FORWARD
                    | Key::BTN_BACK
                    | Key::BTN_TASK
            ));
            assert!(codes.insert(key.raw()), "duplicate code for {button:?}");
        }
        assert_eq!(codes.len(), PointerButton::ALL.len());
    }

    #[test]
    fn motion_uses_signed_relative_axes() {
        for (direction, axis, sign) in [
            (PointerAxis::Up, Rel::Y, -1),
            (PointerAxis::Down, Rel::Y, 1),
            (PointerAxis::Left, Rel::X, -1),
            (PointerAxis::Right, Rel::X, 1),
        ] {
            let events = move_events(direction, 7);
            assert_eq!(events.len(), 1);
            assert_eq!(events[0].raw_code(), axis.raw());
            assert_eq!(events[0].raw_value(), sign * 7);
        }
    }

    #[test]
    fn unavailable_handle_reports_instead_of_panicking() {
        let (tx, rx) = mpsc::channel();
        drop(rx);
        let pointer = VirtualPointer {
            tx,
            alive: Arc::new(AtomicBool::new(false)),
            path: None,
        };
        assert!(!pointer.is_available());
        assert_eq!(
            pointer.click(PointerButton::Left),
            Err(PointerError::Unavailable)
        );
        assert_eq!(
            pointer.move_by(PointerAxis::Up, 1),
            Err(PointerError::Unavailable)
        );
    }
}

#[cfg(test)]
mod kernel_tests {
    use super::*;
    use evdevil::Evdev;
    use evdevil::event::EventKind;
    use std::time::{Duration, Instant};

    fn wait_for_click(reader: &mut evdevil::EventReader, key: Key) -> (bool, bool) {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut pressed = false;
        let mut released = false;
        while Instant::now() < deadline {
            for event in reader.events() {
                let Ok(event) = event else {
                    return (pressed, released);
                };
                if let EventKind::Key(event) = event.kind()
                    && event.key() == key
                {
                    if event.state() == KeyState::PRESSED {
                        pressed = true;
                    }
                    if event.state() == KeyState::RELEASED {
                        released = true;
                    }
                }
            }
            if pressed && released {
                return (pressed, released);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        (pressed, released)
    }

    fn wait_for_rel(reader: &mut evdevil::EventReader, axis: Rel, value: i32) -> bool {
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            let mut found = false;
            for event in reader.events() {
                let Ok(event) = event else {
                    return false;
                };
                if let EventKind::Rel(event) = event.kind()
                    && event.rel() == axis
                    && event.value() == value
                {
                    found = true;
                }
            }
            if found {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        false
    }

    #[test]
    #[ignore = "needs /dev/uinput and evdev read access"]
    fn click_and_move_reach_the_kernel() {
        let pointer = VirtualPointer::open().expect("virtual pointer");
        let path = pointer.node_path().expect("pointer node").to_path_buf();
        // udev applies the seat ACL after the node appears.
        let deadline = Instant::now() + Duration::from_secs(3);
        let device = loop {
            match Evdev::open(&path) {
                Ok(device) => break device,
                Err(_) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("cannot open {}: {error}", path.display()),
            }
        };
        device.set_nonblocking(true).unwrap();
        let mut reader = device.into_reader().unwrap();

        pointer.click(PointerButton::Left).unwrap();
        let (pressed, released) = wait_for_click(&mut reader, Key::BTN_LEFT);
        assert!(pressed, "click did not press BTN_LEFT");
        assert!(released, "click did not release BTN_LEFT");

        pointer.move_by(PointerAxis::Up, 3).unwrap();
        assert!(
            wait_for_rel(&mut reader, Rel::Y, -3),
            "move did not emit REL_Y=-3"
        );
        pointer.move_by(PointerAxis::Right, 4).unwrap();
        assert!(
            wait_for_rel(&mut reader, Rel::X, 4),
            "move did not emit REL_X=4"
        );
    }
}
