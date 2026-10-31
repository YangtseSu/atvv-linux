// SPDX-License-Identifier: GPL-3.0-or-later

//! One connected ATVV GATT session over BlueZ.
//!
//! The remote is expected to be bonded already; pairing is a separate
//! operation ([`crate::pair`]). Notifications use `AcquireNotify` when BlueZ
//! supports it and fall back to `StartNotify` with the acquisition error
//! recorded in [`BluezLink::notification_mode`].

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use atvv_protocol::gatt;
use bluer::gatt::remote::{Characteristic, CharacteristicWriteRequest, Service};
use bluer::gatt::{CharacteristicFlags, CharacteristicReader, WriteOp};
use bluer::{Device, DeviceEvent, DeviceProperty, Session as BluezSession};
use futures_util::{Stream, StreamExt};
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};

use crate::error::{BluezError, Result};
use crate::{atvv_uuid, build_runtime, parse_address};

/// Default time to wait for a connection.
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Default time to wait for GATT service discovery.
pub const DEFAULT_SERVICES_TIMEOUT: Duration = Duration::from_secs(10);
/// Default time to wait for one command write.
pub const DEFAULT_WRITE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BluezConfig {
    pub address: String,
    /// Adapter name such as `hci0`; the default adapter is used when `None`.
    pub adapter: Option<String>,
    pub connect_timeout: Duration,
    pub services_timeout: Duration,
    pub write_timeout: Duration,
}

impl Default for BluezConfig {
    fn default() -> Self {
        Self {
            address: String::new(),
            adapter: None,
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            services_timeout: DEFAULT_SERVICES_TIMEOUT,
            write_timeout: DEFAULT_WRITE_TIMEOUT,
        }
    }
}

/// A notification or link event from the device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BluezEvent {
    Connected {
        address: String,
        name: String,
    },
    Disconnected {
        reason: String,
    },
    /// Notification on the ATVV control characteristic.
    Control(Vec<u8>),
    /// Notification on the ATVV audio characteristic.
    Audio(Vec<u8>),
}

/// How notifications were enabled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationMode {
    /// `AcquireNotify` on both characteristics.
    Acquire,
    /// `StartNotify` on at least one characteristic; the acquisition error is
    /// kept for diagnostics.
    Start { acquire_error: String },
}

impl NotificationMode {
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Acquire => "acquire-notify",
            Self::Start { .. } => "start-notify",
        }
    }
}

/// A connected BlueZ ATVV link.
pub struct BluezLink {
    runtime: Runtime,
    /// Keeps the BlueZ D-Bus session alive for the lifetime of the link.
    _session: BluezSession,
    device: Device,
    address: String,
    device_name: String,
    transmit: Characteristic,
    write_op: WriteOp,
    write_timeout: Duration,
    connect_timeout: Duration,
    services_timeout: Duration,
    sender: Sender<BluezEvent>,
    events: Receiver<BluezEvent>,
    tasks: Vec<JoinHandle<()>>,
    mode: NotificationMode,
    /// Incremented on every reconnect; stale notification tasks must not
    /// publish events for a previous generation.
    generation: Arc<AtomicU64>,
}

impl BluezLink {
    /// Connect to `config.address` and resolve the ATVV characteristics.
    pub fn connect(config: BluezConfig) -> Result<Self> {
        let runtime = build_runtime()?;
        let address_text = config.address.clone();
        let address = parse_address(&address_text)?;
        let (sender, events) = mpsc::channel();

        let adapter_name = config.adapter.clone();
        let connect_timeout = config.connect_timeout;
        let services_timeout = config.services_timeout;
        // Bound the whole operation, not only the BlueZ `Connect` call: a
        // pending `Connect` can make property reads wait behind it.
        let total = connect_timeout + services_timeout + Duration::from_secs(5);
        let (session, device, device_name, characteristics) = runtime.block_on(async {
            bounded(total, "connect", async {
                let session = BluezSession::new().await?;
                let adapter = select_adapter(&session, adapter_name.as_deref()).await?;
                if !adapter.is_powered().await? {
                    adapter.set_powered(true).await?;
                }
                let device = adapter.device(address)?;
                let device_name = device_name(&device, &address_text).await;
                wait_connected(&device, connect_timeout).await?;
                wait_services_resolved(&device, services_timeout).await?;
                let characteristics = resolve_atvv(&device).await?;
                Ok::<_, BluezError>((session, device, device_name, characteristics))
            })
            .await
        })?;

        let mut link = Self {
            runtime,
            _session: session,
            device,
            address: address_text,
            device_name,
            transmit: characteristics.transmit.clone(),
            write_op: characteristics.write_op,
            write_timeout: config.write_timeout,
            connect_timeout,
            services_timeout,
            sender,
            events,
            tasks: Vec::new(),
            mode: NotificationMode::Acquire,
            generation: Arc::new(AtomicU64::new(0)),
        };
        link.start_notifications(&characteristics)?;
        let monitor = link.spawn_device_monitor();
        link.tasks.push(monitor);
        link.publish(BluezEvent::Connected {
            address: link.address.clone(),
            name: link.device_name.clone(),
        });
        Ok(link)
    }

    #[must_use]
    pub fn address(&self) -> &str {
        &self.address
    }

    #[must_use]
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    #[must_use]
    pub fn notification_mode(&self) -> &NotificationMode {
        &self.mode
    }

    /// Return one event, or `None` when `limit` expires.
    pub fn poll(&mut self, limit: Duration) -> Result<Option<BluezEvent>> {
        match self.events.recv_timeout(limit) {
            Ok(event) => Ok(Some(event)),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => Err(BluezError::Closed),
        }
    }

    /// Write one ATVV command to the transmit characteristic.
    pub fn write_command(&mut self, bytes: &[u8]) -> Result<()> {
        let characteristic = self.transmit.clone();
        let request = CharacteristicWriteRequest {
            op_type: self.write_op,
            ..CharacteristicWriteRequest::default()
        };
        let limit = self.write_timeout;
        self.runtime.block_on(async move {
            match timeout(limit, characteristic.write_ext(bytes, &request)).await {
                Ok(result) => result.map_err(BluezError::from),
                Err(_) => Err(BluezError::Timeout {
                    operation: "write",
                    timeout: limit,
                }),
            }
        })
    }

    /// Reconnect after a link loss and re-acquire notifications.
    pub fn reconnect(&mut self) -> Result<()> {
        self.generation.fetch_add(1, Ordering::Relaxed);
        self.stop_tasks();
        let device = self.device.clone();
        let connect_timeout = self.connect_timeout;
        let services_timeout = self.services_timeout;
        let address_text = self.address.clone();
        let total = connect_timeout + services_timeout + Duration::from_secs(5);
        let (device_name, characteristics) = self.runtime.block_on(async {
            bounded(total, "connect", async {
                wait_connected(&device, connect_timeout).await?;
                wait_services_resolved(&device, services_timeout).await?;
                let characteristics = resolve_atvv(&device).await?;
                let device_name = device_name(&device, &address_text).await;
                Ok::<_, BluezError>((device_name, characteristics))
            })
            .await
        })?;

        self.device_name = device_name;
        self.transmit = characteristics.transmit.clone();
        self.write_op = characteristics.write_op;
        self.start_notifications(&characteristics)?;
        let monitor = self.spawn_device_monitor();
        self.tasks.push(monitor);
        self.publish(BluezEvent::Connected {
            address: self.address.clone(),
            name: self.device_name.clone(),
        });
        Ok(())
    }

    fn stop_tasks(&mut self) {
        for task in self.tasks.drain(..) {
            task.abort();
        }
        // Give BlueZ a moment to observe closed notification sockets before
        // acquiring them again. The timer must be created inside the runtime.
        self.runtime
            .block_on(async { sleep(Duration::from_millis(100)).await });
    }

    fn publish(&self, event: BluezEvent) {
        let _ = self.sender.send(event);
    }

    fn start_notifications(&mut self, characteristics: &AtvvCharacteristics) -> Result<()> {
        let audio = self
            .runtime
            .block_on(subscribe(characteristics.audio.clone()))?;
        let control = self
            .runtime
            .block_on(subscribe(characteristics.control.clone()))?;
        self.mode = merge_modes(&audio.1, &control.1);
        let audio_task = self.spawn_subscription(audio.0, Channel::Audio);
        let control_task = self.spawn_subscription(control.0, Channel::Control);
        self.tasks.push(audio_task);
        self.tasks.push(control_task);
        Ok(())
    }

    fn spawn_subscription(&self, subscription: Subscription, channel: Channel) -> JoinHandle<()> {
        let sender = self.sender.clone();
        let generation = Arc::clone(&self.generation);
        let epoch = generation.load(Ordering::Relaxed);
        self.runtime.spawn(async move {
            let reason = match subscription {
                Subscription::Acquired(reader) => {
                    pump_acquisition(reader, channel, &sender, &generation, epoch).await
                }
                Subscription::Stream(stream) => {
                    pump_stream(stream, channel, &sender, &generation, epoch).await
                }
            };
            if current(&generation, epoch) {
                let _ = sender.send(BluezEvent::Disconnected { reason });
            }
        })
    }

    fn spawn_device_monitor(&self) -> JoinHandle<()> {
        let device = self.device.clone();
        let sender = self.sender.clone();
        let generation = Arc::clone(&self.generation);
        let epoch = generation.load(Ordering::Relaxed);
        self.runtime.spawn(async move {
            let reason = match device.events().await {
                Ok(mut events) => {
                    let mut reason = "device event stream ended".to_string();
                    while let Some(event) = events.next().await {
                        if matches!(
                            event,
                            DeviceEvent::PropertyChanged(DeviceProperty::Connected(false))
                        ) {
                            // A signal queued before the current link may be
                            // delivered after a reconnect; trust the property.
                            if device.is_connected().await.unwrap_or(false) {
                                continue;
                            }
                            reason = "link lost".to_string();
                            break;
                        }
                    }
                    reason
                }
                Err(error) => format!("device events: {error}"),
            };
            if current(&generation, epoch) {
                let _ = sender.send(BluezEvent::Disconnected { reason });
            }
        })
    }
}

fn current(generation: &AtomicU64, epoch: u64) -> bool {
    generation.load(Ordering::Relaxed) == epoch
}

/// Resolved ATVV characteristics for one connection.
struct AtvvCharacteristics {
    transmit: Characteristic,
    audio: Characteristic,
    control: Characteristic,
    write_op: WriteOp,
}

enum Subscription {
    Acquired(CharacteristicReader),
    Stream(Pin<Box<dyn Stream<Item = Vec<u8>> + Send>>),
}

#[derive(Clone, Copy)]
enum Channel {
    Audio,
    Control,
}

impl Channel {
    fn name(self) -> &'static str {
        match self {
            Self::Audio => "audio",
            Self::Control => "control",
        }
    }

    fn event(self, bytes: Vec<u8>) -> BluezEvent {
        match self {
            Self::Audio => BluezEvent::Audio(bytes),
            Self::Control => BluezEvent::Control(bytes),
        }
    }
}

/// Pick the adapter by name, or fall back to BlueZ's default and then to the
/// first adapter.
pub(crate) async fn select_adapter(
    session: &BluezSession,
    name: Option<&str>,
) -> Result<bluer::Adapter> {
    if let Some(name) = name {
        return session.adapter(name).map_err(BluezError::from);
    }
    if let Ok(adapter) = session.default_adapter().await {
        return Ok(adapter);
    }
    let names = session.adapter_names().await?;
    let name = names.first().ok_or(BluezError::NoAdapter)?;
    session.adapter(name).map_err(BluezError::from)
}

pub(crate) async fn find_service(device: &Device) -> Result<Option<Service>> {
    let target = atvv_uuid(gatt::SERVICE_UUID);
    for service in device.services().await? {
        if service.uuid().await? == target {
            return Ok(Some(service));
        }
    }
    Ok(None)
}

pub(crate) async fn find_characteristic(
    service: &Service,
    uuid: &'static str,
) -> Result<Option<Characteristic>> {
    let target = atvv_uuid(uuid);
    for characteristic in service.characteristics().await? {
        if characteristic.uuid().await? == target {
            return Ok(Some(characteristic));
        }
    }
    Ok(None)
}

async fn resolve_atvv(device: &Device) -> Result<AtvvCharacteristics> {
    let service = find_service(device)
        .await?
        .ok_or(BluezError::MissingService {
            uuid: gatt::SERVICE_UUID,
        })?;
    let transmit = find_characteristic(&service, gatt::TRANSMIT_UUID)
        .await?
        .ok_or(BluezError::MissingCharacteristic {
            uuid: gatt::TRANSMIT_UUID,
        })?;
    let audio = find_characteristic(&service, gatt::AUDIO_UUID)
        .await?
        .ok_or(BluezError::MissingCharacteristic {
            uuid: gatt::AUDIO_UUID,
        })?;
    let control = find_characteristic(&service, gatt::CONTROL_UUID)
        .await?
        .ok_or(BluezError::MissingCharacteristic {
            uuid: gatt::CONTROL_UUID,
        })?;
    let flags = transmit.flags().await?;
    let write_op = write_operation(flags).ok_or(BluezError::NotWritable {
        uuid: gatt::TRANSMIT_UUID,
    })?;
    Ok(AtvvCharacteristics {
        transmit,
        audio,
        control,
        write_op,
    })
}

async fn device_name(device: &Device, fallback: &str) -> String {
    if let Ok(alias) = device.alias().await
        && !alias.is_empty()
    {
        return alias;
    }
    device
        .name()
        .await
        .ok()
        .flatten()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

async fn wait_connected(device: &Device, limit: Duration) -> Result<()> {
    if device.is_connected().await? {
        return Ok(());
    }
    let deadline = Instant::now() + limit;
    match timeout(limit, device.connect()).await {
        Ok(Ok(())) => return Ok(()),
        Ok(Err(error)) if connect_in_progress(&error) => {
            // Another client, or a previous call, owns the connection
            // attempt. Wait for the property to settle instead of failing the
            // whole reconnect.
        }
        Ok(Err(error)) => return Err(error.into()),
        Err(_) => {
            return Err(BluezError::Timeout {
                operation: "connect",
                timeout: limit,
            });
        }
    }
    while !device.is_connected().await? {
        if Instant::now() >= deadline {
            return Err(BluezError::Timeout {
                operation: "connect",
                timeout: limit,
            });
        }
        sleep(Duration::from_millis(50)).await;
    }
    Ok(())
}

/// BlueZ reports `InProgress` while another `Connect` call is pending. A
/// `AlreadyConnected` result is equally a successful outcome for the caller.
fn connect_in_progress(error: &bluer::Error) -> bool {
    matches!(
        error.kind,
        bluer::ErrorKind::InProgress | bluer::ErrorKind::AlreadyConnected
    )
}

/// Run one BlueZ operation under a deadline. A pending `Connect` can keep
/// property reads waiting, so the outer deadline is what actually bounds a
/// reconnect attempt.
async fn bounded<T>(
    limit: Duration,
    operation: &'static str,
    future: impl Future<Output = Result<T>>,
) -> Result<T> {
    match timeout(limit, future).await {
        Ok(result) => result,
        Err(_) => Err(BluezError::Timeout {
            operation,
            timeout: limit,
        }),
    }
}

async fn wait_services_resolved(device: &Device, limit: Duration) -> Result<()> {
    let started = Instant::now();
    loop {
        if device.is_services_resolved().await? {
            return Ok(());
        }
        if started.elapsed() >= limit {
            return Err(BluezError::Timeout {
                operation: "service discovery",
                timeout: limit,
            });
        }
        sleep(Duration::from_millis(50)).await;
    }
}

async fn subscribe(characteristic: Characteristic) -> Result<(Subscription, NotificationMode)> {
    match characteristic.notify_io().await {
        Ok(reader) => Ok((Subscription::Acquired(reader), NotificationMode::Acquire)),
        Err(acquire_error) => {
            let stream = characteristic.notify().await?;
            Ok((
                Subscription::Stream(Box::pin(stream)),
                NotificationMode::Start {
                    acquire_error: acquire_error.to_string(),
                },
            ))
        }
    }
}

fn merge_modes(audio: &NotificationMode, control: &NotificationMode) -> NotificationMode {
    match (audio, control) {
        (NotificationMode::Acquire, NotificationMode::Acquire) => NotificationMode::Acquire,
        (NotificationMode::Start { acquire_error }, _) => NotificationMode::Start {
            acquire_error: acquire_error.clone(),
        },
        (_, NotificationMode::Start { acquire_error }) => NotificationMode::Start {
            acquire_error: acquire_error.clone(),
        },
    }
}

async fn pump_acquisition(
    reader: CharacteristicReader,
    channel: Channel,
    sender: &Sender<BluezEvent>,
    generation: &AtomicU64,
    epoch: u64,
) -> String {
    let mut empty = 0_u8;
    loop {
        if !current(generation, epoch) {
            return "stale generation".to_string();
        }
        match reader.recv().await {
            Ok(bytes) if !bytes.is_empty() => {
                empty = 0;
                if sender.send(channel.event(bytes)).is_err() {
                    return "event receiver closed".to_string();
                }
            }
            // BlueZ may deliver a zero-length datagram when notifications are
            // (re)enabled. A closed socket reports EOF as an endless series of
            // empty reads, so bound them and yield to let aborts take effect.
            Ok(_) => {
                empty = empty.saturating_add(1);
                if empty >= 16 {
                    return format!("{} notifications reached EOF", channel.name());
                }
                sleep(Duration::from_millis(1)).await;
            }
            Err(error) => return format!("{} notification: {error}", channel.name()),
        }
    }
}

async fn pump_stream(
    mut stream: Pin<Box<dyn Stream<Item = Vec<u8>> + Send>>,
    channel: Channel,
    sender: &Sender<BluezEvent>,
    generation: &AtomicU64,
    epoch: u64,
) -> String {
    while let Some(bytes) = stream.next().await {
        if !current(generation, epoch) {
            return "stale generation".to_string();
        }
        if bytes.is_empty() {
            continue;
        }
        if sender.send(channel.event(bytes)).is_err() {
            return "event receiver closed".to_string();
        }
    }
    format!("{} notification stream ended", channel.name())
}

/// Select Write Command when the characteristic supports it, otherwise Write
/// Request, otherwise fail.
fn write_operation(flags: CharacteristicFlags) -> Option<WriteOp> {
    if flags.write_without_response {
        Some(WriteOp::Command)
    } else if flags.write {
        Some(WriteOp::Request)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_operation_follows_characteristic_flags() {
        let both = CharacteristicFlags {
            write_without_response: true,
            write: true,
            ..CharacteristicFlags::default()
        };
        assert_eq!(write_operation(both), Some(WriteOp::Command));

        let request_only = CharacteristicFlags {
            write: true,
            ..CharacteristicFlags::default()
        };
        assert_eq!(write_operation(request_only), Some(WriteOp::Request));

        assert_eq!(write_operation(CharacteristicFlags::default()), None);
    }

    #[test]
    fn in_progress_and_already_connected_are_retryable() {
        let error = |kind| bluer::Error {
            kind,
            message: String::new(),
        };
        assert!(connect_in_progress(&error(bluer::ErrorKind::InProgress)));
        assert!(connect_in_progress(&error(
            bluer::ErrorKind::AlreadyConnected
        )));
        assert!(!connect_in_progress(&error(bluer::ErrorKind::Failed)));
    }

    #[test]
    fn bounded_operation_times_out() {
        let runtime = build_runtime().unwrap();
        let result = runtime.block_on(bounded(Duration::from_millis(10), "test", async {
            std::future::pending::<Result<()>>().await
        }));
        assert!(matches!(
            result,
            Err(BluezError::Timeout {
                operation: "test",
                ..
            })
        ));
    }

    #[test]
    fn notification_modes_merge_to_the_weaker_one() {
        let acquire = NotificationMode::Acquire;
        let start = NotificationMode::Start {
            acquire_error: "in progress".to_string(),
        };
        assert_eq!(merge_modes(&acquire, &acquire), acquire);
        assert_eq!(
            merge_modes(&acquire, &start),
            NotificationMode::Start {
                acquire_error: "in progress".to_string()
            }
        );
        assert_eq!(
            merge_modes(&start, &acquire),
            NotificationMode::Start {
                acquire_error: "in progress".to_string()
            }
        );
    }
}
