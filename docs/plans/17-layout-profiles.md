# Step 17 — Layout profiles

Status: ⬜ not-started
Depends on: 13, 14, 15
Touches: `crates/atvv-config/`, `crates/atvv-daemon/`, `crates/atvv-cli/`,
`crates/atvv-gui/`, `docs/`

## Goal

Give the user several named key layouts (button slots plus gesture timing)
and an explicit active layout. `default` is always available and derived from
the device profile; user layouts are stored in `daemon.toml`. Switching a
layout applies it to the input runtime and the GUI immediately, without an
automatic foreground-app switch.

"NFC" is not part of this step; see backlog B02.

## Why layouts, not another profile

`DeviceProfile` already means the hardware/firmware identity
(`miremote-rc003`). A user layout is a different object: a named map of what
the buttons do. The TOML and UI call it a **layout** so the two concepts never
collide.

Prior art:

- mi-ao separates hardware calibration from button presets and keeps the
  official default read-only.
- remote-mic-app keeps a base mapping plus per-device overrides.
- Open Voice Bridge keeps device facts and hotspot geometry as data, not
  actions.
- sayall-omarchy uses one `[button_map]` table with the same action grammar
  ATVV already has.

Evidence and pinned commits: `docs/research/layout-profiles-presets.md`.

## Data model (schema v2)

```toml
schema_version = 2
profile = "miremote-rc003"          # device profile, unchanged
active_layout = "default"
device = "C0:5D:39:F8:2D:4C"
gain_db = 0
input = true
vinput = "trigger"
trigger_key = "DICTATE"
allow_exec = false

[layouts.coding]
name = "Coding"
description = "Enter/Escape navigation plus editor and browser shortcuts"

[layouts.coding.gesture]
long_press_ms = 600

[layouts.coding.buttons.up]
single = "key:KEY_UP"
```

- `default` is synthesized from the device profile and cannot be stored,
  renamed, deleted, or edited. It is the read-only baseline.
- A layout has `name` (required, unique case-insensitively), optional
  `description`, optional `buttons`, and optional `gesture` overrides.
- Layout ids are lowercase ASCII `[a-z0-9][a-z0-9_-]{0,31}`; `default` and the
  preset ids from step 18 are reserved.
- Global fields (`device`, `adapter`, `gain_db`, `input`, `vinput`,
  `trigger_key`, `allow_exec`) stay global. A layout changes only actions and
  gesture timing, never the exec policy or the device.
- A missing slot still means "use the device profile default"; `none` still
  swallows the button.
- Limits: at most 32 layouts; name 1–40 chars; description at most 120 chars.
- A v2 document must not contain the v1 top-level `[buttons]` or `[gesture]`
  tables.

### Migration

- `schema_version = 1` with top-level buttons/gesture becomes a user layout
  `custom` named "Custom", and `active_layout = "custom"`.
- A v1 document with no overrides becomes `active_layout = "default"`.
- `schema_version > 2` is rejected.
- The first save writes v2 and drops the old top-level tables.
- `SetConfig` accepts only v2, so a stale GUI fails loudly instead of
  silently dropping layouts.

## Daemon

- `Config`/`ConfigDocument` gain `active_layout` and `layouts`; the active
  layout's map and gesture timing drive the input runtime.
- Applying a layout restarts the input runtime only when the key set or
  gesture timing changes; gain/audio is not touched.
- `StatusView.layout` reports `{ id, name }`.
- D-Bus:
  - `GetConfig` returns v2.
  - `SetConfig` accepts v2; validation is all-or-nothing.
  - New `SetLayout(s) -> ()` switches the active layout by id, persists, and
    emits `LayoutChanged(s)`.
- A failed switch leaves the active layout, runtime, and file unchanged.

## CLI

- `atvvctl layout list` — ids, names, active marker, source (`default` or
  `user`).
- `atvvctl layout show <id>` — slots and gesture overrides.
- `atvvctl layout use <id>` — `SetLayout`.
- `atvvctl layout new <id> --name <name> [--from <id>]`.
- `atvvctl layout rename <id> <name>`.
- `atvvctl layout delete <id>` — refuses the active layout and `default`.
- `atvvctl layout reset <id>` — clears buttons/gesture back to profile
  defaults.
- `atvvctl config set --button …` edits the active layout; add `--layout <id>`
  to target another one.
- `atvvctl map test` resolves the active layout; add `--layout <id>`.

## GUI

- A layout selector in the sidebar home view (near the battery row) shows the
  active layout and opens a manager.
- Manager: Default (read-only) plus user layouts; New, Duplicate, Rename,
  Delete, Reset. The active layout is marked.
- Switching uses `SetLayout` and refreshes the board/editor immediately.
- Switching is disabled while the document is dirty; the UI says "Apply or
  revert first" instead of discarding edits silently.
- The step 15 mapping board always renders the active layout; the device
  diagram and hotspot geometry do not change per layout.
- After a switch, the header/status shows the active layout name.

## Tests

- Schema v1 migration: buttons/gesture become `custom`, active id is set,
  and a second load is stable.
- Validation: reserved ids, duplicate names, unknown active layout, >32
  layouts, overlong names/descriptions, v1+v2 field mixing, invalid slots.
- D-Bus `SetLayout`: success, unknown id, `default`, persistence across
  restart, `LayoutChanged`.
- CLI: list/show/use/new/rename/delete/reset against an isolated bus.
- GUI: selector lists layouts; switching sends `SetLayout`; dirty switching
  is blocked; editor edits the active layout.
- Real RC003: two layouts with different actions for one button; switch and
  press the remote; the emitted key changes without a daemon restart; the
  active layout survives a restart.

## Verification

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
xvfb-run -a cargo test -p atvv-gui --test ui
```

Real-device check:

```text
atvvctl layout list
atvvctl layout new coding --name Coding
atvvctl config set --layout coding --button up --trigger single --action key:KEY_F13
atvvctl layout use coding
atvvctl status        # layout.id == "coding"
# press Up on the RC003; KEY_F13 is emitted
atvvctl layout use default
# press Up again; the default Up action is emitted
```

## Exit criteria

- ⬜ A v1 file migrates without losing the old button map.
- ⬜ `default` is read-only; user layouts can be created, renamed, deleted,
  and reset.
- ⬜ The active layout is persisted and shown in status.
- ⬜ Switching applies live and changes the emitted key on the real RC003.
- ⬜ Invalid documents or unknown layouts leave the current state untouched.
- ⬜ CLI, GUI, and D-Bus agree on the active layout.
- ⬜ No automatic per-app/focus switching is introduced.

## Risks

- Layout switching restarts the input runtime; a failed restart must roll
  back to the previous layout.
- A stale client that only knows v1 could erase layouts if it writes v2
  fields blindly; reject v1 writes and ship CLI/GUI together.
- Gesture timing per layout means a switch can change double/long behavior;
  show this in `layout show` and the GUI.
- Too many layouts make the selector unwieldy; cap at 32 and keep the
  manager searchable-free.
- `exec:` policy is global by design; a layout must never enable it.
