# Step 23 — Audio feedback and session boundary

Status: ⬜ not-started
Depends on: 04, 06, 14, 19
Touches: `crates/atvv-audio/`, `crates/atvv-daemon/`, `crates/atvv-gui/`,
`docs/`

## Goal

Make the microphone legible and session-scoped:

1. expose a live input level in `StatusView.audio` and show it in the GUI;
2. add a gain control to the GUI so the level can be used to set gain;
3. clear the resident PCM queue when a new session starts, so a consumer never
   reads the tail of a previous session.

No protocol change, no new D-Bus method, and no provider-contract change. The
level is status data; the gain is the existing `gain_db` config field; the
queue flush is an internal correctness fix.

## Evidence

- The real RC003 run recorded `maximum amplitude 0.999969` and
  `RMS 0.356024` at gain 0 dB, so clipping on loud speech is plausible and a
  level display is needed to set gain (`docs/research/rc003-hardware-evidence.md`).
- `StatusView.audio` currently exposes `source`, `available`, `streaming`,
  `queued_samples`, `gain_db`, `frames`, `samples`, `dropped_frames`, and
  `degraded`; it has no level or peak
  (`crates/atvv-daemon/src/status.rs`).
- The GUI has no gain or level control; gain is changed only with
  `atvvctl config set --gain-db` (`docs/configuration.md`).
- `PcmQueue` is sized to one second of samples
  (`queue_capacity = profile.sample_rate`) and has no clear operation. The
  daemon's `on_audio_start` resets the accumulator, decoder, counters, and
  resampler, but not the queued PCM (`crates/atvv-audio/src/source.rs`,
  `crates/atvv-daemon/src/daemon.rs`). A session that streamed while no
  consumer was linked can therefore leave its tail for the next consumer.
- `docs/research/sayall-linux-comparison.md` §3.1 and §3.2 record the
  behavior-only reference: a decaying peak level, a gain slider, and a
  session-scoped pre-roll buffer. Do not copy its GPL-3.0-only code.

## Design

### Level

- Add `level: u16` to `AudioView`, in permille of full scale: `0..=1000`,
  `#[serde(default)]`. Keep `Eq` on the view models; do not add an `f32` field.
- In `Daemon::output_frame`, after gain (and before resampling), compute the
  frame peak over the written samples with `i32::from(sample).unsigned_abs()`,
  convert to permille, and update the envelope:
  `level = max(level * 6 / 10, frame_peak_permille)`.
  Clamp to `1000`. Use integer math; no allocation and no logging per frame.
- Reset `level` to 0 on `VoiceStarted` (before the first frame) and on
  `VoiceStopped`/disconnect. `publish_status` already runs on audio frames and
  state changes; make sure the zero is published when the session stops.
- The level reflects the post-gain signal that consumers receive. The
  resampler is identity for the RC003 graph rate; if it is not, document that
  the level is pre-resample.
- `atvvctl status`, `atvv-mcp status`, and the GUI read the same field; no
  separate CLI plumbing.
- GUI: add `AudioInfo` to the `DaemonStatus` mirror in
  `crates/atvv-gui/src/client.rs` with `level: u16` and `gain_db: i8`
  (`#[serde(default)]`). The existing 2 s status poll is too slow for a level
  bar: poll `GetStatus` at 200 ms only while the microphone dialog is open,
  keep at most one request in flight, and stop the timer when it closes. Do
  not change the global 2 s poll.

### Gain

- Add a **Microphone…** row to the header menu and an `AdwDialog` containing:
  - a `GtkLevelBar` driven by `level / 1000.0`;
  - a gain `GtkScale` from -24 to 24 dB, step 1;
  - the source name and format from `StatusView.audio`/`contract`;
  - a hint that the setting is saved with the main window's **Apply**.
- Add `GuiModel::set_gain_db(i8) -> Result<bool, String>`: validate the
  configured range, set `document.gain_db`, mark the document dirty, and do
  not touch the active layout. The existing **Apply** saves the whole
  document through `SetConfig`; **Revert** restores it. This keeps one write
  path and never writes `daemon.toml` directly.
- Do not add AGC. Gain remains the existing post-decode, pre-resample
  multiplication.
- Add the dialog labels and hints to `crates/atvv-gui/src/i18n.rs` in English
  and Simplified Chinese.

### Queue flush

- Add `PcmQueue::clear()` that drains the queue with `pop` until empty.
- Add `AudioSource::clear()` delegating to the queue.
- Add `clear()` to the `PcmSink` trait; `PipeWireSink` delegates when the
  source exists and is a no-op when degraded. Update every test sink.
- Call `self.audio.clear()` from `on_voice_started`, next to the existing
  accumulator/decoder reset. Do not clear on `VoiceStopped`: the consumer may
  still be draining the last frames; the next session start clears them.
- No flush is needed when the source is recreated for a profile change; that
  path already builds a new queue.

## Deliverables

- ⬜ `AudioView.level` with permille semantics, integer peak/decay, and a
  reset on start/stop/disconnect.
- ⬜ `AudioInfo` in the GUI client and a 200 ms status poll while the
  microphone dialog is open.
- ⬜ Microphone dialog with level bar, gain scale, source summary, and the
  Apply hint.
- ⬜ `GuiModel::set_gain_db` with range validation and dirty tracking.
- ⬜ `PcmQueue::clear`, `AudioSource::clear`, and `PcmSink::clear`; flush on
  `VoiceStarted`.
- ⬜ Tests: queue clear; daemon flush count on start/stop/start; level rise,
  decay, and zero on stop; gain model validation; GUI level/gain interaction.
- ⬜ Docs: `docs/gui.md` and `docs/gui.zh-CN.md` describe the level and gain
  control; `docs/configuration.md` points at it; the status field is mentioned
  in the GUI/CLI docs.
- ⬜ `docs/research/sayall-linux-comparison.md` §6 links this step.

## Verification

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
xvfb-run -a cargo test -p atvv-gui --test ui
```

Synthetic checks (no hardware required):

```text
cargo test -p atvv-audio clear
cargo test -p atvv-daemon level
cargo test -p atvv-daemon clear
cargo test -p atvv-gui --lib gain
```

Live GUI smoke with the mock daemon:

```text
dbus-run-session -- bash -c '
  cargo run -q -p atvv-daemon -- --mock &
  sleep 1
  cargo run -q -p atvv-gui
'
```

Open **Microphone…**, observe the level bar with a mock audio frame, change
gain, click **Apply**, reopen the daemon config with `atvvctl config get`, and
confirm the saved `gain_db`.

## Exit criteria

- ⬜ `StatusView.audio.level` is present, bounded to `0..=1000`, zero when
  idle, and updates from decoded frames.
- ⬜ The GUI shows a live level and can change `gain_db`; the value is saved
  through the existing Apply path and survives reopen.
- ⬜ A new session starts with an empty PCM queue; a stop does not discard
  frames the consumer has not read.
- ⬜ The level/gain UI is bilingual and follows the existing catalog.
- ⬜ No new D-Bus method, protocol field, or config schema field is added.
- ⬜ Tests cover the field, the flush boundary, the model setter, and the
  dialog interaction.
- ⬜ The docs describe the control and the queue behavior.

## Risks

- A per-frame level update must stay out of the logging path; only the status
  publish at 200 ms may carry it. Do not allocate in `output_frame`.
- The GUI currently polls status every 2 s. A 200 ms poll only while the
  dialog is open avoids a background D-Bus load; do not shorten the global
  poll.
- Applying gain through the existing document means a dirty template triggers
  Save-as-layout on Apply, as it already does for other edits. The dialog's
  hint must say so.
- `PcmQueue::clear` runs on the daemon control thread while the PipeWire
  process callback may pop. The queue is lock-free; a torn read is impossible,
  but a frame may be produced between the flush and the first new frame. That
  is acceptable; the queue is pre-roll, not an exact boundary.
- The level is pre-resample. For devices whose `CAPS_RESP` rate differs from
  the graph rate, the displayed peak may differ slightly from the final
  output. Document it; do not add a resampled peak unless a real device needs
  it.

## Progress log

- 2026-11-02 — plan created from
  `docs/research/sayall-linux-comparison.md` §3.1, §3.2, and §6.

## Next action

Implement `AudioView.level` and the `PcmSink::clear` path with tests first,
then the GUI client field, the microphone dialog, and the gain setter.
