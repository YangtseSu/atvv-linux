# atvv-linux

**English** | [简体中文](README.zh-CN.md)

ATVV v1.0 userspace stack for Linux: BLE voice remotes as a PipeWire
microphone, with evdev/uinput button mapping and a D-Bus control surface.

This is not a kernel driver, a BlueZ plugin, or a PipeWire module. BlueZ owns
pairing, GATT, and HID; a userspace daemon speaks ATVV; PipeWire owns the audio
graph; evdev/uinput owns input remapping.

## Status

ATVV v1.0 capture, input remapping, the daemon and D-Bus API, the CLI, the MCP
server, and the optional GTK configuration app are implemented. The RC003
audio path and the fcitx5-vinput source path have been verified on real devices
and system services; evidence is linked below.

Automatic fcitx5-vinput setup depends on an upstream consumer change. Until it
lands, select the ATVV source by name and add the declared trigger key
manually; see the
[fcitx5-vinput guide](docs/integrations/fcitx5-vinput.md).

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
- `atvv-config`: the single `daemon.toml` schema parser, the versioned JSON
  configuration document, per-button action validation, gesture overrides,
  and the `exec:` policy. Live changes are applied through `atvvctl config`
  or the D-Bus `SetConfig` method.
- `atvv-bluez`: BlueZ GATT transport (`bluer`), adapter/device selection,
  `AcquireNotify` with a `StartNotify` fallback, a pairing agent, and
  reconnect handling.
- `atvv-daemon` / `atvvd`: session orchestration behind a `Transport` trait,
  the mock and BlueZ transports, timers, PCM decoding, the resident PipeWire
  source, voice-edge mic control, trigger-key and `org.fcitx.Vinput` modes, the
  input layer, and a JSON view model over `org.atvv.Daemon`.
- `atvvctl`: profile inspection, CAPS decoding, a hardware-free RC003 session
  simulation, an audio-source smoke test, the fake input remote, plus
  `status`, `contract`, `mic start|stop|toggle`, `reload`, `config
  get|set`, `capture`, `map test`, `pair`, `record`, `monitor`, and `doctor`.
- `atvv-mcp`: stdio MCP server with `status`, `contract`, `voice_start`,
  `voice_stop`, `voice_toggle`, `list_devices`, and `reload`.
- `atvv-gui` / `atvv-config`: GTK4 + libadwaita configuration app with a
  clickable remote diagram, per-slot action editor, keyboard and remote key
  learning, recording-key output selection, test mode, and a script policy.
  The interface follows the desktop locale: English, or Simplified Chinese
  for `zh*` locales. It is a D-Bus client only; see [`docs/gui.md`](docs/gui.md).

The first profile is the Xiaomi Bluetooth Remote 2 Pro (`miremote-rc003`,
ATVV v1.0, 16 kHz ADPCM, 120-byte frames). The 2026-10-30 run
(`C0:5D:39:F8:2D:4C`) verified real CAPS, audio, keepalive, and reconnect; see
[`docs/research/rc003-hardware-evidence.md`](docs/research/rc003-hardware-evidence.md)
for the exact evidence and remaining gaps (pairing gesture, intelligibility,
installed-unit PipeWire restart).

## Install (Arch)

Requirements: Arch Linux, a systemd user session, PipeWire, BlueZ, and (for
voice-key integration) fcitx5-vinput. The daemon is a user service; never run
it as root. The optional `atvv-gui` package additionally needs GTK4,
libadwaita, and librsvg.

The AUR packages are built from the release source archive:

```sh
git clone https://aur.archlinux.org/atvv.git
cd atvv
makepkg -si

# optional GUI:
git clone https://aur.archlinux.org/atvv-gui.git
cd atvv-gui
makepkg -si
```

Install `atvv` before `atvv-gui`; an AUR helper resolves that dependency. The
GUI app is optional; the daemon and CLI do not depend on GTK.

Pair the remote, then enable the user service:

```sh
atvvctl pair C0:5D:39:F8:2D:4C
systemctl --user daemon-reload
systemctl --user enable --now atvvd
atvvctl doctor
```

`atvvd` exposes `atvv-miremote-rc003` through PipeWire. Select it in
fcitx5-vinput by name (see [`docs/integrations/fcitx5-vinput.md`](docs/integrations/fcitx5-vinput.md));
do not make it the system default.

Button mappings, gesture timing, `exec:` opt-in, and the live `SetConfig`
path are documented in [`docs/configuration.md`](docs/configuration.md).

The package installs `/usr/bin/{atvvd,atvvctl,atvv-mcp}`;
`/usr/lib/systemd/user/atvvd.service`;
`/usr/lib/udev/rules.d/70-atvv-linux.rules` (`uaccess` for the RC003 event
nodes and `/dev/uinput`); `/usr/lib/modules-load.d/atvv-linux.conf` (`uinput`);
man pages; and bash/zsh/fish completions. No system-wide D-Bus policy is
installed: the daemon uses the session bus.

## Troubleshooting

| Symptom | Check |
|---|---|
| `cannot connect to the session D-Bus` | `systemctl --user status atvvd`; `systemctl --user enable --now atvvd` |
| `The name is not activatable` | `atvvd` is not running; check `systemctl --user status atvvd`. `0.1.0-2` keeps the daemon up while the remote is disconnected and retries the link. |
| `no device address` | `atvvctl pair <MAC>` writes `~/.config/atvv/daemon.toml` |
| Source missing | `pw-cli info atvv-miremote-rc003`; `atvvctl doctor` |
| `uinput` not writable or keys are not grabbed | `/dev/uinput` exists and is writable; `uinput` module loaded; re-login after installing the udev rule |
| Remote not connected or CAPS missing | `bluetoothctl info <MAC>`; `atvvctl doctor`; `atvvctl monitor` |
| Voice key does not trigger fcitx | The daemon defaults to `trigger_key = "DICTATE"`. Keep the user's `[TriggerKey]` entries and add `1=Dictate`; stop fcitx5 before editing and restart it after. A commented `[TriggerKey]` section disables the trigger. `F24` is the fallback key. |
| F5 reaches the focused application | The RC003 node is not grabbed; check `input.grabbed` in `atvvctl status` and close other grabbers |
| No source after a PipeWire restart | `PartOf=pipewire.service` restarts `atvvd`; check `atvvctl status` and `pw-cli info` |
| Source exists but fcitx does not list it | restart `vinput-daemon` after the source appears |

Device support and the real-device record are in
[`docs/device-support.md`](docs/device-support.md) and
[`docs/research/rc003-hardware-evidence.md`](docs/research/rc003-hardware-evidence.md).

## Known defects

Open correctness issues found during real-device testing on 2026-11-01. Each
is independent of the device that exposed it.

- **`output_frame` panics on `i16::MIN`.** `crates/atvv-daemon/src/daemon.rs`
  takes `sample.abs()` for the verbose first-frame peak. `i16::abs()` panics on
  `-32768`, so `atvvd --verbose` aborts the moment a clipped frame arrives.
  Reproduced with `atvvd --device <MAC> --verbose` plus `atvvctl mic start`;
  the process dies with `attempt to negate with overflow` at `daemon.rs:613`.
  In release builds the same expression wraps silently and returns `-32768`,
  so the reported peak is negative instead of panicking; both behaviours were
  confirmed with a standalone `i16::MIN.abs()` check. The fix is to widen to
  `i32` before `abs` (or use `unsigned_abs`), and to keep the diagnostic path
  free of arithmetic that can panic.
- **`CAPS_RESP` is not authoritative for the sample rate.**
  `crates/atvv-daemon/src/main.rs` builds the PipeWire source from
  `profile.sample_rate`, and `crates/atvv-cli/src/main.rs` `record` writes the
  WAV at `RC003.sample_rate`. The daemon parses `caps.codec` and reports
  `adpcm-8k` in `atvvctl status`, but the audio graph stays at 16 kHz. A remote
  that negotiates 8 kHz therefore plays at double speed with underrun gaps;
  measured fill ratio was 43.9 % against the 52.2 % that a 268-sample frame
  implies. ADR-worthy follow-up: derive the source rate from the negotiated
  codec and resample when it disagrees with the profile, or renegotiate the
  PipeWire node when `CAPS_RESP` arrives. `atvv-audio` has no resampling path
  today.
- **`AUDIO_SYNC` shorter than 7 bytes is dropped without a bounded-rate
  diagnostic.** `crates/atvv-protocol/src/events.rs` requires
  `data.len() >= 7` for opcode `0x0A` and otherwise returns
  `CtlEvent::Unknown`. The daemon then prints one `unknown control
  notification` line per packet under `--verbose` (151 lines in ~2 s observed),
  which violates the "log protocol anomalies at a bounded rate" rule in
  `AGENTS.md`. A malformed or firmware-truncated sync also leaves the decoder
  permanently stale, because `0x0A` is the only resynchronization mechanism in
  v1.0. The fix is a dedicated error variant plus a rate-limited warning.
- **`DeviceProfile::stop_echo` and `direct_audio_start` are never read by the
  daemon.** Both fields are printed by `atvvctl profile show` and set by the
  RC003 profile and the fake remote, but no control-flow branch consumes them;
  the stop-echo behavior is unconditional in `Session::on_audio_stop`. Either
  wire them into the session state machine or delete them, so the profile
  stays the single source of device facts.

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
cargo run -q -p atvv-cli -- config get --json
cargo run -q -p atvv-cli -- config set --button up --trigger single --action key:KEY_F13
cargo run -q -p atvv-cli -- map test --button up --trigger single
cargo run -q -p atvv-cli -- capture --timeout 10
cargo run -q -p atvv-cli -- doctor
cargo run -q -p atvv-cli -- pair C0:5D:39:F8:2D:4C
cargo run -q -p atvv-daemon -- --device C0:5D:39:F8:2D:4C --verbose
cargo run -q -p atvv-cli -- record --wav /tmp/rc003.wav --seconds 10
cargo run -q -p atvv-cli -- monitor
cargo run -q -p atvv-mcp
cargo run -q -p atvv-gui
```

## Layout

```text
crates/atvv-protocol/   protocol, ADPCM, frames, profiles, session state
crates/atvv-config/     daemon.toml schema, validation, JSON config document
crates/atvv-audio/      native PipeWire Audio/Source and PCM queue
crates/atvv-input/      evdev discovery/grab, gestures, uinput keyboard
crates/atvv-bluez/      BlueZ GATT transport and pairing agent
crates/atvv-daemon/     atvvd session daemon, mock transport, D-Bus API
crates/atvv-cli/        atvvctl diagnostics and D-Bus control
crates/atvv-mcp/        stdio MCP server for agents
crates/atvv-gui/        GTK4/libadwaita configuration app (atvv-config)
packaging/              systemd user unit, udev rule, GUI desktop metadata
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

## Acknowledgements

This project referenced several open-source projects during development.
Sources, pinned snapshots, licenses, and reuse boundaries are listed in
[`docs/research/reference-projects.md`](docs/research/reference-projects.md).

## License

GPL-3.0-or-later. See `LICENSE`.
