# Handoff

Updated: 2026-10-31

## Project

- License: GPL-3.0-or-later
- Git: `main` tracks `origin/main`; check `git status` for unpushed commits. Do not push without explicit approval.
- Latest commit: run `git log -1 --oneline` (do not hard-code a hash that this file would invalidate).
- Tests: `cargo test --workspace` → 166 passed, 4 ignored tests (two
  kernel-uinput input tests, one fake-remote live-map test, and the new
  real-device GUI test; the first three pass with `/dev/uinput` access).

## Read first

1. `AGENTS.md` — hard rules and commands.
2. `docs/plans/README.md` — canonical step index and M0 exit criteria.
3. `docs/research/README.md` — evidence index and current deltas.
4. `docs/decisions/0005-pipewire-restart.md` — systemd owns PipeWire restart.
5. `docs/decisions/0008-gui-toolkit.md` — GTK4/libadwaita/GNOME HIG.
6. The next step file and its research links before editing code.

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
- **09 packaging and acceptance (complete)**: version `0.1.0-3`; clap
  `completions`/`manpages`; `atvvd` CLI in `atvv-daemon/src/cli.rs`; packaged
  user unit, udev `uaccess` rule, `modules-load.d`; deterministic Arch source
  tarball and PKGBUILD; cargo-deny, REUSE, third-party notices; CI with
  fmt/clippy/test/cargo-deny/REUSE; daily Dependabot PRs; README
  install/troubleshooting; device support matrix. `0.1.0-3` is root-installed
  (Install Date 2026-10-30 17:36:53); after the 21:15 reboot the enabled unit
  started `/usr/bin/atvvd`, `atvvctl doctor` was all green, and the end-to-end
  fcitx text commit was operator-confirmed. Completed record:
  `docs/acceptance/rc003-2026-10-30.md`.
- **0.1.0-5 package (installed and active)**: the step-12 tree produced
  `atvv-linux 0.1.0-5` (5.9 MiB; package sha256
  `0099a95a36946146239daeab5036e0677561d02d7abaa1a7806f208aa1e3e447`).
  It is installed on 2026-10-31; the enabled user unit runs
  `/usr/bin/atvvd`, the live source advertises
  `atvv.contract_version="1"`, `atvv.profile="miremote-rc003"`, and
  `atvv.recording_keys="DICTATE,VOICECOMMAND,ASSISTANT"`, and
  `atvvctl doctor` reports `properties ok` and `contract ok`. The previous
  `0.1.0-4` package predated the contract output.
- **0.1.0-6 package (built, not installed)**: the step-13 tree produced
  `atvv-linux 0.1.0-6` (6.2 MiB; package sha256
  `5717585d93f5efa282232dc14f6773dbec39cf2b32c80b69280d6f3586286f81`)
  with the source tarball sha256
  `693464fb202a0870af21e08226acfd3a757ed78c777651ad6debb310d6440589`.
  `makepkg -f` runs the release workspace tests and installs the three
  binaries, user unit, udev rule, modules-load file, man pages for `config`/
  `capture`/`map`, completions, README, device-support matrix, and licenses.
  The PKGBUILD now cleans `target/` in `prepare()`: the deterministic source
  tarball uses epoch mtimes, so a reused `src/.../target` could otherwise make
  Cargo package stale artifacts. `namcap` only reports the expected implicit
  glibc/libgcc/libpipewire and runtime-service (`pipewire`, `bluez`) notes.
- **11 evdevil input backend (complete)**: `atvv-input` now uses `evdevil`
  0.5.0 (`Key`, `Evdev`, `EventReader`, `UinputDevice`) instead of `evdev`
  0.13.2; 0BSD added to `deny.toml`; discovery skips unreadable/vanished
  nodes, pointer safety preserved; sysfs resolves the uinput event node;
  `VOICECOMMAND`, `ASSISTANT`, and `DICTATE` resolve and emit; fake remote and
  both ignored uinput tests pass. The daemon default trigger is now `DICTATE`
  (vinput `Dictate` at the next free index; default install `1=Dictate`).
  A real RC003 press emitted `KEY_DICTATE`, streamed 104 frames / 24,960
  samples, and made fcitx5-vinput record from `atvv-miremote-rc003` at 16 kHz.
  See `docs/plans/11-input-evdevil.md`.
- **13 keymap configuration backend (complete and verified)**: `atvv-config`
  is now the only `daemon.toml` parser. It carries `schema_version`, `profile`,
  `device`/`adapter`, gain, input, vinput mode, trigger key, `allow_exec`,
  `[gesture]` timing overrides, and `[buttons.<button>]` `single`/`double`/
  `long` actions. The daemon applies map/timing/trigger/vinput changes live by
  recreating the input runtime (uinput key registration depends on the map),
  restores the previous runtime if the new one cannot start, and rolls the
  runtime back if the atomic file write fails. D-Bus `GetConfig`/`SetConfig`
  and `BeginCapture`/`EndCapture`/`ButtonEvent` are implemented; capture ends
  when the calling connection leaves the bus. `atvvctl config get|set`,
  `capture`, and `map test` are implemented. `exec:` is opt-in, runs one
  command at a time on a worker thread with a 5 s timeout, closes stdio, and
  refuses to run as root. The fake-remote test changed `up` to `KEY_F13` live
  and observed it without a daemon restart. See
  `docs/configuration.md` and `docs/plans/13-keymap-config.md`.
- **0.1.0-9 packages (built, not installed)**: the recording-key tree
  produced the split `atvv-linux 0.1.0-9` (6.2 MiB; package sha256
  `a7b102c219771502d573da8361a0fca37ff553f0a2cbb6cf1ad5ba10b9a693d9`)
  and `atvv-linux-gui 0.1.0-9` (1.3 MiB; package sha256
  `20311d5a7638d36caf96c9c5f6ca0a6e2707e3eed25e22cb9d98e8c8bfa1c3a3`) from
  source tarball sha256
  `a727e4bb0e5b19e8782869d01277eabcdb3e8fea1f4eede97153a30100890f21`.
  The base package keeps `pipewire`/`bluez`/`dbus` dependencies and the GUI
  package adds `gtk4`/`libadwaita`/`librsvg` plus `atvv-linux`. The packaged
  `atvv-config` launches under Xvfb with the packaged `atvvd --mock` and
  renders the embedded diagram, which mirrors the real RC003 layout
  (power/voice top, circular D-pad, back/home/menu left, volume/TV right).
  `namcap` reports only the expected implicit
  libraries and runtime-service notes. `pacman -U` needs the operator's
  password, so 0.1.0-9 is not installed. The installed `atvv-linux 0.1.0-7`
  (07:01 build) predates the test-mode and recording-key fixes; reinstall from
  0.1.0-9 to get capture suppression, `KEY_HOME`/`KEY_COMPOSE`, the SVG region
  highlight, the pointer-click fix, and the selectable recording-key output.
  The package check now runs release library unit tests only; D-Bus, Xvfb, and
  real-device integration tests run in CI to keep package rebuilds short.
- **14 configuration GUI (complete and verified)**:
  `atvv-gui` / `atvv-config` is a GTK4/libadwaita D-Bus client with an
  embedded SVG remote diagram matching the RC003 layout (power/voice at the
  top, circular D-pad, back/home/menu left, volume/TV right; the voice key
  opens the recording-key page), twelve action hotspots plus the recording-key
  hotspot (XTest verified the twelve action keys; the UI test selects the
  recording key), the single/double/long action editor, keyboard and remote
  learning, test mode, `exec` confirmation, banners/toasts, and a
  daemon-unavailable state. The recording-key page edits only `trigger_key`
  (`DICTATE` default; `F23`, a chord, or `none`), not action slots.
  `RunAction` was added to `org.atvv.Daemon` for test mode. Split packaging is
  implemented (`atvv-linux-gui` with desktop entry, AppStream metadata, icon,
  and GTK runtime dependencies). Worker tests run against the real daemon
  service on a private bus; a headless `xvfb-run` test drives the window; a
  real RC003 run saved `key:KEY_F13` for `up/double` through the GUI and the
  mapping survived a daemon restart. Test mode then observed all thirteen
  physical keys (Power, OK, Voice, Up/Right/Left/Down, Back, Home,
  Volume +/-, Menu, TV), stayed active after OK, and left system volume
  unchanged. Home/Menu now use `KEY_HOME`/`KEY_COMPOSE` from the HID
  descriptor; capture suppresses mapped keys, exec, dictation, and voice
  session control. See `docs/gui.md` and `docs/plans/14-config-gui.md`.
- **12 provider contract (complete and verified)**: `StatusView.contract` is
  the single source of truth (version, profile, source, media class, format,
  declared keys, active key, input/keyboard readiness, D-Bus/MCP endpoints,
  recipe). `atvvctl contract [--json]` prints it, `atvvctl status` carries it,
  `atvvctl doctor` checks it plus the live `atvv.*` properties, and
  `atvv-mcp` exposes it as the `contract` tool. Browser AI chat and IM recipes
  are first-class documents. See `docs/plans/12-integration-contract.md`.

## Not implemented

- **10 vinput zero-config (blocked)**: ATVV is the provider; vinput owns
  source selection and trigger binding (ADR 0007). The primary upstream
  request is a consumer-side capture-device auto-selection/priority list and
  auto-binding of the declared recording keys. Until it exists, the one-time
  `vinput device use atvv-miremote-rc003` plus `Dictate` at the next free
  index (default install: `[TriggerKey] 1=Dictate`) is the correct layered
  setup, not a workaround. The ATVV-side `SetCaptureDevice` push and
  `vinput_device` policy are dropped. `vinput = "trigger"` remains the
  default; `vinput = "dbus"` is an explicit fallback. The live `daemon.toml`
  already sets `trigger_key = "DICTATE"`; the installed `0.1.0-5` service
  accepts it. Issues #491 and #492 are open; PR #493 is ready and PRs #494
  and #495 are drafts. See `docs/plans/10-vinput-zero-config.md`,
  `docs/research/vinput-upstream-api.md`, and
  `docs/decisions/0007-integration-direction.md`.
- **15 sayall-style mapping board (planned)**: replace the single-key
  inspector with a central remote and per-button cards showing all three
  slots; the action editor becomes a modal dialog. GTK4/libadwaita and the
  original SVG stay; SayAll/Open Voice Bridge assets and code are not copied.
  Depends on step 14; see `docs/plans/15-mapping-board.md`.- zbus 6 migration (the zbus 5 split crates are unmaintained; denied
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
- GUI: Rust + `gtk4-rs` + `libadwaita`, GNOME HIG; GUI is an optional D-Bus
  client. See ADR 0008. No TUI/GPUI/web UI.
- Device facts live in `DeviceProfile`; RC003 profile id is
  `miremote-rc003`. Button keys and gesture timing are profile fields.
- The fake remote profile (`test-fake-remote`, VID `0x1209`, PID `0x0002`) is
  test-only and never part of `PROFILES`.
- D-Bus boundary: `org.atvv.Daemon` at `/org/atvv/Daemon`; `GetStatus` returns
  JSON from the `StatusView` model; `GetConfig` returns the versioned
  `ConfigDocument`; `SetConfig` validates, applies, and persists atomically;
  `BeginCapture`/`EndCapture` control capture mode; `ListDevices` returns
  addresses; `StartVoice`, `StopVoice`, `ToggleVoice`, `ReloadConfig`;
  signals `StateChanged(s)`, `VoiceStarted()`, `VoiceStopped()`, and
  `ButtonEvent(s,s,b)`. Protocol structs do not cross D-Bus.
- GUI: `atvv-gui` is a separate GTK4/libadwaita D-Bus client (`atvv-config`,
  `org.atvv.Config`). It uses `GetConfig`/`SetConfig`,
  `BeginCapture`/`EndCapture`, and `RunAction`; it never writes
  `daemon.toml`, grabs evdev, or touches BlueZ/PipeWire. `RunAction` parses a
  checked action and runs key/chord through the input runtime, `exec` through
  the serial worker, and dictation through the session. The daemon package
  stays GTK-free; `atvv-linux-gui` is a split package.
- Config: `$XDG_CONFIG_HOME/atvv/daemon.toml`; missing file = defaults;
  invalid file is reported by `ReloadConfig` and `atvvctl doctor`.
  `atvv-config` is the only parser. A slot that is absent means "profile
  default"; `none` swallows the button. The voice key is not mappable.
  `exec:` requires `allow_exec = true`; the daemon runs it as the user with a
  5 s timeout and refuses to run when the effective uid is 0.
- Live config: `SetConfig` validates the complete document, applies the
  runtime, then writes the file atomically; a failed runtime apply restores
  the previous runtime, and a failed write rolls the runtime back. The input
  runtime is recreated only when the map, gesture timing, trigger key, vinput
  mode, or input enable changes.
- Capture: the daemon emits `ButtonEvent(button, trigger, pressed)` only while
  capture is active; `trigger` is the recognized gesture slot (`single`,
  `double`, `long`) or `release`. The owner is the unique bus name of the
  `BeginCapture` caller; a poll of `NameHasOwner` ends capture when that client
  disconnects.
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
  it does not auto-merge. The runner is pinned to `ubuntu-26.04` with
  `actions/checkout@v7` and `fsfe/reuse-action@v6` (Node 24); the Dependabot
  actions PR is superseded.
- `deny.toml` allows GPL-3.0-or-later, MIT, Apache-2.0 (including the LLVM
  exception), BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0, and Zlib.
  GPL-3.0-only is deliberately absent. Step 11 added 0BSD for `evdevil`
  and its `uoctl` dependency.
- Input backend: `atvv-input` uses `evdevil` 0.5.0; the `evdev` fork is
  abandoned (`docs/decisions/0006-evdevil-input-backend.md`). The three
  consumer keys are available; context-dependent mapping remains undecided
  (`docs/research/consumer-keys.md`).
- Default synthetic trigger: `DEFAULT_TRIGGER_KEY = "DICTATE"` (`KEY_DICTATE`,
  evdev `0x24a`), by user direction 2026-10-30. vinput uses `Dictate` at the
  next free index (default install `1=Dictate`); the names
  `VOICECOMMAND`/`ASSISTANT`/`DICTATE` (daemon) and
  `VoiceCommand`/`Assistant`/`Dictate` (fcitx) stay distinct.
  `VOICECOMMAND` and `ASSISTANT` are the fallbacks.
- fcitx5-vinput rewrites `~/.config/fcitx5/conf/vinput.conf` on addon exit;
  edit it while fcitx5 is stopped.
- vinput integration direction: ATVV is the lower layer and exposes a
  PipeWire source plus a uinput key; vinput owns source selection and trigger
  binding. ATVV does not write vinput config or call `org.fcitx.Vinput` in the
  primary path. The D-Bus `vinput` mode is an opt-in fallback. The primary
  upstream request is consumer-side source auto-selection plus auto-binding of
  `atvv.recording_keys`; `AddTriggerKey` on `org.fcitx.Vinput` is out of scope
  (ADR 0007).
- Provider contract: the source exposes `atvv.contract_version=1`,
  `atvv.profile`, and `atvv.recording_keys=DICTATE,VOICECOMMAND,ASSISTANT`.
  Consumers bind the declared set; the provider emits one active key.
  Diagnostics are generic (`atvvctl`/MCP), with per-consumer recipes in
  `docs/integrations/provider-contract.md`. No per-application diagnostics.

## Verified

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p atvv-input -- --ignored --test-threads=1
cargo test -p atvv-daemon --lib -- --ignored
cargo test -p atvv-daemon --test config_dbus
cargo deny check
cargo tree -i evdev                  # expected to fail: no evdev dependency
reuse lint
udevadm verify packaging/udev/70-atvv-linux.rules
```

- `cargo test --workspace` → 144 passed, 3 ignored; `cargo test -p atvv-input -- --ignored --test-threads=1` → 2 passed; `cargo test -p atvv-daemon --lib -- --ignored` → 1 passed.
- 2026-10-31 `cargo run -q -p atvv-cli -- audio test --seconds 5` and
  `pw-cli info atvv-test` → `media.class="Audio/Source"`,
  `atvv.contract_version="1"`, `atvv.profile="test"`,
  `atvv.recording_keys="DICTATE,VOICECOMMAND,ASSISTANT"`.
- `cargo run -q -p atvv-cli -- input fake-remote` → discovered and grabbed
  `/dev/input/event27`, created `/dev/input/event28`, recognized 12/12
  logical buttons, clean voice edges (evdevil backend).
- `cargo tree -i evdev` → `package ID specification 'evdev' did not match any
  packages`; `Cargo.lock` contains `evdevil 0.5.0` and `uoctl 1.0.1`.
- `cargo deny check` → advisories, bans, licenses, sources ok; 0BSD accepted
  for `evdevil`/`uoctl`; zbus 5 unmaintained advisories
  `RUSTSEC-2026-0370/0371/0372` ignored with a migration note.
- `reuse lint` → REUSE 3.3 compliant, 96 files.
- `udevadm verify` → 1 rule file, 0 failures.
- `systemd-analyze verify --user` with a temp copy pointing at the built
  binary → pass. The packaged unit was enabled and started from a temporary
  user unit copy whose `ExecStart` was the staged packaged binary, then
  removed.
- `makepkg -f` in `packaging/arch` → `atvv-linux 0.1.0-1` (5.8 MiB) plus a
  debug package; after the connect fix, rebuilt `atvv-linux 0.1.0-2`, and
  after the trigger-key change, `atvv-linux 0.1.0-3`.
- 2026-10-30 `makepkg -f` with the step-11 source at `d6a09b0` →
  `atvv-linux 0.1.0-4` (5.9 MiB) plus a debug package; source checksum
  `e3693e441679fdbb623a7775308e3d5792129bc7142217ff8bbf7f0a4044d38c`.
  Installed on 2026-10-31; the enabled unit runs `/usr/bin/atvvd`.
- CI run 41080061199 on `d6a09b0` failed in `cargo clippy`: `libdbus-sys`
  could not find `dbus-1.pc`. The test job now installs `libdbus-1-dev` and
  `pkg-config`; the next run is pending.
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
- Step 11 real RC003 run (2026-10-30, `C0:5D:39:F8:2D:4C`): workspace
  `atvvd` with `trigger_key = "DICTATE"` grabbed `/dev/input/event25`, created
  `atvv-virtual-keyboard` with `KEY_DICTATE` (code 586), and a physical press
  streamed 104 frames / 24,960 samples. fcitx logged `Key(Dictate)` and
  vinput-daemon recorded from `atvv-miremote-rc003`
  (`rate=16000 channels=1 fmt=259`); no speech, so it reported
  `returned no text`.
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
- 2026-10-31 step 12 contract check: `cargo test -p atvv-cli --test contract`
  → 1 passed; `cargo test -p atvv-mcp --test stdio` → 1 passed. A temporary
  workspace `atvvd --mock` on the real PipeWire graph exposed
  `atvv.contract_version="1"`, `atvv.profile="miremote-rc003"`, and
  `atvv.recording_keys="DICTATE,VOICECOMMAND,ASSISTANT"` on
  `atvv-miremote-rc003`; `atvvctl contract`/`status` printed the contract;
  `atvvctl doctor` reported `properties ok` and `contract ok`;
  `timeout 3 pw-cat --record --target atvv-miremote-rc003 --rate 16000
  --channels 1 --format s16` produced a 96,246-byte WAV. The installed
  `0.1.0-4` unit was stopped for the check and restarted afterwards.
- 2026-10-31 after installing `0.1.0-5`: `pacman -Q atvv-linux` =
  `0.1.0-5`; the user unit is active and connected over `bluez`; `atvvctl
  doctor` is all green including `properties` and `contract`; `atvvctl
  contract` prints the declared keys and `DICTATE`; `pw-cli info
  atvv-miremote-rc003` shows all three `atvv.*` properties.
- 2026-10-31 `makepkg -f` from the step-12 tree (`dadc23b`) →
  `atvv-linux 0.1.0-5` (5.9 MiB) plus a debug package; package sha256
  `0099a95a36946146239daeab5036e0677561d02d7abaa1a7806f208aa1e3e447`.
  Installed on 2026-10-31; the unit restarted and reports the contract.
- 2026-10-31 `./packaging/arch/make-source.sh` from the step-13 commit
  (`660efca`) → source tarball sha256
  `693464fb202a0870af21e08226acfd3a757ed78c777651ad6debb310d6440589`;
  `makepkg -f` → `atvv-linux 0.1.0-6` (6.2 MiB) plus a debug package;
  package sha256
  `5717585d93f5efa282232dc14f6773dbec39cf2b32c80b69280d6f3586286f81`.
  Not installed. The packaged `atvvctl --version` is `0.1.0`; the new
  `config get|set`, `capture`, and `map test` help and man pages are present.
- 2026-10-31 step 13 verification: `cargo test --workspace` →
  144 passed, 3 ignored; `cargo test -p atvv-input -- --ignored
  --test-threads=1` → 2 passed; `cargo test -p atvv-daemon --lib --
  --ignored` → the live-map fake-remote test passed (`up` changed to
  `key:KEY_F13` while the daemon kept running; the fake `Up` press produced
  `KEY_F13`). `cargo test -p atvv-daemon --test config_dbus` → 4 passed
  (`GetConfig` shape, SetConfig persistence/rollback, capture owner
  disconnect). An isolated `dbus-run-session` CLI run against `atvvd --mock`
  printed the document, applied and persisted `key:KEY_F13`, resolved
  `map test`/`combo:ctrl+alt+v`, timed out cleanly on `capture --timeout 1`,
  and rejected `key:nope` with the file unchanged. `cargo fmt --all --
  --check` and `cargo clippy --workspace --all-targets -- -D warnings` are
  clean.

## Next step

**Step 14 GTK4/libadwaita GUI is complete and verified.** The split
`0.1.0-9` packages are built in `packaging/arch/` but not installed;
the stale `atvv-linux 0.1.0-7` build remains active. Install them with
`sudo pacman -U packaging/arch/atvv-linux-0.1.0-9-x86_64.pkg.tar.zst
packaging/arch/atvv-linux-gui-0.1.0-9-x86_64.pkg.tar.zst` and restart the
user unit. The GUI uses `GetConfig`/`SetConfig`, `BeginCapture`/`EndCapture`,
and `RunAction`; it never writes `daemon.toml` or grabs evdev. See
`docs/gui.md` and `docs/plans/14-config-gui.md`.

**Step 15 is the next UI step.** It replaces the single-key inspector with a
sayall-style board (central remote + per-key single/double/long cards) while
staying GTK4/libadwaita. See `docs/plans/15-mapping-board.md`.

**Step 10 vinput-zero-config is blocked** on upstream review/merge: ATVV is
the provider and vinput is the consumer. Issues #491 and #492 are open; PR
#493 is ready, PRs #494 and #495 are drafts. `vinput = "trigger"` stays the
default; `vinput = "dbus"` is an explicit fallback.

Immediate action: install the 0.1.0-9 packages; then start step 15.
In parallel, wait for upstream review on PR #493; after it merges, rebase PR
#494 onto `main`, then PR #495, rebuild vinput, and record the real auto-bind
acceptance.

## Commands

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p atvv-input -- --ignored --test-threads=1
cargo test -p atvv-daemon --lib -- --ignored
cargo test -p atvv-daemon --test config_dbus
cargo deny check
cargo tree -i evdev                  # expected to fail: no evdev dependency
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
cargo run -q -p atvv-cli -- contract --json
cargo run -q -p atvv-cli -- mic start
cargo run -q -p atvv-cli -- mic stop
cargo run -q -p atvv-cli -- mic toggle
cargo run -q -p atvv-cli -- reload
cargo run -q -p atvv-cli -- config get
cargo run -q -p atvv-cli -- config get --json
cargo run -q -p atvv-cli -- config set --button up --trigger single --action key:KEY_F13
cargo run -q -p atvv-cli -- config set --allow-exec
cargo run -q -p atvv-cli -- capture --timeout 10
cargo run -q -p atvv-cli -- map test --button up --trigger single
cargo run -q -p atvv-cli -- map test --action "combo:ctrl+alt+v"
cargo run -q -p atvv-cli -- doctor
cargo test -p atvv-gui
xvfb-run -a cargo test -p atvv-gui --test ui
cargo test -p atvv-gui --test real_run -- --ignored
cargo run -q -p atvv-gui
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
