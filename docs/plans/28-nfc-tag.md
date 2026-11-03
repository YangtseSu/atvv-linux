# 28 — NFC tag investigation and optional trigger

Status: ✅ done
Depends on: none for the research; an NFC reader was attached on 2026-11-03
Touches: `docs/research/` (investigation), possibly a separate helper; not the
daemon core

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

**Decision:** no RC003-specific helper. A dedicated blank NTAG215 is the
better local trigger: it never touches the cast tag, can be mounted where it
is convenient, and a helper can take a configurable UID. The RC003 tag is a
UID-only blank NTAG215, so there is no payload to reuse and no reason to write
it.

## Evidence

- The RC003's NFC is a passive tag used by Xiaomi's phone tap-to-cast: the
  phone touches the remote and starts casting to the paired Xiaomi TV/box
  ([Notebookcheck](https://www.notebookcheck.net/Xiaomi-Bluetooth-Remote-2-Pro-debuts-with-NFC-screen-cast-and-USB-C-charging.1266620.0.html),
  [IT之家](https://www.ithome.com/0/935/995.htm)).
- BlueZ exposes no NFC service; the tag is independent of BLE/ATVV.
- None of the reference projects in
  [`layout-profiles-presets.md`](../research/layout-profiles-presets.md)
  handles NFC.
- A reader is attached and the tag is read: see
  [`nfc-rc003.md`](../research/nfc-rc003.md) for the full dump, commands, and
  output. Reader: ACS ACR122U, firmware `ACR122U214`. Tag: NTAG215, UID
  `1D A4 2A 0B B8 00 00`, empty NDEF, writable, no password. Dump SHA-256
  `935555af6e7f20e45e1fe05bd03b5b9235376e21e6fad3fbbe4405b1e141bb6d`;
  the dump file stays outside the repository.

Closed unknowns:

- Tag IC and memory size: NTAG215, 135 pages, 504 bytes of user memory.
- UID and NDEF payload: UID `1da42a0bb80000`; the NDEF TLV is `03 00 FE`
  (empty message) and pages 5–129 are zero.
- Lock bits and writability: static and dynamic lock bytes are zero, AUTH0 is
  `0xFF`, PWD/PACK are unset; the tag is writable and not password-protected.
- Whether a rewrite changes phone behavior: not tested; no write was made.

## Plan

1. ✅ **Reader.** ACR122U (`072f:2200`, `bcdDevice 2.14`, firmware
   `ACR122U214`), verified with `nfc-list`, `nfc-poll`, and `pcsc_scan`.
2. ✅ **Identify.** `nfc-poll` reads UID `1D A4 2A 0B B8 00 00`;
   `nfc-mfultralight r` reads 135/135 pages into a file outside the
   repository.
3. ✅ **NDEF.** The dump decodes to an empty NDEF message (`03 00 FE`); there
   is no URI, MIME, or Xiaomi payload.
4. ✅ **Lock state.** Static lock0/lock1, dynamic lock bytes, AUTH0, PWD, and
   PACK are recorded; the tag is writable. No write was performed.
5. ✅ **Choose the mode.** Read-only is the correct mode, and no helper is
   shipped: the UID is stable, the tag is blank, and a dedicated blank NTAG215
   is the better trigger target. A rewrite has no benefit.
6. ✅ **Integration boundary.** No NFC code was added. A future helper, if
   wanted, is a separate consumer-side script with a configurable UID using
   libnfc and the existing D-Bus API. No root daemon, kernel module, or BlueZ
   patch.
7. ✅ **Record.** The tag identity, dump hash, NDEF payload, lock state,
   commands, and the UID-gated D-Bus check are in
   [`nfc-rc003.md`](../research/nfc-rc003.md).

## Verification

```text
nfc-poll
nfc-list -t 1 -v
nfc-mfultralight r /tmp/rc003-nfc.mfd
sha256sum /tmp/rc003-nfc.mfd
pcsc_scan
atvvctl mic start
atvvctl status
atvvctl mic stop
```

Recorded 2026-11-03:

- `nfc-poll` → ISO/IEC 14443A target, ATQA `00 44`, UID `1d a4 2a 0b b8 00 00`,
  SAK `00`.
- `nfc-mfultralight r` → `NTAG215 (504 user bytes)`, `135 of 135 pages read
  (0 pages failed)`.
- `sha256sum` → `935555af6e7f20e45e1fe05bd03b5b9235376e21e6fad3fbbe4405b1e141bb6d`.
- `pcsc_scan` → reader `ACS ACR122U 00 00`, card ATR `3B 00`.
- With the RC003 connected (`C0:5D:39:F8:2D:4C`), the UID-gated D-Bus check
  went `ready → streaming → ready` through `atvvctl mic start/stop`.

No tag write, NDEF change, reader firmware change, udev rule, or kernel module
was needed.

## Exit criteria

- ✅ [hardware] Reader model/firmware recorded: ACR122U, `ACR122U214`.
- ✅ Tag IC, UID, memory size, and full dump hash recorded: NTAG215,
  `1da42a0bb80000`, 504 user bytes, dump SHA-256 above.
- ✅ NDEF payload recorded and explained: empty message, so the cast feature is
  not driven by an NDEF payload in this state.
- ✅ Lock state is known: the tag is writable and not password-protected.
- ✅ No write was tested; the original dump is retained and the tag is
  unchanged.
- ✅ [hardware] Read-only trigger explicitly closed as not useful for the RC003
  tag: the UID-gated D-Bus action works, but a dedicated blank NTAG215 is the
  preferred trigger and no helper is shipped.
- ✅ No NFC dependency or NFC code enters the daemon core; no code was added.

## Risks

- Writing an NDEF payload can break or permanently disable the cast tag;
  never write before a verified dump and a restore path. This tag is writable
  but blank, so writing has no benefit for a UID trigger.
- The tag may be locked read-only; this one is not.
- NFC reader firmware and libnfc support vary; record the exact hardware. The
  ACR122 USB init is intermittent after re-enumeration; retry the tool or use
  the PC/SC path.
- A phone may cache the old tag or require the Xiaomi TV app; behavior can
  differ by phone model. Untested here.
- A tag UID is not a secret and can be cloned; do not use NFC alone for
  security-sensitive actions.
- The remote is a physical object shared with the TV; do not leave a rewritten
  tag in place after the experiment.

## Progress log

- 2026-11-01 — plan reviewed; no NFC reader is attached, no work started.
- 2026-11-03 — promoted from B02 to step 28 after an ACR122U was attached.
  `nfc-poll` read the tag and `nfc-mfultralight` dumped 135/135 pages.
  The tag is an NTAG215 with UID `1da42a0bb80000`, an empty NDEF message, and
  clear lock bytes; no write was made. The UID-gated `atvvctl mic` check passed
  on the connected RC003. Evidence is in
  [`nfc-rc003.md`](../research/nfc-rc003.md). The one-per-presentation helper
  decision remains.
- 2026-11-03 — closed with no helper. A dedicated blank NTAG215 is the better
  local trigger: it does not touch the cast tag, can be placed anywhere, and
  a future helper can take a configurable UID. The RC003 tag read stands as
  the negative result: empty NDEF, no payload to reuse, no write needed.
