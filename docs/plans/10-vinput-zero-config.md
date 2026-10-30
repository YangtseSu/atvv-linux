# Step 10 — zero-config vinput integration

Status: 🚧 in-progress
Depends on: 08 (integration); can start before 09 acceptance
Touches: `crates/atvv-daemon/src/config.rs`,
`crates/atvv-input/src/keymap.rs`, `crates/atvv-daemon/src/status.rs`,
`crates/atvv-cli/src/main.rs`, `docs/`, upstream fcitx5-vinput

## Goal

Expose the remote as a standard interface: a resident PipeWire
`Audio/Source` plus a standard voice key. After `atvvctl pair` and a one-time
vinput configuration, fcitx5-vinput records from `atvv-miremote-rc003` and
commits text. `atvvd` never writes vinput's configuration and never calls
vinput in the primary path. Zero configuration is still the long-term target,
but it requires a consumer-side auto-selection feature in vinput, not a device
setter called by ATVV (ADR 0007).

## Current state

- vinput discovers the resident `atvv-miremote-rc003` source. Selecting it is
  the consumer's job: run `vinput device use atvv-miremote-rc003` once. The
  daemon re-reads `global.capture_device` at every `StartRecordingInternal`
  and calls `capture_->SetTargetObject`, so the next recording already uses
  the new device; the CLI's "Restart daemon to apply changes" sentence is
  stale. The selected device is `global.capture_device` in
  `~/.config/vinput/config.json`. `org.fcitx.Vinput.Service` has no
  capture-device setter or getter; ATVV does not need one.
- The fcitx addon trigger key is `[TriggerKey]` in
  `~/.config/fcitx5/conf/vinput.conf`. It is a multi-entry list owned by the
  addon; the daemon and `org.fcitx.Vinput` cannot change it. fcitx's
  `org.fcitx.Fcitx.Controller1` `GetConfig`/`SetConfig` can apply an addon
  config change live without a direct file write; that is an optional
  consumer-side management path, not part of the primary flow.
- A bare synthetic `Alt_R` did not trigger fcitx5-vinput 2.4.2 on 2026-10-30.
  A dedicated consumer key did, with the user's existing `0=` entry kept.
  The `Control+Control_R` chord also worked.
- Step 11 changed the daemon default to `DEFAULT_TRIGGER_KEY = "DICTATE"`;
  vinput keeps the user's entries and adds `Dictate` at the next free index.
  On a default vinput install (`[TriggerKey] 0=Alt_R`) that is `1=Dictate`.
  `VOICECOMMAND` and `ASSISTANT` are the supported fallbacks. The live
  installed `0.1.0-3` unit still reads `trigger_key = "ASSISTANT"` until the
  new build is installed.
- External `StartRecording` (D-Bus fallback only) reaches the daemon
  correctly, but the addon resolves its target input context from
  `last_active_ic_`, which is not updated on focus-in. A mouse-only focus
  change before the next key press can leave it stale.
  `fcitx::Instance::mostRecentInputContext()` is the correct fallback.
- `StopRecording("")` is a valid contract for the D-Bus fallback:
  `scene::Resolve` falls back to the configured active scene.
  `StartRecording` does not return a scene id, and external callers do not
  need one for dictation.

## Deliverables

- ✅ Recording-key support: `DICTATE` is the default active key;
  `VOICECOMMAND` and `ASSISTANT` are the supported fallbacks, all resolved by
  `atvv-input`. Never assume a user shortcut.
- ⬜ Keep the user's existing `[TriggerKey]` entries: document `Dictate` at
  the next free index; never replace the user's entry. On a default install
  (`0=Alt_R`) that is `1=Dictate`.
- ⬜ Document the one-time consumer setup: select the source with
  `vinput device use atvv-miremote-rc003` and append `Dictate` at the next
  free index without replacing the user's trigger.
- ⬜ Do not add an ATVV-side device push. Drop the `SetCaptureDevice` call from
  `atvvd` and the `vinput_device = "auto" | "none"` policy; source selection
  belongs to the consumer (ADR 0007).
- ⬜ Upstream vinput issue/PR: consumer-side capture-device auto-selection.
  - Add a generic priority/pattern list (for example `atvv-*`, then
    `default`) or an equivalent; vinput picks the first available source.
  - ATVV only exposes standard node metadata (`node.name=atvv-<profile>`,
    description, rate). It does not call a setter.
  - On `device use`, read the source's `atvv.recording_keys` (canonical
    evdev names), map them to fcitx names, and append them to `[TriggerKey]`
    while preserving existing entries. If auto-apply is rejected, print the
    exact entries to add.
  - Optional: a generic `GetCaptureDevice`/`SetCaptureDevice` control API only
    if upstream wants it for other consumers; it is not required by step 10.
  - Optional: include `Dictate` in vinput's default trigger list or document
    the one-entry addition. Do not change the upstream default without
    maintainer agreement.
- ⬜ Keep `vinput = "trigger"` as the default and `vinput = "dbus"` as an
  explicit fallback for environments where the virtual keyboard cannot reach
  the IME. Do not make D-Bus the zero-config default.
- ⬜ Add generic provider diagnostics: extend `atvvctl status`/`doctor` with
  the declared `atvv.recording_keys`, the active key, and the source/consumer
  state. Step 12 owns the cross-consumer contract output.
- ⬜ Tests: ATVV never writes vinput's `config.json`/`vinput.conf`; the
  provider diagnostics report the declared keys; the D-Bus fallback still
  starts/stops; the source name and `DICTATE` mapping are stable.
- ⬜ Real vinput check: one-time `device use` plus `Dictate` at the next free
  index (default install: `1=Dictate`), hold the voice key, record, commit
  text. If the upstream auto-selection lands, repeat without the manual
  device step.
- ⬜ Record the default-mode decision and the layering rationale (ADR 0007).

## Design notes

- The dedicated-key rule: add the declared keys to vinput's trigger list
  without replacing the user's key. The default active key is the dictation
  consumer key (`Dictate`); `VOICECOMMAND` and `ASSISTANT` are the supported
  fallbacks.
- Do not assume `Alt_R` or any user default. Changing shortcuts is normal; a
  bare synthetic modifier is not a reliable trigger for vinput 2.4.2.
- The three consumer keys reach the tested GNOME 49/fcitx5 5.1.23 input
  context. Bind the declared set in the consumer and emit one active key; do
  not emit all three simultaneously (chord parsing and duplicate actions).
- The consumer owns its trigger list. The user (or vinput itself) appends
  `Dictate` at the next free index; ATVV never writes or replaces the user's
  `[TriggerKey]`.
- fcitx rewrites `~/.config/fcitx5/conf/vinput.conf` on addon exit. Edit it
  while fcitx is stopped; otherwise the edit is lost.
- fcitx5-vinput is GPL-3.0. Use its D-Bus API; do not link, vendor, or copy
  its code or config handling.
- The daemon must not write `~/.config/vinput/config.json` or
  `~/.config/fcitx5/conf/vinput.conf` directly. Those are another
  application's user configuration and would override a deliberate choice.
- The ATVV source must not become the system default; other applications keep
  their microphone.
- Layering (ADR 0007): ATVV provides a standard PipeWire source, a uinput key,
  and read-only status. vinput owns source selection and trigger binding.
  ATVV does not call `org.fcitx.Vinput` in the primary path.
- The D-Bus `vinput = "dbus"` mode is an explicit fallback, not the default.
  It remains useful when the compositor/IME cannot see the virtual keyboard.
- Device-selection policy belongs to the consumer. True zero-config needs a
  vinput-side auto-selection feature; until then the one-time
  `vinput device use` plus `Dictate` at the next free index (default:
  `1=Dictate`) is the correct layered setup.
- Broader consumers (browser AI chat, coding agents, IM applications) are
  step 12; this step covers the vinput recipe and the declared-key contract
  they share.

## Verification

```text
cargo test -p atvv-input
cargo test -p atvv-daemon
cargo test --workspace
dbus-run-session -- <mock-vinput test>
# manual one-time consumer setup:
vinput device list
vinput device use atvv-miremote-rc003
# add Dictate at the next free index while fcitx5 is stopped
# (default install: 0=Alt_R, so 1=Dictate)
# real device, done 2026-10-30:
# daemon trigger_key=DICTATE; vinput [TriggerKey] Dictate; voice press records
vinput daemon status                   # recording on voice press
journalctl --user -u vinput-daemon     # target.object=atvv-miremote-rc003
# if a consumer-side auto-selection feature lands:
# capture_device = auto/priority; repeat without the manual device step
```

- `DICTATE` real-device path verified 2026-10-30 after step 11: fcitx
  received `Key(Dictate)` and vinput recorded from the ATVV source. The
  `Control+Control_R` chord was an earlier verified alternative.
- The manual one-time setup is the expected path until vinput selects the
  source itself; ATVV must not perform that selection.

## Exit criteria

- ⬜ Fresh install + pair + one-time vinput configuration (`device use` and
  `Dictate` at the next free index; default install: `1=Dictate`):
  fcitx5-vinput records from the ATVV source and commits text. `atvvd` does
  not write vinput's configuration.
- ⬜ If a consumer-side auto-selection/auto-bind feature lands, the same run
  works without manual entries; this is the only path to true zero-config.
- ⬜ A user with a customized trigger key keeps it and can use `Dictate`
  (or `VOICECOMMAND`/`ASSISTANT`) as an additional ATVV trigger.
- ⬜ A user's explicit non-ATVV device is preserved; ATVV never selects it or
  overrides it.
- ⬜ The primary path has no `atvvd` -> `org.fcitx.Vinput` call; the D-Bus mode
  remains an explicit fallback.
- ⬜ Real-device run recorded with date, address, vinput version, and the
  relevant upstream commit/feature state.
- ⬜ The default-mode decision and the layering rationale are recorded
  (ADR 0007).

## Risks

- Upstream may reject a consumer-side auto-selection feature. Then the one-time
  `vinput device use` plus `Dictate` at the next free index (default:
  `1=Dictate`) remains the documented setup; do not claim zero-config.
- If auto-selection lands, it must not fight an explicit user choice or the
  system default.
- The D-Bus fallback still needs an end-to-end real-device text-commit run. Its
  focus caveat remains, and it is opt-in only.
- The source is created before the daemon loop; `PipeWireSink::start` failure
  currently has no in-process retry. If PipeWire is late, the source is absent
  and no consumer can select it. That is a separate reliability gap, not an
  ATVV-to-vinput control problem.
- Some applications may bind the declared consumer keys; the active key is
  configurable. `DICTATE` is semantically owned by dictation software, so a
  future app may claim it.

## Progress log

- 2026-10-30 — step planned after the zero-config review; no implementation.
- 2026-10-30 — revised after review: the daemon must not assume `Alt_R` or any
  user trigger key.
- 2026-10-30 — step 11 changed the code default to `DICTATE`; vinput keeps
  the user's entries and adds `Dictate`. The same real press drove fcitx
  `Key(Dictate)` and vinput recording at 16 kHz.
- 2026-10-30 — upstream source review (`xifan2333/fcitx5-vinput` `bd7dd4365`,
  installed 2.4.2). The upstream scope is PR A (capture-device
  `GetCaptureDevice`/`SetCaptureDevice` plus CLI live apply) and PR B (addon
  `mostRecentInputContext` focus fallback). `device use` already applies on
  the next recording, so the restart instruction is stale. Trigger keys are
  addon-owned and cannot be changed by `org.fcitx.Vinput`; the trigger-key
  option is a CLI over fcitx's `Controller1` `GetConfig`/`SetConfig`, not a
  daemon method, and is deferred until D-Bus mode is re-evaluated. Empty
  `StopRecording` scene ids already resolve to the active scene. Evidence:
  [`../research/vinput-upstream-api.md`](../research/vinput-upstream-api.md).
- 2026-10-31 — direction corrected. ATVV is the lower layer; consumers own
  source and trigger configuration. The `SetCaptureDevice` push, the
  `vinput_device` policy, and the "D-Bus is zero-config" plan are dropped.
  The primary interface is the PipeWire source plus the `Dictate` uinput key;
  the manual one-time vinput setup is correct layering, not a workaround.
  True zero-config depends on a consumer-side auto-selection feature. See
  [ADR 0007](../decisions/0007-integration-direction.md).
- 2026-10-31 — consumer-integration goal recorded in `AGENTS.md` and
  [`../integrations/provider-contract.md`](../integrations/provider-contract.md).
  The source advertises `atvv.contract_version=1`, `atvv.profile`, and
  `atvv.recording_keys=DICTATE,VOICECOMMAND,ASSISTANT`; consumers auto-bind
  the declared set and the provider emits one active key. Examples use the
  default vinput install index (`0=Alt_R`, so `1=Dictate`). Generic contract
  output is step 12.
- 2026-10-31 — step 12 completed first: the provider contract is now exposed
  by `atvvctl contract [--json]`, the `contract` object in `atvvctl status`,
  and the `atvv-mcp` `contract` tool. The ATVV-side deliverables of this step
  (declared keys, docs, no config writes, vinput trigger default) are already
  in place. Remaining work is upstream-only: the consumer-side
  auto-selection/auto-bind issue/PR and the real vinput auto-bind check. The
  fork at `YangtseSu/fcitx5-vinput` exists; no upstream issue or PR has been
  submitted. The installed `0.1.0-4` package predates the provider contract
  output.
