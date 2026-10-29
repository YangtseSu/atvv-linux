# Step 04 — PipeWire source

Status: ⬜ not-started
Depends on: 01
Touches: `crates/atvv-audio/`, `Cargo.toml`

## Goal

Expose PCM as a native PipeWire `Audio/Source` without a remote. A synthetic
16 kHz mono signal must be recordable by `pw-record`, and the source must
survive PipeWire restarts without taking the daemon down.

## Deliverables

- ⬜ `atvv-audio` crate using `pipewire-rs`; no BLE dependency.
- ⬜ Native source node: `media.class=Audio/Source`, `node.virtual=true`,
  `node.name=atvv-<id>`, `node.description`, S16LE mono, 16 kHz (profile rate).
- ⬜ Bounded SPSC queue between the producer and the PipeWire process callback;
  no allocation, locking, or blocking in the RT callback.
- ⬜ Source supervisor: recreate the stream after a PipeWire core error/restart
  and log one bounded diagnostic.
- ⬜ Test generator: 2 seconds of a 440 Hz sine at 16 kHz, written to the source.
- ⬜ `atvvctl audio test` or an example binary that starts the source, feeds the
  tone, and reports the node name.
- ⬜ Unit tests for queue overflow policy and PCM byte conversion.

## Design notes

- The daemon owns the source; the protocol crate never imports PipeWire.
- A persistent source is preferred for fcitx5-vinput device selection. It emits
  silence when idle and is marked virtual so WirePlumber does not treat it as a
  hardware device.
- `node.passive` is not used; it makes the source dependent on a link and
  complicates the first buffer.
- The source rate comes from the negotiated codec/profile. Do not hard-code
  16 kHz in the audio crate; RC003's profile supplies 16 kHz.
- PipeWire can resample to 48 kHz for consumers; the daemon does not resample.

## Out of scope

ATVV decoding, BLE, input, D-Bus, and hardware capture.

## Verification

```text
cargo test -p atvv-audio
cargo run -q -p atvv-cli -- audio test    # node name printed
pw-record --target atvv-test /tmp/atvv-tone.wav
soxi /tmp/atvv-tone.wav                   # 16 kHz, mono, s16le
```

Also verify that `wpctl status` shows the source and that a PipeWire restart
causes the source to reappear.

## Exit criteria

- ⬜ `pw-record` captures a non-empty 16 kHz mono WAV from the source.
- ⬜ No RT callback allocation/lock is introduced.
- ⬜ The source is created, removed, and recreated after a PipeWire restart.
- ⬜ The node appears with `node.virtual=true` and the configured description.
- ⬜ `cargo test --workspace` remains clean.

## Risks

- `pipewire-rs` API/version skew across distributions; pin a compatible range.
- A persistent virtual source may be mistaken for a hardware microphone; the
  description and documentation must say otherwise.
- Recording from a source with no real device proves format/transport, not
  ATVV audio.

## Progress log

- 2026-10-29 — step planned; no implementation yet.
