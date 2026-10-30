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
  A dedicated regular key `F24` did, with the user's existing `0=` entry kept.
  The `Control+Control_R` chord also worked.
- Step 11 changed the daemon default to `DEFAULT_TRIGGER_KEY = "DICTATE"`;
  vinput keeps the user's entries and adds `2=Dictate`. `F24` remains an
  explicit alternative. The live installed `0.1.0-3` unit still reads
  `trigger_key = "ASSISTANT"` from the user's config until the new build is
  installed.
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

- ✅ Dedicated trigger key: `F24` was implemented first, resolved to
  `KEY_F24` by `atvv-input`, with parser/daemon tests and docs. Step 11
  superseded the default with `DICTATE`; `F24` remains configurable. Never
  assume a user shortcut.
- ⬜ Keep the user's existing `[TriggerKey]` entries: document
  `2=Dictate` alongside `0=<user key>`; never replace the user's entry.
- ⬜ Document the one-time consumer setup: select the source with
  `vinput device use atvv-miremote-rc003` and append `2=Dictate` without
  replacing the user's trigger. This is vinput's own configuration; `atvvd`
  must not write it.
- ⬜ Do not add an ATVV-side device push. Drop the `SetCaptureDevice` call from
  `atvvd` and the `vinput_device = "auto" | "none"` policy; source selection
  belongs to the consumer (ADR 0007).
- ⬜ Upstream vinput issue/PR: consumer-side capture-device auto-selection.
  - Add a generic priority/pattern list (for example `atvv-*`, then
    `default`) or an equivalent; vinput picks the first available source.
  - ATVV only exposes standard node metadata (`node.name=atvv-<profile>`,
    description, rate). It does not call a setter.
  - Optional: a generic `GetCaptureDevice`/`SetCaptureDevice` control API only
    if upstream wants it for other consumers; it is not required by step 10.
  - Optional: include `Dictate` in vinput's default trigger list or document
    the one-entry addition. Do not change the upstream default without
    maintainer agreement.
- ⬜ Keep `vinput = "trigger"` as the default and `vinput = "dbus"` as an
  explicit fallback for environments where the virtual keyboard cannot reach
  the IME. Do not make D-Bus the zero-config default.
- ⬜ Add a read-only `atvvctl vinput` diagnostic: report the ATVV mode/key,
  whether `org.fcitx.Vinput` is running, the source vinput selected (read-only
  config fallback until an upstream API exists), the configured trigger
  entries, and the exact one-line fix. It must not write vinput files.
- ⬜ Tests: ATVV never writes vinput's `config.json`/`vinput.conf`; the
  diagnostic reads and suggests; the D-Bus fallback still starts/stops; the
  source name and `DICTATE` mapping are stable.
- ⬜ Real vinput check: one-time `device use` plus `2=Dictate`, hold the voice
  key, record, commit text. If the upstream auto-selection lands, repeat
  without the manual device step.
- ⬜ Record the default-mode decision and the layering rationale (ADR 0007).

## Design notes

- The dedicated-key rule: add a key to vinput's trigger list without
  replacing the user's key. The current default is the dictation consumer key
  (`Dictate`); `F24` is a regular-key fallback if a downstream IME or
  compositor cannot bind a consumer key.
- Do not assume `Alt_R` or any user default. Changing shortcuts is normal; a
  bare synthetic modifier is not a reliable trigger for vinput 2.4.2.
- `F24` is a normal key, so fcitx's input-context path is preserved; it is not
  a bare modifier. `Dictate` also reaches the input context on the tested
  GNOME 49/fcitx5 5.1.23 path.
- The consumer owns its trigger list. The user (or vinput itself) appends
  `2=Dictate`; ATVV never writes or replaces the user's `[TriggerKey]`.
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
  `vinput device use` plus `2=Dictate` is the correct layered setup.

## Verification

```text
cargo test -p atvv-input
cargo test -p atvv-daemon
cargo test --workspace
dbus-run-session -- <mock-vinput test>
# manual one-time consumer setup:
vinput device list
vinput device use atvv-miremote-rc003
# add 2=Dictate to [TriggerKey] while fcitx5 is stopped
# real device, done 2026-10-30:
# daemon trigger_key=DICTATE; vinput [TriggerKey] 2=Dictate; voice press records
vinput daemon status                   # recording on voice press
journalctl --user -u vinput-daemon     # target.object=atvv-miremote-rc003
# if a consumer-side auto-selection feature lands:
# capture_device = auto/priority; repeat without the manual device step
```

- `DICTATE` real-device path verified 2026-10-30 after step 11: fcitx
  received `Key(Dictate)` and vinput recorded from the ATVV source. The `F24`
  and `Control+Control_R` paths remain alternatives.
- The manual one-time setup is the expected path until vinput selects the
  source itself; ATVV must not perform that selection.

## Exit criteria

- ⬜ Fresh install + pair + one-time vinput configuration (`device use` and
  `2=Dictate`): fcitx5-vinput records from the ATVV source and commits text.
  `atvvd` does not write vinput's configuration.
- ⬜ If a consumer-side auto-selection feature lands, the same run works
  without the manual device step; this is the only path to true zero-config.
- ⬜ A user with a customized trigger key keeps it and can use `Dictate`
  (or `F24`) as an additional ATVV trigger.
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
  `vinput device use` plus `2=Dictate` remains the documented setup; do not
  claim zero-config.
- If auto-selection lands, it must not fight an explicit user choice or the
  system default.
- The D-Bus fallback still needs an end-to-end real-device text-commit run. Its
  focus caveat remains, and it is opt-in only.
- The source is created before the daemon loop; `PipeWireSink::start` failure
  currently has no in-process retry. If PipeWire is late, the source is absent
  and no consumer can select it. That is a separate reliability gap, not an
  ATVV-to-vinput control problem.
- Some keyboards/apps may bind `F24`; the key is configurable. `DICTATE` is
  semantically owned by dictation software, so a future app may claim it.

## Progress log

- 2026-10-30 — step planned after the zero-config review; no implementation.
- 2026-10-30 — revised after review: the daemon must not assume `Alt_R` or any
  user trigger key.
- 2026-10-30 — `F24` dedicated-key path verified on the real RC003: daemon
  `trigger_key = "F24"`, vinput `[TriggerKey] 0=Alt_R, 1=F24`. The default
  trigger was `F24` at that point; the user's existing entry was preserved.
- 2026-10-30 — step 11 changed the code default to `DICTATE`; vinput keeps
  `0=Alt_R`, `1=Assistant` and adds `2=Dictate`. The same real press drove
  fcitx `Key(Dictate)` and vinput recording at 16 kHz.
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
