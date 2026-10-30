# Integrations

The daemon owns the ATVV session and a resident PipeWire source. Everything
else consumes one of two boundaries:

- **Audio**: `Audio/Source` node `atvv-miremote-rc003`, 16 kHz mono s16le. Use
  fcitx5-vinput, an ASR adapter, or a recorder. The daemon never performs ASR.
- **Control**: `org.atvv.Daemon` on the session bus, `atvvctl`, or `atvv-mcp`.

| Integration | Document | Boundary |
|---|---|---|
| Provider contract | [provider-contract](provider-contract.md) | source metadata + recording keys + generic control |
| fcitx5-vinput | [fcitx5-vinput](fcitx5-vinput.md) | source + declared recording keys |
| Coding agents | [coding-agents](coding-agents.md) | MCP tools + agent text delivery |
| Browser AI chat | [provider-contract](provider-contract.md#browser-ai-chat-pages) | `getUserMedia` device selection |
| IM applications | [provider-contract](provider-contract.md#im-applications) | source selection in the app |
| CLI | [`atvvctl`](../../README.md) | `org.atvv.Daemon` |

The provider declares its contract on the source
(`atvv.contract_version`, `atvv.profile`, `atvv.recording_keys`). Consumers
own their configuration and should bind the declared keys; the provider does
not add per-application diagnostics or per-application control paths.

The mic is never opened because a consumer linked to the source. On the RC003
the physical voice key gates microphone data, so the daemon opens `MIC_OPEN` on
the key press and closes it on release. A warm PipeWire stream only means the
consumer is ready to receive audio.

Live checks are event-driven and repeatable. The daemon keeps running; press
the key whenever you are at the machine. Do not build countdowns or timed
"press within N seconds" tests.
