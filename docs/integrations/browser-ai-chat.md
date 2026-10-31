# Browser AI chat pages

English | [简体中文](browser-ai-chat.zh-CN.md)

Browser pages consume the ATVV source through standard `getUserMedia`. There
is no ATVV-specific browser API, extension, or cloud endpoint. The page owns
its record button; the remote owns the microphone gate.

## Prerequisites

- `atvvd` is running in the user session.
- The remote is paired and the source is resident:

  ```sh
  pw-cli info atvv-miremote-rc003
  atvvctl contract --json
  ```

- The page is served over HTTPS or `localhost`; `getUserMedia` requires a
  secure context.

## Select the source

The ATVV source is not the system default. Choose it by name in the page's
microphone picker, or select its `deviceId` after permission is granted:

```js
const devices = await navigator.mediaDevices.enumerateDevices();
const atvv = devices.find(
  (device) =>
    device.kind === "audioinput" &&
    (device.label.includes("atvv-") || device.label.includes("ATVV")),
);
const stream = await navigator.mediaDevices.getUserMedia({
  audio: {
    deviceId: atvv ? { exact: atvv.deviceId } : undefined,
    channelCount: 1,
    sampleRate: 16000,
  },
});
```

Labels are empty until the user grants microphone permission, so enumerate
again after the first `getUserMedia` call if the page needs to preselect the
source.

## Record

1. Press the remote's voice key and keep holding it.
2. Start the page's recording.
3. Release the voice key to stop; the daemon closes the microphone.

The source is resident and outputs silence while the remote is idle. A stream
that connects but records silence usually means the voice key was not held:
the RC003 microphone is gated by the physical key, not by the host stream.

The page may resample the 16 kHz mono source to its own rate; that is normal.
Do not ask the daemon for a different rate.

## Verify

```sh
atvvctl contract        # source, declared keys, active key
atvvctl status          # contract plus live audio and input state
pw-cat --record --target atvv-miremote-rc003 /tmp/atvv.wav
```

Hold the voice key while `pw-cat` runs. Without the key, the WAV is silent.

## Troubleshooting

| Symptom | Cause | Action |
|---|---|---|
| No `atvv-*` device in the picker | PipeWire or the daemon is not running, or the browser cannot see the node | check `atvvctl doctor`; restart the browser after the source exists |
| Device selected, meter stays flat | voice key not held | hold the key for the whole recording |
| Permission error | page is not a secure context | use HTTPS or `localhost` |
| Wrong microphone at startup | no remembered `deviceId` | select the source again; the page owns persistence |

## Related

- [Provider contract](provider-contract.md) — source metadata, declared keys,
  and control endpoints.
- [Generic recorder](provider-contract.md#generic-recorder) — `pw-cat` check
  outside the browser.
