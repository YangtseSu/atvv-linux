# Research notes

This directory preserves the evidence behind the implementation plan. It is
not a second plan and it does not override `docs/plans/`.

## Documents

- [`landscape.md`](landscape.md) — Linux layers, reference-project evaluation,
  extraction matrix, recommended architecture, project-level risks.
- [`protocol-and-device.md`](protocol-and-device.md) — ATVV v1.0 wire facts,
  RC003 profile, session behavior, BlueZ/PipeWire/input facts, fcitx5-vinput
  interface.
- [`reference-projects.md`](reference-projects.md) — exact source URLs, pinned
  snapshots, licenses, reuse boundaries, and upstream references.
- [`consumer-keys.md`](consumer-keys.md) — `KEY_VOICECOMMAND`,
  `KEY_ASSISTANT`, and `KEY_DICTATE`: kernel/XKB/fcitx5 names, desktop usage,
  the `DICTATE` trigger default, `evdev`/`evdevil` support, and the deferred
  context-dependent mapping.
- [`vinput-upstream-api.md`](vinput-upstream-api.md) — fcitx5-vinput
  `bd7dd4365` D-Bus surface, capture-device ownership, addon-owned trigger
  keys, the external-start focus gap, and the resulting integration direction.
- [`layout-profiles-presets.md`](layout-profiles-presets.md) — reference
  models for named layouts and presets (Open Voice Bridge, mi-ao, vimo_remote,
  sayall-omarchy, remote-mic-app), the two-layer conclusion, and the RC003 NFC
  facts/unknowns.
- [`rc003-hardware-evidence.md`](rc003-hardware-evidence.md) — real RC003
  runs (2026-10-30): CAPS hex, start/stop bytes, audio, keepalive, reconnect,
  PipeWire behavior, the step 08 trigger/compositor run and the automated
  fcitx/source check, and the fixes found. The combined real-press fcitx
  recording was not observed; its legs were verified separately.

## Evidence rules

- A claim is **verified** only with a command, observed output, date, and (for
  hardware) device model/address. Upstream reports and synthetic tests are
  evidence for a code path, not for the device.
- The first RC003 run is recorded in
  [`rc003-hardware-evidence.md`](rc003-hardware-evidence.md) (2026-10-30,
  `C0:5D:39:F8:2D:4C`). Values not covered there remain unverified.
- Do not put untrusted web pages in `docs/plans/`. Record the fact and its URL
  here first.
- Do not copy GPL-3.0-only code. See the license table in
  [`reference-projects.md`](reference-projects.md).
- Reference external projects by GitHub URL and pinned commit. Do not record
  local clone paths.

## Read map

| Work | Read |
|---|---|
| 05 input/uinput | `landscape.md` §3.2 and §5, `protocol-and-device.md` §6 |
| 06 daemon/D-Bus | `landscape.md` §3.1 and §6, `protocol-and-device.md` §7 |
| 07 RC003 hardware | `protocol-and-device.md` §2–§5, `reference-projects.md`, `rc003-hardware-evidence.md` |
| 08 fcitx/MCP | `protocol-and-device.md` §8, `landscape.md` §6, `rc003-hardware-evidence.md` §Step 08 |
| 09 packaging | `reference-projects.md` §License boundaries, `landscape.md` §4 |
| 10 vinput zero-config | `vinput-upstream-api.md`, `protocol-and-device.md` §8 |
| 11 input backend | `consumer-keys.md`, `reference-projects.md` §License boundaries |
| 12 integration contract | `../integrations/provider-contract.md`, `../decisions/0007-integration-direction.md` |
| 17 layout profiles | `layout-profiles-presets.md` §1–§3, `../configuration.md` §Schema |
| 18 layout presets | `layout-profiles-presets.md` §2–§3, the step 17 file |
| B02 NFC | `layout-profiles-presets.md` §4, `rc003-hardware-evidence.md` |

## Current deltas after the research snapshot

The research was recorded on 2026-10-29. These decisions changed the
implementation direction:

- **ATVV v1.0 only.** v0.4, Philips vendor handshake, and multi-version
  dispatch are out of scope. Do not add them without changing the plan.
- **Original implementation.** The code is original; the reference projects
  are used for behavior and protocol facts, not copied source. MIT references
  still require notices if code is ever reused.
- **Names.** The D-Bus name is `org.atvv.Daemon`; the CLI is `atvvctl`; the
  device profile id remains `miremote-rc003`.
- **PipeWire lifecycle.** `atvv-audio` recreates its node after registry
  removal. The source sets `node.pause-on-idle=false` so a linked consumer
  wakes it without a playback driver; `node.always-process` freewheels and is
  not used. A full PipeWire service restart is owned by the systemd unit
  (`PartOf=pipewire.service`, `Restart=always`), not by an in-process monitor
  (ADR 0005).
- **Hardware state.** Step 07 ran on a real RC003 (`C0:5D:39:F8:2D:4C`) on
  2026-10-30. Steps 01–09 and 11 are complete; step 10 remains in progress.
  Step 09's root package install, post-reboot run, and end-to-end fcitx text
  commit are recorded in
  [`../acceptance/rc003-2026-10-30.md`](../acceptance/rc003-2026-10-30.md).
  Step 11's evdevil run emitted `KEY_DICTATE` and made fcitx5-vinput record
  from the ATVV source. The verified facts are in
  [`rc003-hardware-evidence.md`](rc003-hardware-evidence.md).
- **Input backend.** `evdev` 0.13.2's manual key table was stale and lacked
  `KEY_DICTATE`; the fork experiment was abandoned. Step 11 migrated
  `atvv-input` to `evdevil` 0.5.0, which includes the three consumer keys and
  the required evdev/uinput surface. The daemon default trigger is now
  `DICTATE`. The evdev names used by the daemon
  (`VOICECOMMAND`, `ASSISTANT`, `DICTATE`) and the fcitx5-vinput names
  (`VoiceCommand`, `Assistant`, `Dictate`) remain distinct. See
  [`consumer-keys.md`](consumer-keys.md) and
  [`../decisions/0006-evdevil-input-backend.md`](../decisions/0006-evdevil-input-backend.md).
- **Layouts.** `DeviceProfile` remains the hardware/firmware identity. User
  keymaps will be `Layout` objects in schema v2, with a read-only `default`
  synthesized from the profile. Presets are read-only templates that create
  user layouts; they never become the active document. See
  [`layout-profiles-presets.md`](layout-profiles-presets.md).
- **vinput upstream API.** A 2026-10-30 source review of fcitx5-vinput
  `bd7dd4365` reduced step 10's upstream scope to the consumer side: ATVV
  provides the PipeWire source and the `Dictate` key; vinput owns source
  selection and trigger binding. The primary upstream request is a
  capture-device auto-selection/priority list. `AddTriggerKey` on
  `org.fcitx.Vinput` is not viable, and ATVV does not call
  `SetCaptureDevice` in the primary path (ADR 0007). See
  [`vinput-upstream-api.md`](vinput-upstream-api.md).
- **Consumer integration.** The provider contract is implemented and recorded
  in `AGENTS.md` and
  [`../integrations/provider-contract.md`](../integrations/provider-contract.md);
  `atvvctl contract [--json]`, the `contract` object in `status`, and the
  `atvv-mcp` `contract` tool expose it:
  PipeWire source, declared recording keys (`DICTATE`, `VOICECOMMAND`,
  `ASSISTANT`), generic control/status, and consumer-owned configuration.
  Per-application diagnostics are out of scope.
- **RC003 mic gating.** A host `MIC_OPEN` alone produces `AUDIO_START` but no
  audio frames; the physical voice key must be held. The daemon opens
  `MIC_OPEN` on the grabbed voice-key edge and closes on release, and the input
  runtime synthesizes the configured fcitx trigger program (a key or chord).
  The device may also send its own direct `AUDIO_START`; the session treats
  either order as streaming.

## Open questions

These cannot be closed by reading code or by synthetic tests:

- Whether the `AUDIO_STOP` echo is always absent on HTT release, and what
  other stop reasons emit. (No echo was observed after `00 02`.)
- Real 16 kHz clipping on loud speech and human/ASR intelligibility. (The
  capture is non-silent with peaks near full scale.)
- Pairing gesture and LE Secure Connections numeric-comparison behavior. (The
  device was already bonded during step 07.)
- Whether a single real-device press (voice key → daemon → synthetic trigger →
  fcitx) records from `atvv-miremote-rc003` and commits text. **Closed
  2026-10-30 for recording, not for text:** the step-11 press produced
  `KEY_DICTATE`, 104 frames / 24,960 samples, and fcitx5-vinput recorded from
  the ATVV source at 16 kHz. No speech was captured, so the transcript check
  remains a separate operator action.
- fcitx5-vinput warm-stream behavior with the real daemon.
- Which physical RC003 controls map to `VOICECOMMAND`, `ASSISTANT`, and
  `DICTATE`, and whether the mapping should change with the active
  application/workflow. The current plan makes the codes available; the
  policy is deliberately undecided.
- The root-installed unit after a real reboot. The unit's `PartOf` restart
  behavior was verified with a transient unit using the packaged properties;
  `pacman -U` into `/usr` and the post-reboot run remain operator actions.
