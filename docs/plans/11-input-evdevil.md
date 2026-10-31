# Step 11 — evdevil input backend

Status: ✅ done
Depends on: 05 (input/uinput), 08 (trigger configuration), 10 (trigger-key
semantics)
Touches: `crates/atvv-input/`, `crates/atvv-daemon/`, `Cargo.toml`,
`Cargo.lock`, `deny.toml`, `THIRD-PARTY-NOTICES.md`, `docs/`

## Goal

Replace `evdev` 0.13.2 in `atvv-input` with `evdevil` 0.5.0 without changing
the ATVV protocol, the RC003 profile, or the daemon's public behaviour. The
migration makes `VOICECOMMAND`, `ASSISTANT`, and `DICTATE` resolvable and
emittable, and preserves discovery, grabbing, event delivery, gesture
recognition, lost-node handling, and the synthetic trigger.

The default synthetic trigger is the dictation consumer key (`DICTATE`, evdev
`0x24a`), by user direction on 2026-10-30. vinput keeps the user's own
entries.

The decision and evidence are in
[ADR 0006](../decisions/0006-evdevil-input-backend.md) and
[`docs/research/consumer-keys.md`](../research/consumer-keys.md).

## Non-goals

- Do not implement context-dependent consumer-key mapping in this step.
- Do not fork or vendor `evdev`, `evdev-rs`, or `input-linux`.
- Do not change ATVV frames, CAPS, device-profile fields, or the fcitx5-vinput
  D-Bus contract.
- Do not add per-app mapping or assume a physical button assignment for the
  three consumer keys.

## Deliverables

- ✅ Remove `evdev` from the workspace and `atvv-input`; add
  `evdevil = "=0.5.0"`.
- ✅ Add `0BSD` to `deny.toml` and confirm `cargo deny check` accepts the
  dependency graph (`evdevil` and `uoctl`).
- ✅ Migrate `keymap.rs` from `KeyCode` to `evdevil::event::Key`; preserve the
  existing aliases, `parse_key_program`, modifier ordering, and tests.
- ✅ Migrate discovery from `evdev::enumerate()`/`Device` to
  `evdevil::enumerate()`/`Evdev`. Skip `PermissionDenied`/unreadable and
  `ENODEV` nodes; preserve the pointer-node safety filter.
- ✅ Migrate the runtime reader from `fetch_events()` to `EventReader`; map
  `KeyEvent::state()` to the press/release edges; forward unmapped grabbed keys
  to the virtual keyboard; preserve lost-node releases.
- ✅ Migrate the virtual keyboard from `VirtualDeviceBuilder` to
  `UinputDevice::builder()`; write `KeyEvent` batches; resolve the device node
  through `sysname()` and sysfs for diagnostics and tests.
- ✅ Migrate the fake remote and its uinput test path; keep the fake profile
  out of `PROFILES`.
- ✅ Update public `atvv-input` types (`KeyProgram`, `KeyStep`, `KeyMap`,
  `MatchedDevice`) and the `atvv-daemon` caller of `parse_key_program`.
- ✅ Add regression tests that `VOICECOMMAND`, `ASSISTANT`, and `DICTATE`
  resolve to the expected codes, and that the uinput writer can emit them.
- ✅ Document the fcitx5-vinput names (`VoiceCommand`, `Assistant`, `Dictate`)
  and the evdev/daemon names side by side.
- ✅ Change `DEFAULT_TRIGGER_KEY` to `DICTATE`. vinput keeps the user's
  entries and adds `Dictate` at the next free index.
- ✅ Update the plan index, research index, and handoff in the same commit.

## Verification

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
reuse lint
cargo tree -i evdev                 # fails: no evdev dependency
cargo run -q -p atvv-cli -- input fake-remote
cargo test -p atvv-input -- --ignored --test-threads=1
```

Observed 2026-10-30:

- `cargo test --workspace` → 117 passed, 2 ignored.
- The two ignored uinput tests pass with `/dev/uinput` access:
  `voice_key_emits_the_trigger_key_and_keeps_the_raw_edge` (emits
  `KEY_DICTATE`) and `hold_and_release_reach_the_kernel`.
- `cargo run -q -p atvv-cli -- input fake-remote` → discovered and grabbed
  `/dev/input/event27`, created `/dev/input/event28`, recognized 12/12
  logical buttons, clean voice press/release.
- `cargo tree -i evdev` → `package ID specification 'evdev' did not match any
  packages`.
- `cargo deny check` → advisories, bans, licenses, sources ok; 0BSD accepted.
- `reuse lint` → REUSE 3.3 compliant, 96 files.

### Real RC003

Address `C0:5D:39:F8:2D:4C`, adapter `hci0`, 2026-10-30. The step-11 build ran
from the workspace with a temporary config (`trigger_key = "DICTATE"`); the
installed `0.1.0-3` unit was stopped for the run and restarted afterwards.

- `atvvctl status`: `connected=true`, `state=ready`, grabbed
  `/dev/input/event25`, `keyboard=true`, `vinput.trigger=DICTATE`.
- The virtual keyboard advertised `KEY_DICTATE` (event code 586); `evtest`
  observed `586 (KEY_DICTATE) value 1` then `value 0` during a physical press.
- Physical press captured by the daemon status sampler: `state=streaming`,
  `voice_key=true`, frames 0 → 104, samples 0 → 24,960; on release
  `state=ready`, `voice_key=false`, no `last_error`.
- With fcitx5-vinput `[TriggerKey]` containing `Dictate`, the same press
  produced fcitx `Key(Dictate)` press/release events
  (`keycode 594`, 23:10:51) and `vinput-daemon` used
  `target.object=atvv-miremote-rc003`, negotiated
  `rate=16000 channels=1 fmt=259`, and reported `recognition error: returned
  no text` because nothing was spoken. This is the previously missing joint
  real-press → daemon → synthetic trigger → vinput recording check.

## Exit criteria

- ✅ No `evdev` dependency remains in `Cargo.lock` or any workspace manifest.
- ✅ All workspace checks above pass.
- ✅ A fake-remote run discovers, grabs, reads, and emits keys through the
  evdevil backend.
- ✅ The daemon accepts `trigger_key = "DICTATE"` (and retains
  `VOICECOMMAND`/`ASSISTANT`), with the vinput side documented as
  `VoiceCommand`/`Assistant`/`Dictate`.
- ✅ A real RC003 press drives `MIC_OPEN`/`MIC_CLOSE`, the synthetic
  `KEY_DICTATE`, and vinput recording; the run is recorded above with address
  and date.
- ✅ The consumer-key research and the deferred context-mapping decision are
  recorded; no per-app mapping code is added in this step.
- ✅ Handoff and plan index describe the new state.

## Risks

- `evdevil` 0.5 is young; the requirement allows patch releases and the
  migration is reversible.
- `enumerate()` error semantics differ; unreadable or vanished nodes are
  skipped per entry.
- Dynamic uinput nodes may be root-owned without a uaccess rule; fake-remote
  tests need `/dev/uinput` access.
- `UinputDevice::sysname()` is `inputN`, not `eventN`; node lookup uses
  `/sys/class/input/<sysname>/event*`.
- Public key types changed; downstream code and tests were updated.
- `evdevil` and `uoctl` use 0BSD; the allow-list and third-party notices are
  updated.
- fcitx rewrites `~/.config/fcitx5/conf/vinput.conf` on addon exit. Edit the
  file while fcitx is stopped, otherwise the change is overwritten by the
  in-memory configuration.
- A synthetic test proves the code path, not the RC003. The real-device run
  above is the hardware evidence.

## Progress log

- 2026-10-30 — step planned after the evdevil feasibility spike and the
  decision to abandon the evdev fork. No implementation yet.
- 2026-10-30 — migrated `atvv-input` to `evdevil` 0.5.0: `Key`, `Evdev`,
  `EventReader`, `UinputDevice`, sysfs node lookup, 0BSD allow-list, notices,
  and consumer-key regression tests. Workspace checks and both ignored uinput
  tests pass.
- 2026-10-30 — user direction: the default trigger is `DICTATE`; vinput keeps
  the user's entries and adds `Dictate` at the next free index. The daemon
  default is now `DEFAULT_TRIGGER_KEY = "DICTATE"`.
- 2026-10-30 — real RC003 run: evdevil grabbed `/dev/input/event25`, emitted
  `KEY_DICTATE`, streamed 104 frames / 24,960 samples on a press, and
  fcitx5-vinput recorded from `atvv-miremote-rc003` at 16 kHz on the same
  press. The joint real-press check is closed.
- 2026-10-31 — dependency upgrade: the workspace requirement is now
  `evdevil = "0.5"`, so patch releases flow in automatically; the migration
  remains reversible and ADR 0006 carries the current policy.
