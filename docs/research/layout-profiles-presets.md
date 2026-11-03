# Layout profiles, presets, and NFC

Research snapshot for steps 17–18 and backlog B02 (2026-10-31). URLs,
pinned commits, and licenses are kept in
[`reference-projects.md`](reference-projects.md); no local clone path is part
of this repository.

## 1. Projects read

URLs, pinned snapshots, and licenses are in
[`reference-projects.md`](reference-projects.md) §1. This document records
behavior and data-model observations only; no source is copied.

| Project | Relevant feature |
|---|---|
| Open Voice Bridge | hardware device profiles, hotspot layout, per-device mapping |
| mi-ao | two-layer hardware profile + button presets |
| vimo_remote | mode/profile switch (cursor vs 3D teleop) |
| sayall-omarchy | single `[button_map]` TOML, hot reload |
| remote-mic-app-windows | button-profile host, app preset probes (private impl) |
| remote-mic-app | per-device layouts, base mapping + profile overrides |

No source is copied, in particular from the GPL-3.0-only projects.

## 2. What each reference does

### Open Voice Bridge

- `device-profiles/xiaomi-rc003.json` is a **hardware fact** document:
  identity (Bluetooth names, VID/PID), transports, capabilities, protocols,
  and per-platform support. It does not contain user actions.
- `apps/windows/rc003/src/ovb_rc003/remote_layout.py` is **display geometry**:
  thirteen hotspot rectangles as fractions of the product photo, plus the
  physical display names. It is pure data with a unit test, not a user
  layout.
- The Windows settings UI keeps one button mapping list per selected device
  profile (`selected_device_profile`), plus a voice trigger mode. There is no
  named user layout or preset catalog.
- Takeaway: hardware profiles and display geometry are already separate from
  actions in a mature reference. ATVV's `DeviceProfile` remains the hardware
  layer; user layouts are a new, separate layer.

### mi-ao

The closest model for this work. `docs/BUTTON_PRESETS.md` describes two
independent contracts:

- The **hardware profile** answers "which physical usage is this button?" and
  is calibrated per device/firmware.
- The **button preset** answers "what does this button do now?". Presets are
  actions, not hardware facts.

Details worth carrying over:

- The official `pointer` preset is **read-only**, so a baseline always exists.
- Users create a new preset from the default or duplicate the current one;
  presets can be renamed.
- Presets live in `button-presets.json` with `schemaVersion`, written
  atomically with directory `0700` / file `0600`; a corrupt file is
  quarantined as `button-presets.invalid-<UTC>.json`, and a newer schema is
  loaded read-only instead of being overwritten.
- Import is bounded (1 MB, regular file) and validated before overwrite;
  export writes private permissions.
- The TV key can explicitly jump to another saved preset; self-reference,
  deleting a referenced preset, and saving a missing target are rejected.
- Safety: the voice key and menu key are not user-editable; dangerous
  shortcuts (quit, force-quit, lock) are rejected; runtime changes apply
  without restart.

The action vocabulary is macOS-specific (pointer moves, Codex actions).
ATVV maps the same idea onto its existing action grammar
(`key:`, `combo:`, `dictation:`, `none`).

### remote-mic-app (public host)

- Settings keep a **base mapping plus per-device layouts**; the Xiaomi,
  Siri Remote, and Chromecast layouts have different buttons and images.
- A **button profile** can override slots per device; clearing an override
  inherits the base mapping. Import/export and a preview of every button are
  present.
- Button profiles and membership are partly delivered by a private component;
  the public repository documents the host behavior only.
- Takeaway: per-device layout selection is useful, but the paid/private
  implementation is not a model to copy. ATVV keeps one device profile per
  build and adds user layouts on top.

### remote-mic-app-windows

- `crates/sayall-windows-button-profiles/src/lib.rs` is an explicit **stub**
  in the public tree; the real implementation lives in a private repository.
- The public `preset_apps_probe` / `preset_inject_probe` examples show
  presets are oriented toward app-targeted injection.
- Takeaway: app-targeted *injection* is not a good fit for ATVV's provider
  contract. The 2026-11-02 decision allows static, user-selected templates
  verified against named apps; it still forbids detection or automatic
  switching (ADR 0010).

### sayall-omarchy

- `config/sayall.toml.default` has one `[button_map]` table with
  `single`/`double`/`long` actions, hot reload, and no named layouts.
- Actions are `key:`, `combo:`, `dictation:`, `exec:`, `none` — the same
  grammar ATVV already uses.
- Takeaway: the action grammar and TOML shape are already compatible; the
  missing piece is naming and switching multiple maps.

### vimo_remote

- `src/config.rs` stores a `Profile` enum (`Cursor` vs 3D teleop) and the web
  console edits mappings for the selected mode.
- This is a **mode** switch, not a named keymap catalog, but it demonstrates
  that users benefit from switching the remote's semantics as a whole.
- Takeaway: ATVV should switch a complete layout, not individual keys, and
  should keep the device profile unchanged.

## 3. Conclusions for ATVV

1. **Two layers, two names.** `DeviceProfile` stays the hardware/firmware
   identity. A new `Layout` (user-facing "layout") owns button slots and
   gesture timing. Do not overload the existing `profile` field.
2. **A read-only baseline always exists.** `default` is synthesized from the
   device profile and cannot be deleted or edited. User layouts are copies.
3. **Presets are templates, not live state.** A preset is applied by
   creating/resetting a user layout; presets themselves never become the
   active document and cannot contain `exec:`.
4. **Explicit switching only.** No foreground-app detection or automatic
   switching. The active layout is stored and changed by the user through
   CLI, GUI, or D-Bus.
5. **Live apply.** Switching a layout applies the map and gesture timing
   immediately; the input runtime restarts only when the key set changes.
6. **Persistence is versioned and atomic.** Schema v2, migration from v1,
   atomic writes, invalid-file quarantine, and a bounded import path follow
   the mi-ao model without copying it.
7. **Presets.** Presets use `key:`/`combo:`/`none` only and never enable
   `allow_exec`. The 2026-10-31 conclusion that preset content must be
   app-neutral was superseded on 2026-11-02: presets are static templates
   verified against named apps (ADR 0010). Foreground detection and automatic
   switching remain out of scope.
8. **NFC is not part of the core.** No reference project implements the
   RC003 NFC tag. Any use requires an external reader and a separate
   investigation (step 28); the daemon must not gain an NFC dependency.

## 4. NFC facts and measured tag

Verified behavior:

- The RC003's NFC is a **passive tag** used by Xiaomi's phone tap-to-cast:
  the phone reads the tag and starts casting to the paired Xiaomi TV/box
  ([Notebookcheck](https://www.notebookcheck.net/Xiaomi-Bluetooth-Remote-2-Pro-debuts-with-NFC-screen-cast-and-USB-C-charging.1266620.0.html),
  [IT之家](https://www.ithome.com/0/935/995.htm)).
- BlueZ exposes no NFC service for the remote. The NFC path is independent of
  BLE/ATVV.
- The reference projects above contain no NFC handling.
- Measured on 2026-11-03 with an ACS ACR122U: the tag is an **NTAG215**,
  135 pages, 504 bytes of user memory, UID `1D A4 2A 0B B8 00 00`, capability
  container `E1 10 3E 00`, and an **empty NDEF message** (`03 00 FE`). User
  pages 5–129 are zero. Static and dynamic lock bytes are clear, AUTH0 is
  `0xFF`, and PWD/PACK are unset, so the tag is writable and not
  password-protected. No write was made. The dump hash, commands, and memory
  map are in [`nfc-rc003.md`](nfc-rc003.md).
- Because the NDEF message is empty, the cast feature does not consume an NDEF
  payload in this state. A read-only UID trigger is the viable path; the tag
  does not need a rewrite. No NFC code is in the daemon.

Linux tooling: `libnfc` with a PN532 or ACR122U reader (`nfc-list`,
`nfc-poll`, `nfc-mfultralight`) is the least invasive path; `neard` is an
alternative D-Bus daemon with a smaller device list. Both are userspace only.

## 5. Preset decision update (2026-11-02)

The generic `media`/`coding`/`slides` draft from 2026-10-31 was not
shippable: `coding.home = combo:super+l` is GNOME's default screen lock, and
the other chords had no verifiable target. ADR 0010 replaces it with three
static, app-verified presets (`mpv`, `slides`, `omp`). Each preset maps all
twelve action buttons, contains no `exec:`, `dictation:`, or `none`, and is
verified against the app version recorded in its description. The provider
still does not detect the foreground app or switch layouts automatically;
the user chooses a template and edits the resulting user layout.

Recorded app runs: mpv 0.41.0 (pause, fullscreen, mute, seek), LibreOffice
Impress 26.8.1 and WPS Presentation 12.1.2 (F5, navigation, black screen,
Esc), omp 18.14.2 (Enter, Esc, Ctrl+R, Shift+Tab, Alt+R retry).
