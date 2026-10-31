# B02 — NFC tag investigation and optional trigger

Status: ⏸ backlog
Depends on: none for the research; an NFC reader is required
Touches: `docs/research/` (investigation), possibly a separate helper; not
the daemon core

## Goal

Find out what the RC003 NFC tag actually is, document its payload and lock
state, and decide whether it can serve as an optional physical trigger. Two
outcomes are acceptable:

1. **Read-only trigger**: a Linux NFC reader reads the tag UID and a helper
   maps it to a local action (for example `atvvctl mic toggle`), with no tag
   modification.
2. **Reversible rewrite**: only if the tag is confirmed writable and unlocked,
   back up the dump, write a test NDEF payload, verify, and restore the
   original.

If the tag is read-only or locked, the step closes with the documented dump
and no integration.

## Evidence

- The RC003's NFC is a passive tag used by Xiaomi's phone tap-to-cast: the
  phone touches the remote and starts casting to the paired Xiaomi TV/box
  ([Notebookcheck](https://www.notebookcheck.net/Xiaomi-Bluetooth-Remote-2-Pro-debuts-with-NFC-screen-cast-and-USB-C-charging.1266620.0.html),
  [IT之家](https://www.ithome.com/0/935/995.htm)).
- BlueZ exposes no NFC service; the tag is independent of BLE/ATVV.
- None of the reference projects in
  [`layout-profiles-presets.md`](../research/layout-profiles-presets.md)
  handles NFC.
- This workstation has no NFC reader (`/dev/nfc*` absent, no USB reader), so
  the tag cannot be measured yet.

Unknowns to close:

- Tag IC and memory size (NTAG21x, MIFARE Ultralight, another family).
- UID and NDEF payload.
- Lock bits, password protection, and writability.
- Whether a rewrite changes phone behavior and whether restore is exact.

## Plan

1. **Reader.** Attach a libnfc-supported reader (PN532 or ACR122U) and verify
   it with `nfc-list`. Record the reader model and firmware.
2. **Identify.** Read the tag UID and type with `nfc-list`; for
   Ultralight/NTAG families, dump all pages with `nfc-mfultralight` into a
   file kept outside the repository.
3. **NDEF.** Read the NDEF message (`nfc-ndef` / `nfc-read`). Decode the URI,
   MIME type, and any Xiaomi-specific payload. Record the exact bytes.
4. **Lock state.** Inspect the configuration/lock pages. Determine static and
   dynamic locks, password/authentication, and whether any page is writable.
   Do not write in this phase.
5. **Choose the mode.**
   - Read-only: use the UID (or a stable NDEF field) as a trigger. A helper
     process reads the tag and calls the daemon D-Bus (`Mic`/`StartVoice`,
     or another consumer action) once per tap. No tag writes.
   - Rewrite: only after a verified full dump. Write a harmless test NDEF
     (for example a local `http://127.0.0.1:…/nfc` URL), check the phone
     behavior, then restore the dump and verify the original cast behavior.
6. **Integration boundary.** Keep NFC out of `atvv-daemon` and `atvv-protocol`.
   If an integration is delivered, it is a separate optional helper or a
   consumer-side script using libnfc/neard and the existing D-Bus API. No
   root daemon, no kernel module, no BlueZ patch.
7. **Record.** Add the tag identity, dump hash (not the raw dump), NDEF
   payload, lock state, and commands to `docs/research/`.

## Verification

```text
nfc-list
nfc-mfultralight r tag-backup.bin
nfc-ndef r
sha256sum tag-backup.bin
# after the optional write test:
nfc-ndef r
# restore:
nfc-mfultralight w tag-backup.bin
```

A read-only trigger is demonstrated by tapping the remote on the reader and
seeing exactly one daemon action (no repeat while the tag stays present).

## Exit criteria

- ⬜ Reader model/firmware recorded.
- ⬜ Tag IC, UID, memory size, and full dump hash recorded.
- ⬜ NDEF payload recorded and explained.
- ⬜ Lock state is known; the step states whether the tag is writable.
- ⬜ If writable and a write was tested, the original dump is restored and the
  cast behavior is re-verified.
- ⬜ Read-only trigger either demonstrated once per tap or explicitly closed
  as not useful.
- ⬜ No NFC dependency or NFC code enters the daemon core.

## Risks

- Writing an NDEF payload can break or permanently disable the cast tag;
  never write before a verified dump and a restore path.
- The tag may be locked read-only; in that case only the UID trigger is
  possible.
- NFC reader firmware and libnfc support vary; record the exact hardware.
- A phone may cache the old tag or require the Xiaomi TV app; behavior can
  differ by phone model.
- A tag UID is not a secret and can be cloned; do not use NFC alone for
  security-sensitive actions.
- The remote is a physical object shared with the TV; do not leave a rewritten
  tag in place after the experiment.
