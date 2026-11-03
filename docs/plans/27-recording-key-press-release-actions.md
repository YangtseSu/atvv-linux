# 27 — Recording-key press and release actions

Status: ✅ done
Depends on: 14 (recording-key page)
Touches: `crates/atvv-config/`, `crates/atvv-daemon/`, `crates/atvv-gui/`,
`docs/configuration.md`, `docs/gui.md`, tests

## Goal

The top-right recording key is grabbed and fixed: its edges start and stop the
ATVV microphone, and the input runtime emits `trigger_key` while it is held.
Add two optional actions, one on press and one on release, without changing the
microphone path, the synthetic trigger, or the provider contract.

Typical uses: announce recording with a command on press, emit a marker key on
release, or run a notifier per edge. Consumers still bind one of the declared
recording keys; these actions are local configuration, not a new consumer
interface.

## Interface

```toml
[recording]
press = "none"      # default
release = "none"
```

- The action grammar is the button grammar: `none`, `key:<name>`,
  `combo:<mods>+<name>`, `exec:<command>`, and `pointer:<action>`.
  `dictation:*` is rejected here because the physical key already owns mic
  start/stop; two dictation sources would race.
- `key`, `combo`, and `pointer` actions are taps at the edge (down then up),
  matching `RunAction`, not keys held for the duration of the recording. A
  held-key action needs a separate action form and is out of scope.
- `exec` keeps the existing policy: `allow_exec = true`, one command at a
  time, 5 s timeout, closed stdio, never as root. The microphone edge runs
  first; the action follows. A release `exec` can wait behind a press `exec`.
- Capture/test mode reports the edge but never executes either action.
- The synthetic `trigger_key` and a press action are independent. If both emit
  the same key, both are sent; set `trigger_key = "none"` to use only the
  action.
- No provider-contract change: `atvv.recording_keys`, `active_key`, and the
  PipeWire source metadata remain as they are.

## Deliverables

- ✅ `atvv-config`: `[recording]` in `daemon.toml` and `ConfigDocument`;
  optional fields default to `none`; validation rejects dictation, unknown
  keys, malformed actions, and `exec` without `allow_exec`; `atvvctl config
  set --recording-press ... --recording-release ...`; `config get` prints the
  section. `schema_version` stays `3`: both fields are optional, and older
  binaries reject the unknown field instead of silently dropping it.
- ✅ `atvv-daemon`: the press action runs after the press edge and the release
  action after the release edge, outside capture, through the same checked
  action dispatcher as `RunAction`; errors use the bounded `last_error` path;
  the input runtime is not restarted when only these fields change.
- ✅ `atvv-gui`: the recording-key page has **On press** and **On release**
  rows with per-row Learn Key and Run once buttons, shared Update/Reset,
  inline validation, and atomic Update. Test mode shows both edges and
  executes neither action.
- ✅ Docs: configuration schema, action grammar, recording-key page, the
  apply path, and the stated provider-contract non-change.
- ✅ Tests:
  - Config parse/serialize/default round-trip; reject dictation and
    `exec` without permission; a rejected edge leaves the document unchanged.
  - Daemon fake-remote press/release: each action runs once in edge order;
    mic start/stop still happens; capture suppresses both and reports both
    edges.
  - GUI model/UI smoke: edit, validate, apply, and display the actions;
    `SetConfig` persists `[recording]` to the file.
  - No hardware behavior is claimed: the edge actions were verified with the
    fake remote and the GUI only, so the `[hardware]` run was not required.

## Verification

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --exclude atvv-gui
scripts/gui-tests.sh
```

Recorded 2026-11-03:

- `cargo fmt --all -- --check` → clean.
- `cargo clippy --workspace --all-targets -- -D warnings` → clean.
- `cargo test --workspace --exclude atvv-gui` → 217 passed, 7 ignored.
- `scripts/gui-tests.sh` → 59 passed, 2 ignored. The GUI suite runs on a
  private `mutter --headless` Wayland display, so no test window appears on
  the desktop session.
- `atvv-config` → 31 passed; `atvv-daemon` → 44 passed, 1 ignored.

## Out of scope

- A held-key action form (an action that stays active while the recording key
  is held).
- Any per-application branch or new consumer interface.

## Design notes

- The dedicated-key rule: the recording key is not action-mappable; it always
  owns the microphone edge. The optional actions are additions, not a
  replacement.
- The edge order is fixed: microphone first, then the configured action. A
  failing action never rolls back the microphone edge.
- The few-millisecond tap of `key`/`combo` is intentional. Holding a key for
  the duration of the recording would need press/release semantics of its own.
- The release action may queue behind the press action in the serial executor;
  the 5 s bound and `status.last_error` keep it observable.
- Old GUI/CLI builds cannot deserialize a document with `[recording]` because
  the structs deny unknown fields; the daemon, CLI, and GUI ship together.
- Test mode uses the existing `ButtonEvent` voice edges (`single` and
  `release`); it never runs the configured action, and the page's Run once is
  the explicit execution path.

## Exit criteria

- ✅ A `daemon.toml` with `[recording]` round-trips; an absent section means
  both actions are `none`; an invalid action leaves the old config active.
- ✅ One fake-remote press and release each runs its configured action exactly
  once, in the documented order, while the microphone and trigger key behave
  as before.
- ✅ Test mode reports both edges and executes neither action.
- ✅ The GUI saves the two fields through the daemon and displays them after a
  restart; no direct file write.
- ✅ The provider contract and PipeWire metadata are unchanged.

## Risks

- `exec` latency: a release command can wait for a press command; the mic
  still closes first, but the action may be late. The 5 s bound is visible in
  status and the log.
- A user may map both `trigger_key` and a press action to the same key and get
  a duplicate. The docs state the relationship instead of deduplicating.
- An edge can be lost when the daemon or input runtime restarts mid-press; no
  synthetic release action is invented. The next press is a fresh edge.
- Old GUI/CLI builds cannot deserialize a document with `[recording]` because
  the structs deny unknown fields; the daemon, CLI, and GUI ship together.
- A press action that fails must not roll back the microphone edge; it is
  reported through the existing bounded error path.

## Progress log

- 2026-10-31 — planned as B01 after the step-14 recording-key page. It was
  backlog, not scheduled; step 15 remained the next numbered step.
- 2026-11-03 — promoted from B01 to step 27 and implemented. `RecordingToml`,
  `RecordingActions`, and `RecordingDocument` carry the optional edges through
  TOML, validation, and the D-Bus document; `parse_recording_action` shares
  the button grammar and rejects `dictation:*`. The daemon extracts
  `dispatch_action` from `RunAction` and calls it after the voice edge, before
  the vinput branch; capture returns before both. The GUI page applies the
  output key and both edges in one `set_recording_edges` transaction, and the
  test bar now updates on voice release as well as press. Verification is
  recorded above; the XTest pointer clicks still need an X server and are not
  part of the headless Wayland suite.
