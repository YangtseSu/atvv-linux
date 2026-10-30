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

use evdevil::event::{InputEvent, Key, KeyEvent, KeyState};
use evdevil::uinput::UinputDevice;
use evdevil::{Bus, InputId};
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
    pub key: Key,
    pub state: KeyState,
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
        .map(|key| KeyStep {
            key: *key,
            state: KeyState::PRESSED,
        })
        .collect()
}

/// Release frame for a program: reverse order.
#[must_use]
pub fn release_frame(program: &KeyProgram) -> Vec<KeyStep> {
    program
        .release
        .iter()
        .map(|key| KeyStep {
            key: *key,
            state: KeyState::RELEASED,
        })
        .collect()
}

/// Tap is a press frame followed by a release frame, never one merged frame,
/// so the compositor cannot observe the key as held.
#[must_use]
pub fn tap_frames(program: &KeyProgram) -> [Vec<KeyStep>; 2] {
    [hold_frame(program), release_frame(program)]
}

/// Build the kernel events for one frame. `UinputDevice::write_events`
/// appends `SYN_REPORT`.
#[must_use]
pub fn key_events(steps: &[KeyStep]) -> Vec<InputEvent> {
    steps
        .iter()
        .map(|step| KeyEvent::new(step.key, step.state).into())
        .collect()
}

enum Command {
    /// Frame batches; each inner slice is terminated by `SYN_REPORT`.
    Frames(Vec<Vec<KeyStep>>),
}

/// Resolve the `/dev/input/eventN` node of a uinput device through sysfs.
///
/// `UinputDevice::sysname()` returns `inputN`; the matching event node is a
/// child of `/sys/class/input/inputN/`. The node appears asynchronously after
/// `UI_DEV_CREATE`, so callers poll.
fn event_node(device: &UinputDevice) -> std::io::Result<Option<PathBuf>> {
    let sysname = device.sysname()?;
    let Some(sysname) = sysname.to_str() else {
        return Ok(None);
    };
    let Ok(entries) = std::fs::read_dir(Path::new("/sys/class/input").join(sysname)) else {
        return Ok(None);
    };
    for entry in entries {
        let name = entry?.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("event") {
            return Ok(Some(Path::new("/dev/input").join(name.as_ref())));
        }
    }
    Ok(None)
}

/// Wait for the uinput device's event node to appear.
pub(crate) fn wait_for_node(device: &UinputDevice) -> Option<PathBuf> {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        if let Ok(Some(path)) = event_node(device) {
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
    /// Create the virtual keyboard, registering `keys`.
    ///
    /// Fails when `/dev/uinput` is missing or not writable.
    pub fn open(keys: impl IntoIterator<Item = Key>) -> std::io::Result<Self> {
        let keys: Vec<Key> = keys.into_iter().collect();
        if keys.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "virtual keyboard needs at least one key code",
            ));
        }
        let device = UinputDevice::builder()?
            .with_input_id(InputId::new(
                Bus::VIRTUAL,
                VIRTUAL_VENDOR_ID,
                VIRTUAL_PRODUCT_ID,
                0x0001,
            ))?
            .with_keys(keys)?
            .build(VIRTUAL_KEYBOARD_NAME)?;
        let path = wait_for_node(&device);

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
                        if device.write_events(&key_events(&steps)).is_err() {
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
    pub fn emit_raw(&self, key: Key, state: KeyState) -> Result<(), KeyboardError> {
        self.send(vec![vec![KeyStep { key, state }]])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::KeyProgram;

    fn combo() -> KeyProgram {
        KeyProgram {
            press: vec![Key::KEY_LEFTMETA, Key::KEY_TAB],
            release: vec![Key::KEY_TAB, Key::KEY_LEFTMETA],
        }
    }

    #[test]
    fn tap_is_two_frames_with_reversed_release() {
        let program = combo();
        let [press, release] = tap_frames(&program);
        assert_eq!(
            press
                .iter()
                .map(|step| (step.key, step.state))
                .collect::<Vec<_>>(),
            vec![
                (Key::KEY_LEFTMETA, KeyState::PRESSED),
                (Key::KEY_TAB, KeyState::PRESSED)
            ]
        );
        assert_eq!(
            release
                .iter()
                .map(|step| (step.key, step.state))
                .collect::<Vec<_>>(),
            vec![
                (Key::KEY_TAB, KeyState::RELEASED),
                (Key::KEY_LEFTMETA, KeyState::RELEASED)
            ]
        );
    }

    #[test]
    fn hold_does_not_synthesize_a_release() {
        let frame = hold_frame(&combo());
        assert!(frame.iter().all(|step| step.state == KeyState::PRESSED));
        let release = release_frame(&combo());
        assert!(release.iter().all(|step| step.state == KeyState::RELEASED));
    }

    #[test]
    fn frame_builder_emits_key_events_in_order() {
        let steps = [
            KeyStep {
                key: Key::KEY_LEFTCTRL,
                state: KeyState::PRESSED,
            },
            KeyStep {
                key: Key::KEY_C,
                state: KeyState::PRESSED,
            },
        ];
        let events = key_events(&steps);
        assert_eq!(events.len(), 2);
        assert!(
            events
                .iter()
                .all(|event| event.event_type() == evdevil::event::EventType::KEY)
        );
        assert_eq!(events[0].raw_code(), Key::KEY_LEFTCTRL.raw());
        assert_eq!(events[1].raw_code(), Key::KEY_C.raw());
        assert!(events.iter().all(|event| event.raw_value() == 1));
    }

    #[test]
    fn consumer_keys_are_emittable() {
        let steps = [
            KeyStep {
                key: Key::KEY_VOICECOMMAND,
                state: KeyState::PRESSED,
            },
            KeyStep {
                key: Key::KEY_ASSISTANT,
                state: KeyState::PRESSED,
            },
            KeyStep {
                key: Key::KEY_DICTATE,
                state: KeyState::PRESSED,
            },
        ];
        let events = key_events(&steps);
        assert_eq!(
            events
                .iter()
                .map(|event| event.raw_code())
                .collect::<Vec<_>>(),
            vec![
                Key::KEY_VOICECOMMAND.raw(),
                Key::KEY_ASSISTANT.raw(),
                Key::KEY_DICTATE.raw()
            ]
        );
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
            kb.emit_raw(Key::KEY_A, KeyState::PRESSED),
            Err(KeyboardError::Unavailable)
        );
    }
}

#[cfg(test)]
mod kernel_tests {
    use super::*;
    use evdevil::Evdev;
    use evdevil::event::EventKind;
    use std::time::{Duration, Instant};

    fn wait_for(reader: &mut evdevil::EventReader, key: Key, state: KeyState) -> bool {
        let deadline = Instant::now() + Duration::from_secs(1);
        while Instant::now() < deadline {
            let mut found = false;
            for event in reader.events() {
                let Ok(event) = event else {
                    return false;
                };
                if let EventKind::Key(event) = event.kind()
                    && event.key() == key
                    && event.state() == state
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
    fn hold_and_release_reach_the_kernel() {
        let program = KeyProgram {
            press: vec![Key::KEY_LEFTALT],
            release: vec![Key::KEY_LEFTALT],
        };
        let keyboard = VirtualKeyboard::open([Key::KEY_LEFTALT]).unwrap();
        let path = keyboard.node_path().unwrap().to_path_buf();
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
        keyboard.hold(&program).unwrap();
        assert!(
            wait_for(&mut reader, Key::KEY_LEFTALT, KeyState::PRESSED),
            "hold did not press the key"
        );
        std::thread::sleep(Duration::from_millis(200));
        keyboard.release(&program).unwrap();
        assert!(
            wait_for(&mut reader, Key::KEY_LEFTALT, KeyState::RELEASED),
            "release did not lift the key"
        );
    }
}
