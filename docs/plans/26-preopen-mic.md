# 26 — Keep-open microphone

Status: ✅ done
Depends on: 02, 04, 06, 07
Touches: `crates/atvv-daemon/`, `crates/atvv-config/`, `crates/atvv-cli/`,
`docs/research/`, user docs

## Goal

Measure whether pre-opening the ATVV microphone session removes the documented
first-word delay on the RC003, and add an opt-in `preopen_mic` setting only if
the measured benefit justifies the lifecycle cost. Promote the backlog
experiment B03 to a scheduled step, run it on the real device, and either
implement the opt-in path or close it with evidence.

The gate passed: the opt-in `preopen_mic` setting (default `false`) is
implemented. See the A/B result below.

## Evidence

- Before this step the daemon opened `MIC_OPEN` on the grabbed voice-key edge.
  The fcitx guide told users to wait roughly 0.3 s after pressing before
  speaking, which was the delay this experiment targets.
- The RC003 sends a direct `AUDIO_START` on the voice key and gates microphone
  data on the physical key. A host-only `MIC_OPEN` opens the session but
  produces no frames until the key is held
  (`docs/research/rc003-hardware-evidence.md`).
- The reference project pre-opens the microphone and re-opens after
  `AUDIO_STOP`; its `preopen_mic` default is true
  (`docs/research/sayall-linux-comparison.md` §3.3). It sends no
  `MIC_EXTEND`, so its lifecycle does not cover a long hold. Its code was not
  copied (GPL-3.0-only).
- Our keepalive already extends a streaming session every 10 s and completed a
  59.3 s held press. The open question was latency, not the 60 s hold.
- The official v1.0 spec adds three constraints the experiment had to respect
  (`docs/research/google-atvv-1.0-spec.md`):
  - a second `MIC_OPEN` while the microphone is already open restarts the
    stream after a previous `MIC_OPEN`, and returns `MIC_OPEN_ERROR(0x0F80)`
    during an ongoing PTT/HTT interaction;
  - `0x0F80` must not interrupt a `Streaming` session;
  - Audio Transfer Timeout is 15 s–1 min and `MIC_EXTEND` may be sent every
    5–10 s, so the 10 s keepalive is inside the normative range.

## Design

### Armed state

`preopen_mic` defaults to `false`. When it is enabled:

- the daemon sends `MIC_OPEN` when the session reaches `ready` and sends
  `MIC_EXTEND` every 10 s while armed;
- the device's `AUDIO_START` prepares the decoder silently: consumers do not
  receive `VoiceStarted`/`VoiceStopped`, `atvvctl status` keeps
  `state = "ready"` and `streaming = false`, and `mic_armed` is `true`;
- the voice press publishes `VoiceStarted` and emits the configured trigger;
  the release publishes `VoiceStopped` without sending `MIC_CLOSE`; the
  device's `AUDIO_STOP 00 02` returns the session to `ready`, is not
  consumer-visible, and schedules a re-open after 200 ms;
- the frame timeout does not close an idle armed session;
- `MIC_OPEN_ERROR(0x0F80)` in `Streaming` is ignored; a failed open in
  `Opening` returns to `ready` and retries after a 1 s floor;
- `preopen_mic` is inert for a profile whose voice key has no local evdev edge
  (`trigger_from_session`, the Chromecast): pre-opening would suppress the
  `AUDIO_START` that synthesizes its consumer trigger;
- manual `StartVoice`/`StopVoice` starts and ends a consumer-visible recording
  on the armed session without closing it;
- toggling `preopen_mic` live arms immediately when enabled and sends
  `MIC_CLOSE` for an idle armed session when disabled, without publishing a
  stop.

The setting is global configuration, not a layout field. The implementation
uses the existing config document and is not a second source of truth.

### Measurement

Temporary, debug-only instrumentation prefixed every verbose line with a
monotonic millisecond timestamp. The input runtime timestamped the evdev voice
edge and the daemon timestamped the first decoded frame and the first non-zero
sample. It was removed after the experiment; no timing telemetry remains in
the production path.

- **A, baseline:** voice-edge `MIC_OPEN`.
- **B, armed:** the temporary path above, run through an environment switch
  before it became the `preopen_mic` setting.
- Each press held the voice key for about 2 s, started a fixed counting phrase
  at the press, and was followed by about 3 s of silence.
- Battery was read through `org.bluez.Battery1` before and after 10 minutes of
  connected idle for both runs.

## Verification

Real RC003 `C0:5D:39:F8:2D:4C` on `hci0`, BlueZ 5.87, 2026-11-03. The installed
user unit was stopped for the runs and restarted afterwards.

```text
cargo build --release -p atvv-daemon --bin atvvd
./target/release/atvvd --device C0:5D:39:F8:2D:4C --verbose   # A
ATVV_PREOPEN_MIC=1 ./target/release/atvvd --device C0:5D:39:F8:2D:4C --verbose  # B
```

`voice press - first decoded frame`:

| Run | Presses | median | p95 | min | max |
|---|---|---|---|---|---|
| A, voice-edge `MIC_OPEN` | 25 | 155.6 ms | 174.3 ms | -9.7 ms | 184.7 ms |
| B, armed | 20 | -11.7 ms | -5.0 ms | -34.4 ms | -4.4 ms |

- Median improvement **167.3 ms**, p95 improvement **179.3 ms**.
- B's `MIC_OPEN` at ready was answered with `AUDIO_START [04, 00, 02, 00]`
  after 26 ms; `MIC_EXTEND` ran every 10 s, 73 times across the presses and
  the idle window.
- B: 0 failed `MIC_OPEN`, 0 reconnects beyond the initial connect, 0 frame
  timeouts, and 20 device stops, each at the voice release (expected, not
  unexpected).
- A: 0 failed opens and no reconnect; one press hit an unsupported 8 kHz
  `AUDIO_SYNC` and closed the stream through the existing ADR 0009 path.
- Idle battery, 10 minutes connected: A 60% → 60% (00:09:58–00:20:03), B 60% →
  60% (00:23:08–00:33:30). Delta 0 in both runs at 1% resolution.

Synthetic and live checks:

```text
cargo test -p atvv-daemon --lib armed_session_is_invisible_and_survives_errors
cargo test -p atvv-daemon --lib preopen_is_inert_for_session_triggered_devices
# The first test covers no VoiceStarted/VoiceStopped for the armed open,
# 0x0F80 keeping Streaming, the idle frame timeout, manual start/stop,
# re-arming after a stop, and a silent disarm. The second proves the setting
# does not arm a session-triggered Chromecast profile.

atvvctl config set --preopen-mic true
atvvctl status | jq '{state, streaming, preopen_mic, mic_armed}'
# ready / false / true / true
atvvctl mic start
# streaming / true / true / false
atvvctl mic stop
# ready / false / true / true (the armed session stays open)
atvvctl config set --preopen-mic false
# ready / false / false / false; the daemon logs `disarming MIC_CLOSE`.
```

`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D
warnings`, and `cargo test --workspace` are clean (269 passed, 9 ignored).

## Exit criteria

- ✅ [hardware] Baseline A: 25 presses recorded with date, address, commands,
  and median/p95 deltas.
- ✅ [hardware] Pre-open B: 20 presses recorded the same way.
- ✅ [hardware] Idle battery delta and unexpected-stop count recorded for A
  and B.
- ✅ Decision recorded: the gate passed and the opt-in path is implemented.
- ✅ Armed path suppresses the voice-edge `MIC_OPEN`; `MIC_OPEN_ERROR(0x0F80)`
  does not leave `Streaming`.
- ✅ Opt-in `preopen_mic` config, armed status, keepalive, re-open, no
  consumer-visible start/stop, tests, and docs; default `false`.
- ✅ The fail branch was not taken; the temporary instrumentation and the
  environment switch were removed, so no dead experiment path remains.
- ✅ The research note records the measured result and the final decision
  (`docs/research/rc003-hardware-evidence.md`).

## Risks

- The armed state interacts with the session state machine and the D-Bus view
  model. A pre-opened session must not look like an active dictation to
  consumers or to `atvvctl status`.
- A permanently open BLE session increases remote power use and traffic. The
  experiment measures it; it does not assume it is negligible.
- The remote's `MIC_OPEN` session may expire or behave differently after a
  reconnect. Reconnects during the idle window were counted.
- Latency measured in the daemon excludes consumer startup; a 169 ms daemon
  improvement may be smaller end-to-end if the consumer takes longer to start
  recording. The docs state the metric precisely and do not claim end-to-end
  latency.
- Temporary instrumentation must not become a per-frame log. It was kept
  behind `--verbose` and removed with the experiment.

## Progress log

- 2026-11-02 — backlog created from
  `docs/research/sayall-linux-comparison.md` §3.3 and §6.
- 2026-11-03 — promoted from B03 to step 26 and executed on the RC003. Baseline
  A: n=25, median 155.6 ms, p95 174.3 ms. Armed B: n=20, median -11.7 ms, p95
  -5.0 ms (the first frame is already queued when the evdev edge arrives).
  Median improvement 167.3 ms; no failed opens, reconnects, or unexpected
  stops; 10-minute idle battery delta 0% in both runs. Gate passed and
  `preopen_mic` (default `false`) was implemented with the armed status, 10 s
  keepalive, 200 ms re-open, silent live toggle, synthetic test, and docs.
  Temporary timestamps, the `RemoteEvent::Voice` timestamp, and the
  environment switch were removed.
