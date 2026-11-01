# ADR 0010: Layout presets are app-verified templates

Status: accepted 2026-11-02

## Context

Step 17 added named user layouts and left three ids reserved for presets
(`media`, `coding`, `slides`). The step 18 draft defined generic presets:
`media` for television navigation, `coding` for editor shortcuts, and
`slides` for presentations. The mappings were guesses. Two problems surfaced
during review:

- The `coding` draft mapped `home` to `Super+L`. On GNOME that is the default
  screen-lock binding, so pressing Home would lock the session. `Super+I` has
  no standard meaning. A generic preset cannot promise a working chord across
  editors, browsers, desktops, and input methods.
- The `media` draft mostly reproduced the device defaults. A real player's
  Up/Down keys mean seek by a minute in mpv but something else in another
  player, so calling the map "media" hid an unverified assumption.

The provider contract (ADR 0007) says ATVV does not detect or adapt to an
application. That rule is about automatic behavior: foreground detection,
shortcut injection, and per-app diagnostics. It does not require preset data
to be app-agnostic. The research snapshot had recorded the stricter
app-neutral conclusion; this ADR deliberately supersedes that part.

## Decision

1. **Presets are read-only templates verified against named apps and
   versions.** Each preset carries `verified_with` (app and version) and is
   only shipped after a recorded run in that app. The initial catalog is
   `mpv` (mpv), `slides` (LibreOffice Impress and WPS Presentation, which
   share the standard presentation keys), and `omp` (omp).
2. **Every preset maps all twelve action-mapped buttons on a single press.**
   No preset contains `none`, `exec:`, `dictation:`, or pointer actions.
   Double and long presses stay at the profile defaults so a preset does not
   add gesture latency.
3. **Presets are not live state.** Applying one creates a user layout; it
   never stores a preset, makes it active automatically, or edits `default`.
   `--activate` (CLI) and the GUI's **Use** action are the only ways to switch.
   Resetting a user layout to a preset keeps that layout's name and
   description.
4. **Preset ids stay reserved.** `default`, `mpv`, `slides`, and `omp` cannot
   be user layout ids. A layout created from a preset with the same display
   name therefore gets a `-layout` suffix (`mpv-layout`), which keeps `layout
   show mpv` unambiguous.
5. **No per-application detection.** The user selects a preset or layout;
   ATVV does not observe the foreground app, switch layouts, or inject
   app-specific behavior. Preset content is static configuration data, and
   the user can edit the resulting layout freely.

## Consequences

- Every preset needs a real-app run, recorded with app version, commands, and
  observed behavior. A synthetic grammar test alone is not enough. App
  versions are part of the user-facing description because key bindings can
  change; a major update requires re-verification.
- The catalog stays small and fixed. Adding an app is a code change with a
  verification record, not an open-ended support promise. The GUI calls the
  copies templates and the docs say they are starting points.
- The `coding`/editor preset is dropped. Editing is keyboard-first, and an
  editor preset would duplicate keys the user already has. The agent preset
  targets the conversational session loop instead: submit, interrupt,
  history, tool output, thinking level, and retry.
- omp user remaps (`~/.omp/agent/keybindings.yml`) and Vim editing mode can
  change the behavior of the mapped keys. The preset is verified against the
  default key bindings and the docs state that.
- mpv user `input.conf` files can override the defaults. The mpv preset is
  verified with `--no-config` (default bindings); the docs say so.
- The slides preset names two apps. If their mappings diverge in a future
  version, the catalog splits into separate presets instead of guessing.

### Rejected alternatives

- **Keep the generic `media`/`coding`/`slides` presets.** Their chords are
  unverifiable, and one of them locks the GNOME session.
- **Detect the foreground app and switch layouts automatically.** This is the
  provider behavior ADR 0007 forbids, and it fails for terminals, remote
  sessions, and Wayland clients that do not expose the focused app.
- **Drop presets and remove the reserved ids.** That leaves the user with no
  starting point and makes the already-paid reservation dead weight. It was
  the fallback only if the catalog could not be verified.
- **Ship an app-specific preset without a recorded run.** This repeats the
  problem the app-verified rule exists to prevent.
