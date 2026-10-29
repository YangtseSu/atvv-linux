// SPDX-License-Identifier: GPL-3.0-or-later

//! Pure ATVV session state machine.
//!
//! The state machine emits actions; it never touches BlueZ, timers, or
//! PipeWire. A daemon can drive it from real notifications and execute the
//! resulting [`SessionAction`] values.

use std::fmt;

use crate::caps::Capabilities;
use crate::events::{AudioStopReason, CtlEvent, StreamId};
use crate::profile::DeviceProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Connected,
    Ready,
    Opening,
    Streaming,
    Disconnected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionAction {
    Send(Vec<u8>),
    VoiceStarted,
    VoiceStopped,
}

impl fmt::Display for SessionAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Send(bytes) => write!(f, "send {bytes:02x?}"),
            Self::VoiceStarted => f.write_str("voice started"),
            Self::VoiceStopped => f.write_str("voice stopped"),
        }
    }
}

/// Stateful ATVV session.
#[derive(Debug, Clone)]
pub struct Session {
    profile: DeviceProfile,
    state: SessionState,
    caps: Option<Capabilities>,
    stream_id: Option<StreamId>,
}

impl Session {
    #[must_use]
    pub fn new(profile: DeviceProfile) -> Self {
        Self {
            profile,
            state: SessionState::Connected,
            caps: None,
            stream_id: None,
        }
    }

    #[must_use]
    pub fn state(&self) -> SessionState {
        self.state
    }

    #[must_use]
    pub fn caps(&self) -> Option<Capabilities> {
        self.caps
    }

    #[must_use]
    pub fn profile(&self) -> DeviceProfile {
        self.profile
    }

    /// Apply a capability response. This moves a fresh connection to `Ready`.
    pub fn on_caps(&mut self, caps: Capabilities) -> Vec<SessionAction> {
        self.caps = Some(caps);
        if self.state == SessionState::Connected {
            self.state = SessionState::Ready;
        }
        Vec::new()
    }

    /// Handle one control notification.
    pub fn on_event(&mut self, event: CtlEvent) -> Vec<SessionAction> {
        match event {
            CtlEvent::CapsResp(caps) => self.on_caps(caps),
            CtlEvent::StartSearch => self.on_start_search(),
            CtlEvent::AudioStart { stream_id, .. } => self.on_audio_start(stream_id),
            CtlEvent::AudioStop { reason } => self.on_audio_stop(reason),
            CtlEvent::MicOpenError(_) => {
                self.state = SessionState::Ready;
                self.stream_id = None;
                Vec::new()
            }
            CtlEvent::AudioSync { .. } | CtlEvent::Unknown(_) => Vec::new(),
        }
    }

    /// The remote's voice key was pressed.
    pub fn on_start_search(&mut self) -> Vec<SessionAction> {
        match self.state {
            SessionState::Ready => {
                self.state = SessionState::Opening;
                vec![SessionAction::Send(self.profile.mic_open())]
            }
            SessionState::Opening | SessionState::Streaming => {
                let was_streaming = self.state == SessionState::Streaming;
                let stream = self.stream_id.unwrap_or(StreamId::MIC_OPEN);
                self.state = SessionState::Ready;
                self.stream_id = None;
                let mut actions = Vec::with_capacity(2);
                if was_streaming {
                    actions.push(SessionAction::VoiceStopped);
                }
                actions.push(SessionAction::Send(self.profile.mic_close(stream)));
                actions
            }
            SessionState::Connected | SessionState::Disconnected => Vec::new(),
        }
    }

    /// Audio started. RC003 reaches this state without a preceding
    /// `START_SEARCH`.
    pub fn on_audio_start(&mut self, stream_id: StreamId) -> Vec<SessionAction> {
        if self.state == SessionState::Disconnected {
            return Vec::new();
        }

        self.stream_id = Some(stream_id);
        if self.state == SessionState::Streaming {
            return Vec::new();
        }

        self.state = SessionState::Streaming;
        vec![SessionAction::VoiceStarted]
    }

    /// Audio stopped. A stop outside `Streaming` is an echo or a stray packet.
    pub fn on_audio_stop(&mut self, reason: AudioStopReason) -> Vec<SessionAction> {
        if self.state != SessionState::Streaming {
            return Vec::new();
        }

        let stream = self.stream_id.unwrap_or(StreamId::MIC_OPEN);
        self.state = SessionState::Ready;
        self.stream_id = None;

        let mut actions = vec![SessionAction::VoiceStopped];
        if !matches!(reason, AudioStopReason::HttButtonRelease) {
            actions.push(SessionAction::Send(self.profile.mic_close(stream)));
        }
        actions
    }

    /// Keepalive payload while streaming.
    #[must_use]
    pub fn keepalive(&self) -> Option<Vec<u8>> {
        if self.state == SessionState::Streaming {
            Some(
                self.profile
                    .mic_extend(self.stream_id.unwrap_or(StreamId::MIC_OPEN)),
            )
        } else {
            None
        }
    }

    /// Device disconnected. Emits `VoiceStopped` if a session was active.
    pub fn disconnect(&mut self) -> Vec<SessionAction> {
        let was_streaming = self.state == SessionState::Streaming;
        self.state = SessionState::Disconnected;
        self.caps = None;
        self.stream_id = None;
        if was_streaming {
            vec![SessionAction::VoiceStopped]
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::caps::{Codec, InteractionModel, ProtocolVersion};
    use crate::profile::RC003;

    fn caps() -> Capabilities {
        Capabilities {
            version: ProtocolVersion::V1_0,
            codec_mask: 0x02,
            interaction: InteractionModel::HoldToTalk,
            frame_size: 120,
        }
    }

    fn ready() -> Session {
        let mut session = Session::new(RC003);
        session.on_caps(caps());
        session
    }

    #[test]
    fn caps_moves_a_fresh_connection_to_ready() {
        let mut session = Session::new(RC003);
        assert_eq!(session.state(), SessionState::Connected);
        session.on_caps(caps());
        assert_eq!(session.state(), SessionState::Ready);
        assert_eq!(session.caps(), Some(caps()));
    }

    #[test]
    fn rc003_direct_audio_start_is_a_press() {
        let mut session = ready();
        let actions = session.on_event(CtlEvent::AudioStart {
            reason: crate::events::AudioStartReason::HoldToTalk,
            codec: Some(Codec::Adpcm16k),
            stream_id: StreamId(1),
        });
        assert_eq!(actions, vec![SessionAction::VoiceStarted]);
        assert_eq!(session.state(), SessionState::Streaming);
    }

    #[test]
    fn start_search_opens_and_audio_start_activates() {
        let mut session = ready();
        assert_eq!(
            session.on_event(CtlEvent::StartSearch),
            vec![SessionAction::Send(vec![0x0C, 0x00])]
        );
        assert_eq!(session.state(), SessionState::Opening);
        assert_eq!(
            session.on_event(CtlEvent::AudioStart {
                reason: crate::events::AudioStartReason::MicOpen,
                codec: Some(Codec::Adpcm16k),
                stream_id: StreamId(0),
            }),
            vec![SessionAction::VoiceStarted]
        );
        assert_eq!(session.state(), SessionState::Streaming);
    }

    #[test]
    fn audio_stop_sends_close_once_and_ignores_the_echo() {
        let mut session = ready();
        session.on_audio_start(StreamId(4));
        assert_eq!(
            session.on_event(CtlEvent::AudioStop {
                reason: AudioStopReason::MicClose,
            }),
            vec![SessionAction::VoiceStopped, SessionAction::Send(vec![0x0D])]
        );
        assert_eq!(session.state(), SessionState::Ready);
        assert_eq!(
            session.on_event(CtlEvent::AudioStop {
                reason: AudioStopReason::MicClose,
            }),
            Vec::new()
        );
    }

    #[test]
    fn htt_release_does_not_send_mic_close() {
        let mut session = ready();
        session.on_audio_start(StreamId(2));
        assert_eq!(
            session.on_event(CtlEvent::AudioStop {
                reason: AudioStopReason::HttButtonRelease,
            }),
            vec![SessionAction::VoiceStopped]
        );
        assert_eq!(session.state(), SessionState::Ready);
    }

    #[test]
    fn keepalive_only_exists_while_streaming() {
        let mut session = ready();
        assert_eq!(session.keepalive(), None);
        session.on_audio_start(StreamId(1));
        assert_eq!(session.keepalive(), Some(vec![0x0E]));
    }

    #[test]
    fn second_start_search_toggles_off() {
        let mut session = ready();
        session.on_start_search();
        session.on_audio_start(StreamId(1));
        assert_eq!(
            session.on_start_search(),
            vec![SessionAction::VoiceStopped, SessionAction::Send(vec![0x0D])]
        );
        assert_eq!(session.state(), SessionState::Ready);
    }

    #[test]
    fn disconnect_reports_a_stopped_session() {
        let mut session = ready();
        session.on_audio_start(StreamId(1));
        assert_eq!(session.disconnect(), vec![SessionAction::VoiceStopped]);
        assert_eq!(session.state(), SessionState::Disconnected);
        assert_eq!(session.keepalive(), None);
    }
}
