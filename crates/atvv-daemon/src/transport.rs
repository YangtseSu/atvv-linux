// SPDX-License-Identifier: GPL-3.0-or-later

//! Transport boundary and the hardware-free mock device.
//!
//! The daemon only sees [`TransportEvent`]s and [`Transport`] writes. The mock
//! implements a device: it answers `GET_CAPS`, reacts to `MIC_OPEN` with a
//! direct `AUDIO_START`, emits ADPCM frames, and echoes `AUDIO_STOP` after a
//! `MIC_CLOSE`. The session logic stays in `atvv-protocol`.

use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use thiserror::Error;

use atvv_bluez::HardwareInfo;

/// A notification or link event from the device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportEvent {
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

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("transport is closed")]
    Closed,
    #[error("the device is not connected")]
    NotConnected,
    #[error("{0}")]
    Other(String),
}

/// One connected ATVV device.
///
/// Step 07 adds the BlueZ implementation of this trait; the daemon must not
/// change when that happens.
pub trait Transport: Send {
    /// Stable name for status reports: `mock`, `bluez`.
    fn kind(&self) -> &'static str;
    fn address(&self) -> &str;
    fn device_name(&self) -> &str;
    /// Whether the transport must connect before it can emit events. The
    /// daemon schedules an immediate reconnect for a lazy transport.
    fn needs_connect(&self) -> bool {
        false
    }
    /// Return one event, or `None` when `timeout` expires.
    fn poll(&mut self, timeout: Duration) -> Result<Option<TransportEvent>, TransportError>;
    /// Current passive hardware facts. The default is an empty set; the mock
    /// can carry canned values and BlueZ refreshes the battery.
    fn hardware(&mut self) -> Result<HardwareInfo, TransportError> {
        Ok(HardwareInfo::default())
    }
    /// Write to the ATVV command characteristic.
    fn write_command(&mut self, bytes: &[u8]) -> Result<(), TransportError>;
    /// Ask the transport to re-establish the link after a disconnect.
    fn reconnect(&mut self) -> Result<(), TransportError>;
}

/// RC003-shaped `CAPS_RESP`: v1.0, ADPCM 16 kHz, hold-to-talk, 120-byte frames.
pub const DEFAULT_CAPS: [u8; 7] = [0x0B, 0x01, 0x00, 0x02, 0x03, 0x00, 0x78];

/// Mock device behavior.
#[derive(Debug, Clone)]
pub struct MockConfig {
    pub address: String,
    pub name: String,
    pub caps: Vec<u8>,
    pub frame_size: usize,
    pub frame_interval: Duration,
    pub response_delay: Duration,
    pub audio_start_delay: Duration,
    pub echo_delay: Duration,
    /// Canned hardware facts returned by `Transport::hardware`.
    pub hardware: HardwareInfo,
}

impl Default for MockConfig {
    fn default() -> Self {
        Self {
            address: "02:00:00:00:00:01".to_string(),
            name: "ATVV mock remote".to_string(),
            caps: DEFAULT_CAPS.to_vec(),
            frame_size: 120,
            frame_interval: Duration::from_millis(20),
            response_delay: Duration::from_millis(5),
            audio_start_delay: Duration::from_millis(15),
            echo_delay: Duration::from_millis(20),
            hardware: HardwareInfo::default(),
        }
    }
}

enum PendingEvent {
    Connected,
    Caps,
    AudioStart,
    Control(Vec<u8>),
}

struct Pending {
    at: Instant,
    event: PendingEvent,
}

enum MockMessage {
    Control(Vec<u8>),
    Audio(Vec<u8>),
    PauseAudio,
    Disconnect { reason: String },
}

/// Device-side control for tests and the CLI; not exposed over D-Bus.
#[derive(Debug, Clone)]
pub struct MockHandle {
    sender: Sender<MockMessage>,
    commands: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl MockHandle {
    /// Emit a direct `AUDIO_START`, as the RC003 does on the voice key.
    pub fn start_audio(&self) {
        let _ = self
            .sender
            .send(MockMessage::Control(vec![0x04, 0x03, 0x02, 0x00]));
    }

    /// Emit `AUDIO_STOP` with a wire reason.
    pub fn stop_audio(&self, reason: u8) {
        let _ = self.sender.send(MockMessage::Control(vec![0x00, reason]));
    }

    /// Emit an arbitrary control notification.
    pub fn send_control(&self, bytes: Vec<u8>) {
        let _ = self.sender.send(MockMessage::Control(bytes));
    }

    /// Emit raw audio-characteristic bytes (for late-audio/discard tests).
    pub fn send_audio(&self, bytes: Vec<u8>) {
        let _ = self.sender.send(MockMessage::Audio(bytes));
    }

    /// Stop the frame stream without an `AUDIO_STOP` (frame-timeout tests).
    pub fn pause_audio(&self) {
        let _ = self.sender.send(MockMessage::PauseAudio);
    }

    /// Drop the link and emit `Disconnected`.
    pub fn disconnect(&self, reason: &str) {
        let _ = self.sender.send(MockMessage::Disconnect {
            reason: reason.to_string(),
        });
    }

    /// Commands written by the daemon, oldest first.
    #[must_use]
    pub fn commands(&self) -> Vec<Vec<u8>> {
        self.commands
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Number of commands whose first byte is `opcode`.
    #[must_use]
    pub fn command_count(&self, opcode: u8) -> usize {
        self.commands
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .filter(|command| command.first() == Some(&opcode))
            .count()
    }

    pub fn clear_commands(&self) {
        self.commands
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }
}

/// A mock ATVV device implementing [`Transport`].
pub struct MockTransport {
    config: MockConfig,
    receiver: Receiver<MockMessage>,
    commands: Arc<Mutex<Vec<Vec<u8>>>>,
    pending: VecDeque<Pending>,
    connected: bool,
    streaming: bool,
    next_frame: Option<Instant>,
}

impl MockTransport {
    /// Create the mock and a handle for device-side events.
    #[must_use]
    pub fn new(config: MockConfig) -> (Self, MockHandle) {
        let (sender, receiver) = mpsc::channel();
        let commands = Arc::new(Mutex::new(Vec::new()));
        let handle = MockHandle {
            sender,
            commands: Arc::clone(&commands),
        };
        let mut transport = Self {
            config,
            receiver,
            commands,
            pending: VecDeque::new(),
            connected: false,
            streaming: false,
            next_frame: None,
        };
        transport.schedule(Duration::ZERO, PendingEvent::Connected);
        (transport, handle)
    }

    fn schedule(&mut self, delay: Duration, event: PendingEvent) {
        self.pending.push_back(Pending {
            at: Instant::now() + delay,
            event,
        });
    }

    fn pop_due(&mut self, now: Instant) -> Option<TransportEvent> {
        while self
            .pending
            .front()
            .is_some_and(|pending| pending.at <= now)
        {
            let pending = self.pending.pop_front()?;
            if let Some(event) = self.apply_pending(pending.event, now) {
                return Some(event);
            }
        }
        None
    }

    fn apply_pending(&mut self, event: PendingEvent, now: Instant) -> Option<TransportEvent> {
        match event {
            PendingEvent::Connected => {
                self.connected = true;
                Some(TransportEvent::Connected {
                    address: self.config.address.clone(),
                    name: self.config.name.clone(),
                })
            }
            PendingEvent::Caps => Some(TransportEvent::Control(self.config.caps.clone())),
            PendingEvent::AudioStart => {
                self.streaming = true;
                self.next_frame = Some(now);
                Some(TransportEvent::Control(vec![0x04, 0x03, 0x02, 0x00]))
            }
            PendingEvent::Control(bytes) => {
                if bytes.first() == Some(&0x00) {
                    self.streaming = false;
                    self.next_frame = None;
                }
                Some(TransportEvent::Control(bytes))
            }
        }
    }

    fn pop_frame(&mut self, now: Instant) -> Option<TransportEvent> {
        if !self.streaming {
            return None;
        }
        let due = self.next_frame?;
        if due > now {
            return None;
        }
        self.next_frame = Some(now + self.config.frame_interval);
        Some(TransportEvent::Audio(self.audio_payload()))
    }

    fn audio_payload(&self) -> Vec<u8> {
        // Deterministic non-silent nibbles so the decoded PCM is observable.
        const PATTERN: [u8; 4] = [0x07, 0x00, 0x0F, 0x02];
        (0..self.config.frame_size)
            .map(|index| PATTERN[index % PATTERN.len()])
            .collect()
    }

    fn apply_message(&mut self, message: MockMessage, now: Instant) -> Option<TransportEvent> {
        match message {
            MockMessage::Control(bytes) => {
                match bytes.first() {
                    Some(0x04) => {
                        self.streaming = true;
                        self.next_frame = Some(now);
                    }
                    Some(0x00) => {
                        self.streaming = false;
                        self.next_frame = None;
                    }
                    _ => {}
                }
                Some(TransportEvent::Control(bytes))
            }
            MockMessage::Audio(bytes) => Some(TransportEvent::Audio(bytes)),
            MockMessage::PauseAudio => {
                self.streaming = false;
                self.next_frame = None;
                None
            }
            MockMessage::Disconnect { reason } => {
                self.connected = false;
                self.streaming = false;
                self.next_frame = None;
                self.pending.clear();
                Some(TransportEvent::Disconnected { reason })
            }
        }
    }

    fn next_due(&self) -> Option<Instant> {
        let scheduled = self.pending.front().map(|pending| pending.at);
        match (scheduled, self.next_frame) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }
}

impl Transport for MockTransport {
    fn kind(&self) -> &'static str {
        "mock"
    }

    fn address(&self) -> &str {
        &self.config.address
    }

    fn device_name(&self) -> &str {
        &self.config.name
    }

    fn hardware(&mut self) -> Result<HardwareInfo, TransportError> {
        Ok(self.config.hardware.clone())
    }

    fn poll(&mut self, timeout: Duration) -> Result<Option<TransportEvent>, TransportError> {
        let deadline = Instant::now() + timeout;
        loop {
            let now = Instant::now();
            if let Ok(message) = self.receiver.try_recv() {
                if let Some(event) = self.apply_message(message, now) {
                    return Ok(Some(event));
                }
                continue;
            }
            if let Some(event) = self.pop_due(now) {
                return Ok(Some(event));
            }
            if let Some(event) = self.pop_frame(now) {
                return Ok(Some(event));
            }

            let wake = self.next_due().map_or(deadline, |due| due.min(deadline));
            let wait = wake.saturating_duration_since(now);
            if wait.is_zero() {
                if now >= deadline {
                    return Ok(None);
                }
                continue;
            }

            match self.receiver.recv_timeout(wait) {
                Ok(message) => {
                    if let Some(event) = self.apply_message(message, Instant::now()) {
                        return Ok(Some(event));
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Err(TransportError::Closed),
            }
            if Instant::now() >= deadline {
                return Ok(None);
            }
        }
    }

    fn write_command(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        self.commands
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(bytes.to_vec());

        if !self.connected {
            return Err(TransportError::NotConnected);
        }

        match bytes.first() {
            Some(0x0A) => self.schedule(self.config.response_delay, PendingEvent::Caps),
            Some(0x0C) => self.schedule(self.config.audio_start_delay, PendingEvent::AudioStart),
            Some(0x0D) => {
                // A real device keeps sending late audio briefly and then
                // echoes AUDIO_STOP. The echo must not cause another close.
                self.streaming = false;
                self.next_frame = None;
                self.schedule(
                    self.config.echo_delay,
                    PendingEvent::Control(vec![0x00, 0x00]),
                );
            }
            _ => {}
        }
        Ok(())
    }

    fn reconnect(&mut self) -> Result<(), TransportError> {
        self.connected = false;
        self.streaming = false;
        self.next_frame = None;
        self.pending.clear();
        self.schedule(Duration::ZERO, PendingEvent::Connected);
        Ok(())
    }
}

/// BlueZ-backed transport for `atvvd`.
///
/// The link is created on the first [`Transport::reconnect`], so the daemon can
/// start, own `org.atvv.Daemon`, and report `disconnected` while the remote is
/// asleep or BlueZ is still connecting. This adapter only translates events and
/// errors; the session path is shared with the mock transport.
pub struct BluezTransport {
    config: atvv_bluez::BluezConfig,
    link: Option<atvv_bluez::BluezLink>,
    hardware: HardwareInfo,
    verbose: bool,
}

impl BluezTransport {
    #[must_use]
    pub fn new(config: atvv_bluez::BluezConfig, verbose: bool) -> Self {
        Self {
            config,
            link: None,
            hardware: HardwareInfo::default(),
            verbose,
        }
    }

    #[must_use]
    pub fn notification_mode(&self) -> Option<&atvv_bluez::NotificationMode> {
        self.link
            .as_ref()
            .map(atvv_bluez::BluezLink::notification_mode)
    }

    fn connect(&mut self) -> Result<(), TransportError> {
        // Drop the old link first: its notification tasks must not publish
        // events for the new link generation.
        self.link = None;
        let link = atvv_bluez::BluezLink::connect(self.config.clone())
            .map_err(|error| TransportError::Other(error.to_string()))?;
        if self.verbose {
            eprintln!(
                "atvvd: notifications via {}",
                link.notification_mode().label()
            );
            if let atvv_bluez::NotificationMode::Start { acquire_error } = link.notification_mode()
            {
                eprintln!("atvvd: AcquireNotify fallback: {acquire_error}");
            }
        }
        self.hardware = link.hardware().clone();
        self.link = Some(link);
        Ok(())
    }
}

impl Transport for BluezTransport {
    fn kind(&self) -> &'static str {
        "bluez"
    }

    fn address(&self) -> &str {
        &self.config.address
    }

    fn device_name(&self) -> &str {
        self.link.as_ref().map_or("not connected", |link| {
            atvv_bluez::BluezLink::device_name(link)
        })
    }

    fn hardware(&mut self) -> Result<HardwareInfo, TransportError> {
        if let Some(link) = self.link.as_mut() {
            link.refresh_battery();
            self.hardware = link.hardware().clone();
        }
        Ok(self.hardware.clone())
    }

    fn needs_connect(&self) -> bool {
        self.link.is_none()
    }

    fn poll(&mut self, timeout: Duration) -> Result<Option<TransportEvent>, TransportError> {
        let Some(link) = &mut self.link else {
            std::thread::sleep(timeout);
            return Ok(None);
        };
        match link.poll(timeout) {
            Ok(Some(atvv_bluez::BluezEvent::Connected { address, name })) => {
                Ok(Some(TransportEvent::Connected { address, name }))
            }
            Ok(Some(atvv_bluez::BluezEvent::Disconnected { reason })) => {
                Ok(Some(TransportEvent::Disconnected { reason }))
            }
            Ok(Some(atvv_bluez::BluezEvent::Control(bytes))) => {
                Ok(Some(TransportEvent::Control(bytes)))
            }
            Ok(Some(atvv_bluez::BluezEvent::Audio(bytes))) => {
                Ok(Some(TransportEvent::Audio(bytes)))
            }
            Ok(None) => Ok(None),
            Err(error) => Err(TransportError::Other(error.to_string())),
        }
    }

    fn write_command(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        let link = self.link.as_mut().ok_or(TransportError::NotConnected)?;
        link.write_command(bytes)
            .map_err(|error| TransportError::Other(error.to_string()))
    }

    fn reconnect(&mut self) -> Result<(), TransportError> {
        self.connect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connected() -> (MockTransport, MockHandle) {
        let (mut transport, handle) = MockTransport::new(MockConfig::default());
        let event = transport.poll(Duration::from_millis(50)).unwrap();
        assert!(matches!(event, Some(TransportEvent::Connected { .. })));
        (transport, handle)
    }

    fn next(transport: &mut MockTransport) -> TransportEvent {
        transport
            .poll(Duration::from_millis(200))
            .unwrap()
            .expect("an event")
    }

    #[test]
    fn answers_caps_and_mic_open() {
        let (mut transport, handle) = connected();
        transport
            .write_command(&[0x0A, 0x01, 0x00, 0x00, 0x03, 0x03])
            .unwrap();
        assert_eq!(
            next(&mut transport),
            TransportEvent::Control(DEFAULT_CAPS.to_vec())
        );

        transport.write_command(&[0x0C, 0x00]).unwrap();
        assert_eq!(
            next(&mut transport),
            TransportEvent::Control(vec![0x04, 0x03, 0x02, 0x00])
        );
        assert_eq!(
            next(&mut transport),
            TransportEvent::Audio(transport.audio_payload())
        );
        assert_eq!(
            handle.commands(),
            vec![vec![0x0A, 0x01, 0x00, 0x00, 0x03, 0x03], vec![0x0C, 0x00]]
        );
    }

    #[test]
    fn close_stops_frames_and_echoes_stop() {
        let (mut transport, handle) = connected();
        transport.write_command(&[0x0C, 0x00]).unwrap();
        let _ = next(&mut transport); // AUDIO_START
        let _ = next(&mut transport); // first frame

        transport.write_command(&[0x0D]).unwrap();
        assert_eq!(
            next(&mut transport),
            TransportEvent::Control(vec![0x00, 0x00])
        );
        assert_eq!(handle.command_count(0x0D), 1);
        assert_eq!(transport.poll(Duration::from_millis(60)).unwrap(), None);
    }

    #[test]
    fn direct_start_and_device_stop_are_available() {
        let (mut transport, handle) = connected();
        handle.start_audio();
        assert_eq!(
            next(&mut transport),
            TransportEvent::Control(vec![0x04, 0x03, 0x02, 0x00])
        );
        handle.stop_audio(0x00);
        assert_eq!(
            next(&mut transport),
            TransportEvent::Control(vec![0x00, 0x00])
        );
        handle.send_audio(vec![0xAA; 120]);
        assert_eq!(next(&mut transport), TransportEvent::Audio(vec![0xAA; 120]));
        handle.disconnect("test");
        assert_eq!(
            next(&mut transport),
            TransportEvent::Disconnected {
                reason: "test".to_string()
            }
        );
    }

    #[test]
    fn writes_are_rejected_while_disconnected() {
        let (mut transport, _handle) = MockTransport::new(MockConfig::default());
        assert!(matches!(
            transport.write_command(&[0x0A]),
            Err(TransportError::NotConnected)
        ));
    }
}
