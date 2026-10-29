# Research notes

This directory records implementation-relevant protocol facts and source
boundaries. Code in this repository is original unless a file says otherwise.

## Public protocol facts

- ATVV service: `AB5E0001-5A21-4F05-BC7D-AF01F617B664`
- Command characteristic: `AB5E0002-5A21-4F05-BC7D-AF01F617B664`
- Audio characteristic: `AB5E0003-5A21-4F05-BC7D-AF01F617B664`
- Control characteristic: `AB5E0004-5A21-4F05-BC7D-AF01F617B664`
- v1.0 capabilities: `0B version(2) codecs(1) interaction(1) frame(2)`
- RC003 profile: 16 kHz IMA/DVI ADPCM, 120-byte headerless frames, 0 dB gain

## Source boundaries

- ATVVoice and sayall are MIT references for protocol and Linux integration
  behavior; their notices must be preserved when code is reused.
- remote-mic-app and Open Voice Bridge are GPL-3.0-only references. Their code
  is not copied into this GPL-3.0-or-later tree; their documentation and
  behavior are used as reference only.
- BlueZ, PipeWire, evdev, and the kernel are used through their public APIs;
  this project does not patch them.

## Open questions

- Real RC003 `CAPS_RESP`, `AUDIO_STOP` reason, and echo timing must be recorded
  on hardware (step 07).
- Real 16 kHz gain and microphone quality must be measured on hardware.
- fcitx5-vinput warm-stream behavior must be verified with the real daemon
  (step 08).
