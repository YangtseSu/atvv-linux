# B01 — Recording-key press and release actions

Status: ⏸ backlog
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
  `combo:<mods>+<name>`, `exec:<command>`. `dictation:*` is rejected here
  because the physical key already owns mic start/stop; two dictation sources
  would race.
- `key` and `combo` are taps at the edge (down then up), matching `RunAction`,
  not keys held for the duration of the recording. A held-key action needs a
  separate action form and is out of scope.
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

- ⬜ `atvv-config`: `[recording]` in `daemon.toml` and `ConfigDocument`;
  optional fields default to `none`; validation rejects dictation, unknown
  keys, malformed actions, and `exec` without `allow_exec`; `atvvctl config
  set --recording-press ... --recording-release ...`; `config get` prints the
  section. Keep `schema_version = 1`: both fields are optional, and older
  binaries reject the unknown field instead of silently dropping it.
- ⬜ `atvv-daemon`: execute the press action after `request_voice(true)` and
  the release action after `request_voice(false)`, outside capture; reuse the
  checked-action executor; log bounded errors; do not restart the input
  runtime when only these fields change. `RunAction` can execute either action
  for the GUI's Run once.
- ⬜ `atvv-gui`: the recording-key page gets Press and Release rows with
  Learn Key, Update/Reset, inline validation, and Run once; dirty/Apply sends
  the complete document through `SetConfig`; test mode shows both edges
  without executing them.
- ⬜ Docs: configuration schema, action grammar, recording-key page, and a
  stated non-change for the provider contract.
- ⬜ Tests:
  - Config parse/serialize/default round-trip; reject dictation and
    `exec` without permission.
  - Daemon fake-remote press/release: each action runs once in edge order;
    mic start/stop still happens; capture suppresses both.
  - GUI model/UI smoke: edit, validate, apply, and display the actions.
  - Real RC003 run only if claiming hardware behavior: a harmless `exec:`
    marker on each edge plus a working mic session; record it.

## Exit criteria

- ⬜ A `daemon.toml` with `[recording]` round-trips; an absent section means
  both actions are `none`; an invalid action leaves the old config active.
- ⬜ One fake-remote press and release each runs its configured action exactly
  once, in the documented order, while the microphone and trigger key behave
  as before.
- ⬜ Test mode reports both edges and executes neither action.
- ⬜ The GUI saves the two fields through the daemon and displays them after a
  restart; no direct file write.
- ⬜ The provider contract and PipeWire metadata are unchanged.

## Risks

- `exec` latency: a release command can wait for a press command; the mic
  still closes first, but the action may be late. The 5 s bound is visible in
  status and the log.
- A user may map both `trigger_key` and a press action to the same key and get
  a duplicate. Document the relationship instead of deduplicating.
- An edge can be lost when the daemon or input runtime restarts mid-press; no
  synthetic release action is invented. The next press is a fresh edge.
- Old GUI/CLI builds cannot deserialize a document with `[recording]` because
  the structs deny unknown fields; the daemon, CLI, and GUI ship together.
- A press action that fails must not roll back the microphone edge; report it
  through the existing bounded error path.

## Progress log

- 2026-10-31 — planned as B01 after the step-14 recording-key page. It is
  backlog, not scheduled; step 15 remains the next numbered step.
