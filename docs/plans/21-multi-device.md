# Step 21 — Multi-device switching

Status: 🚧 in-progress
Depends on: 06, 13, 15, 17
Touches: `crates/atvv-protocol/`, `crates/atvv-config/`, `crates/atvv-input/`,
`crates/atvv-bluez/`, `crates/atvv-daemon/`, `crates/atvv-cli/`,
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

## Next action

Connect the RC003 and Chromecast to this host and record the two-device switch
(and back) with a real audio stream.

## Current gap

- The two-device switch has synthetic coverage and a mock-D-Bus smoke run, but
  no recorded RC003 + Chromecast run on one host.
- `GetDevices` does not track a last-seen timestamp; it reports the registry,
  active/connected state, layout, name, label, profile, and battery.

## Decision

Single active device, explicit switch. Do not add simultaneous sources in
v1: the provider contract has one source name (`atvv-<profile>`), and
consumer-side capture selection would need a per-device source and routing.

## Data model

- Device registry and active device in schema v3. Step 17 chose schema v2;
  this step migrates the single `device`/`active_layout` pair into a registry
  entry and writes v3:
  ```toml
  active_device = "C0:5D:39:F8:2D:4C"

  [devices."C0:5D:39:F8:2D:4C"]
  label = "Living room"          # optional user label
  profile = "miremote-rc003"
  layout = "default"
  ```
- `label`, `profile`, and `layout` persist. Battery and connection state are
  runtime status, not config; expose them through `GetDevices` and
  `StatusView.devices`. A last-seen timestamp is not stored yet.
- `atvvctl pair` infers the profile from the BlueZ PnP ids, adds or updates the
  device entry, and sets it active; it does not discard other known devices or
  layouts.
- Devices of different profiles are supported: the active device selects the
  session profile, source name, input keymap, layout, and board button set.
  The diagram, cards, and preset preview follow the active profile.

## Daemon

- A supervisor owns the active address and transport. `SelectDevice` stops
  the current session, ends capture/test, releases the evdev grab, disconnects
  the old device, persists the address, builds the new transport, and starts
  the session.
- The resident source keeps its name when the profile is unchanged; a profile
  change drops the old source and creates `atvv-<new-profile>`. A failed
  transport creation restores the previous device and config; the GUI sees an
  error.
- The old HID node must be gone before the new one is grabbed. BlueZ
  disconnect runs first, and discovery filters by the evdev `uniq` address as
  a second guard.
- `StatusView` adds the active device and a device list; `atvvctl doctor`
  checks the registry and the active link.

## D-Bus

- `ListDevices() -> as` returns known addresses (compatible).
- `GetDevices() -> s` returns JSON metadata: address, label, name, profile,
  active, connected, battery, and layout. A last-seen timestamp is not
  tracked yet.
- `SelectDevice(s) -> ()`; signal `DeviceChanged(s)`.
- `GetConfig`/`SetConfig` include the registry; the existing `device` field
  becomes `active_device` or is migrated.

## GUI

- Header device chip and popover from the step 15 reference: label, address,
  profile, battery, connected/last seen, active check, pair hint.
- Switching is blocked while the document is dirty; the user applies or
  reverts first. Test mode ends on a switch.
- The board and editor reload the active device's layout and button set. The
  Chromecast exposes Mute, YouTube, and Netflix and hides Menu; its `tv` slot
  is labelled Input. A failed switch keeps the old device selected and shows
  an error.
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
- GUI: chip/popover, dirty block, per-device map, error state; UI smoke with
  the RC003 and Chromecast button sets, preset preview, and the
  session-driven Assistant test event.
- [hardware] Real RC003: switch to a second device and back; the active RC003
  still streams and the source survives. The second device path needs real
  hardware; a fake remote proves only the code path. No hardware claim without
  a recorded run.

## Verification

Recorded 2026-11-02 on the second development machine:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`cargo test --workspace` → all suites passed, including 40 GUI model tests and
the new registry, migration, switch, rollback, and capture-end tests.
`xvfb-run` is not installed on this machine, so the GTK window/UI tests were
not rerun here; the GUI compiles and its model tests pass.

```text
atvvctl device list
atvvctl device use 02:00:00:00:00:02
atvvctl status
atvvctl doctor
```

A `dbus-run-session` smoke run with `atvvd --mock` and two devices printed both
devices, renamed one, switched the active device, changed the source to
`atvv-google-chromecast`, reloaded layout `second`, switched that device to
`default`, and persisted `active_device = "02:00:00:00:00:02"` plus the label
and layout in the TOML file.

Real Chromecast run (2026-11-02, `74:46:B3:22:2D:D5`): `atvvctl pair`
inferred `google-chromecast` from PnP `18d1:9450`; the daemon advertised
`atvv-google-chromecast` with `atvv.profile="google-chromecast"`, decoded
240-byte frames (54 frames / 25,920 samples in two seconds), sent bare
`MIC_EXTEND`, stopped cleanly, and reconnected after
`bluetoothctl disconnect` (CAPS re-read, final `state ready`). A physical
press log mapped Power `KEY_COFFEE`, Home `KEY_HOMEPAGE`, Back `KEY_BACK`, OK
`KEY_SELECT`, Input `KEY_TV`, Mute `KEY_MUTE`, YouTube
`KEY_CAMERA_ACCESS_DISABLE`, and Netflix `KEY_CAMERA_ACCESS_TOGGLE`. The
Assistant key emitted no evdev event and sent `AUDIO_START 04 03 02 14`
directly; the daemon logged `session trigger held` / `session trigger
released` around two real presses. See
`docs/research/chromecast-hardware-evidence.md`.

## Out of scope

- Simultaneous devices and per-device sources (`atvv-<address>`).
- GUI pairing, forgetting, or BlueZ scanning; `atvvctl pair` remains the
  pairing path.

## Exit criteria

- ✅ Known devices persist and the active device survives a daemon restart.
- ✅ Switching reconnects the transport and keeps the source name for the same
  profile; a failed switch restores the old device.
- ✅ Per-device layouts load into the board/input runtime; dirty switching is
  blocked.
- ⬜ Only the active device's HID node is grabbed; capture/test ends on switch.
  Address filtering and capture/test teardown are implemented and unit-tested;
  the Chromecast node was grabbed alone and its key map recorded, but the real
  two-remote HID run is still pending.
- ✅ CLI, MCP, status, and doctor expose the active device; docs updated.
- ✅ The `-git` package contains the GUI; GTK stays out of the base package.
  `atvv-git`/`atvv-gui-git 0.2.0.r21.g9721e88-1` are built.

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
- 2026-11-02 — schema v3 registry and migration implemented; `pair` infers the
  profile from PnP ids; `SelectDevice` swaps transport, session, input, and
  source (recreating it on a profile change); `GetDevices`/`DeviceChanged`,
  `atvvctl device list|use|rename`, MCP `devices`/`select_device`, the GUI
  device popover with a dirty block, and per-device layouts are implemented.
  The Google Chromecast profile was added and its ATVV audio, keepalive,
  reconnect, physical HID map, and direct-Assistant `AUDIO_START` run are
  recorded; the daemon now synthesizes the session trigger for that profile.
  Mock two-device D-Bus smoke passed; the package was rebuilt as
  `0.2.0.r12.g7ec5357-1`. Pending: the real RC003+Chromecast switch.
- 2026-11-02 — A3 stream-id command test: keepalive `[0E, 01]` was sent twice
  during a held stream and the stream continued to a normal release; manual
  close `[0D, 00]` stopped the stream and a second session streamed again.
  `google-chromecast` now uses stream-id close/keepalive, matching the
  documented A0 firmware. Recorded in
  `docs/research/chromecast-hardware-evidence.md`; A0 Linux run remains
  pending.
- 2026-11-02 — built the r21 `-git` packages from the physical-order commit
  for operator installation with the split recipe pointed at the local
  repository; the recipe is restored. `docs/handoff.md` carries the sizes and
  sha256 values.
- 2026-11-02 — the board columns are now a per-profile presentation fact in
  `RemoteLayout`, ordered by each key's hotspot position and split evenly
  (6/6 RC003, 7/7 Chromecast). The board moves cards between columns when the
  profile changes; the GUI smoke asserts the actual column order.
- 2026-11-02 — built the profile-driven board `-git` packages from commit
  `e3da3c6` for operator installation with the split recipe pointed at the
  local repository; the recipe is restored. `docs/handoff.md` carries the
  sizes and sha256 values.
- 2026-11-02 — profile-driven board: `RemoteButton` gained `mute`, `youtube`,
  and `netflix`; `DeviceProfile` now declares its physical `buttons` and
  label overrides, and the Chromecast maps `KEY_MUTE`,
  `KEY_CAMERA_ACCESS_DISABLE`, and `KEY_CAMERA_ACCESS_TOGGLE` instead of
  passing them through. Pass-through defaults are derived from the profile's
  primary keys. The GUI hides buttons the active profile lacks (no Menu on
  the Chromecast), labels its `tv` slot as Input, adds the three hotspots,
  and filters the preset preview/application to the profile's buttons.
  Capture mode now publishes the session-driven Assistant edge as a `voice`
  `ButtonEvent` and does not hold the configured trigger while testing.
  `cargo test --workspace` and workspace clippy passed.
- 2026-11-02 — the GUI now selects an SVG asset and hotspot table per device
  profile. The RC003 asset moved to `remote-miremote-rc003.svg`; the new
  `remote-google-chromecast.svg` follows the Chromecast layout and shows the
  side volume buttons, which the front photo cannot show. The Chromecast asset
  was then redrawn from the product photo: full stadium body, neutral white
  palette instead of the warm RC003 tones, photo-derived button sizes and row
  spacing, solid icons, thin black button rings, condensed wordmarks, and
  `CHROMECAST_LAYOUT` hotspots updated to the new geometry.
  `cargo test --workspace` (239 passed, 9 ignored) and
  `cargo clippy --workspace --all-targets -- -D warnings` passed.
