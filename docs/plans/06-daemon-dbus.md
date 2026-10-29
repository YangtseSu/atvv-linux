# Step 06 — daemon and D-Bus skeleton

Status: ⬜ not-started
Depends on: 01, 02, 04, 05
Touches: `crates/atvv-daemon/`, `crates/atvv-cli/`, `Cargo.toml`

## Goal

An `atvvd` daemon that wires the session state machine, the PipeWire source,
the input layer, and a session D-Bus API behind a mock BLE transport. No real
device is required.

## Deliverables

- ⬜ `atvv-daemon` binary `atvvd` with `--device <MAC>` and `--verbose`.
- ⬜ Mock BLE transport behind a trait; inject `CAPS_RESP`, direct
  `AUDIO_START`, `AUDIO_STOP`, and the stop echo.
- ⬜ D-Bus `org.atvv.Daemon` on the session bus:
  - `GetStatus() -> s` (JSON) and `ListDevices() -> as`
  - `StartVoice()`, `StopVoice()`, `ReloadConfig()`
  - signals `StateChanged(s)`, `VoiceStarted()`, `VoiceStopped()`
- ⬜ `atvvctl` gains `status`, `mic start|stop|toggle`, and `reload` using
  D-Bus when available; existing offline diagnostics remain.
- ⬜ State machine, source supervisor, and input layer are wired; the mock
  transport drives the same code path as the future BLE transport.
- ⬜ Integration test under `dbus-run-session` (or an isolated bus) covering
  direct start, stop, echo, and source state.
- ⬜ `atvvctl doctor` reports D-Bus, PipeWire node, uinput access, and config.

## Design notes

- The transport trait returns notification streams and accepts writes. The
  daemon must not care whether the bytes came from BlueZ or a fixture.
- The daemon owns timers: capability timeout, frame timeout, keepalive,
  reconnect backoff. The protocol crate stays clock-free.
- D-Bus is the stable integration boundary; do not expose internal session
  types directly.
- The daemon must be a systemd user service; no root.

## Out of scope

Real BlueZ, real GATT, real RC003, ASR, and GUI.

## Verification

```text
cargo test -p atvv-daemon
dbus-run-session -- cargo run -q -p atvv-daemon -- --mock
busctl --user call org.atvv.Daemon /org/atvv/Daemon org.atvv.Daemon GetStatus
busctl --user call org.atvv.Daemon /org/atvv/Daemon org.atvv.Daemon StartVoice
```

Also verify `atvvctl status` and `atvvctl doctor` against the mock daemon.

## Exit criteria

- ⬜ Mock direct `AUDIO_START` produces `VoiceStarted` and a streaming source.
- ⬜ Mock `AUDIO_STOP` produces one `MIC_CLOSE`; the echo is ignored.
- ⬜ D-Bus methods/signals are introspectable and tested.
- ⬜ Missing PipeWire/uinput is reported as degraded, not fatal.
- ⬜ No real BLE code is required for the tests.

## Risks

- D-Bus tests can be flaky if they use the developer's session bus; use an
  isolated bus.
- Prematurely coupling D-Bus to protocol types will make the API unstable; keep
  a view model.
- The mock must not become a second implementation of the session logic.

## Progress log

- 2026-10-29 — step planned; no implementation yet.
