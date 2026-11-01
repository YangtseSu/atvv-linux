# Step 18 — Layout presets

Status: ✅ done
Depends on: 17
Touches: `crates/atvv-config/`, `crates/atvv-cli/`, `crates/atvv-gui/`,
`docs/`

## Goal

Ship a fixed catalog of app-verified, read-only layout presets that a user can
preview and turn into a user layout. Presets are templates, not live state:
the active layout is still a layout from step 17, and `default` remains the
synthesized profile template. See
[ADR 0010](../decisions/0010-app-verified-presets.md) for why the catalog is
app-verified instead of generic.

## Presets

Initial catalog (static constants in `atvv-config`, no `exec:`, no
`dictation:`, no `none`):

| id | Name | Target | Verified with |
|---|---|---|---|
| `mpv` | mpv | Media playback | mpv 0.41.0 default bindings |
| `slides` | Slides | LibreOffice Impress and WPS Presentation | Impress 26.8.1, WPS Office 12.1.2 |
| `omp` | omp | Coding-agent session control | omp 18.14.2 default bindings |

Every preset maps all twelve action-mapped buttons on its single-press slot:

| Button | `mpv` | `slides` | `omp` |
|---|---|---|---|
| Power | `key:KEY_Q` (quit) | `key:KEY_END` (last slide) | `combo:alt+r` (retry) |
| OK | `key:KEY_SPACE` (pause) | `key:KEY_SPACE` (next) | `key:KEY_ENTER` (submit) |
| Back | `key:KEY_ESC` (leave fullscreen) | `key:KEY_ESC` (end show) | `key:KEY_ESC` (interrupt) |
| Up | `key:KEY_UP` (seek +60 s) | `key:KEY_PAGEUP` | `key:KEY_UP` |
| Down | `key:KEY_DOWN` (seek −60 s) | `key:KEY_PAGEDOWN` | `key:KEY_DOWN` |
| Left | `key:KEY_LEFT` (seek −5 s) | `key:KEY_LEFT` | `key:KEY_LEFT` |
| Right | `key:KEY_RIGHT` (seek +5 s) | `key:KEY_RIGHT` | `key:KEY_RIGHT` |
| Home | `key:KEY_HOME` (start) | `key:KEY_HOME` (first slide) | `combo:ctrl+r` (history) |
| Menu | `key:KEY_M` (mute) | `key:KEY_B` (black screen) | `combo:shift+tab` (thinking) |
| TV | `key:KEY_F` (fullscreen) | `key:KEY_F5` (start show) | `combo:ctrl+o` (tool output) |
| Volume up | `key:KEY_VOLUMEUP` | `key:KEY_VOLUMEUP` | `key:KEY_VOLUMEUP` |
| Volume down | `key:KEY_VOLUMEDOWN` | `key:KEY_VOLUMEDOWN` | `key:KEY_VOLUMEDOWN` |

The `omp` mapping avoids `Space` (omp push-to-talk) and `Ctrl+L` (omp live
voice), so the ATVV recording key stays the only dictation path. `Alt+R` is
omp's retry binding; `F5` is the other default but collides with the profile's
physical voice key, so the preset uses `Alt+R`.

## Data model

- `LayoutPreset { id, name, description, verified_with, slots, long_press_ms }`
  is a static catalog in `atvv-config`; `LAYOUT_PRESETS` lists the three and
  `layout_preset(id)` resolves one. The GUI and CLI read the same constants.
- Preset ids are reserved: `default`, `mpv`, `slides`, `omp`. The v1 migration
  and user layouts cannot take them. `media` and `coding` are no longer
  reserved because no such presets exist.
- `LayoutPreset::layout()` builds a `LayoutDocument` with one single-press
  action per logical button and no double/long overrides.
- `LayoutPreset::apply(document, id, name, profile)` validates the id, the
  free slot, the layout count, and the resulting document, then inserts an
  inactive layout. It leaves `default`, `active_layout`, `allow_exec`,
  `vinput`, `trigger_key`, `device`, and `gain_db` untouched.
- `LayoutPreset::reset_to(document, id, profile)` replaces a user layout's
  buttons and gesture timing, keeping its name and description and the active
  layout. `default` and unknown ids are rejected.

## CLI

```text
atvvctl layout preset list [--json]
atvvctl layout preset show <preset> [--json]
atvvctl layout preset apply <preset> [--as <id>] [--name <name>] [--activate]
atvvctl layout reset <id> --preset <preset>
```

`apply` defaults the new id and name to the preset (`mpv` becomes the
`mpv-layout` id because `mpv` is reserved). It creates the layout inactive
unless `--activate` is given. `layout reset <id>` without `--preset` still
clears the layout; with `--preset` it replaces the slots and keeps the name
and description. Both commands persist through `SetConfig`, so the daemon
validates and applies the document atomically.

## GUI

- The layout manager footer has **New from preset…**. It opens a dialog that
  lists the three presets with their display name, translated description,
  and `verified_with`; selecting one previews every button's action
  (`Alt+R`, `Ctrl+O`, `Page Up`, …) and pre-fills a unique name.
- **Create** stages the new layout as inactive; the board stays on the
  current layout and the document is dirty until **Apply**. The user chooses
  **Use** afterwards.
- Each user layout's trailing menu has **Reset to preset…**. It opens the same
  picker with the affected layout named in the warning; **Reset** asks for
  confirmation and replaces the slots while keeping the layout name and
  description. `Default` does not offer it.
- Preset rows are never edited or deleted; they are templates. The dialog is
  localized in English and Simplified Chinese; app names (`mpv`, `omp`) stay
  untranslated.

## Tests

- `preset_catalog_covers_every_action_button_without_none` — every preset has
  a valid id, unique reserved id, non-empty name/description/verification,
  and exactly the twelve action buttons, each with a parseable key or chord
  action and no `none`.
- `preset_catalog_matches_the_reviewed_snapshot` — a line-for-line snapshot so
  an accidental action change is visible in review.
- `applying_a_preset_creates_an_inactive_valid_layout`,
  `preset_apply_rejects_reserved_and_existing_ids`,
  `preset_reset_replaces_slots_and_keeps_identity`,
  `preset_reset_rejects_template_and_unknown_layouts` — apply/reset behavior.
- `preset_creation_stages_an_inactive_layout`,
  `preset_reset_keeps_the_layout_name_and_active_state` — GUI model behavior.
- The GUI smoke test opens **New from preset…**, selects omp, asserts the
  preview (`Alt+R`, `Ctrl+O`), creates `omp-layout`, verifies the active
  layout did not change, then opens **Reset to preset…** and asserts the
  warning names the affected layout and the preview updates to mpv.

## Verification

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace                         # 230 passed, 8 ignored
GDK_BACKEND=x11 xvfb-run -a -s "-screen 0 1280x1024x24" \
  cargo test -p atvv-gui --test ui             # 1 passed
```

CLI smoke on a private bus with `atvvd --mock` and a temporary
`XDG_CONFIG_HOME`:

```text
atvvctl layout preset list                     # mpv, slides, omp
atvvctl layout preset show omp                 # twelve single slots
atvvctl layout preset apply mpv                # created mpv-layout (inactive)
atvvctl layout preset apply omp --as agent --activate
atvvctl layout reset mpv-layout --preset slides
atvvctl layout list                            # agent active, mpv-layout reset
```

The persisted `daemon.toml` held `active_layout = "agent"`, the `agent`
layout with omp's slots, and `mpv-layout` with the slides slots under its
original name and description.

Real-app runs (2026-11-02):

- **mpv 0.41.0**, `--no-config --vo=x11 --ao=null`, keys sent with XTest to
  the focused window and state read through `--input-ipc-server`: Space
  changed `pause` false→true, `f` changed `fullscreen` false→true, Esc
  changed it back, `m` changed `mute` false→true, Right moved `time-pos`
  2.9→26.0 s (keyframe-limited seek), and Home moved it to 1.0 s.
- **LibreOffice Impress 26.8.1** under Xvfb with a pandoc-generated three-slide
  PPTX: F5 showed the title slide; Down/Down advanced to SLIDE ONE and SLIDE
  TWO; Up returned to SLIDE ONE; Right advanced to SLIDE TWO; Left returned
  to SLIDE ONE; End showed SLIDE THREE; Home returned to the title slide;
  `b` produced a black screen (mean luminance 0.0); Esc ended the show and
  left only the editor window.
- **WPS Presentation 12.1.2** (`wpp`) under Xvfb with the same PPTX: F5
  showed the title slide; Space advanced to SLIDE ONE; Down advanced to
  SLIDE TWO; Up returned to SLIDE ONE; Right advanced to SLIDE TWO; Left
  returned to SLIDE ONE; End showed SLIDE THREE; Home returned to the title
  slide; `b` produced a black screen (mean luminance 0.0); Esc ended the show.
- **omp 18.14.2** in tmux with `--no-session`: Shift+Tab cycled the thinking
  indicator `⟳`→`◔`→`◒`; Ctrl+R opened the history search overlay and Esc
  closed it; Enter submitted a prompt that ran a bash tool; Esc aborted the
  running tool ("Command aborted", "F5 to retry"); Alt+R retried the turn.
  Ctrl+O is omp's documented `app.tools.expand` binding and is included but
  was not distinguished visually; the preset description names the version.
- Volume up/down are bound to `KEY_VOLUMEUP`/`KEY_VOLUMEDOWN` in every preset
  and are covered by the input-path tests; they are system actions, not app
  actions. mpv's `q` (Power) is its default quit binding; the Xvfb run did
  not observe the process exit, so that slot rests on the default binding and
  is documented as such.

## Exit criteria

- ✅ Three presets are listed and previewable.
- ✅ Applying a preset creates a valid user layout and never edits `default`.
- ✅ Presets cannot be edited, deleted, or used as active ids.
- ✅ No preset contains `exec:`, `dictation:`, `none`, or changes the exec
  policy.
- ✅ CLI and GUI apply the same catalog data.
- ✅ Every preset action is covered by a grammar test.
- ✅ Docs describe the presets and the fact that they are templates.

## Risks

- App bindings drift across versions. Every preset names its verified version;
  a major update requires a new recorded run.
- The slides preset names two apps. They share the standard presentation keys
  today; if a future version diverges, split the preset instead of guessing.
- omp user remaps (`~/.omp/agent/keybindings.yml`) and Vim editing mode can
  change the mapped keys. The preset is verified against the default bindings;
  the docs state this.
- mpv user `input.conf` files can override the defaults. The preset follows
  mpv's default bindings.
- The catalog stays small on purpose. Adding an app needs a verification run
  and a plan update, not just another table row.
- Preset ids are reserved, so a layout created with the default name gets a
  `-layout` id (`mpv-layout`). This is deliberate; `layout show mpv` remains a
  user-layout lookup.
- `--activate` switches to the created user layout, never to a preset; a
  missing runtime apply leaves the previous layout active through the
  existing `SetConfig` path.

## Progress log

- 2026-11-01 — plan reviewed; the generic `media`/`coding`/`slides` draft and
  its `Super+L` lock-screen binding were identified as unshippable.
- 2026-11-02 — ADR 0010 accepted: app-verified presets. Implemented the static
  catalog, apply/reset helpers, reserved ids, CLI commands, GUI preset dialog
  and reset flow, English and Chinese strings, and the tests above.
- 2026-11-02 — CLI smoke on an isolated bus passed; real-app runs recorded
  for mpv 0.41.0, Impress 26.8.1, WPS 12.1.2, and omp 18.14.2.
