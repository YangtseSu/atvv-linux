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

## Evidence rules

- A claim is **verified** only with a command, observed output, date, and (for
  hardware) device model/address. Upstream reports and synthetic tests are
  evidence for a code path, not for the device.
- No RC003 hardware has been tested in this repository. Step 07 owns the first
  real-device evidence.
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
| 07 RC003 hardware | `protocol-and-device.md` §2–§5, `reference-projects.md` |
| 08 fcitx/MCP | `protocol-and-device.md` §8, `landscape.md` §6 |
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
  removal. A full PipeWire service restart is owned by the systemd unit
  (`PartOf=pipewire.service`, `Restart=always`), not by an in-process monitor
  (ADR 0005).
- **M0 state.** Steps 01–04 are complete; 05–09 are planned. No hardware claim
  is made.

## Open questions

These cannot be closed by reading code or by synthetic tests:

- Real RC003 `CAPS_RESP` bytes, `AUDIO_STOP` reason, echo timing, and whether
  the stop echo is always present.
- Real 16 kHz gain, clipping, and microphone intelligibility.
- Pairing gesture and LE Secure Connections numeric-comparison behavior.
- fcitx5-vinput warm-stream behavior with the real daemon.
- PipeWire service restart behavior of the installed systemd unit.
