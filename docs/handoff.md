# Handoff

Updated: 2026-10-30

## Project

- License: GPL-3.0-or-later
- Git: `main` tracks `origin/main`; check `git status` for unpushed commits. Do not push without explicit approval.
- Latest commit: run `git log -1 --oneline` (do not hard-code a hash that this file would invalidate).
- Tests: `cargo test --workspace` → 96 passed (7 test binaries),
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
- **07 RC003 hardware**: `atvv-bluez` (`bluer`) with adapter selection,
  connect, ATVV service/characteristic lookup, AcquireNotify with StartNotify
  fallback, auto-confirm pairing agent, link generation and reconnect,
  `atvvctl pair|record|monitor`, and real-device evidence. RC003 run
  (`C0:5D:39:F8:2D:4C`): CAPS `0b 01 00 02 03 00 78 00 00`, direct
  `AUDIO_START`, 120-byte frames, `AUDIO_STOP 00 02`/`00 08`, six keepalives
  over a 60 s hold, non-silent 16 kHz WAV, auto-reconnect. Evidence:
  `docs/research/rc003-hardware-evidence.md`.

## Not implemented

- **08 fcitx5-vinput/MCP**: trigger key, D-Bus bridge, MCP. Unblocked by 07;
  the RC003 microphone only produces frames while the voice key is held.
- **09 Arch packaging**: PKGBUILD, udev, installed systemd unit, CI, hardware
  acceptance. Depends on all prior steps; the installed-unit PipeWire restart
  check is still open.

Do not claim unverified hardware behavior (pairing gesture, adapter
power-cycle, intelligibility). Step 07 verified the fields recorded in
`docs/research/rc003-hardware-evidence.md`.

## Decisions

- Research basis, reference snapshots, licenses, and open hardware questions:
  `docs/research/`.
- Userspace stack only: BlueZ + daemon + PipeWire + evdev/uinput.
- ATVV v1.0+ only; v0.4 is out of scope.
- Rust 2024; MSRV 1.87 (zbus 5 requires it); `atvv-protocol` is I/O-free.
- Arch Linux packaging only; Debian/Fedora/Nix/Flatpak are out.
- PipeWire service restart belongs to systemd (`PartOf=pipewire.service`,
  `Restart=always`), not to `atvv-audio`. The unit exists but is not installed
  or verified until step 09. The in-process source does not recover from a
  full PipeWire daemon restart; it is recreated on daemon start.
- The resident source sets `node.pause-on-idle=false` so a linked consumer
  wakes it without a playback driver. `node.always-process` is not used: it
  freewheels and drains the queue while unlinked.
- `atvv-bluez` owns a private Tokio runtime and exposes a synchronous
  `BluezLink`; `atvv-daemon` adapts it to `Transport`. Notifications use
  `AcquireNotify` with a recorded `StartNotify` fallback. Reconnects invalidate
  the previous notification generation so stale EOF/errors cannot publish
  events.
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
  `~/.config/atvv/daemon.toml`): optional `device`, `adapter`, `gain_db`
  (±24), and `input`. `atvvctl pair` writes `device` and preserves the rest.
  Missing file = defaults; invalid file is reported by `ReloadConfig` and
  `atvvctl doctor`.
- On the RC003, the physical voice key gates microphone data: a host
  `MIC_OPEN` produces `AUDIO_START` but no audio frames. Step 08 must
  synthesize the key for fcitx; `StartVoice` alone is not enough for audio.
- CI installs `libpipewire-0.3-dev`, `libspa-0.2-dev`, `libclang-dev`, and
  `dbus` on `ubuntu-latest`: the workspace builds a native PipeWire client and
  `libspa-sys` runs bindgen.

## Verified

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p atvv-daemon
```

- `cargo test --workspace` → 96 passed, 1 ignored.
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
  `pw-cat --record --target atvv-miremote-rc003` captured 16 kHz mono s16le.
- Step 07 hardware run (2026-10-30, RC003 `C0:5D:39:F8:2D:4C`, `hci0`):
  `atvvctl pair` → paired/trusted and saved to config; `atvvctl doctor` all
  `ok`; real CAPS `0b 01 00 02 03 00 78 00 00` (v1.0, ADPCM 16 kHz, HTT,
  120 bytes); `AUDIO_START 04 03 02 03`; `AUDIO_STOP 00 02` and `00 08`;
  3,954 frames / 948,960 samples over a ~60 s hold with six `MIC_EXTEND`
  sends; 65 s WAV at 16 kHz, max 0.999969, RMS 0.356024.
- `atvvctl record --wav` uses `pw-cat --record`; a non-zero pw-cat exit is
  accepted when the WAV contains data. `atvvctl monitor` printed
  `opening`/`voice started`/`streaming`/`voice stopped`/`ready`.
- Reconnect after `bluetoothctl disconnect` reached `ready` with a fresh CAPS
  without a daemon restart; an eager first attempt can drop again and retry.
- Virtual-source scheduling: with no playback driver,
  `pw-cat --record --target atvv-test` captured the synthetic tone
  (max 0.2009, RMS 0.1222) after `node.pause-on-idle=false`. Without it the
  node stayed suspended and consumers recorded silence.
- `systemctl --user restart pipewire` → the in-process source was not
  recreated; a daemon restart restored it. The installed-unit path belongs to
  step 09 (ADR 0005).
- GitHub Actions `ci` for `2c0c435` → success (fmt, clippy, workspace tests);
  the runner installs `libpipewire-0.3-dev`, `libspa-0.2-dev`, `libclang-dev`,
  and `dbus`.
- `cargo test -p atvv-input hold_and_release_reach_the_kernel -- --ignored`
  (step 05) → passed: uinput hold pressed and release lifted `KEY_LEFTALT`.
- `atvvctl audio test --seconds 5` + `pw-record --target atvv-test` (step 04)
  captured 16 kHz mono s16le with amplitude ≈ 0.2.
- `pw-cli destroy <node>` recreated the source (step 04).
- Full `systemctl --user restart pipewire` is not verified; the unit is not
  installed. See ADR 0005.

## Next step

**08 vinput-and-mcp** is next. Read:

```bash
# run from the repository root
cat docs/research/README.md
cat docs/plans/08-vinput-and-mcp.md
```

Keep the RC003 key gating in mind: the physical voice key must be synthesized
for fcitx; a host `StartVoice`/`MIC_OPEN` produces `AUDIO_START` but no audio
frames. Do not add an in-process PipeWire service-restart monitor; the unit
owns that (ADR 0005).

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
cargo run -q -p atvv-cli -- pair C0:5D:39:F8:2D:4C
cargo run -q -p atvv-daemon -- --device C0:5D:39:F8:2D:4C --verbose
cargo run -q -p atvv-daemon -- --verbose   # device from daemon.toml
cargo run -q -p atvv-cli -- monitor
cargo run -q -p atvv-cli -- record --wav /tmp/rc003.wav --seconds 10
cargo test -p atvv-input hold_and_release_reach_the_kernel -- --ignored
```

## Rules

- No kernel/BlueZ/PipeWire patches.
- No root daemon; systemd user service + udev `uaccess`.
- No hardware claim without a recorded run.
- No push without explicit approval.
- Update the step file and plan index in the same commit as code.
