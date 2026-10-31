# ADR 0006 — evdevil input backend

## Status

Accepted, 2026-10-30. Implementation is tracked by
[step 11](../plans/11-input-evdevil.md); this ADR does not claim the migration
is complete.

## Context

`atvv-input` used `evdev` 0.13.2 for discovery, `EVIOCGRAB`, event reading,
and the uinput virtual keyboard. Its key table is a manual 2015 snapshot and
lacks `KEY_DICTATE`; the same range (`0x249`–`0x24f`) contains other modern
consumer keys. The upstream issue for missing scancodes has been open since
2026-02-25 without a response, and the latest release still lacks the key.

We evaluated keeping `evdev` with a local patch, forking it, `evdev-rs`, and
`input-linux`. A fork would work but would make this project own a data table
and a dependency fork. `evdev-rs` is a libevdev binding with a C dependency;
`input-linux` is a lower-level UAPI layer without the high-level device and
sync API we need.

`evdevil` 0.5.0 provides a modern Rust evdev/uinput API with the complete
consumer-key set, MT slots, clock selection, hotplug, async readers, and
uinput LED/sound support. A compile-and-run spike created a uinput device with
`KEY_DICTATE`, `KEY_VOICECOMMAND`, and `KEY_ASSISTANT`, wrote press/release
events, opened a physical evdev keyboard, and read capabilities. The license
is 0BSD.

## Decision

- Replace `evdev` 0.13.2 with `evdevil`; the requirement is `0.5` so patch
  releases flow in automatically (the exact `=0.5.0` pin was removed on
  2026-10-31).
- Do not fork `evdev` and do not add a patched `evdev` git dependency.
- Add `0BSD` to the cargo-deny license allow-list.
- Keep `atvv-input`'s public behaviour and its `#![forbid(unsafe_code)]`
  boundary. The migration is an implementation change, not a protocol or
  device-profile change.
- Keep the daemon `trigger_key` vocabulary (`VOICECOMMAND`, `ASSISTANT`,
  `DICTATE`) and the fcitx5-vinput names (`VoiceCommand`, `Assistant`,
  `Dictate`) distinct. The daemon resolves evdev names; vinput configuration
  uses fcitx names.
- Treat context-dependent mapping of the three consumer keys as future work.
  The research and options are recorded in
  [`docs/research/consumer-keys.md`](../research/consumer-keys.md).

Follow-up (2026-10-30): after the migration, step 11 changed the daemon's
default synthetic trigger to `DICTATE` by user direction. This is a policy
choice, not an evdevil requirement. The declared recording keys are
`DICTATE`, `VOICECOMMAND`, and `ASSISTANT`. The real-device run is recorded in
[step 11](../plans/11-input-evdevil.md).

## Consequences

- `atvv-input` types move from `evdev::KeyCode` to
  `evdevil::event::Key`, from `Device` to `Evdev`, and from
  `VirtualDeviceBuilder` to `UinputDevice::builder()`. Downstream crates only
  use `parse_key_program`, so the external surface is small.
- `evdevil::enumerate()` propagates per-node errors instead of silently
  skipping unreadable nodes; discovery must handle `PermissionDenied` and
  `ENODEV`.
- `evdevil` exposes `UinputDevice::sysname()` but not a direct `eventN` path;
  virtual-device node discovery must resolve the sysfs entry or enumerate by
  device name.
- `evdevil` 0.5 is young; the requirement allows patch releases, and the
  migration stays reversible. Re-evaluate when upstream makes a new release.
- No upstream PR is on the critical path. The fork clone was only an
  experiment and was removed.
- The consumer-key mapping policy remains undecided, including whether one
  physical control should emit different codes in different applications.
