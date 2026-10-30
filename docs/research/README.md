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
  2026-10-30. Steps 01–07 are complete; step 08 is implemented and awaiting
  one untimed live fcitx capture; step 09 remains. The verified facts are in
  [`rc003-hardware-evidence.md`](rc003-hardware-evidence.md).
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
  fcitx) records from `atvv-miremote-rc003`. The legs were verified separately:
  a real press produced ATVV audio and the trigger reached the compositor, and
  an automated focused-entry test made fcitx record from the ATVV source at
  16 kHz. A future joint spot check is useful but must not be time-boxed.
- fcitx5-vinput warm-stream behavior with the real daemon.
- PipeWire service restart behavior of the installed systemd unit.
