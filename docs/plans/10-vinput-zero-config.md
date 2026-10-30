# Step 10 — zero-config vinput integration

Status: ⬜ not-started
Depends on: 08 (integration); can start before 09 acceptance
Touches: `crates/atvv-daemon/src/vinput.rs`, `crates/atvv-daemon/src/config.rs`,
`docs/`, upstream fcitx5-vinput

## Goal

After `atvvctl pair`, a user starts `atvvd` and fcitx5-vinput records from
`atvv-miremote-rc003` without editing atvv or vinput configuration.

## Current state

- vinput discovers the resident `atvv-miremote-rc003` source, but the user must
  run `vinput device use atvv-miremote-rc003` and restart `vinput-daemon`.
- The selected device is `global.capture_device` in
  `~/.config/vinput/config.json`. `org.fcitx.Vinput.Service` has no
  capture-device setter.
- The fcitx addon trigger key is `[TriggerKey]` in
  `~/.config/fcitx5/conf/vinput.conf`. It is not exposed over D-Bus.
- Users commonly change the trigger key; `Alt_R` is only vinput's default.
  A daemon that always synthesizes `Alt_R` silently fails for any customized
  setup, so the default must not assume it.
- `vinput = "dbus"` avoids trigger-key matching, but still uses whatever
  capture device vinput has selected; it does not solve device selection.
- Defaulting to `dbus` before the device-selection API is unsafe: a voice edge
  would start vinput on `global.capture_device`. With the initial `default`,
  that is the system microphone, not `atvv-miremote-rc003`.

## Deliverables

- ⬜ Upstream proposal/PR to fcitx5-vinput:
  - `SetCaptureDevice(s name)` (apply live; validate that the PipeWire node
    exists) and `GetCaptureDevice() -> s`.
  - `GetTriggerKey() -> s` and `SetTriggerKey(s chord)`, or a
    "follow source name" option, applied without a daemon restart.
  - `StartRecording() -> s` returning the scene id, or an explicit documented
    empty-id contract, so `StopRecording` does not require guessing.
  - Tests and documentation in the upstream project. These additions are
    acceptable PR scope because they make the atvv integration work without
    assuming user defaults.
- ⬜ Record the pinned upstream snapshot and the final API contract in
  `docs/research/reference-projects.md` and
  `docs/research/protocol-and-device.md` §8.
- ⬜ Daemon integration over D-Bus only:
  - When the source is created/recreated, select `atvv-miremote-rc003` only if
    the user has not chosen another explicit device.
  - Never override an explicit non-ATVV choice.
  - Define the source-disappears/reconnect transition and test it.
- ⬜ Config policy: `vinput_device = "auto" | "none"` or equivalent. Never
  make the ATVV source the system default.
- ⬜ Change the default mode so it never assumes a trigger key:
  - `auto` (target): if vinput exposes `GetTriggerKey`, synthesize the actual
    key; else if `org.fcitx.Vinput` is present, use D-Bus; else leave the ATVV
    session usable and report a diagnostic.
  - `dbus`: no trigger-key dependency; still uses the selected device and the
    scene-id contract.
  - `auto` must refuse to start on a non-ATVV device when it cannot select the
    source; report the mismatch instead of silently recording the system
    microphone.
  - `trigger`: only when the key is explicitly configured or discovered.
    Never assume `Alt_R` or any other default.
  - Record the migration: an explicit `vinput = "trigger"` keeps its behavior;
    configuration without `vinput` moves to the new default.
- ⬜ Add a read-only `atvvctl vinput` diagnostic: report
  `global.capture_device`, `[TriggerKey]`, daemon mode/trigger, and the exact
  one-line fix.
- ⬜ Mock-vinput D-Bus tests for `SetCaptureDevice`, no-override, and fallback.
- ⬜ Real vinput check once the upstream PR is available: pair, start, hold the
  voice key, record, commit text.

## Design notes

- fcitx5-vinput is GPL-3.0. Use its D-Bus API; do not link, vendor, or copy
  its code or config handling.
- The daemon must not write `~/.config/vinput/config.json` or
  `~/.config/fcitx5/conf/vinput.conf` directly. Those are another
  application's user configuration and would override a deliberate choice.
- `SetCaptureDevice` must be additive and stable; protocol structs must not
  cross D-Bus.
- The trigger key is addon-local. A daemon cannot infer a custom key unless
  the addon exposes it; the alternatives are D-Bus mode or one manual
  alignment.
- Do not assume `Alt_R` or any user default. Changing shortcuts is normal;
  the daemon must discover the active key or use D-Bus.
- Upstream additions beyond device selection are acceptable when they improve
  the integration; keep them additive and documented.
- The source is resident and vinput may cache the device list;
  `SetCaptureDevice` should apply without a daemon restart, or the upstream
  API should expose a reload method.
- The ATVV source must not become the system default; other applications keep
  their microphone.

## Verification

```text
cargo test -p atvv-daemon vinput
cargo test --workspace
dbus-run-session -- <mock-vinput test>
# after the upstream PR:
vinput device list                     # ATVV active without manual use
vinput daemon status                   # recording on voice press
journalctl --user -u vinput-daemon     # target.object=atvv-miremote-rc003
```

- Mock vinput with a non-default trigger key: D-Bus mode records; trigger mode
  works only when the key is discovered or explicitly configured.

## Exit criteria

- ⬜ Fresh install + pair + start `atvvd`: fcitx5-vinput records from the ATVV
  source without editing atvv or vinput configuration.
- ⬜ A user with a customized trigger key records without editing atvv or
  vinput configuration.
- ⬜ A user's explicit non-ATVV device is preserved.
- ⬜ No direct writes to another application's configuration; upstream API
  only.
- ⬜ Real-device run recorded with date, address, vinput version, and
  upstream PR/commit.
- ⬜ The default-mode decision and its rationale are recorded.

## Risks

- Upstream may reject the API or choose a different shape; keep daemon
  integration behind a capability check.
- A live `SetCaptureDevice` may race with vinput's source enumeration; retry
  after the source appears and after reconnect.
- D-Bus mode's `StopRecording` scene-id contract is less certain than the
  trigger path; test both.
- Auto-selection must not fight an explicit user choice or the system default.

## Progress log

- 2026-10-30 — step planned after the zero-config review; no implementation.
- 2026-10-30 — revised after review: the daemon must not assume `Alt_R` or any
  default trigger key. The upstream PR may add trigger get/set, live apply,
  and scene-id improvements; the default mode moves to `auto`/`dbus`.
