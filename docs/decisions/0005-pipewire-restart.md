# ADR 0005 — PipeWire service restart ownership

## Status

Accepted, 2026-10-30.

## Context

The PipeWire source supervisor can recover when its node is removed from a live
connection (registry `global_remove`). Recovering from a full
`systemctl --user restart pipewire` is a different failure mode: the client
connection is torn down, and the in-process PipeWire main loop was not reliably
woken to rebuild the stream. Keeping a second in-process monitor for socket
inodes, process ids, and heartbeats added complexity without a verified result.

## Decision

- `atvv-audio` owns source creation, node-removal recovery, and core/stream
  error handling.
- A full PipeWire service restart is owned by **systemd**: `atvvd.service` will
  use `PartOf=pipewire.service` (and `Restart=always`) so the daemon is
  restarted with PipeWire, rebuilding the source.
- Do not add more in-process service-restart detection to `atvv-audio`.
- `atvvctl audio test` is a diagnostic; if PipeWire restarts, it may exit and
  must be restarted by the operator.

## Consequences

- Step 04 is complete for source creation, recording, and node-removal
  recovery.
- Step 06/09 must implement and verify the systemd restart relationship; until
  then, no claim is made that a running daemon survives a PipeWire restart.
- The source code stays small and the recovery responsibility is in one place.
