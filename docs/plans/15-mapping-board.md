# Step 15 — SayAll-style mapping board

Status: ⬜ not-started
Depends on: 14
Touches: `crates/atvv-gui/`, `docs/`, screenshots

## Goal

Replace the single-key inspector in `atvv-config` with a mapping board that
shows every logical button and its `single`/`double`/`long` actions at once:
a central RC003 diagram surrounded by per-button cards. Clicking a hotspot or
a card slot selects it; a modal action editor edits the selected slot. The app
remains GTK4 + libadwaita + GNOME HIG, uses the original vector diagram, and
does not copy the SayAll or Open Voice Bridge code, photos, or screenshots.

References for the information architecture only:

- SayAll / `remote-mic-app` (GPL-3.0-only code; website design, photos, logo,
  and screenshots are not GPL): <https://sayall.app>
- Open Voice Bridge (GPL-3.0): <https://github.com/nijez/open-voice-bridge>

## Window shape

```text
AdwApplicationWindow
  AdwHeaderBar
    [menu] [device status] [Test] [Revert] [Apply]
  banners (error / dirty / capture)
  GtkScrolledWindow
    GtkBox (horizontal, board)
      left card column: Power, Up, Left, Back, Home, Menu
      center column: RemoteDiagram (original SVG) + voice-key note
      right card column: Right, OK, Down, Volume +, Volume -, TV
  bottom bar: last test result + Run once
  AdwToastOverlay
```

The board scrolls horizontally at narrow widths; the header and test bar stay
visible. The action editor is an `AdwDialog`, not a permanent pane.

## Deliverables

- ⬜ `MappingBoard` widget or module:
  - Twelve `KeyCard`s, each with the button label and three real `GtkButton`
    slots (`Single`, `Double`, `Long`).
  - Slot labels show the resolved action in a compact form (`Return`,
    `F13`, `Ctrl+Alt+V`, `Dictation`, `Command…`, `None`).
  - Selected card/slot CSS uses the system accent color; hotspot selection
    and card selection stay in sync.
  - Clicking a hotspot selects the card and its `single` slot; clicking a card
    slot selects that exact slot.
  - The voice key is drawn but is not a card or hotspot; a note explains that
    it drives the ATVV microphone.
  - No card-connector arrows in the first cut; adjacency and selection carry
    the mapping. Add lines later only if they can be drawn without fragile
    coordinate coupling.
- ⬜ Action editor dialog:
  - `AdwDialog` with the existing type/value/learn-key/exec/dictation fields,
    inline validation, Reset, Cancel, and Update.
  - `exec` remains opt-in; the confirmation and `allow_exec` switch are
    available from the dialog or a small Advanced dialog.
  - Keyboard key learning and remote-button capture keep the same behavior as
    step 14; no second evdev grab.
- ⬜ Test mode:
  - A physical press selects the matching card/slot and highlights it.
  - The bottom bar shows the resolved action and the explicit Run once.
  - `exec` still requires a separate confirmation.
- ⬜ Save/revert:
  - The header Apply uses `SetConfig` with the same atomic validation and
    persistence; Revert reloads. No direct file writes.
- ⬜ Accessibility and HIG:
  - Every card slot is a focusable button with an accessible name and tooltip.
  - Tab order follows the visual board; the dialog traps focus and restores it.
  - System light/dark theme; no hard-coded palette beyond key-state CSS.
  - Cards and the diagram scale/scroll without losing alignment.
- ⬜ Tests:
  - Unit tests for action display labels and card/slot mapping.
  - UI smoke: twelve cards × three slots exist, hotspot and card selection
    update the model, the dialog edits the correct slot, invalid input leaves
    the old map, test mode selects the pressed slot.
  - XTest pointer clicks on every card slot and hotspot under `xvfb-run`.
  - Real RC003 save/persistence check after the layout change.
- ⬜ Docs and screenshot:
  - Update `docs/gui.md` with the board, editor dialog, and test bar.
  - Replace `docs/gui.png` with the board screenshot.
- ⬜ Packaging: rebuild the split `atvv-linux-gui` package; GTK dependencies
  remain out of `atvv-linux`.

## Out of scope

- Copying SayAll/Open Voice Bridge SwiftUI layout, photos, or assets.
- Per-application mapping layers or shortcut discovery.
- A new toolkit or a web/TUI front end.
- Daemon protocol or `RunAction` changes unless the board exposes a missing
  action operation.

## Verification

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
xvfb-run -a cargo test -p atvv-gui --test ui
cargo run -q -p atvv-gui
```

Manual:

```text
1. Open atvv-config with atvvd running.
2. Click a card slot and a remote hotspot; the same card/slot is selected.
3. Edit a key, combo, dictation, and exec action; verify validation.
4. Apply; restart atvvd; verify the mapping persists.
5. Enable Test and press a physical remote key; verify the card slot and
   bottom bar show the right action; run it once.
6. Resize narrow/wide and switch light/dark; verify scrolling, focus, and
   hotspot/card alignment.
```

## Exit criteria

- ⬜ All twelve buttons show three slots with their current actions.
- ⬜ Hotspot and card selection stay in sync and edit the correct slot.
- ⬜ The action editor is modal, keyboard accessible, and validates inline.
- ⬜ Test mode highlights the pressed card slot and Run once is explicit.
- ⬜ Apply/Revert remain atomic; invalid input leaves the old map active.
- ⬜ No SayAll/Open Voice Bridge code, photo, or screenshot is copied.
- ⬜ The board is usable at 1040×760 and at a narrow window via scrolling.
- ⬜ `0.1.0-8` (or the next package revision) contains the new GUI and keeps
  GTK out of `atvv-linux`.

## Risks

- A board with twelve cards is wide; horizontal scrolling must not hide the
  selected card without scrolling it into view.
- Three slot buttons per card can become dense; labels must stay readable in
  light/dark themes and at 1.25×/1.5× scaling.
- Reusing the step-14 editor in a dialog can drift from the tested model
  logic; share `ActionDraft`/`GuiModel`, do not duplicate validation.
- Connector arrows couple card coordinates to the SVG; skip them in the first
  cut rather than adding a fragile geometry layer.

## Progress log

- 2026-10-31 — planned after reviewing SayAll/`remote-mic-app` and Open Voice
  Bridge. Step 14 keeps the split view and finishes first; this step is the
  sayall-style board.
