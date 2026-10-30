# Step 10 — zero-config vinput integration

Status: 🚧 in-progress
Depends on: 08 (integration); can start before 09 acceptance
Touches: `crates/atvv-daemon/src/config.rs`, `crates/atvv-input/src/keymap.rs`,
`docs/`, upstream fcitx5-vinput

## Goal

After `atvvctl pair`, a user starts `atvvd` and fcitx5-vinput records from
`atvv-miremote-rc003` without replacing the user's trigger key or capture
device. The long-term target is zero configuration; until the upstream API
exists, adding one dedicated trigger entry is the only manual step.

## Current state

- vinput discovers the resident `atvv-miremote-rc003` source, but the user must
  run `vinput device use atvv-miremote-rc003` and restart `vinput-daemon`.
- The selected device is `global.capture_device` in
  `~/.config/vinput/config.json`. `org.fcitx.Vinput.Service` has no
  capture-device setter.
- The fcitx addon trigger key is `[TriggerKey]` in
  `~/.config/fcitx5/conf/vinput.conf`. It is a multi-entry list; users commonly
  change it and may keep several entries.
- A bare synthetic `Alt_R` did not trigger fcitx5-vinput 2.4.2 on 2026-10-30.
  A dedicated regular key `F24` did, with the user's existing `0=` entry kept.
  The `Control+Control_R` chord also worked.
- Step 11 changed the daemon default to `DEFAULT_TRIGGER_KEY = "DICTATE"`;
  vinput keeps the user's entries and adds `2=Dictate`. `F24` remains an
  explicit alternative. The live installed `0.1.0-3` unit still reads
  `trigger_key = "ASSISTANT"` from the user's config until the new build is
  installed.

## Deliverables

- ✅ Dedicated trigger key: `F24` was implemented first, resolved to
  `KEY_F24` by `atvv-input`, with parser/daemon tests and docs. Step 11
  superseded the default with `DICTATE`; `F24` remains configurable. Never
  assume a user shortcut.
- ⬜ Keep the user's existing `[TriggerKey]` entries: document
  `2=Dictate` alongside `0=<user key>`; never replace the user's entry.
- ⬜ Upstream proposal/PR to fcitx5-vinput:
  - `SetCaptureDevice(s name)` (apply live; validate the PipeWire node) and
    `GetCaptureDevice() -> s`.
  - `AddTriggerKey(s key)` / `SetTriggerKey(s key)` or a dedicated
    "ATVV trigger" that appends the configured key (`Dictate`) without
    replacing user entries; apply without a daemon restart.
  - `StartRecording() -> s` returning the scene id, or a documented empty-id
    contract.
  - Tests and documentation upstream.
- ⬜ Daemon device selection over D-Bus only:
  - When the source appears, select `atvv-miremote-rc003` only if the user has
    not chosen another explicit device.
  - Never override an explicit non-ATVV choice or the system default.
- ⬜ Config policy: `vinput_device = "auto" | "none"` or equivalent. Never make
  the ATVV source the system default.
- ⬜ Decide whether `trigger` (now `DICTATE`) remains the default or `auto`
  prefers D-Bus when the upstream API is available. The consumer-key default
  removes the user-key assumption, so `trigger` remains viable; D-Bus is the
  fallback for focus/scene-id edge cases.
- ⬜ Add a read-only `atvvctl vinput` diagnostic: report
  `global.capture_device`, the configured trigger entries, daemon mode/key,
  and the exact one-line fix.
- ⬜ Mock-vinput D-Bus tests for device selection, trigger appending, and
  no-override.
- ⬜ Real vinput check: pair, start, hold the voice key, record, commit text.

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
- Upstream integration must append the configured key, not overwrite the
  user's `[TriggerKey]` list.
- fcitx rewrites `~/.config/fcitx5/conf/vinput.conf` on addon exit. Edit it
  while fcitx is stopped; otherwise the edit is lost.
- fcitx5-vinput is GPL-3.0. Use its D-Bus API; do not link, vendor, or copy
  its code or config handling.
- The daemon must not write `~/.config/vinput/config.json` or
  `~/.config/fcitx5/conf/vinput.conf` directly. Those are another
  application's user configuration and would override a deliberate choice.
- The ATVV source must not become the system default; other applications keep
  their microphone.
- Device selection is the remaining hard blocker for true zero-config.

## Verification

```text
cargo test -p atvv-input
cargo test -p atvv-daemon
cargo test --workspace
dbus-run-session -- <mock-vinput test>
# real device, done 2026-10-30:
# daemon trigger_key=DICTATE; vinput [TriggerKey] 2=Dictate; voice press records
# after the upstream PR:
vinput device list                     # ATVV active without manual use
vinput daemon status                   # recording on voice press
journalctl --user -u vinput-daemon     # target.object=atvv-miremote-rc003
```

- `DICTATE` real-device path verified 2026-10-30 after step 11: fcitx
  received `Key(Dictate)` and vinput recorded from the ATVV source. The `F24`
  and `Control+Control_R` paths remain alternatives.
- Mock vinput with a non-default trigger: the dedicated key is appended and
  the user's key still works.

## Exit criteria

- ⬜ Fresh install + pair + start `atvvd`: fcitx5-vinput records from the ATVV
  source without replacing the user's trigger key or capture device.
- ⬜ A user with a customized trigger key keeps it and can use `Dictate`
  (or `F24`) as an additional ATVV trigger.
- ⬜ A user's explicit non-ATVV device is preserved.
- ⬜ No direct writes to another application's configuration; upstream API
  only.
- ⬜ Real-device run recorded with date, address, vinput version, and
  upstream PR/commit.
- ⬜ The default-mode decision and its rationale are recorded.

## Risks

- Upstream may reject the API or choose a different shape; keep daemon
  integration behind a capability check.
- A live `AddTriggerKey`/`SetCaptureDevice` may race with vinput's config or
  source enumeration; retry after the source appears and after reconnect.
- D-Bus mode's `StopRecording` scene-id contract is less certain than the
  trigger path; test both.
- Auto-selection must not fight an explicit user choice or the system default.
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
