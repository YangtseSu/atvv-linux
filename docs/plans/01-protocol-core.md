# Step 01 — protocol core

Status: ✅ done
Depends on: —
Touches: `Cargo.toml`, `crates/atvv-protocol/`

## Goal

A pure, testable ATVV v1.0 protocol library: capability negotiation, command
encoding, control-event parsing, IMA/DVI ADPCM, frame accumulation, device
profiles, and the RC003 profile. It must build and test without BlueZ,
PipeWire, a remote, or root.

## Deliverables

- ✅ Workspace with `atvv-protocol` and `atvv-cli`; Rust 2024, GPL-3.0-or-later.
- ✅ `caps`: v1.0 `CAPS_RESP` parser, codec mask, interaction model, frame size,
  and the `codecs == 0` promotion quirk. v0.4 is rejected as an unknown version.
- ✅ `commands`: `GET_CAPS`, short/codec `MIC_OPEN`, bare/stream `MIC_CLOSE`,
  bare/stream `MIC_EXTEND`.
- ✅ `events`: `AUDIO_START`, `AUDIO_STOP`, `START_SEARCH`, `AUDIO_SYNC`,
  `CAPS_RESP`, `MIC_OPEN_ERROR`, and preserved unknown packets.
- ✅ `adpcm`: stateful decoder with standard IMA step/index tables, high-nibble
  first, and bounded step index.
- ✅ `frame`: fixed-size accumulator with partial-frame retention and flush.
- ✅ `profile`: `DeviceProfile` facts and the `miremote-rc003` profile.
- ✅ GATT UUID constants.
- ✅ Unit tests for all of the above.

## Design notes

- Only ATVV v1.0 is accepted. The project is documented as ATVV v1.0+; v0.4 is
  a separate future decision.
- The codec byte in `AUDIO_START` is not authoritative; `CAPS_RESP` is. It is
  parsed for diagnostics only.
- ADPCM state persists across v1.0 frames. A future `0x0A` sync packet resets it;
  the daemon owns that call.
- The frame accumulator is deliberately simple in M0. A real-time SPSC queue
  arrives with the PipeWire step; this crate stays synchronous.
- The RC003 profile is data. The daemon must not hard-code `120`, `0 dB`, or
  `[0C 00]` outside the profile.

## Out of scope

PipeWire, BlueZ, evdev, D-Bus, pairing, timers, reconnect, and audio
post-processing.

## Verification

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Observed 2026-10-29: formatting and clippy clean; **33 tests passed across 3
suites**.

## Exit criteria

- ✅ A v1.0 CAPS payload parses into version, codec mask, interaction, and frame
  size.
- ✅ An RC003-like payload parses as 16 kHz and 120 bytes.
- ✅ ADPCM decodes high nibble first and keeps the step index in range.
- ✅ The frame accumulator emits complete frames and retains partial data.
- ✅ The RC003 profile builds short `MIC_OPEN` and bare close/keepalive
  commands.
- ✅ The crate has no BlueZ, PipeWire, tokio, or input dependency.

## Risks

- The tests use synthetic payloads; a real `CAPS_RESP` may expose a firmware
  variation. Step 07 records the real values before the profile is treated as
  verified.
- The ADPCM post-processing (click removal, smoothing, gain) is not here yet;
  raw decoded PCM is the contract of this step.

## Progress log

- 2026-10-29 — step opened: caps, commands, events, ADPCM, frame, profile, and
  GATT constants implemented; workspace gates recorded.
