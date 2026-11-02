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
schema_version = 3
active_device = "C0:5D:39:F8:2D:4C"  # written by `atvvctl pair`
adapter = "hci0"                    # optional BlueZ adapter
gain_db = 0                         # -24..=24
input = true                        # evdev/uinput path
vinput = "trigger"                  # trigger | dbus | off
trigger_key = "DICTATE"             # key or chord; none disables
allow_exec = false                  # opt in to exec: actions

[devices."C0:5D:39:F8:2D:4C"]
label = "Living room"               # optional
profile = "miremote-rc003"
layout = "editor"                   # default or a user layout id

[devices."74:46:B3:22:2D:D5"]
profile = "google-chromecast"
layout = "default"

[layouts.editor]
name = "Editor"
description = "Editor and browser shortcuts"

[layouts.editor.gesture]
long_press_ms = 700

[layouts.editor.buttons.up]
single = "key:KEY_F13"

[layouts.editor.buttons.ok]
long = "combo:ctrl+alt+v"
```

| Field | Default | Notes |
|---|---|---|
| `schema_version` | `3` | Version 1 and 2 migrate on load; higher versions are rejected. |
| `active_device` | none | Bluetooth address of the active device; `atvvctl pair` writes it and makes the device active. |
| `[devices.<address>]` | none | Known devices: optional `label`, required `profile`, and `layout`. Addresses are normalized to uppercase. |
| `adapter` | BlueZ default | Adapter name such as `hci0`. |
| `gain_db` | profile value | Clamped to ±24 dB; no AGC. |
| `input` | `true` | Disables discovery, grab, and uinput when false. |
| `vinput` | `trigger` | `trigger` synthesizes `trigger_key`; `dbus` calls `org.fcitx.Vinput`; `off` disables integration. |
| `trigger_key` | `DICTATE` | Single key or chord, for example `F18` or `ctrl+rightctrl`. `none` disables emission. |
| `allow_exec` | `false` | Required for any `exec:` action. |
| `[layouts.<id>]` | none | User layouts: `name`, optional `description`, `[gesture]`, and `[buttons.<button>]`. |

Only the active device owns the GATT session, the PipeWire source, the HID
grab, and the board. `atvvctl device use <address>` switches between known
devices without restarting the daemon; the old device is disconnected and its
grab is released before the new one connects. Per-device layouts are stored in
the device entry, so switching reloads the board with that device's layout.
Known devices persist across daemon restarts. `atvvctl device rename <address>
<label>` sets or clears the optional label; an empty label clears it.

The twelve mappable buttons are `power`, `ok`, `back`, `up`, `down`, `left`,
`right`, `home`, `menu`, `tv`, `volume_up`, and `volume_down`. The voice key is
not mappable: it drives the ATVV microphone and the vinput trigger. The
`google-chromecast` profile uses `KEY_SELECT` for OK, `KEY_HOMEPAGE` for Home,
and `KEY_TV` for Input; unmapped app and mute keys pass through. The
Assistant key sends ATVV `AUDIO_START` directly and the daemon synthesizes
the configured trigger from the session.

Slots are `single`, `double`, and `long`. A missing slot means "use the profile
default", not "unmapped". The default map re-emits the original key for every
logical button except `power`, which defaults to `none` so it does not race
logind.

## Layouts

A layout is a named map of button actions plus gesture-timing overrides. The
device profile stays the hardware/firmware identity; layouts are the user's key
maps. `default` is synthesized from the profile, is never stored, and is a
**template**: edit it on the board and Apply saves the edits as a new user
layout, leaving the template unchanged.

A layout has a required name (unique case-insensitively), an optional
description, optional per-button overrides, and optional gesture overrides. Ids
match `[a-z0-9][a-z0-9_-]{0,31}`; `default` and the preset ids `mpv`, `slides`,
and `omp` are reserved. At most 32 user layouts are allowed; a name is 1–40
characters and a description at most 120.

Global fields (`adapter`, `gain_db`, `input`, `vinput`, `trigger_key`,
`allow_exec`) are not part of a layout. Switching a layout never changes the
exec policy, the active device, or the recording key. `default` cannot be
stored, renamed, deleted, or reset, and it is not listed in `layouts`. The
active device's `layout` entry is updated when a layout is switched.

Gesture overrides apply to the whole active layout, so switching a layout can
change double/long behavior. `atvvctl layout show` prints the effective timing.

### Presets

Three built-in presets are read-only templates for a user layout:

| id | Name | Verified with | Purpose |
|---|---|---|---|
| `mpv` | mpv | mpv 0.41.0 | Space pauses, `f` toggles fullscreen, arrows seek, `m` mutes, `q` quits |
| `slides` | Slides | LibreOffice Impress 26.8.1, WPS Office 12.1.2 | PageUp/PageDown, B for a black screen, F5 starts, Esc ends |
| `omp` | omp | omp 18.14.2 | Enter submits, Esc interrupts, Ctrl+R searches history, Ctrl+O expands tools, Shift+Tab changes thinking, Alt+R retries |

Every preset maps all twelve action-mapped buttons on a single press and
contains no `none`, `exec:`, or `dictation:`. Double and long presses stay at
the profile defaults. The mappings were verified against each app's default
bindings; user remaps in the app (mpv `input.conf`, omp
`~/.omp/agent/keybindings.yml`, omp Vim mode) can change them.

Presets are not live state. Applying one creates a user layout and leaves the
active layout unchanged unless `--activate` is given; `default` is never
edited. The preset ids are reserved, so applying `mpv` with the default name
creates the layout id `mpv-layout`. Reset a user layout to a preset with
`layout reset <id> --preset <preset>`; the layout keeps its name and
description. The GUI exposes the same catalog as **New from preset…** and
**Reset to preset…**. ATVV never detects the foreground app or switches
layouts automatically; the resulting layout is editable like any other.

### Migration

A version 1 file with top-level `[buttons]` or `[gesture]` tables is migrated
on load to a user layout `custom` named "Custom"; a version 1 file without
overrides uses the `default` template. A version 2 file must not contain the
top-level tables. The next save writes version 3: the single `device` field
becomes an `[devices.<address>]` entry with the file's profile and
`active_layout`, `active_device` points at it, and the old top-level `profile`,
`device`, and `active_layout` keys are dropped. A version 3 file keeps full
control over its registry. `SetConfig` accepts only version 3, so a stale
client fails loudly instead of dropping layouts.

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
validates every field before applying anything, including every stored layout,
then:

1. applies gain, input, vinput mode, trigger key, gesture timing, and the
   active layout's action map;
2. restarts the input runtime when the map, timing, trigger, or vinput mode
   changes, because the uinput key registration depends on the map. A failed
   restart restores the previous runtime and reports the error;
3. writes the file atomically. A failed write rolls the runtime back.

An invalid document leaves the active map, the runtime, and the file
untouched. `atvvctl layout use` applies the selected layout through the same
path and persists `active_layout`; a failed runtime apply or write leaves the
previous layout active. `atvvctl reload` re-reads the file through the same
path and returns the validation or apply error instead of reporting success.

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
atvvctl layout list
atvvctl layout show editor
atvvctl layout use editor
atvvctl layout new editor --name Editor --from default
atvvctl layout rename editor "Editor layout"
atvvctl layout delete editor
atvvctl layout reset editor
atvvctl layout reset editor --preset omp
atvvctl layout preset list
atvvctl layout preset show omp
atvvctl layout preset apply mpv
atvvctl layout preset apply omp --as agent --activate
atvvctl config set --layout editor --button up --trigger single --action key:KEY_F13
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
atvvctl map test --layout editor --button up --trigger single
atvvctl map test --action "combo:ctrl+alt+v"
atvvctl map test --action "pointer:click:left"
atvvctl capture --timeout 30    # one press; prints the logical button and slot
```

`atvvctl layout use` sends `SetLayout`. `layout new` and `layout rename` do not
switch the active layout. `layout preset apply` also stays inactive unless
`--activate` is passed. `layout delete` refuses the active layout, and
`layout delete`, `layout rename`, and `layout reset` refuse `default`. When the
active layout is `default`, `config set --button` refuses and points at
`atvvctl layout new <id> --from default`; pass `--layout <id>` to edit another
user layout.

`atvvctl map test` resolves the active layout by default, or `--layout <id>`,
or a single action string. It never executes `exec:`; it only validates and
prints the key program. A
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
| `SetConfig` | `(s) -> ()` | Validates version 2, applies, and persists. Invalid input returns `org.freedesktop.DBus.Error.InvalidArgs` with the field error. |
| `SetLayout` | `(s) -> ()` | Switches the active layout by id, applies it, and persists. |
| `BeginCapture` | `() -> ()` | Starts capture for the calling connection. |
| `EndCapture` | `() -> ()` | Stops capture. |
| `LayoutChanged` | signal `(s)` | New active layout id. |
| `ButtonEvent` | signal `(s, s, b)` | `button`, `trigger`, `pressed` while capture is active. |

`GetStatus` reports the active layout as `layout.id` and `layout.name`.
Capture mode ends automatically when the connection that called `BeginCapture`
leaves the bus. The payload is versioned by `schema_version`; clients must send
the complete version 2 document back to `SetConfig`, including every layout.
