// SPDX-License-Identifier: GPL-3.0-or-later

//! evdev discovery for ATVV remotes.
//!
//! A profile match is either exact (`vendor_id` + `product_id`) or by the
//! profile's full device names. Pointer-like nodes (relative or absolute
//! axes, mouse/touch buttons) are never grabbed: an air-mouse or gyro node
//! must keep working.

use std::fmt;
use std::path::PathBuf;

use atvv_protocol::profile::DeviceProfile;
use evdev::{Device, KeyCode};

use crate::keymap::KeyMap;

/// How a node matched a profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchQuality {
    /// Exact vendor and product id.
    Exact,
    /// Full device-name match with different ids.
    Name,
}

/// Why a profile-matching node was not selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// Exposes pointer axes or pointer buttons.
    Pointer,
    /// Does not expose any key the profile maps.
    NoMappedKeys,
}

impl fmt::Display for SkipReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pointer => f.write_str("pointer node"),
            Self::NoMappedKeys => f.write_str("no profile keys"),
        }
    }
}

/// One node selected for grabbing.
#[derive(Debug)]
pub struct MatchedDevice {
    pub path: PathBuf,
    pub name: String,
    pub quality: MatchQuality,
    pub supported_keys: Vec<KeyCode>,
    pub device: Device,
}

/// One profile-matching node rejected by the safety filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedDevice {
    pub path: PathBuf,
    pub name: String,
    pub reason: SkipReason,
}

/// Result of one discovery pass.
#[derive(Debug, Default)]
pub struct Discovery {
    pub matched: Vec<MatchedDevice>,
    pub skipped: Vec<SkippedDevice>,
}

/// Match one node's identity against a profile. `Exact` IDs win; a full
/// device-name match is the fallback for firmware with changed ids.
#[must_use]
pub fn match_quality(
    profile: &DeviceProfile,
    vendor: u16,
    product: u16,
    name: &str,
) -> Option<MatchQuality> {
    if vendor == profile.vendor_id && product == profile.product_id {
        Some(MatchQuality::Exact)
    } else if profile.matches_name(name) {
        Some(MatchQuality::Name)
    } else {
        None
    }
}

fn pointer_buttons() -> [KeyCode; 5] {
    [
        KeyCode::BTN_LEFT,
        KeyCode::BTN_RIGHT,
        KeyCode::BTN_MIDDLE,
        KeyCode::BTN_TOUCH,
        KeyCode::BTN_STYLUS,
    ]
}

fn is_pointer(device: &Device) -> bool {
    let has_axes = device
        .supported_relative_axes()
        .is_some_and(|axes| axes.iter().next().is_some())
        || device
            .supported_absolute_axes()
            .is_some_and(|axes| axes.iter().next().is_some());
    if has_axes {
        return true;
    }
    let pointer = pointer_buttons();
    device
        .supported_keys()
        .is_some_and(|keys| pointer.iter().any(|button| keys.contains(*button)))
}

/// Enumerate evdev nodes and select the profile's keyboard/consumer nodes.
#[must_use]
pub fn discover(profile: &DeviceProfile, keymap: &KeyMap) -> Discovery {
    let mut discovery = Discovery::default();
    for (path, device) in evdev::enumerate() {
        let name = device.name().unwrap_or_default().to_string();
        let id = device.input_id();
        let Some(quality) = match_quality(profile, id.vendor(), id.product(), &name) else {
            continue;
        };
        if is_pointer(&device) {
            discovery.skipped.push(SkippedDevice {
                path,
                name,
                reason: SkipReason::Pointer,
            });
            continue;
        }
        let supported_keys: Vec<KeyCode> = device
            .supported_keys()
            .map(|keys| keys.iter().collect())
            .unwrap_or_default();
        if !supported_keys
            .iter()
            .any(|code| keymap.button(*code).is_some())
        {
            discovery.skipped.push(SkippedDevice {
                path,
                name,
                reason: SkipReason::NoMappedKeys,
            });
            continue;
        }
        discovery.matched.push(MatchedDevice {
            path,
            name,
            quality,
            supported_keys,
            device,
        });
    }
    discovery
}

#[cfg(test)]
mod tests {
    use super::*;
    use atvv_protocol::RC003;

    #[test]
    fn exact_ids_win() {
        assert_eq!(
            match_quality(&RC003, 0x2717, 0x32B8, "anything"),
            Some(MatchQuality::Exact)
        );
    }

    #[test]
    fn full_names_are_the_fallback() {
        assert_eq!(
            match_quality(&RC003, 0, 0, "Xiaomi Bluetooth Remote 2 Pro"),
            Some(MatchQuality::Name)
        );
        assert_eq!(
            match_quality(&RC003, 0, 0, "MI RC 2 Pro"),
            Some(MatchQuality::Name)
        );
    }

    #[test]
    fn no_bare_brand_or_partial_matches() {
        assert_eq!(match_quality(&RC003, 0, 0, "Xiaomi"), None);
        assert_eq!(match_quality(&RC003, 0, 0, "MI"), None);
        assert_eq!(
            match_quality(&RC003, 0, 0, "AT Translated Set 2 keyboard"),
            None
        );
        assert_eq!(match_quality(&RC003, 0x2717, 0x0000, "AT keyboard"), None);
    }
}
