# Layout profiles, presets, and NFC

Research snapshot for steps 17–18 and backlog B02 (2026-10-31). Each project
is identified by its GitHub URL and the commit that was read; no local clone
path is part of this repository.

## 1. Snapshots

| Project | URL | Snapshot | License | Relevant feature |
|---|---|---|---|---|
| Open Voice Bridge | <https://github.com/nijez/open-voice-bridge> | `1796b149f752` | GPL-3.0 | hardware device profiles, hotspot layout, per-device mapping |
| mi-ao | <https://github.com/fanxeon/mi-ao> | `d6019bcede80` | MIT | two-layer hardware profile + button presets |
| vimo_remote | <https://github.com/zyxkv/vimo_remote> | `56963df494bf` | MIT | mode/profile switch (cursor vs 3D teleop) |
| sayall-omarchy | <https://github.com/fan-tastic-z/sayall-omarchy> | `4d10971c88eb` | MIT | single `[button_map]` TOML, hot reload |
| remote-mic-app-windows | <https://github.com/GetSayAll/remote-mic-app-windows> | `6dcc34b663b0` | GPL-3.0-only | button-profile host, app preset probes (private impl) |
| remote-mic-app | <https://github.com/HD838A/remote-mic-app> | `834d6a789e43` | GPL-3.0-only | per-device layouts, base mapping + profile overrides |

Only behavior and data-model ideas are used. No source is copied, in
particular from the GPL-3.0-only projects.

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
- Takeaway: app-targeted presets are not a good fit for ATVV's provider
  contract; presets must stay explicit and app-neutral.

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
7. **Generic presets.** Presets use `key:`/`combo:`/`none` only. No
   app-specific actions, no dangerous shortcuts, no automatic `allow_exec`.
8. **NFC is not part of the core.** No reference project implements the
   RC003 NFC tag. Any use requires an external reader and a separate
   investigation (B02); the daemon must not gain an NFC dependency.

## 4. NFC facts and unknowns

Verified behavior:

- The RC003's NFC is a **passive tag** used for Xiaomi's phone tap-to-cast:
  the phone reads the tag and starts casting to the paired Xiaomi TV/box
  ([Notebookcheck](https://www.notebookcheck.net/Xiaomi-Bluetooth-Remote-2-Pro-debuts-with-NFC-screen-cast-and-USB-C-charging.1266620.0.html),
  [IT之家](https://www.ithome.com/0/935/995.htm)).
- BlueZ exposes no NFC service for the remote. The NFC path is independent of
  BLE/ATVV.
- The reference projects above contain no NFC handling.
- This workstation has no NFC reader (`/dev/nfc*` absent, no USB reader).

Unknown until a real tag is measured:

- Tag IC and memory size (NTAG21x, MIFARE Ultralight, or another family).
- Tag UID and NDEF payload.
- Static/dynamic lock bits, password protection, and whether the tag is
  writable at all.
- Whether a rewritten NDEF changes the phone behavior and whether the
  original payload can be restored.

Linux tooling: `libnfc` with a PN532 or ACR122U reader (`nfc-list`,
`nfc-mfultralight`, `nfc-ndef`) is the least invasive path; `neard` is an
alternative D-Bus daemon with a smaller device list. Both are userspace only.
