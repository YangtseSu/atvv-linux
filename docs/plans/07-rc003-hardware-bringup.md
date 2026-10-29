# Step 07 — RC003 hardware bring-up

Status: ⬜ not-started
Depends on: 01, 02, 04, 06
Touches: `crates/atvv-bluez/`, `crates/atvv-daemon/`, `crates/atvv-protocol/`

## Goal

Replace the mock transport with a real BlueZ GATT session for the Xiaomi
Bluetooth Remote 2 Pro. Record the real capabilities, audio, stop reason, and
echo behavior; adjust only the profile and evidence-backed state handling.

## Deliverables

- ⬜ `atvv-bluez` using `bluer`: adapter selection, discovery by ATVV UUID,
  connect, service resolution, characteristic lookup.
- ⬜ Pairing command with an auto-confirm agent for LE Secure Connections
  numeric comparison; trust the device and save its address.
- ⬜ GATT notifications for TX/Audio/Control, with `AcquireNotify` preferred and
  a documented fallback.
- ⬜ Real session wiring: `CAPS_RESP`, direct `AUDIO_START`, 120-byte frames,
  `0x0A` sync, `AUDIO_STOP`, keepalive, frame timeout, reconnect.
- ⬜ `atvvctl pair <MAC>`, `atvvctl record --wav`, `atvvctl doctor`, and
  `atvvctl monitor` against the real daemon.
- ⬜ Evidence file with the real CAPS hex, codec, frame size, stop reason, echo
  behavior, gain check, and a WAV/transcription result.

## Design notes

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

Hold the voice key for 3 seconds, then 60 seconds; repeat after disconnect,
adapter power-cycle, and PipeWire restart. Record the exact `CAPS_RESP` and
`AUDIO_STOP` bytes in the evidence section.

## Exit criteria

- ⬜ Pairing succeeds without `bluetoothctl` prompts.
- ⬜ The daemon reports the real ATVV version, codec, frame size, and gain.
- ⬜ A 16 kHz WAV is captured and intelligible/transcribable.
- ⬜ A 60-second session survives keepalive; release closes the mic once.
- ⬜ Reconnect and PipeWire restart recover without a daemon restart.
- ⬜ The stop echo does not cause a close storm.

## Risks

- Pairing may require Home+Menu; document the exact gesture.
- Real firmware may differ from the recorded profile; do not force it.
- BLE MTU/fragmentation can change the frame path; capture notification
  lengths.
- No hardware result may be claimed from a mock.

## Progress log

- 2026-10-29 — step planned; waiting for a device.
