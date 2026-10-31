# Handoff

Updated: 2026-11-01

## Project

- License: GPL-3.0-or-later
- Git: `main` tracks `origin/main`; check `git status` for unpushed commits. Do not push without explicit approval.
- Latest commit: run `git log -1 --oneline` (do not hard-code a hash that this file would invalidate).
- Tests: `cargo test --workspace` → 174 passed, 5 ignored tests (two
  kernel-uinput input tests, one fake-remote live-map test, the real-device
  GUI test, and a locale-detection test; the first three pass with
  `/dev/uinput` access, and the locale test runs under `LANG=zh_CN.UTF-8`).

## Read first

1. `AGENTS.md` — hard rules and commands.
2. `docs/plans/README.md` — canonical step index and M0 exit criteria.
3. `docs/research/README.md` — evidence index and current deltas.
4. `docs/decisions/0005-pipewire-restart.md` — systemd owns PipeWire restart.
5. `docs/decisions/0008-gui-toolkit.md` — GTK4/libadwaita/GNOME HIG.
6. The next step file and its research links before editing code.
7. For step 15, the interface reference
   [`docs/plans/15-mapping-board.html`](plans/15-mapping-board.html).

This file is updated in place at the end of every step/session. Do not create
per-session copies; `docs/plans/` remains the canonical plan.

## Design reference

- `docs/plans/15-mapping-board.html` is the committed interactive interface
  reference for the step 15 mapping board: three-column layout, per-button
  cards, modal action editor, banners, test bar, and header device chip. It is
  a design artifact, not shipped code. The device popover and per-device maps
  preview step 21; the second remote is illustrative. It also contains the
  decided step 17 layout UI: header layout chip, layout popover, manager
  dialog, template Default, and the manager's Apply/Revert footer. Step 17
  implementation is still pending.

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
- **`atvv-linux-git` packages (built, not installed)**: the split VCS
  package `atvv-linux-git 0.1.0.r64.g6f86ec3-1` (6.3 MiB; package sha256
  `9b71c0ad126b41392d5d56575f6a4559e69a09e67af3581f2e39c9e3aa4d9495`) and
  `atvv-linux-gui-git 0.1.0.r64.g6f86ec3-1` (1.3 MiB; package sha256
  `bdcab38c44a0ba2c8e1e4fe32e8a3e8ab2887b1fb4aaa0215915152f796e9e5a`)
  were built from commit `6f86ec3` with `makepkg -f`. They provide and replace
  `atvv-linux` / `atvv-linux-gui`; the installed `0.1.0-7` packages remain
  active until the operator installs them. `makepkg` now compiles the release
  binaries once (CI owns the test suite); the VCS target is preserved, and a
  repeat `cargo build --release` finishes in 0.1 s. The GUI package adds
  `gtk4`/`libadwaita`/`librsvg` plus `atvv-linux-git`; the base package keeps
  `pipewire`/`bluez`/`dbus`.
- **14 configuration GUI (complete and verified)**:
  `atvv-gui` / `atvv-config` is a GTK4/libadwaita D-Bus client with an
  embedded SVG remote diagram matching the RC003 layout (power/voice at the
  top, circular D-pad, back/home/menu left, volume/TV right; the voice key
  opens the recording-key page), twelve action hotspots plus the recording-key
  hotspot (XTest verified the twelve action keys; the UI test selects the
  recording key), the single/double/long action editor, keyboard and remote
  learning, test mode, `exec` confirmation, banners/toasts, and a
  daemon-unavailable state. The remote illustration follows the straight-on
  product proportions (slim body, silver power/mic keys, 50%-diameter D-pad
  center, rounded-square `TV` key, NFC N-mark, `xiaomi` wordmark); the voice
  key uses the standard microphone glyph. Re-verified on 2026-10-31 with
  `cargo fmt`, `cargo clippy -p atvv-gui`, `xvfb-run cargo test -p atvv-gui`
  (20 passed, 1 ignored), and a headless mock-daemon capture. The
  recording-key page edits only `trigger_key`
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
- **Recording-key declared chooser (2026-11-01)**: the recording-key page
  offers the contract's declared keys (`DICTATE`, `VOICECOMMAND`,
  `ASSISTANT`) plus `None (disable synthetic key)` through **Declared key…**.
  The menu is rebuilt from `StatusView.contract.recording_keys` only when the
  list changes; selecting fills the free-form output field without saving.
  The page shows the fcitx5 name mapping and warns when `vinput` is `dbus` or
  `off`. GUI status now parses `contract` and `vinput`; UI/i18n tests cover
  declared selection, `none`, custom `F23`, and the Chinese hint.
- **12 provider contract (complete and verified)**: `StatusView.contract` is
  the single source of truth (version, profile, source, media class, format,
  declared keys, active key, input/keyboard readiness, D-Bus/MCP endpoints,
  recipe). `atvvctl contract [--json]` prints it, `atvvctl status` carries it,
  `atvvctl doctor` checks it plus the live `atvv.*` properties, and
  `atvv-mcp` exposes it as the `contract` tool. Browser AI chat and IM recipes
  are first-class documents. See `docs/plans/12-integration-contract.md`.
- **Dependencies (2026-10-31)**: zbus 6.2, gtk4 0.11, libadwaita 0.9, toml
  1.1, and the current patch versions. `evdevil` moved from the exact
  `=0.5.0` pin to the `0.5` requirement. zbus 6 removed the blocking API, so
  the daemon `Client`/`VinputClient`, the GUI worker, `atvvctl`, and the
  tests drive the async API with `zbus::block_on`; the `tokio-multithread`
  feature keeps the process-wide connection runtime running outside
  `block_on`. The zbus 5 split crates are gone, so their `deny.toml`
  advisories were removed; third-party notices were regenerated. Dependabot
  now opens one PR per version update instead of grouped PRs. Rust 1.99
  clippy required let-chain collapses and an `as_chunks` cleanup in
  `atvv-input`, `atvv-bluez`, `atvv-daemon`, and `atvvctl`.
- **16 hardware info (complete and verified)**: `StatusView.hardware` carries
  the battery percentage from `org.bluez.Battery1` and the GATT Device
  Information Service facts (firmware/hardware/software revisions, serial,
  manufacturer, model). `atvv-bluez` reads them during connect/reconnect and
  refreshes only the battery; the daemon refreshes every 60 s while connected
  and keeps the last known values on disconnect. `atvvctl doctor` prints a
  `hardware` line, `atvvctl status` and `atvv-mcp status` expose the JSON
  object, and the GUI sidebar home shows a Battery row plus a Hardware group.
  Real RC003 (2026-10-31): battery 62 %, firmware `2671`, HW `V2.0`, SW
  `A.7.0.6`, serial `250519`. Evidence:
  `docs/research/rc003-hardware-evidence.md`.
- **19 i18n and bilingual docs (complete and verified)**: `atvv-gui` resolves
  the desktop locale once at startup and localizes every app-owned string from
  a msgid-keyed Rust catalog (`crates/atvv-gui/src/i18n.rs`): English is the
  source and fallback, any `zh*` locale selects Simplified Chinese. Protocol
  values, key names, addresses, and daemon diagnostics stay untranslated.
  Desktop entry and AppStream metadata carry Chinese names and descriptions.
  English and Chinese headless windows are tested, and an ignored
  locale-detection test passes with `LANG=zh_CN.UTF-8`. User-facing documents
  have linked `.zh-CN.md` siblings (README, configuration, GUI, device
  support, all integration guides, Arch package README); development records
  stay English. See `docs/plans/19-i18n-and-bilingual-docs.md`.
- **20 first release and external packaging (prepared, push pending)**:
  `.github/workflows/release.yml` builds x86_64/aarch64 binaries, packages
  them with the runtime assets and docs, and publishes them with the source
  archive and checksums; `v0.1.0` is tagged locally. Arch recipes are outside the repository: one
  split AUR pkgbase `atvv` (`atvv`, `atvv-gui`) under `~/aur/atvv`, and one
  split VCS pkgbase `atvv-git` (`atvv-git`, `atvv-gui-git`) in the personal
  `arch-pkgs` repository. Both declare `x86_64` and `aarch64`; aarch64 was not
  built here. The split Cargo build compiles the shared graph once. See
  `docs/plans/20-release-and-external-packaging.md`.

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
  The interface reference is `docs/plans/15-mapping-board.html`; it previews
  the step 21 device chip/popover and per-device maps. Depends on step 14; see
  `docs/plans/15-mapping-board.md`.
- **17 layout profiles (planned)**: add named user layouts (button slots plus
  gesture timing) beside the device profile, with a synthesized `default`
  template whose edits save as a new layout; schema v2, v1 migration,
  `SetLayout`/`LayoutChanged`, CLI and GUI switching, and live apply.
  Depends on 13, 14, and 15; see
  `docs/plans/17-layout-profiles.md` and
  `docs/research/layout-profiles-presets.md`.
- **18 layout presets (planned)**: ship static `media`, `coding`, and
  `slides` templates that create editable user layouts; presets never become
  live state and never contain `exec:`. Depends on 17; see
  `docs/plans/18-layout-presets.md`.
- **21 multi-device switching (planned)**: one active device at a time, a
  known-device registry, `SelectDevice`/`GetDevices`/`DeviceChanged`, and
  per-device layouts. The step 15 reference previews the device chip/popover.
  Simultaneous devices are out of scope. Depends on 06, 13, 15, and 17; see
  `docs/plans/21-multi-device.md`.
- **B02 NFC tag (backlog)**: characterize the RC003's passive NFC tag (IC,
  UID, NDEF, lock state) with an external libnfc reader; prefer a read-only
  UID trigger and only test a rewrite after a verified dump and restore. NFC
  stays out of the daemon core. Needs an NFC reader; see
  `docs/plans/B02-nfc-tag.md`.

Do not claim unverified hardware behavior (pairing gesture, adapter
power-cycle, ASR intelligibility, a root-installed unit after reboot). Step 07
verified the fields recorded in `docs/research/rc003-hardware-evidence.md`.

## Decisions

- Research basis, reference snapshots, licenses, and open hardware questions:
  `docs/research/`.
- Userspace stack only: BlueZ + daemon + PipeWire + evdev/uinput.
- ATVV v1.0+ only; v0.4 is out of scope.
- Rust 2024; MSRV 1.99 tracks the stable toolchain (zbus 6 needs 1.89,
  gtk4 0.11 needs 1.92); `atvv-protocol` is I/O-free.
- `DeviceProfile` is hardware/firmware identity. User keymaps are separate
  `Layout` objects (schema v2) with a synthesized `default` template;
  presets are static templates that create user layouts. No automatic
  per-app/focus switching.
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
- Package facts: AUR split pkgbase `atvv` (`atvv`, `atvv-gui`) and external
  VCS pkgbase `atvv-git` (`atvv-git`, `atvv-gui-git`); binaries `atvvd`,
  `atvvctl`, `atvv-mcp`, `atvv-config`; user unit at
  `/usr/lib/systemd/user/atvvd.service`; udev rule
  `/usr/lib/udev/rules.d/70-atvv-linux.rules`; modules-load
  `/usr/lib/modules-load.d/atvv-linux.conf`; no system D-Bus policy.
- The Arch recipes live outside this repository. The repository carries no
  PKGBUILD. `atvv`/`atvv-gui` build from the release archive; the VCS pair
  tracks the GitHub default branch. One Cargo invocation builds both split
  packages; tests stay in CI.
- Man pages are generated from clap by `atvvctl manpages`; `atvvctl
  completions <shell>` prints completions. `atvvd` shares its clap definition
  from `atvv-daemon/src/cli.rs`.
- CI runs fmt, clippy, workspace tests, cargo-deny, and REUSE. A paths-filter
  skips the compile/test job for docs-only changes; the compliance job still
  runs. A newer push to the same ref cancels the older run. Dependabot
  checks Cargo and GitHub Actions daily and opens one PR per version update
  (no groups); it does not auto-merge. The runner is pinned to `ubuntu-26.04`
  with `actions/checkout@v7` and `fsfe/reuse-action@v6` (Node 24); the
  Dependabot actions PR is superseded.
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
- Localization: the GUI ships English and Simplified Chinese through a
  msgid-keyed Rust catalog and follows the desktop locale; English is the
  fallback. Code, commits, CLI output, man pages, and development docs are
  English. User-facing docs have linked `.zh-CN.md` siblings; device data and
  quoted output may keep their source language.

## Verified

- 2026-11-01 recording-key chooser: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings` clean;
  `cargo test --workspace` → 174 passed, 5 ignored;
  `xvfb-run -a cargo test -p atvv-gui --test ui` → 1 passed;
  `xvfb-run -a cargo test -p atvv-gui --test i18n` → 1 passed, 1 ignored;
  `cargo test -p atvv-gui --lib` → 21 passed.
- 2026-11-01 local `-git` test build from `fcd422b`:
  `atvv-git 0.1.0.r8.gfcd422b-1` (sha256
  `9ea7e6ffb250aeb4fb109d7876397f6145c2452e0ce28c9937eca9c47c02e72d`) and
  `atvv-gui-git 0.1.0.r8.gfcd422b-1` (sha256
  `0582c339899264b2e7b357ebd3f11544f88da436363f79daadf019408e9d9200`).
  The temporary PKGBUILD pointed at the local repository because the
  checked-in VCS recipe tracks GitHub. The installed
  `atvv-linux-git`/`atvv-linux-gui-git` pair owns the same binary paths, so
  installing the new packages requires removing that pair first. `namcap`
  reports only the expected implicit-library/runtime-service notes.
- 2026-10-31 step 20: the tag-triggered release workflow YAML parses; it
  builds x86_64 and aarch64 binaries on GitHub runners;
  `v0.1.0` is tagged; the AUR PKGBUILD pins the source archive checksum.
  The split AUR pkgbase built `atvv` (6.4 MiB / 18.1 MB installed) and
  `atvv-gui` (1.3 MiB / 3.34 MB installed) in one 86 s run with the existing
  `$srcdir` (Cargo 1m20s). `namcap PKGBUILD` is clean; package warnings are
  the expected implicit libraries and runtime-service notes. The AUR split
  repository and the `arch-pkgs` split `atvv-git` pkgbase are committed.
  `aarch64` is declared but not built on this host.
- 2026-10-31 step 19: `cargo fmt --all -- --check` and
  `cargo clippy --workspace --all-targets -- -D warnings` are clean;
  `cargo test --workspace` → 171 passed, 5 ignored;
  `xvfb-run -a cargo test -p atvv-gui --test ui --test i18n` → English and
  Chinese windows passed;
  `LANG=zh_CN.UTF-8 LANGUAGE=zh_CN xvfb-run -a cargo test -p atvv-gui
  --test i18n -- --ignored` → locale detection passed.
  `desktop-file-validate packaging/gui/org.atvv.Config.desktop` and
  `appstreamcli validate --no-net packaging/gui/org.atvv.Config.metainfo.xml`
  are clean; `reuse lint` is compliant (151 files); a local Markdown link
  check reports no broken targets; the msgid audit found 127 catalog entries
  for 127 call sites.

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
  for `evdevil`/`uoctl`. zbus 6 merged the split crates, so
  `RUSTSEC-2026-0370/0371/0372` no longer appear in the graph and their
  ignores are gone from `deny.toml`.
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

**Step 20 is complete and published.** The tag `v0.1.0`, the source archive,
and the x86_64/aarch64 binary archives are on the GitHub release. The AUR
split pkgbase `atvv` is published as `atvv`/`atvv-gui` `0.1.0-1`; the
personal `arch-pkgs` split pkgbase `atvv-git` is ready. The repository carries
no PKGBUILD. See `docs/plans/20-release-and-external-packaging.md`.

**Step 19 (localization) is complete.** The GUI follows the desktop locale
(English or Simplified Chinese) and the user-facing docs have Chinese
counterparts. Step 15 remains the next UI step: it replaces the single-key
inspector with a sayall-style board (central remote + per-key
single/double/long cards) while staying GTK4/libadwaita. Use the committed
interface reference `docs/plans/15-mapping-board.html` as the visual and
interaction basis; the second remote and per-device maps there preview step
21, so step 15 implements the one-device board and its device chip without
switching. See `docs/plans/15-mapping-board.md`. Step 17's layout-selector UI
is decided in the same reference; its plan now defines Default as a template:
board edits are staged, and Apply saves them as a new layout. Implementation
remains in step 17. B01 (recording-key press/release actions) is backlog and does not block step 15.

**Step 10 vinput-zero-config is blocked** on upstream review/merge: ATVV is
the provider and vinput is the consumer. Issues #491 and #492 are open; PR
#493 is ready, PRs #494 and #495 are drafts. `vinput = "trigger"` stays the
default; `vinput = "dbus"` is an explicit fallback.

Immediate action: start step 15. In parallel, wait for upstream review on PR
#493; after it merges, rebase PR #494 onto `main`, then PR #495, rebuild
vinput, and record the real auto-bind acceptance.

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
git tag -l
makepkg -f                           # in an AUR package checkout
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
