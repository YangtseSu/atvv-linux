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
| C — Hardware bring-up | 07 | RC003 pairing, real CAPS, audio capture, reconnect |
| D — Integration | 08 | fcitx5-vinput trigger/D-Bus and a proper MCP surface |
| E — Release | 09 | Arch packaging, permissions, CI, hardware acceptance |

M0 is steps 01–06. Steps 07–09 require a device or a release decision.

## Step files

| # | Phase | Step | Status | Depends on |
|---|-------|------|--------|------------|
| 01 | A | [protocol-core](01-protocol-core.md) | ✅ done | — |
| 02 | A | [session-state-machine](02-session-state-machine.md) | ✅ done | 01 |
| 03 | A | [cli-diagnostics](03-cli-diagnostics.md) | ✅ done | 01, 02 |
| 04 | B | [pipewire-source](04-pipewire-source.md) | ✅ done | 01 |
| 05 | B | [input-uinput](05-input-uinput.md) | ✅ done | 01 |
| 06 | B | [daemon-dbus](06-daemon-dbus.md) | ✅ done | 01, 02, 04, 05 |
| 07 | C | [rc003-hardware-bringup](07-rc003-hardware-bringup.md) | ⬜ not-started | 01, 02, 04, 06 |
| 08 | D | [vinput-and-mcp](08-vinput-and-mcp.md) | ⬜ not-started | 03, 06, 07 |
| 09 | E | [packaging-and-acceptance](09-packaging-and-acceptance.md) | ⬜ not-started | all of 01–08 |

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

## Current state — 2026-10-30

M0 (steps 01–06) is complete without hardware. The PipeWire source is verified
for creation, recording, and node-removal recreation; full PipeWire service
restart is owned by systemd (ADR 0005) and must be verified in step 09. The
input path is verified with a fake uinput remote: discovery, `EVIOCGRAB`,
gesture timing, and compositor-visible uinput keys. The daemon's mock
transport drives the real `Session` over `org.atvv.Daemon`; D-Bus signals,
introspection, start/stop, the ignored stop echo, and config reload are covered
by an isolated-bus integration test. The workspace has 88 passing tests and one
ignored kernel-uinput test. Steps 07–09 are planned; step 07 is blocked on an
RC003 and the hardware acceptance part of step 09 depends on it.
