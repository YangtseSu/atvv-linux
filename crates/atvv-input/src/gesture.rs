// SPDX-License-Identifier: GPL-3.0-or-later

//! Clock-free button gesture state machine.
//!
//! Ported behavior (not code) from the RC003 field reports: the remote emits
//! spurious release/press cycles roughly 449 ms apart while a key is held.
//! A release therefore moves the button to a pending state and only resolves
//! after the stable-release latch; a press inside the latch cancels it and
//! restores the original press time so long-press timing stays continuous.
//!
//! The caller owns time: feed [`GestureEngine::press`] / [`GestureEngine::release`]
//! with the observed instant and call [`GestureEngine::advance`] at
//! [`GestureEngine::next_deadline`] to resolve timers. Emitted gestures are
//! deterministic for a given event sequence and instants.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use atvv_protocol::profile::{GestureTiming, RemoteButton};

use crate::action::Trigger;

/// Timing windows, in durations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    pub double_click: Duration,
    pub long_press: Duration,
    pub repeat_start: Duration,
    pub stable_release: Duration,
    pub suppression: Duration,
}

impl From<GestureTiming> for Timing {
    fn from(value: GestureTiming) -> Self {
        Self {
            double_click: Duration::from_millis(u64::from(value.double_click_ms)),
            long_press: Duration::from_millis(u64::from(value.long_press_ms)),
            repeat_start: Duration::from_millis(u64::from(value.repeat_start_ms)),
            stable_release: Duration::from_millis(u64::from(value.stable_release_ms)),
            suppression: Duration::from_millis(u64::from(value.suppression_ms)),
        }
    }
}

/// Per-button behavior derived from the action map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Policy {
    pub double_click: bool,
    pub long_press: bool,
    /// Auto-repeat interval once the repeat has started.
    pub repeat: Option<Duration>,
}

/// Recognized gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    SingleClick(RemoteButton),
    DoubleClick(RemoteButton),
    LongPress(RemoteButton),
    RepeatTick(RemoteButton),
    Release(RemoteButton),
}

impl Gesture {
    /// The button this gesture belongs to.
    #[must_use]
    pub const fn button(self) -> RemoteButton {
        match self {
            Self::SingleClick(button)
            | Self::DoubleClick(button)
            | Self::LongPress(button)
            | Self::RepeatTick(button)
            | Self::Release(button) => button,
        }
    }

    /// The action slot a recognized gesture resolves to.
    ///
    /// `Release` carries no action; repeat ticks use the single-click slot.
    #[must_use]
    pub const fn trigger(self) -> Option<Trigger> {
        match self {
            Self::SingleClick(_) | Self::RepeatTick(_) => Some(Trigger::Single),
            Self::DoubleClick(_) => Some(Trigger::Double),
            Self::LongPress(_) => Some(Trigger::Long),
            Self::Release(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum State {
    /// Pressed, awaiting long-press, repeat ticks, or release.
    Pressed {
        at: Instant,
        long_fired: bool,
        click_on_press: bool,
    },
    /// Released; the verdict waits behind the stable-release latch.
    PendingRelease {
        released_at: Instant,
        pressed_at: Instant,
        long_fired: bool,
        click_on_press: bool,
    },
    /// Resolved click; waiting for a second press.
    AwaitingSecond { at: Instant },
}

#[derive(Debug, Clone, Copy)]
struct RepeatArm {
    button: RemoteButton,
    next: Instant,
    interval: Duration,
}

/// Gesture recognizer for every logical button.
#[derive(Debug, Clone)]
pub struct GestureEngine {
    timing: Timing,
    policies: BTreeMap<RemoteButton, Policy>,
    states: BTreeMap<RemoteButton, State>,
    suppress_until: BTreeMap<RemoteButton, Instant>,
    repeat: Option<RepeatArm>,
}

impl GestureEngine {
    #[must_use]
    pub fn new(timing: Timing) -> Self {
        Self {
            timing,
            policies: BTreeMap::new(),
            states: BTreeMap::new(),
            suppress_until: BTreeMap::new(),
            repeat: None,
        }
    }

    #[must_use]
    pub fn timing(&self) -> Timing {
        self.timing
    }

    pub fn set_policy(&mut self, button: RemoteButton, policy: Policy) {
        self.policies.insert(button, policy);
    }

    #[must_use]
    pub fn policy(&self, button: RemoteButton) -> Policy {
        self.policies.get(&button).copied().unwrap_or_default()
    }

    fn suppress(&mut self, button: RemoteButton, now: Instant) {
        self.suppress_until
            .insert(button, now + self.timing.suppression);
    }

    fn disarm_repeat(&mut self, button: RemoteButton) {
        if self.repeat.is_some_and(|arm| arm.button == button) {
            self.repeat = None;
        }
    }

    /// Button pressed at `now`.
    pub fn press(&mut self, button: RemoteButton, now: Instant) -> Vec<Gesture> {
        if self
            .suppress_until
            .get(&button)
            .is_some_and(|until| now < *until)
        {
            return Vec::new();
        }
        let policy = self.policy(button);
        match self.states.get(&button).copied() {
            Some(State::PendingRelease {
                released_at,
                pressed_at,
                long_fired,
                click_on_press,
            }) if self.timing.stable_release > Duration::ZERO
                && now.saturating_duration_since(released_at) < self.timing.stable_release =>
            {
                // Spurious release while the key is still held.
                self.states.insert(
                    button,
                    State::Pressed {
                        at: pressed_at,
                        long_fired,
                        click_on_press,
                    },
                );
                if click_on_press {
                    if let Some(interval) = policy.repeat {
                        self.repeat = Some(RepeatArm {
                            button,
                            next: now + interval,
                            interval,
                        });
                    }
                }
                Vec::new()
            }
            Some(State::AwaitingSecond { at }) => {
                if policy.double_click
                    && now.saturating_duration_since(at) <= self.timing.double_click
                {
                    self.states.remove(&button);
                    self.disarm_repeat(button);
                    self.suppress(button, now);
                    vec![Gesture::DoubleClick(button)]
                } else {
                    let mut out = vec![Gesture::SingleClick(button)];
                    out.extend(self.begin_press(button, now, policy));
                    out
                }
            }
            // Duplicate press without a release: keep the original state.
            Some(State::Pressed { .. }) => Vec::new(),
            None | Some(State::PendingRelease { .. }) => self.begin_press(button, now, policy),
        }
    }

    fn begin_press(&mut self, button: RemoteButton, now: Instant, policy: Policy) -> Vec<Gesture> {
        let click_on_press = policy.repeat.is_some() && !policy.double_click && !policy.long_press;
        self.states.insert(
            button,
            State::Pressed {
                at: now,
                long_fired: false,
                click_on_press,
            },
        );
        if click_on_press {
            if let Some(interval) = policy.repeat {
                self.repeat = Some(RepeatArm {
                    button,
                    next: now + self.timing.repeat_start,
                    interval,
                });
            }
            vec![Gesture::SingleClick(button)]
        } else {
            Vec::new()
        }
    }

    /// Button released at `now`.
    pub fn release(&mut self, button: RemoteButton, now: Instant) -> Vec<Gesture> {
        self.disarm_repeat(button);
        let policy = self.policy(button);
        match self.states.get(&button).copied() {
            Some(State::Pressed {
                at,
                long_fired,
                click_on_press,
            }) => {
                if self.timing.stable_release > Duration::ZERO {
                    self.states.insert(
                        button,
                        State::PendingRelease {
                            released_at: now,
                            pressed_at: at,
                            long_fired,
                            click_on_press,
                        },
                    );
                    Vec::new()
                } else if long_fired {
                    self.states.remove(&button);
                    vec![Gesture::Release(button)]
                } else {
                    let held = now.saturating_duration_since(at);
                    self.states.remove(&button);
                    if policy.long_press && held >= self.timing.long_press {
                        vec![Gesture::Release(button), Gesture::LongPress(button)]
                    } else if policy.double_click {
                        self.states
                            .insert(button, State::AwaitingSecond { at: now });
                        vec![Gesture::Release(button)]
                    } else {
                        self.suppress(button, now);
                        vec![Gesture::Release(button), Gesture::SingleClick(button)]
                    }
                }
            }
            // Double release or a release without a press: pass through.
            _ => vec![Gesture::Release(button)],
        }
    }

    /// Resolve every timer due at `now`.
    pub fn advance(&mut self, now: Instant) -> Vec<Gesture> {
        let mut out = Vec::new();

        if let Some(arm) = self.repeat {
            let mut next = arm.next;
            let mut ticks = 0;
            while next <= now && ticks < 16 {
                if !matches!(self.states.get(&arm.button), Some(State::Pressed { .. })) {
                    break;
                }
                out.push(Gesture::RepeatTick(arm.button));
                next += arm.interval;
                ticks += 1;
            }
            if matches!(self.states.get(&arm.button), Some(State::Pressed { .. })) {
                self.repeat = Some(RepeatArm { next, ..arm });
            } else {
                self.repeat = None;
            }
        }

        let buttons: Vec<RemoteButton> = self.states.keys().copied().collect();
        for button in buttons {
            let policy = self.policy(button);
            match self.states.get(&button).copied() {
                Some(State::Pressed {
                    at,
                    long_fired: false,
                    click_on_press,
                }) if policy.long_press
                    && now.saturating_duration_since(at) >= self.timing.long_press =>
                {
                    self.states.insert(
                        button,
                        State::Pressed {
                            at,
                            long_fired: true,
                            click_on_press,
                        },
                    );
                    self.disarm_repeat(button);
                    out.push(Gesture::LongPress(button));
                }
                Some(State::PendingRelease {
                    released_at,
                    pressed_at,
                    long_fired,
                    click_on_press,
                }) if now.saturating_duration_since(released_at) >= self.timing.stable_release => {
                    out.push(Gesture::Release(button));
                    self.states.remove(&button);
                    if long_fired || click_on_press {
                        continue;
                    }
                    let held = released_at.saturating_duration_since(pressed_at);
                    if policy.long_press && held >= self.timing.long_press {
                        out.push(Gesture::LongPress(button));
                        continue;
                    }
                    if policy.double_click {
                        self.states
                            .insert(button, State::AwaitingSecond { at: now });
                        continue;
                    }
                    self.suppress(button, now);
                    out.push(Gesture::SingleClick(button));
                }
                Some(State::AwaitingSecond { at })
                    if now.saturating_duration_since(at) >= self.timing.double_click =>
                {
                    self.states.remove(&button);
                    self.suppress(button, now);
                    out.push(Gesture::SingleClick(button));
                }
                _ => {}
            }
        }

        out.sort_by_key(|gesture| gesture.button());
        out
    }

    /// Earliest instant at which [`Self::advance`] would do work.
    #[must_use]
    pub fn next_deadline(&self) -> Option<Instant> {
        let mut next: Option<Instant> = None;
        let mut consider = |instant: Instant| {
            if next.is_none_or(|current| instant < current) {
                next = Some(instant);
            }
        };
        if let Some(arm) = self.repeat {
            consider(arm.next);
        }
        for (button, state) in &self.states {
            let policy = self.policy(*button);
            match state {
                State::Pressed {
                    at,
                    long_fired: false,
                    ..
                } if policy.long_press => consider(*at + self.timing.long_press),
                State::PendingRelease { released_at, .. } => {
                    consider(*released_at + self.timing.stable_release);
                }
                State::AwaitingSecond { at } => consider(*at + self.timing.double_click),
                _ => {}
            }
        }
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::ActionMap;
    use atvv_protocol::RC003;

    fn at_ms(ms: u64) -> Instant {
        static BASE: std::sync::LazyLock<Instant> = std::sync::LazyLock::new(Instant::now);
        *BASE + Duration::from_millis(ms)
    }

    fn rc003() -> Timing {
        Timing::from(RC003.gesture_timing)
    }

    fn engine() -> GestureEngine {
        GestureEngine::new(rc003())
    }

    fn clicks(gestures: &[Gesture]) -> Vec<Gesture> {
        gestures
            .iter()
            .copied()
            .filter(|gesture| {
                matches!(
                    gesture,
                    Gesture::SingleClick(_) | Gesture::DoubleClick(_) | Gesture::LongPress(_)
                )
            })
            .collect()
    }

    #[test]
    fn single_click_resolves_after_the_latch() {
        let mut engine = engine();
        assert!(engine.press(RemoteButton::Ok, at_ms(0)).is_empty());
        assert!(engine.press(RemoteButton::Ok, at_ms(1)).is_empty());
        assert!(engine.release(RemoteButton::Ok, at_ms(100)).is_empty());
        assert!(engine.advance(at_ms(500)).is_empty());
        let out = engine.advance(at_ms(700));
        assert_eq!(clicks(&out), vec![Gesture::SingleClick(RemoteButton::Ok)]);
        assert!(out.contains(&Gesture::Release(RemoteButton::Ok)));
        assert_eq!(engine.next_deadline(), None);
    }

    #[test]
    fn spurious_release_cycle_keeps_long_press_timing() {
        let mut engine = engine();
        engine.set_policy(
            RemoteButton::Ok,
            Policy {
                long_press: true,
                ..Policy::default()
            },
        );
        assert!(engine.press(RemoteButton::Ok, at_ms(0)).is_empty());
        assert!(engine.release(RemoteButton::Ok, at_ms(449)).is_empty());
        assert!(engine.advance(at_ms(449)).is_empty());
        assert!(engine.press(RemoteButton::Ok, at_ms(451)).is_empty());
        let out = engine.advance(at_ms(560));
        assert_eq!(clicks(&out), vec![Gesture::LongPress(RemoteButton::Ok)]);
        // The release after a long press emits no click.
        assert!(engine.release(RemoteButton::Ok, at_ms(700)).is_empty());
        let out = engine.advance(at_ms(2000));
        assert!(out.contains(&Gesture::Release(RemoteButton::Ok)));
        assert!(clicks(&out).is_empty());
    }

    #[test]
    fn long_press_fires_at_550ms() {
        let mut engine = engine();
        engine.set_policy(
            RemoteButton::Ok,
            Policy {
                long_press: true,
                ..Policy::default()
            },
        );
        assert!(engine.press(RemoteButton::Ok, at_ms(0)).is_empty());
        assert!(engine.advance(at_ms(540)).is_empty());
        assert_eq!(
            clicks(&engine.advance(at_ms(550))),
            vec![Gesture::LongPress(RemoteButton::Ok)]
        );
    }

    #[test]
    fn double_click_window_opens_after_the_latch() {
        let mut engine = engine();
        engine.set_policy(
            RemoteButton::Ok,
            Policy {
                double_click: true,
                ..Policy::default()
            },
        );
        engine.press(RemoteButton::Ok, at_ms(0));
        engine.release(RemoteButton::Ok, at_ms(100));
        let out = engine.advance(at_ms(700));
        assert_eq!(out, vec![Gesture::Release(RemoteButton::Ok)]);
        assert_eq!(engine.next_deadline(), Some(at_ms(1000)));
        let out = engine.press(RemoteButton::Ok, at_ms(800));
        assert_eq!(clicks(&out), vec![Gesture::DoubleClick(RemoteButton::Ok)]);
    }

    #[test]
    fn second_press_outside_the_window_is_a_single() {
        let mut engine = engine();
        engine.set_policy(
            RemoteButton::Ok,
            Policy {
                double_click: true,
                ..Policy::default()
            },
        );
        engine.press(RemoteButton::Ok, at_ms(0));
        engine.release(RemoteButton::Ok, at_ms(100));
        engine.advance(at_ms(700));
        let out = engine.press(RemoteButton::Ok, at_ms(1200));
        assert_eq!(clicks(&out), vec![Gesture::SingleClick(RemoteButton::Ok)]);
        // The fresh press is live; the double window starts on release.
        assert_eq!(engine.next_deadline(), None);
    }

    #[test]
    fn double_window_timeout_resolves_a_single() {
        let mut engine = engine();
        engine.set_policy(
            RemoteButton::Ok,
            Policy {
                double_click: true,
                ..Policy::default()
            },
        );
        engine.press(RemoteButton::Ok, at_ms(0));
        engine.release(RemoteButton::Ok, at_ms(100));
        engine.advance(at_ms(700));
        let out = engine.advance(at_ms(1000));
        assert_eq!(clicks(&out), vec![Gesture::SingleClick(RemoteButton::Ok)]);
    }

    #[test]
    fn suppression_drops_phantom_presses() {
        let mut engine = engine();
        engine.press(RemoteButton::Ok, at_ms(0));
        engine.release(RemoteButton::Ok, at_ms(10));
        engine.advance(at_ms(700)); // click at 700, suppressed until 880
        assert!(engine.press(RemoteButton::Ok, at_ms(800)).is_empty());
        assert_eq!(engine.next_deadline(), None);
        assert!(engine.press(RemoteButton::Ok, at_ms(900)).is_empty());
        assert_eq!(engine.next_deadline(), None);
    }

    #[test]
    fn repeat_fires_on_press_then_ticks() {
        let mut engine = engine();
        engine.set_policy(
            RemoteButton::Up,
            Policy {
                repeat: Some(Duration::from_millis(100)),
                ..Policy::default()
            },
        );
        assert_eq!(
            engine.press(RemoteButton::Up, at_ms(0)),
            vec![Gesture::SingleClick(RemoteButton::Up)]
        );
        assert_eq!(engine.next_deadline(), Some(at_ms(350)));
        assert!(engine.advance(at_ms(340)).is_empty());
        let out = engine.advance(at_ms(560));
        assert_eq!(
            out.iter()
                .filter(|gesture| matches!(gesture, Gesture::RepeatTick(_)))
                .count(),
            3
        );
    }

    #[test]
    fn release_stops_repeat_and_resolves_through_the_latch() {
        let mut engine = engine();
        engine.set_policy(
            RemoteButton::Back,
            Policy {
                repeat: Some(Duration::from_millis(50)),
                ..Policy::default()
            },
        );
        engine.press(RemoteButton::Back, at_ms(0));
        engine.advance(at_ms(400));
        engine.release(RemoteButton::Back, at_ms(500));
        assert_eq!(engine.next_deadline(), Some(at_ms(1100)));
        let out = engine.advance(at_ms(1200));
        assert_eq!(out, vec![Gesture::Release(RemoteButton::Back)]);
    }

    #[test]
    fn long_press_disarms_repeat() {
        let mut engine = engine();
        engine.set_policy(
            RemoteButton::Up,
            Policy {
                long_press: true,
                repeat: Some(Duration::from_millis(50)),
                ..Policy::default()
            },
        );
        engine.press(RemoteButton::Up, at_ms(0));
        let out = engine.advance(at_ms(560));
        assert!(out.contains(&Gesture::LongPress(RemoteButton::Up)));
        let out = engine.advance(at_ms(900));
        assert!(
            !out.iter()
                .any(|gesture| matches!(gesture, Gesture::RepeatTick(_)))
        );
    }

    #[test]
    fn zero_latch_resolves_on_release() {
        let mut engine = GestureEngine::new(Timing {
            stable_release: Duration::ZERO,
            ..rc003()
        });
        assert!(engine.press(RemoteButton::Ok, at_ms(0)).is_empty());
        let out = engine.release(RemoteButton::Ok, at_ms(100));
        assert_eq!(
            out,
            vec![
                Gesture::Release(RemoteButton::Ok),
                Gesture::SingleClick(RemoteButton::Ok)
            ]
        );
    }

    #[test]
    fn next_deadline_tracks_all_timers() {
        let mut engine = engine();
        engine.set_policy(
            RemoteButton::Ok,
            Policy {
                long_press: true,
                ..Policy::default()
            },
        );
        assert_eq!(engine.next_deadline(), None);
        engine.press(RemoteButton::Ok, at_ms(0));
        assert_eq!(engine.next_deadline(), Some(at_ms(550)));
        engine.release(RemoteButton::Ok, at_ms(100));
        assert_eq!(engine.next_deadline(), Some(at_ms(700)));
        engine.advance(at_ms(700));
        assert_eq!(engine.next_deadline(), None);
    }

    #[test]
    fn map_policies_are_applied() {
        let map = ActionMap::defaults();
        let timing = RC003.gesture_timing;
        let mut engine = engine();
        for button in RemoteButton::LOGICAL {
            engine.set_policy(button, map.policy(button, &timing));
        }
        let out = engine.press(RemoteButton::Up, at_ms(0));
        assert_eq!(clicks(&out), vec![Gesture::SingleClick(RemoteButton::Up)]);
        assert!(engine.press(RemoteButton::Ok, at_ms(0)).is_empty());
    }
}
