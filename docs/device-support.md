# Device support

English | [简体中文](device-support.zh-CN.md)

Support means: a compiled `DeviceProfile`, a real-device run recorded in this
repository, and a package that installs the profile. Synthetic tests and mock
transports prove the code path, not the device.

## Matrix

| Device | USB/HID ids | Profile | ATVV | Audio | State | Evidence |
|---|---|---|---|---|---|---|
| Xiaomi Bluetooth Remote 2 Pro / RC003 | `2717:32b8` | `miremote-rc003` | v1.0 | ADPCM 16 kHz, 120 B frames | Verified 2026-10-30 | [`rc003-hardware-evidence.md`](research/rc003-hardware-evidence.md) |
| Google Chromecast Remote / A3 | `18d1:9450` | `google-chromecast` | v1.0 | ADPCM 16 kHz, 240 B frames | Partial 2026-11-02: ATVV audio, stream-id close/keepalive, pairing, reconnect, and HID map verified; two-device switch pending | [`chromecast-hardware-evidence.md`](research/chromecast-hardware-evidence.md) |
| Google Chromecast Voice Remote / DIS A0, firmware 26.7 | `18d1:9450` | `google-chromecast` | v1.0 | ADPCM 16 kHz, 160 B frames (upstream) | Not run on Linux; same profile, frame size from `CAPS_RESP`, stream-id close/keepalive | [`zolo1978/google-remote-voice-mac@e57c0a4`](https://github.com/zolo1978/google-remote-voice-mac/tree/e57c0a47cece2014734f5bc74f4b5a81bc4e54f9) |
| Xiaomi RC family, other firmware | vendor `2717`, name `MI RC*` | none | unknown | unknown | Not supported; needs a profile and a run | — |
| ATVV v1.0 remotes without a profile | unknown | none | v1.0 | profile-dependent | Not supported; add a `DeviceProfile` | — |
| ATVV v0.4 remotes | unknown | none | v0.4 | — | Out of scope | — |
| `test-fake-remote` | `1209:0002` (uinput) | test only | simulated | simulated | Test-only, never in `PROFILES` | [`05-input-uinput.md`](plans/05-input-uinput.md) |

Package acceptance: [`docs/acceptance/rc003-2026-10-30.md`](acceptance/rc003-2026-10-30.md).

## What the RC003 profile fixes

- Direct `AUDIO_START` with no `START_SEARCH`.
- Short `MIC_OPEN` `[0C 00]`, bare `MIC_CLOSE` `[0D]`, bare `MIC_EXTEND` `[0E]`.
- Headerless 120-byte frames, high nibble first, 16 kHz, 0 dB gain.
- 300 ms post-stop discard and an `AUDIO_STOP` echo that must not cause a
  second `MIC_CLOSE`.
- Buttons arrive as a HID keyboard; the voice key is `KEY_F5` and is grabbed so
  it cannot reach the focused application. Home is `KEY_HOME`, the
  application/menu key is `KEY_COMPOSE`, and TV is `KEY_GRAVE` on the measured
  firmware. The HID descriptor advertises `KEY_HOME` and `KEY_COMPOSE`, not
  `KEY_HOMEPAGE`/`KEY_MENU`.

## What the Chromecast profile fixes

- Direct `AUDIO_START` with no `START_SEARCH`.
- Short `MIC_OPEN` `[0C 00]`, stream-id `MIC_CLOSE` `[0D <stream>]`, and
  stream-id `MIC_EXTEND` `[0E <stream>]`. The A3 accepts both bare and stream-id
  forms; the stream-id form matches the A0 firmware.
- Frame size comes from `CAPS_RESP`: 240 B on the A3 and 160 B on the documented
  A0 firmware. The profile's 240 B value is only a fallback. Frames are
  headerless, high nibble first, 16 kHz, 0 dB gain.
- `CAPS_RESP` is ten bytes (`0b 01 00 02 03 00 f0 01 00 1a`); the extra
  `01 00 1a` trailer is ignored.
- HID consumer controls: `KEY_COFFEE` is Power, `KEY_SELECT` is OK,
  `KEY_HOMEPAGE` is Home, `KEY_BACK` is Back, `KEY_TV` is Input, and
  `KEY_MUTE` is Mute. YouTube and Netflix are `KEY_CAMERA_ACCESS_DISABLE`
  and `KEY_CAMERA_ACCESS_TOGGLE` and pass through. The Assistant key emits
  no evdev key; it sends ATVV `AUDIO_START` directly and the daemon
  synthesizes the consumer trigger from the session.

## Adding a device

1. Add a `DeviceProfile` to `crates/atvv-protocol/src/profile.rs` with ids,
   name prefixes, codec, frame size, command variants, timing, and keys.
2. Add the matching udev rule only if the vendor/product ids differ from
   `2717:32b8`.
3. Run the profile with a mock/simulated session, then with the real device.
4. Record the run in `docs/research/` with date, address, CAPS bytes, audio
   statistics, keepalive, and reconnect results.
5. Update this matrix only after the recorded run.
