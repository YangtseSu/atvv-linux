# Step 14 — GTK4/libadwaita configuration app

Status: ✅ done
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

- ✅ `atvv-gui` crate and `atvv-config` binary. The GUI is optional; `atvvd`
  and `atvvctl` work without it.
- ✅ GTK4 + libadwaita application:
  - `AdwApplication`, `AdwApplicationWindow`, `AdwHeaderBar`,
    `AdwNavigationSplitView`, `AdwPreferencesPage`, `AdwPreferencesGroup`,
    `AdwActionRow`/`AdwEntryRow`/`AdwComboRow`, `AdwBanner`, `AdwToast`,
    `AdwAboutDialog`.
  - GNOME HIG spacing, typography, focus rings, keyboard navigation, adaptive
    narrow layout, light/dark system theme.
- ✅ Remote diagram:
  - SVG remote asset loaded with `GtkPicture`.
  - `GtkOverlay`/`GtkFixed` with one real `GtkButton` hotspot per logical
    button; accessible label and tooltip.
  - CSS states: normal, hover, focus, selected, bound, unbound, conflict.
  - Scales with the window without losing hotspot alignment.
  - Click selects the key; no custom hit-testing if a button overlay works.
- ✅ Action editor:
  - Action type: `none`, `key`, `combo`, `exec`, `dictation`.
  - Trigger slot: single, double, long; profile-driven availability.
  - “Learn key” captures a keyboard key/chord.
  - “Learn button” calls `BeginCapture` and highlights the physical remote key
    as it is pressed; no second evdev grab.
  - `exec` editor shows a warning and is disabled unless `allow_exec` is on.
  - Validation errors inline; no silent fallback.
- ✅ Test mode:
  - Press a remote button and highlight it.
  - Show the action that would run.
  - “Run once” is explicit; `exec` requires a separate confirmation.
- ✅ Save/revert:
  - Dirty state, Apply via `SetConfig`, Revert, daemon-not-running banner.
  - Success toast; no direct file writes.
- ✅ Packaging:
  - GUI package or split package (`atvv-linux-gui`) with `gtk4`,
    `libadwaita`, `librsvg`; the daemon package must not gain GTK
    dependencies.
  - `.desktop` file, icon, AppStream metadata, About dialog.
  - No root service; user session only.
- ✅ Tests:
  - Config/action editor unit tests.
  - D-Bus mock for load/save/capture.
  - Headless GTK smoke under `xvfb-run` (window opens, hotspots exist,
    selection updates model).
  - Manual real RC003 run: click key, set action, save, press remote, verify.
- ✅ Docs: installation, keymap editing, test mode, script safety, screenshot.

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

- ✅ The remote diagram is usable at common window sizes and HiDPI.
- ✅ Clicking a key selects it and edits the correct action slot.
- ✅ “Learn button” works through the daemon and never grabs evdev in the GUI.
- ✅ Save/apply/reload is atomic; invalid input leaves the old map active.
- ✅ `exec` is opt-in and cannot run from the GUI without explicit consent.
- ✅ Keyboard focus, accessible names, and dark/light themes work.
- ✅ The GUI starts without root and works with no daemon (read-only error
  state).
- ✅ Real RC003 mapping persists across daemon restart.
- ✅ The GUI package does not add GTK dependencies to the daemon package.

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
- 2026-10-31 — implemented `atvv-gui` (`atvv-config`): GTK4/libadwaita window
  with an embedded SVG remote and twelve `GtkButton` hotspots, action/gesture
  editor, keyboard and remote learning, test mode, `exec` confirmation, and
  daemon banners/toasts. The D-Bus worker loads/saves/captures/RunAction off
  the GTK thread.
- 2026-10-31 — `cargo test -p atvv-gui` → 15 unit tests, 3 private-bus worker
  tests, and 1 `xvfb-run` UI smoke test passed. `cargo test --workspace`
  passed; `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo fmt --all -- --check` are clean.
- 2026-10-31 — real RC003 run with the workspace daemon and a scratch
  `XDG_CONFIG_HOME`: the GUI selected `up`, saved `double = "key:KEY_F13"`
  through `SetConfig`, and after stopping and restarting the daemon
  `atvvctl config get` still returned `key:KEY_F13`. The physical test-mode
  press was not observed in the 45 s window; the input runtime itself was
  verified on the RC003 in steps 07/11.
- 2026-10-31 — split `PKGBUILD` into `atvv-linux` and `atvv-linux-gui`;
  added desktop entry, AppStream metadata, scalable icon, `docs/gui.md`, an
  embedded remote asset, and the dark/light theme check.
- 2026-10-31 — `makepkg -f` produced `atvv-linux 0.1.0-7` (6.2 MiB, sha256
  `3b248084…02c35`) and `atvv-linux-gui 0.1.0-7` (1.3 MiB, sha256
  `470cca59…064b3`) from source tarball `7c276027…a362e`. The base package
  still depends only on `pipewire`, `bluez`, and `dbus`; the GUI package adds
  `gtk4`, `libadwaita`, `librsvg`, and `atvv-linux`. Extracted packages were
  run under Xvfb with `atvvd --mock`; the packaged `atvv-config` renders the
  diagram. `namcap` reports only expected implicit-library and runtime-service
  notes. Packages are not installed because `pacman -U` requires the
  operator's password.
- 2026-10-31 — a second real Test-mode window (45 s) again saw no physical
  press; the remote was idle. Save/persistence and the daemon capture API are
  verified, but step 14 stays in progress until one press is recorded.
- 2026-10-31 — replaced the placeholder diagram with an original vector
  illustration of the real RC003 layout: power and voice at the top, the
  circular D-pad, back/home/menu on the left, and the volume rocker plus TV on
  the right. The voice key is drawn but is not a hotspot. Hotspot coordinates
  were moved to match; the UI smoke test, screenshot, icon, and legend were
  refreshed.
- 2026-10-31 — rebuilt `0.1.0-7` after the diagram fix; the staged package
  now renders the RC003 layout. The third physical Test-mode window (45 s)
  also saw no key event; the operator was not at the remote.
- 2026-10-31 — layout direction decided after reviewing SayAll (`remote-mic-app`)
  and Open Voice Bridge: step 14 keeps the GNOME split view and finishes; the
  central-remote, per-key three-slot board is planned as step 15.
- 2026-10-31 — fixed hotspot pointer input. `GtkOverlay` allocated the
  overlay `GtkFixed` at `0×0`, so no button had a hit area; the diagram was
  visually present but unclickable. Replaced the overlay with a `GtkFixed`
  surface, a 220×564 initial request, and a tick-based re-layout on size
  changes. XTest clicks selected all twelve hotspots in order; the UI smoke
  test now asserts the overlay and every hotspot have a real allocation.
- 2026-10-31 — rebuilt `0.1.0-7` with the click fix. The packaged
  `atvv-config` selected `Back` (`KEY_BACK`) when clicked under Xvfb, so the
  staged binary has the working hit areas.
- 2026-10-31 — final `0.1.0-8` rebuild: `atvv-linux` sha256
  `d6792082…718076`, `atvv-linux-gui` sha256 `7fc99deb…c4c117`, source
  tarball `1752544f…c541451`. Built from the completed step-14 tree; not
  installed. The earlier installed 0.1.0-7 predates these fixes.
- 2026-10-31 — test-mode fixes verified on the RC003: capture now suppresses
  mapped keys, exec, dictation, the synthetic voice trigger, and voice-session
  start/stop; voice edges and release edges are forwarded as `ButtonEvent`.
  All thirteen keys were observed; OK did not exit Test mode; system volume
  stayed unchanged. `KEY_HOME`/`KEY_COMPOSE` replace the old
  `KEY_HOMEPAGE`/`KEY_MENU` home/menu mapping. The CSS provider is installed
  after GTK init, so the selected/pressed SVG regions render; the voice key has
  a non-editable hotspot and a fixed-key note.
