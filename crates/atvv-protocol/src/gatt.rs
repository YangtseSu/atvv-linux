// SPDX-License-Identifier: GPL-3.0-or-later

//! ATVV GATT UUIDs.

/// ATVV service UUID.
pub const SERVICE_UUID: &str = "AB5E0001-5A21-4F05-BC7D-AF01F617B664";
/// Host-to-remote command characteristic.
pub const TRANSMIT_UUID: &str = "AB5E0002-5A21-4F05-BC7D-AF01F617B664";
/// Remote-to-host audio characteristic.
pub const AUDIO_UUID: &str = "AB5E0003-5A21-4F05-BC7D-AF01F617B664";
/// Remote-to-host control characteristic.
pub const CONTROL_UUID: &str = "AB5E0004-5A21-4F05-BC7D-AF01F617B664";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuids_share_the_atvv_base() {
        for uuid in [SERVICE_UUID, TRANSMIT_UUID, AUDIO_UUID, CONTROL_UUID] {
            assert!(uuid.starts_with("AB5E000"));
            assert!(uuid.ends_with("-5A21-4F05-BC7D-AF01F617B664"));
        }
    }
}
