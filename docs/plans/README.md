# atvv-linux implementation plans

Master index for the build-out of `atvv-linux`: an ATVV v1.0 userspace stack
for Linux, with the Xiaomi Bluetooth Remote 2 Pro (RC003) as the first device.

Related: [`AGENTS.md`](../../AGENTS.md), [`docs/handoff.md`](../handoff.md),
[`docs/decisions/`](../decisions/), and [`docs/research/`](../research/).

## How these plans are used

- One file per step: `NN-kebab-case-title.md`.
- Read `docs/research/README.md` and the step's research links before editing;
  research records evidence, `docs/plans` records the decision.
- A step is worked on alone, top to bottom. Every `- ⬜` item is one unit of
  work.
- Progress uses the same markers inside each step file:
  - `Status:` is one of `⬜ not-started`, `🚧 in-progress`, `⛔ blocked`, `✅ done`.
  - `- ⬜` open / `- ✅` done on every deliverable and exit criterion.
  - `## Progress log` is append-only: `YYYY-MM-DD — note`.
- A step is `✅ done` only when every item is checked and the Verification and
  Exit criteria sections have recorded commands and observed results.
- Update the step file and this index in the same commit as the code. If reality
  diverges from the plan, change the plan in that commit and say why.
- New work is appended as a new numbered step. It runs after the existing tail.
- Hardware claims require a real-device run; synthetic tests do not count.

## Scope

In scope: ATVV v1.0+, Linux, BlueZ, PipeWire, evdev/uinput, D-Bus,
fcitx5-vinput, coding-agent integration, device profiles, packaging.

Out of scope: ATVV v0.4, ASR engines, unofficial cloud protocols,
non-ATVV transports, kernel/BlueZ/PipeWire patches, macOS/Windows.

## Phases and milestones

| Phase | Steps | Milestone |
|---|---|---|
| A — M0 protocol foundation | 01–03 | **done**: protocol, session state machine, and `atvvctl` diagnostics run without hardware |
| B — M0 system foundation | 04–06 | **done**: PipeWire source, fake uinput remote, daemon/D-Bus skeleton |
| C — Hardware bring-up | 07 | **done**: RC003 pairing state, real CAPS, audio capture, keepalive, reconnect |
| D — Integration | 08, 10, 11, 12 | 08 **done**; 10 **blocked** on upstream review/merge (issues and PRs are open); 11 **done**: evdevil input backend and `DICTATE` trigger default; 12 **done**: provider contract, browser recipe, generic IM guidance |
| E — Release | 09 | **done**: Arch package, permissions, CI, and signed hardware acceptance |
| F — Configuration | 13–14 | 13 **done**: persistent keymap, live config API, capture; 14 **in progress**: GTK4/libadwaita GNOME-HIG app implemented, physical test-mode press pending |

M0 is steps 01–06. Step 07 (hardware bring-up) is complete with a real RC003;
step 08 is complete and verified; step 09 is complete. Step 10 is blocked
on approval/upstream; steps 11–13 are complete.

## Step files

| # | Phase | Step | Status | Depends on |
|---|-------|------|--------|------------|
| 01 | A | [protocol-core](01-protocol-core.md) | ✅ done | — |
| 02 | A | [session-state-machine](02-session-state-machine.md) | ✅ done | 01 |
| 03 | A | [cli-diagnostics](03-cli-diagnostics.md) | ✅ done | 01, 02 |
| 04 | B | [pipewire-source](04-pipewire-source.md) | ✅ done | 01 |
| 05 | B | [input-uinput](05-input-uinput.md) | ✅ done | 01 |
| 06 | B | [daemon-dbus](06-daemon-dbus.md) | ✅ done | 01, 02, 04, 05 |
| 07 | C | [rc003-hardware-bringup](07-rc003-hardware-bringup.md) | ✅ done | 01, 02, 04, 06 |
| 08 | D | [vinput-and-mcp](08-vinput-and-mcp.md) | ✅ done | 03, 06, 07 |
| 09 | E | [packaging-and-acceptance](09-packaging-and-acceptance.md) | ✅ done | all of 01–08 |
| 10 | D | [vinput-zero-config](10-vinput-zero-config.md) | ⛔ blocked | 08 |
| 11 | D | [input-evdevil](11-input-evdevil.md) | ✅ done | 05, 08, 10 |
| 12 | D | [integration-contract](12-integration-contract.md) | ✅ done | 03, 06 (consumer recipe: 10) |
| 13 | F | [keymap-config](13-keymap-config.md) | ✅ done | 12, 06 |
| 14 | F | [config-gui](14-config-gui.md) | 🚧 in-progress | 13 |

## Architecture contract

```text
ATVV remote
  ├─ BLE HID ──> BlueZ ──> evdev ──> atvv-input ──> uinput ──> compositor/IME
  └─ ATVV GATT ─> BlueZ ──> atvv-daemon (atvv-protocol) ──> PipeWire source
                                                   └────────> D-Bus / MCP
```

Binding rules:

- `atvv-protocol` is pure: no BlueZ, PipeWire, tokio, or input dependencies.
- The session state machine emits actions; it never calls a transport or sleeps.
- `CAPS_RESP` is authoritative for the codec. `AUDIO_START` payload bytes are
  an event, not a codec source.
- The daemon owns the GATT session, source lifecycle, reconnect, and D-Bus.
- `atvvctl` formats state; it does not duplicate protocol logic.
- Device facts live in `DeviceProfile`, not in conditionals scattered through
  the daemon.
- ATVV is the provider: it exposes a PipeWire source, a standard input key,
  and read-only status. Consumers such as fcitx5-vinput own source selection
  and trigger binding; ATVV does not write their configuration or call their
  daemon in the primary path (ADR 0007).
- The GUI is an optional GTK4/libadwaita client over D-Bus; the daemon
  remains headless and must not gain GUI dependencies (ADR 0008).
- The source advertises the provider contract: `atvv.contract_version=1`,
  `atvv.profile`, and `atvv.recording_keys` (`DICTATE`, `VOICECOMMAND`,
  `ASSISTANT`). Consumers bind the declared set; the provider emits one active
  key. See
  [`docs/integrations/provider-contract.md`](../integrations/provider-contract.md).

## RC003 profile facts

`miremote-rc003` is ATVV v1.0, 16 kHz IMA/DVI ADPCM, 120-byte headerless
frames, high-nibble-first, short `MIC_OPEN` `[0C 00]`, bare `MIC_CLOSE` `[0D]`,
bare `MIC_EXTEND` `[0E]`, direct `AUDIO_START`, 0 dB gain, 300 ms post-stop
discard, and an `AUDIO_STOP` echo that must not trigger another close.

## M0 exit criteria

- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D
  warnings`, and `cargo test --workspace` are clean.
- `atvvctl profile show`, `caps decode`, and `simulate` produce the documented
  results.
- A PipeWire source can be created and recorded from without a remote (step 04).
- A fake uinput remote exercises the input path without a physical device
  (step 05).
- ✅ A mock BLE session drives the daemon D-Bus API, including a direct
  `AUDIO_START` and an ignored stop echo (step 06).
- No claim of RC003 hardware support is made before step 07.

## Current state — 2026-10-31

Steps 01–09 and 11–14 are implemented; step 14 awaits a recorded physical
test-mode press. Step 07 ran on a real RC003
(`C0:5D:39:F8:2D:4C`): real `CAPS_RESP`, direct `AUDIO_START`, 120-byte
frames, a 60-second keepalive session, a non-silent 16 kHz WAV, and automatic
reconnect without a daemon restart. The run also fixed the resident PipeWire
source, which had stayed suspended without a playback driver. Full evidence is
in [`docs/research/rc003-hardware-evidence.md`](../research/rc003-hardware-evidence.md).

Step 08 verified the voice-edge mic control, the synthetic trigger, the
`org.fcitx.Vinput` client, `atvv-mcp`, and the fcitx/source path at 16 kHz;
the same evidence document records the real-press and automated runs.

Step 09 packaging and acceptance is complete: `atvv-linux 0.1.0-3` is the
acceptance baseline, `0.1.0-5` is installed and active with the provider contract; the enabled user unit starts
`/usr/bin/atvvd`, and `atvvctl doctor` reports all green on the real RC003.
The end-to-end fcitx text commit is operator-confirmed and the vinput journal
records the ATVV source at 16 kHz. The completed record is
[`docs/acceptance/rc003-2026-10-30.md`](../acceptance/rc003-2026-10-30.md).
CI also runs `cargo deny` and `reuse lint`, and Dependabot checks Cargo and
GitHub Actions daily.

Step 10 is blocked on upstream review/merge. ATVV is the
provider: it exposes the resident `Audio/Source` and the standard `Dictate`
key. vinput is the consumer and owns source selection and trigger binding.
Issues
[#491](https://github.com/xifan2333/fcitx5-vinput/issues/491) and
[#492](https://github.com/xifan2333/fcitx5-vinput/issues/492) are open. Local
consumer patches exist in the `YangtseSu/fcitx5-vinput` fork on three stacked
issue-named branches (auto selection, recording-key metadata, and `device use`
binding); the branches are pushed and
[PR #493](https://github.com/xifan2333/fcitx5-vinput/pull/493) is ready, with
[PR #494](https://github.com/xifan2333/fcitx5-vinput/pull/494) and
[PR #495](https://github.com/xifan2333/fcitx5-vinput/pull/495) as drafts. The
manual `device use` plus `Dictate` path is verified; automatic binding and the
post-merge real check remain. The primary upstream request is a
consumer-side capture-device auto-selection/priority list plus auto-binding of
the declared recording keys; until it exists, the one-time
`vinput device use` plus `Dictate` at the next free index (default
`1=Dictate`) is the correct layered setup.
The earlier ATVV-side `SetCaptureDevice` push and the `vinput_device` policy
are dropped (ADR 0007). D-Bus mode remains an opt-in fallback, not the
zero-config default. The daemon trigger default is `DICTATE` (`KEY_DICTATE`);
the declared set is `DICTATE`, `VOICECOMMAND`, `ASSISTANT`. Evidence:
[`docs/research/vinput-upstream-api.md`](../research/vinput-upstream-api.md) and
[ADR 0007](../decisions/0007-integration-direction.md).

Step 12 is complete: the provider contract
(`atvv.contract_version`, `atvv.profile`, `atvv.recording_keys`) is on the
source and documented in
[`docs/integrations/provider-contract.md`](../integrations/provider-contract.md).
`atvvctl contract [--json]`, the `contract` object in `atvvctl status`, the
`atvv-mcp` `contract` tool, the browser recipe, and generic IM guidance are
implemented. The dependency is `03, 06`; the vinput consumer recipe is step 10
and is blocked. Automatic fcitx5-vinput binding is not claimed here. The target
consumers are fcitx5-vinput, browser AI chat pages, coding agents (Claude Code,
Codex, OMP), and IM applications (Telegram, WeChat). No per-application
diagnostics are provided.

Step 11 is complete. `atvv-input` now uses `evdevil` 0.5.0 instead of
`evdev` 0.13.2, so `VOICECOMMAND`, `ASSISTANT`, and `DICTATE` resolve and
emit. The default synthetic trigger is `DICTATE`; the daemon and vinput names
are documented side by side. A real RC003 press at 2026-10-30 produced
`KEY_DICTATE`, streamed 104 frames / 24,960 samples, and made fcitx5-vinput
record from `atvv-miremote-rc003` at 16 kHz. The evdev fork experiment was
abandoned; the decision and evidence are in
[ADR 0006](../decisions/0006-evdevil-input-backend.md) and
[`consumer-keys.md`](../research/consumer-keys.md). Context-dependent mapping
of the three keys is deferred.

Step 13 is complete: `atvv-config` is the single schema parser; per-button
mappings, gesture overrides, `exec:` opt-in, and live D-Bus
`GetConfig`/`SetConfig`/capture are implemented and verified with the fake
remote.

Step 14 is implemented and packaged: `atvv-gui` (`atvv-config`) is a GTK4/libadwaita
client with an embedded SVG remote diagram, twelve accessible hotspots, a
per-slot action editor, keyboard and remote learning, test mode with an
explicit Run once, `exec` confirmation, banners/toasts, and a daemon-unavailable
state. `RunAction` was added to `org.atvv.Daemon` for test mode. The worker
tests use a private bus and the real daemon service; a headless `xvfb-run`
test drives the window. A real RC003 run saved a mapping through the GUI and
verified it after a daemon restart. The physical test-mode press is still
open. Packaging is a split `atvv-linux-gui` package with desktop entry,
AppStream metadata, icon, and GTK runtime dependencies kept out of
`atvv-linux`; `0.1.0-7` is built and staged but not installed. The GUI decision is recorded in
[ADR 0008](../decisions/0008-gui-toolkit.md). Step 10 remains blocked on
upstream review.

## Backlog after 14

- AUR/release packaging for the GUI; AppStream and screenshots.
- Optional tray/status indicator: connection, recording, battery, error.
- PipeWire initial-start retry; suspend/resume and adapter power-cycle tests.
- Second ATVV v1.0 device to validate the profile abstraction.
- zbus 6 migration and removal of ignored zbus 5 advisories.
- Optional manual layers/profiles; no automatic per-app focus detection.
- Optional import/export and config migration UI.
