# Step 03 — CLI diagnostics

Status: ✅ done
Depends on: 01, 02
Touches: `crates/atvv-cli/`, `Cargo.toml`

## Goal

`atvvctl` gives a human or agent a hardware-free view of the compiled profiles,
CAPS payloads, and the RC003 state machine. It is the first smoke surface for
the project and a prerequisite for the daemon's diagnostics.

## Deliverables

- ✅ `atvvctl profile list` lists every compiled profile.
- ✅ `atvvctl profile show <id>` prints identity, ATVV version, codec, frame,
  nibble order, command variants, gain, and key names.
- ✅ `atvvctl caps decode <hex>` parses a v1.0 `CAPS_RESP` and prints version,
  codec mask, supported codecs, preferred codec, interaction, and frame size.
- ✅ `atvvctl simulate <id>` drives the RC003 state machine through caps,
  direct `AUDIO_START`, keepalive, stop, and stop echo.
- ✅ Hex parser accepts `0x` prefix, spaces, `:` and `-`; rejects odd/invalid
  input.
- ✅ CLI unit tests for hex parsing; no protocol logic duplicated.

## Design notes

- The CLI is intentionally read-only in M0. It will become a D-Bus client in
  step 06; the current commands remain useful as offline diagnostics.
- `simulate` uses the same `Session` as the future daemon. It is a behavior
  test, not a mock of it.
- CAPS is authoritative; `simulate` uses a synthetic v1.0 capability block only
  because no device is present.

## Out of scope

D-Bus, real BLE, PipeWire, input, and TUI/GUI.

## Verification

```text
cargo run -q -p atvv-cli -- profile list
cargo run -q -p atvv-cli -- profile show miremote-rc003
cargo run -q -p atvv-cli -- caps decode 0B010002000078
cargo run -q -p atvv-cli -- simulate miremote-rc003
```

Observed 2026-10-29:

- `profile list` prints `miremote-rc003`, ATVV `1.0`, `120 B`, `16000 Hz`.
- `caps decode` prints version `1.0`, codec mask `0x02`, `adpcm16k`,
  preferred `Adpcm16k`, interaction `OnRequest`, frame `120 B`.
- `simulate` moves `Connected` → `Ready` → `Streaming` → `Ready`, prints
  `voice started`, `keepalive [0e]`, `voice stopped`, `send [0d]`, and ignores
  the repeated stop.

## Exit criteria

- ✅ Every compiled profile is discoverable by id.
- ✅ RC003 facts are visible without reading source.
- ✅ A CAPS payload can be decoded from a hex string.
- ✅ The state machine can be exercised without hardware.
- ✅ `cargo test --workspace` includes the CLI tests.

## Risks

- The CAPS example is synthetic; real payloads are recorded in step 07.
- The CLI will change from local data to D-Bus in step 06; the commands should
  keep their names to avoid churn.

## Progress log

- 2026-10-29 — step opened: profile/caps/simulate commands, hex parser, and
  smoke output recorded.
