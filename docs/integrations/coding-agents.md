# Coding agents

The ATVV stack gives a coding agent two things and withholds one:

- **Control**: `atvv-mcp` exposes the daemon's status and voice commands over
  stdio MCP.
- **Text**: fcitx5-vinput commits the transcript into the focused application.
- **Not provided**: audio or transcript streaming through MCP. MCP is
  request/response. Audio never leaves the machine unless the chosen ASR
  backend uploads it.

## MCP server

`atvv-mcp` speaks newline-delimited JSON-RPC 2.0 on stdio and supports the
handshake revisions `2024-11-05` through `2025-11-25`
(`initialize`, `tools/list`, `tools/call`). It connects to
`org.atvv.Daemon` lazily, so it may start before the daemon.

Register the binary with the agent:

```sh
claude mcp add atvv -- /usr/bin/atvv-mcp
```

or in an MCP JSON configuration:

```json
{
  "mcpServers": {
    "atvv": {
      "command": "/usr/bin/atvv-mcp",
      "args": []
    }
  }
}
```

### Tools

| Tool | Arguments | Result |
|---|---|---|
| `status` | none | daemon status JSON (`state`, `connected`, `streaming`, `device`, `caps`, `audio`, `input`, `vinput`, `config`, `last_error`) |
| `voice_start` | none | opens the remote mic (`MIC_OPEN`) |
| `voice_stop` | none | closes the remote mic (`MIC_CLOSE`) |
| `voice_toggle` | none | starts when idle, stops while opening/streaming |
| `list_devices` | none | configured device addresses |
| `reload` | none | re-reads `daemon.toml` (`gain_db`, `input`) |

A backend failure is returned as `isError: true` with the error text, so the
agent can report "daemon not running" instead of hanging. An unknown tool is a
JSON-RPC `-32602` error.

On the RC003, `voice_start` opens the session but the physical voice key still
gates audio frames. For hands-free operation use fcitx5-vinput's trigger mode;
MCP is for status and orchestration, not for capturing speech.

## Delivering text

The transcript is produced by fcitx5-vinput and committed through the active
input context. That is the primary path: speak into the focused editor or
terminal, and the agent receives normal text input.

When an agent must push text into another terminal, prefer the terminal's own
control channel over synthetic keyboard injection:

| Surface | Command |
|---|---|
| tmux | `tmux send-keys -t <session>:<window>.<pane> -- "text" Enter` |
| WezTerm | `wezterm cli send-text --pane-id <id> -- "text"` |
| kitty | `kitty @ send-text --match id:<id> -- "text"` |
| Anything with a clipboard | `wl-copy <<< "text"`, then the user's paste key |

Guidelines:

- Address a specific pane/window; do not rely on focus.
- Prefer paste over character-by-character typing for code, quoting, and
  non-ASCII text. `send-keys` interprets some arguments; use `--` and a single
  quoted argument.
- Do not use `wtype` as the core path. It depends on compositor support for the
  virtual-keyboard protocol, injects into whatever has focus, and can type into
  the wrong window. Keep it as an optional fallback only.
- Never paste secrets into a pane or clipboard without the user's explicit
  request.

## Example flow

```text
1. User holds the RC003 voice key and speaks:
   "run the workspace tests and summarize failures"
2. fcitx5-vinput records from atvv-miremote-rc003, transcribes, commits text
   into the agent's terminal.
3. The agent runs the requested command.
4. The agent may call tools/call status to check that the remote link is still
   ready before asking for another dictation.
```

MCP `status` is also useful for diagnostics: `vinput.mode` distinguishes
trigger and D-Bus integration, `input.keyboard` reports whether the virtual
keyboard is available, and `audio.frames`/`audio.samples` prove that decoded
frames reached the source.
