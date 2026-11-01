# Step 17 — Layout profiles

Status: ✅ done
Depends on: 13, 14, 15
Touches: `crates/atvv-config/`, `crates/atvv-daemon/`, `crates/atvv-cli/`,
`crates/atvv-gui/`, `docs/`

## Goal

Give the user several named key layouts (button slots plus gesture timing)
and an explicit active layout. `default` is always available and derived from
the device profile; user layouts are stored in `daemon.toml`. `default` is a
template: it can be selected and edited on the board, but its edits are staged
in a working copy and Apply saves them as a new user layout. Switching a
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

[layouts.editor]
name = "Editor"
description = "Enter/Escape navigation plus editor and browser shortcuts"

[layouts.editor.gesture]
long_press_ms = 600

[layouts.editor.buttons.up]
single = "key:KEY_UP"
```

- `default` is synthesized from the device profile and cannot be stored,
  renamed, deleted, or reset. It is a template, not a stored layout: no
  `[layouts.default]` table is written, and the template itself always
  resolves to profile defaults. Editing a slot while `default` is active
  changes only a working copy; Apply saves that copy as a new user layout and
  activates it. Revert discards the staged copy, and switching layouts or
  devices is blocked while the document is dirty.
- A layout has `name` (required, unique case-insensitively), optional
  `description`, optional `buttons`, and optional `gesture` overrides.
- Layout ids are lowercase ASCII `[a-z0-9][a-z0-9_-]{0,31}`; `default` and the
  preset ids from step 18 (`media`, `coding`, `slides`) are reserved. The
  examples here use `editor` for that reason.
- Global fields (`device`, `adapter`, `gain_db`, `input`, `vinput`,
  `trigger_key`, `allow_exec`) stay global. A layout changes only actions and
  gesture timing, never the exec policy or the device.
- A missing slot still means "use the device profile default"; `none` still
  swallows the button.
- Limits: at most 32 layouts; name 1–40 chars; description at most 120 chars.
- A v2 document must not contain the v1 top-level `[buttons]` or `[gesture]`
  tables.
- Per-device layout assignment is step 21. Keep layout records separate and
  reference them by id; step 21 adds the device registry and `active_device`
  without changing the layout payload.

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
- `atvvctl layout rename <id> <name>` — refuses `default`.
- `atvvctl layout delete <id>` — refuses the active layout and `default`.
- `atvvctl layout reset <id>` — clears buttons/gesture back to profile
  defaults; refuses `default`.
- `atvvctl config set --button …` edits the active layout; add `--layout <id>`
  to target another one. When the active or target layout is `default`, the
  command refuses with a pointer to `atvvctl layout new <id> --from default`.
  The selected user layout receives the edit.
- `atvvctl map test` resolves the active layout; add `--layout <id>`.

## GUI

The layout UI is decided in the layout section of
[`15-mapping-board.html`](15-mapping-board.html); implement it with GTK4 and
libadwaita widgets, not the HTML/CSS.

- A **layout chip** sits in the board header next to the device chip and shows
  the active layout name. It opens a popover with every layout: active check,
  a `Template` tag on Default, and `Manage layouts…`.
- The popover switches layouts directly. Switching is blocked while the
  document is dirty and says "Apply or revert before switching layouts."
- `Manage layouts…` opens an `AdwDialog`:
  - Layout rows show name, description, active marker, and `Template` for
    Default. The row itself switches; a trailing action opens inline
    `Duplicate`, `Rename`, `Delete`, `Reset` actions. Default offers
    `Duplicate` only; Rename, Delete, and Reset are disabled.
  - `New layout…`, `Rename`, and the template save flow use the same inline
    name/description form with Create/Save and Cancel. Names are unique
    case-insensitively; Default and the preset ids are reserved.
  - Delete refuses Default and any layout active on a device; Reset clears a
    user layout to profile defaults.
  - The footer carries `New layout…`; while the document is dirty it also
    carries `Revert` and `Apply`, because the window header is behind the modal.
- **Default is a template, not read-only.** The board is editable while
  Default is active: slot and hotspot clicks open the normal editor and write
  to a working copy. No lock banner is shown. The dirty banner reads "Unsaved
  changes on Default — Apply saves them as a new layout."
  - Apply on a changed template opens the **Save as layout** form instead of
    writing the document. Name, description, and the staged slot map create a
    new user layout, which becomes active; Default itself stays unchanged.
  - When only the layout list changed (New, Rename, Delete, or Reset of other
    layouts) and the template's slot map is untouched, Apply saves those list
    changes in place without creating a layout.
  - Revert discards the staged template edits and leaves Default active.
  - The header Apply and the manager footer Apply follow the same rule.
  - Duplicate on a template creates a user layout from the profile baseline.
    Use Save as layout to keep staged edits.
- Switching uses `SetLayout` and refreshes the board/editor immediately. Test
  mode stays active but clears the last pressed result. The active layout is
  assigned per device (step 21); switching devices updates the chip.
- The header/status shows the active layout name. The device diagram and
  hotspot geometry do not change per layout.

## Tests

- Schema v1 migration: buttons/gesture become `custom`, active id is set,
  and a second load is stable.
- Validation: reserved ids, duplicate names, unknown active layout, >32
  layouts, overlong names/descriptions, v1+v2 field mixing, invalid slots.
- D-Bus `SetLayout`: success, unknown id, `default`, persistence across
  restart, `LayoutChanged`.
- CLI: list/show/use/new/rename/delete/reset against an isolated bus.
- GUI: the layout chip/popover lists layouts; switching sends `SetLayout`;
  dirty switching is blocked; editor edits the active layout.
- GUI template: Default slots and hotspots open the editor without a lock
  banner; edits mark the document dirty and set the template dirty-banner
  text; Apply opens Save as layout; saving creates and activates a user layout
  with the staged map; Default still resolves to profile defaults; the manager
  tags Default `Template` and disables Rename/Delete/Reset; Revert discards the
  staged edits.
- GUI manager: New/Rename validate unique names; Duplicate, Delete, and Reset
  update the list; a dirty manager footer shows Revert/Apply; Apply with only
  layout-list changes persists them without opening Save as layout; the chip
  updates after a device switch (step 21).
- ✅ [hardware] Real RC003: two layouts with different actions for one button;
  switch and press the remote; the emitted key changes without a daemon
  restart; the active layout survives a restart. 2026-11-01, device
  `C0:5D:39:F8:2D:4C`: with `editor` active (`up/single = key:KEY_F13`),
  `evtest /dev/input/event26` recorded `code 183 (KEY_F13), value 1` then
  `value 0`; after `atvvctl layout use default`, the next press recorded
  `code 103 (KEY_UP), value 1` then `value 0`, with no daemon restart between
  the presses. After `atvvctl layout use editor` and a daemon restart,
  `atvvctl status` reported `layout.id=editor` and the input runtime
  reattached.

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
atvvctl layout new editor --name Editor
atvvctl config set --layout editor --button up --trigger single --action key:KEY_F13
atvvctl layout use editor
atvvctl status        # layout.id == "editor"
# press Up on the RC003; KEY_F13 is emitted
atvvctl layout use default
# press Up again; the default Up action is emitted
```

## Exit criteria

- ✅ A v1 file migrates without losing the old button map.
  `v1_button_map_migrates_to_the_custom_layout` loads a flat file, keeps the
  gesture override, saves v2 with `[layouts.custom.buttons.up]`, and reloads
  to an identical `Config`.
- ✅ `default` is a template: its slot edits can only be saved as a new user
  layout, and it cannot be renamed, deleted, or reset; user layouts can be
  created, renamed, deleted, and reset. `template_edits_are_saved_as_a_new_layout`,
  `the_template_cannot_be_renamed_deleted_or_reset`, and
  `manager_creates_renames_duplicates_deletes_and_resets` cover the model; the
  daemon rejects a stored `default` id and the CLI refuses the template.
- ✅ The active layout is persisted and shown in status. `SetLayout` writes
  `active_layout`, `StatusView.layout` reports `{id, name}`, and
  `set_layout_switches_persists_and_rejects_unknown_ids` reloads the file.
- ✅ [hardware] Switching applies live and changes the emitted key on the real
  RC003. `evtest` on `/dev/input/event26` recorded `KEY_F13` (code 183) on
  `editor` and `KEY_UP` (code 103) on `default` after `SetLayout`, without a
  daemon restart; the active layout also survived a restart.
- ✅ Invalid documents or unknown layouts leave the current state untouched.
  `set_config` and `set_layout` apply before writing and roll the runtime back
  on failure; the D-Bus test checks an unknown id and an invalid action leave
  the document and file unchanged.
- ✅ CLI, GUI, and D-Bus agree on the active layout. The isolated-bus CLI run
  below lists, creates, renames, resets, deletes, and switches layouts; the GUI
  reads and writes the same v2 document through `GetConfig`/`SetConfig` and
  `SetLayout`.
- ✅ No automatic per-app/focus switching is introduced; switching is explicit
  through `atvvctl layout use`, the GUI chip, or `SetLayout`.

## Risks

- Layout switching restarts the input runtime; a failed restart must roll
  back to the previous layout.
- A stale client that only knows v1 could erase layouts if it writes v2
  fields blindly; reject v1 writes and ship CLI/GUI together.
- Gesture timing per layout means a switch can change double/long behavior;
  show this in `layout show` and the GUI.
- Too many layouts make the selector unwieldy; cap at 32 and keep the
  manager searchable-free.
- A staged Default edit must never be written into the template or silently
  dropped: Apply always opens Save as layout for a changed slot map, and
  switching layouts or devices stays blocked until Apply or Revert.
- `exec:` policy is global by design; a layout must never enable it.

## Verification results

```text
cargo fmt --all -- --check                                   clean
cargo clippy --workspace --all-targets -- -D warnings        clean
cargo test --workspace                                       222 passed, 8 ignored
xvfb-run -a cargo test -p atvv-gui --test ui                 1 passed
```

Isolated-bus CLI run (`dbus-run-session`, `atvvd --mock`, scratch
`XDG_CONFIG_HOME`):

```text
atvvctl layout new editor --name Editor --from default   created layout editor (inactive)
atvvctl config set --layout editor --button up --trigger single --action key:KEY_F13
atvvctl layout show editor                               up single=key:KEY_F13, profile timing
atvvctl layout use editor                                active layout: editor
atvvctl status                                           layout.id=editor, layout.name=Editor
atvvctl map test --button up --trigger single            layout: editor, up single key:KEY_F13
atvvctl layout rename editor "Editor layout"             renamed layout editor
atvvctl layout delete editor                             refused: active layout
atvvctl layout use default                               active layout: default
atvvctl layout delete editor                             deleted layout editor
atvvctl config set --button up ...                       refused: default is the template
atvvctl layout reset player                              reset layout player
atvvctl config set (player active)                       up single=key:KEY_F14
atvvctl layout use default; map test                     up single=key:KEY_UP
```

Real RC003 (`C0:5D:39:F8:2D:4C`, 2026-11-01): the workspace daemon connected
(`connected to 小米蓝牙语音遥控器`), grabbed `/dev/input/event25`, and created
`atvv-virtual-keyboard`. `layout new editor`, `config set --layout editor`,
and `layout use editor` persisted
`active_layout = "editor"` and `[layouts.editor.buttons.up] single = "key:KEY_F13"`.

Recorded presses on `evtest /dev/input/event26`:

```text
editor:  Event: time 1793545618.757346, type 1 (EV_KEY), code 183 (KEY_F13), value 1
         Event: time 1793545618.757367, type 1 (EV_KEY), code 183 (KEY_F13), value 0
default: Event: time 1793545638.954774, type 1 (EV_KEY), code 103 (KEY_UP), value 1
         Event: time 1793545638.954792, type 1 (EV_KEY), code 103 (KEY_UP), value 0
```

`atvvctl layout use default` ran between the two presses; the daemon stayed
up. `atvvctl layout use editor` followed by a daemon restart kept
`layout.id=editor` and reattached `/dev/input/event25`. The installed `atvvd`
unit was restored afterwards.

## Progress log

- 2026-11-01 — plan reviewed; a `Next action` line was added and the hardware
  items tagged. No implementation started.
- 2026-11-01 — implemented schema v2 in `atvv-config`: `active_layout`,
  `[layouts.<id>]` records with raw slots and gesture overrides, the
  synthesized `default` template, reserved ids (`default`, `media`, `coding`,
  `slides`), the v1 `custom` migration, id/name/description/32-layout limits,
  and `SetConfig` v2-only validation. `default` is never stored, and empty
  legacy tables are omitted on save.
- 2026-11-01 — daemon: `StatusView.layout`, `SetLayout`, `LayoutChanged`,
  live apply and persistence with runtime rollback; `SetConfig` applies the
  active layout. CLI: `layout list|show|use|new|rename|delete|reset`,
  `config set --layout`, `map test --layout`, and v2 `config get`. GUI:
  layout chip/popover, manager dialog with inline forms, template staging and
  Save as layout, dirty switching block, and test-result reset on switch.
  Docs: configuration and GUI pages in English and Chinese.
- 2026-11-01 — synthetic verification is complete (see Verification
  results).
- 2026-11-01 — recorded the real RC003 hardware item: `KEY_F13` on `editor`,
  `KEY_UP` on `default`, and persistence across a daemon restart; the
  installed `atvvd` unit was restored. Step complete.
