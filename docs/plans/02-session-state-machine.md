# Step 02 — session state machine

Status: ✅ done
Depends on: 01
Touches: `crates/atvv-protocol/src/session.rs`

## Goal

A pure ATVV session state machine that turns control events into commands and
voice-start/stop actions. It must encode the RC003 firmware behavior without
sleeping, scheduling, or I/O.

## Deliverables

- ✅ `SessionState`: `Connected`, `Ready`, `Opening`, `Streaming`, `Disconnected`.
- ✅ `SessionAction`: `Send(bytes)`, `VoiceStarted`, `VoiceStopped`; human-readable
  `Display`.
- ✅ `CAPS_RESP` moves a fresh connection to `Ready`.
- ✅ `START_SEARCH` in `Ready` sends `MIC_OPEN` and enters `Opening`.
- ✅ A second `START_SEARCH` while `Opening`/`Streaming` sends `MIC_CLOSE` and
  returns to `Ready`; a streaming session emits `VoiceStopped`.
- ✅ `AUDIO_START` is accepted directly from `Ready` (RC003 sends no
  `START_SEARCH`) and emits `VoiceStarted`.
- ✅ `AUDIO_STOP` while streaming emits `VoiceStopped` and sends `MIC_CLOSE`
  unless the reason is `HttButtonRelease`.
- ✅ An `AUDIO_STOP` outside `Streaming` is ignored, so the RC003's
  `MIC_CLOSE` echo cannot cause a close loop.
- ✅ `keepalive()` returns the profile's extend payload only while streaming.
- ✅ `disconnect()` clears caps/stream and emits `VoiceStopped` if needed.

## Design notes

- The machine is driven by events; it never waits for a timeout. Frame timeout,
  keepalive cadence, and reconnect backoff belong to the daemon.
- The RC003's direct `AUDIO_START` is a first-class path, not an error.
- `AUDIO_STOP` handling is state-gated. A stop in `Ready` is an echo or a stale
  packet and is deliberately ignored.
- The profile supplies the payload variants; the state machine does not contain
  `[0D]` or `[0E]` literals.

## Out of scope

BLE notification wiring, timers, source lifecycle, audio decoding, and D-Bus.

## Verification

```text
cargo test -p atvv-protocol
```

Observed 2026-10-29: session tests pass as part of the 33-test workspace run,
including direct start, start-search open, stop/echo, HTT release, keepalive,
toggle-off, and disconnect.

## Exit criteria

- ✅ RC003 direct `AUDIO_START` produces `VoiceStarted` without a preceding
  `START_SEARCH`.
- ✅ A normal start-search flow opens and then streams.
- ✅ Stop sends one close; a repeat stop is ignored.
- ✅ HTT release does not send a close.
- ✅ Keepalive exists only while streaming.
- ✅ Disconnect reports a stopped session.

## Risks

- The real RC003 `AUDIO_STOP` reason is not yet captured; step 07 records it.
  If it differs from `MicClose`, the profile/state handling is adjusted with
  evidence.
- Timer-driven behavior is intentionally absent; step 06 will test the daemon's
  timeout and keepalive policy.

## Progress log

- 2026-10-29 — step opened: state machine, actions, and tests implemented.
