# RC003 NFC tag

Read on 2026-11-03 with an ACS ACR122U. No tag write was performed.

## Reader

- Model: ACS ACR122U PICC Interface, USB `072f:2200`, `bcdDevice 2.14`.
- Firmware: `ACR122U214` (the `FF 00 48 00 00` response is the ASCII string
  without a `9000` trailer).
- Software: libnfc 1.8.0 (`nfc-list`, `nfc-poll`, `nfc-mfultralight`),
  pcsc-lite and pcsc-tools 1.7.5. `pcscd` was running during the read.
- `pcsc_scan` sees the reader as `ACS ACR122U 00 00`; the tag's PC/SC ATR is
  `3B 00`.

## Tag identity

- ISO/IEC 14443A, 106 kbps, ATQA `00 44`, SAK `00`, double-size UID.
- UID: `1D A4 2A 0B B8 00 00` (`1da42a0bb80000`).
- Type: **NTAG215**, 135 pages, 504 bytes of user memory. `nfc-mfultralight`
  reports `NTAG215 (504 user bytes)` from the GET_VERSION reply.
- Capability Container, page 3: `E1 10 3E 00` (NFC Forum Type 2, version 1.0,
  size byte `0x3E`).
- Full dump: 135/135 pages, 0 pages failed, 540 bytes.
  SHA-256 `935555af6e7f20e45e1fe05bd03b5b9235376e21e6fad3fbbe4405b1e141bb6d`.
  The dump file itself is kept outside the repository.

## Memory contents

| Page | Bytes | Meaning |
|---:|---|---|
| 0 | `1D A4 2A 1B` | UID0-2 and BCC0 (`0x1B` verifies) |
| 1 | `0B B8 00 00` | UID3-6 (UID `1D A4 2A 0B B8 00 00`) |
| 2 | `B3 A3 00 00` | BCC1 `0xB3`, internal `0xA3`, static lock0 `0x00`, lock1 `0x00` |
| 3 | `E1 10 3E 00` | Capability Container |
| 4 | `03 00 FE 00` | NDEF TLV type `03`, length `0`, terminator `FE` |
| 5–129 | `00 00 00 00` | user memory, all zero |
| 130 | `00 00 00 BD` | dynamic lock bytes `00 00 00`, RFUI `BD` |
| 131 | `02 00 00 FF` | CFG0: MIRROR disabled, AUTH0 `FF` (no password protection) |
| 132 | `00 00 00 00` | CFG1: no PWD_PROT, no NFC counter, AUTHLIM 0 |
| 133 | `00 00 00 00` | PWD unset |
| 134 | `00 00 00 00` | PACK unset |

## NDEF

The tag carries an **empty NDEF message**: `03 00 FE`, and pages 5–129 are
zero. There is no URI, MIME record, or Xiaomi-specific payload. This tag is
formatted but blank, so the tap-to-cast feature does not read a payload from
this tag's current NDEF message; it must key on the UID (or the tag state is
written by a TV-side flow that has not run here).

A write was not attempted. The tag is writable — static and dynamic lock bits
are clear, AUTH0 is `0xFF`, and PWD/PACK are unset — but rewriting a blank cast
tag would change state without any need for a read-only UID trigger.

## Read-only trigger

The UID is stable across reads and can gate a local D-Bus action. With the RC003
connected (`C0:5D:39:F8:2D:4C`) and the tag on the reader:

```text
$ atvvctl mic start
ok
$ atvvctl status | jq '{state, streaming: .audio.streaming}'
{
  "state": "streaming",
  "streaming": true
}
$ atvvctl mic stop
ok
$ atvvctl status | jq '{state, streaming: .audio.streaming}'
{
  "state": "ready",
  "streaming": false
}
```

The gating logic was not shipped as a helper. A one-action-per-presentation
helper can read the UID with `nfc-poll` (it reports the target and waits for
removal) and call the existing D-Bus API; that is step 28's remaining decision.

## Commands used

```text
nfc-poll
nfc-list -t 1 -v
nfc-mfultralight r /tmp/rc003-nfc.mfd
sha256sum /tmp/rc003-nfc.mfd
lsusb -v -d 072f:2200
pcsc_scan
```

Direct libnfc access worked while `pcscd` owned the reader, but the ACR122 USB
init is not completely stable: after a re-enumeration one `nfc-list` run
failed with `Command Code verification failed`, while `nfc-poll` and
`nfc-mfultralight` succeeded. Retry the tool or use the PC/SC path when a read
fails. No kernel module, udev rule, or reader firmware change was needed.
