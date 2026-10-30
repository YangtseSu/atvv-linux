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
| D — Integration | 08 | **done**: fcitx5-vinput trigger/D-Bus, MCP surface, and an automated fcitx/source check |
| E — Release | 09 | Arch packaging, permissions, CI, hardware acceptance |

M0 is steps 01–06. Step 07 (hardware bring-up) is complete with a real RC003;
step 08 is complete and verified; step 09 is next.

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
| 09 | E | [packaging-and-acceptance](09-packaging-and-acceptance.md) | 🚧 in-progress | all of 01–08 |

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

Steps 01–08 are complete. Step 07 ran on a real RC003
(`C0:5D:39:F8:2D:4C`): real `CAPS_RESP`, direct `AUDIO_START`, 120-byte
frames, a 60-second keepalive session, a non-silent 16 kHz WAV, and automatic
reconnect without a daemon restart. The run also fixed the resident PipeWire
source, which had stayed suspended without a playback driver. Full evidence is
in [`docs/research/rc003-hardware-evidence.md`](../research/rc003-hardware-evidence.md).

Step 08 verified the voice-edge mic control, the synthetic trigger, the
`org.fcitx.Vinput` client, `atvv-mcp`, and the fcitx/source path at 16 kHz;
the same evidence document records the real-press and automated runs.

Step 09 packaging is implemented: `atvv-linux 0.1.0-1` builds from the
deterministic source tarball and installs the binaries, user unit, udev rule,
modules-load file, man pages, and completions. The release and staged-package
binaries passed `atvvctl doctor`, 16 kHz source, reconnect, and PipeWire
restart checks on the real RC003. Remaining acceptance work is the root
`pacman -U` install, a post-reboot run, and the end-to-end fcitx text commit;
see [`docs/acceptance/rc003-2026-10-30.md`](../acceptance/rc003-2026-10-30.md).
CI also runs `cargo deny` and `reuse lint`, and Dependabot checks Cargo and
GitHub Actions daily. Step 09 stays in progress until those operator checks
and the sign-off are recorded.
