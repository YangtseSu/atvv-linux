# ADR 0001 — userspace ATVV stack

## Status

Accepted, 2026-10-29.

## Context

ATVV is a Google GATT application protocol. The remote's audio does not travel
over HID or a standard Bluetooth audio profile. BlueZ already provides pairing,
GATT access, and HID-over-GATT; PipeWire already provides the audio graph;
Linux already provides evdev and uinput.

## Decision

Build a userspace daemon:

- BlueZ is the Bluetooth/GATT/HID provider.
- The daemon is the ATVV GATT client and protocol state machine.
- PipeWire gets a native `Audio/Source` node.
- Input uses evdev with `EVIOCGRAB` and a uinput virtual keyboard.
- Control and integration use a session D-Bus API.

No kernel module, BlueZ plugin, or PipeWire plugin is part of the design.

## Consequences

- No root or custom kernel/BlueZ/PipeWire build is required.
- The protocol can be tested on a machine without a remote by feeding synthetic
  notifications into the state machine and replaying frames.
- The daemon must handle BlueZ reconnects, GATT service resolution, and PipeWire
  restarts itself.
- Hardware acceptance remains mandatory; a synthetic test does not prove a
  remote's firmware behavior.
