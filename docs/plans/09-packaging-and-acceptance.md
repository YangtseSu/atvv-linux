# Step 09 — packaging and acceptance

Status: 🚧 in-progress
Depends on: all of 01–08
Touches: `packaging/`, `docs/`, `README.md`, `.github/workflows/`

## Goal

A packageable, permission-correct, reproducible release with a recorded
hardware acceptance run.

## Deliverables

- ✅ systemd user service `atvvd.service`; `After=bluetooth.target
  pipewire.service`, `Wants=pipewire.service`, `PartOf=pipewire.service`,
  `Restart=always`.
- ✅ udev rules using `TAG+="uaccess"` for `/dev/uinput` and RC003 event nodes;
  `modules-load.d/atvv-linux.conf` loads `uinput`.
- ✅ No D-Bus policy: the daemon is a session-bus service and Arch's default
  session policy is sufficient. The absence is documented in
  `packaging/arch/README.md`; no broad `/etc/dbus-1/system.d` file was added.
- ✅ Arch PKGBUILD only. Debian, Fedora, Nix, and Flatpak are explicitly out of
  scope.
- ✅ `cargo-deny` with `GPL-3.0-or-later`, MIT, Apache-2.0, BSD-2-Clause allowed;
  compatible BSD-3-Clause, ISC, Unicode-3.0, and the LLVM exception are also
  allowed for real transitive dependencies. `GPL-3.0-only` is deliberately
  absent from the allow-list and requires relicensing the whole work. Three
  zbus-5 unmaintained advisories are ignored with a zbus-6 migration note.
- ✅ SPDX/REUSE check; `LICENSE`, `LICENSES/GPL-3.0-or-later.txt`, and
  `THIRD-PARTY-NOTICES.md`.
- ✅ Man pages (generated from clap), bash/zsh/fish completions, README
  install/troubleshooting, and a device support matrix.
- ⬜ Hardware acceptance record: device model/address, CAPS, 16 kHz capture,
  key mapping, fcitx5-vinput output, reconnect, PipeWire restart, 60-second
  keepalive, and package install. The record exists and the daemon/source/
  reconnect/PipeWire-restart parts are verified; package install, post-reboot,
  and the end-to-end fcitx text commit are pending.
- ✅ CI: fmt, clippy, test, cargo-deny, REUSE.
- ✅ Dependabot: daily Cargo and GitHub Actions checks that open grouped
  update PRs automatically; no auto-merge.

## Design notes

- Research: `docs/research/reference-projects.md` §3 and
  `docs/research/landscape.md` §4.
- No root daemon, no custom kernel/BlueZ/PipeWire build.
- Do not add `EnableExperimental` or `audio` group membership by default.
- The udev rule matches VID/PID and `MI RC*`, not only `*RC003*`; the actual
  evdev name may not contain `RC003`. The measured RC003 is `2717:32b8` and
  reports a localized name, so the id match is the authoritative one.
- The package must install the CLI and daemon; the GUI, if any, is optional.
- A release is not "hardware supported" until the acceptance record exists.
- Version `0.1.0` is the first packaged release. The Arch source tarball is
  deterministic (`packaging/arch/make-source.sh`); `packaging/arch` is
  export-ignored, so pinning its checksum does not change the archive.
- `atvvctl manpages` and `atvvctl completions` are the single source of truth
  for the installed documentation; `atvvd`'s clap definition lives in
  `atvv-daemon/src/cli.rs` so its man page cannot drift.
- `atvv-linux` links `libpipewire`; `pipewire` is a runtime dependency for the
  daemon and its client library. `bluez` and `dbus` are runtime services, not
  link-time libraries; `namcap` reports them as "may not be needed", which is
  expected.

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
udevadm verify packaging/udev/70-atvv-linux.rules
makepkg -f                          # in packaging/arch
systemd-analyze verify packaging/systemd/atvvd.service   # after install, or with a temp copy whose ExecStart is the built binary
sudo pacman -U atvv-linux-0.1.0-2-x86_64.pkg.tar.zst
udevadm test /sys/class/input/eventX  # root; after the rule is installed
```

Then install from the PKGBUILD on a clean machine and run the hardware
acceptance commands from step 07. `systemd-analyze verify` and `udevadm test`
need the installed paths or root; the syntax checks above do not.

### Recorded results — 2026-10-30

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace`: 114 passed, 2 ignored (uinput kernel tests).
- `cargo deny check`: advisories, bans, licenses, sources all ok. Three zbus 5
  unmaintained advisories (`RUSTSEC-2026-0370/0371/0372`) are ignored with a
  documented zbus 6 migration note.
- `reuse lint`: compliant with REUSE 3.3; 92 files with copyright/license
  information.
- `udevadm verify packaging/udev/70-atvv-linux.rules`: 1 rule file, success.
- `systemd-analyze verify --user` with a temp copy whose `ExecStart` is
  `target/release/atvvd`: pass.
- `makepkg -f`: built `atvv-linux 0.1.0-1` (5.8 MiB) and a debug package;
  after the connect fix, rebuilt `atvv-linux 0.1.0-2` with the same contents.
  Contents: three binaries, user unit, udev rule, modules-load file, 24 man
  pages, bash/zsh/fish completions, LICENSE, third-party notices, README,
  device-support matrix.
- Staged install with `fakeroot pacman --root /tmp/atvv-pacroot ... -U
  --assume-installed pipewire --assume-installed bluez --assume-installed dbus`:
  installed; packaged `atvvctl --version` reports `0.1.0`.
- Packaged `atvvd` started as a transient user service from the staging root;
  `atvvctl doctor` all `ok`. A temporary copy of the packaged user unit with
  `ExecStart` pointing at the staged binary was `enable`d and `start`ed, then
  removed.
- Post-reboot fix: the installed `0.1.0-1` unit looped on
  `org.bluez.Error.InProgress`. `0.1.0-2` uses a lazy `BluezTransport`,
  handles `InProgress`/`AlreadyConnected` as pending/success, and bounds the
  whole connect. With the remote disconnected, `atvvctl doctor` reports
  `daemon`, `source`, and `pipewire` ok; the daemon stays active and retries
  instead of exiting.
- Real RC003 (`C0:5D:39:F8:2D:4C`, `hci0`): `atvvctl doctor` all `ok`; CAPS
  `0b 01 00 02 03 00 78 00 00`; source `Audio/Source` 16 kHz; 1 s capture
  16,000 samples; `AUDIO_START 04 00 02 00` then frame timeout without the key;
  reconnect after `bluetoothctl disconnect`; `systemctl --user restart
  pipewire.service` restarted the unit (`MainPID` 85063 → 87720) and recreated
  the source at 16 kHz.
- 60-second keepalive and key mapping are the step 07/08 real-device results
  recorded in `docs/research/rc003-hardware-evidence.md`; the step 09 run did
  not repeat the 60 s hold.
- Pending: `pacman -U` into `/usr` (root), post-reboot run, end-to-end fcitx
  text commit.

## Exit criteria

- ⬜ The Arch package installs and `atvvd` starts as the user. Staged install
  and packaged-binary user-service start pass; the host install needs root.
- ✅ `atvvctl doctor` reports all green on a real RC003.
- ⬜ RC003 works after reboot, reconnect, and PipeWire restart. Reconnect and
  PipeWire restart pass; reboot is pending.
- ⬜ fcitx5-vinput receives text from voice input. The source and trigger legs
  are verified; the joint text commit is pending.
- ✅ CI and license checks are clean locally (`cargo deny`, `reuse lint`); the
  GitHub Actions run starts on the next push.
- ⬜ The hardware acceptance record is signed and dated. The record is drafted
  and dated; operator sign-off and the three pending checks remain.

## Risks

- Arch udev/D-Bus policy differences; only Arch is supported by this plan.
- GPU/GUI packaging is separate; do not block the core release on it.
- A package without a hardware acceptance record is not a support claim.
- zbus 5's split crates are unmaintained upstream; zbus 6 merges them. Migrate
  in a dedicated dependency step, not inside the packaging step.

## Progress log

- 2026-10-29 — step planned; no implementation yet.
- 2026-10-30 — implemented version 0.1.0, the user unit, udev and
  modules-load files, PKGBUILD, deterministic source script, generated man
  pages/completions, cargo-deny, REUSE, third-party notices, CI, Dependabot,
  README install/troubleshooting, device support matrix, and the acceptance
  record. Built and staged-installed the package; verified the packaged binary
  as a user service; ran the real-device daemon/source/reconnect/PipeWire
  restart checks.
- 2026-10-30 — remaining: root install, post-reboot check, end-to-end fcitx
  text commit, and operator sign-off. Step stays in progress.
- 2026-10-30 — post-reboot regression on the installed `0.1.0-1` unit:
  `org.bluez.Error.InProgress` made `atvvd` exit and systemd restart-loop it,
  so `org.atvv.Daemon` was not activatable. Added a lazy BlueZ transport,
  retryable `InProgress`/`AlreadyConnected` handling, and a bounded connect;
  bumped `pkgrel` to 2. Verified with the release binary while the remote was
  disconnected (`daemon`/`source`/`pipewire` ok, daemon stays active). The
  remote still needs to be woken for the physical link.
