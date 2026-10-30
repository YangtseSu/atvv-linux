# Handoff

Updated: 2026-10-30

## Project

- License: GPL-3.0-or-later
- Git: `main` tracks `origin/main`; check `git status` for unpushed commits. Do not push without explicit approval.
- Latest commit: run `git log -1 --oneline` (do not hard-code a hash that this file would invalidate).
- Tests: `cargo test --workspace` → 114 passed (12 test binaries),
  2 ignored kernel-uinput tests.

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
- **08 fcitx/MCP (complete and verified)**: `vinput` and `trigger_key` config
  (single key or `+` chord), voice-edge `MIC_OPEN`/`MIC_CLOSE`, synthetic
  trigger emission with lost-node release, optional `org.fcitx.Vinput` client
  with fast name-owner failure, `atvv-mcp` stdio server (`status`,
  `voice_start`, `voice_stop`, `voice_toggle`, `list_devices`, `reload`),
  isolated-bus mock tests, and `docs/integrations/`.
- **09 packaging (implemented, acceptance partial)**: version `0.1.0`; clap
  `completions`/`manpages`; `atvvd` CLI in `atvv-daemon/src/cli.rs`; packaged
  user unit, udev `uaccess` rule, `modules-load.d`; deterministic Arch source
  tarball and PKGBUILD; cargo-deny, REUSE, third-party notices; CI with
  fmt/clippy/test/cargo-deny/REUSE; daily Dependabot PRs; README
  install/troubleshooting; device support matrix; acceptance record
  `docs/acceptance/rc003-2026-10-30.md`. Built `atvv-linux 0.1.0-1`,
  staged-installed it, and found the post-reboot connect loop on the installed
  unit. `0.1.0-2` fixes it with a lazy BlueZ transport, `InProgress` handling,
  and a bounded connect; the packaged binary is rebuilt and verified with
  `atvvctl doctor` while the remote is disconnected. Remaining: root install,
  post-reboot run, end-to-end fcitx text commit, sign-off.

## Not implemented

- **09 remaining acceptance**: `sudo pacman -U` into `/usr`, a post-reboot
  `atvvctl doctor` + source check, and a live fcitx text commit. These need
  root/the operator and are event-driven; do not put them on a timer.
- **10 vinput zero-config (planned)**: upstream fcitx5-vinput
  `SetCaptureDevice` plus trigger get/set and scene-id PR, daemon
  auto-selection that preserves an explicit user device, and a default mode
  that never assumes `Alt_R` (`auto`/`dbus`). Until it lands, the one-time
  `vinput device use` step remains. See
  `docs/plans/10-vinput-zero-config.md`.
- zbus 6 migration (the zbus 5 split crates are unmaintained; denied
  advisories are documented in `deny.toml`). Do it as a separate dependency
  step.

Do not claim unverified hardware behavior (pairing gesture, adapter
power-cycle, ASR intelligibility, a root-installed unit after reboot). Step 07
verified the fields recorded in `docs/research/rc003-hardware-evidence.md`.

## Decisions

- Research basis, reference snapshots, licenses, and open hardware questions:
  `docs/research/`.
- Userspace stack only: BlueZ + daemon + PipeWire + evdev/uinput.
- ATVV v1.0+ only; v0.4 is out of scope.
- Rust 2024; MSRV 1.87 (zbus 5 requires it); `atvv-protocol` is I/O-free.
- Arch Linux packaging only; Debian/Fedora/Nix/Flatpak are out.
- PipeWire service restart belongs to systemd (`PartOf=pipewire.service`,
  `Restart=always`), not to `atvv-audio`. Verified with a transient unit using
  the packaged properties (`MainPID` changed 85063 → 87720 and the source was
  recreated); the root-installed unit's post-reboot run is still pending.
- The resident source sets `node.pause-on-idle=false` so a linked consumer
  wakes it without a playback driver. `node.always-process` is not used.
- `atvv-bluez` owns a private Tokio runtime and exposes a synchronous
  `BluezLink`; `atvv-daemon` adapts it to `Transport`.
- ASR is external; no unofficial cloud protocol in the core.
- Device facts live in `DeviceProfile`; RC003 profile id is
  `miremote-rc003`. Button keys and gesture timing are profile fields.
- The fake remote profile (`test-fake-remote`, VID `0x1209`, PID `0x0002`) is
  test-only and never part of `PROFILES`.
- D-Bus boundary: `org.atvv.Daemon` at `/org/atvv/Daemon`; `GetStatus` returns
  JSON from the `StatusView` model; `ListDevices` returns addresses;
  `StartVoice`, `StopVoice`, `ToggleVoice`, `ReloadConfig`; signals
  `StateChanged(s)`, `VoiceStarted()`, `VoiceStopped()`. Protocol structs do
  not cross D-Bus.
- Config: `$XDG_CONFIG_HOME/atvv/daemon.toml`; missing file = defaults;
  invalid file is reported by `ReloadConfig` and `atvvctl doctor`.
- The RC003 mic is gated by the physical voice key; host `MIC_OPEN` alone
  yields `AUDIO_START` and then a frame timeout. Voice-key edges drive both
  `MIC_OPEN`/`MIC_CLOSE` and the synthetic trigger.
- Package facts: `atvv-linux` version `0.1.0`; binaries `atvvd`, `atvvctl`,
  `atvv-mcp`; user unit at `/usr/lib/systemd/user/atvvd.service`; udev rule
  `/usr/lib/udev/rules.d/70-atvv-linux.rules`; modules-load
  `/usr/lib/modules-load.d/atvv-linux.conf`; no system D-Bus policy.
- The Arch source tarball is deterministic (`packaging/arch/make-source.sh`);
  `packaging/arch` is `export-ignore`, so the pinned checksum is stable.
- Man pages are generated from clap by `atvvctl manpages`; `atvvctl
  completions <shell>` prints completions. `atvvd` shares its clap definition
  from `atvv-daemon/src/cli.rs`.
- CI runs fmt, clippy, workspace tests, cargo-deny, and REUSE. Dependabot
  checks Cargo and GitHub Actions daily and opens grouped minor/patch PRs;
  it does not auto-merge.
- `deny.toml` allows GPL-3.0-or-later, MIT, Apache-2.0 (including the LLVM
  exception), BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0, and Zlib.
  GPL-3.0-only is deliberately absent.

## Verified

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
reuse lint
udevadm verify packaging/udev/70-atvv-linux.rules
```

- `cargo test --workspace` → 114 passed, 2 ignored.
- `cargo deny check` → advisories, bans, licenses, sources ok; zbus 5
  unmaintained advisories `RUSTSEC-2026-0370/0371/0372` ignored with a
  migration note.
- `reuse lint` → REUSE 3.3 compliant, 92 files.
- `udevadm verify` → 1 rule file, 0 failures.
- `systemd-analyze verify --user` with a temp copy pointing at the built
  binary → pass. The packaged unit was enabled and started from a temporary
  user unit copy whose `ExecStart` was the staged packaged binary, then
  removed.
- `makepkg -f` in `packaging/arch` → `atvv-linux 0.1.0-1` (5.8 MiB) plus a
  debug package; after the connect fix, rebuilt `atvv-linux 0.1.0-2`.
  Contents: three binaries, user unit, udev rule, modules-load, 24 man pages,
  bash/zsh/fish completions, LICENSE, third-party notices, README,
  device-support matrix.
- Staged `pacman --root`/`fakeroot` install → succeeded; packaged
  `atvvctl --version` = `0.1.0`. `namcap` only warns that glibc/libpipewire/
  libgcc are implicit and that `pipewire`/`bluez` may not be needed (they are
  runtime services).
- Post-reboot `0.1.0-1` failure and fix: the installed unit looped on
  `org.bluez.Error.InProgress`; `0.1.0-2` starts `atvvd` and owns
  `org.atvv.Daemon` before connecting, treats `InProgress`/`AlreadyConnected`
  as pending/success, and bounds the whole connect. The HID input runtime is
  attached when its node appears and retried; dropping it releases the
  synthetic trigger. With the remote disconnected, `atvvctl doctor` reports
  `daemon`, `source`, and `pipewire` ok; the daemon stays active and logs
  `reconnect failed: connect timed out after 10s` per retry. The remote must
  be woken for the physical link.
- Real RC003 (`C0:5D:39:F8:2D:4C`, `hci0`, 2026-10-30): release and packaged
  `atvvd` → CAPS `0b 01 00 02 03 00 78 00 00`; `atvvctl doctor` all green;
  `Audio/Source` at 16 kHz; 1 s capture = 16,000 samples; `MIC_OPEN` →
  `AUDIO_START 04 00 02 00` then frame timeout without the key; reconnect
  after `bluetoothctl disconnect`; `systemctl --user restart
  pipewire.service` restarted the unit and recreated the source.
- Step 07/08 evidence remains the source for 60 s keepalive (3,954 frames /
  948,960 samples, six `MIC_EXTEND`), key mapping, and the fcitx source/trigger
  run: `docs/research/rc003-hardware-evidence.md`.
- `atvvctl record --wav` uses `pw-cat --record`; a non-zero pw-cat exit is
  accepted when the WAV contains data.
- `atvvctl completions <shell>` and `atvvctl manpages <dir>` work from the
  release binary; a broken stdout pipe returns an error instead of panicking.

## Next step

**09 packaging-and-acceptance** remains. Do these in order, without a timer:

1. Install the built package as root:
   `sudo pacman -U packaging/arch/atvv-linux-0.1.0-2-x86_64.pkg.tar.zst`
   (the deterministically generated source tarball and pinned checksum are
   already in place; rebuild with `makepkg -f` if the package file is absent).
2. `systemctl --user daemon-reload && systemctl --user enable --now atvvd`;
   `atvvctl doctor` must be all green. Check that `systemctl --user status
   atvvd` uses `/usr/bin/atvvd`.
3. Run `udevadm test /sys/class/input/eventX` as root for the RC003 node and
   record the `uaccess` tag; re-login if the ACL is missing.
4. Reboot, log in, and verify `atvvctl doctor`, the source, and a real voice
   press. Record the result in `docs/acceptance/rc003-2026-10-30.md`.
5. Hold the voice key in a focused fcitx input context and commit text; record
   the systemd unit, source, and text result. This is the last open exit
   criterion. If it cannot be done now, leave the step in progress; do not
   fabricate a result.
6. Sign the acceptance record and mark step 09 done only when every exit
   criterion has a recorded command/output.

Then **10 vinput-zero-config**: upstream fcitx5-vinput `SetCaptureDevice` plus
trigger get/set and scene-id PR, daemon auto-selection that never overrides an
explicit user device, and a default mode that never assumes `Alt_R`
(`auto`/`dbus`). See
[`docs/plans/10-vinput-zero-config.md`](plans/10-vinput-zero-config.md). Until
it lands, `vinput device use atvv-miremote-rc003` remains a one-time step.

## Commands

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
reuse lint
udevadm verify packaging/udev/70-atvv-linux.rules
./packaging/arch/make-source.sh
cd packaging/arch && makepkg -f
cargo run -q -p atvv-cli -- profile show miremote-rc003
cargo run -q -p atvv-cli -- simulate miremote-rc003
cargo run -q -p atvv-cli -- audio test --seconds 3
cargo run -q -p atvv-cli -- input fake-remote
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
cargo run -q -p atvv-cli -- completions bash
cargo run -q -p atvv-cli -- manpages /tmp/atvv-man
cargo test -p atvv-input hold_and_release_reach_the_kernel -- --ignored
```

## Rules

- No kernel/BlueZ/PipeWire patches.
- No root daemon; systemd user service + udev `uaccess`.
- No hardware claim without a recorded run.
- No push without explicit approval.
- Update the step file and plan index in the same commit as code.
