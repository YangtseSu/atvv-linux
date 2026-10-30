# Consumer keys: Voice Command, Assistant, Dictate

Status: research snapshot 2026-10-30. This document records evidence and open
questions; the implementation decision is in
[`docs/plans/11-input-evdevil.md`](../plans/11-input-evdevil.md) and
[ADR 0006](../decisions/0006-evdevil-input-backend.md).

## 1. Scope

The RC003 voice/assistant controls have three plausible Linux event targets:

| Function | evdev key | XKB keysym | xkeyboard-config `inet` | fcitx5 key name |
|---|---|---:|---|---|
| Voice command | `KEY_VOICECOMMAND` `0x246` | `XF86VoiceCommand` `0x10081246` | `I590` | `VoiceCommand` |
| Desktop assistant | `KEY_ASSISTANT` `0x247` | `XF86Assistant` `0x10081247` | `I591`; `FK23` level 2 | `Assistant` |
| Dictation | `KEY_DICTATE` `0x24a` | `XF86Dictate` `0x1008124a` | `I594` | `Dictate` |

The kernel added `KEY_DICTATE` in commit
[`bfa26ba343c7`](https://github.com/torvalds/linux/commit/bfa26ba343c7)
(2022-03-04, "HID: add mapping for KEY_DICTATE"). The kernel key table also
contains `KEY_EMOJI_PICKER = 0x249`; it is adjacent, but it is not part of this
three-key target set.

The names are not interchangeable across layers:

- XKB/xkbcommon uses the `XF86...` names.
- fcitx5 uses its own portable names without the `XF86` prefix. Its key table
  lists `VoiceCommand`, `Assistant`, and `Dictate`; `XF86VoiceCommand` is not
  in that table. A `[TriggerKey]` value of `XF86VoiceCommand` was observed to
  be parsed as invalid and serialized back as an empty entry on 2026-10-30.
  The valid vinput entry is `1=VoiceCommand`.
- The ATVV daemon's `trigger_key` is an evdev name. It resolves
  `VOICECOMMAND`, `ASSISTANT`, and (after the evdevil migration) `DICTATE` to
  the kernel codes above.

## 2. Who uses these keys

- **`XF86Assistant` is a real hardware key on newer laptops.** The Microsoft
  Copilot key sends `Meta+Shift+F23`; xkeyboard-config 2.44 and later map
  `FK23` level 2 to `XF86Assistant` with the
  `PC_SHIFT_SUPER_LEVEL2` type. Users remap the key with `keyd`, KDE
  shortcuts, or custom XKB layouts. It is therefore the most collision-prone
  of the three.
- **`XF86Dictate` is semantically owned by dictation software.** It is the
  HID usage intended to start/stop a dictation session. Older kwin/xkbcommon
  versions have logged `unrecognized keysym "XF86Dictate"`; xkbcommon v5.17
  added the keysym. A future dictation tool may claim it.
- **`XF86VoiceCommand` has the fewest observed default consumers.** That
  makes it the least occupied of the three today, not a private key: it is
  still system-defined and a desktop or voice application may bind it later.

On the test GNOME system, the shell/mutter/window/media-key schemas had no
default binding for these three keys; `mic-mute` uses `XF86AudioMicMute`
instead. This is evidence for one desktop configuration, not a guarantee for
all desktops.

## 3. Current tooling support

- `evdev` 0.13.2 has `KEY_VOICECOMMAND` and `KEY_ASSISTANT` but not
  `KEY_DICTATE`. Its `scancodes.rs` is a 2015 manual snapshot; issue
  [#170](https://github.com/emberian/evdev/issues/170) lists the missing
  codes and remains open.
- `evdevil` 0.5.0 includes all three codes, `Key::from_str("KEY_DICTATE")`,
  and the uinput path needed to emit them. The migration is the subject of
  step 11.
- `fcitx5` 5.1.23 uses the names in the table above. vinput's `[TriggerKey]`
  is an `fcitx::KeyList` and therefore follows fcitx's names, not the XKB or
  evdev names.

## 4. Mapping requirement and open design

Recorded product direction (2026-10-30): the remote's consumer controls are
intended to map to this three-key set. The exact physical-button assignment
is not yet fixed.

### Default trigger decision (2026-10-30)

The daemon's default synthetic trigger is now `DICTATE`, by user direction.
The remote's voice control is a dictation function, so the trigger and the
device function coincide instead of using a spare function key. The vinput
entry is `Dictate`; users keep their own `[TriggerKey]` entries.

`F24` remains an explicit `trigger_key` alternative. It is not a consumer key
and is still useful for IMEs or compositors that cannot bind these codes. Do
not treat `F24` as the default or as a device fact.

Real-device evidence (2026-10-30, `C0:5D:39:F8:2D:4C`): the daemon's evdevil
virtual keyboard emitted `KEY_DICTATE`; fcitx5 5.1.23 received `Key(Dictate)`
(keycode 594), and fcitx5-vinput recorded from `atvv-miremote-rc003` at
16 kHz. A synthetic `ydotool` event with code 586 was not delivered to fcitx
in the same session, while the daemon's virtual keyboard was; use the real
press or an evdevil-based emitter when checking these consumer keys.

### Context-dependent mapping is TBD

A future application may want the same physical control to produce a
different key depending on the active application or workflow. Options, none
selected yet:

| Option | How it works | Cost/risk |
|---|---|---|
| Fixed profile mapping | Each physical control always emits one of the three keys | Simple, predictable; cannot satisfy per-app semantics |
| User profile mapping | User chooses the code per control in the device profile | Configurable; still not context-aware |
| Daemon/agent mapping | Daemon or an external agent selects the code from the active app/scene | Needs a reliable focus/app source; more moving parts |
| Compositor/IME binding | Emit one stable key and let the desktop bind it per app | No device changes; limited to consumers that can bind it |

The decision depends on the future software design and is deliberately
deferred. Step 11 only makes all three codes available; it does not implement
context-sensitive mapping.

## 5. Sources

- Linux `KEY_DICTATE` commit:
  <https://github.com/torvalds/linux/commit/bfa26ba343c7>
- xkeyboard-config `inet` symbols:
  <https://gitlab.freedesktop.org/xkeyboard-config/xkeyboard-config/-/blob/master/symbols/inet>
- fcitx5 key-name list:
  <https://github.com/fcitx/fcitx5/blob/5.1.23/src/lib/fcitx-utils/keylist>
- vinput `TriggerKey` definition:
  <https://github.com/xifan2333/fcitx5-vinput/blob/d9a409d/src/common/config/vinput_config.cpp>
- `evdev` missing scancodes:
  <https://github.com/emberian/evdev/issues/170>
- `evdevil`:
  <https://github.com/SludgePhD/evdevil>
- Copilot-key mapping discussion:
  <https://discussion.fedoraproject.org/t/remap-so-called-copilot-key/162323/21>
- KDE Copilot-key report:
  <https://bugs.kde.org/show_bug.cgi?id=502639>

## 6. Open questions

- Which physical controls map to `VOICECOMMAND`, `ASSISTANT`, and `DICTATE`?
- Should mapping be fixed, user-configurable, or selected from the active
  application/workflow?
- Which applications are expected to consume each key, and do they bind it
  by default?
- Does the target desktop/IME expose a reliable active-application signal for
  context-dependent mapping?
