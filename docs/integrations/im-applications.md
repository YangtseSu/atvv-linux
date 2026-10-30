# IM applications

Status: generic guidance. Messaging clients consume the ATVV source through
their own microphone settings. There is no ATVV-specific integration,
per-client shortcut, or per-application diagnostic.

Typical clients include Telegram Desktop, WeChat, and Matrix clients. Their
menus, shortcuts, and recording modes are owned by those clients and are not
documented here.

## Setup

1. Confirm the source exists:

   ```sh
   pw-cli info atvv-miremote-rc003
   atvvctl contract
   ```

2. Select the ATVV source by name (`atvv-miremote-rc003`) in the client's
   audio or voice-message settings. If the client has no source picker, the
   fix belongs in that client or in its desktop portal permissions; ATVV does
   not change the system default.
3. Hold the remote's voice key while recording, and release it to stop. The
   remote key gates the microphone; it does not send the message. Use the
   client's own send control.

The source is resident and silent while the remote is idle. A recording that
captures only silence usually means the voice key was not held.

## Sandboxed clients

A Flatpak or portal-sandboxed client sees only the devices the desktop portal
exposes. If the ATVV source is missing from its picker, grant microphone access
in the desktop settings or run a build that can select a PipeWire source. Do
not set the ATVV source as the system default as a workaround.

## Verify

```sh
atvvctl status          # contract, input state, audio counters
pw-cat --record --target atvv-miremote-rc003 /tmp/atvv.wav
```

The recording is silent until the voice key is held.

## Related

- [Provider contract](provider-contract.md) — declared recording keys and
  control endpoints.
- [Browser AI chat pages](browser-ai-chat.md) — browser microphone picker.
