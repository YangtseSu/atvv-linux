# atvv-linux implementation plans

Master index for the build-out of `atvv-linux`: an ATVV v1.0 userspace stack
for Linux, with the Xiaomi Bluetooth Remote 2 Pro (RC003) as the first device.

Related: [`AGENTS.md`](../../AGENTS.md), [`docs/handoff.md`](../handoff.md),
[`docs/decisions/`](../decisions/), and [`docs/research/`](../research/).

## How these plans are used

The live plan is one file per scheduled step: `NN-kebab-case-title.md`.
Deferred work that is understood but not scheduled is a backlog file:
`B<NN>-kebab-case-title.md`. This index is the canonical order, the phase
table, and the backlog table.

- Read `docs/research/README.md` and the step's research links before editing;
  research records evidence, `docs/plans` records the decision.
- A step is worked on alone, top to bottom. Every `- ⬜` item is one unit of
  work.
- Progress uses the same markers inside every step and backlog file. This list
  is their only definition; other documents link here.
  - `Status:` is one of `⬜ not-started`, `🚧 in-progress`, `⛔ blocked`,
    `✅ done`, `⏸ backlog`.
  - `- ⬜` open / `- ✅` done on every deliverable and exit criterion.
  - `## Progress log` is append-only: `YYYY-MM-DD — note`.
- A file that is not `✅ done` carries a `## Next action` section: the single
  action that resumes it.
- An item tagged `[hardware]` needs a recorded run on physical hardware (the
  RC003, or an NFC reader for B02): command, observed output, date, and the
  device address or model. Synthetic tests do not satisfy it.
- A step is `✅ done` only when every item is checked, including every
  `[hardware]` item, and the Verification and Exit criteria sections have
  recorded commands and observed results.
- A step file reports its own evidence. `docs/research/` holds research,
  `docs/decisions/` holds decisions, and `docs/handoff.md` holds current state;
  do not copy that content into a plan file.
- Update the step file and this index in the same commit as the code. If
  reality diverges from the plan, change the plan in that commit and say why.
- Keep the `## Next action` line current whenever the resume point changes.
- The plan invariants checked before a commit are listed in
  [`AGENTS.md`](../../AGENTS.md).
- New scheduled work is appended as a new numbered step. A B-series entry is
  promoted to the next number when it is scheduled; its file keeps the
  evidence and its progress log gains the promotion note.
- A one-off task (a CI fix, a documentation edit, a bumped constant, a bug
  fixed on the spot) is a `fix:`/`chore:`/`docs:` commit and does not create a
  plan file.
- Do not cite `docs/plans/` from code: plans are the process record, not a
  runtime contract. Code comments carry the reason and measured value inline.

## Backlog (B series)

The B series holds deferred work. It is not part of the numbered execution
order until promoted. A backlog entry still carries a real exit criterion so
it can be picked up without re-deriving the design.

| # | Step | Status | Depends on | Exit, in one line |
|---|------|--------|------------|-------------------|
| B01 | [Recording-key press and release actions](B01-recording-key-press-release-actions.md) | ⏸ backlog | 14 | one press and one release action run per edge while the mic, trigger key, and provider contract stay unchanged |
| B02 | [NFC tag investigation and optional trigger](B02-nfc-tag.md) | ⏸ backlog | — (needs an NFC reader) | the RC003 tag's IC, UID, NDEF, and lock state are recorded, and a read-only trigger is demonstrated or the rewrite path is restored exactly |
| B03 | [Keep-open microphone experiment](B03-preopen-mic.md) | ⏸ backlog | 02, 04, 06, 07 | an RC003 A/B measures first-frame latency and idle battery, and the opt-in path is implemented only if it clears the gate |

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
| D — Integration | 08, 10, 11, 12 | 08 **done**; 10 **blocked** on upstream's own implementation after the maintainer closed the ATVV PRs during an architecture refactor; 11 **done**: evdevil input backend and `DICTATE` trigger default; 12 **done**: provider contract, browser recipe, generic IM guidance |
| E — Release | 09 | **done**: Arch package, permissions, CI, and signed hardware acceptance |
| F — Configuration | 13–15 | 13 **done**: persistent keymap, live config API, capture; 14 **done**: GTK4/libadwaita GNOME-HIG app, clickable diagram, test mode; 15 **done**: mapping board with per-button cards, modal action editor, test bar |
| G — Device information | 16 | **done**: battery, firmware, and DIS facts in status, CLI, MCP, and the GUI home view |
| H — Layout profiles | 17–18 | 17 **done**: named user layouts with a template default, live switching, CLI/GUI/D-Bus support, v1 migration, and recorded RC003 key changes. 18 **done**: app-verified presets for mpv, Impress/WPS presentation, and omp |
| I — Localization | 19 | **done**: GUI en/zh-Hans catalog and bilingual user docs |
| J — Release | 20 | **done**: v0.1.0 release and external Arch packaging |
| K — Device management | 21 | **done**: single-active device registry and switching, per-device layouts, Chromecast profile, and a recorded real two-device switch |
| L — Pointer input | 22 | **done**: mouse clicks and relative pointer motion from remote buttons on a dedicated uinput device, verified with the RC003 |
| M — Consumer polish | 23–24 | 23 **not started**: live level/gain and a session-scoped PCM queue; 24 **not started**: first-run checklist and recovery docs |
| N — ATVV v1.0 conformance | 25 | **not started**: `MIC_OPEN_ERROR`/`AUDIO_SYNC` handling, dynamic-rate resampler rebuild, sequence-gap diagnostics |

M0 is steps 01–06. Step 07 (hardware bring-up) is complete with a real RC003;
step 08 is complete and verified; step 09 is complete. Step 10 is blocked on
upstream's own implementation; steps 11–18 are complete. Step 17 is complete
and verified with the real RC003; step 18 is complete with recorded app runs
on mpv, Impress, WPS Presentation, and omp. Step 21 is complete with the
recorded RC003 + Chromecast two-device switch. Steps 23–24 are the planned
consumer polish from the `sayall-linux` comparison; step 25 is the ATVV v1.0
conformance fixes; B03 is the hardware-gated keep-open experiment.

## Step files

Completed steps stay in this directory as the record of what was built and
verified; they are not reopened. Live work is listed first.

### Live steps

| # | Phase | Step | Status | Depends on | Exit, in one line |
|---|-------|------|--------|------------|-------------------|
| 10 | D | [vinput-zero-config](10-vinput-zero-config.md) | ⛔ blocked | 08 | a consumer auto-binds the declared keys with no local config |
| 23 | M | [audio-feedback](23-audio-feedback.md) | ⬜ not-started | 04, 06, 14, 19 | status/GUI show the live level and gain, and a new session clears the PCM queue |
| 24 | M | [first-run-and-troubleshooting](24-first-run-and-troubleshooting.md) | ⬜ not-started | 12, 14, 19, 23 | a generic checklist finds the missing layer and the bilingual recovery docs are linked |
| 25 | N | [atvv-spec-conformance](25-atvv-spec-conformance.md) | ⬜ not-started | 01, 02, 04, 06 | errors do not end a stream, a valid sync changes the decode rate, and sequence gaps are counted |

### Completed steps

| # | Phase | Step | Status | Depends on | Exit, in one line |
|---|-------|------|--------|------------|-------------------|
| 01 | A | [protocol-core](01-protocol-core.md) | ✅ done | — | CAPS, control events, and ADPCM pass without I/O |
| 02 | A | [session-state-machine](02-session-state-machine.md) | ✅ done | 01 | a fake transport reaches every state and emits actions without sleeping |
| 03 | A | [cli-diagnostics](03-cli-diagnostics.md) | ✅ done | 01, 02 | `profile`, `caps decode`, and `simulate` print the documented output |
| 04 | B | [pipewire-source](04-pipewire-source.md) | ✅ done | 01 | a resident source records a synthetic tone without a remote |
| 05 | B | [input-uinput](05-input-uinput.md) | ✅ done | 01 | a fake uinput remote exercises grab, gestures, and the trigger |
| 06 | B | [daemon-dbus](06-daemon-dbus.md) | ✅ done | 01, 02, 04, 05 | a mock BLE session drives D-Bus and ignores the stop echo |
| 07 | C | [rc003-hardware-bringup](07-rc003-hardware-bringup.md) | ✅ done | 01, 02, 04, 06 | real RC003 CAPS, audio, keepalive, and reconnect are recorded |
| 08 | D | [vinput-and-mcp](08-vinput-and-mcp.md) | ✅ done | 03, 06, 07 | trigger integration and all MCP tools work end to end |
| 09 | E | [packaging-and-acceptance](09-packaging-and-acceptance.md) | ✅ done | all of 01–08 | a split Arch package, uaccess rule, and signed hardware acceptance |
| 11 | D | [input-evdevil](11-input-evdevil.md) | ✅ done | 05, 08, 10 | the evdevil path and `DICTATE` default are verified |
| 12 | D | [integration-contract](12-integration-contract.md) | ✅ done | 03, 06 (consumer recipe: 10) | contract, status, and the consumer recipe agree and are tested |
| 13 | F | [keymap-config](13-keymap-config.md) | ✅ done | 12, 06 | `config get/set`, capture, and live apply are atomic |
| 14 | F | [config-gui](14-config-gui.md) | ✅ done | 13 | the GTK app edits, saves, and tests mappings on the diagram |
| 15 | F | [mapping-board](15-mapping-board.md) | ✅ done | 14 | the board shows every button/slot and edits the selected one; [interface reference](15-mapping-board.html) |
| 16 | G | [hardware-info](16-hardware-info.md) | ✅ done | 06, 07, 14 | battery, firmware, and DIS facts appear in status, CLI, MCP, and the GUI home view |
| 17 | H | [layout-profiles](17-layout-profiles.md) | ✅ done | 13, 14, 15 | named layouts with a template default whose edits save as a new layout; the active layout is explicit and switches live |
| 18 | H | [layout-presets](18-layout-presets.md) | ✅ done | 17 | app-verified presets create editable user layouts and never become live state |
| 19 | I | [i18n-and-bilingual-docs](19-i18n-and-bilingual-docs.md) | ✅ done | 14 | the GUI is English and Simplified Chinese by locale; user docs have linked Chinese versions |
| 20 | J | [release-and-external-packaging](20-release-and-external-packaging.md) | ✅ done | 19 | v0.1.0 is released; AUR and VCS recipes live outside the repository |
| 21 | K | [multi-device](21-multi-device.md) | ✅ done | 06, 13, 15, 17 | one known-device switch with per-device layouts and a stable source |
| 22 | L | [pointer-actions](22-pointer-actions.md) | ✅ done | 05, 13, 14, 15 | pointer clicks and relative motion on a dedicated uinput device, verified with the RC003 |

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

## M0 exit criteria (met)

M0 is complete. The criteria are kept as the definition of the milestone.

- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D
  warnings`, and `cargo test --workspace` are clean.
- `atvvctl profile show`, `caps decode`, and `simulate` produce the documented
  results.
- A PipeWire source can be created and recorded from without a remote (step 04).
- A fake uinput remote exercises the input path without a physical device
  (step 05).
- A mock BLE session drives the daemon D-Bus API, including a direct
  `AUDIO_START` and an ignored stop echo (step 06).
- No claim of RC003 hardware support is made before step 07.

## Current state

See [`docs/handoff.md`](../handoff.md) for the current commit, test result,
blockers, and the next action. This index does not repeat it.

## Unscheduled ideas

- Structured `app:` action (shell-free desktop launch) once user demand exists;
  see [`sayall-linux-comparison.md`](../research/sayall-linux-comparison.md)
  §3.6.
- Optional tray/status indicator: connection, recording, battery, error.
- PipeWire initial-start retry; suspend/resume and adapter power-cycle tests.
- Real two-device switch validation with the RC003 and Chromecast on one host.
- Layout import/export and a config migration UI.
