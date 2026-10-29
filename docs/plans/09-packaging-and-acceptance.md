# Step 09 — packaging and acceptance

Status: ⬜ not-started
Depends on: all of 01–08
Touches: `packaging/`, `docs/`, `README.md`, `.github/workflows/`

## Goal

A packageable, permission-correct, reproducible release with a recorded
hardware acceptance run.

## Deliverables

- ⬜ systemd user service `atvvd.service`; `After=bluetooth.target
  pipewire.service`, `Wants=pipewire.service`, `PartOf=pipewire.service`,
  `Restart=always`.
- ⬜ udev rules using `TAG+="uaccess"` for `/dev/uinput` and RC003 event nodes;
  `modules-load.d/atvv-linux.conf` loads `uinput`.
- ⬜ Optional minimal D-Bus policy only if a target distribution requires it;
  no broad `/etc/dbus-1/system.d` policy.
- ⬜ Arch PKGBUILD only. Debian, Fedora, Nix, and Flatpak are explicitly out of
  scope.
- ⬜ `cargo-deny` with `GPL-3.0-or-later`, MIT, Apache-2.0, BSD-2-Clause
  allowed; explicit rejection of `GPL-3.0-only` unless the project is
  deliberately relicensed.
- ⬜ SPDX/REUSE check; `LICENSE` and third-party notices.
- ⬜ Man page, shell completions, README install/troubleshooting, and a device
  support matrix.
- ⬜ Hardware acceptance record: device model/address, CAPS, 16 kHz capture,
  key mapping, fcitx5-vinput output, reconnect, PipeWire restart, 60-second
  keepalive, and package install.
- ⬜ CI: fmt, clippy, test, cargo-deny, REUSE.

## Design notes

- No root daemon, no custom kernel/BlueZ/PipeWire build.
- Do not add `EnableExperimental` or `audio` group membership by default.
- The udev rule matches VID/PID and `MI RC*`, not only `*RC003*`; the actual
  evdev name may not contain `RC003`.
- The package must install the CLI and daemon; the GUI, if any, is optional.
- A release is not "hardware supported" until the acceptance record exists.

## Out of scope

Flatpak (Bluetooth/uinput access is not a good fit), Windows/macOS, and GUI
packaging before a GUI exists.

## Verification

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
reuse lint
systemd-analyze verify packaging/systemd/atvvd.service
udevadm test /sys/class/input/eventX
```

Then install from the PKGBUILD on a clean machine and run the hardware
acceptance commands from step 07.

## Exit criteria

- ⬜ The Arch package installs and `atvvd` starts as the user.
- ⬜ `atvvctl doctor` reports all green on a real RC003.
- ⬜ RC003 works after reboot, reconnect, and PipeWire restart.
- ⬜ fcitx5-vinput receives text from voice input.
- ⬜ CI and license checks are clean.
- ⬜ The hardware acceptance record is signed and dated.

## Risks

- Arch udev/D-Bus policy differences; only Arch is supported by this plan.
- GPU/GUI packaging is separate; do not block the core release on it.
- A package without a hardware acceptance record is not a support claim.

## Progress log

- 2026-10-29 — step planned; no implementation yet.
