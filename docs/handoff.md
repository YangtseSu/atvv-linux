# Handoff

Updated: 2026-11-03

## Project

- License: GPL-3.0-or-later
- Git: `main` tracks `origin/main`; check `git status` for unpushed commits. Do not push without explicit approval.
- Latest commit: run `git log -1 --oneline` (do not hard-code a hash that this file would invalidate).
- Tests: `cargo test --workspace --exclude atvv-gui` → 217 passed, 7 ignored;
  `scripts/gui-tests.sh` → 59 passed, 2 ignored. Total 276 passed, 9 ignored
  (run 2026-11-03). The ignored tests are five kernel-uinput input tests, the
  real RC003 pointer test, one daemon live test, the real-device GUI test, and
  a locale-detection test; the kernel tests pass with `/dev/uinput` access, the
  pointer test also needs the RC003 and physical presses, and the locale test
  runs under `LANG=zh_CN.UTF-8`. The GUI suite runs on a private
  `mutter --headless` Wayland display, so no test window appears on the
  desktop. The GUI XTest suite still needs `GDK_BACKEND=x11` and an X screen at
  least as large as the window; the Wayland suite skips the pointer section.
- Live state (2026-11-03): the AUR `-git` packages are installed
  (`atvv-git`/`atvv-gui-git` `0.3.1.r2.g72835d4-2`) and `atvvd.service` runs
  from them; that build predates both GUI sizing fixes, so the installed
  `atvv-config` still opens at the fixed 760 px height. The stable
  `atvv`/`atvv-gui` `0.3.2-1` packages are built in `~/aur/atvv` and replace
  the `-git` pair (`conflicts`); installing them ships the fixes.

## Read first

1. `AGENTS.md` — hard rules and commands.
2. `docs/plans/README.md` — canonical step index and M0 exit criteria.
3. `docs/research/README.md` — evidence index and current deltas.
4. `docs/decisions/0005-pipewire-restart.md` — systemd owns PipeWire restart.
5. `docs/decisions/0008-gui-toolkit.md` — GTK4/libadwaita/GNOME HIG.
6. `docs/decisions/0009-caps-authoritative-sample-rate.md` — `CAPS_RESP` owns
   the initial stream rate; a valid `AUDIO_SYNC` can change it mid-session and
   the daemon rebuilds the resampler to the fixed graph rate.
7. The next step file and its research links before editing code.

This file is updated in place at the end of every step/session. Do not create
per-session copies; `docs/plans/` remains the canonical plan.

## Design reference

- `docs/plans/15-mapping-board.html` is the committed interactive interface
  reference for the step 15 mapping board, now implemented in
  `crates/atvv-gui/src/board.rs` and `crates/atvv-gui/src/editor.rs`. It is a
  design artifact, not shipped code. The device popover and per-device maps
  are now implemented; the second remote in the reference is illustrative. It
  also contains the decided step 17 layout UI: header layout chip, layout
  popover, manager dialog, template Default, and the manager's Apply/Revert
  footer. Steps 17, 18, and the step 21 GUI are implemented; the board has the
  layout chip. The preset dialog is not part of the HTML reference.

## Completed

- **v0.4.0 (prepared 2026-11-03)**: workspace version 0.4.0 over v0.3.2. The
  release carries the optional recording-key press and release actions
  (step 27), the headless `mutter` GUI test runner, the NFC tag research
  record (step 28), and the detached-backend flake fix (`426d451`) that
  v0.3.2 missed. The annotated tag runs the release workflow for
  x86_64/aarch64 binaries, the source archive, and checksums.
- **Step 27 recording-key edges (complete, 2026-11-03)**: the optional
  `[recording] press` and `release` actions are validated in `atvv-config`
  (button grammar, `dictation:*` rejected, `exec` needs `allow_exec`), exposed
  by `atvvctl config set --recording-press/--recording-release` and `config
  get`, dispatched by the daemon after each voice edge outside capture, and
  edited by the GUI's recording-key page. Test mode shows both edges and runs
  neither action. Verification: 217 passed / 7 ignored outside the GUI, 59
  passed / 2 ignored in the headless GUI suite, fmt and clippy clean. Synthetic
  only; no hardware claim. See `docs/plans/27-recording-key-press-release-actions.md`.
- **Step 28 NFC tag (complete, 2026-11-03)**: the ACR122U read the RC003 tag
  as an NTAG215 (`1D A4 2A 0B B8 00 00`) with an empty NDEF message, clear
  lock bytes, and no password. No write was made; the UID-gated
  `atvvctl mic` check passed. The step closed without an RC003-specific
  helper: a dedicated blank NTAG215 is the better local trigger, and a future
  helper should take a configurable UID. See
  `docs/plans/28-nfc-tag.md` and `docs/research/nfc-rc003.md`.
- **Headless GUI tests (2026-11-03)**: `scripts/gui-tests.sh` starts a private
  `mutter --headless` Wayland display (1280x1024 virtual monitor) and points
  GTK at it, so the GUI suite no longer opens windows on the desktop session.
  A plain `cargo test -p atvv-gui` still uses the desktop; use the script. The
  XTest pointer clicks still need Xvfb/X11; the Wayland suite skips that
  section.
- **v0.3.2 (published 2026-11-03)**: workspace version 0.3.2 over v0.3.1;
  the annotated tag `v0.3.2` (`008ca78`) published
  [the release](https://github.com/YangtseSu/atvv-linux/releases/tag/v0.3.2)
  with x86_64 (9,348,381 B) and aarch64 (8,823,608 B) binaries, the
  2,621,440 B source archive, and checksums; release workflow run 41567472373
  succeeded. Source sha256
  `0da7663cfaa73a1d074c3a5b3ea3327cfa111f0bcac2ed464389e2c897ba3bf5`. The
  release carries the two GUI sizing fixes (the diagram takes the card
  columns' height, the window grows to the board's height), README badges and
  the board screenshot, and corrected AUR install instructions: `atvv` and
  `atvv-gui` are one split pkgbase, so the documented `atvv-gui.git` clone
  never existed. AUR `atvv` is published as `0.3.2-2` (commit `5271def`):
  `0.3.2-1` shipped only four hand-picked doc files, so the installed
  `README.md` carried a dangling `docs/gui.png` and `docs/gui.md` had never
  found its screenshots; `pkgrel 2` installs the whole docs tree from `atvv`
  and drops `atvv-gui`'s duplicate `gui.md`. Built in `~/aur/atvv`:
  `atvv-0.3.2-2-x86_64.pkg.tar.zst` (6,761,254 B, sha256
  `fd2b2007b960b8380eaee595ccc1dab7bad0154a15ea2dff4b6813b6094d7592`) and
  `atvv-gui-0.3.2-2-x86_64.pkg.tar.zst` (1,461,552 B, sha256
  `ab011aaa43ad639f0f58f3b609cf45c78f52cd0e28c7fc6f53bbe09d42a19c2d`).
- **Detached test backend (fixed 2026-11-03)**: `Backend::detached` started a
  worker against an unreachable bus that answered every request with
  `LoadFailed` or `Status(Err)`. The window's 2 s status poll then cleared the
  `daemon_alive` flag a widget test had set through `set_status`, so
  `tests/ui.rs` saw the checklist flip from `Ready` to `Missing` at
  `Layer::Daemon` once the poll fired. `window_smoke` runs for about five
  seconds across that boundary, so it failed on timing: the 0.3.2 main CI run
  41567468926 failed while the identical tag run 41567472354 passed, and the
  0.3.1 push run 41495210429 failed the same way. The detached backend now
  owns a closed request channel and an event channel whose sender is dropped,
  so it produces no events; reproduction showed `daemon=Ready` at 0.06 s and
  0.5 s, then `Missing` from 1.0 s on. The fix is on `main` (`426d451`) after
  the published tag, so `v0.3.2` still carries the flake; main CI run
  41568713084 passed. `cargo test --workspace`: 272 passed, 9 ignored.
- **Window height follows the board (fixed 2026-11-03)**: the window opened at a
  fixed 1040×760 and clipped the board on the profiles and locales that need
  more room. Measured board natural heights: RC003/English 572 px,
  Chromecast/English 663 px, RC003/Chinese 650 px, Chromecast/Chinese 747 px;
  the scroll viewport was 653–659 px, so Chromecast/Chinese hid 94 px and the
  installed configuration (Chromecast, `LANG=zh_CN.UTF-8`) was the worst case.
  The window now grows to its measured height once a document loads:
  `ScrolledWindow::propagate_natural_height` forwards the board's natural
  height, `AppWindow::fit_height_to_content` measures at the window's width and
  applies `set_default_size`, and `schedule_fit` defers the measurement to the
  next frame because `MappingBoard` refits the diagram from its own tick
  callback. Growth is limited to a window whose height still equals the value
  this module set, so a manual resize, a tiling layout, and a maximized window
  are never overridden and the window never shrinks. A compositor still clamps
  the request to the workarea (verified: 1040×1500 became 1040×1048). With the
  live daemon and `wayland-0` the window opens 1040×854 with zero vertical
  overflow; `tests/window_fit.rs` fails on the previous code (4 px hidden on
  Chromecast/English) and covers both profiles in both languages.
  Workspace: fmt and clippy clean.
- **Board proportions (fixed 2026-11-03)**: the remote diagram claimed the
  SVG's intrinsic 320x950 size through `GtkPicture`, so the center column's
  natural height was 950 px against 540 px card columns; the picture then grew
  with the window while the columns stayed 540 px and were centered in the
  surplus. The diagram root is now a `GtkOverlay` whose base is an empty
  `GtkDrawingArea`, with the picture and the hotspot layer as overlay children
  under `set_measure_overlay(false)`, so nothing measures the texture. A
  `GtkAspectFrame` was not enough: it derives its natural height from the
  offered width (831 px at 280 px). `MappingBoard::fit` sizes the illustration
  from the taller card column minus the hint and voice card, measured at the
  center column's width; a frame-clock callback refits when that width changes,
  which covers window resize and font changes. All three columns align to the
  top, so surplus height is one area below the content. `tests/layout.rs`
  fails on the previous code (585 px diagram against 540 px columns) for both
  profiles. Workspace: 271 passed, 9 ignored; fmt and clippy clean.
- **v0.3.1 release (published 2026-11-03)**: workspace version 0.3.1; the
  annotated tag `v0.3.1` (`8320855`) published
  [the release](https://github.com/YangtseSu/atvv-linux/releases/tag/v0.3.1)
  with x86_64 (9,341,741 B; sha256
  `9c88b8f0e7631d750bbbb1bb50fa21bbf3a991a509961bf9796387bd7b16afe2`) and
  aarch64 (8,819,661 B; sha256
  `77e8c1c0e708e9cb54926043845d5e7e541c58c1be4d53b6aede4cc93e7c9d1a`)
  binaries, the 2,611,200 B source archive (sha256
  `938ac765b2da74f278542106b297cdcab8f4f2be1817d7a7ed62033a31661bea`),
  checksums, and generated notes. Release workflow run 41495210552 succeeded.
  The fix decodes a raw ADPCM frame that starts with `0x0A` instead of
  treating it as `AUDIO_SYNC`, which could drop the frame, reset the decoder,
  record a bogus sequence gap, and close the stream as an unsupported 8 kHz
  codec. The stable AUR update `0.3.1-1` was pushed as AUR commit `a6f4619`;
  `atvv` is 7,045,187 B (sha256
  `8e089a68c8fecd98dd16e7c0713197e4d415fcf503d2bbfa17286023aca5b638`) and
  `atvv-gui` is 1,589,069 B (sha256
  `cb3899690d65fe52572f1739fa0d5aa003ca6d2ad72eda0c4fee02befb1817e4`).
  `makepkg --printsrcinfo` matches `.SRCINFO`; `namcap` only reports the
  expected implicit glibc/libgcc/libpipewire and runtime-service notes.
- **Audio-sync frame misclassification (fixed 2026-11-03)**: a raw 120-byte
  ADPCM frame beginning with `0x0A` was parsed as `AUDIO_SYNC` on the audio
  characteristic. The frame was dropped, the decoder reset from its bytes, a
  bogus 32,334-frame sequence gap was recorded, and a second byte of `0x01`
  closed the stream as an unsupported 8 kHz codec (the stop seen in the
  keep-open run). The daemon now parses `0x0A` on the audio characteristic
  only for the 7-byte v1.0 sync and the 3-byte counter form. Verified on the
  RC003: three holds, 225 frames / 54,000 samples, 0 dropped, 0 sequence
  gaps, 0 errors, and fcitx5-vinput transcribed all three phrases. Workspace:
  270 passed, 9 ignored; fmt and clippy clean. See
  `docs/research/rc003-hardware-evidence.md`.
- **v0.3.0 release (published 2026-11-03)**: workspace version 0.3.0; the
  annotated tag `v0.3.0` (`ad39a49`) published
  [the release](https://github.com/YangtseSu/atvv-linux/releases/tag/v0.3.0)
  with x86_64 (9,340,611 B) and aarch64 (8,816,824 B) binaries, the 2,600,960 B
  source archive, checksums, and generated notes. Release workflow run
  41487705793 succeeded; the tag CI passed. The main CI first run failed in
  the GUI `window_smoke` checklist assertion (`Missing` vs `Ready`) and passed
  on re-run, so it is a timing flake. Highlights since v0.2.0: live level/gain
  and the session-boundary PCM flush (23), the first-run checklist and
  bilingual recovery docs (24), ATVV v1.0 `MIC_OPEN_ERROR`/`AUDIO_SYNC`
  conformance (25), the opt-in pre-opened microphone session (26), the
  second-device pairing fix, and profile-driven board ordering. The stable
  AUR `atvv` update to `0.3.0-1` was pushed as AUR commit `88215f3`; the
  external VCS recipes remain separate.
- **Step 26 keep-open microphone (complete, 2026-11-03)**: B03 was promoted
  and executed on the real RC003 (`C0:5D:39:F8:2D:4C`). With temporary
  verbose timestamps, 25 baseline presses measured a median 155.6 ms from the
  evdev voice edge to the first decoded frame; 20 armed presses measured
  -11.7 ms (the first frame is already queued when the edge arrives). Median
  improvement 167.3 ms, p95 improvement 179.3 ms; 0 failed opens, 0 reconnects
  beyond the initial connect, 0 unexpected stops, and a 0% 10-minute idle
  battery delta in both runs. The gate passed. `preopen_mic` (default `false`,
  `atvvctl config set --preopen-mic`) opens at `ready`, sends `MIC_EXTEND`
  every 10 s, re-opens 200 ms after a device stop, keeps `state = "ready"` and
  `streaming = false` with `mic_armed = true`, suppresses armed
  `VoiceStarted`/`VoiceStopped`, ignores `0x0F80` in `Streaming`, disables the
  idle frame timeout, and arms/disarms live. The setting is inert on a profile
  whose voice key has no local evdev edge (the Chromecast), because
  pre-opening would suppress the `AUDIO_START` that synthesizes its trigger.
  Verification: 269 workspace tests passed, 9 ignored; clippy and fmt clean; a
  live `config set --preopen-mic true/false` smoke showed `mic_armed` toggling
  and `disarming MIC_CLOSE`.
  Temporary timestamps, the `RemoteEvent::Voice` timestamp, and the
  environment switch were removed. See `docs/plans/26-preopen-mic.md` and
  `docs/research/rc003-hardware-evidence.md`.
- **Step 25 ATVV v1.0 conformance (complete, 2026-11-02)**: a `Streaming`
  session now ignores `MIC_OPEN_ERROR` instead of falling back to `Ready`;
  `Opening` still returns to `Ready` and stores the named code. Added
  `mic_open_error_name` (`0x0F02` remote inactive, `0x0F03` notifications
  disabled, `0x0F80` interaction in progress, `0x0FFF` internal error) and
  `SeqTracker`. `apply_audio_sync` is shared by the control and audio
  characteristics: it resets the decoder, counts sequence gaps, rebuilds the
  resampler for an advertised codec change, closes the stream with a named
  error for an unadvertised codec, and keeps the rate for an invalid codec
  byte. `AudioView` carries `stream_rate` and `sequence_gaps` (both
  `#[serde(default)]`). ADR 0009 has the mid-session rule, and the research
  docs no longer claim an unimplemented feature. Verification: 267 workspace
  tests passed, 9 ignored; clippy and fmt clean; the mock `atvvctl status`
  shows `"stream_rate": 16000` and `"sequence_gaps": 0`. Synthetic only: the
  RC003 and Chromecast profiles are fixed 16 kHz, so no real dynamic-rate
  device was run. See `docs/plans/25-atvv-spec-conformance.md`.
- **Step 24 first-run checklist and recovery docs (complete, 2026-11-02)**:
  the GUI menu has **Getting started…**, a read-only dialog with four
  independent checks (daemon, device, source, input/trigger) and an
  informational consumer row. It computes state in pure
  `atvv_gui::checklist::evaluate`, shows only the commands for failing or
  waiting layers, copies them to the clipboard, refreshes at 1 s only while
  visible, and auto-opens once only when the loaded document has no
  `active_device`. The status view now carries `input`, `audio.available`, and
  the contract's `profile`, `active_key`, and `recipe`; a failed `GetStatus`
  clears the checklist's view of status so the daemon row fails on the next
  refresh. Added `docs/troubleshooting.md` and `.zh-CN.md` with
  symptom/command/expected-output sections, linked them from the READMEs and
  integration indexes, and added bilingual **Known limitations** sections to
  the READMEs. Verification: 256 workspace tests passed, 9 ignored; clippy and
  fmt clean; `reuse lint` clean; 76 Markdown files link-checked; a mock-daemon
  Xvfb run confirmed no auto-open with a configured device, the menu dialog,
  clipboard content, and the daemon-loss refresh. See
  `docs/plans/24-first-run-and-troubleshooting.md`.
- **Step 23 audio feedback (complete, 2026-11-02)**: `StatusView.audio.level`
  is the decaying post-gain peak in permille (`0..=1000`); the GUI's
  **Menu → Microphone…** dialog shows it with a 200 ms dialog-only poll, sets
  the existing `gain_db` in the editable document, and saves through the
  normal **Apply** path. `PcmQueue::clear`/`AudioSource::clear`/
  `PcmSink::clear` flush the resident queue on `VoiceStarted`; a stop keeps
  the tail for the consumer. The workspace suite reported 250 passed and 9
  ignored; a throwaway real-window/mock-daemon run persisted `gain_db = 6` to
  `GetConfig` and `daemon.toml`. See `docs/plans/23-audio-feedback.md`.
- **sayall-linux comparison (research, 2026-11-02)**:
  `docs/research/sayall-linux-comparison.md` compares
  `ZXC011/sayall-linux@d2b914a` (GPL-3.0-only, behavior only) with this stack.
  It records four borrow candidates (GUI audio level/gain, session-boundary
  PCM queue flush, opt-in keep-open mic, first-run checklist), a
  troubleshooting-doc gap, and the rejected default-source/Vokie-config
  shortcuts. No code change; the hardware next action below is unchanged.
- **Google ATVV 1.0 spec summary (research, 2026-11-02)**:
  `docs/research/google-atvv-1.0-spec.md` summarizes the archived PDF without
  vendoring it (marked Google Confidential). It corrects the `GET_CAPS`
  description and the `CAPS_RESP` layout in `protocol-and-device.md`, adds the
  `MIC_OPEN_ERROR`/timeout/stream-id semantics, and records three spec gaps:
  `MIC_OPEN_ERROR` must not end a `Streaming` session, dynamic 8/16 kHz
  switching is unsupported, and `AUDIO_SYNC` sequence numbers are parsed but
  not used for gap diagnostics. No runtime change.
- **v0.2.0 release (published 2026-11-02)**: workspace version 0.2.0; the
  annotated tag `v0.2.0` (`073fa93`) published
  [the release](https://github.com/YangtseSu/atvv-linux/releases/tag/v0.2.0)
  with x86_64 (9.1 MB) and aarch64 (8.6 MB) binaries, the 2.24 MB source
  archive, checksums, and generated notes. The tag CI and the follow-up main
  CI passed; the one main failure was a language-test race, fixed in
  `ba99bcf` without changing the binaries. Highlights since v0.1.0: named
  layouts (17), app-verified presets (18), pointer actions (22), the key
  picker and BlueZ alias, and the CAPS-rate fix. The stable AUR
  `atvv`/`atvv-gui` `0.2.0-1` were built from this archive and published;
  `.SRCINFO` matches and `namcap PKGBUILD` is clean.
- **Step 21 multi-device (complete and verified)**: schema v3 adds `active_device` and a
  `[devices.<address>]` registry with per-device layouts. `atvvctl pair`
  infers the profile from the BlueZ PnP ids and sets the device active;
  `SelectDevice` swaps transport, session, input, and source (recreating
  `atvv-<profile>` on a profile change), rolls back on failure, ends
  capture/test, and persists the registry. `GetDevices`/`DeviceChanged`,
  `atvvctl device list|use|rename`, MCP `devices`/`select_device`, the GUI
  device popover with a dirty block, `StatusView.devices`, and a registry
  doctor check are implemented. The Google Chromecast Remote profile
  (`google-chromecast`, `18d1:9450`, 240-byte ADPCM frames, stream-id
  close/keepalive; the A3 accepts both forms) was added and its ATVV
  audio/pairing/reconnect run recorded. The profile uses the stream-id form so
  the documented A0 G9N9N firmware shares one profile; an A0 Linux run is
  pending. The GUI selects an SVG and hotspot table per profile; the
  Chromecast asset includes the side volume buttons. The illustration was
  redrawn from the product photo on 2026-11-02: stadium body, neutral white
  palette, photo-derived button geometry, and matching hotspots. A physical
  press log mapped Power/Home/Back/OK/Input/volume/Mute/YouTube/Netflix; the
  Assistant key sends `AUDIO_START` directly, so the daemon synthesizes the
  session trigger (`trigger_from_session`). Mock two-device switching and
  persistence passed. The clean dev package `atvv-git 0.2.0.r12.g7ec5357-1`
  (sha256 `247870309d6bbd38e2d53747417e46b9740b66b25215f6d885956478b7d2d957`)
  and `atvv-gui-git 0.2.0.r12.g7ec5357-1` were built from commit `7ec5357`;
  the installed r10 pair already runs the same trigger code.
  The board, editor, and preset dialog now derive the visible key set from
  `DeviceProfile.buttons`: the Chromecast shows Mute, YouTube, Netflix, and
  Input, hides Menu, and the preset preview drops slots the device lacks.
  Capture/test mode publishes the session-driven Assistant edge as a `voice`
  `ButtonEvent` instead of holding the configured trigger. The real RC003 +
  Chromecast run on one host passed: both devices stayed known, the source and
  grab followed the active device, the RC003 streamed from physical voice-key
  presses after switching back, and capture mode ended on a switch. Pairing a
  second device into a schema-2 file now keeps the existing device's profile.
  See `docs/plans/21-multi-device.md`.
- **Step 21 `-git` packages (built, not installed)**: `atvv-git
  0.2.0.r27.g90a3894-1` (7,045,883 bytes; sha256
  `27e49cebd03749b0b4eb872ded445d12ed7176883d45a7adb4a085f98b2e56a9`) and
  `atvv-gui-git 0.2.0.r27.g90a3894-1` (1,561,163 bytes; sha256
  `02dcb8e0feed402c8edf624a6c45f2fc988fde4ad3b136705cce9c18cbeaa170`),
  built from commit `90a3894` with the split recipe pointed at the local
  repository; the recipe is restored and the local mirror removed. They carry
  the schema-2 pairing fix. The installed r26 pair remains active until the
  operator installs them; root installation needs the operator's password.
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
- **15 mapping board (complete and verified)**: `atvv-config` now shows a
  three-column board: twelve `KeyCard`s (Power, Up, Left, Back, Home, Menu /
  Right, OK, Down, Volume +, Volume -, TV) around the original RC003 diagram,
  each with Single/Double/Long slots and compact action labels (`Return`,
  `F13`, `Ctrl+Alt+V`, `Command…`, `Start dictation`, `None`). Hotspots and
  cards share `GuiModel` selection; clicking either opens a modal `AdwDialog`
  editor with inline validation, Reset/Cancel/Update, keyboard learning,
  `Learn remote…`, and the `allow_exec` confirmation. Test mode selects and
  highlights the pressed slot; the bottom bar shows the resolved action and an
  explicit Run once. The header device chip shows the BlueZ alias (falling
  back to the profile display name/model) and opens a read-only
  device-information dialog, replacing the old sidebar. The
  diagram minimum width dropped to 130 px so the board fits 1040×760 without
  scrolling. 36 card slots and 13 hotspots were clicked through XTest under
  Xvfb. Real-daemon save/persistence: the workspace GUI saved
  `up/double = key:KEY_F13` to the live `atvv-git 0.1.0.r8.gfcd422b-1`
  daemon, the value survived `systemctl --user restart atvvd`, and the slot
  was restored; the RC003 was disconnected, so no RF press was involved.
  Screenshots: `docs/gui.png`, `docs/gui-editor.png`,
  `docs/gui-recording-key.png`. See `docs/gui.md` and
  `docs/plans/15-mapping-board.md`.
- **Step 15 `-git` packages (built, not installed)**: `atvv-git
  0.1.0.r18.g0eac227-1` (6,648,052 bytes; sha256
  `e0cd5dff9a740ee37c351e7494ff797d7af383c43a7e2d0f5eff63f540b56cfb`) and
  `atvv-gui-git 0.1.0.r18.g0eac227-1` (1,357,669 bytes; sha256
  `dd403bb1c3186ee4260e3f94a9aade7017717a852085395fc1918c60d8155030`),
  built from commit `0eac227` with the checked-in split recipe pointed at the
  local repository. `atvv-gui-git` carries `gtk4`/`libadwaita`/`librsvg` and
  depends on `atvv-git`; the base package keeps `pipewire`/`bluez`/`dbus`.
  `namcap` reports only the expected implicit-library/runtime notes; the
  packaged `atvv-config` rendered the board under Xvfb. The installed r8 pair
  remains active. The published `arch-pkgs` recipe is unchanged.
- **Key-name picker and BlueZ alias (2026-11-01)**: the action editor's key
  row gained **Choose…**, a searchable picker over
  `atvv_input::KEY_NAMES` (all 524 resolvable `KEY_*` names, common keys
  first; friendly labels from the catalog). The filter matches the canonical
  name, the localized label, and
  the English label, and ignores spaces/separators, so English queries
  (`print`, `printscreen`, `key_sysrq`) work under any locale. It inserts
  the canonical name and, in a chord, replaces only the final key and keeps
  the modifiers. Learn Key maps the GDK `Print`/`Sys_Req`
  keysyms to `KEY_SYSRQ` instead of the legacy `KEY_PRINT`;
  `printscreen`/`prtsc`/`sysrq` aliases resolve to `KEY_SYSRQ`, and the board
  shows `Print Screen`. The daemon now stores the BlueZ alias/name from
  `Connected` and reports it as `StatusView.device.name` (falling back to the
  profile display name); the GUI chip uses it, the subtitle shows
  `{model} · {state}`, and the device dialog has a Name row. Live check with
  the workspace daemon and the connected RC003: `atvvd` logged
  `connected to 小米蓝牙语音遥控器` and `atvvctl status` reported
  `device.name = 小米蓝牙语音遥控器`, `state ready`; `bluetoothctl` shows the
  same alias. Tests: keymap alias/curated-name tests, the Print Screen learn
  test, picker/chip/subtitle assertions in the UI and i18n suites, and the
  daemon ready-status assertion. Screenshots refreshed.
- **Key picker and alias `-git` packages (built, not installed)**: the
  picker/alias/locale/completeness fixes produced
  `atvv-git 0.1.0.r25.ge9fc598-1` (6,646,891 bytes; sha256
  `963dab9ed70d97ba75ccab7988ca5b07c2d855d2aece99d6d74791079433eae7`) and
  `atvv-gui-git 0.1.0.r25.ge9fc598-1` (1,374,946 bytes; sha256
  `82c23ac5b0740ad3f9bd4e1623f71cffc5e0a16cd84fa2ee7c8a0443e1ccfd79`),
  built from commit `e9fc598` with the checked-in split recipe pointed at the
  local repository. `atvv-gui-git` carries the key picker with all 524
  resolvable `KEY_*` names, locale-independent search, Print Screen mapping,
  and alias-aware chip; `atvv-git` keeps `pipewire`/`bluez`/`dbus` only. This
  supersedes the r23 build (incomplete key list) and r20 (locale-bound
  search); both were removed before install. The installed r8 pair remains
  active until the operator installs these. Both are copied to
  `~/projects/arch-pkgs/atvv-git/`.
- **Pointer actions `-git` packages (built, not installed)**: `atvv-git
  0.1.0.r27.g1b64ac7-1` (6,683,731 bytes; sha256
  `27ef3492a07ed17e8cae9ead7f8b90f50f8d7029d5f7daf5d39aa37ec9ad2adc`) and
  `atvv-gui-git 0.1.0.r27.g1b64ac7-1` (1,381,779 bytes; sha256
  `19dfbcedefb99af05c4bb51a11d21c058661370aaf2a3f822ec49939c8a66053`),
  built from commit `1b64ac7` with the split recipe pointed at the local
  repository. They carry the pointer grammar, `atvv-virtual-pointer`, the
  status field, and the GUI *Pointer* editor. The installed r8 pair remains
  active; the published recipe tracks the branch after push. Both packages
  are in `~/projects/arch-pkgs/atvv-git/`.
- **Recording-key declared chooser (2026-11-01)**: the recording-key page
  offers the contract's declared keys (`DICTATE`, `VOICECOMMAND`,
  `ASSISTANT`) plus `None (disable synthetic key)` through **Declared key…**.
  The menu is rebuilt from `StatusView.contract.recording_keys` only when the
  list changes; selecting fills the free-form output field without saving.
  The page shows the fcitx5 name mapping and warns when `vinput` is `dbus` or
  `off`. GUI status now parses `contract` and `vinput`; UI/i18n tests cover
  declared selection, `none`, custom `F23`, and the Chinese hint.
- **22 pointer actions (complete and verified)**: `atvv-input` now opens a
  dedicated `atvv-virtual-pointer` uinput device (`REL_X`/`REL_Y`,
  `INPUT_PROP_POINTER`, and eight mouse buttons; vendor `0x1209`, product
  `0x0003`) only when the action map contains a `pointer:` action. New
  grammar: `pointer:click:left|right|middle|side|extra|forward|back|task` and
  `pointer:move:up|down|left|right:<units>` with units 1–255; raw
  `key:BTN_*` is rejected with an error that names `pointer:click`. The three
  gesture slots choose the distance (`Up` Single 1, Double 10, Long 50 works).
  Test/capture mode suppresses pointer output through the existing
  `actions_enabled` flag; `RunAction` and `InputRuntime::tap_action` support
  pointer actions. `InputView.pointer` exposes the device state in
  `atvvctl status`. The GUI editor has a *Pointer* type with Click/Move,
  button, direction, and unit rows; the board renders `Left click` and
  `Pointer up N` in English and Chinese. Kernel tests observed BTN_LEFT
  press/release, REL_X/REL_Y, and the fake-remote dispatch path. Real RC003
  run (2026-11-01, `C0:5D:39:F8:2D:4C`): physical OK and Up presses produced
  `REL_Y -5`, `BTN_LEFT PRESSED`, and `BTN_LEFT RELEASED` on
  `/dev/input/event27`; the final run with `INPUT_PROP_POINTER` passed in
  7.93 s and `atvvd` was restarted.
  `docs/configuration*.md` and `docs/gui*.md` document the relative-unit and
  pointer-acceleration limitation. See `docs/plans/22-pointer-actions.md`.
- **17 layout profiles (complete and verified)**: schema v2 adds
  `active_layout` and `[layouts.<id>]` records with raw slots and gesture
  overrides. `default` is synthesized, never stored, and is a template: GUI
  slot edits are staged and Apply saves them as a new user layout. The v1 flat
  map migrates to `custom`; `default`, `mpv`, `slides`, and `omp` are
  reserved; limits are 32 layouts, 40-character names, and 120-character
  descriptions. The daemon exposes `SetLayout`/`LayoutChanged`, applies the
  active layout live with runtime rollback, and reports `StatusView.layout`.
  `atvvctl layout list|show|use|new|rename|delete|reset`,
  `config set --layout`, and `map test --layout` are implemented. The GUI has
  the header layout chip, switcher popover, manager dialog with inline
  New/Rename/Save-as forms, template staging, dirty switching block, and
  `LayoutChanged` reloads. Real RC003 (2026-11-01): `editor`
  (`up/single=key:KEY_F13`) emitted `KEY_F13` (code 183); after
  `layout use default` the next press emitted `KEY_UP` (code 103) without a
  daemon restart; `layout.id=editor` survived a restart. Docs and screenshots
  updated in English and Chinese. See `docs/plans/17-layout-profiles.md`.
- **18 layout presets (complete and verified)**: `atvv-config` carries a
  static `LayoutPreset` catalog (`mpv`, `slides`, `omp`) with verified app
  versions and all twelve single-press slots; no `none`, `exec:`, or
  `dictation:`. Preset ids are reserved; applying creates an inactive user
  layout and reset keeps the layout's name and description. CLI
  `layout preset list|show|apply` and `layout reset <id> --preset <preset>`;
  GUI **New from preset…** with a slot preview and **Reset to preset…** with
  confirmation; English and Chinese strings. Recorded runs: mpv 0.41.0
  (Space/`f`/Esc/`m`/seek/Home), Impress 26.8.1 and WPS 12.1.2 (F5,
  PageUp/PageDown, arrows, Home/End, B black screen, Esc), omp 18.14.2
  (Enter, Esc, Ctrl+R, Shift+Tab, Alt+R retry). See
  `docs/plans/18-layout-presets.md` and
  `docs/decisions/0010-app-verified-presets.md`.
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
  archive and checksums; `v0.1.0` is tagged locally. Arch recipes are outside
  the repository: one split AUR pkgbase `atvv` (`atvv`, `atvv-gui`), and one
  split VCS pkgbase `atvv-git` (`atvv-git`, `atvv-gui-git`) in the personal
  `arch-pkgs` repository. Both declare `x86_64` and `aarch64`; aarch64 was not
  built here. The split Cargo build compiles the shared graph once. See
  `docs/plans/20-release-and-external-packaging.md`.
- **Plan docs pass (2026-11-01)**: the earlier archive/format rewrite was
  reverted and `docs/plans/` stays flat. The seven live plans gained a
  `## Next action` section; the index was split into live and completed
  tables and its duplicated current-state narrative replaced by a handoff
  pointer; `[hardware]` items are tagged; steps 17/18/B02 gained a progress
  log and steps 21/B01 a verification block; step 10's text-commit criterion
  now matches the acceptance record. The plan invariants are listed in
  `AGENTS.md`.
- **Correctness fixes (2026-11-01)**: four defects found during real-device
  testing are fixed. `output_frame` takes the first-frame peak on `i32`;
  `CAPS_RESP` decides the stream rate and `atvv_audio::Resampler` converts
  each decoded frame to the resident graph rate (ADR 0009); a truncated
  `AUDIO_SYNC` returns `CtlEvent::MalformedAudioSync` and protocol-anomaly
  logging is rate limited; the dead `DeviceProfile::stop_echo` and
  `direct_audio_start` fields are gone. The 8 kHz conversion is covered by
  `cap_negotiated_8k_is_resampled_to_the_graph_rate`; no real 8 kHz device is
  verified, and the 16 kHz RC003 path is the identity path.
- **Chromecast redraw `-git` packages (built, not installed)**: `atvv-git
  0.2.0.r17.g5938e2f-1` (6,240,527 bytes; sha256
  `c6742df8d9a4f6a3c656a3d69ef2d4e63f33e2930d2eb06091a7df118076960b`) and
  `atvv-gui-git 0.2.0.r17.g5938e2f-1` (1,429,676 bytes; sha256
  `5c2a213eae50ec07f9d159b75d23ad5167734238490b061a8f7ebbf7487081ab`) were
  built from local commit `5938e2f` with the split recipe pointed at the
  local repository. The packaged `atvv-config` embeds the redrawn Chromecast
  SVG; `namcap` reports only the expected implicit-library/runtime notes. The
  installed r10 pair remains active. Both packages are in
  `~/projects/arch-pkgs/atvv-git/`.
- **Profile-driven board `-git` packages (built, not installed)**: `atvv-git
  0.2.0.r19.ge3da3c6-1` (6,241,630 bytes; sha256
  `ac17c17d1bce207c88339848ddd261ef4da7156d93a6166194a99b45b7ad4a30`) and
  `atvv-gui-git 0.2.0.r19.ge3da3c6-1` (1,436,300 bytes; sha256
  `e1ef97324cb443ed2ba97feaa654b00f6834e5dcfd35cd09679469408224e3c6`) were
  built from local commit `e3da3c6` with the split recipe temporarily pointed
  at the local repository; the recipe is restored to the GitHub source and
  the build clone's origin is back on GitHub. The packaged `atvv-config`
  embeds the profile-driven board cards and the YouTube/Netflix/Mute
  hotspots, and the packaged docs carry the new button contract. `namcap`
  reports only the expected implicit-library/runtime notes. The installed r17
  pair remains active until the operator installs these. Superseded by r21.
- **Physical-order board `-git` packages (built, not installed)**: `atvv-git
  0.2.0.r21.g9721e88-1` (6,241,632 bytes; sha256
  `686a0f88f14b69021dc652d69c1e96fe1668c6d7461894df40fe6f8d6a3c7814`) and
  `atvv-gui-git 0.2.0.r21.g9721e88-1` (1,438,370 bytes; sha256
  `4addbfe973e0845bf6f5a7989e99b824cd2ed2e4177f2d03450f73a26c83bac4`) were
  built from local commit `9721e88` with the split recipe temporarily pointed
  at the local repository; the recipe is restored to the GitHub source and
  the build clone's origin is back on GitHub. They carry the per-profile
  columns ordered top-to-bottom by physical position (RC003 6/6, Chromecast
  7/7). `namcap` reports only the expected implicit-library/runtime notes.
  The installed r17 pair remains active until the operator installs these.

## Not implemented

- **10 vinput zero-config (blocked)**: ATVV is the provider; vinput owns
  source selection and trigger binding (ADR 0007). The consumer-side feature
  is a capture-device auto-selection/priority list and auto-binding of the
  declared recording keys. Until upstream ships it, the one-time
  `vinput device use atvv-miremote-rc003` plus `Dictate` at the next free
  index (default install: `[TriggerKey] 1=Dictate`) is the correct layered
  setup, not a workaround. The ATVV-side `SetCaptureDevice` push and
  `vinput_device` policy are dropped. `vinput = "trigger"` remains the
  default; `vinput = "dbus"` is an explicit fallback. The live `daemon.toml`
  uses the default trigger with `trigger_key = "DICTATE"`; editing
  vinput's `[TriggerKey]` requires a full fcitx5 restart
  (`fcitx5-remote -r` is not enough).
  Upstream closed PRs #493–#495 and issues #491/#492 on
  2026-10-30 during a low-level architecture refactor that is not merging
  new features; the maintainer said auto-selection and recording-key binding
  are in its follow-up design and will be implemented there. Until a release
  ships, the manual setup above remains the supported path. See
  `docs/plans/10-vinput-zero-config.md`,
  `docs/research/vinput-upstream-api.md`, and
  `docs/decisions/0007-integration-direction.md`.
- **21 multi-device switching (in progress)**: schema v3 registry, explicit
  `SelectDevice`, per-device layouts, `GetDevices`/`DeviceChanged`, CLI/MCP/GUI
  switching, and a `google-chromecast` profile are implemented. Remaining: the
  real RC003+Chromecast switch on one host. Simultaneous devices stay out of
  scope. See `docs/plans/21-multi-device.md`.
- **Chromecast trigger (2026-11-02)**: the obstacle was fcitx5-vinput's
  trigger list being loaded only when the addon starts. Editing
  `[TriggerKey] 1=Dictate` and running `fcitx5-remote -r` is not enough;
  stop fcitx5, edit the file, and start fcitx5 again. After a full restart,
  the synthetic `KEY_DICTATE` from `atvvd` started vinput and it recorded from
  `atvv-google-chromecast` at 16 kHz. The live config uses the default
  `vinput = "trigger"`. The Doubao provider's earlier
  `concurrency quota exceeded` was caused by an active proxy; switching the
  proxy node fixed recognition. The D-Bus fallback remains documented for
  trigger-key conflicts, not as a workaround for this machine.
- **NFC tag (step 28, complete)**: the RC003 tag is an NTAG215 with UID
  `1D A4 2A 0B B8 00 00`, an empty NDEF message, and clear lock bytes. No
  write was made. The step closed without a helper: a dedicated blank NTAG215
  is the better trigger, and a future helper should take a configurable UID.
  NFC stays out of the daemon core. See `docs/plans/28-nfc-tag.md` and
  `docs/research/nfc-rc003.md`.

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
  `Layout` objects (schema v3) with a synthesized `default` template;
  presets are static templates that create user layouts. No automatic
  per-app/focus switching. Each known device has its own `layout` entry.
- Device facts live in `DeviceProfile`; RC003 profile id is
  `miremote-rc003` and the Chromecast profile id is `google-chromecast`.
  Button keys and gesture timing are profile fields.
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
- The fake remote profile (`test-fake-remote`, VID `0x1209`, PID `0x0002`) is
  test-only and never part of `PROFILES`.
- D-Bus boundary: `org.atvv.Daemon` at `/org/atvv/Daemon`; `GetStatus` returns
  JSON from the `StatusView` model (including the active device and the known
  device list); `GetConfig` returns the versioned `ConfigDocument`; `SetConfig`
  validates, applies, and persists atomically (it rejects an active-device
  change); `BeginCapture`/`EndCapture` control capture mode; `ListDevices`
  returns addresses; `GetDevices` returns registry/runtime JSON;
  `SelectDevice(address)` switches the active device; `StartVoice`,
  `StopVoice`, `ToggleVoice`, `ReloadConfig`; signals `StateChanged(s)`,
  `DeviceChanged(s)`, `VoiceStarted()`, `VoiceStopped()`, and
  `ButtonEvent(s,s,b)`. Protocol structs do not cross D-Bus.
- GUI: `atvv-gui` is a separate GTK4/libadwaita D-Bus client (`atvv-config`,
  `org.atvv.Config`). It uses `GetConfig`/`SetConfig`,
  `BeginCapture`/`EndCapture`, and `RunAction`; it never writes
  `daemon.toml`, grabs evdev, or touches BlueZ/PipeWire. `RunAction` parses a
  checked action and runs key/chord through the input runtime, `exec` through
  the serial worker, and dictation through the session. The daemon package
  stays GTK-free; `atvv-gui` is a split package. The mapping board keeps one
  selection model (`GuiModel.selected`/`trigger`) shared by the diagram
  hotspots, the card slots, the modal editors, and test mode; the header
  layout chip switches layouts and the device popover calls `SelectDevice`.
- Config: `$XDG_CONFIG_HOME/atvv/daemon.toml`; missing file = defaults;
  schema v3 stores the active device and a known-device registry; invalid
  file is reported by `ReloadConfig` and `atvvctl doctor`.
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
  primary path. The D-Bus `vinput` mode is an opt-in fallback. Consumer-side
  source auto-selection and auto-binding of `atvv.recording_keys` were
  proposed upstream and closed during its architecture refactor; upstream
  will implement both in its own follow-up design. `AddTriggerKey` on
  `org.fcitx.Vinput` is out of scope (ADR 0007).
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

Historical session log, kept as recorded. Step-level evidence belongs in the
step file's `## Verification`; new results go there, not here.

- 2026-11-01 step 17: `cargo fmt --all -- --check` and
  `cargo clippy --workspace --all-targets -- -D warnings` are clean;
  `cargo test --workspace` → 222 passed, 8 ignored;
  `xvfb-run -a cargo test -p atvv-gui --test ui` → 1 passed. An isolated-bus
  CLI run created, showed, switched, renamed, reset, and deleted layouts,
  refused the `default` template for edits and deletion, edited a non-active
  layout with `--layout`, and resolved `key:KEY_F13`/`key:KEY_UP` in
  `map test`. A workspace `atvvd --device C0:5D:39:F8:2D:4C --config
  /tmp/atvv17/daemon.toml` connected to 小米蓝牙语音遥控器 and grabbed
  `/dev/input/event25`; two `evtest` windows on `/dev/input/event26` (20 s and
  30 s) received no press, so the hardware key change is not yet recorded.
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
  checked-in VCS recipe tracks GitHub. The installed `atvv-git`/`atvv-gui-git`
  pair now owns those binary paths; the older `atvv-linux-git`/
  `atvv-linux-gui-git` pair is no longer installed. `namcap` reports only the
  expected implicit-library/runtime-service notes.
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

**NFC (step 28) is closed.** The attached ACR122U read the RC003 tag: NTAG215,
UID `1D A4 2A 0B B8 00 00`, empty NDEF (`03 00 FE`), clear lock bytes,
writable and not password-protected. No write was made. The UID-gated
`atvvctl mic start/stop` check passed on the connected RC003, but no
RC003-specific helper was shipped: a dedicated blank NTAG215 is the better
trigger. Evidence: [`research/nfc-rc003.md`](research/nfc-rc003.md).

**Tag and publish v0.4.0, then update the stable AUR `atvv`.** The workspace
carries 0.4.0 with the recording-key edge actions, the headless GUI test
runner, the NFC research record, and the detached-backend flake fix that
v0.3.2 missed. Push the tag `v0.4.0`, wait for the release workflow, then set
the AUR `atvv` `pkgver` to 0.4.0 with the new source-archive checksum.

**Detached-backend flake fix is in this release.** `426d451` makes
`Backend::detached` emit no events, so `tests/ui.rs` no longer races the 2 s
status poll. `v0.3.2` predates it; v0.4.0 carries it.

**v0.3.1 is published.** The release workflow and the AUR `0.3.1-1` update are
complete; the audio-sync fix needs no further code change.

**v0.3.0 is published.** The annotated tag `v0.3.0` (`ad39a49`) published the
release with x86_64/aarch64 binaries, the source archive, checksums, and
generated notes; the AUR `atvv`/`atvv-gui` `0.3.0-1` update was pushed (AUR
commit `88215f3`). `Cargo.toml` and `Cargo.lock` carry 0.3.0.

**v0.2.0 is published.** The tag `v0.2.0` (`073fa93`) has the x86_64/aarch64
binaries, source archive, checksums, and generated notes at
<https://github.com/YangtseSu/atvv-linux/releases/tag/v0.2.0>. The AUR
`atvv`/`atvv-gui` `0.2.0-1` update is published from the same source archive.
The v0.2.0 tag carries 0.2.0.

**Step 20 is complete and published.** The tag `v0.1.0`, the source archive,
and the x86_64/aarch64 binary archives are on the GitHub release. The AUR
split pkgbase `atvv` is published as `atvv`/`atvv-gui` `0.1.0-1`; the
personal `arch-pkgs` split pkgbase `atvv-git` is ready. The repository carries
no PKGBUILD. See `docs/plans/20-release-and-external-packaging.md`.

**Step 15 (mapping board) is complete and verified.** The single-key inspector
is gone: `atvv-config` shows twelve per-button cards around the RC003 diagram,
a modal action editor, a test bar, and a header device chip; see
`docs/plans/15-mapping-board.md`.

**Step 17 (layout profiles) is complete and verified.** Schema v2, live
layout switching, CLI/GUI/D-Bus support, v1 migration, and the real-RC003 key
changes are recorded in `docs/plans/17-layout-profiles.md`. The installed
`atvvd` unit was restored after the hardware run.

**Step 22 (pointer actions) is complete and verified.** Remote buttons can
emit mouse clicks and relative pointer motion through `atvv-virtual-pointer`;
the three gesture slots choose the distance, and the result was observed on
the real RC003; see `docs/plans/22-pointer-actions.md`. This closes the
keyboard-only action limitation; raw `BTN_*` is intentionally rejected in
favor of the pointer editor.

**Step 18 (layout presets) is complete and verified.** The built-in catalog
is `mpv`, `slides` (Impress/WPS), and `omp`; applying one creates an inactive
user layout, and reset keeps the layout's name and description. Real-app runs
are recorded in `docs/plans/18-layout-presets.md`; see ADR 0010.

**Step 21 (multi-device) is complete and verified.** The registry, schema v3
migration, `SelectDevice`/`GetDevices`/`DeviceChanged`, per-device layouts,
the `google-chromecast` profile, CLI/MCP/GUI switching, and the real two-device
run are recorded in `docs/plans/21-multi-device.md`. Pairing a second device
into a schema-2 file now preserves the existing device's profile.

**Step 10 vinput-zero-config is blocked** on upstream's own implementation:
ATVV is the provider and vinput is the consumer. The maintainer closed PRs
#493–#495 and issues #491/#492 on 2026-10-30 during a low-level architecture
refactor and said auto-selection and recording-key binding are in its
follow-up design. `vinput = "trigger"` stays the default; `vinput = "dbus"`
is an explicit fallback.

**Step 23 is complete and verified.** The live level/gain dialog and the
session-boundary PCM flush are in `docs/plans/23-audio-feedback.md`. Step 10
stays blocked on upstream.

**Step 25 (ATVV v1.0 conformance) is complete.** `MIC_OPEN_ERROR` no longer
interrupts a streaming session, a valid mid-session `AUDIO_SYNC` codec change
rebuilds the resampler, and sequence gaps are counted and summarized. It is
synthetic evidence only: no current profile switches rate. The plan is in
`docs/plans/25-atvv-spec-conformance.md`.

**Step 26 (keep-open microphone) is complete and verified.** The RC003 A/B
measured 155.6 ms median (n=25) with the voice-edge `MIC_OPEN` and -11.7 ms
(n=20) with an armed session; the opt-in `preopen_mic` default is `false`.
The plan is in `docs/plans/26-preopen-mic.md` and the evidence in
`docs/research/rc003-hardware-evidence.md`.

**All numbered steps are complete except step 10, which is blocked on
upstream.** Step 26 executed the keep-open experiment on the RC003 and set the
opt-in `preopen_mic` default to `false`. The immediate action is to watch
upstream vinput releases; when one ships its own auto-selection/declared-key
binding, rebuild vinput and record the zero-config RC003 run. Do not reopen the
closed PRs.

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
cargo run -q -p atvv-cli -- map test --action "pointer:click:left"
cargo run -q -p atvv-cli -- layout preset list
cargo run -q -p atvv-cli -- layout preset show omp
cargo run -q -p atvv-cli -- layout preset apply omp --as agent --activate
cargo run -q -p atvv-cli -- layout reset agent --preset slides
cargo run -q -p atvv-cli -- doctor
systemctl --user stop atvvd
cargo test -p atvv-input --test real_pointer -- --ignored --nocapture
systemctl --user start atvvd
scripts/gui-tests.sh                                    # whole GUI suite, headless
scripts/gui-tests.sh --test ui
GDK_BACKEND=x11 xvfb-run -a -s "-screen 0 1280x1024x24" \
  cargo test -p atvv-gui --test ui                     # adds the XTest pointer clicks
GDK_BACKEND=x11 xvfb-run -a -s "-screen 0 1280x1024x24" \
  cargo test -p atvv-gui --test real_run -- --ignored  # live atvvd
cargo run -q -p atvv-gui
cargo run -q -p atvv-cli -- pair C0:5D:39:F8:2D:4C
cargo run -q -p atvv-daemon -- --device C0:5D:39:F8:2D:4C --verbose
cargo run -q -p atvv-daemon -- --verbose   # device from daemon.toml
cargo run -q -p atvv-cli -- monitor
cargo run -q -p atvv-cli -- record --wav /tmp/rc003.wav --seconds 10
cargo run -q -p atvv-cli -- completions bash
cargo run -q -p atvv-cli -- manpages /tmp/atvv-man
cargo test -p atvv-input hold_and_release_reach_the_kernel -- --ignored
nfc-poll
nfc-list -t 1 -v
nfc-mfultralight r /tmp/rc003-nfc.mfd
pcsc_scan
```

## Rules

- No kernel/BlueZ/PipeWire patches.
- No root daemon; systemd user service + udev `uaccess`.
- No hardware claim without a recorded run.
- No push without explicit approval.
- Update the step file and plan index in the same commit as code.
