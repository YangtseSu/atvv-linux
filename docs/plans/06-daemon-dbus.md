# Step 06 — daemon and D-Bus skeleton

Status: ✅ done
Depends on: 01, 02, 04, 05
Touches: `crates/atvv-daemon/`, `crates/atvv-cli/`, `Cargo.toml`,
`packaging/systemd/`

## Goal

An `atvvd` daemon that wires the session state machine, the PipeWire source,
the input layer, and a session D-Bus API behind a mock BLE transport. No real
device is required.

## Deliverables

- ✅ `atvv-daemon` binary `atvvd` with `--device <MAC>`, `--verbose`, `--mock`,
  and `--config`.
- ✅ Mock BLE transport behind the `Transport` trait; injects `CAPS_RESP`,
  direct `AUDIO_START`, `AUDIO_STOP`, and the stop echo, and emits 120-byte
  ADPCM frames while streaming.
- ✅ D-Bus `org.atvv.Daemon` on the session bus:
  - `GetStatus() -> s` (JSON) and `ListDevices() -> as`
  - `StartVoice()`, `StopVoice()`, `ToggleVoice()`, `ReloadConfig()`
  - signals `StateChanged(s)`, `VoiceStarted()`, `VoiceStopped()`
- ✅ `atvvctl` gains `status`, `mic start|stop|toggle`, `reload`, and `doctor`
  using D-Bus when available; existing offline diagnostics remain.
- ✅ State machine, source supervisor, and input layer are wired; the mock
  transport drives the same `Session` code path as the future BLE transport.
  Input `dictation:*` actions drive voice start/stop/toggle; missing input or
  uinput is reported as degraded.
- ✅ Integration test starts an isolated `dbus-daemon`, then covers direct
  start, stop, echo, source state, signals, and introspection.
- ✅ `atvvctl doctor` reports D-Bus, PipeWire node, uinput access, and config.
- ✅ `packaging/systemd/atvvd.service` with `PartOf=pipewire.service` and
  `Restart=always` (ADR 0005). Installation/ExecStart finalization belongs to
  step 09; the unit is not installed by step 06.

## Design notes

- Research: `docs/research/landscape.md` §3.1 and §6, and
  `docs/research/protocol-and-device.md` §7.
- `Transport` returns `Connected`/`Disconnected`/`Control`/`Audio` events and
  accepts command-characteristic writes. The mock is a device simulation, not
  a second session implementation.
- The daemon owns timers: capability timeout, frame timeout, keepalive,
  reconnect backoff, post-stop discard. `atvv-protocol` stays clock-free.
- `StartVoice` sends `MIC_OPEN`; the mock answers with a direct `AUDIO_START`
  (no `START_SEARCH`). A device `AUDIO_STOP` produces one `MIC_CLOSE`; the
  mock's echo arrives after `Ready` and is ignored by `Session`.
- D-Bus carries a JSON view model (`StatusView`), not protocol structs.
- Audio sinks are behind `PcmSink`: `PipeWireSink` in production and a
  counting/degraded sink in tests, so tests do not require PipeWire.
- The object is registered before `org.atvv.Daemon` is claimed. Claiming the
  name first left a startup window where a client could call an unregistered
  object and wait forever; the integration test and CLI smoke run exercise the
  corrected order.
- `rust-version` moved from 1.85 to 1.87 because zbus 5 requires it.

## Out of scope

Real BlueZ, real GATT, real RC003, ASR, and GUI.

## Verification

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p atvv-daemon
dbus-run-session -- target/debug/atvvd --mock --verbose
busctl --user call org.atvv.Daemon /org/atvv/Daemon org.atvv.Daemon GetStatus
busctl --user call org.atvv.Daemon /org/atvv/Daemon org.atvv.Daemon StartVoice
target/debug/atvvctl status
target/debug/atvvctl mic stop
target/debug/atvvctl doctor
```

Observed on 2026-10-30:

- `cargo test --workspace` → 88 passed, 1 ignored (kernel-uinput test).
- `cargo test -p atvv-daemon` → 13 unit tests + 1 isolated-bus integration
  test; the integration test starts its own `dbus-daemon`.
- `dbus-run-session -- target/debug/atvvd --mock --verbose` plus
  `busctl --user call ... GetStatus` and `StartVoice` → state `ready` →
  `opening` → `streaming`; `atvvctl status` showed decoded `frames`/`samples`
  and `atvvctl mic stop` returned state `ready`.
- The integration test asserted:
  - direct `AUDIO_START` → `VoiceStarted` and `streaming` with decoded frames;
  - device `AUDIO_STOP` → exactly one `0x0D`, and the echo 150 ms later did not
    add a second one;
  - `StartVoice`/`StopVoice` → `MIC_OPEN`/`MIC_CLOSE` with one close;
  - introspection XML contains all methods and signals;
  - `VoiceStarted`/`VoiceStopped` signals arrive on an isolated bus;
  - `ReloadConfig` applies `gain_db = 6` from a real file.
- Additional unit tests cover the 10 s keepalive, frame timeout with a close,
  capability timeout after two `GET_CAPS` retries, the 300 ms post-stop
  discard, and reconnect backoff.
- `atvvctl doctor` against the mock daemon:
  - `daemon ok state=ready transport=mock`
  - `source ok atvv-miremote-rc003 available`
  - `pipewire ok node atvv-miremote-rc003 found`
  - `uinput ok /dev/uinput is writable`
  - `config warn ... not found; defaults in use`
- PipeWire recording from the daemon's source:
  `pw-cli info atvv-miremote-rc003` reported `media.class=Audio/Source`,
  `node.name=atvv-miremote-rc003`, `audio.rate=16000`,
  `node.virtual=true`; `pw-record --target atvv-miremote-rc003 --rate 16000
  --channels 1 --format s16` produced a 1.70 s file (`soxi`), and
  `sox ... stat` reported RMS amplitude 0.366.

## Exit criteria

- ✅ Mock direct `AUDIO_START` produces `VoiceStarted` and a streaming source.
- ✅ Mock `AUDIO_STOP` produces one `MIC_CLOSE`; the echo is ignored.
- ✅ D-Bus methods/signals are introspectable and tested.
- ✅ Missing PipeWire/uinput is reported as degraded, not fatal (unit tests for
  both sinks and input; `atvvctl doctor` warns).
- ✅ No real BLE code is required for the tests.

## Risks

- D-Bus tests can be flaky if they use the developer's session bus; the
  integration test starts its own bus.
- Prematurely coupling D-Bus to protocol types will make the API unstable; the
  view model is separate.
- The mock must not become a second implementation of the session logic; it
  only produces device-side events.
- The systemd unit cannot work until step 07 supplies the BlueZ transport;
  do not install it with `--mock`.

## Progress log

- 2026-10-29 — step planned; no implementation yet.
- 2026-10-30 — implemented `atvv-daemon` (transport trait, mock device,
  session/timer/audio/input loop, zbus interface and client, JSON status,
  TOML config reload), `atvvd`, `atvvctl status|mic|reload|doctor`, the
  isolated-bus integration test, and the systemd user unit. `Transport` writes
  are rejected while disconnected; the mock prioritizes control messages over
  generated audio frames. The D-Bus name is claimed after object registration
  to remove a startup call-loss window. Unit tests also cover the 10 s
  keepalive, the 300 ms post-stop discard, and the reconnect backoff. Raised
  the workspace MSRV to 1.87 for zbus 5. Verification above is hardware-free;
  no RC003 claim is made.
- 2026-10-30 — CI failed at `libspa-sys` because the runner lacked
  `libpipewire-0.3`. The workflow now installs `libpipewire-0.3-dev`,
  `libspa-0.2-dev`, `libclang-dev`, and `dbus`; GitHub Actions `ci` for
  `2c0c435` passed fmt, clippy, and the workspace tests.
