// SPDX-License-Identifier: GPL-3.0-or-later

//! Read-only BlueZ diagnostics for `atvvctl doctor`.

use atvv_protocol::gatt;
use bluer::gatt::CharacteristicFlags;
use bluer::gatt::remote::Service;

use crate::error::{BluezError, Result};
use crate::link::{find_characteristic, find_service, select_adapter};
use crate::{BluezConfig, build_runtime, parse_address};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeReport {
    pub adapter: String,
    pub powered: bool,
    pub address: String,
    pub name: Option<String>,
    pub connected: bool,
    pub paired: bool,
    pub trusted: bool,
    pub services_resolved: bool,
    /// Present only when the device is connected and its services resolved.
    pub atvv: Option<AtvvInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtvvInfo {
    pub service_uuid: String,
    pub transmit: CharacteristicInfo,
    pub audio: CharacteristicInfo,
    pub control: CharacteristicInfo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacteristicInfo {
    pub uuid: String,
    pub flags: Vec<String>,
    pub mtu: usize,
}

/// Query BlueZ without connecting or enabling notifications.
pub fn probe(config: &BluezConfig) -> Result<ProbeReport> {
    let runtime = build_runtime()?;
    let address = parse_address(&config.address)?;
    let adapter_name = config.adapter.clone();
    runtime.block_on(async {
        let session = bluer::Session::new().await?;
        let adapter = select_adapter(&session, adapter_name.as_deref()).await?;
        let powered = adapter.is_powered().await?;
        let device = adapter.device(address)?;
        let name = match device.alias().await {
            Ok(alias) if !alias.is_empty() => Some(alias),
            _ => device
                .name()
                .await
                .ok()
                .flatten()
                .filter(|name| !name.is_empty()),
        };
        let connected = device.is_connected().await?;
        let paired = device.is_paired().await?;
        let trusted = device.is_trusted().await?;
        let services_resolved = device.is_services_resolved().await?;
        let atvv = if connected && services_resolved {
            match find_service(&device).await? {
                Some(service) => Some(atvv_info(&service).await?),
                None => None,
            }
        } else {
            None
        };
        Ok(ProbeReport {
            adapter: adapter.name().to_string(),
            powered,
            address: device.address().to_string(),
            name,
            connected,
            paired,
            trusted,
            services_resolved,
            atvv,
        })
    })
}

async fn atvv_info(service: &Service) -> Result<AtvvInfo> {
    Ok(AtvvInfo {
        service_uuid: service.uuid().await?.to_string(),
        transmit: characteristic_info(service, gatt::TRANSMIT_UUID).await?,
        audio: characteristic_info(service, gatt::AUDIO_UUID).await?,
        control: characteristic_info(service, gatt::CONTROL_UUID).await?,
    })
}

async fn characteristic_info(service: &Service, uuid: &'static str) -> Result<CharacteristicInfo> {
    let characteristic = find_characteristic(service, uuid)
        .await?
        .ok_or(BluezError::MissingCharacteristic { uuid })?;
    Ok(CharacteristicInfo {
        uuid: characteristic.uuid().await?.to_string(),
        flags: flag_names(characteristic.flags().await?),
        mtu: characteristic.mtu().await?,
    })
}

fn flag_names(flags: CharacteristicFlags) -> Vec<String> {
    let mut names = Vec::new();
    if flags.broadcast {
        names.push("broadcast");
    }
    if flags.read {
        names.push("read");
    }
    if flags.write_without_response {
        names.push("write-without-response");
    }
    if flags.write {
        names.push("write");
    }
    if flags.notify {
        names.push("notify");
    }
    if flags.indicate {
        names.push("indicate");
    }
    if flags.authenticated_signed_writes {
        names.push("authenticated-signed-writes");
    }
    if flags.extended_properties {
        names.push("extended-properties");
    }
    if flags.reliable_write {
        names.push("reliable-write");
    }
    if flags.writable_auxiliaries {
        names.push("writable-auxiliaries");
    }
    if flags.encrypt_read {
        names.push("encrypt-read");
    }
    if flags.encrypt_write {
        names.push("encrypt-write");
    }
    if flags.encrypt_authenticated_read {
        names.push("encrypt-authenticated-read");
    }
    if flags.encrypt_authenticated_write {
        names.push("encrypt-authenticated-write");
    }
    if flags.secure_read {
        names.push("secure-read");
    }
    if flags.secure_write {
        names.push("secure-write");
    }
    if flags.authorize {
        names.push("authorize");
    }
    names.into_iter().map(str::to_string).collect()
}
