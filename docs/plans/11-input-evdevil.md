# Step 11 — evdevil input backend

Status: ⬜ not-started
Depends on: 05 (input/uinput), 08 (trigger configuration), 10 (trigger-key
semantics)
Touches: `crates/atvv-input/`, `Cargo.toml`, `Cargo.lock`, `deny.toml`,
`docs/`

## Goal

Replace `evdev` 0.13.2 in `atvv-input` with `evdevil` 0.5.0 without changing
the ATVV protocol, the RC003 profile, or the daemon's public behaviour. The
migration must make `VOICECOMMAND`, `ASSISTANT`, and `DICTATE` resolvable and
emittable, and must preserve discovery, grabbing, event delivery, gesture
recognition, lost-node handling, and the synthetic trigger.

The decision and evidence are in
[ADR 0006](../decisions/0006-evdevil-input-backend.md) and
[`docs/research/consumer-keys.md`](../research/consumer-keys.md).

## Non-goals

- Do not implement context-dependent consumer-key mapping in this step.
- Do not fork or vendor `evdev`, `evdev-rs`, or `input-linux`.
- Do not change ATVV frames, CAPS, device-profile fields, or the fcitx5-vinput
  D-Bus contract.
- Do not replace the F24 trigger with a consumer key as part of the migration;
  trigger choice remains a separate decision.

## Deliverables

- ⬜ Remove `evdev` from the workspace and `atvv-input`; add
  `evdevil = "=0.5.0"`.
- ⬜ Add `0BSD` to `deny.toml` and confirm `cargo deny check` accepts the
  dependency graph.
- ⬜ Migrate `keymap.rs` from `KeyCode` to `evdevil::event::Key`; preserve the
  existing aliases, `parse_key_program`, modifier ordering, and tests.
- ⬜ Migrate discovery from `evdev::enumerate()`/`Device` to
  `evdevil::enumerate()`/`Evdev`. Skip `PermissionDenied` and `ENODEV` nodes;
  preserve the pointer-node safety filter.
- ⬜ Migrate the runtime reader from `fetch_events()` to `EventReader`; map
  `KeyEvent::state()` to the current press/release/repeat values; forward
  unmapped grabbed keys to the virtual keyboard; preserve lost-node releases.
- ⬜ Migrate the virtual keyboard from `VirtualDeviceBuilder` to
  `UinputDevice::builder()`; write `KeyEvent`/`InputEvent` batches; resolve
  the device node through `sysname()`/sysfs for diagnostics and tests.
- ⬜ Migrate the fake remote and its uinput test path; keep the fake profile
  out of `PROFILES`.
- ⬜ Update public `atvv-input` types (`KeyProgram`, `KeyStep`, `KeyMap`,
  `MatchedDevice`) and the `atvv-daemon` caller of `parse_key_program`.
- ⬜ Add regression tests that `VOICECOMMAND`, `ASSISTANT`, and `DICTATE`
  resolve to the expected codes, and that the uinput writer can emit them.
- ⬜ Document the fcitx5-vinput names (`VoiceCommand`, `Assistant`, `Dictate`)
  and the evdev/daemon names side by side.
- ⬜ Update the plan index, research index, and handoff in the same commit.

## Verification

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
reuse lint
cargo tree -i evdev                 # must fail: no evdev dependency
cargo run -q -p atvv-cli -- input fake-remote
```

Run the ignored uinput test when `/dev/uinput` and the fake-remote node are
accessible:

```sh
cargo test -p atvv-input voice_key_emits_the_trigger_key_and_keeps_the_raw_edge -- --ignored
```

Real-device verification is required before claiming hardware support:

```sh
cargo run -q -p atvv-cli -- doctor
cargo run -q -p atvv-cli -- monitor
# focus an fcitx input context, hold the RC003 voice key, release
```

## Exit criteria

- ⬜ No `evdev` dependency remains in `Cargo.lock` or any workspace manifest.
- ⬜ All workspace checks above pass.
- ⬜ A fake-remote run discovers, grabs, reads, and emits keys through the
  evdevil backend.
- ⬜ The daemon accepts `trigger_key = "DICTATE"` (and retains
  `VOICECOMMAND`/`ASSISTANT`), with the vinput side documented as
  `VoiceCommand`/`Assistant`/`Dictate`.
- ⬜ A real RC003 press still drives `MIC_OPEN`/`MIC_CLOSE`, the synthetic
  trigger, and vinput recording; the run is recorded with the address and date.
- ⬜ The consumer-key research and the deferred context-mapping decision are
  recorded; no per-app mapping code is added in this step.
- ⬜ Handoff and plan index describe the new state.

## Risks

- `evdevil` 0.5.0 is young; pin it exactly and keep the migration reversible.
- `enumerate()` error semantics differ; an unreadable node must not abort
  discovery.
- Dynamic uinput nodes may be root-owned without a uaccess rule; fake-remote
  tests may need an explicit permission check or sudo.
- `UinputDevice::sysname()` is `inputN`, not `eventN`; node lookup must use
  sysfs or enumeration.
- Public key types change; downstream code and tests may need small updates.
- `evdevil` uses 0BSD; the allow-list and third-party notices must be updated.
- A synthetic test proves the code path, not the RC003. Hardware claims require
  the recorded real-device run.

## Progress log

- 2026-10-30 — step planned after the evdevil feasibility spike and the
  decision to abandon the evdev fork. No implementation yet.
