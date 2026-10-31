# Step 21 — Multi-device switching

Status: ⬜ not-started
Depends on: 06, 13, 15, 17
Touches: `crates/atvv-config/`, `crates/atvv-daemon/`, `crates/atvv-cli/`,
`crates/atvv-gui/`, `crates/atvv-mcp/`, `docs/`

## Goal

One `atvvd` serves one active ATVV remote at a time. A registry of known
devices lets the user switch device without editing `daemon.toml` or
restarting the service. The active device owns the GATT session, the resident
PipeWire source, the HID grab, test mode, and the board. Mappings and layouts
are per device. Simultaneous devices are out of scope for v1.

The step 15 interface reference previews the header device chip and the
known-device popover (`docs/plans/15-mapping-board.html`); the second remote
there is illustrative, not verified hardware.

## Current gap

- `ListDevices()` returns `vec![self.address]`
  (`crates/atvv-daemon/src/dbus.rs`).
- `daemon.toml` has one `device`; `main` starts one `RC003` transport
  (`crates/atvv-daemon/src/main.rs`).
- `atvv-input` grabs every matched node
  (`crates/atvv-input/src/runtime.rs`); two same-profile remotes connected at
  once would both be grabbed.
- Step 17 keeps layouts global; per-device assignment is this step.

## Decision

Single active device, explicit switch. Do not add simultaneous sources in
v1: the provider contract has one source name (`atvv-<profile>`), and
consumer-side capture selection would need a per-device source and routing.

## Data model

- Device registry and active device. The exact schema version is decided with
  step 17 to avoid a second migration:
  ```toml
  active_device = "C0:5D:39:F8:2D:4C"

  [devices."C0:5D:39:F8:2D:4C"]
  label = "Living room"          # optional user label
  profile = "miremote-rc003"
  layout = "default"
  ```
- `label`, `profile`, and `layout` persist. Last seen and battery are runtime
  status, not config; expose them through `GetDevices`.
- `atvvctl pair` adds or updates the device entry and sets it active; it must
  not discard other known devices or layouts.
- A device whose profile does not match the loaded profile is rejected with a
  clear error until the profile-driven board/audio work exists. The first cut
  supports multiple devices of the current profile.

## Daemon

- A supervisor owns the active address and transport. `SelectDevice` stops
  the current session, ends capture/test, releases the evdev grab, disconnects
  the old device, persists the address, builds the new transport, and starts
  the session.
- The resident source keeps its name when the profile is unchanged. A failed
  connect restores the previous device and config; the GUI sees an error.
- The old HID node must be gone before the new one is grabbed. Rely on BlueZ
  disconnect; if evdev exposes the device address (`uniq`), filter discovery
  by the active address as a second guard.
- `StatusView` adds the active device and a device list; `atvvctl doctor`
  checks the registry and the active link.

## D-Bus

- `ListDevices() -> as` returns known addresses (compatible).
- `GetDevices() -> s` returns JSON metadata: address, label, name, profile,
  active, connected, battery, last seen, layout.
- `SelectDevice(s) -> ()`; signal `DeviceChanged(s)`.
- `GetConfig`/`SetConfig` include the registry; the existing `device` field
  becomes `active_device` or is migrated.

## GUI

- Header device chip and popover from the step 15 reference: label, address,
  profile, battery, connected/last seen, active check, pair hint.
- Switching is blocked while the document is dirty; the user applies or
  reverts first. Test mode ends on a switch.
- The board and editor reload the active device's layout. A failed switch
  keeps the old device selected and shows an error.
- The GUI never talks to BlueZ or PipeWire; it calls D-Bus only. Pairing stays
  in `atvvctl pair`; the GUI shows the command.

## CLI and MCP

- `atvvctl device list`, `atvvctl device use <address>`,
  `atvvctl device rename <address> <label>`.
- `atvvctl status` shows the active device; `doctor` checks it.
- `atvv-mcp` exposes `devices` and `select_device`.

## Tests

- Registry persistence, migration, and validation; `pair` preserves entries.
- Two fake devices on a private bus: switch, layout reload, capture ends,
  failure rollback, old address not grabbed, source name unchanged.
- GUI: chip/popover, dirty block, per-device map, error state; UI smoke.
- Real RC003: switch to a second device and back; the active RC003 still
  streams and the source survives. The second device path needs real hardware;
  a fake remote proves only the code path. No hardware claim without a
  recorded run.

## Out of scope

- Simultaneous devices and per-device sources (`atvv-<address>`).
- A second hardware profile's diagram/button geometry; the board is still
  RC003-shaped. That needs a profile-driven diagram/button set.
- GUI pairing, forgetting, or BlueZ scanning; `atvvctl pair` remains the
  pairing path.

## Exit criteria

- ⬜ Known devices persist and the active device survives a daemon restart.
- ⬜ Switching reconnects the transport and keeps the source name for the same
  profile; a failed switch restores the old device.
- ⬜ Per-device layouts load into the board/input runtime; dirty switching is
  blocked.
- ⬜ Only the active device's HID node is grabbed; capture/test ends on switch.
- ⬜ CLI, MCP, status, and doctor expose the active device; docs updated.
- ⬜ The `-git` package contains the GUI; GTK stays out of the base package.

## Risks

- Two connected same-profile remotes can both match evdev discovery; BlueZ
  disconnect and address filtering are both required.
- Address identity can change after re-pairing; keep the label and let the
  user remove/re-pair an entry.
- A source rebuild or reconnect gap can drop the first samples; consumers own
  their capture and may need a reselect for a different profile.
- Per-device layouts need step 17's schema decision; do not ship v2 and then
  a second migration without reviewing that plan.

## Progress log

- 2026-11-01 — planned after the step 15 interface review. The device chip and
  popover are previewed in `docs/plans/15-mapping-board.html`; no code yet.
