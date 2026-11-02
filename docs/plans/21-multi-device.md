# Step 21 — Multi-device switching

Status: ✅ done
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
  a recorded run. Recorded 2026-11-02 with the RC003 and Chromecast; see
  Verification.

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

### Two-device switch (2026-11-02)

Both devices were connected to this host: RC003 `C0:5D:39:F8:2D:4C` and
Chromecast `74:46:B3:22:2D:D5`. The existing schema-2 file held only the
RC003. `atvvctl pair 74:46:B3:22:2D:D5` migrated it to schema 3 and preserved
the per-device profiles:

```text
schema_version = 3
active_device = "74:46:B3:22:2D:D5"

[devices."74:46:B3:22:2D:D5"]
profile = "google-chromecast"
layout = "default"

[devices."C0:5D:39:F8:2D:4C"]
profile = "miremote-rc003"
layout = "custom"
```

The daemon ran from the packaged `atvv-git 0.2.0.r26.g2f788b5-1` binary with
`--verbose`. On the Chromecast:

```text
$ atvvctl device list
active: 74:46:B3:22:2D:D5
* 74:46:B3:22:2D:D5 Chromecast Remote            google-chromecast      default          connected 94%
  C0:5D:39:F8:2D:4C Xiaomi Bluetooth Remote 2 Pro (RC003) miremote-rc003         custom           offline

status: ready 74:46:B3:22:2D:D5 google-chromecast atvv-google-chromecast
input.grabbed: ["/dev/input/event25"]
EVIOCGRAB probe: event25 GRABBED (EBUSY); event26 free

$ atvvctl mic start   # manual MIC_OPEN
audio frames=81 samples=38880 streaming=true dropped=0
$ atvvctl record --wav /tmp/chromecast-4s.wav --seconds 4 --trigger
recording 4s at 16000 Hz ... captured
# WAV: 64,000 frames, 4.000 s, 49.3% non-zero, peak 32768
```

Capture mode was verified while the Chromecast was active: `atvvctl capture
--timeout 10` plus `atvvctl mic start` printed
`button: voice trigger: single pressed: true` and exited 0.

Switching to the RC003:

```text
$ atvvctl device use C0:5D:39:F8:2D:4C
active: C0:5D:39:F8:2D:4C
atvvd: device switched to C0:5D:39:F8:2D:4C (miremote-rc003 ...)
atvvd: control [0b, 01, 00, 02, 03, 00, 78, 00, 00]; caps frame=120
state: ready; source: atvv-miremote-rc003; layout: custom
input.grabbed: ["/dev/input/event26"]
EVIOCGRAB probe: event26 GRABBED; event25 free
```

Manual `MIC_OPEN` on the RC003 returned `AUDIO_START` but no frames and hit
`audio frame timeout`, so the firmware opens the microphone only from its own
voice key. Two physical presses produced `control [04, 03, 02, 57]` /
`[04, 03, 02, 58]`, `voice key pressed` / `voice key released`, and
`frames=236 samples=56640` (7.08 s) with `dropped_frames=0` and a 32768 peak.

Capture teardown across a switch was tested with the Chromecast's
session-driven voice edge as the control: `atvvctl capture --timeout 15` was
started on the RC003, the device was switched to the Chromecast, and
`atvvctl mic start` produced no `ButtonEvent`; capture printed
`no button pressed within 15s` (exit 1). The control run above emitted the
same edge while capture was live, so the switch ended capture mode.

Switching back restored `atvv-miremote-rc003`, grabbed only
`/dev/input/event26`, and `atvvctl doctor` reported `registry ok 2 known;
active C0:5D:39:F8:2D:4C` with every check ok. The config persisted both
registry entries and `active_device = "C0:5D:39:F8:2D:4C"`; a systemd user
service restart came up on the RC003 with the same source and grab.

The pairing path had a schema-2 migration defect: `save_device` used the new
device's profile as the fallback for the file's existing single device, so
pairing the Chromecast rewrote the RC003 entry as `google-chromecast`. It now
takes the legacy profile explicitly (`RC003` is the only profile a schema-2
file can hold), with a regression test that pairs a second device into a
schema-2 file.

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
- ✅ Only the active device's HID node is grabbed; capture/test ends on switch.
  Recorded 2026-11-02 with both remotes connected: `status.input.grabbed` and
  an external `EVIOCGRAB` probe showed only the active node (event25 for the
  Chromecast, event26 for the RC003), and capture mode ended on the switch.
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
- 2026-11-02 — fixed the schema-2 → 3 pairing path: `save_device` used the
  profile of the device being paired as the fallback for the file's existing
  single device, so pairing the Chromecast rewrote the RC003 entry as
  `google-chromecast`. Migration now takes the legacy device's profile
  explicitly (`RC003` is the only profile a schema-2 file can hold), with a
  regression test that pairs a second device into a schema-2 file.
  `cargo test --workspace` → 245 passed, 9 ignored; workspace clippy clean.
- 2026-11-02 — real two-device run with the RC003 (`C0:5D:39:F8:2D:4C`) and
  the Chromecast (`74:46:B3:22:2D:D5`) on one host: paired/migrated to schema 3
  with both entries, started on the Chromecast (`atvv-google-chromecast`,
  event25 grabbed, 81 frames / 38,880 samples, a 4 s WAV with 49.3% non-zero),
  switched to the RC003 (`atvv-miremote-rc003`, event26 grabbed, physical voice
  presses → 236 frames / 56,640 samples), switched back, and confirmed capture
  mode ended on a switch. Both devices stayed known and the service restarted
  on the RC003. The RC003 accepted `AUDIO_START` for a manual `MIC_OPEN` but
  sent no frames; it opens the microphone only from its own voice key.
