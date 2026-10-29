# Handoff

Updated: 2026-10-30

## Project

- Path: `~/projects/atvv-linux`
- License: GPL-3.0-or-later
- Git: local `main`; **not pushed**.
- Handoff base commit: `837df6f Add handoff for the next session` (this handoff update is the next commit).
- Tests: `cargo test --workspace` → 36 passed (5 suites).

## Read first

1. `AGENTS.md` — hard rules and commands.
2. `docs/plans/README.md` — canonical step index and M0 exit criteria.
3. `docs/decisions/0005-pipewire-restart.md` — systemd owns PipeWire restart.
4. The next step file before editing code.

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
  systemd; the unit is not implemented yet.

## Not implemented

- **05 input/uinput**: evdev, grab, gestures, uinput, fake remote.
- **06 daemon/D-Bus**: `atvvd`, mock BLE, D-Bus API, systemd unit.
- **07 RC003 hardware**: real pairing/GATT/CAPS/audio/reconnect.
- **08 fcitx5-vinput/MCP**: trigger key, D-Bus bridge, MCP.
- **09 Arch packaging**: PKGBUILD, udev, systemd, acceptance.

No RC003 is available. Do not claim hardware support. Step 07 and the hardware
acceptance part of step 09 are blocked on a device.

## Decisions

- Userspace stack only: BlueZ + daemon + PipeWire + evdev/uinput.
- ATVV v1.0+ only; v0.4 is out of scope.
- Rust 2024; `atvv-protocol` is I/O-free.
- Arch Linux packaging only; Debian/Fedora/Nix/Flatpak are out.
- PipeWire service restart belongs to systemd (`PartOf=pipewire.service`,
  `Restart=always`), not to `atvv-audio`.
- ASR is external; no unofficial cloud protocol in the core.
- Device facts live in `DeviceProfile`; RC003 profile id is
  `miremote-rc003`.

## Verified

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- `atvvctl audio test --seconds 5` + `pw-record --target atvv-test` captured
  16 kHz mono s16le with amplitude ≈ 0.2 (tone present).
- `pw-cli destroy <node>` recreated the source.
- Full `systemctl --user restart pipewire` was not recovered in-process; see
  ADR 0005.

## Next step

Start **05 input-uinput**:

```bash
cd ~/projects/atvv-linux
cat docs/plans/05-input-uinput.md
```

Deliver a fake uinput remote, evdev discovery/grab, key map, gesture FSM, and
uinput emission. Test with `evtest`/`libinput`; do not grab a real keyboard.

## Commands

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -q -p atvv-cli -- profile show miremote-rc003
cargo run -q -p atvv-cli -- simulate miremote-rc003
cargo run -q -p atvv-cli -- audio test --seconds 3
```

## Rules

- No kernel/BlueZ/PipeWire patches.
- No root daemon; systemd user service + udev `uaccess`.
- No hardware claim without a recorded run.
- No push without explicit approval.
- Update the step file and plan index in the same commit as code.
