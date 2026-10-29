# Reference projects and sources

Pinned snapshots used for the research snapshot on 2026-10-29. URLs are stable;
local clones are not required. Code reuse follows the license table below.

## 1. Snapshots

| Project | URL | Snapshot | License | Role |
|---|---|---|---|---|
| ATVVoice | <https://github.com/b0o/ATVVoice> | `v0.2.0-13-g0e8f641` | MIT | ATVV protocol, native PipeWire, D-Bus API |
| sayall-omarchy | <https://github.com/fan-tastic-z/sayall-omarchy> | `4d10971` | MIT per `Cargo.toml`; no standalone `LICENSE` in snapshot | RC003 field behavior, input/gestures, pairing |
| remote-mic-app | <https://github.com/HD838A/remote-mic-app> | `v2.0.0-470-gdec29b88` | GPL-3.0-only | Feature/behavior reference; macOS |
| fcitx5-vinput | <https://github.com/xifan2333/fcitx5-vinput> | `v2.4.2-106-gd9a409d` | GPL-3.0 | ASR/TTS surface, D-Bus interface |
| Open Voice Bridge | <https://github.com/nijez/open-voice-bridge> | research snapshot | GPL-3.0 | Profile schema and layer architecture prior art |

`sayall-omarchy` has no top-level license file in the snapshot; its
`Cargo.toml` declares `license = "MIT"`. If code is ever reused, preserve the
copyright and license notice and verify the upstream repository at that commit.

## 2. What to consult

| Need | Read | Use |
|---|---|---|
| ATVV message/state design | ATVVoice `src/atvv.rs`, `src/protocol/`, `src/ble.rs`, `src/pw.rs`, `src/dbus.rs` | Protocol and source structure; MIT |
| RC003 quirks | sayall `crates/sayall-atvv`, `crates/sayall-core`, `docs/ARCHITECTURE.md` | Field behavior and timing; MIT |
| Input/gesture/uinput | sayall `crates/sayall-hid`, `crates/sayall-core` | `EVIOCGRAB`, key map, gesture FSM; MIT |
| Pairing | sayall pairing agent | LE SC numeric-comparison auto-confirm; MIT |
| Feature semantics | remote-mic-app `TECHNICAL.en.md`, `README.md` | Hold/tap, pre-roll/drain, mapping, permissions; behavior only |
| Profile schema | Open Voice Bridge `docs/ARCHITECTURE.md`, `specs/device-profile.schema.json` | Fact-directory idea; no code copy |
| fcitx integration | fcitx5-vinput `src/common/dbus/dbus_interface.h`, settings docs | D-Bus names/methods/signals; GPL-3.0 |
| System properties | PipeWire props documentation, bluer README | `Audio/Source`, `Channels=1`, `Cache=no` |

## 3. License boundaries

| Source | License | Can code be copied into this GPL-3.0-or-later tree? |
|---|---|---|
| ATVVoice | MIT | Yes, with copyright/license notice |
| sayall-omarchy | MIT | Yes, with copyright/license notice; verify snapshot |
| remote-mic-app | GPL-3.0-only | **No** unless the whole combined work becomes GPL-3.0-only |
| Open Voice Bridge | GPL-3.0 | **No** unless the whole combined work becomes GPL-3.0-only |
| fcitx5-vinput | GPL-3.0 | Integrate over D-Bus; do not static-link or copy into a differently licensed work |
| BlueZ / PipeWire / kernel | their licenses | Use public APIs only; do not patch or vendor |

Current project rule: original code; MIT references may be used with notices;
GPL-only references are documentation/behavior only.

## 4. Upstream references

### ATVV protocol and BlueZ

- BlueZ issue #1086 (ATVV support discussion):
  <https://github.com/bluez/bluez/issues/1086>
- BlueZ GATT API:
  <https://bluez.readthedocs.io/en/latest/gatt-api>
- bluer Rust bindings:
  <https://github.com/bluez/bluer>
- Google Voice over BLE spec v1.0 (archived copy):
  <https://web.archive.org/web/20260324183034/https://wangefan.github.io/linux_kernel_driver/resources/Google_Voice_over_BLE_spec_v1.0.pdf>
- Android legacy Voice over HID driver (different transport):
  <https://android.googlesource.com/kernel/x86_64/+/f2aa02f9e6f019f7cee08b6b06d4636b210ffe36/drivers/hid/hid-atv-remote.c>
- Infineon CYW20829 voice-remote reference firmware:
  <https://github.com/Infineon/mtb-example-btstack-freertos-cyw20829-voice-remote>
- ATVVoice issues:
  - #1 why a user-space daemon: <https://github.com/b0o/ATVVoice/issues/1>
  - #9 sequence gap: <https://github.com/b0o/ATVVoice/issues/9>
  - #14 cannot stop mic: <https://github.com/b0o/ATVVoice/issues/14>
  - #15 audio sometimes does not start: <https://github.com/b0o/ATVVoice/issues/15>
  - #19 source disappears after PipeWire restart: <https://github.com/b0o/ATVVoice/issues/19>
  - #20 frame timeout: <https://github.com/b0o/ATVVoice/issues/20>

### PipeWire and input

- PipeWire properties:
  <https://docs.pipewire.org/page_man_pipewire-props_7.html>
- keyd (evdev/uinput prior art; not a dependency):
  <https://github.com/rvaiya/keyd>

### fcitx5-vinput

- Project: <https://github.com/xifan2333/fcitx5-vinput>
- D-Bus constants in snapshot:
  `src/common/dbus/dbus_interface.h`
- The integration contract is reproduced in
  [`protocol-and-device.md`](protocol-and-device.md) §8.

## 5. What the references do not settle

The references are sufficient for architecture, protocol implementation,
mock tests, input/gesture design, and integration contracts. They are not
sufficient for hardware acceptance:

- RC003 real `CAPS_RESP`, `AUDIO_START`/`AUDIO_STOP` bytes, echo timing, and
  notification fragmentation.
- RC003 gain/clipping and transcription quality.
- Pairing gesture and real numeric-comparison behavior.
- fcitx warm-stream behavior with this daemon.
- PipeWire service-restart behavior of the installed systemd unit.

Those are step 07/09 evidence items. No reference document can replace them.
