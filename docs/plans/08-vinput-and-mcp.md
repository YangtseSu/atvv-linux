# Step 08 — fcitx5-vinput and MCP

Status: 🚧 in-progress
Depends on: 03, 06, 07
Touches: `crates/atvv-cli/`, `crates/atvv-mcp/`, `docs/integrations/`

## Goal

Make the source and button events useful to fcitx5-vinput and coding agents
without putting ASR inside the daemon.

## Deliverables

- ✅ Documented fcitx5-vinput setup: `vinput device use atvv-<id>`, daemon
  restart, 16 kHz capture, and hold/tap trigger behavior.
- ⬜ Trigger-key mode: grab the RC003 voice key and emit a configurable
  synthetic key (`Alt_R` by default) on press/release; verify in fcitx5 and at
  least one compositor keybind.
  - Implementation, config, kernel-level uinput test, and the GNOME compositor
    bind are done. The fcitx5 live check needs a focused input context and one
    physical press; it must be repeatable whenever the operator is present, not
    inside a countdown.
- ✅ D-Bus mode: optional `StartRecording`/`StopRecording` calls to
  `org.fcitx.Vinput`, with a fallback when the service is absent.
- ✅ Warm-stream caveat handled: do not open the remote mic solely because a
  PipeWire consumer is linked; drive mic open/close from ATVV events.
- ✅ `atvv-mcp`: stdio MCP with `initialize`, `tools/list`, `tools/call`;
  tools `status`, `voice_start`, `voice_stop`, `voice_toggle`, `list_devices`,
  `reload`.
- ✅ Integration tests with a mock `org.fcitx.Vinput` on an isolated bus; no ASR
  audio is uploaded.
- ✅ Agent documentation: `tmux send-keys`, WezTerm/Kitty send-text, clipboard
  fallback; no `wtype` as the core path.

## Design notes

- Research: `docs/research/protocol-and-device.md` §8 and
  `docs/research/landscape.md` §6.
- The daemon exposes events and a source; it never performs ASR.
- fcitx5-vinput is the preferred ASR/TTS surface for desktop use. The daemon
  must not duplicate its scene/LLM logic.
- `Alt_R` is the default trigger because it is the vinput default; a rare key
  such as F18 is an alternative. The target application must be configured to
  the same key. `trigger_key` also accepts a `+` chord (`ctrl+alt+v`) because
  GNOME custom shortcuts refuse bare modifier keys.
- MCP is request/response; do not pretend it is a streaming audio transport.
  Transcript delivery belongs to the ASR adapter.
- Verification must not require a human inside a time window. The daemon is
  event-driven and keeps running; live checks are repeatable whenever the
  operator is present. Do not add countdowns or "press within N seconds" steps.

## Out of scope

ASR engines, cloud credentials, GUI, and automatic text typing.

## Verification

Completed on 2026-10-30:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace                 # 111 passed, 2 ignored
cargo test -p atvv-daemon             # 19 unit, 1 D-Bus, 2 vinput D-Bus
cargo test -p atvv-mcp                # 7 unit, 1 stdio contract
cargo test -p atvv-input voice_key_emits_the_trigger_key_and_keeps_the_raw_edge -- --ignored
```

Observed:

- The mock `org.fcitx.Vinput` on an isolated bus received `StartRecording` and
  `StopRecording("")`; with no name owner the client failed immediately,
  `available = false`, and the daemon status carried the error.
- `atvv-mcp` over stdio against the mock daemon: `initialize`, `tools/list`
  (six tools), `tools/call status`, `voice_start` → `0x0C`, `voice_stop` →
  `0x0D`, `list_devices`, and tool-error results.
- Real RC003, one physical voice-key press with `trigger_key = "ctrl+alt+v"`:
  direct `AUDIO_START 04 03 02 05`, streaming, 141 frames, `AUDIO_STOP 00 02`,
  `voice key pressed`/`released`; a GNOME custom keybinding
  `<Control><Alt>v` fired from the synthetic chord. `vinput device list`
  listed `atvv-miremote-rc003` as the active source.
- That press did not start a vinput recording: the vinput-daemon log had no
  capture negotiation. The fcitx focused-input-context path is unverified.

Still open, intentionally not time-boxed:

```text
atvvd --verbose                 # keep running
vinput device use atvv-miremote-rc003
systemctl --user restart vinput-daemon
atvvctl monitor                 # prints voice started/stopped whenever you press
vinput daemon status            # recording while the key is held
```

Focus an editor that uses fcitx, press and hold the voice key whenever
convenient, speak, release. Repeat as needed; no step expires.

## Exit criteria

- ⬜ fcitx5-vinput records from the ATVV source with 16 kHz input. The source is
  16 kHz, resident, and selectable in vinput; a live recording during a press
  was not observed.
- ⬜ The voice key starts/stops recording with no F5 leak. The daemon path,
  grab, trigger emission, and compositor bind are verified; the fcitx recording
  leg is not yet observed on a live press.
- ✅ D-Bus mode works or falls back cleanly.
- ✅ MCP handshake/tools are valid and tested.
- ✅ No ASR/network code is added to the daemon.

## Risks

- A warm capture stream can keep the mic open if consumer presence is used as
  the gate; event-driven mic control is mandatory.
- Trigger-key mode conflicts with applications that use the same key; document
  it as configurable.
- MCP client support varies; keep a CLI/JSON fallback.
- The fcitx live check needs an operator to press the physical key with a
  focused input context. Never gate it on a countdown; leave the daemon running
  and let the operator repeat the press whenever they notice.

## Progress log

- 2026-10-29 — step planned; no implementation yet.
- 2026-10-30 — implemented trigger-key mode (`vinput`/`trigger_key` config,
  single keys and `+` chords, voice-edge mic control, lost-node release), the
  `org.fcitx.Vinput` client with name-owner fast-fail, the `atvv-mcp` crate,
  mock integration tests, and the integration docs. `cargo test --workspace`
  → 111 passed, 2 ignored.
- 2026-10-30 — real RC003 run with the daemon: one physical press produced
  `AUDIO_START 04 03 02 05`, 141 frames, and `AUDIO_STOP 00 02`; the synthetic
  `ctrl+alt+v` chord fired a GNOME custom keybinding. The same press did not
  start a vinput recording (no capture negotiation in the vinput-daemon log),
  so the fcitx live leg remains open. User configuration was restored after the
  test.
- 2026-10-30 — removed the timed verification attempt. Live checks are
  repeatable and have no deadline; the step stays `🚧 in-progress` until a
  focused-editor press is observed.
