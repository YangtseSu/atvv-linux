# Provider contract

Status: source metadata and generic contract output implemented
2026-10-31; this is the current guidance.

## Goal

Make ATVV easy to consume without linking our crates or reading our source.
The remote is useful to:

- fcitx5-vinput and other dictation frontends;
- browser pages that capture a microphone with `getUserMedia`;
- coding agents such as Claude Code, Codex, and OMP;
- IM applications such as Telegram and WeChat;
- generic recorders such as `pw-cat`.

The provider exposes standard boundaries. Consumers own their configuration.
The per-button keymap and gesture timing are local `daemon.toml` configuration;
they are not part of the provider contract.

```text
RC003 voice key ──HID──> atvv-input ──uinput recording key──> consumer shortcut
ATVV audio      ──GATT─> atvvd ──PipeWire Audio/Source──────> consumer recorder
```

`atvvd` does not select a consumer's source, write its configuration, or call
its daemon in the primary path. The microphone is gated by the physical voice
key; a host-side "start" cannot produce frames.

## Audio contract

- Node: `Audio/Source`, name `atvv-<profile>` (for example
  `atvv-miremote-rc003`).
- Format: 16 kHz, mono, S16LE. `CAPS_RESP` is authoritative for the ATVV
  codec; the PipeWire node always presents 16 kHz mono.
- Lifecycle: the source is resident for the daemon lifetime and outputs
  silence while the remote is idle.
- Gate: the RC003 microphone sends frames only while the physical voice key is
  held. Opening a PipeWire stream does not open the microphone.
- No system default: do not make the ATVV source the system default. Select it
  by name in the consumer.

Generic check:

```sh
pw-cli info atvv-miremote-rc003
pw-cat --record --target atvv-miremote-rc003 /tmp/atvv.wav
```

## Recording-key contract

The provider declares the keys that can start a recording. Consumers bind the
declared set; the provider emits one active key per press.

| Canonical name | evdev | fcitx5 name | Role |
|---|---|---:|---|
| `DICTATE` | `KEY_DICTATE` `0x24a` | `Dictate` | Default active key |
| `VOICECOMMAND` | `KEY_VOICECOMMAND` `0x246` | `VoiceCommand` | Supported fallback |
| `ASSISTANT` | `KEY_ASSISTANT` `0x247` | `Assistant` | Supported fallback |

- Declared set: all three names, in the order above.
- Active key: `DICTATE` by default; `trigger_key` may select
  `VOICECOMMAND`, `ASSISTANT`, or a raw key/chord for a specific consumer.
- Binding rule: add the whole declared set to the consumer's shortcut list,
  preserving the user's existing shortcuts. Pressing the remote emits the
  active key; the other bindings are inert until the active key changes.
- Do not emit all three simultaneously. Consumers may parse the extra keys as
  a chord or trigger duplicate actions.

### Discovery

The source advertises the contract as PipeWire properties:

```text
atvv.contract_version = 1
atvv.profile          = miremote-rc003
atvv.recording_keys   = DICTATE,VOICECOMMAND,ASSISTANT
```

Consumers that can read properties should auto-bind the declared keys when the
user selects the source. Consumers that cannot auto-bind must tell the user to
set the application shortcut to one of the declared keys. The provider does
not try to discover an arbitrary application's shortcut.

## Control and status

- `org.atvv.Daemon` on the session bus: status JSON, `StartVoice`,
  `StopVoice`, `ToggleVoice`, `ReloadConfig`, `ListDevices`.
- `atvvctl`: human and JSON diagnostics; no per-application commands.
- `atvvctl contract [--json]`: the provider contract consumers discover on the
  source (profile, source, media class, format, declared keys, active key,
  input/keyboard readiness, D-Bus/MCP endpoints, and a generic recipe).
- `atvvctl status`: the full daemon view; its `contract` object is the same
  contract data.
- `atvv-mcp`: stdio MCP for coding agents (`status`, `contract`, `voice_*`,
  `list_devices`, `reload`). MCP is control/status, not an audio transport.

## Consumer recipes

### fcitx5-vinput

1. Select the source: `vinput device use atvv-miremote-rc003`.
2. Bind the declared keys. On a default install `[TriggerKey]` contains
   `0=Alt_R`; append `Dictate` as the next free index:

   ```ini
   [TriggerKey]
   0=Alt_R
   1=Dictate
   ```

   A customized list uses its next free index; never hard-code an index from
   another configuration. Stop fcitx5 before editing `vinput.conf`, because
   the addon rewrites it on exit.
3. The upstream feature request is: when a source advertises
   `atvv.recording_keys`, append the mapped fcitx names automatically on
   `vinput device use`, preserving existing entries. Implementation path:
   `vinput device use` already enumerates sources, so it can read the property
   and use fcitx's `org.fcitx.Fcitx.Controller1` `GetConfig`/`SetConfig` on
   `fcitx://config/addon/vinput` to append the entries live. If fcitx or the
   property is unavailable, print the exact entries instead. The manual entry
   above is the supported fallback.

The optional `vinput = "dbus"` mode is a fallback for environments where the
virtual keyboard cannot reach the input method; it is not the default.

### Browser AI chat pages

See [browser-ai-chat](browser-ai-chat.md).

### Coding agents

Use `atvv-mcp` for status and session control. The transcript reaches the
focused editor through the IME/dictation frontend; MCP does not stream audio.
If the agent needs the raw audio, it can record from the PipeWire source with
`pw-cat` while the user holds the key, then run its own ASR.

### IM applications

See [im-applications](im-applications.md).

### Generic recorder

```sh
pw-cat --record --target atvv-miremote-rc003 /tmp/atvv.wav
```

## Diagnostics

Generic only:

- `atvvctl status` — daemon, source, input, and the `contract` object.
- `atvvctl doctor` — provider-side checks (D-Bus, BlueZ, PipeWire, uinput,
  config) including whether the live source advertises `atvv.*` properties.
- `atvvctl contract [--json]` — the provider contract reported by the running
  daemon.
- `atvv-mcp contract` — the same object for coding agents.

Per-consumer setup belongs in the documents above. If a consumer cannot see
the source or its shortcut does not match, the fix belongs in that consumer's
configuration or in the provider contract, not in an application-specific
branch.

## Versioning

`atvv.contract_version` starts at `1`. Additive properties are allowed;
removing or changing the meaning of a declared key or source property requires
an ADR and a version bump.
