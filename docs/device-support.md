# Device support

Support means: a compiled `DeviceProfile`, a real-device run recorded in this
repository, and a package that installs the profile. Synthetic tests and mock
transports prove the code path, not the device.

## Matrix

| Device | USB/HID ids | Profile | ATVV | Audio | State | Evidence |
|---|---|---|---|---|---|---|
| Xiaomi Bluetooth Remote 2 Pro / RC003 | `2717:32b8` | `miremote-rc003` | v1.0 | ADPCM 16 kHz, 120 B frames | Verified 2026-10-30 | [`rc003-hardware-evidence.md`](research/rc003-hardware-evidence.md) |
| Xiaomi RC family, other firmware | vendor `2717`, name `MI RC*` | none | unknown | unknown | Not supported; needs a profile and a run | — |
| ATVV v1.0 remotes without a profile | unknown | none | v1.0 | profile-dependent | Not supported; add a `DeviceProfile` | — |
| ATVV v0.4 remotes | unknown | none | v0.4 | — | Out of scope | — |
| `test-fake-remote` | `1209:0002` (uinput) | test only | simulated | simulated | Test-only, never in `PROFILES` | [`05-input-uinput.md`](plans/05-input-uinput.md) |

Package acceptance: [`docs/acceptance/rc003-2026-10-30.md`](acceptance/rc003-2026-10-30.md)
(partial: package install, post-reboot run, and end-to-end fcitx text commit
are pending operator actions).

## What the RC003 profile fixes

- Direct `AUDIO_START` with no `START_SEARCH`.
- Short `MIC_OPEN` `[0C 00]`, bare `MIC_CLOSE` `[0D]`, bare `MIC_EXTEND` `[0E]`.
- Headerless 120-byte frames, high nibble first, 16 kHz, 0 dB gain.
- 300 ms post-stop discard and an `AUDIO_STOP` echo that must not cause a
  second `MIC_CLOSE`.
- Buttons arrive as a HID keyboard; the voice key is `KEY_F5` and is grabbed so
  it cannot reach the focused application. Home is `KEY_HOMEPAGE`, TV is
  `KEY_GRAVE` on the measured firmware.

## Adding a device

1. Add a `DeviceProfile` to `crates/atvv-protocol/src/profile.rs` with ids,
   name prefixes, codec, frame size, command variants, timing, and keys.
2. Add the matching udev rule only if the vendor/product ids differ from
   `2717:32b8`.
3. Run the profile with a mock/simulated session, then with the real device.
4. Record the run in `docs/research/` with date, address, CAPS bytes, audio
   statistics, keepalive, and reconnect results.
5. Update this matrix only after the recorded run.
