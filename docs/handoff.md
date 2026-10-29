# Handoff

Updated: 2026-10-30

## Project

- License: GPL-3.0-or-later
- Git: `main` tracks `origin/main`; check `git status` for unpushed commits. Do not push without explicit approval.
- Latest commit: run `git log -1 --oneline` (do not hard-code a hash that this file would invalidate).
- Tests: `cargo test --workspace` → 74 passed (7 test binaries), 1 ignored
  kernel-uinput test.

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
  systemd; the unit is not implemented yet.
- **05 input/uinput**: `atvv-input` with profile key maps, pointer-safe evdev
  discovery, `EVIOCGRAB`, the RC003 gesture FSM (600 ms latch), action grammar,
  uinput virtual keyboard, and `atvvctl input fake-remote`.

## Not implemented

- **06 daemon/D-Bus**: `atvvd`, mock BLE, D-Bus API, systemd unit.
- **07 RC003 hardware**: real pairing/GATT/CAPS/audio/reconnect.
- **08 fcitx5-vinput/MCP**: trigger key, D-Bus bridge, MCP.
- **09 Arch packaging**: PKGBUILD, udev, systemd, acceptance.

No RC003 is available. Do not claim hardware support. Step 07 and the hardware
acceptance part of step 09 are blocked on a device.

## Decisions

- Research basis, reference snapshots, licenses, and open hardware questions:
  `docs/research/`.
- Userspace stack only: BlueZ + daemon + PipeWire + evdev/uinput.
- ATVV v1.0+ only; v0.4 is out of scope.
- Rust 2024; `atvv-protocol` is I/O-free.
- Arch Linux packaging only; Debian/Fedora/Nix/Flatpak are out.
- PipeWire service restart belongs to systemd (`PartOf=pipewire.service`,
  `Restart=always`), not to `atvv-audio`.
- ASR is external; no unofficial cloud protocol in the core.
- Device facts live in `DeviceProfile`; RC003 profile id is
  `miremote-rc003`. Button keys and gesture timing are profile fields.
- Input actions use the documented grammar; power defaults to `none` so logind
  policy is not chosen implicitly.
- The fake remote profile (`test-fake-remote`, VID `0x1209`, PID `0x0002`) is
  test-only and never part of `PROFILES`.
- The local `/dev/uinput` smoke used the `input` group; udev `uaccess` policy
  belongs to step 09.

## Verified

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- `cargo test --workspace` → 74 passed, 1 ignored.
- `cargo run -q -p atvv-cli -- input fake-remote` → grabbed only the fake node,
  recognized 12/12 logical buttons, emitted voice press/release, exit 0.
- `libinput debug-events --device /dev/input/event26` (during `--hold`) saw
  `atvv-virtual-keyboard` as `cap:k` and `KEY_UP` press/release every ~1 s.
- `cargo test -p atvv-input hold_and_release_reach_the_kernel -- --ignored`
  → passed: uinput hold pressed and release lifted `KEY_LEFTALT`.
- `atvvctl audio test --seconds 5` + `pw-record --target atvv-test` captured
  16 kHz mono s16le with amplitude ≈ 0.2 (tone present).
- `pw-cli destroy <node>` recreated the source.
- Full `systemctl --user restart pipewire` was not recovered in-process; see
  ADR 0005.

## Next step

Start **06 daemon-dbus**:

```bash
# run from the repository root
cat docs/research/README.md
cat docs/plans/06-daemon-dbus.md
```

Deliver `atvvd`, a mock BLE transport driving the existing `Session`, the
D-Bus API on `org.atvv.Daemon`, a systemd user unit, and integration of
`atvv-audio` plus `atvv-input` events. The mock must not become a second session
implementation.

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
cargo test -p atvv-input hold_and_release_reach_the_kernel -- --ignored
```

## Rules

- No kernel/BlueZ/PipeWire patches.
- No root daemon; systemd user service + udev `uaccess`.
- No hardware claim without a recorded run.
- No push without explicit approval.
- Update the step file and plan index in the same commit as code.
