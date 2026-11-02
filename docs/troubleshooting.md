# Troubleshooting

English | [简体中文](troubleshooting.zh-CN.md)

Start here when the remote, the source, or a consumer does not work. Run the
check first; it reports every layer in order:

```sh
atvvctl doctor
```

A healthy setup prints `ok` on every line. The GUI **Getting started…** dialog
shows the same four layers from `GetStatus`/`GetConfig`; its commands can be
copied. Neither check starts a service, opens Bluetooth, or writes a consumer's
configuration.

## `atvvctl` cannot reach the daemon

Symptom:

```text
atvvctl: org.freedesktop.DBus.Error.ServiceUnknown: The name org.atvv.Daemon was not provided by any .service files
```

`atvvctl doctor` reports `daemon fail` and skips the source checks. Check the
user unit and the log:

```sh
systemctl --user status atvvd --no-pager
systemctl --user enable --now atvvd
journalctl --user -u atvvd -n 50 --no-pager
```

Expected: `Active: active (running)` and one start line carrying the transport
and integration mode:

```text
atvvd: bluez transport for C0:5D:39:F8:2D:4C on org.atvv.Daemon /org/atvv/Daemon; vinput=trigger trigger=DICTATE
```

If the unit is running but the name is missing, the GUI stays on **Daemon
unavailable**; retry after the unit reports active. If the log stops at `no
device address`, continue with the next section.

## No remote configured, or the remote is disconnected

Symptom: `atvvctl doctor` reports `daemon fail` or `device fail`; the GUI
checklist marks **Device** missing; `atvvctl status` returns `"connected":
false`. The GUI's battery value reads `last known` while disconnected.

```sh
atvvctl device list
atvvctl pair C0:5D:39:F8:2D:4C
bluetoothctl info C0:5D:39:F8:2D:4C
atvvctl device use C0:5D:39:F8:2D:4C
```

`atvvctl device list` marks the active address with `*`:

```text
active: C0:5D:39:F8:2D:4C
* C0:5D:39:F8:2D:4C 小米蓝牙语音遥控器  miremote-rc003  custom  connected 60%
```

Expected from `bluetoothctl`: `Paired: yes`, `Trusted: yes`, and
`Connected: yes`. The daemon retries a dropped link; wake the remote with a key
press and watch `atvvctl monitor`. `atvvctl pair` writes the address to
`daemon.toml` unless `--no-save` is passed. Use `atvvctl device use <MAC>` to
switch between known devices without restarting the daemon.

## The source is missing from PipeWire

Symptom: `atvvctl doctor` reports `source fail` or `pipewire fail`; the GUI
checklist marks **Source** missing; a consumer does not list the source.

```sh
atvvctl status
pw-cli info atvv-miremote-rc003
pw-cli list-objects | grep atvv
```

Expected node facts:

```text
type: PipeWire:Interface:Node/3
node.name = "atvv-miremote-rc003"
media.class = "Audio/Source"
audio.rate = "16000"
atvv.contract_version = "1"
atvv.profile = "miremote-rc003"
atvv.recording_keys = "DICTATE,VOICECOMMAND,ASSISTANT"
```

`atvvctl status` reports `"audio": {"available": true, ...}`. If the daemon is
fine but the node is gone, restart the daemon; it recreates the source:

```sh
systemctl --user restart atvvd
atvvctl doctor
```

## The source exists but the consumer does not record

The source is resident, but it only carries data while the remote's voice key
is held down. A linked consumer does not open the microphone.

```sh
atvvctl contract --json
pw-link -l
```

`atvvctl contract --json` prints the source, declared keys, active key, and
the one-line consumer recipe. `pw-link -l` lists current links only, so an
empty result means no consumer is linked. Select `atvv-<profile>` in the
consumer and bind one declared recording key. fcitx5-vinput does not bind it
automatically:

- keep the user's existing `[TriggerKey]` entries and add `1=Dictate`
  (the default `0=Alt_R` may stay);
- stop fcitx5 before editing and start it fully afterwards;
  `fcitx5-remote -r` can keep the old trigger list loaded;
- a commented-out `[TriggerKey]` section disables the trigger.

The provider emits one active key: `DICTATE` by default, or the configured
`trigger_key`. See [`fcitx5-vinput.md`](integrations/fcitx5-vinput.md) and
[`provider-contract.md`](integrations/provider-contract.md). The GUI's
recording-key dialog and the `trigger_key` config value change that output; do
not add a second trigger binding in the consumer for the same key.

## The trigger key does not arrive, or F5 reaches the focused application

The remote's evdev node must be grabbed, and `uinput` must be writable so the
synthetic key can be emitted.

```sh
atvvctl status
atvvctl doctor
ls -l /dev/uinput
```

Expected in `atvvctl status`:

```text
"input": {"available": true, "enabled": true, "grabbed": ["/dev/input/event26"], "keyboard": true, ...}
```

`atvvctl doctor` prints `uinput ok /dev/uinput is writable`. If the node is not
grabbed, close the other program that holds it (another input daemon or a
debugger) and restart `atvvd`. If `/dev/uinput` is not writable, check the udev
`uaccess` ACL and log out and back in after installing the package:

```sh
getfacl /dev/uinput
```

`F5` reaching the focused application means the grab failed; the same check
applies. The GUI checklist marks **Input and trigger** missing when the
keyboard or the active key is unavailable.

## After a PipeWire service restart

The packaged unit is `PartOf=pipewire.service` with `Restart=always`, so systemd
restarts `atvvd` with PipeWire. The daemon does not restart PipeWire itself.

```sh
systemctl --user status atvvd --no-pager
atvvctl doctor
pw-cli info atvv-miremote-rc003
```

If the node is still missing after the unit reports active, restart the daemon
once (`systemctl --user restart atvvd`) and check the journal for
`atvv-audio: PipeWire source restart:` followed by the PipeWire error.

## Remote reconnect and stale device facts

The daemon keeps the session and retries the link after a disconnect. Battery,
firmware, and hardware facts from the last connection stay visible and are
marked `last known`; they are not live while `connected` is false.

```sh
atvvctl monitor
bluetoothctl info C0:5D:39:F8:2D:4C
```

Wake the remote and wait for `state=` to change in `atvvctl monitor`. If the
remote stays disconnected, power-cycle it, then check `bluetoothctl` and the
journal. A stale `last_error` is informational; `atvvctl doctor` is the current
state.

## No level, or the signal clips

The GUI **Menu → Microphone…** dialog shows the post-gain peak and a gain
slider. The same values are in `atvvctl status`:

```text
"audio": {"level": 0, "gain_db": 0, "available": true, "streaming": false, ...}
```

`level` is permille of full scale (`1000` = 0 dBFS), decays between frames, and
is measured after gain. A level that stays at `0` while `streaming` is true
means silence reached the source; check the physical voice key and the remote
link. A level that repeatedly reaches `1000` clips: lower the gain in the GUI
or with:

```sh
atvvctl config set --gain-db -6
```

Gain is applied after ADPCM decoding and before the source. The default is
`0 dB`; the allowed range is `-24..=24 dB` (see `MAX_GAIN_DB`). The value is
saved with the rest of the configuration and applied live.

## Logs and diagnostics

```sh
journalctl --user -u atvvd -f
atvvctl monitor
atvvctl doctor
atvvctl status
atvvctl contract --json
```

- `atvvd: bluez transport for <MAC> on org.atvv.Daemon ...` is the normal
  startup line; `vinput=trigger trigger=DICTATE` names the active integration.
- `atvvd: transport error: ...` and `atvvd: write failed: ...` appear with
  `--verbose`; the same text is in `atvvctl status` under `last_error`.
- `atvv-audio: PipeWire source restart: ...` means the source was recreated
  after the graph changed.
- `atvvctl doctor` checks the registry, daemon, source, PipeWire node,
  contract properties, hardware facts, BlueZ, device, ATVV service, uinput,
  vinput, and the configuration file in that order.

Do not add per-application checks here. The provider cannot see a consumer's
configuration; the integration guides are the next step:
[`docs/integrations/`](integrations/README.md).
