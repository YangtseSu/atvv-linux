# Step 10 — zero-config vinput integration

Status: ⛔ blocked
Depends on: 08 (integration); blocked on operator approval and upstream
review
Touches: `docs/`, upstream fcitx5-vinput

## Goal

Expose the remote as a standard interface: a resident PipeWire
`Audio/Source` plus a standard voice key. After `atvvctl pair` and a one-time
vinput configuration, fcitx5-vinput records from `atvv-miremote-rc003` and
commits text. `atvvd` never writes vinput's configuration and never calls
vinput in the primary path. Zero configuration is still the long-term target,
but it requires a consumer-side auto-selection feature in vinput, not a device
setter called by ATVV (ADR 0007).

## Current state

- ATVV is installed as `atvv-linux 0.1.0-5`; the enabled user unit runs
  `/usr/bin/atvvd` with `trigger_key = "DICTATE"`. The live source advertises
  `atvv.contract_version=1`, `atvv.profile`, and
  `atvv.recording_keys=DICTATE,VOICECOMMAND,ASSISTANT`; step 12 exposes the
  same contract through `atvvctl contract`, `atvvctl status.contract`, and the
  `atvv-mcp` `contract` tool.
- The manual consumer path is verified: `vinput device use
  atvv-miremote-rc003` selects the source, the daemon re-reads
  `global.capture_device` at every recording, and a physical voice press
  records from `atvv-miremote-rc003` at 16 kHz. The one-time setup is
  `vinput device use atvv-miremote-rc003` plus `Dictate` at the next free
  `[TriggerKey]` index (default install: `1=Dictate`).
- The fcitx addon owns `[TriggerKey]`; `org.fcitx.Vinput` cannot change it.
  fcitx's `org.fcitx.Fcitx.Controller1` `GetConfig`/`SetConfig` is the correct
  consumer-side path and is now implemented in the local vinput patches.
- Local consumer patches exist in the `YangtseSu/fcitx5-vinput` fork on three
  stacked branches, all based on upstream `bd7dd43`:
  - `feat/issue-491-auto-capture-device` (`42ccc26`): `auto` and comma-separated
    wildcard priority lists; exact names win; unmatched patterns use the
    system default; CLI/GUI Auto.
  - `feat/issue-492-recording-keys-metadata` (`3e3ebdf`): read `atvv.recording_keys`
    from node info (not the registry global) and map canonical names to
    fcitx names.
  - `feat/issue-492-bind-recording-keys` (`c8e51b2`): on `device use`, append missing
    keys through fcitx's controller, preserving entries and using the next
    free indices; no direct file write and no D-Bus activation; fallback
    prints the exact entries.
  Upstream issues
  [#491](https://github.com/xifan2333/fcitx5-vinput/issues/491) and
  [#492](https://github.com/xifan2333/fcitx5-vinput/issues/492) are open. The
  branches are local-only: nothing is pushed and no PR exists.
- The opt-in `vinput = "dbus"` fallback still has the addon
  `mostRecentInputContext` focus gap. That fallback is not part of the primary
  path and is deferred; see Deferred below.

## Deliverables

- ✅ Recording-key support: `DICTATE` is the default active key;
  `VOICECOMMAND` and `ASSISTANT` are the supported fallbacks, all resolved by
  `atvv-input`. Never assume a user shortcut.
- ✅ Keep the user's existing `[TriggerKey]` entries: `Dictate` goes at the
  next free index and never replaces the user's entry. Documented in
  [`../integrations/provider-contract.md`](../integrations/provider-contract.md)
  and [`../integrations/fcitx5-vinput.md`](../integrations/fcitx5-vinput.md).
- ✅ Document the one-time consumer setup: select the source with
  `vinput device use atvv-miremote-rc003` and append `Dictate` at the next
  free index without replacing the user's trigger.
- ✅ No ATVV-side device push. The `SetCaptureDevice` call and
  `vinput_device = "auto" | "none"` policy are absent; ADR 0007 records the
  decision. Source selection belongs to the consumer.
- ⬜ Upstream issue/PR A: consumer-side capture-device auto-selection and
  priority list. Issue
  [#491](https://github.com/xifan2333/fcitx5-vinput/issues/491) is open; code
  is ready on `feat/issue-491-auto-capture-device`; not pushed and no PR
  exists.
- ⬜ Upstream issue/PR B: read `atvv.recording_keys` and append missing trigger
  keys on `device use`. Issue
  [#492](https://github.com/xifan2333/fcitx5-vinput/issues/492) is open; code
  is ready on the stacked `feat/issue-492-recording-keys-metadata` and
  `feat/issue-492-bind-recording-keys` branches; not pushed and no PR exists.
- ✅ Keep `vinput = "trigger"` as the default and `vinput = "dbus"` as an
  explicit fallback. Do not make D-Bus the zero-config default.
- ✅ Generic provider diagnostics: `atvvctl status.contract`,
  `atvvctl contract`, `atvvctl doctor` (declared keys, active key, live
  `atvv.*` properties), and the `atvv-mcp` `contract` tool. Step 12 owns the
  cross-consumer contract output.
- ✅ Tests: source contract metadata, declared-key mapping, fcitx trigger
  update, CLI/MCP contract output, D-Bus fallback start/stop, and the
  no-consumer-config-write check. The CLI integration test leaves an empty
  temporary `XDG_CONFIG_HOME`; no ATVV code path writes `vinput.conf` or
  `config.json`.
- ✅ Manual real check: `device use` plus `Dictate` records from
  `atvv-miremote-rc003`; the end-to-end fcitx text commit was
  operator-confirmed with the prior trigger. The local auto-bind test added
  `VoiceCommand` at the next free index and was idempotent on a second run.
- ⬜ Real auto-bind check after upstream merge/release: no manual device step
  and no manual trigger entry.
- ✅ Record the default-mode decision and the layering rationale (ADR 0007).

## Deferred

- The generic `GetCaptureDevice`/`SetCaptureDevice` control API is not
  pursued. ATVV does not call a setter (ADR 0007).
- The addon `mostRecentInputContext` focus fallback is not implemented. It
  only affects the opt-in `vinput = "dbus"` mode; keep it as an optional
  upstream follow-up.
- Including `Dictate` in vinput's upstream default trigger list is not
  pursued; the documented one-entry addition remains.

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
cargo test --workspace
cargo test -p atvv-cli --test contract
cargo test -p atvv-mcp --test stdio
# vinput fork, local branches (not pushed):
cmake --build build
ctest --test-dir build --output-on-failure --timeout 60
python3 scripts/check-i18n.py
clang-format --dry-run --Werror <changed files>
# manual consumer path:
vinput device list
vinput device use atvv-miremote-rc003
# hold the voice key and record
```

Recorded 2026-10-31:

- ATVV `cargo test --workspace` → 126 passed, 2 ignored;
  `atvv-cli --test contract` and `atvv-mcp --test stdio` each → 1 passed.
- vinput fork `cmake --build` and `ctest` → 40/40 passed;
  `scripts/check-i18n.py` → 0 missing/obsolete/untranslated; clang-format
  clean.
- Local vinput check: `vinput device use auto` resolved to
  `atvv-miremote-rc003`; live fcitx5 5.1.23 added `VoiceCommand` at
  `3=VoiceCommand`, a second run was idempotent, and a private bus without
  fcitx5 printed the exact entries without writing the config.
- The local vinput build used `WERROR=OFF` because the host's fcitx5 5.1.23
  deprecates `StandardPath` in pre-existing code; CI runs its own Werror
  configuration.

## Exit criteria

- ✅ Manual one-time setup (`device use` plus `Dictate` at the next free
  index): fcitx5-vinput records from the ATVV source and `atvvd` does not
  write vinput's configuration. The Dictate text-commit leg is
  recording-verified; the full text commit was operator-confirmed with the
  prior trigger and remains an operator spot check.
- ⬜ Full Dictate text commit (operator spot check).
- ⬜ If the consumer-side auto-selection/auto-bind feature lands, the same run
  works without manual entries; this is the only path to true zero-config.
- ✅ A user with a customized trigger key keeps it and can use `Dictate`
  (or `VOICECOMMAND`/`ASSISTANT`) as an additional ATVV trigger.
- ✅ A user's explicit non-ATVV device is preserved; ATVV never selects it or
  overrides it.
- ✅ The primary path has no `atvvd` -> `org.fcitx.Vinput` call; the D-Bus mode
  remains an explicit fallback.
- ✅ Real-device run recorded with date, address, vinput version, and the
  relevant upstream feature state (`C0:5D:39:F8:2D:4C`, vinput 2.4.2,
  branches local-only).
- ✅ The default-mode decision and the layering rationale are recorded
  (ADR 0007).

## Risks

- Upstream may reject a consumer-side auto-selection feature. Then the one-time
  `vinput device use` plus `Dictate` at the next free index (default:
  `1=Dictate`) remains the documented setup; do not claim zero-config.
- If auto-selection lands, it must not fight an explicit user choice or the
  system default.
- Some applications may bind the declared consumer keys; the active key is
  configurable. `DICTATE` is semantically owned by dictation software, so a
  future app may claim it.
- The source is created before the daemon loop; `PipeWireSink::start` failure
  currently has no in-process retry. If PipeWire is late, the source is absent
  and no consumer can select it. That is a separate reliability gap, not an
  ATVV-to-vinput control problem.
- Step completion is gated by operator approval (push, issues, draft PRs),
  upstream review/merge, and the post-merge auto-bind run.

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
- 2026-10-31 — local consumer patches implemented in the
  `YangtseSu/fcitx5-vinput` fork on three stacked branches:
  `feat/auto-capture-device` (`auto` / comma-separated wildcard priority
  lists, exact-name precedence, GUI/CLI Auto), `feat/recording-keys-metadata`
  (read `atvv.recording_keys` from node info; canonical-to-fcitx mapping), and
  `feat/bind-recording-keys` (`device use` appends missing keys through
  `org.fcitx.Fcitx.Controller1` with no direct file write and no D-Bus
  activation). Verification: `cmake --build`, 40/40 `ctest`,
  `check-i18n.py` clean, clang-format clean; live fcitx5 5.1.23 added
  `VoiceCommand` at `3=VoiceCommand` on `device use auto`, the second run was
  idempotent, and the no-fcitx fallback printed the exact entries without
  writing. The branches are not pushed and no upstream issue/PR exists; push
  and PR creation need the operator's explicit approval.

- 2026-10-31 — plan corrected after review. The 2026-10-30 "PR A / PR B"
  entry above is historical and superseded by ADR 0007: the actual work is
  the three stacked branches listed under Current state. Completed ATVV-side
  deliverables and tests are checked; the remaining items are the two
  upstream issues/PRs and the post-merge auto-bind check. Status is blocked
  on operator approval (push, issues, draft PRs) and upstream review/merge.
  The D-Bus focus fallback and the generic capture-device setter are explicitly
  deferred rather than left as vague risks.

- 2026-10-31 — upstream issues
  [#491](https://github.com/xifan2333/fcitx5-vinput/issues/491) (auto
  selection) and
  [#492](https://github.com/xifan2333/fcitx5-vinput/issues/492) (recording-key
  binding) created. Branches renamed to the issue-based names and commit
  messages updated with the issue numbers. Nothing is pushed and no PR exists;
  the remaining gate is operator approval for push and draft PRs.
