# Step 25 — ATVV v1.0 conformance fixes

Status: ⬜ not-started
Depends on: 01, 02, 04, 06
Touches: `crates/atvv-protocol/`, `crates/atvv-daemon/`,
`crates/atvv-audio/`, `docs/decisions/`, `docs/research/`, `docs/`

## Goal

Close the three spec gaps found while reading the archived Google Voice over
BLE 1.0 spec (`docs/research/google-atvv-1.0-spec.md` §7):

1. `MIC_OPEN_ERROR(0x0F80)` during an active PTT/HTT stream must not end the
   stream;
2. a mid-session `AUDIO_SYNC` codec change must rebuild the resampler instead
   of silently playing at the wrong rate;
3. `AUDIO_SYNC` sequence numbers must be tracked so gaps are reported.

Also name the `MIC_OPEN_ERROR` codes in logs and `last_error` so a manual
open failure is diagnosable.

No hardware claim is made: the current RC003 and Chromecast profiles are fixed
16 kHz, so the dynamic-rate path is proven synthetically only.

## Evidence

- `Session::on_event` currently handles every `CtlEvent::MicOpenError` by
  setting `Ready` and clearing the stream id:
  `crates/atvv-protocol/src/session.rs`. The spec says `0x0F80` is returned
  when a `MIC_OPEN` arrives during an ongoing PTT/HTT interaction and must not
  interrupt that stream.
- `Daemon::on_control`/`on_audio` parse `AudioSync` but use only
  `predictor`/`step_index`; `codec` and `seq` are discarded:
  `crates/atvv-daemon/src/daemon.rs`.
- `on_voice_started` builds one `Resampler` from `CAPS_RESP` and never changes
  it. The spec says the `AUDIO_SYNC` codec applies to audio after the sync.
- `protocol-and-device.md` now documents that sequence-gap logging is not
  implemented; this step makes the doc true.
- `MicOpenError` is currently logged as a raw hex code; the spec defines
  `0x0F02`, `0x0F03`, `0x0F80`, `0x0FFF`, and reserved `0x0F01`.

## Design

### `MIC_OPEN_ERROR`

- `Session::on_event(CtlEvent::MicOpenError(code))`:
  - `Streaming`: return no actions and keep the state, stream id, and
    keepalive. `0x0F80` is expected here; other codes are also non-fatal
    because the stream is already active.
  - `Opening`: set `Ready`, clear the stream id, return no actions. This is a
    failed open; the next user edge can retry.
  - `Ready`/`Connected`/`Disconnected`: no-op.
- Do not use the error code itself to decide the state; the state is the
  contract.
- Add `atvv_protocol::events::mic_open_error_name(u16) -> &'static str` for
  `0x0F02` remote inactive, `0x0F03` notifications disabled, `0x0F80`
  interaction in progress, `0x0FFF` internal error, and `0x0F01` reserved;
  return `"unknown"` for other values. The daemon formats the hex code next
  to the name. Unit-test the mapping.
- Daemon logging:
  - trace `mic open error 0x0f80 (interaction in progress)`.
  - when the session was `Streaming`, do not overwrite `last_error`; the
    error is expected and the stream continues.
  - when the session was `Opening`, set `last_error` to the named error.
- Keep the existing `execute(actions)` path; the session returns no actions.

### `AUDIO_SYNC` rate changes

- Add `stream_codec: Option<Codec>` (or `stream_rate: u32`) to the daemon's
  audio state, initialized in `on_voice_started` from
  `session.caps().and_then(Capabilities::best_codec)` and falling back to the
  profile's preferred codec.
- Factor one method:
  `apply_audio_sync(codec: Option<Codec>, seq: u16, predictor: i16, step_index: u8, now)`.
  Call it from both the control-channel and audio-characteristic `0x0A`
  branches; do not duplicate the logic.
- In that method:
  - always reset the decoder predictor/step;
  - if `codec` is `Some` and differs from `stream_codec`:
    - accept it only when `profile.supports(codec)` and the negotiated
      `CAPS_RESP` mask supports it;
    - rebuild `Resampler::new(codec.sample_rate(), graph_rate)` and update
      `stream_codec`/`stream_rate`;
    - reset the resampler phase (the sync is a discontinuity), not the
      accumulator frame size;
    - trace the old and new rates.
  - if `codec` is `Some` but the profile or negotiated `CAPS_RESP` mask does
    not support it:
    - set a named `last_error`, trace a bounded-rate warning, and close the
      stream through the existing `on_audio_stop(MicClose)` path so the
      daemon stops decoding at the wrong rate. Do not silently switch to an
      unadvertised rate or keep misplaying the old one;
  - if `codec` is `None` (an invalid wire value), keep the current rate,
    reset the decoder, and trace a bounded-rate warning; the packet may be a
    firmware sync with a garbage codec byte.
  - a valid sync or an absent codec does not change session state or stream
    id; only the unsupported-codec path uses the normal close actions.
- Add `stream_rate: u32` to `AudioView` with `#[serde(default)]`, initialized
  to `profile.sample_rate`, updated on `VoiceStarted` and on a valid sync, and
  reset with the other per-session counters. `contract.sample_rate` remains
  the resident graph rate.
- `on_voice_started` keeps `CAPS_RESP` authoritative for the initial rate
  (ADR 0009). `AUDIO_START`'s codec remains non-authoritative on RC003.
- Amend ADR 0009 with a "Dynamic rate changes" section: `CAPS_RESP` decides
  the initial stream rate; a valid `AUDIO_SYNC` codec changes the decode rate
  mid-session; the PipeWire graph rate stays fixed and the resampler is
  rebuilt. Remove the old implication that CAPS is the only rate source.

### Sequence gaps

- Add a small `SeqTracker` (in `atvv-protocol` or a private daemon struct):
  - `next: Option<u16>`, `gaps: u64`;
  - on `AUDIO_SYNC(seq)`: if `next` is Some and `seq != expected`, count the
    forward gap (`seq.wrapping_sub(expected)`, treat a value above `0x8000`
    as a backwards/rollover anomaly and do not count it); set
    `next = Some(seq)`;
  - after each complete decoded frame: `next = Some(next.wrapping_add(1))`;
  - reset on `VoiceStarted` and on disconnect.
- Log at most one gap summary per `ANOMALY_INTERVAL`; use the existing
  bounded-rate pattern rather than one line per gap.
- Add `sequence_gaps: u64` to `AudioView` with `#[serde(default)]`.
- If no `AUDIO_SYNC` has arrived, do not guess a starting sequence; the
  counter stays zero.
- Unit tests: consecutive sync (no gap), skipped frames (gap counted), u16
  rollover (`0xFFFF` to `0x0000`), reset on start.
- Do not log every frame; the counter and the bounded summary are the
  observable.

### Docs

- Update `docs/research/protocol-and-device.md`:
  - remove the "not implemented" caveat for sequence gaps once the tracker
    exists;
  - mark dynamic-rate support as implemented and note that no real
    dynamic-rate device has been run.
- Update `docs/research/google-atvv-1.0-spec.md` §7: mark the three gaps
  closed by this step, leaving the "no hardware run" caveat.
- Update `docs/decisions/0009-caps-authoritative-sample-rate.md` with the
  mid-session `AUDIO_SYNC` rule.
- Update `docs/handoff.md` and this step's progress log.

## Deliverables

- ⬜ `Session` keeps `Streaming` on `MIC_OPEN_ERROR`; `Opening` still returns
  to `Ready`.
- ⬜ `mic_open_error_name` mapping and named daemon logs/`last_error`.
- ⬜ `apply_audio_sync` shared by both sync paths; valid codec changes rebuild
  the resampler; invalid/absent codecs keep the current rate and warn.
- ⬜ `AudioView.stream_rate` and `AudioView.sequence_gaps` with
  `#[serde(default)]`.
- ⬜ `SeqTracker` with forward-gap counting, rollover handling, and reset on
  start/disconnect.
- ⬜ Tests for all three areas, including a daemon mock sequence
  `AUDIO_START -> MIC_OPEN_ERROR(0x0F80) -> AUDIO_STOP` that still produces
  `VoiceStopped`, and an `AUDIO_SYNC` 16 kHz -> 8 kHz switch that changes the
  decode rate and the output sample count.
- ⬜ ADR 0009 amended; research docs, plan index, and handoff updated.

## Verification

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p atvv-protocol mic_open_error
cargo test -p atvv-protocol seq
cargo test -p atvv-daemon sync
cargo test -p atvv-daemon mic_open_error
```

Synthetic daemon check:

```text
dbus-run-session -- bash -c '
  cargo run -q -p atvv-daemon -- --mock --verbose &
  sleep 1
  cargo run -q -p atvv-cli -- status
'
```

The mock should expose `audio.stream_rate` and `audio.sequence_gaps`; no
hardware is required for the pass. A real dynamic-rate device run is a
separate evidence item and is not claimed here.

## Exit criteria

- ⬜ `MIC_OPEN_ERROR` while `Streaming` leaves the state, stream id, and
  keepalive unchanged; `0x0F80` does not set `last_error`.
- ⬜ `MIC_OPEN_ERROR` while `Opening` returns to `Ready` and names the code.
- ⬜ A valid `AUDIO_SYNC` codec change updates `stream_rate` and the
  resampler; an unsupported codec closes the stream with a named error; an
  invalid/absent codec keeps the current rate and is reported at a bounded
  rate.
- ⬜ Sequence gaps are counted in `audio.sequence_gaps` and summarized at a
  bounded rate; rollover is not reported as a gap.
- ⬜ ADR 0009 states the CAPS-at-start / AUDIO_SYNC-mid-stream rule.
- ⬜ The research docs no longer claim an unimplemented feature.
- ⬜ Tests cover the three paths; no hardware claim is made.

## Risks

- A device may send `AUDIO_SYNC` with a codec byte that disagrees with the
  actual data. Validate against the profile and CAPS mask; keep the current
  rate and warn rather than switching blindly.
- Rebuilding the resampler at a sync can introduce a short discontinuity; that
  is the correct behavior at a rate change, and the decoder is reset by the
  same packet.
- `MIC_OPEN_ERROR(0x0F80)` in `Opening` may mean an interaction started but
  its `AUDIO_START` was lost. Returning to `Ready` lets the next edge retry;
  do not keep `Opening` forever.
- Sequence numbers can be unreliable on firmware that does not send
  `AUDIO_SYNC`; tracking starts only after the first sync, so a missing sync
  cannot create false gaps.
- The RC003's `AUDIO_START` codec quirk must not be generalized: only
  `AUDIO_SYNC` changes the rate mid-stream; `AUDIO_START` stays
  non-authoritative for that profile.

## Progress log

- 2026-11-02 — plan created from
  `docs/research/google-atvv-1.0-spec.md` §7 and the current session code.

## Next action

Add the `MIC_OPEN_ERROR` state tests and the named error mapping first; then
factor `apply_audio_sync`, add the `SeqTracker`, and update ADR 0009 and the
research docs.
