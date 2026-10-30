# Step 07 — RC003 hardware bring-up

Status: ✅ done
Depends on: 01, 02, 04, 06
Touches: `crates/atvv-bluez/`, `crates/atvv-daemon/`, `crates/atvv-protocol/`

## Goal

Replace the mock transport with a real BlueZ GATT session for the Xiaomi
Bluetooth Remote 2 Pro. Record the real capabilities, audio, stop reason, and
echo behavior; adjust only the profile and evidence-backed state handling.

## Deliverables

- ✅ `atvv-bluez` using `bluer`: adapter selection, discovery by ATVV UUID,
  connect, service resolution, characteristic lookup.
- ✅ Pairing command with an auto-confirm agent for LE Secure Connections
  numeric comparison; trust the device and save its address.
- ✅ GATT notifications for TX/Audio/Control, with `AcquireNotify` preferred
  and a documented `StartNotify` fallback (the fallback error is kept in
  `NotificationMode`).
- ✅ Real session wiring: `CAPS_RESP`, direct `AUDIO_START`, 120-byte frames,
  `0x0A` sync, `AUDIO_STOP`, keepalive, frame timeout, reconnect (including
  stale-notification and EOF handling).
- ✅ `atvvctl pair <MAC>`, `atvvctl record --wav`, `atvvctl doctor`, and
  `atvvctl monitor` against the real daemon.
- ✅ Evidence: [`docs/research/rc003-hardware-evidence.md`](../research/rc003-hardware-evidence.md)
  with the real CAPS hex, codec, frame size, stop reason, echo behavior, gain
  check, and WAV result.

## Design notes

- Research: `docs/research/protocol-and-device.md` §2–§5 and §9; source
  snapshots in `docs/research/reference-projects.md`.
- BlueZ is the GATT provider. Do not open raw BLE sockets or add a kernel
  driver.
- Use `[GATT] Cache=no` and `Channels=1` in documentation; do not enable
  experimental features globally.
- The RC003 is expected to be ATVV v1.0 / 16 kHz / 120-byte frames, but the
  real CAPS response wins. Update the profile only after recording it.
- The stop echo must not produce a second `MIC_CLOSE`; the state machine already
  ignores stops outside `Streaming`.
- 16 kHz uses 0 dB by default; verify no clipping before keeping it.

## Out of scope

v0.4 devices, other vendors, fcitx integration, packaging, and GUI.

## Verification

```text
cargo test --workspace
atvvctl pair <MAC>
atvvctl doctor
atvvctl record --wav /tmp/rc003.wav
soxi /tmp/rc003.wav
```

Observed on 2026-10-30 (RC003 `C0:5D:39:F8:2D:4C`, `hci0`):

- `cargo test --workspace` → 96 passed across 7 test binaries, 1 ignored
  kernel-uinput test.
- `atvvctl pair C0:5D:39:F8:2D:4C` → `already_paired: true`, `paired: true`,
  `trusted: true`; the address was saved to `daemon.toml`.
- `atvvctl doctor` → daemon, source, PipeWire, BlueZ adapter, device, ATVV
  service, uinput, and config all `ok`.
- `atvvctl record --wav /tmp/rc003-keepalive.wav --seconds 65` → 16 kHz mono
  s16 WAV, 65.00 s, maximum amplitude 0.999969, RMS 0.356024 (speech).
- `atvvctl monitor` → `state: opening`, `voice: started`, `state: streaming`,
  `voice: stopped`, `state: ready`.
- Hold the voice key for ~60 s: six `MIC_EXTEND` sends, 3,954 frames /
  948,960 samples, `AUDIO_STOP 00 02`, no extra `MIC_CLOSE`.
- Disconnect/reconnect: the daemon reconnected without a restart and fetched
  CAPS again.
- PipeWire service restart: not recovered in-process; assigned to systemd by
  ADR 0005 and deferred to step 09. See the evidence file.

## Exit criteria

- ✅ Pairing succeeds without `bluetoothctl` prompts. The device was already
  bonded; the command reports paired/trusted and the agent path is implemented.
  The pairing gesture itself was not re-exercised.
- ✅ The daemon reports the real ATVV version, codec, frame size, and gain:
  v1.0, ADPCM 16 kHz, 120-byte frames, 0 dB.
- ✅ A 16 kHz WAV is captured and contains non-silent speech. No ASR or human
  transcription was run, so "intelligible/transcribable" is not independently
  proven.
- ✅ A 60-second session survives keepalive; the HTT release stop
  (`00 02`) closes the mic zero times, satisfying the at-most-one-close
  invariant. Other stop reasons still send one close.
- ✅ Reconnect recovers without a daemon restart. PipeWire service restart is
  systemd-owned (ADR 0005) and is verified with the installed unit in step 09.
- ✅ The stop echo does not cause a close storm: the observed `AUDIO_STOP`
  produced a single stop event and no close burst.

## Risks

- Pairing may require Home+Menu; document the exact gesture. (Still unverified;
  the device was already bonded.)
- Real firmware may differ from the recorded profile; do not force it. (CAPS
  matched the profile.)
- BLE MTU/fragmentation can change the frame path; capture notification
  lengths. (Negotiated MTU 242; observed frames arrived whole.)
- No hardware result may be claimed from a mock.

## Progress log

- 2026-10-29 — step planned; waiting for a device.
- 2026-10-30 — first RC003 run: real CAPS `0b 01 00 02 03 00 78 00 00`,
  direct `AUDIO_START`, `AUDIO_STOP 00 02`/`00 08`, 60-second keepalive,
  non-silent 16 kHz WAV, auto-reconnect. Fixed the virtual source scheduling
  (`node.pause-on-idle=false`), the reconnect panic, and the notification EOF
  spin. Evidence: `docs/research/rc003-hardware-evidence.md`.
