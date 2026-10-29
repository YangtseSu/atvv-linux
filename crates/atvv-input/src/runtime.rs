// SPDX-License-Identifier: GPL-3.0-or-later

//! Blocking input runtime: evdev readers → gesture engine → uinput.
//!
//! One thread polls each grabbed node (non-blocking, so shutdown is prompt),
//! one engine thread owns the gesture state and its timers, and the uinput
//! writer owns its own thread. The daemon consumes [`RemoteEvent`]s over an
//! unbounded channel; the input side never sleeps on a transport.

use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use atvv_protocol::profile::{DeviceProfile, RemoteButton};
use evdev::{EventType, KeyCode};
use thiserror::Error;

use crate::action::{Action, ActionError, ActionMap, Trigger};
use crate::device::{SkippedDevice, discover};
use crate::gesture::{Gesture, GestureEngine, Timing};
use crate::keymap::{KeyMap, KeyMapError, KeyResolver};
use crate::vkbd::VirtualKeyboard;

/// Event delivered to the daemon / CLI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteEvent {
    /// Raw voice-key edge; the ATVV session owns debouncing.
    Voice { pressed: bool },
    /// A recognized gesture, the action it resolved to, and any dispatch error.
    Button {
        button: RemoteButton,
        gesture: Gesture,
        action: Option<Action>,
        error: Option<String>,
    },
    /// Optional hardware degraded; the runtime keeps running.
    Degraded { message: String },
    /// A grabbed node could not be read any more.
    DeviceLost { path: PathBuf, error: String },
}

#[derive(Debug, Error)]
pub enum InputError {
    #[error("profile {profile}: {error}")]
    Profile { profile: String, error: KeyMapError },
    #[error("action map: {0}")]
    Action(#[from] ActionError),
    #[error("no evdev node matches profile {0}")]
    NoDevice(String),
    #[error("no matching evdev node could be grabbed")]
    NotGrabbable,
    #[error("cannot start the input threads: {0}")]
    Thread(#[from] io::Error),
}

#[derive(Debug, Clone, Copy)]
enum RawEdge {
    Voice { pressed: bool },
    Button { button: RemoteButton, pressed: bool },
}

struct Dispatcher {
    map: ActionMap,
    resolver: KeyResolver,
    vkbd: Option<VirtualKeyboard>,
    events: Sender<RemoteEvent>,
}

impl Dispatcher {
    fn voice(&self, pressed: bool) {
        let _ = self.events.send(RemoteEvent::Voice { pressed });
    }

    fn dispatch(&self, gestures: Vec<Gesture>) {
        for gesture in gestures {
            let Some(trigger) = trigger_for(gesture) else {
                continue;
            };
            let button = gesture.button();
            let action = self.map.resolve(button, trigger).cloned();
            let mut error = None;
            if let Some(action) = &action {
                match action {
                    Action::Key { .. } | Action::Combo { .. } => {
                        match self.resolver.program(action) {
                            Ok(program) => match &self.vkbd {
                                Some(keyboard) => {
                                    if let Err(dispatch_error) = keyboard.tap(&program) {
                                        error = Some(dispatch_error.to_string());
                                    }
                                }
                                None => {
                                    error = Some(
                                        "virtual keyboard unavailable; key mapping degraded"
                                            .to_string(),
                                    );
                                }
                            },
                            Err(resolve_error) => error = Some(resolve_error.to_string()),
                        }
                    }
                    Action::None | Action::Exec { .. } | Action::Dictation(_) => {}
                }
            }
            let _ = self.events.send(RemoteEvent::Button {
                button,
                gesture,
                action,
                error,
            });
        }
    }
}

fn trigger_for(gesture: Gesture) -> Option<Trigger> {
    match gesture {
        Gesture::SingleClick(_) | Gesture::RepeatTick(_) => Some(Trigger::Single),
        Gesture::DoubleClick(_) => Some(Trigger::Double),
        Gesture::LongPress(_) => Some(Trigger::Long),
        Gesture::Release(_) => None,
    }
}

/// A running input pipeline for one profile.
pub struct InputRuntime {
    grabbed: Vec<PathBuf>,
    skipped: Vec<SkippedDevice>,
    degraded: Vec<String>,
    events: Receiver<RemoteEvent>,
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
    vkbd: Option<VirtualKeyboard>,
}

impl InputRuntime {
    /// Discover, grab, and start reading the profile's nodes.
    pub fn start(profile: DeviceProfile, map: ActionMap) -> Result<Self, InputError> {
        let keymap = KeyMap::from_profile(&profile).map_err(|error| InputError::Profile {
            profile: profile.id.to_string(),
            error,
        })?;
        let discovery = discover(&profile, &keymap);
        if discovery.matched.is_empty() {
            return Err(InputError::NoDevice(profile.id.to_string()));
        }

        let resolver = KeyResolver;
        let mut registration: BTreeSet<KeyCode> = keymap.codes().collect();
        registration.extend(resolver.codes_for(&map)?);
        for matched in &discovery.matched {
            registration.extend(matched.supported_keys.iter().copied());
        }

        let mut degraded = Vec::new();
        let mut grabbed_devices = Vec::new();
        for mut matched in discovery.matched {
            match matched.device.grab() {
                Ok(()) => grabbed_devices.push(matched),
                Err(error) => degraded.push(format!(
                    "cannot grab {} ({}) : {error}",
                    matched.path.display(),
                    matched.name
                )),
            }
        }
        if grabbed_devices.is_empty() {
            return Err(InputError::NotGrabbable);
        }

        let vkbd = match VirtualKeyboard::open(registration) {
            Ok(keyboard) => Some(keyboard),
            Err(error) => {
                degraded.push(format!("uinput unavailable; key mapping degraded: {error}"));
                None
            }
        };

        let stop = Arc::new(AtomicBool::new(false));
        let (raw_tx, raw_rx) = mpsc::channel::<RawEdge>();
        let (event_tx, events) = mpsc::channel::<RemoteEvent>();
        let mut threads = Vec::new();
        let mut grabbed = Vec::new();

        for matched in grabbed_devices {
            let path = matched.path.clone();
            let error_path = path.clone();
            let name = matched.name.clone();
            if let Err(error) = matched.device.set_nonblocking(true) {
                degraded.push(format!(
                    "cannot poll {} non-blocking: {error}",
                    path.display()
                ));
            }
            let tx = raw_tx.clone();
            let event_tx = event_tx.clone();
            let vkbd = vkbd.clone();
            let stop = Arc::clone(&stop);
            let keymap = keymap.clone();
            let reader = thread::Builder::new()
                .name(format!("atvv-evdev-{name}"))
                .spawn(move || {
                    let mut device = matched.device;
                    while !stop.load(Ordering::Relaxed) {
                        match device.fetch_events() {
                            Ok(events) => {
                                for event in events {
                                    if event.event_type() != EventType::KEY {
                                        continue;
                                    }
                                    let value = event.value();
                                    if !matches!(value, 0..=2) {
                                        continue;
                                    }
                                    let code = KeyCode::new(event.code());
                                    match keymap.button(code) {
                                        Some(RemoteButton::Voice) => {
                                            let _ = tx.send(RawEdge::Voice {
                                                pressed: value != 0,
                                            });
                                        }
                                        Some(button) => {
                                            let _ = tx.send(RawEdge::Button {
                                                button,
                                                pressed: value != 0,
                                            });
                                        }
                                        None => {
                                            // Re-emit keys the profile does not map,
                                            // otherwise grabbing them loses input.
                                            if let Some(keyboard) = &vkbd {
                                                let _ = keyboard.emit_raw(code, value);
                                            }
                                        }
                                    }
                                }
                            }
                            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                                thread::sleep(Duration::from_millis(10));
                            }
                            Err(error) => {
                                let _ = event_tx.send(RemoteEvent::DeviceLost {
                                    path: path.clone(),
                                    error: error.to_string(),
                                });
                                break;
                            }
                        }
                    }
                });
            match reader {
                Ok(handle) => {
                    threads.push(handle);
                    grabbed.push(error_path);
                }
                Err(error) => degraded.push(format!(
                    "cannot start a reader for {}: {error}",
                    error_path.display()
                )),
            }
        }
        if grabbed.is_empty() {
            return Err(InputError::NotGrabbable);
        }
        drop(raw_tx);

        let engine_stop = Arc::clone(&stop);
        let engine_vkbd = vkbd.clone();
        let engine_map = map.clone();
        let timing = profile.gesture_timing;
        let engine_event_tx = event_tx.clone();
        let engine = thread::Builder::new()
            .name("atvv-gesture".into())
            .spawn(move || {
                let mut engine = GestureEngine::new(Timing::from(timing));
                for button in RemoteButton::LOGICAL {
                    engine.set_policy(button, engine_map.policy(button, &timing));
                }
                let dispatcher = Dispatcher {
                    map: engine_map,
                    resolver: KeyResolver,
                    vkbd: engine_vkbd,
                    events: engine_event_tx,
                };
                while !engine_stop.load(Ordering::Relaxed) {
                    let now = Instant::now();
                    let timeout = engine
                        .next_deadline()
                        .map(|deadline| deadline.saturating_duration_since(now))
                        .unwrap_or(Duration::from_millis(250));
                    match raw_rx.recv_timeout(timeout) {
                        Ok(RawEdge::Voice { pressed }) => dispatcher.voice(pressed),
                        Ok(RawEdge::Button { button, pressed }) => {
                            let now = Instant::now();
                            let gestures = if pressed {
                                engine.press(button, now)
                            } else {
                                engine.release(button, now)
                            };
                            dispatcher.dispatch(gestures);
                        }
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                    dispatcher.dispatch(engine.advance(Instant::now()));
                }
            });
        let engine = match engine {
            Ok(handle) => handle,
            Err(error) => {
                stop.store(true, Ordering::Relaxed);
                return Err(InputError::Thread(error));
            }
        };
        threads.push(engine);
        drop(event_tx);

        Ok(Self {
            grabbed,
            skipped: discovery.skipped,
            degraded,
            events,
            stop,
            threads,
            vkbd,
        })
    }

    /// `EVIOCGRAB`-ed device paths.
    #[must_use]
    pub fn grabbed(&self) -> &[PathBuf] {
        &self.grabbed
    }

    /// Profile-matching nodes rejected by the safety filter.
    #[must_use]
    pub fn skipped(&self) -> &[SkippedDevice] {
        &self.skipped
    }

    /// Non-fatal degradation messages collected at startup.
    #[must_use]
    pub fn degraded(&self) -> &[String] {
        &self.degraded
    }

    /// `/dev/input/eventN` of the virtual keyboard, when uinput is available.
    #[must_use]
    pub fn keyboard_path(&self) -> Option<&Path> {
        self.vkbd.as_ref().and_then(VirtualKeyboard::node_path)
    }

    /// Whether uinput key emission is available.
    #[must_use]
    pub fn keyboard_available(&self) -> bool {
        self.vkbd
            .as_ref()
            .is_some_and(VirtualKeyboard::is_available)
    }

    /// Receive the next event, waiting up to `timeout`.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<RemoteEvent, RecvTimeoutError> {
        self.events.recv_timeout(timeout)
    }

    /// Non-blocking receive.
    pub fn try_recv(&self) -> Result<RemoteEvent, mpsc::TryRecvError> {
        self.events.try_recv()
    }
}

impl Drop for InputRuntime {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for handle in self.threads.drain(..) {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_degrades_without_uinput_instead_of_panicking() {
        let (tx, rx) = mpsc::channel();
        let dispatcher = Dispatcher {
            map: ActionMap::defaults(),
            resolver: KeyResolver,
            vkbd: None,
            events: tx,
        };
        dispatcher.dispatch(vec![Gesture::SingleClick(RemoteButton::Ok)]);
        match rx.recv().expect("button event") {
            RemoteEvent::Button {
                button,
                action,
                error,
                ..
            } => {
                assert_eq!(button, RemoteButton::Ok);
                assert!(action.is_some());
                assert!(
                    error.expect("degraded error").contains("degraded"),
                    "error should describe the degraded mapping"
                );
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[test]
    fn gestures_map_to_their_slots() {
        assert_eq!(
            trigger_for(Gesture::SingleClick(RemoteButton::Ok)),
            Some(Trigger::Single)
        );
        assert_eq!(
            trigger_for(Gesture::RepeatTick(RemoteButton::Up)),
            Some(Trigger::Single)
        );
        assert_eq!(
            trigger_for(Gesture::DoubleClick(RemoteButton::Ok)),
            Some(Trigger::Double)
        );
        assert_eq!(
            trigger_for(Gesture::LongPress(RemoteButton::Menu)),
            Some(Trigger::Long)
        );
        assert_eq!(trigger_for(Gesture::Release(RemoteButton::Ok)), None);
    }
}
