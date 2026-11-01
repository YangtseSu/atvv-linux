# ADR 0009: `CAPS_RESP` is authoritative for the source sample rate

Status: accepted 2026-11-01

## Context

A profile carries `sample_rate` because the first device (the RC003)
negotiates ADPCM 16 kHz. Real remotes negotiate either 8 kHz or 16 kHz, and
`CAPS_RESP` is the field that says which (`0x01` = ADPCM 8 kHz, `0x02` =
ADPCM 16 kHz, `0x03` = both). The protocol layer already parses it and
`atvvctl status` already reports `codec: "adpcm-8k"`.

The audio path ignored it:

- `atvv-daemon/src/main.rs` built the resident PipeWire source from
  `profile.sample_rate`, before any `CAPS_RESP` existed.
- `atvv-cli/src/main.rs` `record` wrote the WAV header at `RC003.sample_rate`.
- `atvv-audio` had no resampling path.

A remote that negotiates 8 kHz therefore fed 8 kHz PCM into a 16 kHz graph.
A 120-byte ATVV frame carries 240 samples (30 ms at 8 kHz) while the graph
consumes 480 samples in the same window, so the source underruns and inserts
silence. Playback is also double speed and one octave high.

## Decision

1. **The negotiated codec decides the stream rate.**
   `Capabilities::best_codec()` is the authority, matching the recorded
   finding that the `AUDIO_START` payload codec is not trustworthy.
   `DeviceProfile::sample_rate` is only the fallback used before `CAPS_RESP`
   arrives. A profile whose `MIC_OPEN` carries a codec
   (`MicOpenVariant::Codec`) requests `preferred_codec()`; that first entry
   must equal what `best_codec()` picks (16 kHz when both are supported), or
   the daemon converts from a rate the device did not choose.
2. **The source stays resident and keeps one rate.** The PipeWire node is
   created once at the profile rate and is never torn down mid-session. The
   provider contract keeps the source resident for the daemon lifetime so a
   consumer can select `atvv-<profile>` by name independently of a remote
   connection, and a `pw_stream` format is fixed at `connect`, so changing the
   node rate means a new stream and every consumer's link is lost.
3. **The daemon resamples when the negotiated rate differs from the graph.**
   `atvv-audio` gains a stateful `Resampler`; the daemon owns one instance per
   stream and feeds it after gain. A rate mismatch is a normal device
   property, not an error, so it must not surface as `degraded`.
4. **`atvvctl record` asks the daemon for the rate.** It uses
   `status.contract.sample_rate`, which is the rate the source actually runs
   at, instead of a compiled profile constant. `ContractView.sample_rate`
   comes from the graph rate (`DaemonConfig.graph_rate`, the value
   `AudioSourceConfig.sample_rate` was created with), never from
   `DeviceProfile::sample_rate`.
5. The resampler is linear interpolation: no dependency, one small state
   machine, easy to test. It is an approximation. Upsampling 2:1 leaves a
   mirror image at `8 kHz − f` — measured with this algorithm at −16.7 dB for
   a 2 kHz tone and −7.9 dB at 3.5 kHz — and the RMS response falls up to
   3 dB at 4 kHz. That is accepted for the first implementation. ATVV v1.0
   defines only the 2:1 ratios, so a short halfband FIR can replace the
   kernel if a recorded listening or ASR check shows the image matters.

### Rejected alternatives

- **Recreate the `pw_stream` at the device rate when `CAPS_RESP` arrives.**
  The format is fixed at `connect`, so this creates a new node and drops
  every consumer link and its `node.target` selection (decision 2).
- **Create the source lazily after the first `CAPS_RESP`.** Consumers select
  the source by name before the remote connects
  (`docs/integrations/provider-contract.md`).
- **Keep `audio.rate` at the codec rate and let PipeWire convert.** PipeWire
  converts the node rate to the graph rate, but the node rate is fixed for
  the daemon lifetime, so it cannot follow a per-session codec without one of
  the two rejected options.

## Consequences

- `AudioSourceConfig.sample_rate` remains the graph rate; the daemon keeps
  advertising it in `atvv.contract` so consumers and `record` agree.
- `ContractView.sample_rate` is the graph rate, not the device rate.
  `CapsView.codec` already reports the device-side codec for diagnostics.
- 48 kHz and other rates are still rejected: ATVV v1.0 defines only 8 and
  16 kHz, and `Codec` has exactly those two variants.
- A future profile with a native 8 kHz device may set `sample_rate: 8_000`.
  `best_codec()` still prefers 16 kHz, so a dual-codec device would then be
  downsampled, and linear decimation has no anti-alias filter. That profile
  must narrow its codec preference or accept the alias before it ships.
- Resampling costs a few floating-point operations per output sample. At
  16 kHz mono this is negligible next to ADPCM decode and the PipeWire copy.
