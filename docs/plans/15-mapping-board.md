# Step 15 — SayAll-style mapping board

Status: ✅ done
Depends on: 14
Touches: `crates/atvv-gui/`, `docs/`, screenshots

## Goal

Replace the single-key inspector in `atvv-config` with a mapping board that
shows every logical button and its `single`/`double`/`long` actions at once:
a central RC003 diagram surrounded by per-button cards. Clicking a hotspot or
a card slot selects it; a modal action editor edits the selected slot. The app
remains GTK4 + libadwaita + GNOME HIG, uses the original vector diagram, and
does not copy the SayAll or Open Voice Bridge code, photos, or screenshots.

## Design reference

[`15-mapping-board.html`](15-mapping-board.html) is the interactive interface
reference for this step: the three-column board, per-button cards, modal action
editor, banners, test bar, and header device chip. It is a design artifact, not
shipped code; the implementation uses GTK4/libadwaita widgets and CSS classes.

The second remote in the device popover, its address, battery, and per-device
mappings preview step 21. The layout chip, popover, and manager preview step
17. This step renders the board and the device chip for the one active device;
it does not implement layout switching or per-device layouts.

References for the information architecture only:

- SayAll / `remote-mic-app` (GPL-3.0-only code; website design, photos, logo,
  and screenshots are not GPL): <https://sayall.app>
- Open Voice Bridge (GPL-3.0): <https://github.com/nijez/open-voice-bridge>

## Window shape

```text
AdwApplicationWindow
  AdwHeaderBar
    [menu] [device chip] [title/subtitle] [Test] [Revert] [Apply]
  banners (error / dirty / capture)
  GtkScrolledWindow
    GtkBox (horizontal, board)
      left card column: Power, Up, Left, Back, Home, Menu
      center column: RemoteDiagram (original SVG) + voice card
      right card column: Right, OK, Down, Volume +, Volume -, TV
  bottom bar: test state, last resolved action, Run once
  AdwToastOverlay
```

The board scrolls horizontally at narrow widths; the header and test bar stay
visible. The action editor and recording-key editor are `AdwDialog`s, not a
permanent pane. Hardware facts moved from the old sidebar into a read-only
device-information dialog opened by the header chip and the menu.

## Deliverables

- ✅ `MappingBoard` widget (`crates/atvv-gui/src/board.rs`):
  - Twelve `KeyCard`s, each with the button label and three real `GtkButton`
    slots (`Single`, `Double`, `Long`).
  - Slot labels show the resolved action in a compact form (`Return`, `F13`,
    `Ctrl+Alt+V`, `Command…`, `Start dictation`, `None`); unit tests cover the
    formatter in `model::action_label`.
  - Selected card/slot CSS uses the system accent color; hotspot selection and
    card selection stay in sync through `GuiModel.selected`/`trigger`.
  - Clicking a hotspot selects the card and its `single` slot; clicking a card
    slot selects that exact slot. Both open the editor for the slot.
  - The voice key is drawn and opens the recording-key dialog; it is not a
    card for action slots. The center voice card explains that it drives the
    ATVV microphone and shows the output key.
  - No card-connector arrows; adjacency and selection carry the mapping.
- ✅ Action editor dialog (`crates/atvv-gui/src/editor.rs`):
  - `AdwDialog` with type/value/learn-key/exec/dictation fields, inline
    validation, Reset, Cancel, and Update.
  - `exec` remains opt-in; the confirmation and `allow_exec` switch live in
    the dialog.
  - Keyboard key learning and remote-button capture keep the step-14 behavior
    (`Learn remote…` retargets the open dialog to the physical slot); no second
    evdev grab.
- ✅ Test mode:
  - A physical press selects and highlights the matching card/slot.
  - The bottom bar shows the resolved action and the explicit Run once.
  - `exec` still requires a separate confirmation.
- ✅ Save/revert:
  - Header Apply uses `SetConfig` with the same atomic validation and
    persistence; Revert reloads. No direct file writes.
- ✅ Accessibility and HIG:
  - Every card slot is a focusable button with an accessible name and tooltip.
  - Tab order follows the board; the modal dialogs trap focus and return it.
  - System light/dark theme; no hard-coded palette beyond key-state CSS.
  - Cards and the diagram scale/scroll without losing alignment; the diagram
    minimum shrank so the board fits 1040×760 without scrolling.
- ✅ Tests:
  - Unit tests for action display labels, trigger names, and the card/slot
    column mapping.
  - UI smoke: twelve cards × three slots exist and are allocated, hotspot and
    card selection update the model, the dialog edits the correct slot,
    invalid input leaves the old map, and test mode selects the pressed slot.
  - XTest pointer clicks on every card slot (36) and hotspot (13) under
    `xvfb-run`; each click asserts the model selection and dialog target.
  - [hardware] Real RC003-profile save/persistence check after the layout
    change: the workspace GUI saved `up/double = key:KEY_F13` through the board
    and dialog to the live `atvvd`, and `atvvctl config get --json` returned the
    same value before and after `systemctl --user restart atvvd`. The RC003
    itself was disconnected during the check, so no RF press was involved; the
    mapping was restored with `--action default`.
- ✅ Docs and screenshots:
  - `docs/gui.md` and `docs/gui.zh-CN.md` describe the board, editor dialog,
    device chip, recording-key dialog, and test bar.
  - `docs/gui.png` (board), `docs/gui-editor.png` (editor), and
    `docs/gui-recording-key.png` (recording key) are captured from the built
    app at 1040×760 in the light theme.
- ✅ Packaging: the split VCS package `atvv-git 0.1.0.r9.g…` /
  `atvv-gui-git 0.1.0.r9.g…` built from this commit; GTK dependencies remain
  only in `atvv-gui-git`. The published recipe picks the commit up when the
  branch is pushed.

## Out of scope

- Copying SayAll/Open Voice Bridge SwiftUI layout, photos, or assets.
- Per-application mapping layers or shortcut discovery.
- A new toolkit or a web/TUI front end.
- Layout selection and management (step 17); the HTML reference previews the
  chip, popover, and manager.
- Multi-device switching, the known-device popover, or per-device layouts
  (step 21); the HTML reference previews them.
- Daemon protocol or `RunAction` changes unless the board exposes a missing
  action operation.

## Verification

Run from the repository root on 2026-11-01:

```text
cargo fmt --all -- --check                              # clean
cargo clippy --workspace --all-targets -- -D warnings   # clean
cargo test --workspace                                  # 198 passed, 5 ignored
GDK_BACKEND=x11 xvfb-run -a -s "-screen 0 1280x1024x24" \
  cargo test -p atvv-gui --test ui                      # 1 passed (36+13 clicks)
GDK_BACKEND=x11 xvfb-run -a -s "-screen 0 1280x1024x24" \
  cargo test -p atvv-gui --test i18n                    # 1 passed, 1 ignored
GDK_BACKEND=x11 xvfb-run -a -s "-screen 0 1280x1024x24" \
  cargo test -p atvv-gui --test real_run -- --ignored   # 1 passed (live atvvd)
cargo run -q -p atvv-gui                                # live window, screenshots
```

The XTest section requires the X11 GDK backend and an X screen at least as
large as the window; it skips with a message otherwise. The recorded run used
a 1280×1024 Xvfb screen.

Manual:

1. Open `atvv-config` with `atvvd` running: the board shows twelve cards and
   the center remote; the voice card shows `DICTATE`.
2. Click a card slot and a remote hotspot; the same card/slot is selected and
   the dialog title follows.
3. Edit a key, combo, and dictation action; invalid input shows an inline
   error and leaves the old map.
4. Apply; `atvvctl config get --json` shows the value, and it survives an
   `atvvd` restart.
5. Test mode: a press selects and highlights the slot; Run once is explicit.
6. Narrow and wide windows, light and dark themes: the board scrolls
   horizontally when needed and hotspots stay aligned.

## Exit criteria

- ✅ All twelve buttons show three slots with their current actions.
- ✅ Hotspot and card selection stay in sync and edit the correct slot.
- ✅ The action editor is modal, keyboard accessible, and validates inline.
- ✅ Test mode highlights the pressed card slot and Run once is explicit.
- ✅ The GTK board and device chip match the reference's board portion at
  1040×760; differences are recorded in `docs/gui.md`: no layout chip (step
  17), the device chip opens device information instead of a device switcher
  (step 21), and the editor omits the reference's raw-value preview line.
- ✅ Apply/Revert remain atomic; invalid input leaves the old map active.
- ✅ No SayAll/Open Voice Bridge code, photo, or screenshot is copied.
- ✅ The board is usable at 1040×760 and at a narrow window via scrolling.
- ✅ The current `-git` packages contain the new GUI and keep GTK out of
  `atvv-git`.

## Risks

- A board with twelve cards is wide; horizontal scrolling must not hide the
  selected card without scrolling it into view. The board minimum is 860 px
  and selection calls `scroll_into_view`.
- Three slot buttons per card can become dense; the slots use 10 px gesture
  labels and 12 px action labels, verified in light and dark themes.
- Reusing the step-14 editor in a dialog can drift from the tested model
  logic; the dialog composes `ActionDraft` and writes through
  `GuiModel::set_slot_raw`, so validation is shared.
- Connector arrows would couple card coordinates to the SVG; skipped.

## Progress log

- 2026-10-31 — planned after reviewing SayAll/`remote-mic-app` and Open Voice
  Bridge. Step 14 keeps the split view and finishes first; this step is the
  sayall-style board.
- 2026-11-01 — implemented the three-column `MappingBoard` with twelve
  `KeyCard`s and 36 slot buttons, the shared `GuiModel` selection, the compact
  action labels (`Return`, `F13`, `Ctrl+Alt+V`, `Command…`, …), the modal
  action editor and recording-key editor, the bottom test bar, and the header
  device chip with a read-only device-information dialog. The old
  `NavigationSplitView` sidebar and its persistent editor were removed.
- 2026-11-01 — the first board pass forced a vertical scrollbar because the
  diagram's 200 px minimum width implied a 594 px height; the minimum is now
  130 px (386 px tall), so the board fits 1040×760 and the voice card stays
  visible.
- 2026-11-01 — XTest clicks initially missed the hotspots because the board
  was scrolled; after the size fix all 36 slot buttons and 13 hotspots received
  real pointer clicks and updated the model. `compute_bounds` against the
  toplevel is used for click coordinates.
- 2026-11-01 — unit tests for `action_label`/friendly key names, trigger
  names, card columns, and editor hints; the UI test covers card/slot
  existence, selection sync, inline validation, conflicts, the voice dialog,
  `exec` gating, and test-mode selection.
- 2026-11-01 — real-daemon check against `atvv-git 0.1.0.r8.gfcd422b-1` on the
  X11 backend: saved `up/double = key:KEY_F13` through the board + dialog,
  read it back through `atvvctl config get --json`, restarted `atvvd`, read it
  again, then restored the slot to the profile default. The RC003 was
  disconnected; no RF press was part of the save/persistence check.
- 2026-11-01 — refreshed `docs/gui.md`/`gui.zh-CN.md` and the three
  screenshots; regenerated `THIRD-PARTY-NOTICES.md` for the `x11rb` test
  dependency.
- 2026-11-01 — built the split VCS packages from this commit; `atvv-gui-git`
  carries `gtk4`/`libadwaita`/`librsvg` and `atvv-git` stays GTK-free.
