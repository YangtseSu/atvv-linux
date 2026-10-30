# RC003 hardware run — 2026-10-30

Status: first real-device run for step 07. This file records the observed
ATVV behavior of one Xiaomi Bluetooth Remote 2 Pro (RC003). It supplements the
field-report values in [`protocol-and-device.md`](protocol-and-device.md).

## Environment

| Item | Value |
|---|---|
| Kernel | `7.3.0-3-cachyos` |
| BlueZ | 5.87 (`bluetoothctl --version`) |
| PipeWire | 1.6.10 (`pw-cli --version`) |
| Adapter | `hci0`, `04:ED:33:51:52:FB`, public |
| Device | `C0:5D:39:F8:2D:4C`, 小米蓝牙语音遥控器 (RC003) |
| Device state | paired, bonded, trusted, `AddressType: public`, `LegacyPairing: no` |
| Tools | `target/debug/atvvd --verbose`, `atvvctl`, `busctl --system`, `pw-cat` |

Everything ran as the logged-in user: no root, no kernel module, no BlueZ or
PipeWire patch.

## GATT layout

`busctl --system tree org.bluez` and
`org.freedesktop.DBus.Properties.GetAll` on the ATVV objects:

| Role | UUID | Handle | Flags |
|---|---|---|---|
| Service | `ab5e0001-5a21-4f05-bc7d-af01f617b664` | — | primary |
| TX | `ab5e0002-5a21-4f05-bc7d-af01f617b664` | 769 | `write-without-response`, `write` |
| Audio | `ab5e0003-5a21-4f05-bc7d-af01f617b664` | 771 | `notify` (CCCD present) |
| Control | `ab5e0004-5a21-4f05-bc7d-af01f617b664` | 774 | `notify` (CCCD present) |

BlueZ reported a negotiated MTU of **242** for all three characteristics
after connecting. Before the link was established, a cached 247 was visible.

`atvvctl doctor` prints the same service, flags, and MTU.

## Pairing

The remote was already bonded from desktop use. `atvvctl pair
C0:5D:39:F8:2D:4C --no-save`:

```text
address: C0:5D:39:F8:2D:4C
name: 小米蓝牙语音遥控器
already_paired: true
paired: true
trusted: true
```

No agent callback was invoked because no pairing exchange was started. The
auto-confirm agent (`DisplayYesNo`, numeric comparison) is implemented but was
not exercised; the RC003 pairing gesture is still unverified.

`atvvctl pair` without `--no-save` wrote `device = "C0:5D:39:F8:2D:4C"` to
`~/.config/atvv/daemon.toml`. `atvvd` then started without `--device`.

## CAPS_RESP

Verbose control trace:

```text
control [0b, 01, 00, 02, 03, 00, 78, 00, 00]
caps v1.0 codecs=0x02 interaction=HoldToTalk frame=120
```

- The notification is **9 bytes**: the documented 7-byte layout plus two
  trailing `00 00` bytes. The parser reads the first 7 and ignores the rest.
- version `01 00`; codec mask `0x02` (ADPCM 16 kHz only); interaction `0x03`
  (hold-to-talk); frame size `0x0078` = 120 bytes.
- The profile values for version, codec, interaction, and frame size match the
  real device.

## Start and stop

```text
control [04, 03, 02, 03]   # AUDIO_START reason=HTT, codec=16k, stream=3
control [04, 03, 02, 04]   # later session; stream id increments
control [00, 02]           # AUDIO_STOP reason=HTT button release
control [00, 08]           # AUDIO_STOP reason=transfer timeout (link loss)
```

- `START_SEARCH` was not observed in any session.
- `AUDIO_STOP 00 02` emitted `VoiceStopped` and **no** `MIC_CLOSE`, as the
  session rules require for an HTT button release.
- The stop echo described by field reports was not observed after `00 02` in
  the same notification stream; the 300 ms post-stop discard was not
  exercised because no late audio arrived.
- `AUDIO_STOP 00 08` arrived once during a deliberate link loss.

## Host-initiated `MIC_OPEN`

`atvvctl mic start` sent `0c 00`. The device answered `AUDIO_START` after
roughly 300–400 ms but sent **no audio notifications** until the physical
voice key was pressed. `atvvctl mic stop` while opening sent `0d`, and the
delayed `AUDIO_START` still arrived.

Consequence: on the RC003 the microphone data path is gated by the voice key.
A host-only `MIC_OPEN` opens the session but does not produce frames. Step 08
must synthesize the voice key, not just call `StartVoice`.

## Audio

- Frames arrived whole at the negotiated MTU; the accumulator produced
  120-byte frames. First traced frame prefix:
  `ff ff fb a2 a1 80 80 80 80 88 08 08 80 08 08 19`, decoded peak 1053.
- A ~4.8 s press produced 318 frames / 76,320 samples (4.77 s of 16 kHz PCM).
- A ~59.3 s press produced 3,954 frames / 948,960 samples, with no dropped
  frames and no errors.
- `atvvctl record --wav /tmp/rc003-keepalive.wav --seconds 65`:
  `soxi` reports 1 channel, 16 kHz, 16-bit, 65.00 s; `sox stat` reports
  maximum amplitude 0.999969 and RMS 0.356024. The capture is non-silent
  speech.
- No ASR/transcription was run and the file was not auditioned by a human in
  this session. "Intelligible/transcribable" is therefore not independently
  verified, only that loud speech-like PCM was captured.
- Gain stayed at 0 dB. The peak is near full scale, so clipping on loud speech
  is plausible; no AGC was added and gain remains configurable.

## Keepalive

While streaming, the daemon sent `0e` every 10 s. The 60-second hold logged
six `keepalive MIC_EXTEND` sends and ended only on key release
(`AUDIO_STOP 00 02`). No transfer-timeout stop occurred during the hold.

## Reconnect

`bluetoothctl disconnect C0:5D:39:F8:2D:4C` produced:

```text
disconnected: device event stream ended
state: disconnected
reconnecting
connected to 小米蓝牙语音遥控器
state: connected
```

In one run the first reconnect dropped again immediately (`disconnected: link
lost`) and a second attempt reached `ready` with a fresh `CAPS_RESP`. No
daemon restart was needed; after reconnect the daemon settled at ~0.6 % CPU.

Two bugs were found and fixed during this run:

- `tokio::time::sleep` was constructed outside the runtime in
  `BluezLink::stop_tasks`; the first reconnect panicked the daemon.
- A notification socket at EOF returned empty reads forever. The pump spun at
  ~82 % CPU and ignored task aborts. Empty reads are now bounded and yield,
  and a link generation prevents stale tasks from publishing events after a
  reconnect.

## PipeWire restart

`systemctl --user restart pipewire` left the daemon's source node gone. The
daemon did not recreate it in-process within 8 s and kept reporting
`audio.available = true` because the sink handle still existed. Restarting the
daemon recreated the source and recording worked again.

ADR 0005 assigns full PipeWire service restart to systemd
(`PartOf=pipewire.service`, `Restart=always`), not to `atvv-audio`. The unit is
not installed yet, so this path belongs to step 09. The stale
`audio.available` value during that window is a known status inaccuracy.

## Virtual source scheduling fix

The resident `Audio/Source` stayed suspended whenever no playback stream drove
the graph, and consumers recorded digital silence.

- Without `node.pause-on-idle=false`, the node stays `suspended` with no active
  driver; a linked consumer did not wake it in this environment.
- With `node.pause-on-idle=false`, the node starts suspended, becomes
  `running` when a consumer links, and delivers data. Synthetic 440 Hz tone:
  `pw-cat --record --target atvv-test` captured max 0.2009, RMS 0.1222.
- `node.always-process=true` is not a valid fix: the node freewheels
  (`pw-top` rate 0), drains the queue while unlinked, and consumers then
  record silence.

## Input

- The voice key is `KEY_F5` on `/dev/input/event25`; the daemon grabbed that
  node.
- evdev autorepeat (`value = 2`) previously produced a flood of
  `voice key pressed` lines. Repeats are now ignored for mapped keys and
  forwarded for unmapped keys, so the compositor still sees them.
- On link loss the kernel logs `Failed to ungrab device: No such device`; the
  input thread reports the lost device and exits cleanly.

## Not verified on hardware

- Pairing gesture and the numeric-comparison prompt (device was already
  bonded).
- Notification fragmentation and `0x0A` sync frequency (first frames arrived
  whole; sync traffic was not counted).
- Stop-echo timing (no echo observed after `AUDIO_STOP 00 02`).
- Human or ASR transcription of the captured speech.
- Full PipeWire service restart through the installed systemd unit (step 09).
