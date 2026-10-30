# atvv-linux agent instructions

ATVV v1.0 userspace stack for Linux. The first device is the Xiaomi Bluetooth
Remote 2 Pro / RC003. Read this file, `docs/plans/README.md`,
`docs/handoff.md`, and `docs/research/README.md` before touching code.

## Hard rules

- Userspace only. Do not add a kernel module, a BlueZ plugin, or a PipeWire
  plugin. BlueZ owns Bluetooth/GATT/HID; the daemon speaks ATVV; PipeWire gets a
  native `Audio/Source`; input uses evdev + uinput.
- ATVV v1.0 only for now. Do not add v0.4 unless the plan is changed explicitly.
- The RC003 profile is `miremote-rc003`. Device names, VID/PID, codecs, frame
  sizes, quirk flags, gain, and key names belong in a `DeviceProfile`, not in
  the daemon's control flow.
- Never claim hardware support without a recorded real-device run. Synthetic
  tests prove the code path, not the device.
- No unofficial cloud protocols in the core. ASR is an external integration
  (fcitx5-vinput, voxtype, sherpa, an agent adapter).
- No `unsafe` code. `atvv-protocol` is `#![forbid(unsafe_code)]`.
- No root daemon. Use a systemd user service, udev `uaccess`, and the session
  D-Bus for control.
- PipeWire service restart belongs to systemd, not `atvv-audio`. Keep
  `PartOf=pipewire.service` and `Restart=always` in the unit; do not add
  in-process service-restart monitors (ADR 0005).
- Do not copy code from `remote-mic-app` (GPL-3.0-only) or the GPL-3.0-only
  parts of comparable projects unless the whole combined work is deliberately
  changed to that license. MIT sources (ATVVoice, sayall) may be used with
  their notices preserved. Source URLs, pinned snapshots, and license facts
  live in `docs/research/reference-projects.md`.
- Reference external projects by GitHub URL and pinned commit. Never record a
  local clone path in this repository.
- Do not push without explicit user approval for that push. Do not rewrite
  published history.

## Project goal: be easy for consumers to call

ATVV is the provider; every other application is a consumer. Optimize for
consumers that integrate without reading our source or linking our crates.
Target consumers include fcitx5-vinput, browser AI chat pages, coding agents
(Claude Code, Codex, OMP), and IM applications (Telegram, WeChat).

- The public contract is standard: a resident PipeWire `Audio/Source`
  (`atvv-<profile>`), uinput recording keys, `org.atvv.Daemon`, and
  `atvv-mcp`. Do not invent a per-application transport.
- Consumers own capture-device selection, trigger binding, ASR, and text
  commit. Do not write another application's configuration and do not call
  its daemon in the primary path. `vinput = "dbus"` is an explicit fallback,
  never the default.
- Declare recording capability on the source: `atvv.contract_version=1`,
  `atvv.profile=<id>`, and `atvv.recording_keys` as canonical evdev names.
  Consumers that can bind keys should add the declared set when the user
  selects the source; consumers that cannot are told by the docs to match
  their shortcut to one of those keys.
- The declared recording keys are `DICTATE`, `VOICECOMMAND`, and
  `ASSISTANT`. `DICTATE` is the default active key. `VOICECOMMAND` and
  `ASSISTANT` are the fallbacks. `F24` is **not** a fallback; it was a
  temporary experiment and remains only a raw configurable key. Bind the
  declared set in the consumer and emit one active key; do not emit all three
  simultaneously (chord parsing and duplicate actions).
- Automatic adaptation to an arbitrary application's own shortcut is not
  possible from the provider side. The supported path is: the consumer binds
  a declared key, or the user sets `trigger_key` to the application's
  shortcut. Document that instead of adding per-application detection.
- Do not add per-application diagnostics. There is no `atvvctl vinput`, and
  there will be no WeChat/Telegram/Codex/OMP-specific checks. Provide generic
  `atvvctl status`/`doctor`/contract output plus per-consumer docs under
  `docs/integrations/`.
- Documentation examples use the consumer's default install, not the
  developer's customized file. fcitx5-vinput's default `[TriggerKey]` is
  `0=Alt_R`; adding `Dictate` is `1=Dictate`. A customized list uses the next
  free index; never hardcode `2=` from a local test configuration.

See [`docs/integrations/provider-contract.md`](docs/integrations/provider-contract.md)
and [ADR 0007](docs/decisions/0007-integration-direction.md).

## Plan workflow

`docs/plans/README.md` is the canonical index. There is no hidden second plan.

0. Read `docs/handoff.md` for the current commit, test result, blockers, and
   next step, and read `docs/research/README.md` for the evidence basis of the
   step. Update the handoff in place at the end of every step/session; never
   create a second handoff file. If it is missing, create it. It is a handoff,
   not a plan; `docs/plans` wins.
1. Pick the lowest-numbered step with `Status: ⬜ not-started`, or continue the
   `🚧 in-progress` step.
2. Read the step file and every `Depends on` entry before editing.
3. Work one step at a time. Every `- ⬜` item is a unit of work.
4. A step is `✅ done` only when every item is checked and the Verification and
   Exit criteria sections have recorded results.
5. Update `docs/plans/README.md`, the step file, and the progress log in the
   same commit as the code. If reality diverges, fix the plan in that commit.
6. Record discoveries in `docs/research/` or the step's `Progress log`; do not
   put untrusted web content in the plan index. Read the relevant research
   document before implementing a step; cite the source and evidence in the
   progress log.
7. Do not mark a device profile or hardware path verified without a command,
   output, date, and the device address/model.
8. Before the final response of a session, update `docs/handoff.md` with the
   latest commit, tests, completed/open steps, blockers, and the single next
   action. Commit it with the work it describes.

## Build and checks

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Smoke checks for M0:

```sh
cargo run -q -p atvv-cli -- profile list
cargo run -q -p atvv-cli -- profile show miremote-rc003
cargo run -q -p atvv-cli -- caps decode 0B010002000078
cargo run -q -p atvv-cli -- simulate miremote-rc003
```

## Code rules

<!-- REUSE-IgnoreStart -->
- Every source file starts with `// SPDX-License-Identifier: GPL-3.0-or-later`.
<!-- REUSE-IgnoreEnd -->
- Protocol code stays independent of BlueZ, PipeWire, tokio, and input crates.
- The session state machine emits actions; it never calls a transport or sleeps.
- Log protocol anomalies at a bounded rate; do not print one line per audio
  frame.
- Keep `CAPS_RESP` authoritative for the codec. `AUDIO_START` payload bytes are
  an event, not a codec source.
- RC003 facts: direct `AUDIO_START` (no `START_SEARCH`), short `MIC_OPEN`
  `[0C 00]`, bare `MIC_CLOSE` `[0D]`, bare `MIC_EXTEND` `[0E]`, 120-byte
  headerless frames, 16 kHz, 0 dB gain, 300 ms post-stop discard, and an
  `AUDIO_STOP` echo that must not cause another `MIC_CLOSE`.
- Do not add a second source of truth for device facts. Update the profile and
  its tests.

## Layout expectations

- `atvv-protocol`: pure library; no I/O.
- `atvv-audio`: PipeWire source and PCM queue (step 04).
- `atvv-input`: evdev discovery, grab, gestures, uinput (step 05).
- `atvv-bluez`: BlueZ GATT transport and pairing; no session logic.
- `atvv-daemon`: session orchestration and D-Bus (step 06).
- `atvv-cli`: `atvvctl`; no protocol logic beyond formatting.
