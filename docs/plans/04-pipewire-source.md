# Step 04 — PipeWire source

Status: 🚧 in-progress
Depends on: 01
Touches: `crates/atvv-audio/`, `crates/atvv-cli/`, `Cargo.toml`

## Goal

Expose PCM as a native PipeWire `Audio/Source` without a remote. A synthetic
16 kHz mono signal must be recordable by `pw-record`, and the source must
recover when its PipeWire node is removed.

## Deliverables

- ✅ `atvv-audio` crate using `pipewire-rs`; no BLE dependency.
- ✅ Native source node: `media.class=Audio/Source`, `node.virtual=true`,
  `node.name=atvv-<id>`, `node.description`, S16LE mono, 16 kHz.
- ✅ Bounded lock-free queue between producer and RT callback; no allocation,
  locking, or blocking in the callback.
- ✅ Source supervisor: registry node-removal handling, core error handling,
  core heartbeat, and socket/PID monitoring for restart detection.
- ✅ Test generator: 440 Hz sine at 16 kHz, fed through the source.
- ✅ `atvvctl audio test --seconds N` creates `atvv-test`, feeds the tone, and
  stops cleanly.
- ✅ Unit tests for queue overflow and config validation.
- ⬜ Full `systemctl --user restart pipewire` recovery is not verified; the
  node-removal path is verified.

## Design notes

- The daemon owns the source; the protocol crate never imports PipeWire.
- A persistent source is preferred for fcitx5-vinput device selection. It emits
  silence when idle and is marked virtual so WirePlumber does not treat it as a
  hardware device.
- `node.passive` is not used; it makes the source dependent on a link and
  complicates the first buffer.
- The source rate comes from the negotiated codec/profile. Do not hard-code
  16 kHz in the audio crate; RC003's profile supplies 16 kHz.
- The queue is `crossbeam_queue::ArrayQueue`; it is bounded and lock-free. The
  producer drops the oldest sample on overflow; the RT callback only pops.
- The supervisor uses four signals: stream error, core error, core heartbeat
  (`sync` + `done`), and a monitor thread watching the PipeWire socket inode and
  the `pipewire` process id. Node removal via the registry is the verified
  recovery path.

## Out of scope

ATVV decoding, BLE, input, D-Bus, and hardware capture.

## Verification

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Observed 2026-10-30: formatting and clippy clean; **36 tests passed across 5
suites**.

```text
atvvctl audio test --seconds 6
pw-record --target atvv-test --rate 16000 --channels 1 --format s16 /tmp/atvv-final.wav
```

Observed:

- Source node properties: `node.name = "atvv-test"`,
  `media.class = "Audio/Source"`, `media.type = "Audio"`,
  `media.category = "Capture"`, `media.role = "Communication"`.
- `pw-record` captured **2.00 s, 16 kHz, mono, s16le**; `sox stat` reported
  maximum amplitude **0.199982** and RMS **0.140643** (the 440 Hz tone is
  present, not silence).
- `pw-cli destroy <node>` removed the source and the supervisor recreated it
  with a new node id.
- `systemctl --user restart pipewire` was attempted; the source did not
  reappear within 5 s. The restart path remains the open item.

## Exit criteria

- ✅ `pw-record` captures a non-empty 16 kHz mono WAV from the source.
- ✅ No RT callback allocation/lock is introduced.
- ✅ The source is recreated after its PipeWire node is destroyed.
- ✅ The node appears as an `Audio/Source` with the configured description.
- ⬜ Full `systemctl --user restart pipewire` recovery is verified.
- ✅ `cargo test --workspace` remains clean.

## Risks

- The full PipeWire service restart path is the only unverified source-lifecycle
  case. The node-removal path and the supervisor code are in place; do not
  claim service-restart recovery until a real run passes.
- `pipewire-rs` API/version skew across distributions; pin a compatible range.
- A persistent virtual source may be mistaken for a hardware microphone; the
  description and documentation must say otherwise.
- Recording from a source with no real device proves format/transport, not
  ATVV audio.

## Progress log

- 2026-10-30 — step opened: `atvv-audio`, queue, native source, `atvvctl audio
  test`, unit tests, tone recording, and node-removal recreation recorded.
  Full PipeWire service restart remains open.
