# Step 18 — Layout presets

Status: ⬜ not-started
Depends on: 17
Touches: `crates/atvv-config/`, `crates/atvv-cli/`, `crates/atvv-gui/`,
`docs/`

## Goal

Ship a small catalog of built-in, read-only layout presets that a user can
preview and turn into a user layout. Presets are templates, not live state:
the active layout is still a layout from step 17, and `default` remains the
synthesized profile template.

## Presets

Initial catalog (generic, no `exec:`, no app detection):

| id | Name | Intent |
|---|---|---|
| `media` | Media | Television/player navigation; Back sends Escape |
| `coding` | Coding | Editor/browser shortcuts around the D-pad |
| `slides` | Slides | Page Up/Down on the vertical axis for presentations |

Draft actions; every entry is validated by the action grammar in tests:

```text
media:
  ok          single key:KEY_ENTER
  back        single key:KEY_ESC
  up/down     single key:KEY_UP/KEY_DOWN
  left/right  single key:KEY_LEFT/KEY_RIGHT
  home        single key:KEY_HOMEPAGE
  menu        single key:KEY_MENU
  tv          single key:KEY_GRAVE
  volume_up   single key:KEY_VOLUMEUP
  volume_down single key:KEY_VOLUMEDOWN
  power       single none

coding:
  ok          single key:KEY_ENTER
  back        single key:KEY_ESC
  up/down     single key:KEY_UP/KEY_DOWN
  left/right  single key:KEY_LEFT/KEY_RIGHT
  home        single combo:super+l
  menu        single combo:super+i
  volume_up   single combo:ctrl+tab
  volume_down single combo:ctrl+shift+tab
  tv          single key:KEY_GRAVE
  power       single none
  long_press_ms = 600

slides:
  up          single key:KEY_PAGEUP
  down        single key:KEY_PAGEDOWN
  left/right  single key:KEY_LEFT/KEY_RIGHT
  ok          single key:KEY_ENTER
  back        single key:KEY_ESC
  home        single key:KEY_HOME
  menu        single key:KEY_B
  tv          single key:KEY_PERIOD
  volume_up   single key:KEY_VOLUMEUP
  volume_down single key:KEY_VOLUMEDOWN
  power       single none
```

`super` and `ctrl` are the existing key aliases; the GUI shows the platform
spelling. Presets deliberately avoid `exec:`, `dictation:`, unknown keys, and
dangerous system shortcuts. A user can edit the resulting layout freely.

## Data model

- `LayoutPreset { id, name, description, buttons, gesture }` is a static
  `const`/`static` catalog in `atvv-config`, next to the action grammar.
- Preset ids share the layout id rules and are reserved: a user layout may
  not use a preset id. `default` remains reserved separately.
- A preset is never stored in `daemon.toml` and is never the active layout.
- Applying a preset creates a user layout with the preset's buttons and
  gesture overrides; the name defaults to the preset name but can be
  overridden. Applying to an existing id is rejected unless the user
  explicitly chooses "reset to preset" for a user layout.
- Applying never changes `allow_exec`, `vinput`, `trigger_key`, `device`, or
  `gain_db`.

## CLI

- `atvvctl layout preset list` — id, name, description.
- `atvvctl layout preset show <preset>` — every slot in the action grammar.
- `atvvctl layout preset apply <preset> --as <id> [--name <name>]` — creates
  a user layout; the active layout is unchanged unless `--activate` is
  explicit.
- `atvvctl layout reset <id> --preset <preset>` — replaces a user layout's
  buttons/gesture with the preset after confirmation; `default` is rejected.

## GUI

- The layout manager (step 17) offers **New from preset…** and
  **Reset to preset…**.
- The picker lists presets with name and description; selecting one shows a
  read-only preview of all slots before creating the layout.
- Creating from a preset does not switch the active layout automatically; the
  user chooses **Use** afterwards. Reset to preset requires an explicit
  confirmation and affects only the selected user layout.
- Preset rows are never edited or deleted in place; like `default`, a preset
  is a template that produces a user layout.

## Tests

- Every preset id is unique and reserved; every action parses; no preset
  contains `exec:` or `dictation:`; no preset enables `allow_exec`.
- Applying each preset produces a v2 document that passes validation and has
  the expected slots.
- CLI apply/reset behavior, including id collision and `default` rejection.
- GUI picker and preview; applying from a preset leaves the active layout
  alone until **Use**.
- Snapshot-style assertions for the preset catalog so an accidental action
  change is visible in review.

## Verification

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
xvfb-run -a cargo test -p atvv-gui --test ui
```

```text
atvvctl layout preset list
atvvctl layout preset show coding
atvvctl layout preset apply coding --as coding
atvvctl layout use coding
atvvctl config get --json | jq '.layouts.coding'
```

## Exit criteria

- ⬜ Three presets are listed and previewable.
- ⬜ Applying a preset creates a valid user layout and never edits
  `default`.
- ⬜ Presets cannot be edited, deleted, or used as active ids.
- ⬜ No preset contains `exec:`, `dictation:`, or changes the exec policy.
- ⬜ CLI and GUI apply the same catalog data.
- ⬜ Every preset action is covered by a grammar test.
- ⬜ Docs describe the presets and the fact that they are templates.

## Risks

- Presets are opinionated; keep them generic and easy to edit, and avoid
  app-specific actions (Codex/voxtype) that belong to consumers.
- A preset with a platform-specific chord may not suit every desktop; the
  preview and per-slot edit path are the mitigation.
- Preset ids colliding with user layout ids would be confusing; reserve them.
- A future preset schema change must keep old user layouts readable; presets
  are code constants and never stored, so only layout data matters.
