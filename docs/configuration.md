# Configuration

English | [简体中文](configuration.zh-CN.md)

`daemon.toml` is the single configuration file for `atvvd`. It is parsed and
validated by the `atvv-config` crate; the daemon, `atvvctl`, and the GUI never
grow a second parser. The GUI (`atvv-config`, see [gui.md](gui.md)) edits the
document through `GetConfig`/`SetConfig` and never writes this file. The file
is optional: a missing file means profile defaults.

Location: `$XDG_CONFIG_HOME/atvv/daemon.toml`, or `~/.config/atvv/daemon.toml`.

## Schema

```toml
schema_version = 1
profile = "miremote-rc003"          # must match the build's profile
device = "C0:5D:39:F8:2D:4C"        # written by `atvvctl pair`
adapter = "hci0"                    # optional BlueZ adapter
gain_db = 0                         # -24..=24
input = true                        # evdev/uinput path
vinput = "trigger"                  # trigger | dbus | off
trigger_key = "DICTATE"             # key or chord; none disables
allow_exec = false                  # opt in to exec: actions

[gesture]
double_click_ms = 300
long_press_ms = 550
repeat_start_ms = 350
stable_release_ms = 600
suppression_ms = 180
repeat_interval_ms = 100
back_repeat_interval_ms = 50

[buttons.up]
single = "key:KEY_F13"

[buttons.ok]
long = "combo:ctrl+alt+v"
```

| Field | Default | Notes |
|---|---|---|
| `schema_version` | `1` | Unknown versions are rejected. |
| `profile` | active profile | A different id is rejected; the keymap is local to one profile. |
| `device` | none | Bluetooth address; `atvvctl pair` writes it. Changes apply on the next start. |
| `adapter` | BlueZ default | Adapter name such as `hci0`. |
| `gain_db` | profile value | Clamped to ±24 dB; no AGC. |
| `input` | `true` | Disables discovery, grab, and uinput when false. |
| `vinput` | `trigger` | `trigger` synthesizes `trigger_key`; `dbus` calls `org.fcitx.Vinput`; `off` disables integration. |
| `trigger_key` | `DICTATE` | Single key or chord, for example `F18` or `ctrl+rightctrl`. `none` disables emission. |
| `allow_exec` | `false` | Required for any `exec:` action. |
| `[gesture]` | profile timing | Each field overrides one profile value; bounds are validated. |
| `[buttons.<button>]` | profile defaults | Per-slot action overrides. |

The twelve mappable buttons are `power`, `ok`, `back`, `up`, `down`, `left`,
`right`, `home`, `menu`, `tv`, `volume_up`, and `volume_down`. The voice key is
not mappable: it drives the ATVV microphone and the vinput trigger.

Slots are `single`, `double`, and `long`. A missing slot means "use the profile
default", not "unmapped". The default map re-emits the original key for every
logical button except `power`, which defaults to `none` so it does not race
logind.

## Action grammar

```text
none
key:<name>
combo:<mod>+<mod>+<name>
exec:<command>
dictation:start|stop|toggle
pointer:click:left|right|middle|side|extra|forward|back|task
pointer:move:up|down|left|right:<units>
```

Key names use the same resolver as the input runtime: kernel names
(`KEY_F13`, `KEY_SYSRQ` for Print Screen), aliases (`ctrl`, `alt`, `super`,
`enter`, `esc`, `dictate`, `printscreen`), and synthesized `KEY_<NAME>` forms
are accepted. `print` is the legacy `KEY_PRINT`, not the screenshot key.
`KEY_SYSRQ` can also be written as `sysrq` or `prtsc`. Chords press modifiers
first and release them in reverse. A key may appear only once in a chord.

`none` swallows the button. `dictation:start|stop|toggle` drives the ATVV
session only; it does not emit a key.

Pointer actions use a dedicated `atvv-virtual-pointer` uinput device with
`REL_X`/`REL_Y` and the mouse buttons. `pointer:click:<button>` presses and
releases one button. `pointer:move:<direction>:<units>` moves by 1–255
relative counts; up and left are negative. The device exists only while the
map contains a pointer action. Relative counts are not screen pixels: the
desktop's pointer acceleration and speed settings apply. Raw `BTN_*` names are
rejected; use `pointer:click:<button>` so the button goes to the pointer
device instead of the keyboard.

One direction key can carry the whole pointer gesture:

```toml
[buttons.up]
single = "pointer:move:up:1"
double = "pointer:move:up:10"
long = "pointer:move:up:50"
```

Click, side, extra, forward, back, and task buttons can be mapped the same way:

```toml
[buttons.ok]
single = "pointer:click:left"

[buttons.menu]
single = "pointer:click:right"
```

Validation rejects an unknown button, the voice key, an unknown key name, a
duplicate chord key, a malformed action, pointer units outside 1–255, a raw
`BTN_*` key, an out-of-range gain or gesture value, and an `exec:` action when
`allow_exec = false`. Errors name the button, the slot, and the value.

## `exec:` policy

`exec:` is a general escape hatch, not a scripting language. It is disabled
unless `allow_exec = true`. The daemon executes it with `sh -c` as the user,
with stdin/stdout/stderr redirected to `/dev/null`, one command at a time,
with a 5-second timeout. A daemon running as root refuses `exec:` outright. Failures and
timeouts are reported in `atvvctl status` (`last_error`) and the daemon log.

Editing, reading, or previewing an `exec:` action never runs it. Only a
dispatched remote button runs it, and only when `allow_exec` is on.

## Applying changes

`atvvctl config set` sends the whole document through D-Bus. The daemon
validates every field before applying anything, then:

1. applies gain, input, vinput mode, trigger key, gesture timing, and the
   action map;
2. restarts the input runtime when the map, timing, trigger, or vinput mode
   changes, because the uinput key registration depends on the map. A failed
   restart restores the previous runtime and reports the error;
3. writes the file atomically. A failed write rolls the runtime back.

An invalid document leaves the active map, the runtime, and the file
untouched. `atvvctl reload` re-reads the file through the same path and
returns the validation or apply error instead of reporting success.

The keymap is local configuration. It is not part of the provider contract
advertised on the PipeWire source; consumers bind the declared recording keys,
not the button map.

## `atvvctl`

```sh
atvvctl config get                 # human-readable effective document
atvvctl config get --json          # exact GetConfig payload
atvvctl config set --gain-db 6
atvvctl config set --input false
atvvctl config set --vinput trigger --trigger-key DICTATE
atvvctl config set --allow-exec
atvvctl config set --button up --trigger single --action key:KEY_F13
atvvctl config set --button ok --trigger long --action combo:ctrl+alt+v
atvvctl config set --button ok --trigger single --action pointer:click:left
atvvctl config set --button up --trigger single --action pointer:move:up:1
atvvctl config set --button up --trigger double --action pointer:move:up:10
atvvctl config set --button up --trigger long --action pointer:move:up:50
atvvctl config set --button up --trigger single --action none
atvvctl config set --button up --trigger single --action default
atvvctl reload
atvvctl map test --button up --trigger single
atvvctl map test --action "combo:ctrl+alt+v"
atvvctl map test --action "pointer:click:left"
atvvctl capture --timeout 30    # one press; prints the logical button and slot
```

`atvvctl map test` resolves the active document or a single action string. It
never executes `exec:`; it only validates and prints the key program. A
pointer action reports `kind: pointer` and no key program. `atvvctl status`
reports whether the pointer device is active in `input.pointer`; it is false
for keyboard-only maps.

`atvvctl capture` starts capture mode, waits for one logical button press, and
ends it. Capture reports the logical button and gesture slot recognized by the
input runtime. It does not grab evdev itself and does not expose raw key codes.

## D-Bus API

`org.atvv.Daemon` at `/org/atvv/Daemon`:

| Member | Signature | Notes |
|---|---|---|
| `GetConfig` | `() -> s` | JSON `ConfigDocument`. |
| `SetConfig` | `(s) -> ()` | Validates, applies, and persists. Invalid input returns `org.freedesktop.DBus.Error.InvalidArgs` with the field error. |
| `BeginCapture` | `() -> ()` | Starts capture for the calling connection. |
| `EndCapture` | `() -> ()` | Stops capture. |
| `ButtonEvent` | signal `(s, s, b)` | `button`, `trigger`, `pressed` while capture is active. |

Capture mode ends automatically when the connection that called
`BeginCapture` leaves the bus. The payload is versioned by `schema_version`;
clients must send the complete document back to `SetConfig`.
