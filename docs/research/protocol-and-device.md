# ATVV v1.0 and device facts

Status: implementation facts and field reports. RC003 values were verified in
the step 07 hardware run on 2026-10-30 where
[`rc003-hardware-evidence.md`](rc003-hardware-evidence.md) says so; everything
else is an upstream or field-report expectation.

## 1. GATT service

| Role | UUID | Properties | Direction |
|---|---|---|---|
| ATVV service | `AB5E0001-5A21-4F05-BC7D-AF01F617B664` | — | — |
| Command (TX) | `AB5E0002-5A21-4F05-BC7D-AF01F617B664` | Write / Write Without Response | Host → remote |
| Audio (RX) | `AB5E0003-5A21-4F05-BC7D-AF01F617B664` | Notify | Remote → host |
| Control (CTL) | `AB5E0004-5A21-4F05-BC7D-AF01F617B664` | Notify | Remote → host |

ATVV is not HID and not a standard Bluetooth audio profile. The remote also
exposes HID-over-GATT for buttons; BlueZ's HOG plugin turns those reports into
evdev events.

## 2. ATVV v1.0 wire facts

Multi-byte fields are big-endian.

### Commands

| Name | Bytes | Direction | Notes |
|---|---|---|---|
| `GET_CAPS` | `0A 01 00 00 03 03` | Host → remote | v1.0, ADPCM 8/16 kHz, PTT + HTT |
| `MIC_OPEN` short | `0C 00` | Host → remote | RC003 form |
| `MIC_OPEN` codec | `0C 00 00 <codec>` | Host → remote | Older-device fallback; not used by RC003 profile |
| `MIC_CLOSE` bare | `0D` | Host → remote | RC003 form |
| `MIC_CLOSE` stream | `0D <stream>` | Host → remote | Profile variant |
| `MIC_EXTEND` bare | `0E` | Host → remote | RC003 form |
| `MIC_EXTEND` stream | `0E <stream>` | Host → remote | Profile variant |
| `CAPS_RESP` | `0B ...` | Remote → host | Control notification |
| `START_SEARCH` | `08` | Remote → host | Not sent by RC003 |
| `AUDIO_START` | `04 <reason> <codec> <stream>` | Remote → host | Payload codec is not authoritative |
| `AUDIO_STOP` | `00 [reason]` | Remote → host | Reason optional; absent means mic close |
| `MIC_OPEN` error | `0C <err_hi> <err_lo>` | Remote → host | Control notification |

### CAPS_RESP v1.0

Fixed layout, 7 bytes:

```text
0B  version_hi version_lo  codec_mask  interaction  frame_hi frame_lo
```

- Version: `01 00` = v1.0. Other versions are rejected; v0.4 is out of scope.
- Codec mask: bit `0x01` = ADPCM 8 kHz, bit `0x02` = ADPCM 16 kHz. Prefer
  16 kHz when both are present.
- Interaction: `0x00` on-request, `0x01` press-to-talk, `0x03` hold-to-talk;
  unknown values are preserved.
- Frame size: 16-bit big-endian; zero is invalid.
- Firmware quirk: some devices report `codec_mask = 0` and put the codec bits
  in the interaction byte. The parser promotes that byte.

### Audio start/stop reasons

- `AUDIO_START` reason: `0x00` mic-open, `0x01` press-to-talk, `0x03`
  hold-to-talk; other values preserved.
- `AUDIO_STOP` reason: `0x00` mic-close, `0x02` HTT button release, `0x04`
  upcoming audio start, `0x08` transfer timeout, `0x10` notifications disabled.
- RC003 reaches `Streaming` from a direct `AUDIO_START`, without a preceding
  `START_SEARCH`.
- On `AUDIO_STOP` with reason HTT button release, the session emits
  `VoiceStopped` but does not send another `MIC_CLOSE`. For other reasons while
  streaming it sends one close.

## 3. RC003 profile

Profile id: `miremote-rc003`. These values were verified on 2026-10-30 except
where the evidence file notes otherwise:

| Field | Value |
|---|---|
| Names | `MI RC*`, `Xiaomi Bluetooth Remote 2 Pro`, `小米蓝牙语音遥控器` |
| VID/PID | `0x2717` / `0x32b8` |
| Model | RC003 |
| Protocol | v1.0 |
| Codec | ADPCM 16 kHz |
| Frame size | 120 bytes, headerless |
| Nibble order | high first |
| `MIC_OPEN` | short `[0C 00]` |
| `MIC_CLOSE` | bare `[0D]` |
| `MIC_EXTEND` | bare `[0E]` |
| Audio start | direct, no `START_SEARCH` |
| Stop echo | present; ignore it |
| Post-stop discard | 300 ms |
| Source rate | 16 kHz |
| Gain | 0 dB |
| Voice key | `KEY_F5` |
| TV key | `KEY_GRAVE` |

The real `CAPS_RESP` wins. Do not force these values if a device reports
different capabilities; record the bytes and update the profile only after
evidence.

## 4. Audio path

- Notifications can be fragmented by MTU or sent in 20-byte blocks. Append to a
  `FrameAccumulator` and decode only complete `frame_size` frames.
- A 7-byte `0x0A` control event carries codec, sequence, predictor (big-endian),
  and step index. It is a decoder sync: reset predictor/step and do not append
  it to the audio accumulator.
- Do not interpolate over sequence gaps. Record and rate-limit the warning;
  `Channels=1` in BlueZ avoids EATT ordering problems.
- The `AUDIO_START` payload codec can be garbage on RC003; CAPS is
  authoritative.
- RC003 sends late audio after `AUDIO_STOP`; discard the 300 ms window. The
  one hardware run saw no late audio after an HTT release (`AUDIO_STOP 00 02`);
  keep the discard for other stop reasons.
- A host `MIC_OPEN` (`0c 00`) produces `AUDIO_START` after roughly 300–400 ms
  but **no audio frames** until the physical voice key is held. The key gates
  the microphone data path; step 08 must synthesize the key rather than rely
  on `StartVoice` for audio.
- RC003 16 kHz uses 0 dB. Clamp gain to ±24 dB; do not enable AGC by default.
- Optional three-point smoothing/click removal is allowed; keep it
  configurable and avoid over-filtering for ASR.
- Keepalive is `MIC_EXTEND` every 10 seconds while streaming. A 60-second
  session must not be truncated.

## 5. Session state machine

| State | Entry | Exit | Actions |
|---|---|---|---|
| `Disconnected` | initial, link loss | discovery/connect | Emit `VoiceStopped` if a stream was active |
| `Connected` | GATT connected | CAPS received | Resolve characteristics |
| `Ready` | CAPS parsed | `START_SEARCH`, `AUDIO_START`, command | Idle; manual mic open allowed |
| `Opening` | `MIC_OPEN` sent | `AUDIO_START`, timeout | 1.5 s watchdog; retry the other MIC_OPEN variant |
| `Streaming` | `AUDIO_START` | `AUDIO_STOP`, disconnect, timeout | Decode, keepalive, state events |
| `Draining` | `AUDIO_STOP` | discard window/queue drained | One close, notify ASR stop |

Important invariants:

- `AUDIO_STOP` is processed only while `Streaming`; outside that state it is an
  echo or stray packet.
- Send at most one `MIC_CLOSE` per session.
- `START_SEARCH` while opening/streaming acts as a second press and closes the
  existing stream.
- Disconnect emits `VoiceStopped` once if the session was streaming.

## 6. BlueZ, PipeWire, and input facts

### BlueZ

- Use `[GATT] Cache = no` and `Channels = 1`. `Channels = 1` disables EATT,
  which matters for ordered audio notifications.
- Do not enable `EnableExperimental` globally without evidence that the target
  BlueZ needs it.
- Prefer `AcquireNotify`; keep a documented fallback to `StartNotify`.
- Choose Write Request vs Write Command from the characteristic flags. Do not
  assume Write Request always works.
- RC003 pairing needs LE Secure Connections numeric comparison. A
  `NoInputNoOutput` agent can fail with `AuthenticationFailed`; use a
  display/confirm agent and trust the bonded device. The pairing gesture may
  require Home+Menu.
- HID buttons arrive through BlueZ HOG as evdev. Do not parse raw HID reports
  as a second button source.
- Only one GATT client/session per device. A second daemon should fail with a
  clear “device busy” error, not acquire a broken session.

### PipeWire

Recommended source properties:

```text
media.type       = Audio
media.category   = Capture
media.class      = Audio/Source
node.name        = atvv-<id>
node.description = <device name>
node.virtual     = true
audio.rate       = <codec rate>
audio.channels   = 1
audio.position   = [ MONO ]
```

- One source per device, resident after first connection. Idle output is
  silence. Do not destroy the node on disconnect; fcitx and other consumers
  lose their device selection.
- `priority.session` should be low; do not make the ATVV source the system
  default.
- The process callback must not allocate or block. Use a bounded SPSC queue;
  drop oldest data on overflow and count the drops.
- The registry `global_remove` callback recreates the node. A full PipeWire
  service restart is systemd's responsibility (ADR 0005), not an in-process
  monitor.
- PipeWire converts the source rate to the graph rate; do not resample in the
  daemon unless a profile requires it.

### Input

- Grab only the keyboard/consumer evdev node. Do not grab mouse/absolute nodes;
  an air-mouse or gyro can break.
- Re-emit every unmapped key after `EVIOCGRAB`; otherwise the user loses keys.
- RC003 key map from field reports
  (`fan-tastic-z/sayall-omarchy@4d10971`, `sayall-hid/src/device.rs`): voice
  `KEY_F5`, TV `KEY_GRAVE`, power `KEY_POWER`, OK `KEY_ENTER`/`KEY_KPENTER`,
  back `KEY_BACK`/`KEY_ESC`, directions `KEY_UP`/`KEY_DOWN`/`KEY_LEFT`/
  `KEY_RIGHT`, home `KEY_HOMEPAGE` (AC Home, not `KEY_HOME`), menu `KEY_MENU`,
  volume `KEY_VOLUMEUP`/`KEY_VOLUMEDOWN`. The voice key is grabbed so F5 cannot
  reload a browser, but it drives ATVV, not the normal action map.
- uinput must register every keycode the action map can emit. Tap is a press
  frame plus release frame; hold-to-talk needs separate press/release.
- RC003 timing constants from field reports: double window 300 ms, long press
  550 ms, repeat start 350 ms, stable release 600 ms, suppression 180 ms.
  Profile overrides are allowed; step 07 confirms them.
- The stable-release latch exists because the remote emits spurious
  release/press cycles roughly 449 ms apart while a key is held. A release
  resolves only after the 600 ms window; a press inside the window cancels it
  and restores the original press time (`sayall-core/src/gesture.rs`).
- Do not map power directly to `KEY_POWER` unless the desktop policy is known;
  logind can consume it. Prefer an explicit action.
- Permissions: `/dev/uinput` and the remote event node need udev
  `TAG+="uaccess"`; load `uinput` via `modules-load.d`. No `audio`/`input`
  group membership.

## 7. Daemon and D-Bus

- Session bus name: `org.atvv.Daemon`, object `/org/atvv/Daemon`.
- Minimum methods: `GetStatus`, `ListDevices`, `StartVoice`, `StopVoice`,
  `ReloadConfig`.
- Signals: `StateChanged`, `VoiceStarted`, `VoiceStopped`, and a generic error
  signal.
- D-Bus carries a view model (JSON/strings), not protocol structs.
- The daemon owns timers, transports, source lifecycle, reconnect, and config.
  It must not require root.
- Step 06 uses a mock transport to drive the same state machine; the mock must
  not become a second session implementation.

## 8. fcitx5-vinput interface

Snapshot: `xifan2333/fcitx5-vinput`, `v2.4.2-106-gd9a409d`, GPL-3.0. Integrate
over D-Bus; do not link or copy its code.

```text
Bus name:   org.fcitx.Vinput
Object:     /org/fcitx/Vinput
Interface:  org.fcitx.Vinput.Service
Methods:    StartRecording()
            StartCommandRecording()
            StopRecording(s scene_id) -> s
            CancelOperation()
            GetStatus() -> s
            GetAsrBackendState()
            ReloadAsrBackend()
            StartAdapter()
            StopAdapter()
Signals:    StatusChanged(s)
            RecognitionResult(s)
            RecognitionPartial(s)
Status:     idle | recording | inferring | postprocessing | error
```

Integration rules:

- fcitx captures 16 kHz mono S16LE. Select the source by name, for example
  `vinput device use atvv-<id>`, then restart `vinput-daemon` if its
  configuration requires it.
- Default trigger is `Alt_R`; tap toggles and hold is push-to-talk. The daemon
  can grab the RC003 voice key and synthesize the configured trigger key on
  press/release. This preserves fcitx's input-context path.
- D-Bus `StartRecording`/`StopRecording` is a fallback. The fcitx addon's
  external trigger uses the last active input context and can be less reliable
  when no context is active.
- `StopRecording` takes the `scene_id` returned by the start flow; do not
  invent an empty id if the API returns one.
- Warm stream: fcitx may keep a bounded inactive capture stream. Do not use
  consumer presence as the mic gate; open/close the remote mic from ATVV
  events. The source itself remains resident.
- Do not change the system default source. Select by name.

## 9. Real-device questions

Answered by the 2026-10-30 run in
[`rc003-hardware-evidence.md`](rc003-hardware-evidence.md), unless marked
otherwise:

- Exact `CAPS_RESP` hex, version, codec mask, interaction, frame size:
  `0b 01 00 02 03 00 78 00 00`, v1.0, mask `0x02`, HTT, 120 bytes (9-byte
  notification).
- Exact `AUDIO_START` payload and stream id: `04 03 02 03`, then `04 03 02 04`;
  the stream id increments.
- Exact `AUDIO_STOP` bytes and whether an echo follows; timing: `00 02` on
  HTT release, `00 08` on link loss; no echo observed.
- Whether `START_SEARCH` ever appears: no.
- Notification lengths, MTU, fragmentation, and `0x0A` sync frequency:
  negotiated MTU 242; frames arrived whole; sync frequency not counted.
- 16 kHz WAV amplitude/clipping and intelligibility: non-silent speech, peak
  0.999969, RMS 0.356; clipping and intelligibility not independently
  verified.
- Pairing gesture and numeric-comparison prompt: not verified (device was
  already bonded).
- 60-second keepalive, release, disconnect/reconnect, adapter power-cycle, and
  PipeWire service restart: keepalive/release/reconnect verified; adapter
  power-cycle and installed-unit PipeWire restart remain open.
