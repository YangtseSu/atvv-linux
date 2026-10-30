# IM applications

Messaging clients consume the ATVV source through their own microphone
settings. There is no ATVV-specific integration or protocol. The application
owns the call or voice-message button; the remote owns the microphone gate.

Typical clients: Telegram Desktop, WeChat, Element/Matrix clients, and any
other application with a voice-message or call feature.

## Setup

1. Confirm the source exists:

   ```sh
   pw-cli info atvv-miremote-rc003
   atvvctl contract
   ```

2. Open the application's audio or voice settings and select the ATVV source
   by name (`atvv-miremote-rc003`). Do not make it the system default; other
   applications should keep their normal microphone.
3. Record as usual while holding the remote's voice key. Release the key to
   stop; the daemon closes the microphone.

The source is resident and silent while the remote is idle. A voice message
that records silence usually means the voice key was not held during the
recording.

## Sandboxed clients

A Flatpak or portal-sandboxed client sees only the devices the desktop portal
exposes. If the ATVV source is missing from its picker, grant the application
microphone access in the desktop settings or run the non-sandboxed build. Do
not set the ATVV source as the system default to work around the portal.

## Telegram Desktop

Telegram uses the PipeWire microphone selected in **Settings → Calls and
Messages → Input device**. Select the ATVV source there, then hold the voice
key while recording a voice message. Telegram's own echo cancellation may
alter the stream; disable it if the transcript quality is poor.

## WeChat

WeChat's Linux builds and compatibility layers often use the system default
microphone. If the source cannot be selected, use the client's own audio
settings when available; otherwise record the voice message with a client that
can select the source. ATVV does not write another application's configuration.

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
