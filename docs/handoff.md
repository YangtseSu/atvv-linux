# Handoff

Updated: 2026-10-30

## Project

- License: GPL-3.0-or-later
- Git: `main` tracks `origin/main`; check `git status` for unpushed commits. Do not push without explicit approval.
- Latest commit: run `git log -1 --oneline` (do not hard-code a hash that this file would invalidate).
- Tests: `cargo test --workspace` → 88 passed (7 test binaries),
  1 ignored kernel-uinput test.

## Read first

1. `AGENTS.md` — hard rules and commands.
2. `docs/plans/README.md` — canonical step index and M0 exit criteria.
3. `docs/research/README.md` — evidence index and current deltas.
4. `docs/decisions/0005-pipewire-restart.md` — systemd owns PipeWire restart.
5. The next step file and its research links before editing code.

This file is updated in place at the end of every step/session. Do not create
per-session copies; `docs/plans/` remains the canonical plan.

## Completed

- **01 protocol core**: v1.0 CAPS, commands, control events, ADPCM, frame
  accumulator, device profile, GATT UUIDs.
- **02 session state machine**: direct RC003 `AUDIO_START`, start-search flow,
  stop/echo handling, keepalive, disconnect.
- **03 CLI diagnostics**: `atvvctl profile list/show`, `caps decode`,
  `simulate`.
- **04 PipeWire source**: native `Audio/Source`, bounded lock-free queue,
  synthetic-tone test, node-removal recovery. Service restart is delegated to
  systemd.
- **05 input/uinput**: `atvv-input` with profile key maps, pointer-safe evdev
  discovery, `EVIOCGRAB`, the RC003 gesture FSM (600 ms latch), action grammar,
  uinput virtual keyboard, and `atvvctl input fake-remote`.
- **06 daemon/D-Bus**: `atvv-daemon` + `atvvd`, `Transport` trait and mock
  device, timers (CAPS/frame/keepalive/reconnect), ADPCM decode with post-stop
  discard and gain, resident PipeWire sink behind `PcmSink`, input runtime
  wiring, JSON `StatusView`, `org.atvv.Daemon` service/client, signals,
  `atvvctl status|mic|reload|doctor`, isolated-bus integration test, and
  `packaging/systemd/atvvd.service`.

## Not implemented

- **07 RC003 hardware**: real BlueZ GATT transport, pairing, CAPS/audio/
  reconnect. Blocked: no RC003 is available.
- **08 fcitx5-vinput/MCP**: trigger key, D-Bus bridge, MCP. Depends on 07.
- **09 Arch packaging**: PKGBUILD, udev, installed systemd unit, CI, hardware
  acceptance. Depends on all prior steps.

Do not claim hardware support. Step 07 and the hardware acceptance part of
step 09 are blocked on a device.

## Decisions

- Research basis, reference snapshots, licenses, and open hardware questions:
  `docs/research/`.
- Userspace stack only: BlueZ + daemon + PipeWire + evdev/uinput.
- ATVV v1.0+ only; v0.4 is out of scope.
- Rust 2024; MSRV 1.87 (zbus 5 requires it); `atvv-protocol` is I/O-free.
- Arch Linux packaging only; Debian/Fedora/Nix/Flatpak are out.
- PipeWire service restart belongs to systemd (`PartOf=pipewire.service`,
  `Restart=always`), not to `atvv-audio`. The unit exists but is not installed
  or verified until step 09; it cannot run before step 07 adds the BlueZ
  transport.
- ASR is external; no unofficial cloud protocol in the core.
- Device facts live in `DeviceProfile`; RC003 profile id is
  `miremote-rc003`. Button keys and gesture timing are profile fields.
- Input actions use the documented grammar; power defaults to `none` so logind
  policy is not chosen implicitly.
- The fake remote profile (`test-fake-remote`, VID `0x1209`, PID `0x0002`) is
  test-only and never part of `PROFILES`.
- D-Bus boundary: `org.atvv.Daemon` at `/org/atvv/Daemon`; `GetStatus` returns
  JSON from the `StatusView` model; `ListDevices` returns addresses;
  `StartVoice`, `StopVoice`, `ToggleVoice`, `ReloadConfig`; signals
  `StateChanged(s)`, `VoiceStarted()`, `VoiceStopped()`. Protocol structs do
  not cross D-Bus.
- The D-Bus object is registered before the well-known name is claimed;
  claiming first allowed a client to call an unregistered object and wait
  forever.
- `StartVoice` sends `MIC_OPEN`; the mock answers with a direct `AUDIO_START`.
  A device `AUDIO_STOP` sends one `MIC_CLOSE`; the mock echo is ignored by
  `Session`. `Transport` writes are rejected while disconnected.
- Mock control messages take priority over generated audio frames so stop and
  disconnect are not delayed by the frame stream.
- Tests use `PcmSink` implementations instead of requiring PipeWire; the real
  `PipeWireSink` is verified separately with `pw-cli`/`pw-record`.
- Config lives at `$XDG_CONFIG_HOME/atvv/daemon.toml` (or
  `~/.config/atvv/daemon.toml`): optional `gain_db` (±24) and `input`.
  Missing file = defaults; invalid file is reported by `ReloadConfig` and
  `atvvctl doctor`.

## Verified

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p atvv-daemon
```

- `cargo test --workspace` → 88 passed, 1 ignored.
- `cargo test -p atvv-daemon` → 13 unit tests and the isolated-bus integration
  test passed; the integration test starts its own `dbus-daemon`.
- Integration test: direct `AUDIO_START` → `VoiceStarted` + streaming frames;
  device `AUDIO_STOP` → exactly one `0x0D`, echo 150 ms later ignored;
  D-Bus `StartVoice`/`StopVoice` → one `MIC_OPEN`/`MIC_CLOSE`; introspection
  lists every method and signal; `StateChanged`/`VoiceStarted`/`VoiceStopped`
  arrive on the isolated bus; `ReloadConfig` applied `gain_db = 6`.
- `dbus-run-session -- target/debug/atvvd --mock --verbose` +
  `busctl --user call ... GetStatus/StartVoice` + `atvvctl status/mic stop`:
  state `ready` → `opening` → `streaming` (21 frames, 5040 samples) → `ready`.
- `atvvctl doctor` against the mock daemon: `daemon ok`, `source ok`,
  `pipewire ok`, `uinput ok`, `config warn` (missing file, defaults).
- `pw-cli info atvv-miremote-rc003` → `Audio/Source`, 16 kHz, virtual.
  `pw-record --target atvv-miremote-rc003 --rate 16000 --channels 1 --format
  s16` → 1.70 s WAV; `sox ... stat` RMS amplitude 0.366.
- `cargo test -p atvv-input hold_and_release_reach_the_kernel -- --ignored`
  (step 05) → passed: uinput hold pressed and release lifted `KEY_LEFTALT`.
- `atvvctl audio test --seconds 5` + `pw-record --target atvv-test` (step 04)
  captured 16 kHz mono s16le with amplitude ≈ 0.2.
- `pw-cli destroy <node>` recreated the source (step 04).
- Full `systemctl --user restart pipewire` is not verified; the unit is not
  installed. See ADR 0005.

## Next step

**07 rc003-hardware-bringup** is the next step but is blocked on an RC003.
When a device is available:

```bash
# run from the repository root
cat docs/research/README.md
cat docs/plans/07-rc003-hardware-bringup.md
```

Implement `atvv-bluez` behind the existing `Transport` trait, add pairing, and
record real CAPS/audio/echo/reconnect evidence. Do not change the daemon's
session path; the mock and BlueZ transports must share it.

## Commands

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -q -p atvv-cli -- profile show miremote-rc003
cargo run -q -p atvv-cli -- simulate miremote-rc003
cargo run -q -p atvv-cli -- audio test --seconds 3
cargo run -q -p atvv-cli -- input fake-remote
cargo run -q -p atvv-cli -- input fake-remote --hold 30
dbus-run-session -- cargo run -q -p atvv-daemon -- --mock --verbose
cargo run -q -p atvv-cli -- status
cargo run -q -p atvv-cli -- mic start
cargo run -q -p atvv-cli -- mic stop
cargo run -q -p atvv-cli -- mic toggle
cargo run -q -p atvv-cli -- reload
cargo run -q -p atvv-cli -- doctor
cargo test -p atvv-input hold_and_release_reach_the_kernel -- --ignored
```

## Rules

- No kernel/BlueZ/PipeWire patches.
- No root daemon; systemd user service + udev `uaccess`.
- No hardware claim without a recorded run.
- No push without explicit approval.
- Update the step file and plan index in the same commit as code.
