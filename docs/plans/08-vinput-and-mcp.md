# Step 08 — fcitx5-vinput and MCP

Status: ⬜ not-started
Depends on: 03, 06, 07
Touches: `crates/atvv-cli/`, `crates/atvv-mcp/`, `docs/integrations/`

## Goal

Make the source and button events useful to fcitx5-vinput and coding agents
without putting ASR inside the daemon.

## Deliverables

- ⬜ Documented fcitx5-vinput setup: `vinput device use atvv-<id>`, daemon
  restart, 16 kHz capture, and hold/tap trigger behavior.
- ⬜ Trigger-key mode: grab the RC003 voice key and emit a configurable
  synthetic key (`Alt_R` by default) on press/release; verify in fcitx5 and at
  least one compositor keybind.
- ⬜ D-Bus mode: optional `StartRecording`/`StopRecording` calls to
  `org.fcitx.Vinput`, with a fallback when the service is absent.
- ⬜ Warm-stream caveat handled: do not open the remote mic solely because a
  PipeWire consumer is linked; drive mic open/close from ATVV events.
- ⬜ `atvv-mcp`: stdio MCP with `initialize`, `tools/list`, `tools/call`;
  tools `status`, `voice_start`, `voice_stop`, `voice_toggle`, `list_devices`,
  `reload`.
- ⬜ Integration tests with a mock `org.fcitx.Vinput` on an isolated bus; no ASR
  audio is uploaded.
- ⬜ Agent documentation: `tmux send-keys`, WezTerm/Kitty send-text, clipboard
  fallback; no `wtype` as the core path.

## Design notes

- The daemon exposes events and a source; it never performs ASR.
- fcitx5-vinput is the preferred ASR/TTS surface for desktop use. The daemon
  must not duplicate its scene/LLM logic.
- `Alt_R` is the default trigger because it is the vinput default; a rare key
  such as F18 is an alternative. The target application must be configured to
  the same key.
- MCP is request/response; do not pretend it is a streaming audio transport.
  Transcript delivery belongs to the ASR adapter.

## Out of scope

ASR engines, cloud credentials, GUI, and automatic text typing.

## Verification

```text
cargo test --workspace
dbus-run-session -- cargo test -p atvv-mcp
vinput device list
# hold the remote voice key in a focused editor
atvvctl monitor
```

Also verify a mock vinput Start/Stop call, a synthetic trigger key in a
compositor bind, and MCP `tools/list`/`tools/call` over stdio.

## Exit criteria

- ⬜ fcitx5-vinput records from the ATVV source with 16 kHz input.
- ⬜ The voice key starts/stops recording with no F5 leak.
- ⬜ D-Bus mode works or falls back cleanly.
- ⬜ MCP handshake/tools are valid and tested.
- ⬜ No ASR/network code is added to the daemon.

## Risks

- A warm capture stream can keep the mic open if consumer presence is used as
  the gate; event-driven mic control is mandatory.
- Trigger-key mode conflicts with applications that use the same key; document
  it as configurable.
- MCP client support varies; keep a CLI/JSON fallback.

## Progress log

- 2026-10-29 — step planned; no implementation yet.
