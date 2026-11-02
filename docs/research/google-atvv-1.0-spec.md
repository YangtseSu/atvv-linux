# Google Voice over BLE 1.0 — protocol notes

Source: the archived copy of Google's "Google Voice over BLE spec 1.0",
version 1.0, status Published, last updated 2020-07-11, linked from
[`reference-projects.md`](reference-projects.md#4-upstream-references). The
document is marked **Google Confidential** and has no redistribution license.
It is not vendored in this repository; this is an original interoperability
summary of the protocol facts, not a copy of the text or figures.

## 1. Why it is useful

The project's v1.0 facts were assembled from ATVVoice, field reports, other
implementations, and RC003 runs. The official spec adds the normative
semantics that those sources left implicit:

- the exact `GET_CAPS` and `CAPS_RESP` payload layouts, including the two
  bytes after the frame size;
- the meaning of the `MIC_OPEN` payload (microphone mode, not a codec);
- the stream-id rules for `MIC_CLOSE` and `MIC_EXTEND`;
- the `MIC_OPEN_ERROR` codes and the rule that an ongoing PTT/HTT stream must
  not be interrupted;
- the Audio Transfer Timeout range and the `MIC_EXTEND` interval;
- the three assistant interaction models and the HID-key rule;
- `AUDIO_SYNC` as a codec/sequence synchronization point, including the
  optional 8 kHz/16 kHz dynamic-bandwidth mode.

Two statements in [`protocol-and-device.md`](protocol-and-device.md) were
wrong or incomplete and are corrected there: the `GET_CAPS` payload does not
contain codecs, and `CAPS_RESP` is 8 bytes or longer, not fixed at 7.

## 2. Service and characteristics

| Role | UUID | Properties | Direction |
|---|---|---|---|
| ATVV service | `AB5E0001-5A21-4F05-BC7D-AF01F617B664` | primary service | — |
| Command (TX) | `AB5E0002-5A21-4F05-BC7D-AF01F617B664` | Write Without Response | host → remote |
| Audio | `AB5E0003-5A21-4F05-BC7D-AF01F617B664` | Notify | remote → host |
| Control | `AB5E0004-5A21-4F05-BC7D-AF01F617B664` | Notify | remote → host |

The spec defines TX as Write Without Response. The Linux transport may fall
back to a Write Request when a device advertises only that property; the
project already prefers Write Command and knows the fallback.

## 3. Capability exchange

`GET_CAPS` is a 5-byte payload:

```text
version(2)  legacy_0x0003(2)  supported_interaction_models(1)
```

`0A 01 00 00 03 03` means spec version `0x0100`, legacy constant `0x0003`,
and support for Hold-to-Talk, Press-to-Talk, and On-request (`0x03`). It does
**not** carry a codec mask.

`CAPS_RESP` payload is 8 bytes or longer; the notification adds the `0x0B`
opcode:

```text
version(2)  codec_mask(1)  interaction(1)  frame_size(2)  extra_config(1)
reserved(1)  optional_firmware_data(...)
```

- `codec_mask`: `0x01` ADPCM 8 kHz, `0x02` ADPCM 16 kHz, `0x03` both
  (dynamic bandwidth supported).
- `interaction`: `0x00` On-request, `0x01` Press-to-Talk, `0x03`
  Hold-to-Talk; the value describes the model the remote uses.
- `extra_config` bit `0x01`: the host should attempt DLE and a larger ATT MTU.
- `reserved`: normally zero.
- `optional_firmware_data`: free-form; it may be absent.

The RC003's `0B 01 00 02 03 00 78 00 00` is therefore v1.0, 16 kHz only,
Hold-to-Talk, 120-byte frames, `extra_config = 0`, `reserved = 0`, no
firmware data. The current parser consumes the 7-byte prefix (opcode plus the
six bytes through frame size) and ignores the rest; that is compatible with
the spec's forward-compatibility rule.

The spec allows `GET_CAPS` to be sent more than once; the remote must answer
each request.

## 4. Commands and control events

| Message | Opcode | Payload | Direction |
|---|---|---|---|
| `GET_CAPS` | `0x0A` | version(2), legacy(2), interaction(1) | host → remote |
| `MIC_OPEN` | `0x0C` | microphone mode(1) | host → remote |
| `MIC_CLOSE` | `0x0D` | stream id(1) | host → remote |
| `MIC_EXTEND` | `0x0E` | stream id(1) | host → remote |
| `AUDIO_STOP` | `0x00` | reason(1) | remote → host |
| `AUDIO_START` | `0x04` | reason(1), codec(1), stream id(1) | remote → host |
| `START_SEARCH` | `0x08` | — | remote → host |
| `AUDIO_SYNC` | `0x0A` | codec(1), frame no(2), predictor(2), step index(1) | remote → host |
| `CAPS_RESP` | `0x0B` | see above | remote → host |
| `MIC_OPEN_ERROR` | `0x0C` | error code(2) | remote → host |

Multi-byte fields are big-endian. The remote must ignore unknown trailing
bytes in a payload, which is the spec's forward-compatibility rule.

### Stream ids

- `0x00`: a stream opened by `MIC_OPEN`.
- `0x01..0x80`: a stream opened by a Press-to-Talk or Hold-to-Talk
  interaction; the id is auto-incremented.
- `0xFF` in `MIC_CLOSE`/`MIC_EXTEND`: any active stream.

`MIC_CLOSE` is handled only when the microphone is open and the stream id
matches the active stream; otherwise the remote ignores it. That is why a
host that races the remote's own close does not create a second close loop.

### Microphone mode

`MIC_OPEN` takes one byte:

- `0x00` playback mode: real-time consumption, small internal buffer, the
  remote may drop or downsample to preserve time.
- `0x01` capture mode: non-real-time consumption, larger internal buffer to
  avoid loss.

The project sends `0C 00` (playback mode) on the RC003. The older
`0C 00 00 <codec>` form is not the v1.0 payload; it remains a profile variant
for older devices only.

### Errors

`MIC_OPEN_ERROR` codes:

| Code | Meaning |
|---|---|
| `0x0F02` | remote is not active (Active Remote Timeout) |
| `0x0F03` | audio notifications are not enabled |
| `0x0F80` | a PTT/HTT interaction is already in progress |
| `0x0FFF` | internal error |
| `0x0F01` | reserved |

If `MIC_OPEN` arrives while the microphone is already open:

- after a previous `MIC_OPEN`, the remote restarts the stream and sends
  `AUDIO_START`;
- during PTT/HTT, the remote returns `MIC_OPEN_ERROR(0x0F80)` and does **not**
  interrupt the ongoing stream.

## 5. Interaction models and timeouts

- **On-request (legacy, always supported):** the remote sends `START_SEARCH`
  first, then a HID `KEYCODE_ASSIST` (`0x0C:0x221`) or `KEYCODE_SEARCH`; the
  host decides whether to send `MIC_OPEN`.
- **Press-to-Talk:** the remote sends `AUDIO_START` and audio as soon as the
  Assistant button is pressed; it must not send the HID Assistant key.
- **Hold-to-Talk:** same start, and `AUDIO_STOP` on release; no HID Assistant
  key. The host may still send `MIC_CLOSE`. This is the RC003 model.

`AUDIO_START` reasons: `0x00` MIC_OPEN, `0x01` Press-to-Talk, `0x03`
Hold-to-Talk.

`AUDIO_STOP` reasons: `0x00` MIC_CLOSE, `0x02` HTT release, `0x04` an
upcoming `AUDIO_START`, `0x08` Audio Transfer Timeout, `0x10` audio
notifications disabled, `0x80` other.

Audio Transfer Timeout:

- remote-side timer, recommended 15 s to 1 minute;
- starts/resets when `AUDIO_START` is sent;
- is cancelled by `MIC_CLOSE`;
- on expiry the remote sends `AUDIO_STOP(0x08)` and stops audio.

`MIC_EXTEND` resets that timer and has no reply. The spec says the host may
send it every 5–10 seconds. The project's 10-second keepalive is within that
range; the 60-second session is within the recommended timeout range.

Active Remote Timeout is optional: about one minute after the last user
interaction, `MIC_OPEN` should fail with `0x0F02`.

## 6. Audio

- 8 kHz or 16 kHz, 16-bit samples, IMA/DVI ADPCM at 4:1, high nibble first.
- Frames are headerless; the default is 20 bytes, the spec recommends 160
  bytes for 16 kHz at a 20 ms connection interval, and any size is allowed.
  The RC003's 120 bytes and the Chromecast's 240 bytes are field facts.
- `AUDIO_SYNC` carries the codec that applies after the sync point, the
  sequence number of the next frame, the ADPCM predictor, and the step index.
  It can be sent before the first frame, after dropped frames, or when the
  rate changes.
- Dynamic bandwidth adjustment: a device that advertises codec mask `0x03`
  may switch between 8 kHz and 16 kHz; `AUDIO_SYNC` announces the new codec.
  A fixed-profile device (mask `0x02`) does not switch.
- Audio notifications may be skipped; control notifications are not a
  substitute for audio frames.

## 7. Project deltas and gaps

### Field deviations that stay

- The RC003 accepts bare `MIC_CLOSE` `[0D]` and `MIC_EXTEND` `[0E]`; the spec
  defines a one-byte stream id. The RC003 profile keeps the field-verified
  bare form. The Chromecast profile uses the stream-id form and the spec
  semantics.
- The RC003's `AUDIO_START` codec byte has been unreliable; the project keeps
  `CAPS_RESP` authoritative for the stream rate. The spec says the byte is
  the codec, so this remains a documented firmware quirk, not a general
  license to ignore it on other devices.
- The project sends `MIC_OPEN` mode 0 (playback). Capture mode is defined but
  not used; it is a candidate for a future profile field if a device benefits
  from the larger buffer, not a default.

### Real gaps

- **`MIC_OPEN_ERROR` while streaming.** The session currently moves to
  `Ready` on any `MIC_OPEN_ERROR`, including `0x0F80` while an HTT stream is
  active. The spec says that error must not interrupt the ongoing stream.
  The fix is to ignore `MIC_OPEN_ERROR` when the session is already
  `Streaming`, and to keep the existing `Opening`/`Ready` behavior for a
  failed open. Add a regression test:
  `AudioStart -> MicOpenError(0x0F80)` keeps `Streaming`.
- **Dynamic bandwidth.** `AUDIO_SYNC` parses the codec but the daemon only
  resets the decoder; it does not change the resampler or close the stream.
  A device that advertises `0x03` and switches to 8 kHz would be played at
  the wrong rate. Current profiles are fixed 16 kHz, so this is unsupported,
  not exercised. Decide between a loud `last_error`/close on a codec change
  and actual dynamic-rate support; do not silently misplay.
- **Sequence gaps.** `AUDIO_SYNC` carries the next frame number, but the
  daemon ignores `seq`; `protocol-and-device.md` previously claimed gap
  logging. Either add bounded-rate gap logging or correct the doc. The spec
  makes the sequence number useful for diagnostics.
- **Extra configuration.** The `CAPS_RESP` extra-config byte is ignored. The
  current devices transfer whole frames at MTU 242, so DLE is not needed;
  record the byte if a future device reports it.
- **Active Remote Timeout.** Error `0x0F02` is logged as a generic
  `MIC_OPEN error`; naming it in `atvvctl doctor`/logs would explain a manual
  open after a long idle.

## 8. How this affects B03

The keep-open experiment must respect the spec's open-state rules:

- while the session is armed by a pre-opened `MIC_OPEN`, do not send another
  `MIC_OPEN` on the voice edge; a second open can restart the stream (or
  return `0x0F80` during HTT);
- treat `0x0F80` as an expected response during an ongoing PTT/HTT stream,
  never as a reason to drop `Streaming`;
- the 10-second `MIC_EXTEND` interval is inside the spec's 5–10 second
  recommendation; the 15 s–1 min Audio Transfer Timeout is the bound the
  experiment is working against.

## 9. Source handling

The archived PDF URL is recorded in
[`reference-projects.md`](reference-projects.md#4-upstream-references). Do not
commit the PDF, a text extraction, or its figures: it is marked Google
Confidential and has no redistribution grant. Keep this summary factual and
original; link to the archive instead of copying the document.
