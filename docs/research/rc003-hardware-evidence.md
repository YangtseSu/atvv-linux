# RC003 hardware run — 2026-10-30

Status: first real-device run for step 07. This file records the observed
ATVV behavior of one Xiaomi Bluetooth Remote 2 Pro (RC003). It supplements the
field-report values in [`protocol-and-device.md`](protocol-and-device.md).

## Environment

| Item | Value |
|---|---|
| Kernel | `7.3.0-3-cachyos` |
| BlueZ | 5.87 (`bluetoothctl --version`) |
| PipeWire | 1.6.10 (`pw-cli --version`) |
| Adapter | `hci0`, `04:ED:33:51:52:FB`, public |
| Device | `C0:5D:39:F8:2D:4C`, 小米蓝牙语音遥控器 (RC003) |
| Device state | paired, bonded, trusted, `AddressType: public`, `LegacyPairing: no` |
| Tools | `target/debug/atvvd --verbose`, `atvvctl`, `busctl --system`, `pw-cat` |

Everything ran as the logged-in user: no root, no kernel module, no BlueZ or
PipeWire patch.

## GATT layout

`busctl --system tree org.bluez` and
`org.freedesktop.DBus.Properties.GetAll` on the ATVV objects:

| Role | UUID | Handle | Flags |
|---|---|---|---|
| Service | `ab5e0001-5a21-4f05-bc7d-af01f617b664` | — | primary |
| TX | `ab5e0002-5a21-4f05-bc7d-af01f617b664` | 769 | `write-without-response`, `write` |
| Audio | `ab5e0003-5a21-4f05-bc7d-af01f617b664` | 771 | `notify` (CCCD present) |
| Control | `ab5e0004-5a21-4f05-bc7d-af01f617b664` | 774 | `notify` (CCCD present) |

BlueZ reported a negotiated MTU of **242** for all three characteristics
after connecting. Before the link was established, a cached 247 was visible.

`atvvctl doctor` prints the same service, flags, and MTU.

## Pairing

The remote was already bonded from desktop use. `atvvctl pair
C0:5D:39:F8:2D:4C --no-save`:

```text
address: C0:5D:39:F8:2D:4C
name: 小米蓝牙语音遥控器
already_paired: true
paired: true
trusted: true
```

No agent callback was invoked because no pairing exchange was started. The
auto-confirm agent (`DisplayYesNo`, numeric comparison) is implemented but was
not exercised; the RC003 pairing gesture is still unverified.

`atvvctl pair` without `--no-save` wrote `device = "C0:5D:39:F8:2D:4C"` to
`~/.config/atvv/daemon.toml`. `atvvd` then started without `--device`.

## CAPS_RESP

Verbose control trace:

```text
control [0b, 01, 00, 02, 03, 00, 78, 00, 00]
caps v1.0 codecs=0x02 interaction=HoldToTalk frame=120
```

- The notification is **9 bytes**: the documented 7-byte layout plus two
  trailing `00 00` bytes. The parser reads the first 7 and ignores the rest.
- version `01 00`; codec mask `0x02` (ADPCM 16 kHz only); interaction `0x03`
  (hold-to-talk); frame size `0x0078` = 120 bytes.
- The profile values for version, codec, interaction, and frame size match the
  real device.

## Start and stop

```text
control [04, 03, 02, 03]   # AUDIO_START reason=HTT, codec=16k, stream=3
control [04, 03, 02, 04]   # later session; stream id increments
control [00, 02]           # AUDIO_STOP reason=HTT button release
control [00, 08]           # AUDIO_STOP reason=transfer timeout (link loss)
```

- `START_SEARCH` was not observed in any session.
- `AUDIO_STOP 00 02` emitted `VoiceStopped` and **no** `MIC_CLOSE`, as the
  session rules require for an HTT button release.
- The stop echo described by field reports was not observed after `00 02` in
  the same notification stream; the 300 ms post-stop discard was not
  exercised because no late audio arrived.
- `AUDIO_STOP 00 08` arrived once during a deliberate link loss.

## Host-initiated `MIC_OPEN`

`atvvctl mic start` sent `0c 00`. The device answered `AUDIO_START` after
roughly 300–400 ms but sent **no audio notifications** until the physical
voice key was pressed. `atvvctl mic stop` while opening sent `0d`, and the
delayed `AUDIO_START` still arrived.

Consequence: on the RC003 the microphone data path is gated by the voice key.
A host-only `MIC_OPEN` opens the session but does not produce frames. Step 08
must synthesize the voice key, not just call `StartVoice`.

## Audio

- Frames arrived whole at the negotiated MTU; the accumulator produced
  120-byte frames. First traced frame prefix:
  `ff ff fb a2 a1 80 80 80 80 88 08 08 80 08 08 19`, decoded peak 1053.
- A ~4.8 s press produced 318 frames / 76,320 samples (4.77 s of 16 kHz PCM).
- A ~59.3 s press produced 3,954 frames / 948,960 samples, with no dropped
  frames and no errors.
- `atvvctl record --wav /tmp/rc003-keepalive.wav --seconds 65`:
  `soxi` reports 1 channel, 16 kHz, 16-bit, 65.00 s; `sox stat` reports
  maximum amplitude 0.999969 and RMS 0.356024. The capture is non-silent
  speech.
- No ASR/transcription was run and the file was not auditioned by a human in
  this session. "Intelligible/transcribable" is therefore not independently
  verified, only that loud speech-like PCM was captured.
- Gain stayed at 0 dB. The peak is near full scale, so clipping on loud speech
  is plausible; no AGC was added and gain remains configurable.

## Keepalive

While streaming, the daemon sent `0e` every 10 s. The 60-second hold logged
six `keepalive MIC_EXTEND` sends and ended only on key release
(`AUDIO_STOP 00 02`). No transfer-timeout stop occurred during the hold.

## Reconnect

`bluetoothctl disconnect C0:5D:39:F8:2D:4C` produced:

```text
disconnected: device event stream ended
state: disconnected
reconnecting
connected to 小米蓝牙语音遥控器
state: connected
```

In one run the first reconnect dropped again immediately (`disconnected: link
lost`) and a second attempt reached `ready` with a fresh `CAPS_RESP`. No
daemon restart was needed; after reconnect the daemon settled at ~0.6 % CPU.

Two bugs were found and fixed during this run:

- `tokio::time::sleep` was constructed outside the runtime in
  `BluezLink::stop_tasks`; the first reconnect panicked the daemon.
- A notification socket at EOF returned empty reads forever. The pump spun at
  ~82 % CPU and ignored task aborts. Empty reads are now bounded and yield,
  and a link generation prevents stale tasks from publishing events after a
  reconnect.

## PipeWire restart

`systemctl --user restart pipewire` left the daemon's source node gone. The
daemon did not recreate it in-process within 8 s and kept reporting
`audio.available = true` because the sink handle still existed. Restarting the
daemon recreated the source and recording worked again.

ADR 0005 assigns full PipeWire service restart to systemd
(`PartOf=pipewire.service`, `Restart=always`), not to `atvv-audio`. The unit is
not installed yet, so this path belongs to step 09. The stale
`audio.available` value during that window is a known status inaccuracy.

## Virtual source scheduling fix

The resident `Audio/Source` stayed suspended whenever no playback stream drove
the graph, and consumers recorded digital silence.

- Without `node.pause-on-idle=false`, the node stays `suspended` with no active
  driver; a linked consumer did not wake it in this environment.
- With `node.pause-on-idle=false`, the node starts suspended, becomes
  `running` when a consumer links, and delivers data. Synthetic 440 Hz tone:
  `pw-cat --record --target atvv-test` captured max 0.2009, RMS 0.1222.
- `node.always-process=true` is not a valid fix: the node freewheels
  (`pw-top` rate 0), drains the queue while unlinked, and consumers then
  record silence.

## Input

- The voice key is `KEY_F5` on `/dev/input/event25`; the daemon grabbed that
  node.
- evdev autorepeat (`value = 2`) previously produced a flood of
  `voice key pressed` lines. Repeats are now ignored for mapped keys and
  forwarded for unmapped keys, so the compositor still sees them.
- On link loss the kernel logs `Failed to ungrab device: No such device`; the
  input thread reports the lost device and exits cleanly.

## Not verified on hardware

- Pairing gesture and the numeric-comparison prompt (device was already
  bonded).
- Notification fragmentation and `0x0A` sync frequency (first frames arrived
  whole; sync traffic was not counted).
- Stop-echo timing (no echo observed after `AUDIO_STOP 00 02`).
- Human or ASR transcription of the captured speech.
- Full PipeWire service restart through the installed systemd unit (step 09).

## Step 08 trigger and fcitx checks (2026-10-30)

Second run on the same RC003 (`C0:5D:39:F8:2D:4C`), this time exercising the
step 08 input/IME path. `atvvd` used the real BlueZ transport; the resident
source was `atvv-miremote-rc003`; `vinput device list` marked it active after
`vinput device use atvv-miremote-rc003` and a `vinput-daemon` restart.

One physical voice-key press was observed with `trigger_key = "ctrl+alt+v"`
(fcitx's vinput trigger was temporarily set to `Control+Alt+V`):

```text
control [04, 03, 02, 05]
voice started
state: streaming
first frame 120 B raw [77, 72, 80, 98, 09, 81, 88, 09, 89, 90, 18, 13, 00, 10, 36, 30] peak 267
voice key pressed
voice is already active
control [00, 02]
voice stopped
state: ready
voice key released
```

`atvvctl status` afterwards reported 141 decoded frames, state `ready`, and
`input.voice_key = false`. The device sent the direct `AUDIO_START` itself; the
host `MIC_OPEN` triggered by the voice edge was therefore a no-op ("voice is
already active"), and the release `AUDIO_STOP 00 02` closed the session without
a host `MIC_CLOSE`.

A GNOME custom keybinding `<Control><Alt>v` (command `touch
/tmp/atvv-alt-r`) fired from the synthetic chord, proving that the trigger
program reached the compositor.

The same press did **not** start a vinput recording: the vinput-daemon journal
showed no `negotiated format` entry after the ATVV source had been selected,
so fcitx did not consume the trigger in that run. An earlier test with the
desktop's existing vinput trigger `Control+Control_R` (with the default source
still selected) did start recording and negotiated
`rate=16000 channels=1 fmt=259`; that is evidence for the fcitx trigger path,
not for the ATVV source.

The fcitx + ATVV-source leg was then verified automatically, without an
operator and without a time window, by running a mock-transport daemon
(`input = false`) alongside the real session PipeWire, vinput-daemon, and
fcitx:

- `vinput device use atvv-miremote-rc003` marked the resident source active.
- `vinput recording start` reported `recording`; the journal showed
  `using PipeWire target.object=atvv-miremote-rc003 (stream.capture.sink=false)`
  and `negotiated format: rate=16000 channels=1 fmt=259`.
- A concurrent `pw-cat` capture was 8.00 s / 128,000 samples at 16 kHz with
  peak 32767, RMS 14025.9, and 48,939 non-zero samples (the mock's
  deterministic ADPCM pattern).
- `CancelOperation` returned the daemon to `idle` without requesting ASR.
- A focused `zenity --entry` window received a synthetic
  `Control+Control_R` from `ydotool`; `vinput daemon status` became
  `recording`, the ATVV daemon was streaming (101 frames / 24,240 samples),
  and the journal repeated the `target.object=atvv-miremote-rc003` and 16 kHz
  negotiation. `CancelOperation` stopped it before inference.

The real press and the automated run are complementary: the press verifies the
RC003 voice-key edge, real ATVV audio, and the synthetic trigger reaching the
compositor; the automated run verifies the fcitx addon and the ATVV source.
No time-boxed verification was used or should be added. All temporary changes
(fcitx trigger, daemon `trigger_key`, vinput capture device, GNOME custom
keybinding, `ydotoold`, mock daemon) were restored after the runs.

## Step 09 packaging checks (2026-10-30)

Release binaries from this tree (`cargo build --release --locked --workspace
--bins`) were run against the same RC003 (`C0:5D:39:F8:2D:4C`, `hci0`) as a
transient systemd user unit with the packaged unit's properties
(`PartOf=pipewire.service`, `After=pipewire.service`,
`Wants=pipewire.service`, `Restart=always`). All output below was observed as
user `yangtse`; no root was used.

```text
atvvd: notifications via acquire-notify
atvvd: connected to 小米蓝牙语音遥控器 (C0:5D:39:F8:2D:4C)
atvvd: send [0a, 01, 00, 00, 03, 03]
atvvd: control [0b, 01, 00, 02, 03, 00, 78, 00, 00]
atvvd: caps v1.0 codecs=0x02 interaction=HoldToTalk frame=120
atvvd: state: ready
```

- `atvvctl doctor`: `daemon`, `source`, `pipewire`, `bluez`, `device`,
  `atvv` (tx/audio/control MTU 242), `uinput`, `vinput`, and `config` all
  `ok`.
- `atvvctl status`: `input.grabbed=["/dev/input/event25"]`,
  `keyboard=true`, trigger `Alt_R`.
- `pw-cli info atvv-miremote-rc003`: `Audio/Source`, 16 kHz, 1 channel,
  `node.virtual=true`, `node.pause-on-idle=false`.
- `atvvctl record --seconds 1`: WAV 1 channel, 16 kHz, 16-bit, 16,000 samples.
  RMS 0 because the physical key was not held; this verifies the source
  format, not intelligibility.
- `atvvctl mic start`: `AUDIO_START 04 00 02 00`, `voice started`, then
  `audio frame timeout; closing the stream` after about 2 s and one
  `MIC_CLOSE [0d]`. No frames arrived without the voice key. The 60-second
  keepalive remains the step 07 result (six `MIC_EXTEND` sends).
- `bluetoothctl disconnect C0:5D:39:F8:2D:4C`: `disconnected` →
  `reconnecting` → connected with a fresh CAPS and `ready`.
- `systemctl --user restart pipewire.service`: the unit was stopped and
  restarted (`MainPID` 85063 → 87720); `atvvctl status` returned to `ready`,
  the source was recreated at 16 kHz, and the journal showed a fresh CAPS.
- The package was built (`atvv-linux 0.1.0-1`, 5.8 MiB, plus a debug package)
  and installed into a staging root with `pacman --root`/`fakeroot`. The
  packaged `atvvd` started as a user service from the staging root and
  `atvvctl doctor` was all `ok`. A temporary copy of the packaged user unit
  with `ExecStart` pointing at the staged binary was `enable`d and `start`ed
  successfully, then removed.
- `namcap` reported only informational warnings: glibc/libpipewire/libgcc are
  implicitly satisfied, and `pipewire`/`bluez` are runtime services rather
  than link-time dependencies.

## Post-reboot lazy-connect fix (2026-10-30)

The first installed package (`0.1.0-1`) entered a systemd restart loop after
reboot: `BluezLink::connect` returned `org.bluez.Error.InProgress` from a
pending BlueZ connect, `atvvd` exited, and `org.atvv.Daemon` never became
activatable. The remote was asleep, so the loop repeated.

`0.1.0-2` makes `BluezTransport` lazy (the daemon starts without a link),
schedules an immediate reconnect, treats `InProgress`/`AlreadyConnected` as a
pending/successful connect, and bounds the whole connect operation. The input
runtime is attached when the HID-over-GATT node appears (retried every 2 s if
discovery is early), and dropping it releases the synthetic trigger. With the
release binary and the remote still disconnected, `atvvctl doctor` reported
`daemon ok state=disconnected`, `source ok`, and `pipewire ok`; the daemon
stayed active and logged `reconnect failed: connect timed out after 10s` on
each retry. Waking the remote remains necessary for the physical link.



## Earlier dedicated-key experiment (2026-10-30)

Follow-up to the package and trigger tests on the same RC003
(`C0:5D:39:F8:2D:4C`, `hci0`), fcitx5-vinput 2.4.2, fcitx5 5.1.23:

- The RC003 voice key produced 104 decoded frames / 24,960 samples in
  `atvvctl status`, so the ATVV path was live.
- `evtest /dev/input/event26` (the daemon's `atvv-virtual-keyboard`) showed
  `KEY_RIGHTALT` press/release for the configured `Alt_R` trigger.
- vinput stayed `idle` with an explicit `[TriggerKey] 0=Alt_R`, and also with
  the section commented. The `Control+Control_R` chord with daemon
  `trigger_key = "ctrl+rightctrl"` worked.
- A dedicated regular key was then verified: the virtual keyboard emitted it,
  and vinput started recording when that key was in its trigger list. Step 11
  standardized on `DICTATE`; this section is historical.

Step 09 later installed the package into `/usr`, reran the post-reboot check,
and recorded the end-to-end fcitx text commit in
[`docs/acceptance/rc003-2026-10-30.md`](../acceptance/rc003-2026-10-30.md).

## Step 11 evdevil and DICTATE trigger (2026-10-30)

Same RC003 (`C0:5D:39:F8:2D:4C`, `hci0`), fcitx5 5.1.23, and
fcitx5-vinput 2.4.2. The step-11 workspace `atvvd` ran with a temporary
config (`trigger_key = "DICTATE"`); the installed `0.1.0-3` unit was stopped
for the run and restarted afterwards.

- `atvvctl status`: connected, state `ready`, input grabbed
  `/dev/input/event25`, `keyboard=true`, `vinput.trigger=DICTATE`.
- `evtest` on the daemon's `atvv-virtual-keyboard` showed event code 586
  (`KEY_DICTATE`) press and release during a physical press.
- Status sampler during the press: `streaming=true`, frames 0 → 104, samples
  0 → 24,960, `voice_key` true → false, no `last_error`.
- fcitx key trace (`--verbose default=5,key_trace=5`) logged
  `Key(Dictate)` press/release (keycode 594) in the focused window.
- `vinput-daemon` journal: `target.object=atvv-miremote-rc003`,
  `rate=16000 channels=1 fmt=259`, then
  `recognition error: returned no text` because nothing was spoken.
- A synthetic `ydotool` event with code 586 was not delivered to fcitx in
  this session, while the daemon's evdevil virtual keyboard was. Use a real
  press or an evdevil-based emitter for consumer-key checks.
- fcitx rewrites `~/.config/fcitx5/conf/vinput.conf` on addon exit; edit the
  file while fcitx is stopped or the change can be overwritten.

## Test-mode capture and Home/Menu map (2026-10-31)

Same RC003 (`C0:5D:39:F8:2D:4C`, `hci0`) with the workspace `atvvd` and
`atvv-config` real-device test. The installed unit was stopped for the run
and restarted afterwards.

- `evtest /dev/input/event25` descriptor: `KEY_HOME` (102) and `KEY_COMPOSE`
  (127) are advertised; `KEY_HOMEPAGE` and `KEY_MENU` are not. The profile now
  maps the home usage (0x4A) to `KEY_HOME` and the application/menu usage
  (0x65) to `KEY_COMPOSE`. Earlier `KEY_HOMEPAGE`/`KEY_MENU` assumptions never
  matched the Linux HID node.
- Test mode (`BeginCapture` plus `ButtonEvent`) observed all thirteen physical
  keys: Power, OK, Voice, Up, Right, Left, Down, Back, Home, Volume up,
  Volume down, Menu, and TV. The voice result was
  `Voice · single → Microphone key (not remappable)`.
- `test_active=true` after pressing OK, and `wpctl get-volume` stayed at
  `0.37` across the volume presses. Capture now suppresses automatic
  key/chord emission, `exec:`, `dictation:`, synthetic voice-trigger emission,
  and voice-session start/stop; `RunAction` remains the explicit execution
  path. Release edges are forwarded so the GUI clears the pressed highlight.
- The first run with the old profile showed Up/Right/Left/Down/OK/Volume/Back/
  TV/Power/Voice but no Home/Menu; the profile correction is the fix.

## Battery and Device Information (2026-10-31)

Same RC003 (`C0:5D:39:F8:2D:4C`, `hci0`), BlueZ 5.87, workspace `atvvd` on a
private session bus. The installed user unit was stopped for the run and
restarted afterwards.

`org.bluez.Battery1` is present on the device object while the remote is
connected; BlueZ caches the GATT Battery Service and exposes `Percentage` and
`Source`:

```text
busctl --system get-property org.bluez \
  /org/bluez/hci0/dev_C0_5D_39_F8_2D_4C org.bluez.Battery1 Percentage
# y 62
```

The GATT Device Information Service (`0x180A`) is readable while connected:

| Characteristic | Value |
|---|---|
| `0x2A29` Manufacturer Name | `MIOM` |
| `0x2A24` Model Number | `RC003` |
| `0x2A25` Serial Number | `250519` |
| `0x2A27` Hardware Revision | `V2.0` |
| `0x2A26` Firmware Revision | `2671` |
| `0x2A28` Software Revision | `A.7.0.6` |

The workspace daemon read the same values through `bluer` during `connect`
and exposed them in `StatusView.hardware`. `atvvctl status` returned:

```json
"hardware": {
  "battery_percent": 62,
  "firmware_revision": "2671",
  "hardware_revision": "V2.0",
  "manufacturer": "MIOM",
  "model": "RC003",
  "serial_number": "250519",
  "software_revision": "A.7.0.6"
}
```

`atvvctl doctor` printed:

```text
hardware   ok   battery 62% · firmware 2671 · hw V2.0 · sw A.7.0.6 · serial 250519
```

The battery is a cached BlueZ property, not a per-poll GATT read. DIS
characteristics are read once per connection. Both are passive: the daemon
never writes to these services.
