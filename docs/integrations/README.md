# Integrations

The daemon owns the ATVV session and a resident PipeWire source. Everything
else consumes one of two boundaries:

- **Audio**: `Audio/Source` node `atvv-miremote-rc003`, 16 kHz mono s16le. Use
  fcitx5-vinput, an ASR adapter, or a recorder. The daemon never performs ASR.
- **Control**: `org.atvv.Daemon` on the session bus, `atvvctl`, or `atvv-mcp`.

| Integration | Document | Boundary |
|---|---|---|
| fcitx5-vinput | [fcitx5-vinput](fcitx5-vinput.md) | source + trigger key or `org.fcitx.Vinput` |
| Coding agents | [coding-agents](coding-agents.md) | MCP tools + agent text delivery |
| CLI | [`atvvctl`](../../README.md) | `org.atvv.Daemon` |

The mic is never opened because a consumer linked to the source. On the RC003
the physical voice key gates microphone data, so the daemon opens `MIC_OPEN` on
the key press and closes it on release. A warm PipeWire stream only means the
consumer is ready to receive audio.

Live checks are event-driven and repeatable. The daemon keeps running; press
the key whenever you are at the machine. Do not build countdowns or timed
"press within N seconds" tests.
