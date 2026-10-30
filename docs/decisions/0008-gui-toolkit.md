# ADR 0008 — GUI toolkit and UI boundary

## Status

Accepted, 2026-10-31.

## Context

The next user-facing feature is a configuration app for the remote diagram,
button mapping, action editing, and live capture. The project is Rust; the
daemon and CLI are already Rust. The two serious GUI candidates were
`gtk4-rs` and GPUI Kit.

The user decided that if GTK4 is used, `libadwaita` is mandatory and the UI
must follow GNOME HIG. The app is a Linux system utility, not a
cross-platform product; native desktop integration matters more than a custom
GPU-rendered visual identity.

## Decision

Use **Rust + `gtk4-rs` + `libadwaita`**, following GNOME HIG. The GUI is a
separate user process and talks to `atvvd` over D-Bus. The CLI remains the
scriptable and headless surface.

- No TUI as a separate product.
- No GPUI Kit, Electron, Tauri, or web UI.
- `libadwaita` is not optional for this app.
- The daemon package must not grow GTK dependencies; the GUI is a separate
  package/split package.
- The GUI never grabs evdev, opens BlueZ/PipeWire directly, or writes
  `daemon.toml`; it uses the daemon's config/capture API.

## Rationale

- GTK4/libadwaita is native on Linux and available across desktops; GTK4
  itself is not GNOME-only, but this app deliberately adopts GNOME HIG.
- Accessibility, input-method integration, system theme, file dialogs, and
  packaging are mature.
- The remote diagram is a small number of hotspots; `GtkPicture` +
  `GtkOverlay`/`GtkFixed` with real `GtkButton`s is simpler and more
  accessible than a custom hit-testing widget.
- GPUI Kit's system requirements include Vulkan and WebKitGTK in its Linux
  installation guide; it is a younger framework and raises the MSRV. Its GPU
  and custom-visual advantages do not matter for a settings window.
- Keeping the GUI separate preserves the headless daemon and prevents toolkit
  dependencies from leaking into the service.

## Consequences

- New `atvv-gui` crate and `atvv-config` binary.
- New `gtk4`/`libadwaita`/`librsvg` build and runtime dependencies for the GUI
  package.
- GNOME HIG is a product requirement; do not add a second toolkit for
  non-GNOME users.
- The GUI uses a shared `atvv-config` schema and a D-Bus client; it does not
  duplicate config parsing or action logic.
- `exec:` remains opt-in and is executed by the daemon; the GUI only edits and
  previews it.
