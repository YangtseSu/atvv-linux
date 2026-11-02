# Google Chromecast Remote hardware evidence

Recorded on 2026-11-02 on the second development machine. The device is a
Google Chromecast Voice Remote reported by BlueZ as `Chromecast Remote`,
address `74:46:B3:22:2D:D5`, PnP `18d1:9450`, DIS model `A3`.

## Identity and authenticity

| Fact | Observed value |
|---|---|
| Bluetooth name / alias | `Chromecast Remote` |
| Address | `74:46:B3:22:2D:D5` |
| PnP / Modalias | `bluetooth:v18D1p9450d0110` |
| GATT DIS manufacturer | `Google Inc.` |
| GATT DIS model | `A3` |
| GATT DIS hardware revision | `CR PVT/MP` |
| GATT DIS firmware / software | `26.0` / `26.0` |
| GATT DIS serial | `33231QUH1A025B` |
| GATT PnP ID | `01 d1 18 50 94 10 01` (vendor `0x18d1`, product `0x9450`, version `0x0110`) |
| ATVV service | `ab5e0001-5a21-4f05-bc7d-af01f617b664` |
| Battery | 98 % |

The MAC prefix `74:46:B3` is registered to Texas Instruments, not Google. That
is consistent with a Google remote built on a TI BLE SoC but does not prove
origin by itself. The DIS manufacturer, PnP ID, product id, ATVV service, and
firmware strings are all consistent with Google hardware. There is no
cryptographic attestation available over this interface; a clone that copies
all of these fields cannot be excluded from software alone. A physical Google
marking and model label remain the final check.

`atvvctl pair` (workspace build) inferred the profile without a hint:

```text
$ target/debug/atvvctl pair 74:46:B3:22:2D:D5 --no-save
address: 74:46:B3:22:2D:D5
name: Chromecast Remote
profile: google-chromecast
pnp: 18d1:9450
already_paired: true
paired: true
trusted: true
```

## GATT and CAPS

The device exposes the ATVV service and characteristics with the same UUIDs as
the RC003. A verbose daemon run logged:

```text
atvvd: connected to Chromecast Remote (74:46:B3:22:2D:D5)
atvvd: send [0a, 01, 00, 00, 03, 03]
atvvd: control [0b, 01, 00, 02, 03, 00, f0, 01, 00, 1a]
atvvd: caps v1.0 codecs=0x02 interaction=HoldToTalk frame=240
```

`CAPS_RESP` is ten bytes; the parser reads the first seven, so the extra
`01 00 1a` trailer is ignored. Codec `0x02` is ADPCM 16 kHz, interaction
`0x03` is hold-to-talk, and the frame size is 240 bytes (480 samples per frame,
30 ms at 16 kHz).

## Audio

With the daemon started from `~/.config/atvv/daemon.toml` and the profile
active:

```text
$ target/debug/atvvctl contract
profile:        google-chromecast
source:         atvv-google-chromecast
media class:    Audio/Source
format:         16000 Hz mono s16le
recording keys: DICTATE, VOICECOMMAND, ASSISTANT
active key:     DICTATE
```

`pw-cli info atvv-google-chromecast` reports `node.name =
atvv-google-chromecast`, `audio.rate = 16000`, and the three `atvv.*`
contract properties.

A manual `MIC_OPEN` produced a direct `AUDIO_START` (`04 00 02 00`) and
streamed without a physical key press. After two seconds:

```text
"frames": 54,
"samples": 25920,
"streaming": true
```

`54 × 480 = 25,920` samples, confirming the 240-byte frame decode. An initial
16-second hold with the bare profile sent `[0e]` once and logged `keepalive
MIC_EXTEND`, so the device accepts the bare form.

A later run on the same device (`74:46:B3:22:2D:D5`, DIS `A3`, firmware `26.0`)
set the keepalive to the standard stream-id form. The daemon sent `[0e, 01]`
twice while the stream id was `01`; audio continued through both and ended with
a normal release (`control [00, 02]`), not a timeout:

```text
atvvd: control [04, 03, 02, 01]
atvvd: voice started
atvvd: first frame 240 B ...
atvvd: send [0e, 01]
atvvd: keepalive MIC_EXTEND
atvvd: send [0e, 01]
atvvd: keepalive MIC_EXTEND
atvvd: control [00, 02]
atvvd: voice stopped
```

A manual session then tested the stream-id close. The daemon sent `[0d, 00]`
for stream id `00`; the stream stopped and a second manual session subsequently
streamed 52 frames, so the close did not wedge the device:

```text
atvvd: control [04, 00, 02, 00]
atvvd: send [0d, 00]
atvvd: control [00, 00]
...
atvvd: manual MIC_OPEN
atvvd: control [04, 00, 02, 00]
atvvd: state: streaming
```

The A3 therefore accepts both the bare and stream-id `MIC_CLOSE`/`MIC_EXTEND`
forms. The profile uses the stream-id form because the A0 G9N9N firmware
recorded by `zolo1978/google-remote-voice-mac@e57c0a4` sends `[0e, streamID]`
(`Sources/VoiceSession.swift`) and `[0d, streamID]` (`Sources/Engine.swift`);
this gives the two firmware revisions one command dialect. The A0 path still
needs a Linux run. The stream-id close produced a `control [00, 00]` echo; the
state machine ignores it and does not send a second close.

The first frame in one run was:

```text
[ff, fc, 00, 08, 11, 13, 31, 42, 12, 32, 05, 26, 12, 29, 20, 10] peak 324
```

The nibble order is assumed high-first, matching the ATVV v1.0 family; a
speech intelligibility check was not performed. The 300 ms post-stop discard
has not been measured on this device; it is conservative and does not affect
the observed clean stop.

## HID buttons

The HID report descriptor is a two-byte consumer-control array. Linux hid-input
maps its usages to these evdev keys:

```text
KEY_COFFEE, KEY_PLAYPAUSE, KEY_SELECT, KEY_UP, KEY_DOWN, KEY_LEFT, KEY_RIGHT,
KEY_BACK, KEY_HOMEPAGE, KEY_SEARCH, KEY_VOLUMEUP, KEY_VOLUMEDOWN, KEY_MUTE,
KEY_KBDILLUMUP, KEY_CAMERA_ACCESS_DISABLE, KEY_CAMERA_ACCESS_TOGGLE, KEY_TV
```

A physical press log with the `uaccess` rule active mapped the buttons
(2026-11-02, `74:46:B3:22:2D:D5`):

| Physical button | evdev key | Profile mapping |
|---|---|---|
| Power | `KEY_COFFEE` | `power` |
| D-pad up/down/left/right | `KEY_UP`/`KEY_DOWN`/`KEY_LEFT`/`KEY_RIGHT` | arrows |
| D-pad center | `KEY_SELECT` | `ok` |
| Back | `KEY_BACK` | `back` |
| Home | `KEY_HOMEPAGE` | `home` |
| Volume +/− | `KEY_VOLUMEUP`/`KEY_VOLUMEDOWN` | volume |
| Mute | `KEY_MUTE` | `mute` |
| Input (bottom right) | `KEY_TV` | `tv` |
| YouTube | `KEY_CAMERA_ACCESS_DISABLE` | `youtube` |
| Netflix | `KEY_CAMERA_ACCESS_TOGGLE` | `netflix` |
| Assistant/Mic | **no evdev event** | ATVV directly |

The remote has no Menu button and no key labelled TV; the bottom-right Input
button is the `KEY_TV` source. `KEY_PLAYPAUSE` and `KEY_KBDILLUMUP` are
advertised by the descriptor but no physical button produced them. `Menu` is
therefore bound to `KEY_PLAYPAUSE` only to keep every logical slot populated.

The Assistant/Mic key does not emit a HID key. Holding it sends `AUDIO_START`
(`04 03 02 14` with a stream id) directly on the ATVV control characteristic;
releasing sends `AUDIO_STOP 00 02`. Because there is no evdev edge, the daemon
now synthesizes the configured consumer trigger from the session start/stop
when the profile sets `trigger_from_session`. A real press logged:

```text
atvvd: voice started
atvvd: session trigger held
atvvd: state: streaming
atvvd: first frame 240 B raw [...]
atvvd: voice stopped
atvvd: session trigger released
```

## Reconnect

With the daemon connected, `bluetoothctl disconnect 74:46:B3:22:2D:D5` caused
`link lost`, then the daemon reconnected on its own and re-read CAPS:

```text
atvvd: disconnected: link lost
atvvd: state: disconnected
atvvd: reconnecting
atvvd: connected to Chromecast Remote (74:46:B3:22:2D:D5)
atvvd: caps v1.0 codecs=0x02 interaction=HoldToTalk frame=240
```

The final status was `state ready`, `connected true`.

## Pending

- A real speech capture and ASR check.
- Two-device switching with the RC003 on the same host (scheduled for the
  evening of 2026-11-02).
