# Step 13 — keymap configuration backend

Status: ⬜ not-started
Depends on: 12, 06
Touches: `crates/atvv-config/` (new), `crates/atvv-input/`,
`crates/atvv-daemon/`, `crates/atvv-cli/`, `docs/`

## Goal

Make the per-button action map persistent, validated, and editable at runtime.
No GUI is required for this step. A user must be able to change a mapping from
`atvvctl`, reload it, and see the input runtime use it without a daemon
restart.

## Current state

- The provider contract and generic status surface are complete (step 12).
- `atvv-input` already has the action grammar and default `ActionMap`:
  `none`, `key:<name>`, `combo:<mods>+<key>`, `exec:<command>`, and
  `dictation:start|stop|toggle`.
- `daemon.toml` does not yet persist per-button mappings, and there is no
  D-Bus API to read, apply, or capture them.
- Step 10 is upstream-blocked and does not block this step.

## Deliverables

- ⬜ New `atvv-config` crate shared by daemon, CLI, and GUI:
  - Schema version (`schema_version = 1`).
  - Device/profile selection, gain, input enable, vinput mode, trigger key.
  - Per-button `single`/`double`/`long` action strings.
  - Gesture timing overrides where the profile allows them.
  - `allow_exec` opt-in for `exec:` actions.
  - Parse, validate, and serialize using the existing `atvv-input` action
    grammar.
- ⬜ `daemon.toml` accepts `[buttons.<logical-button>]` and validates every
  action at load time. Invalid actions name the button, trigger, and value.
- ⬜ Backward compatibility: an existing config without `[buttons]` loads the
  current defaults (pass-through keys, power `none`, no desktop shortcuts).
- ⬜ The voice key remains special and cannot be mapped to an ordinary action.
- ⬜ The daemon applies a changed map live to the input runtime. A failed
  reload leaves the previous map active and reports the error.
- ⬜ D-Bus additions on `org.atvv.Daemon`:
  - `GetConfig() -> s` (JSON view of the editable config).
  - `SetConfig(s) -> ()` (validate, atomically apply, report errors).
  - `BeginCapture() -> ()`, `EndCapture() -> ()` for “press a remote key”
    mode.
  - Signal `ButtonEvent(s button, s trigger, b pressed)` while capturing.
  - Capture mode ends automatically if the GUI disconnects.
- ⬜ `atvvctl config get`, `atvvctl config set`, `atvvctl capture`, and
  `atvvctl map test` (human and `--json` where applicable).
- ⬜ `exec:` policy:
  - Disabled unless `allow_exec = true`.
  - Runs as the user, never root; timeout and failure are reported.
  - Selecting or previewing an `exec` action in a UI does not run it.
- ⬜ Tests:
  - Config round-trip and schema version.
  - Invalid action, unknown button, voice-key mapping, duplicate/invalid
    chord, `exec` disabled.
  - Live reload success and failure.
  - D-Bus `GetConfig`/`SetConfig`/capture under `dbus-run-session`.
  - Fake remote produces button events; no real evdev grab in tests.
- ⬜ Docs: config schema, action grammar, safety rules, `atvvctl` examples.

## Design notes

- `atvv-config` is the only schema parser. The daemon, CLI, and GUI must not
  grow a second config parser.
- The D-Bus payload is a JSON view, not internal Rust types. Keep it stable
  and versioned with `schema_version`.
- Capture mode reports logical buttons only; gesture recognition stays in the
  input runtime. The GUI uses capture to identify a physical button, not to
  reimplement the gesture FSM.
- `SetConfig` is atomic: parse and validate the whole document before applying
  it. Do not partially apply a failed update.
- The daemon owns `exec` execution. The GUI and CLI only edit or preview it.
- `exec` is a general escape hatch, not a scripting language. Keep command
  strings opaque but bounded; no argument rewriting that changes meaning.
- Do not store the keymap in the provider contract. The contract describes
  audio and recording keys; the keymap is local configuration.
- The existing profile supplies defaults; a user map overrides individual
  slots without replacing the whole profile.

## Out of scope

- GUI widgets and remote diagram (step 14).
- Per-application focus detection or automatic shortcut discovery.
- Multi-device keymaps.
- A plugin system or arbitrary scripting language.
- Writing fcitx5-vinput or other consumer configuration.

## Verification

```text
cargo test -p atvv-config
cargo test -p atvv-input
cargo test -p atvv-daemon
dbus-run-session -- cargo test -p atvv-daemon --test config_dbus
cargo run -q -p atvv-cli -- config get --json
cargo run -q -p atvv-cli -- capture
cargo run -q -p atvv-cli -- map test
```

## Exit criteria

- ⬜ A mapping can be changed in `daemon.toml` or by `atvvctl`, reloaded, and
  observed through the fake remote without restarting `atvvd`.
- ⬜ Invalid config does not replace the active map.
- ⬜ The voice key cannot be mapped to an ordinary action.
- ⬜ `exec:` cannot run unless explicitly enabled.
- ⬜ D-Bus config/capture API is isolated and tested.
- ⬜ No GUI code is required for this step.

## Risks

- A live map update can race with a button press; apply at a defined boundary
  and test the old/new map transition.
- `exec` is powerful; a bad config can run arbitrary user commands. Keep it
  opt-in and make the warning visible in every editor.
- The existing `daemon.toml` is `deny_unknown_fields`; adding `schema_version`
  and `[buttons]` needs a deliberate backward-compatible load path.
- The provider contract and the keymap are separate surfaces; do not merge
  them into one versioned object.

## Progress log

- 2026-10-31 — planned after the GTK4/libadwaita UI decision; no
  implementation yet. Step 12 is complete; step 10 is upstream-blocked.
