# atvv-linux

ATVV v1.0 userspace stack for Linux: BLE voice remotes as a PipeWire
microphone, with evdev/uinput button mapping and a D-Bus control surface.

This is not a kernel driver, a BlueZ plugin, or a PipeWire module. BlueZ owns
pairing, GATT, and HID; a userspace daemon speaks ATVV; PipeWire owns the audio
graph; evdev/uinput owns input remapping.

## Status

**Steps 01–08 complete and verified; packaging (step 09) remains. The RC003
path and the fcitx5-vinput source path have both run on real system services.**

Implemented:

- `atvv-protocol`: v1.0 capability parsing, control events, ADPCM, frame
  accumulation, device profiles, and a pure session state machine.
- `atvv-audio`: native PipeWire `Audio/Source` with a bounded lock-free queue,
  synthetic-tone test, and node-removal recovery. The source wakes when a
  consumer links (`node.pause-on-idle=false`); full PipeWire service restart is
  delegated to the systemd unit (ADR 0005).
- `atvv-input`: profile-driven evdev discovery and `EVIOCGRAB`, the RC003
  gesture engine, a uinput virtual keyboard, and a configurable synthetic
  trigger key/chord for the grabbed voice key. `atvvctl input fake-remote`
  exercises the whole path without hardware.
- `atvv-bluez`: BlueZ GATT transport (`bluer`), adapter/device selection,
  `AcquireNotify` with a `StartNotify` fallback, a pairing agent, and
  reconnect handling.
- `atvv-daemon` / `atvvd`: session orchestration behind a `Transport` trait,
  the mock and BlueZ transports, timers, PCM decoding, the resident PipeWire
  source, voice-edge mic control, trigger-key and `org.fcitx.Vinput` modes, the
  input layer, and a JSON view model over `org.atvv.Daemon`.
- `atvvctl`: profile inspection, CAPS decoding, a hardware-free RC003 session
  simulation, an audio-source smoke test, the fake input remote, plus
  `status`, `mic start|stop|toggle`, `reload`, `pair`, `record`, `monitor`, and
  `doctor`.
- `atvv-mcp`: stdio MCP server with `status`, `voice_start`, `voice_stop`,
  `voice_toggle`, `list_devices`, and `reload`.

Not yet complete:

- Arch packaging and installed systemd/udev integration (step 09)

The first profile is the Xiaomi Bluetooth Remote 2 Pro (`miremote-rc003`,
ATVV v1.0, 16 kHz ADPCM, 120-byte frames). The 2026-10-30 run
(`C0:5D:39:F8:2D:4C`) verified real CAPS, audio, keepalive, and reconnect; see
[`docs/research/rc003-hardware-evidence.md`](docs/research/rc003-hardware-evidence.md)
for the exact evidence and remaining gaps (pairing gesture, intelligibility,
installed-unit PipeWire restart).

## Build and test

```sh
cargo test --workspace
cargo run -q -p atvv-cli -- profile show miremote-rc003
cargo run -q -p atvv-cli -- caps decode 0B010002000078
cargo run -q -p atvv-cli -- simulate miremote-rc003
cargo run -q -p atvv-cli -- audio test --seconds 3
cargo run -q -p atvv-cli -- input fake-remote
dbus-run-session -- cargo run -q -p atvv-daemon -- --mock
cargo run -q -p atvv-cli -- status
cargo run -q -p atvv-cli -- doctor
cargo run -q -p atvv-cli -- pair C0:5D:39:F8:2D:4C
cargo run -q -p atvv-daemon -- --device C0:5D:39:F8:2D:4C --verbose
cargo run -q -p atvv-cli -- record --wav /tmp/rc003.wav --seconds 10
cargo run -q -p atvv-cli -- monitor
cargo run -q -p atvv-mcp
```

## Layout

```text
crates/atvv-protocol/   protocol, ADPCM, frames, profiles, session state
crates/atvv-audio/      native PipeWire Audio/Source and PCM queue
crates/atvv-input/      evdev discovery/grab, gestures, uinput keyboard
crates/atvv-bluez/      BlueZ GATT transport and pairing agent
crates/atvv-daemon/     atvvd session daemon, mock transport, D-Bus API
crates/atvv-cli/        atvvctl diagnostics and D-Bus control
crates/atvv-mcp/        stdio MCP server for agents
packaging/              systemd user unit (step 09 installs it)
docs/plans/             canonical implementation plan and acceptance criteria
docs/decisions/         architecture decisions
docs/research/          landscape, protocol facts, reference snapshots
docs/integrations/      fcitx5-vinput and coding-agent setup
docs/handoff.md         current state for the next session
```

## Scope

In scope: ATVV v1.0+, Linux, BlueZ, PipeWire, evdev/uinput, D-Bus, fcitx5-vinput,
coding-agent integration, device profiles, packaging.

Out of scope for now: ATVV v0.4, ASR engines, unofficial cloud protocols,
non-ATVV transports, kernel/BlueZ/PipeWire patches, macOS/Windows.

## License

GPL-3.0-or-later. See `LICENSE`.
