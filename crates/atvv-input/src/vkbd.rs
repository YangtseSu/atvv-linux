// SPDX-License-Identifier: GPL-3.0-or-later

//! Virtual keyboard backed by `/dev/uinput`.
//!
//! A dedicated blocking thread owns the uinput device because `emit` is a
//! blocking write. Callers send frames over an unbounded channel, so the
//! gesture/reader threads never block on the kernel. Opening `/dev/uinput`
//! can fail; callers degrade key emission but keep the input path alive.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, BusType, EventType, InputEvent, InputId, KeyCode};
use thiserror::Error;

use crate::keymap::KeyProgram;

/// uinput name; deliberately distinct from any device profile so the
/// daemon can never grab its own keyboard.
pub const VIRTUAL_KEYBOARD_NAME: &str = "atvv-virtual-keyboard";
/// Fake vendor id, deliberately not Xiaomi's `0x2717`.
pub const VIRTUAL_VENDOR_ID: u16 = 0x1209;
/// Distinct from the fake remote's product id.
pub const VIRTUAL_PRODUCT_ID: u16 = 0x0001;

/// One key transition in a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyStep {
    pub code: KeyCode,
    /// `1` press, `0` release, `2` kernel auto-repeat.
    pub value: i32,
}

/// Keyboard emission failed or the writer thread exited.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum KeyboardError {
    #[error("virtual keyboard is unavailable (uinput open failed or the writer exited)")]
    Unavailable,
}

/// Press frame for a program: modifiers first, then the key.
#[must_use]
pub fn hold_frame(program: &KeyProgram) -> Vec<KeyStep> {
    program
        .press
        .iter()
        .map(|code| KeyStep {
            code: *code,
            value: 1,
        })
        .collect()
}

/// Release frame for a program: reverse order.
#[must_use]
pub fn release_frame(program: &KeyProgram) -> Vec<KeyStep> {
    program
        .release
        .iter()
        .map(|code| KeyStep {
            code: *code,
            value: 0,
        })
        .collect()
}

/// Tap is a press frame followed by a release frame, never one merged frame,
/// so the compositor cannot observe the key as held.
#[must_use]
pub fn tap_frames(program: &KeyProgram) -> [Vec<KeyStep>; 2] {
    [hold_frame(program), release_frame(program)]
}

/// Build the kernel events for one frame. `VirtualDevice::emit` appends
/// `SYN_REPORT`.
#[must_use]
pub fn key_events(steps: &[KeyStep]) -> Vec<InputEvent> {
    steps
        .iter()
        .map(|step| InputEvent::new_now(EventType::KEY.0, step.code.code(), step.value))
        .collect()
}

enum Command {
    /// Frame batches; each inner slice is terminated by `SYN_REPORT`.
    Frames(Vec<Vec<KeyStep>>),
}

/// `/dev/input/eventN` appears asynchronously after `UI_DEV_CREATE`.
fn wait_for_node(device: &mut VirtualDevice) -> Option<PathBuf> {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        if let Some(path) = device
            .enumerate_dev_nodes_blocking()
            .ok()
            .and_then(|mut nodes| nodes.next())
            .and_then(Result::ok)
        {
            return Some(path);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Handle to the virtual keyboard writer.
#[derive(Debug, Clone)]
pub struct VirtualKeyboard {
    tx: Sender<Command>,
    alive: Arc<AtomicBool>,
    path: Option<PathBuf>,
}

impl VirtualKeyboard {
    /// Create the virtual keyboard, registering `codes`.
    ///
    /// Fails when `/dev/uinput` is missing or not writable.
    pub fn open(codes: impl IntoIterator<Item = KeyCode>) -> std::io::Result<Self> {
        let mut keys = AttributeSet::<KeyCode>::new();
        for code in codes {
            keys.insert(code);
        }
        if keys.iter().next().is_none() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "virtual keyboard needs at least one key code",
            ));
        }
        let mut device = VirtualDevice::builder()?
            .name(VIRTUAL_KEYBOARD_NAME)
            .input_id(InputId::new(
                BusType::BUS_VIRTUAL,
                VIRTUAL_VENDOR_ID,
                VIRTUAL_PRODUCT_ID,
                0x0001,
            ))
            .with_keys(&keys)?
            .build()?;
        let path = wait_for_node(&mut device);

        let alive = Arc::new(AtomicBool::new(true));
        let (tx, rx) = mpsc::channel::<Command>();
        let thread_alive = Arc::clone(&alive);
        let _handle: JoinHandle<()> = std::thread::Builder::new()
            .name("atvv-uinput".into())
            .spawn(move || {
                while let Ok(Command::Frames(frames)) = rx.recv() {
                    for steps in frames {
                        if steps.is_empty() {
                            continue;
                        }
                        let events = key_events(&steps);
                        if device.emit(&events).is_err() {
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

    /// `/dev/input/eventN` of the virtual keyboard, when known.
    #[must_use]
    pub fn node_path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    fn send(&self, frames: Vec<Vec<KeyStep>>) -> Result<(), KeyboardError> {
        if !self.is_available() {
            return Err(KeyboardError::Unavailable);
        }
        self.tx
            .send(Command::Frames(frames))
            .map_err(|_| KeyboardError::Unavailable)
    }

    /// Tap a key program.
    pub fn tap(&self, program: &KeyProgram) -> Result<(), KeyboardError> {
        self.send(tap_frames(program).to_vec())
    }

    /// Hold a key program; release with [`Self::release`].
    pub fn hold(&self, program: &KeyProgram) -> Result<(), KeyboardError> {
        self.send(vec![hold_frame(program)])
    }

    /// Release a key program.
    pub fn release(&self, program: &KeyProgram) -> Result<(), KeyboardError> {
        self.send(vec![release_frame(program)])
    }

    /// Forward one raw key transition (unmapped keys on a grabbed node).
    pub fn emit_raw(&self, code: KeyCode, value: i32) -> Result<(), KeyboardError> {
        self.send(vec![vec![KeyStep { code, value }]])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::KeyProgram;

    fn combo() -> KeyProgram {
        KeyProgram {
            press: vec![KeyCode::KEY_LEFTMETA, KeyCode::KEY_TAB],
            release: vec![KeyCode::KEY_TAB, KeyCode::KEY_LEFTMETA],
        }
    }

    #[test]
    fn tap_is_two_frames_with_reversed_release() {
        let program = combo();
        let [press, release] = tap_frames(&program);
        assert_eq!(
            press
                .iter()
                .map(|step| (step.code, step.value))
                .collect::<Vec<_>>(),
            vec![(KeyCode::KEY_LEFTMETA, 1), (KeyCode::KEY_TAB, 1)]
        );
        assert_eq!(
            release
                .iter()
                .map(|step| (step.code, step.value))
                .collect::<Vec<_>>(),
            vec![(KeyCode::KEY_TAB, 0), (KeyCode::KEY_LEFTMETA, 0)]
        );
    }

    #[test]
    fn hold_does_not_synthesize_a_release() {
        let frame = hold_frame(&combo());
        assert!(frame.iter().all(|step| step.value == 1));
        let release = release_frame(&combo());
        assert!(release.iter().all(|step| step.value == 0));
    }

    #[test]
    fn frame_builder_emits_key_events_in_order() {
        let steps = [
            KeyStep {
                code: KeyCode::KEY_LEFTCTRL,
                value: 1,
            },
            KeyStep {
                code: KeyCode::KEY_C,
                value: 1,
            },
        ];
        let events = key_events(&steps);
        assert_eq!(events.len(), 2);
        assert!(
            events
                .iter()
                .all(|event| event.event_type() == EventType::KEY)
        );
        assert_eq!(events[0].code(), KeyCode::KEY_LEFTCTRL.code());
        assert_eq!(events[1].code(), KeyCode::KEY_C.code());
        assert!(events.iter().all(|event| event.value() == 1));
    }

    #[test]
    fn empty_key_set_is_rejected_before_touching_uinput() {
        assert_eq!(
            VirtualKeyboard::open(std::iter::empty())
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::InvalidInput
        );
    }

    #[test]
    fn unavailable_handle_reports_instead_of_panicking() {
        let (tx, rx) = mpsc::channel();
        drop(rx);
        let kb = VirtualKeyboard {
            tx,
            alive: Arc::new(AtomicBool::new(false)),
            path: None,
        };
        assert!(!kb.is_available());
        assert_eq!(kb.tap(&combo()), Err(KeyboardError::Unavailable));
        assert_eq!(
            kb.emit_raw(KeyCode::KEY_A, 1),
            Err(KeyboardError::Unavailable)
        );
    }
}

#[cfg(test)]
mod kernel_tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn wait_for(device: &mut evdev::Device, value: i32) -> bool {
        let deadline = Instant::now() + Duration::from_secs(1);
        while Instant::now() < deadline {
            match device.fetch_events() {
                Ok(events) => {
                    for event in events {
                        if event.event_type() == EventType::KEY
                            && event.code() == KeyCode::KEY_LEFTALT.code()
                            && event.value() == value
                        {
                            return true;
                        }
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(_) => return false,
            }
        }
        false
    }

    #[test]
    #[ignore = "needs /dev/uinput and evdev read access"]
    fn hold_and_release_reach_the_kernel() {
        let program = KeyProgram {
            press: vec![KeyCode::KEY_LEFTALT],
            release: vec![KeyCode::KEY_LEFTALT],
        };
        let keyboard = VirtualKeyboard::open([KeyCode::KEY_LEFTALT]).unwrap();
        let path = keyboard.node_path().unwrap().to_path_buf();
        // udev applies the seat ACL after the node appears.
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut device = loop {
            match evdev::Device::open(&path) {
                Ok(device) => break device,
                Err(_) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("cannot open {}: {error}", path.display()),
            }
        };
        device.set_nonblocking(true).unwrap();
        keyboard.hold(&program).unwrap();
        assert!(wait_for(&mut device, 1), "hold did not press the key");
        std::thread::sleep(Duration::from_millis(200));
        keyboard.release(&program).unwrap();
        assert!(wait_for(&mut device, 0), "release did not lift the key");
    }
}
