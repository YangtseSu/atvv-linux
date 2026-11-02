# B03 — Keep-open microphone experiment

Status: ⏸ backlog
Depends on: 02, 04, 06, 07
Touches: `crates/atvv-daemon/`, `crates/atvv-config/`, `docs/research/`

## Goal

Measure whether pre-opening the ATVV microphone session removes the documented
first-word delay on the RC003, and add an opt-in `preopen_mic` setting only if
the measured benefit justifies the lifecycle cost. If the experiment does not
pass the gate, close it with recorded evidence and no code.

This is an experiment with a hardware gate, not a scheduled feature. It stays
in the backlog until the RC003 is attached and the daemon baseline is stable.

## Next action

Attach the RC003, record a baseline of 20 voice presses with the current
voice-edge `MIC_OPEN`, then build the temporary pre-open path and record the
same 20 presses with it.

## Evidence

- The current daemon opens `MIC_OPEN` on the grabbed voice-key edge. The fcitx
  guide tells users to wait roughly 0.3 s after pressing before speaking
  (`docs/integrations/fcitx5-vinput.md`), which is the delay this experiment
  would remove.
- The RC003 sends a direct `AUDIO_START` on the voice key and gates microphone
  data on the physical key. A host-only `MIC_OPEN` opens the session but
  produces no frames until the key is held
  (`docs/research/rc003-hardware-evidence.md`).
- The reference project pre-opens the microphone and re-opens after
  `AUDIO_STOP`; its `preopen_mic` default is true
  (`docs/research/sayall-linux-comparison.md` §3.3). It sends no
  `MIC_EXTEND`, so its lifecycle does not cover a long hold. Do not copy its
  code (GPL-3.0-only).
- Our keepalive already extends a streaming session every 10 s and completed a
  59.3 s held press. The open question is latency, not the 60 s hold.

## Design

### Instrumentation

Add temporary, debug-only timestamps to the daemon's verbose trace:

- when the input runtime delivers the voice press (`RemoteEvent::Voice`);
- when the daemon writes `MIC_OPEN`;
- when `AUDIO_START` is handled;
- when the first decoded frame is handled;
- when the first non-zero sample is produced.

The input and BLE events are handled in the same daemon loop, so monotonic
deltas are comparable. Do not merge permanent timing telemetry; if the fields
are useful for `atvvctl doctor`, add them separately with a bounded rate.

### A/B runs

- **A, baseline:** current voice-edge `MIC_OPEN`; 20 presses, hold each for
  about 2 s, fixed test phrase or silence. Record the deltas from the
  daemon's verbose log.
- **B, pre-open:** a temporary branch that sends `MIC_OPEN` when the session
  is ready, re-opens after an unexpected `AUDIO_STOP` while not recording,
  and sends `MIC_EXTEND` every 10 s while armed. An armed session must not
  publish `VoiceStarted`/`VoiceStopped` to consumers and must not be treated
  as dictation.
- Record 20 presses for B in the same way.
- Record battery percentage before and after 10 minutes of connected idle for
  A and B, and count unexpected stops/reconnects. Battery is a 1 % cached
  property; treat the result as directional.

### Decision gate

Pass only if:

- the median or p95 time from voice press to first decoded frame improves by
  at least 100 ms; and
- there is no increase in unexpected stops, reconnects, or failed `MIC_OPEN`;
  and
- the idle battery delta is not worse than baseline beyond noise.

If pass, add an opt-in config field (default false):

```toml
preopen_mic = false
```

with an armed state, the 10 s keepalive, re-open after an unexpected stop, and
no consumer-visible `VoiceStarted` for the armed session. Document the
tradeoff: a permanently open BLE session and slightly higher remote power use
for a lower first-frame latency. On the RC003 the key still gates audio, so
this is latency, not an always-listening microphone.

If fail, remove the temporary path, record the numbers, and close the backlog
item as "not worth the lifecycle cost".

## Verification

Commands and evidence to record on the real RC003 (address and date):

```text
cargo build --release -p atvv-daemon
./target/release/atvvd --device <MAC> --verbose 2>&1 | tee /tmp/preopen-A.log
# 20 presses, hold ~2 s each
atvvctl status | jq .hardware.battery_percent
# 10 minutes idle, then read the battery again

# B: temporary pre-open branch
./target/release/atvvd --device <MAC> --verbose 2>&1 | tee /tmp/preopen-B.log
# same presses and idle measurement
```

Calculate the deltas from the trace; keep the raw logs outside the repository
and record only the summary, device address, date, and commands. Do not put a
local log path in the repository.

A synthetic test can prove the armed state does not emit `VoiceStarted`, but
it cannot satisfy the latency or battery criteria. The `[hardware]` tags below
require the real runs.

## Exit criteria

- ⬜ [hardware] Baseline A: 20 presses recorded with date, address, commands,
  and median/p95 deltas.
- ⬜ [hardware] Pre-open B: 20 presses recorded the same way.
- ⬜ [hardware] Idle battery delta and unexpected-stop count recorded for A
  and B.
- ⬜ Decision recorded: pass and implement, or fail and close.
- ⬜ If pass: opt-in `preopen_mic` config, armed status, keepalive, re-open,
  no consumer-visible start/stop, tests, and docs; default false.
- ⬜ If fail: temporary path removed; no dead config or status field remains.
- ⬜ The research note records the measured result and the final decision.

## Risks

- The armed state interacts with the session state machine and the D-Bus view
  model. A pre-opened session must not look like an active dictation to
  consumers or to `atvvctl status`.
- A permanently open BLE session increases remote power use and traffic. The
  experiment must measure it, not assume it is negligible.
- The remote's `MIC_OPEN` session may expire or behave differently after a
  reconnect. Record reconnects during the idle window.
- Latency measured in the daemon excludes consumer startup; a 100 ms daemon
  improvement may be invisible if fcitx takes longer to start recording.
  Record the metric precisely and do not claim end-to-end latency.
- Temporary instrumentation must not become a per-frame log. Keep it behind
  `--verbose` and remove it if the experiment closes.

## Progress log

- 2026-11-02 — backlog created from
  `docs/research/sayall-linux-comparison.md` §3.3 and §6.
