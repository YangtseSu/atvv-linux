# sayall-linux comparison and borrow list

Snapshot: `ZXC011/sayall-linux` at
`d2b914ae5fdeabc199ebb44a54227451ce458a00` (v0.3.0, 2026-10-18),
GPL-3.0-only. The snapshot was read from the upstream repository; no local
clone path is recorded here.

This is a different project from `fan-tastic-z/sayall-omarchy` (MIT), which is
the Rust/Omarchy/voxtype reference already covered in `landscape.md`. Do not
merge their rows, facts, or licenses.

Reuse boundary: GPL-3.0-only. This document records behavior, UX, and protocol
claims only. No code, asset, icon, or configuration file may be copied into
this GPL-3.0-or-later tree.

## 1. Verdict

The projects overlap, but they are not the same wheel.

- `sayall-linux` is an end-to-end, single-consumer product: Xiaomi remote ->
  Python bridge -> PipeWire virtual microphone -> Vokie dictation. It makes
  Vokie work by switching the default source and injecting Vokie's push-to-talk
  hotkey.
- `atvv-linux` is a provider stack: ATVV session and profiles, a native
  PipeWire `Audio/Source`, declared recording keys, D-Bus and MCP control, and
  a configuration GUI. Consumers own source selection and their own
  configuration (ADR 0007).

The technical core overlaps at the ATVV decoder, frame accumulator, HID grab,
and button mapping. The parts that make this repository larger - the session
state machine, multi-device registry, layouts, provider contract, D-Bus/MCP,
and hardware evidence - have no counterpart there.

What they do better is the last mile: a new user can install a `.deb`, open a
first-run checklist, see a live level meter next to a gain slider, and follow a
symptom-based troubleshooting page. That is the part worth studying.

Their snapshot is 3,249 lines of Python in two files and has no unit tests; CI
byte-compiles the scripts, checks shell syntax, and validates the desktop
entry. Our workspace is about 31k lines of Rust with 244 passing tests plus
hardware evidence. The comparison is therefore about product polish, not
implementation rigor.

## 2. Architecture comparison

| Dimension | sayall-linux | atvv-linux |
|---|---|---|
| Runtime | Python 3.10, PyGObject, `bleak`, `evdev` | Rust workspace, `bluer`, evdevil, libpipewire |
| ATVV transport | `bleak` over BlueZ D-Bus, one connect at startup | `atvv-bluez`, AcquireNotify with StartNotify fallback, pairing agent, reconnect |
| Protocol | CAPS, control events, ADPCM, frame accumulator | Pure `atvv-protocol`: CAPS, events, ADPCM, frame accumulator, session state machine, profiles |
| PipeWire | `pw-loopback` + `pw-cat` subprocesses | Native `Audio/Source` with a bounded lock-free queue |
| Mic lifecycle | `MIC_OPEN` at connect (`preopen_mic`), re-open after `0x00`, no keepalive | `MIC_OPEN` on the voice edge; `MIC_CLOSE` on host stop or non-HTT stop, suppressed for HTT release; `MIC_EXTEND` every 10 s while streaming |
| Input | Vendor/name match, F5/F20/REFRESH, swallow power, gesture timers | Profile VID/PID/key map, `EVIOCGRAB`, evdevil, uinput keyboard, gesture FSM, pointer actions |
| Consumer integration | Writes Vokie settings, injects its hotkey, switches the default source | Declares source properties and recording keys; consumer owns selection, trigger, and config |
| Control surface | `config.json`, `status.json`, `systemctl` | `org.atvv.Daemon`, `atvvctl`, `atvv-mcp`, D-Bus signals |
| Devices | RC001/RC003 family, hard-coded assumptions | Profiles + known-device registry + per-device layouts |
| GUI | GTK4/libadwaita; status, level, gain, mapping, onboarding | GTK4/libadwaita; mapping board, layouts, presets, recording key, test mode |
| Tests | None | 244 passing, 9 ignored; hardware evidence |
| Packaging | `.deb` build script and `install.sh` | AUR, release binaries, CI, cargo-deny, REUSE |
| License | GPL-3.0-only | GPL-3.0-or-later |

## 3. What they do better

### 3.1 Live input level next to gain

Evidence:

- `sayall_bridge.py::_on_audio` computes a per-frame peak and decays the
  displayed level with `level = max(level * 0.6, frame_peak)`.
- `status.json` is rewritten every 0.4 s with `level`, `recording`, `battery`,
  and other fields.
- `sayall_gui.py` refreshes at 600 ms and drives a `Gtk.LevelBar` with
  `min(1.0, level * 3)`; a gain slider writes `gain_db` to the config.

Our gap:

- `StatusView.audio` exposes `frames`, `samples`, `dropped_frames`,
  `queued_samples`, and `gain_db`, but no level or peak.
- The current GUI has no gain control and no level bar; gain is changed with
  `atvvctl config set --gain-db`.
- The RC003 hardware run recorded peaks near full scale, so users need a
  visible signal to set gain without clipping.

Borrow:

- Add a decaying peak (and optionally RMS or dBFS) to `AudioView`, updated in
  `output_frame` and published on the existing 200 ms cadence.
- Show a `Gtk.LevelBar` and a gain slider in the GUI; keep the value bounded
  and do not log per frame.
- Widen to `i32` before `abs` so `i16::MIN` is safe, as the verbose peak path
  already does.

This is the cheapest borrow and it directly addresses a documented hardware
fact.

### 3.2 Session-boundary pre-roll and queue flush

Evidence:

- `sayall_bridge.py` keeps a `pending` buffer of at most 2 s while the mic
  streams before the dictation app starts recording. It flushes the buffer
  when recording starts.
- The buffer is cleared on `AUDIO_START` and on stop, so audio from an earlier
  session cannot leak into the next one.

Our gap:

- `PcmQueue` is sized to one second of samples (`queue_capacity =
  profile.sample_rate`), and the daemon never clears it.
- `on_audio_start` resets the accumulator, decoder, counters, and resampler,
  but not the queued PCM.
- If a session streamed while no consumer was linked, the next consumer can
  read the tail of that session before the new audio. This is an inference
  from the queue implementation, not a recorded hardware failure; the queue
  should be flushed at the session boundary regardless.

Borrow:

- Add `clear()` to the PCM sink/source and call it on `VoiceStarted`.
- Do not clear on `VoiceStopped`: the consumer may still be draining the last
  frames. Clearing at the next start is enough.
- Keep the queue as the bounded pre-roll for the current session; document
  that it is not a cross-session buffer.
- Test it at the queue level: push prior-session samples, start a session,
  assert the queue is empty, then push current samples and read them back.

This is a correctness fix, not a feature copy.

### 3.3 Optional keep-open microphone

Evidence:

- `preopen_mic` defaults to true. The bridge sends `MIC_OPEN` after connect and
  re-opens after every `AUDIO_STOP` while not recording.
- `docs/PROTOCOL.md` says a `MIC_OPEN` session expires after about 60 s and
  that waiting for `0x04` before opening is "one beat late".
- Our fcitx guide measured roughly 0.3 s after pressing as a safe wait; the
  2026-11-03 RC003 run measured a 157 ms median from the voice press to the
  first decoded frame with the default voice-edge `MIC_OPEN`.

Our gap:

- We open the mic on the voice edge. This is correct for privacy and works on
  the RC003, but it leaves a documented first-word race.

Borrow, as an experiment:

- Design an "armed" state separate from dictation: open at ready, extend while
  armed, re-open after an unexpected `AUDIO_STOP`, and never publish
  `VoiceStarted`/`VoiceStopped` for an armed session.
- Keep it opt-in until a real RC003 A/B measures time-to-first-non-silent
  frame and remote battery impact.
- Do not copy their lifecycle wholesale: they send no `MIC_EXTEND`, so a hold
  longer than the device's session window is not protected. Our 10 s keepalive
  already covers a 59.3 s hold.

Status (2026-11-03): the RC003 A/B passed and the path is implemented as the
opt-in `preopen_mic` setting (default `false`). The median voice-press to
first-frame latency improved by 169 ms, the armed session stays invisible to
consumers, and the 10-minute idle battery delta matched the baseline. See
[`rc003-hardware-evidence.md`](rc003-hardware-evidence.md#keep-open-mic-ab-2026-11-03).

Risks:

- Constant BLE traffic and a permanently open remote session.
- State-machine complexity: `streaming` would no longer mean "the user is
  dictating", which the D-Bus view model and consumers currently assume.
- On the RC003 the key still gates audio (a host-only `MIC_OPEN` produces no
  frames), so this is latency, not an always-listening microphone. State that
  explicitly in any user-facing description.

### 3.4 First-run setup checklist in the GUI

Evidence:

- `OnboardingDialog` checks four conditions with live dots: Vokie installed,
  remote paired, bridge active, hotkey configured. It refreshes every 1.5 s
  and offers direct action buttons.
- The dialog can be reopened from the menu, so it doubles as a support tool.

Our gap:

- The GUI has a daemon-unavailable page and a device dialog, but no guided
  sequence that tells a new user which layer is missing.
- Our integration docs are accurate but are reference material, not a
  first-run flow.

Borrow, generically:

- A read-only checklist in `atvv-gui`: daemon reachable; device connected;
  source available with contract properties; input keyboard ready and a
  trigger key configured; consumer selected (or a link to the consumer
  guides).
- Reuse `StatusView` and `contract`; do not add per-application detection.
- Keep the GUI a D-Bus client. Do not start services, write another app's
  config, or add Vokie-specific checks.
- Ship English and Simplified Chinese together.

This matches the provider direction: help the user find the missing boundary,
without inventing per-application control paths.

### 3.5 Troubleshooting and known-limitations documentation

Evidence:

- `docs/TROUBLESHOOTING.md` is organized by symptom and gives the exact
  command and log signature, for example "F5 reaches the browser", "no text",
  "WeChat does not paste", "uinput permissions", and "logs".
- The README carries an explicit "Known limitations" section, including the
  upstream Vokie Wayland paste bug and the unverified RC001.

Our gap:

- The README has a troubleshooting table, but there is no single page with
  the commands, expected output, and log lines for the common failures.
- Consumer-specific detail lives in `docs/integrations/`, which is correct;
  the generic recovery path is scattered.

Borrow:

- Add `docs/troubleshooting.md` and `.zh-CN.md`: daemon not running, source
  missing, source present but consumer not recording, trigger key conflict,
  `uinput` permissions, PipeWire restart, remote reconnect, and `doctor`
  output.
- Link to the integration docs and hardware evidence instead of duplicating
  them.
- Add a short "Known limitations" section to the README that points at the
  device-support matrix and the evidence files.

### 3.6 Safe app launch action (defer)

Evidence: their mapping supports `app:<desktop-id>` and launches it with
`gio launch`, without a shell.

Our gap: the action grammar has `key:`, `combo:`, `exec:`, `dictation:`,
`pointer:`, and `none`. Launching an app requires `exec:` and `allow_exec`.

Opinion: defer. `exec: gtk-launch <id>` covers the case, and adding a second
launch path expands validation, the GUI editor, and the security policy. If
users ask for it, a structured `app:` action is safer than shell execution and
can fail closed on an unknown desktop id.

## 4. What not to borrow

- **Default-source switching.** `voice_down`/`voice_up` change the system
  default source and restore it. This is racy, affects every application, and
  contradicts ADR 0007: consumers own source selection. Do not add it as a
  convenience path.
- **Writing Vokie's configuration and injecting its hotkey.** Their bridge
  reads and writes `~/.config/vokie/app-settings.json` and synthesizes the
  configured chord. That is consumer-owned configuration and a per-app control
  path; our contract is the declared recording key and a standard source.
- **Swallowing `KEY_POWER`.** `SWALLOWED_KEYS` includes power. It may avoid an
  accidental suspend, but it also removes a key the user may expect to work.
  No evidence justifies it here.
- **`pw-loopback` + `pw-cat` subprocess audio.** It works, but it adds two
  processes, a `pw-dump` lookup, and a pipe into the audio path. A native
  source is a core design decision here.
- **GUI writes the shared config and the bridge polls mtime.** Our
  `SetConfig`/atomic document path is the correct model; do not add a second
  writer.
- **Numeric key codes as the mapping key.** Names and profiles are portable
  and validated; `known_keys` is a compatibility patch, not a schema.
- **Their CAPS parser fallback.** It accepts a non-canonical v1.0 shape when
  `codecs` is zero. Our parser treats CAPS as authoritative and validates; do
  not loosen it based on this project.
- **Their protocol document as hardware fact.** `docs/PROTOCOL.md` says
  v1.0 `MIC_CLOSE` is `0D <session_id>` and `MIC_EXTEND` is `0E <session_id>`.
  Our RC003 evidence records bare `0D`/`0E`. Their bridge calls `_mic_close`
  nowhere and never sends `0E`; the `send_mic_commands` flag is assigned but
  never read, so it does not gate either command. The RC003 profile and the
  recorded evidence win.
- **Their `trigger_mode` as a fallback.** The config defaults to `atvv`, but
  the HID path still calls the same `voice_down`/`voice_up` handlers and the
  bridge exits if the HID device cannot be grabbed. It is belt-and-braces, not
  a way to run without input access.
- **Their code or assets.** GPL-3.0-only; behavior only.

## 5. Where this repository is already ahead

- The session state machine handles direct `AUDIO_START`, `AUDIO_STOP`
  reasons, stop echoes, host-initiated close, and `MIC_EXTEND` with recorded
  hardware evidence.
- Profiles keep device facts out of control flow; the known-device registry
  and per-device layouts have no counterpart there.
- The provider contract, `org.atvv.Daemon`, `atvvctl`, and `atvv-mcp` make the
  stack callable by fcitx, browsers, agents, and IM clients without writing
  their configuration.
- Native PipeWire, bounded queues, resampling, node-removal recovery, and
  systemd ownership of service restart are more robust than subprocess audio.
- Tests, CI, packaging, REUSE, cargo-deny, and hardware evidence are stronger.

## 6. Suggested order

| Priority | Item | Size | Gate |
|---|---|---|---|
| P1 | Audio level in `AudioView` and the GUI, plus a gain control | small | synthetic queue/Sink test and a GUI test |
| P1 | Flush the PCM queue on `VoiceStarted` | small | queue-level test; no hardware needed |
| P1 | `docs/troubleshooting.md` + `.zh-CN.md` | small | link check, no runtime |
| P2 | First-run checklist in `atvv-gui` | medium | GUI test with a mock daemon; generic checks only |
| P2 | Opt-in keep-open microphone | done | RC003 A/B passed; `preopen_mic` defaults to false |
| P3 | Structured `app:` action | small | only if users ask; security review of the action grammar |

The P1/P2 items are scheduled: [step 23](../plans/23-audio-feedback.md)
covers the level, gain, and session-boundary flush;
[step 24](../plans/24-first-run-and-troubleshooting.md) covers the checklist,
troubleshooting page, and known-limitations sections. The keep-open experiment
passed the 2026-11-03 RC003 A/B and is implemented as opt-in `preopen_mic`
([step 26](../plans/26-preopen-mic.md)). The `app:` action stays unscheduled
until user demand exists.

## 7. Open questions

- Does a consumer ever observe stale queue audio in practice? The flush is
  cheap either way; a queue-level test should prove the behavior before a
  hardware claim.
- Is the guidance in the fcitx guide causing clipped first words on the real
  device? Answered 2026-11-03: the default voice-edge path has a 155.6 ms
  median, and an armed `preopen_mic` session has the first frame queued when
  the key edge arrives. Consumer start-up remains outside the measurement.
- Can Vokie select a capture device or bind `Dictate`? If not, document it as
  an unsupported consumer instead of adding default-source switching.
