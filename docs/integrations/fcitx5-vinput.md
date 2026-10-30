# fcitx5-vinput

Tested against fcitx5-vinput `2.4.2` (upstream snapshot
`xifan2333/fcitx5-vinput@d9a409d`). Integration is over D-Bus and the PipeWire
source; no vinput code is linked or copied.

## Architecture

```text
RC003 voice key ──HID──> evdev (grabbed) ──> atvv-input
                                              ├─ synthetic Dictate ──> fcitx input context
                                              └─ Voice edge ──────> atvvd
ATVV audio ──GATT──> atvvd ──> Audio/Source atvv-miremote-rc003 ──> fcitx5-vinput
```

`atvvd` owns the session and opens `MIC_OPEN` when the voice key is pressed.
fcitx5-vinput owns recording, ASR, and text commit. The source is resident and
outputs silence while the remote is idle.

## Setup

1. Run the daemon in the user session:

   ```sh
   systemctl --user start atvvd     # after step 09 installs the unit
   # or: atvvd --device C0:5D:39:F8:2D:4C --verbose
   ```

2. Confirm the source exists:

   ```sh
   pw-cli info atvv-miremote-rc003
   atvvctl doctor
   ```

3. Select it in vinput, by name. The daemon re-reads the capture device at the
   start of every recording, so the next recording uses it; no daemon restart
   is needed:

   ```sh
   vinput device list
   vinput device use atvv-miremote-rc003
   vinput device list             # atvv-miremote-rc003 is marked active
   ```

4. Add the ATVV trigger to vinput without replacing the user's shortcut:

   ```ini
   # ~/.config/fcitx5/conf/vinput.conf
   [TriggerKey]
   0=<the user's existing trigger>
   2=Dictate
   ```

   `atvvd` defaults to `trigger_key = "DICTATE"`, which emits the dictation
   consumer key. Stop fcitx5 before editing this file: the addon rewrites its
   in-memory configuration on exit, so an edit made while fcitx5 is running
   can be overwritten. Restart fcitx5 after editing.

   `F24` remains an explicit alternative if the IME cannot bind a consumer
   key: use `trigger_key = "F24"` and add `2=F24` instead.

Do not make the ATVV source the system default. fcitx records at 16 kHz mono
S16LE, matching the source rate.

## Trigger modes

Configure `$XDG_CONFIG_HOME/atvv/daemon.toml` (or
`~/.config/atvv/daemon.toml`). `trigger` is the default.

```toml
# "trigger": synthesize trigger_key from the grabbed voice key (default)
# "dbus":    call org.fcitx.Vinput StartRecording/StopRecording
# "off":     no vinput integration; atvvctl/MCP still control the session
vinput = "trigger"

# DICTATE is the dictation consumer key (KEY_DICTATE). Keep the user's
# existing vinput [TriggerKey] entries and add 2=Dictate.
trigger_key = "DICTATE"
```

`trigger_key` accepts a single key (`DICTATE`, `VOICECOMMAND`, `ASSISTANT`,
`F24`, `F18`) or a `+` chord (`ctrl+rightctrl`). Modifiers are pressed in
order and released in reverse. The target IME and any compositor keybind must
use the same key or chord.

The names are layer-specific and are not interchangeable:

| Function | evdev / daemon | fcitx5-vinput |
|---|---|---|
| Voice command | `VOICECOMMAND` (`KEY_VOICECOMMAND`, `0x246`) | `VoiceCommand` |
| Assistant | `ASSISTANT` (`KEY_ASSISTANT`, `0x247`) | `Assistant` |
| Dictation | `DICTATE` (`KEY_DICTATE`, `0x24a`) | `Dictate` |
| Dedicated fallback | `F24` (`KEY_F24`) | `F24` |

A bare modifier is accepted by fcitx in principle, but a synthetic `Alt_R`
from the ATVV virtual keyboard did not trigger fcitx5-vinput 2.4.2 on
2026-10-30. The dedicated `F24` key and the `Control+Control_R` chord did.

`vinput` and `trigger_key` are read at daemon start; `ReloadConfig` applies
`gain_db` and `input` only. Restart `atvvd` after changing the vinput mode.

### Trigger-key mode (default)

The voice key is grabbed so F5 cannot reach the focused application. Press and
hold emits `DICTATE` down; release emits `DICTATE` up. The vinput addon sees
the same key path as a physical keyboard and starts/stops recording in the
active input context.

- Keep the user's existing vinput trigger and add the ATVV key:
  ```ini
  # ~/.config/fcitx5/conf/vinput.conf
  [TriggerKey]
  0=<the user's existing trigger>
  2=Dictate
  ```
  A fully commented `[TriggerKey]` section means no trigger. The verified
  pair is `2=Dictate` with `trigger_key = "DICTATE"` (2026-10-30, real RC003:
  fcitx `Key(Dictate)`, vinput `target.object=atvv-miremote-rc003`,
  `rate=16000 channels=1 fmt=259`). The `1=F24`/`trigger_key = "F24"` pair and
  the `Control+Control_R` chord with `trigger_key = "ctrl+rightctrl"` were
  verified earlier.
- Hold to talk. The RC003 gates microphone data on the physical key: audio
  flows only while the key is held. A short tap produces a normal key tap for
  fcitx (which may toggle its recording state), but the remote stops sending
  audio as soon as the key is released. Tap-to-toggle cannot sustain a
  recording on this hardware; use push-to-talk.
- Allow roughly 0.3 s after pressing before speaking: `MIC_OPEN` reaches the
  device and the first frames follow `AUDIO_START`.
- If the device node disappears while the key is held (link loss), the input
  runtime emits the trigger release and the daemon stops the recording.

Verify without a compositor: focus an editor using fcitx, hold the voice key,
speak, release. `atvvctl status` must show `streaming` and growing
`audio.frames`; fcitx commits the transcript. A browser must not reload (no F5
leak).

### D-Bus mode

```toml
vinput = "dbus"
```

The daemon calls `org.fcitx.Vinput` directly:

- `StartRecording()` on voice press (no arguments, no return value).
- `StopRecording(s scene_id)` on release; the external path passes the empty
  scene id, matching vinput's own CLI, because start exposes no scene id.

The ATVV mic is still opened from the voice-key edge. If `vinput-daemon` is not
running, the call fails fast (name-ownership check), the daemon keeps the ATVV
session, and the error is reported in `GetStatus` under `vinput.last_error`
and by `atvvctl doctor`. The trigger key is not synthesized in this mode, so
the same key must not be bound in both places.

Use D-Bus mode when the trigger key conflicts with an application binding, or
when the compositor/IME path cannot see the virtual keyboard. External
`StartRecording` resolves the addon's input context from its last key event,
so a mouse-only focus change before the next key press can target a stale
context; trigger-key mode does not have that problem. The upstream focus
fallback (`mostRecentInputContext`) is pending; see
[`docs/research/vinput-upstream-api.md`](../research/vinput-upstream-api.md).

## Verification without a deadline

The daemon is event-driven. Run it and perform the check whenever you happen to
be at the machine; there is no countdown and no window to miss.

```sh
atvvd --verbose
vinput device use atvv-miremote-rc003
vinput device list                    # atvv-miremote-rc003 is active
atvvctl monitor                       # prints voice started/stopped
vinput daemon status                  # "recording" while the key is held
```

Focus an editor that uses fcitx, press and hold the voice key, speak, release.
`atvvctl status` should show `streaming` and growing `audio.frames` during the
press. Repeat the press as often as needed; the check does not expire.

An automated version was used during step 08: a mock-transport daemon plus the
real session services, a focused `zenity --entry`, and `ydotool` delivering the
configured trigger chord. The vinput-daemon journal named
`target.object=atvv-miremote-rc003` and negotiated
`rate=16000 channels=1 fmt=259`. Details are in
[`docs/research/rc003-hardware-evidence.md`](../research/rc003-hardware-evidence.md).

The trigger must match vinput's `[TriggerKey]`
(`~/.config/fcitx5/conf/vinput.conf`). Keep the user's existing entries and
add the ATVV key; a fully commented `[TriggerKey]` section disables the
trigger. Stop fcitx5 before editing, then start it again; the addon rewrites
this file on exit, so an edit made while fcitx5 runs can be overwritten.

```ini
[TriggerKey]
0=Control+Control_R   # or the user's existing trigger
2=Dictate
```

```toml
# ~/.config/atvv/daemon.toml
trigger_key = "DICTATE"
```

Restart `fcitx5` and `atvvd` after changing them. Substituting `1=F24` and
`trigger_key = "F24"` is the dedicated-function-key alternative; the
`Control+Control_R` chord with `trigger_key = "ctrl+rightctrl"` is another
verified path. A GNOME custom shortcut cannot bind a bare modifier key.

## Why not open the mic from the source link?

fcitx5-vinput may keep a warm, inactive capture stream. Consumer presence is
not a microphone request. Opening the remote mic on link would keep the RC003
microphone active indefinitely and drain the battery. The daemon has no
consumer-connection callback; `MIC_OPEN`/`MIC_CLOSE` come only from voice-key
edges, `atvvctl mic`, D-Bus, or MCP.

## Troubleshooting

| Symptom | Check |
|---|---|
| Source missing | `atvvctl status`, `pw-cli info atvv-miremote-rc003`, `atvvctl doctor` |
| Device not in `vinput device list` | the source must exist in PipeWire; the daemon re-reads `global.capture_device` at each recording, so a daemon restart is not needed |
| Trigger does nothing | `atvvctl doctor` → `vinput` and `uinput` lines; `input = false` disables the key path. Ensure `[TriggerKey]` contains `2=Dictate` (or another configured key) and `trigger_key` matches; a commented section disables the trigger. |
| Recording but silence | hold the key; `atvvctl status` must show `streaming`; the key gates RC003 frames |
| F5 still leaks | the RC003 evdev node is not grabbed; check `input.grabbed` in status |
| `vinput` line says dbus mode unavailable | start `vinput-daemon` or switch to `vinput = "trigger"` |

## Out of scope

ASR engines, cloud credentials, and text injection are vinput or agent
concerns. The daemon does not transcribe audio.
