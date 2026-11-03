# Research notes

This directory preserves the evidence behind the implementation plan. It is
not a second plan and it does not override `docs/plans/`.

## Documents

- [`landscape.md`](landscape.md) — Linux layers, reference-project evaluation,
  extraction matrix, recommended architecture, project-level risks.
- [`protocol-and-device.md`](protocol-and-device.md) — ATVV v1.0 wire facts,
  RC003 profile, session behavior, BlueZ/PipeWire/input facts, fcitx5-vinput
  interface.
- [`google-atvv-1.0-spec.md`](google-atvv-1.0-spec.md) — original summary of
  the archived Google Voice over BLE 1.0 spec: payload layouts, open/close
  rules, error codes, timeouts, and the project deltas; the PDF itself is not
  redistributed.
- [`reference-projects.md`](reference-projects.md) — exact source URLs, pinned
  snapshots, licenses, reuse boundaries, and upstream references.
- [`sayall-linux-comparison.md`](sayall-linux-comparison.md) — ZXC011
  `sayall-linux` snapshot: architecture comparison, last-mile UX borrow list
  (level meter, session pre-roll, first-run checklist, troubleshooting), and
  the integration patterns we reject.
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
- [`nfc-rc003.md`](nfc-rc003.md) — the RC003 NFC tag read on 2026-11-03:
  ACR122U reader and firmware, NTAG215 identity and UID, empty NDEF, lock
  bytes, dump hash, and the UID-gated D-Bus check.
- [`rc003-hardware-evidence.md`](rc003-hardware-evidence.md) — real RC003
  runs (2026-10-30): CAPS hex, start/stop bytes, audio, keepalive, reconnect,
  PipeWire behavior, the step 08 trigger/compositor run and the automated
  fcitx/source check, and the fixes found. The combined real-press fcitx
  recording was not observed; its legs were verified separately.
- [`chromecast-hardware-evidence.md`](chromecast-hardware-evidence.md) — real
  Google Chromecast Remote run (2026-11-02, `74:46:B3:22:2D:D5`): DIS/PnP
  identity and authenticity limits, ten-byte `CAPS_RESP` with 240-byte
  frames, `MIC_OPEN`/`MIC_CLOSE`, bare and stream-id close/keepalive
  acceptance, source contract, HID map, reconnect, and the two-device switch
  with the RC003.

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
- Do not vendor the Google Voice over BLE PDF: it is marked Google
  Confidential and has no redistribution grant. Cite the archive link and the
  original summary in [`google-atvv-1.0-spec.md`](google-atvv-1.0-spec.md).
- Reference external projects by GitHub URL and pinned commit. Do not record
  local clone paths.

## Read map

| Work | Read |
|---|---|
| 01/02 protocol | `protocol-and-device.md`, `google-atvv-1.0-spec.md` |
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
| 21 multi-device | `chromecast-hardware-evidence.md`, `protocol-and-device.md` §7 |
| 28 NFC | `layout-profiles-presets.md` §4, `nfc-rc003.md` |
| Consumer UX / troubleshooting | `sayall-linux-comparison.md` §3, `../integrations/provider-contract.md` |

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
  A second real device, the Google Chromecast Remote
  (`74:46:B3:22:2D:D5`, profile `google-chromecast`), passed ATVV audio,
  keepalive, and a profile-inference pairing run on 2026-11-02. See
  [`chromecast-hardware-evidence.md`](chromecast-hardware-evidence.md); the A0
  firmware run remains pending.
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
  selection and trigger binding. The ATVV proposals (auto-selection and
  declared-key binding) were closed upstream on 2026-10-30 during its
  architecture refactor; upstream will implement both in its own follow-up
  design. `AddTriggerKey` on `org.fcitx.Vinput` is not viable, and ATVV does
  not call `SetCaptureDevice` in the primary path (ADR 0007). See
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
- **Keep-open microphone.** The 2026-11-03 RC003 A/B measured a 155.6 ms
  median from the voice edge to the first decoded frame with the default
  voice-edge `MIC_OPEN` (n=25), and the first frame already queued at the edge
  with an armed session (n=20; median improvement 167.3 ms). The path is the
  opt-in `preopen_mic` setting, default `false`; the armed session stays
  invisible to consumers and the 10-minute idle battery delta matched
  baseline. See
  [`rc003-hardware-evidence.md`](rc003-hardware-evidence.md#keep-open-mic-ab-2026-11-03)
  and [step 26](../plans/26-preopen-mic.md).

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
