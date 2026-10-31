// SPDX-License-Identifier: GPL-3.0-or-later

//! Passive hardware facts from the connected remote.
//!
//! Battery comes from BlueZ's cached `org.bluez.Battery1` property. The
//! revisions, serial, model, and manufacturer come from the GATT Device
//! Information Service (`0x180A`). Every field is optional: a device may omit
//! DIS or refuse an encrypted read, and that must not fail the ATVV link.

use std::time::Duration;

use bluer::Device;
use tokio::time::timeout;

/// Device Information Service.
const DIS_SERVICE: &str = "0000180a-0000-1000-8000-00805f9b34fb";
const MANUFACTURER_NAME: &str = "00002a29-0000-1000-8000-00805f9b34fb";
const MODEL_NUMBER: &str = "00002a24-0000-1000-8000-00805f9b34fb";
const SERIAL_NUMBER: &str = "00002a25-0000-1000-8000-00805f9b34fb";
const HARDWARE_REVISION: &str = "00002a27-0000-1000-8000-00805f9b34fb";
const FIRMWARE_REVISION: &str = "00002a26-0000-1000-8000-00805f9b34fb";
const SOFTWARE_REVISION: &str = "00002a28-0000-1000-8000-00805f9b34fb";

/// One characteristic read must not stall the link.
const READ_TIMEOUT: Duration = Duration::from_secs(2);

/// Hardware facts of one remote. All fields are optional.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HardwareInfo {
    /// Battery percentage reported by BlueZ (`org.bluez.Battery1`).
    pub battery_percent: Option<u8>,
    /// Device Information Service `0x2A29`.
    pub manufacturer: Option<String>,
    /// Device Information Service `0x2A24`.
    pub model: Option<String>,
    /// Device Information Service `0x2A25`.
    pub serial_number: Option<String>,
    /// Device Information Service `0x2A27`.
    pub hardware_revision: Option<String>,
    /// Device Information Service `0x2A26`.
    pub firmware_revision: Option<String>,
    /// Device Information Service `0x2A28`.
    pub software_revision: Option<String>,
}

/// Read DIS and the cached battery. Individual failures leave fields empty.
pub(crate) async fn read(device: &Device) -> HardwareInfo {
    let mut info = HardwareInfo {
        battery_percent: read_battery(device).await,
        ..HardwareInfo::default()
    };
    let Ok(services) = device.services().await else {
        return info;
    };
    for service in services {
        let Ok(uuid) = service.uuid().await else {
            continue;
        };
        if uuid != crate::atvv_uuid(DIS_SERVICE) {
            continue;
        }
        let Ok(characteristics) = service.characteristics().await else {
            continue;
        };
        for characteristic in characteristics {
            let Ok(uuid) = characteristic.uuid().await else {
                continue;
            };
            let Some(value) = read_value(&characteristic).await else {
                continue;
            };
            let value = text(&value);
            if value.is_empty() {
                continue;
            }
            if uuid == crate::atvv_uuid(MANUFACTURER_NAME) {
                info.manufacturer = Some(value);
            } else if uuid == crate::atvv_uuid(MODEL_NUMBER) {
                info.model = Some(value);
            } else if uuid == crate::atvv_uuid(SERIAL_NUMBER) {
                info.serial_number = Some(value);
            } else if uuid == crate::atvv_uuid(HARDWARE_REVISION) {
                info.hardware_revision = Some(value);
            } else if uuid == crate::atvv_uuid(FIRMWARE_REVISION) {
                info.firmware_revision = Some(value);
            } else if uuid == crate::atvv_uuid(SOFTWARE_REVISION) {
                info.software_revision = Some(value);
            }
        }
        break;
    }
    info
}

/// Refresh only the battery. Errors and timeouts keep the previous value.
pub(crate) async fn read_battery(device: &Device) -> Option<u8> {
    let result = timeout(READ_TIMEOUT, device.battery_percentage())
        .await
        .ok()?;
    result.ok().flatten()
}

async fn read_value(characteristic: &bluer::gatt::remote::Characteristic) -> Option<Vec<u8>> {
    timeout(READ_TIMEOUT, characteristic.read())
        .await
        .ok()?
        .ok()
}

/// Decode a DIS string: trim trailing NUL padding and surrounding whitespace.
fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_end_matches('\0')
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_padded_dis_strings() {
        assert_eq!(text(b"2671\0\0"), "2671");
        assert_eq!(text(b" V2.0 \0"), "V2.0");
        assert_eq!(text(b""), "");
        assert_eq!(text(&[0xff, 0x00]), "\u{fffd}");
    }
}
