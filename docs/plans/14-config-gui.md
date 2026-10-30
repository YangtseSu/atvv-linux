# Step 14 — GTK4/libadwaita configuration app

Status: ⬜ not-started
Depends on: 13
Touches: `crates/atvv-gui/` (new), `Cargo.toml`, `packaging/`, `docs/`

## Goal

A native GNOME-style configuration app that shows the remote, lets the user
click a key, edit its actions, learn a physical key, test mappings, and save
through the daemon. The GUI is a separate user process; it does not grab
evdev, touch BlueZ/PipeWire directly, or write `daemon.toml`.

The toolkit decision is fixed: **Rust + `gtk4-rs` + `libadwaita`, following
GNOME HIG**. See [ADR 0008](../decisions/0008-gui-toolkit.md).

## Window shape

```text
AdwApplicationWindow
  AdwHeaderBar
    [Device status] [Test mode] [Apply/Revert]
  AdwNavigationSplitView
    sidebar / main:
      remote diagram (GtkPicture + GtkOverlay/GtkFixed hotspots)
    detail:
      AdwPreferencesPage
        selected button
        single / double / long action editor
        test action
      AdwBanner (errors, unsaved changes, capture mode)
  AdwToastOverlay
```

## Deliverables

- ⬜ `atvv-gui` crate and `atvv-config` binary. The GUI is optional; `atvvd`
  and `atvvctl` work without it.
- ⬜ GTK4 + libadwaita application:
  - `AdwApplication`, `AdwApplicationWindow`, `AdwHeaderBar`,
    `AdwNavigationSplitView`, `AdwPreferencesPage`, `AdwPreferencesGroup`,
    `AdwActionRow`/`AdwEntryRow`/`AdwComboRow`, `AdwBanner`, `AdwToast`,
    `AdwAboutDialog`.
  - GNOME HIG spacing, typography, focus rings, keyboard navigation, adaptive
    narrow layout, light/dark system theme.
- ⬜ Remote diagram:
  - SVG remote asset loaded with `GtkPicture`.
  - `GtkOverlay`/`GtkFixed` with one real `GtkButton` hotspot per logical
    button; accessible label and tooltip.
  - CSS states: normal, hover, focus, selected, bound, unbound, conflict.
  - Scales with the window without losing hotspot alignment.
  - Click selects the key; no custom hit-testing if a button overlay works.
- ⬜ Action editor:
  - Action type: `none`, `key`, `combo`, `exec`, `dictation`.
  - Trigger slot: single, double, long; profile-driven availability.
  - “Learn key” captures a keyboard key/chord.
  - “Learn button” calls `BeginCapture` and highlights the physical remote key
    as it is pressed; no second evdev grab.
  - `exec` editor shows a warning and is disabled unless `allow_exec` is on.
  - Validation errors inline; no silent fallback.
- ⬜ Test mode:
  - Press a remote button and highlight it.
  - Show the action that would run.
  - “Run once” is explicit; `exec` requires a separate confirmation.
- ⬜ Save/revert:
  - Dirty state, Apply via `SetConfig`, Revert, daemon-not-running banner.
  - Success toast; no direct file writes.
- ⬜ Packaging:
  - GUI package or split package (`atvv-linux-gui`) with `gtk4`,
    `libadwaita`, `librsvg`; the daemon package must not gain GTK
    dependencies.
  - `.desktop` file, icon, AppStream metadata, About dialog.
  - No root service; user session only.
- ⬜ Tests:
  - Config/action editor unit tests.
  - D-Bus mock for load/save/capture.
  - Headless GTK smoke under `xvfb-run` (window opens, hotspots exist,
    selection updates model).
  - Manual real RC003 run: click key, set action, save, press remote, verify.
- ⬜ Docs: installation, keymap editing, test mode, script safety, screenshot.

## GNOME HIG checklist

- Use libadwaita widgets; do not reimplement standard controls.
- Header bar, navigation split view, preferences groups, banners, toasts.
- Visible focus, keyboard operation, accessible names for hotspot buttons.
- System light/dark theme; no hard-coded palette except key-state CSS.
- Adaptive layout at narrow widths; no fixed window size assumptions.
- Empty/error/daemon-stopped states are explicit; no silent failure.
- `exec` warnings and destructive actions require explicit confirmation.

## Design notes

- The GUI is a D-Bus client only. It never opens `/dev/input/*`, `/dev/uinput`,
  BlueZ, or PipeWire.
- The daemon owns capture mode, button events, the action map, and `exec`.
  The GUI renders and edits.
- The remote diagram uses real buttons for accessibility and focus; do not
  make the whole SVG one custom widget.
- The GUI does not write consumer configuration. It edits only ATVV's own
  config through the daemon.
- The daemon remains usable without a GUI; the GUI can show a read-only
  daemon-stopped state.
- Per-application focus detection is not part of this step. Use explicit
  layers/profiles later if users need them.
- A separate GUI package keeps GTK/Vulkan/WebKit-class dependencies out of
  the daemon package.

## Out of scope

- TUI as a separate product.
- GPUI Kit, Electron, Tauri, or a web UI.
- Per-application focus hacks or automatic shortcut discovery.
- ASR/cloud protocols.
- Kernel/BlueZ/PipeWire plugins.
- v0.4 devices.
- Writing fcitx5-vinput or other consumer configuration.

## Verification

```text
cargo test -p atvv-gui
dbus-run-session -- xvfb-run -a cargo test -p atvv-gui --test ui
cargo run -q -p atvv-gui
```

Manual:

```text
1. Start atvvd and open atvv-config.
2. Click a remote key in the diagram.
3. Set a key/combo action; save.
4. Press the physical remote key; verify the action.
5. Restart atvvd; verify the mapping persists.
6. Disconnect the daemon; verify the GUI reports it and does not crash.
```

## Exit criteria

- ⬜ The remote diagram is usable at common window sizes and HiDPI.
- ⬜ Clicking a key selects it and edits the correct action slot.
- ⬜ “Learn button” works through the daemon and never grabs evdev in the GUI.
- ⬜ Save/apply/reload is atomic; invalid input leaves the old map active.
- ⬜ `exec` is opt-in and cannot run from the GUI without explicit consent.
- ⬜ Keyboard focus, accessible names, and dark/light themes work.
- ⬜ The GUI starts without root and works with no daemon (read-only error
  state).
- ⬜ Real RC003 mapping persists across daemon restart.
- ⬜ The GUI package does not add GTK dependencies to the daemon package.

## Risks

- GTK4/libadwaita is GNOME HIG; non-GNOME users will see a GNOME-style app.
  This is an accepted product decision, not a reason to add a second toolkit.
- Hotspot alignment can drift if the SVG aspect ratio and fixed positions are
  not derived from the same coordinate system.
- A live capture mode can conflict with the user's normal input; bound its
  lifetime and provide an obvious cancel path.
- `exec` confirmation can become a click-through; require an explicit
  confirmation, not a default-focused destructive button.
- A GUI package with GTK dependencies increases install size; keep it optional
  and separate from the daemon package.

## Progress log

- 2026-10-31 — planned after the GTK4/libadwaita decision; no implementation
  yet. Step 13 is the prerequisite.
