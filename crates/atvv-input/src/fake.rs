// SPDX-License-Identifier: GPL-3.0-or-later

//! Fake ATVV remote for hardware-free input tests.
//!
//! The fixture is a real uinput device, so discovery, grabbing, and reading
//! exercise the same code path as a BlueZ HOG node. Its name and ids are
//! unique and it is not in [`atvv_protocol::PROFILES`], so it can never be
//! matched by a production profile.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use atvv_protocol::caps::{Codec, ProtocolVersion};
use atvv_protocol::profile::{
    ButtonKey, DeviceProfile, GestureTiming, KeepaliveVariant, MicCloseVariant, MicOpenVariant,
    NibbleOrder, RC003, RemoteButton,
};
use evdevil::event::{Key, KeyEvent, KeyState};
use evdevil::uinput::UinputDevice;
use evdevil::{Bus, Evdev, InputId};
use thiserror::Error;

use crate::keymap::{KeyMapError, resolve_key};
use crate::vkbd::wait_for_node;

/// Test-only profile. Unique name and ids; never part of production profiles.
pub const FAKE_REMOTE: DeviceProfile = DeviceProfile {
    id: "test-fake-remote",
    display_name: "ATVV fake remote (test fixture)",
    bluetooth_names: &["atvv-fake-remote"],
    vendor_id: 0x1209,
    product_id: 0x0002,
    model: "fake",
    protocol: ProtocolVersion::V1_0,
    codecs: &[Codec::Adpcm16k],
    frame_size: 120,
    nibble_order: NibbleOrder::HighFirst,
    mic_open: MicOpenVariant::Short,
    mic_close: MicCloseVariant::Bare,
    keepalive: KeepaliveVariant::Bare,
    direct_audio_start: true,
    stop_echo: true,
    post_stop_discard_ms: 300,
    sample_rate: 16_000,
    gain_db: 0,
    button_keys: RC003.button_keys,
    // Short windows keep the smoke test quick; timing correctness is
    // exercised with RC003 values in the gesture unit tests.
    gesture_timing: GestureTiming {
        double_click_ms: 120,
        long_press_ms: 200,
        repeat_start_ms: 150,
        stable_release_ms: 100,
        suppression_ms: 30,
        repeat_interval_ms: 50,
        back_repeat_interval_ms: 30,
    },
};

#[derive(Debug, Error)]
pub enum FakeRemoteError {
    #[error(transparent)]
    KeyMap(#[from] KeyMapError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("uinput created the fake remote but no /dev/input node appeared")]
    NoNode,
}

fn wait_until_readable(path: &Path) -> std::io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        match Evdev::open(path) {
            Ok(_) => return Ok(()),
            Err(_) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(error) => return Err(error),
        }
    }
}

/// A uinput remote that emits the profile's key codes.
#[derive(Debug)]
pub struct FakeRemote {
    device: UinputDevice,
    path: PathBuf,
    codes: BTreeMap<RemoteButton, Key>,
}

impl FakeRemote {
    /// Create the fixture and register every key the profile maps.
    pub fn open() -> Result<Self, FakeRemoteError> {
        let mut keys = Vec::new();
        let mut codes = BTreeMap::new();
        for &ButtonKey { button, key } in FAKE_REMOTE.button_keys {
            let code = resolve_key(key).ok_or(KeyMapError::Unknown { button, key })?;
            keys.push(code);
            codes.entry(button).or_insert(code);
        }
        keys.sort_unstable();
        keys.dedup();

        let device = UinputDevice::builder()?
            .with_input_id(InputId::new(
                Bus::VIRTUAL,
                FAKE_REMOTE.vendor_id,
                FAKE_REMOTE.product_id,
                0x0001,
            ))?
            .with_keys(keys)?
            .build(FAKE_REMOTE.bluetooth_names[0])?;
        let path = wait_for_node(&device).ok_or(FakeRemoteError::NoNode)?;
        // UI_DEV_CREATE returns before udev/systemd-udevd has finished
        // making the event node readable; discovery opens nodes by path.
        wait_until_readable(&path)?;
        Ok(Self {
            device,
            path,
            codes,
        })
    }

    /// `/dev/input/eventN` of the fake remote.
    #[must_use]
    pub fn node_path(&self) -> &Path {
        &self.path
    }

    /// Emit one press.
    pub fn press(&mut self, button: RemoteButton) -> std::io::Result<()> {
        self.emit(button, KeyState::PRESSED)
    }

    /// Emit one release.
    pub fn release(&mut self, button: RemoteButton) -> std::io::Result<()> {
        self.emit(button, KeyState::RELEASED)
    }

    /// Press and release in two frames.
    pub fn tap(&mut self, button: RemoteButton) -> std::io::Result<()> {
        self.press(button)?;
        self.release(button)
    }

    fn emit(&mut self, button: RemoteButton, state: KeyState) -> std::io::Result<()> {
        let Some(code) = self.codes.get(&button) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("fake remote has no key for {button}"),
            ));
        };
        self.device
            .write_events(&[KeyEvent::new(*code, state).into()])
    }

    /// Forward a raw key code (for tests of unmapped keys).
    pub fn emit_code(&mut self, code: Key, value: i32) -> std::io::Result<()> {
        self.device
            .write_events(&[KeyEvent::new(code, KeyState::from_raw(value)).into()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atvv_protocol::profile::{PROFILES, find};

    #[test]
    fn fixture_is_not_a_production_profile() {
        assert!(find(FAKE_REMOTE.id).is_none());
        for profile in PROFILES {
            assert_ne!(profile.id, FAKE_REMOTE.id);
            assert_ne!(profile.vendor_id, FAKE_REMOTE.vendor_id);
            assert!(!profile.matches_name(FAKE_REMOTE.bluetooth_names[0]));
        }
    }

    #[test]
    fn fixture_keys_resolve() {
        for &ButtonKey { button, key } in FAKE_REMOTE.button_keys {
            assert!(resolve_key(key).is_some(), "{button}: {key}");
        }
        assert_eq!(FAKE_REMOTE.primary_key(RemoteButton::Voice), Some("KEY_F5"));
    }
}
