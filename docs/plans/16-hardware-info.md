# Step 16 — Hardware and battery information

Status: ✅ done
Depends on: 06, 07, 14
Touches: `crates/atvv-bluez/`, `crates/atvv-daemon/`, `crates/atvv-cli/`,
`crates/atvv-gui/`, `docs/`

## Goal

Show the connected remote's hardware facts without leaving the ATVV tools:
battery percentage, firmware, hardware and software revisions, serial number,
manufacturer, and model. Battery is the first thing the GUI shows on its home
view. The same facts are available in `atvvctl status`, `atvvctl doctor`, and
`atvv-mcp status`, because they come from the one `StatusView`.

## Evidence

Measured on the real RC003 (`C0:5D:39:F8:2D:4C`, BlueZ 5.87, 2026-10-31):

| Fact | Value | Source |
|---|---|---|
| Battery | 62 % | `org.bluez.Battery1.Percentage` |
| Firmware | `2671` | DIS `0x2A26` |
| Hardware revision | `V2.0` | DIS `0x2A27` |
| Software revision | `A.7.0.6` | DIS `0x2A28` |
| Serial number | `250519` | DIS `0x2A25` |
| Model number | `RC003` | DIS `0x2A24` |
| Manufacturer | `MIOM` | DIS `0x2A29` |

`bluer` 0.17.4 exposes `Device::battery_percentage()` and GATT service /
characteristic reads, so the daemon can collect these through the BlueZ link
it already owns. The Battery Service (`0x180F`) is also readable over GATT,
but BlueZ has already read and cached it; use the cached property and avoid a
second GATT transaction per poll.

## Design

- `atvv-bluez` owns collection. `BluezLink::connect` reads the DIS
  characteristics once after service discovery and reads the battery from
  BlueZ. `BluezLink::refresh_battery` updates only the battery; DIS values do
  not change during a session.
- `atvv-daemon` owns the view model. `Transport::hardware` returns the current
  facts; `BluezTransport` caches them and refreshes the battery when asked.
  The daemon asks on connect and every `HARDWARE_REFRESH` (60 s), and keeps
  the last known values when the link is down. `MockConfig` carries canned
  facts so the rest of the stack is testable without hardware.
- `StatusView.hardware` is the only new D-Bus field. `atvvctl status` and
  `atvv-mcp` pick it up automatically; `atvvctl doctor` prints one concise
  `hardware` line.
- `atvv-gui` renders it in the sidebar home view: a battery row at the top
  with a level icon, and a Hardware group with the DIS facts. The GUI reads
  status through the existing 2 s worker poll; it never talks to BlueZ.
- Values are optional. A missing characteristic or a disconnected remote
  yields `null` in JSON and an empty row, never an error.

## Deliverables

- ✅ `atvv-bluez`: `HardwareInfo`; read the DIS service `0x180A`
  characteristics `0x2A24`–`0x2A29`; read `Device::battery_percentage`;
  refresh the battery without re-reading DIS.
- ✅ `Transport::hardware` with a default empty value; `MockConfig.hardware`;
  `BluezTransport` caches facts and refreshes the battery when asked; the
  daemon throttles that to 60 s.
- ✅ `HardwareView` in `StatusView.hardware`; daemon refresh on connect and
  every 60 s while connected; last known values survive a disconnect.
- ✅ `atvvctl doctor` hardware line; `atvvctl status` JSON carries the object.
- ✅ GUI: battery row at the top of the sidebar with a level icon and
  percentage; Hardware group with firmware/hardware/software/serial/
  manufacturer/model; hidden rows when a fact is absent.
- ✅ Tests: mock-backed status propagation, DIS string decoding, GUI row
  update; real RC003 smoke.
- ✅ Docs: `docs/gui.md`, this step, handoff, hardware evidence.

## Verification

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
xvfb-run -a cargo test -p atvv-gui --test ui
```

Real-device smoke (daemon on a private bus, or the installed unit):

```text
atvvctl status | jq .hardware
atvvctl doctor
```

Expected on the RC003: `battery_percent: 62`, `firmware_revision: "2671"`,
`hardware_revision: "V2.0"`, `software_revision: "A.7.0.6"`,
`serial_number: "250519"`.

## Exit criteria

- ✅ `StatusView.hardware` is populated from BlueZ while connected.
- ✅ Battery is visible on the GUI home view without opening a subpage.
- ✅ Mutating hardware is not required: all reads are passive.
- ✅ Missing values stay `null`/hidden; the battery row reads `Unknown`
  instead of inventing a percentage.
- ✅ A disconnect keeps the last known facts and the GUI marks them as stale.
- ✅ The mock transport drives the same JSON and GUI path.
- ✅ Real RC003 values are recorded with the command and date.

## Results

- `cargo fmt --all -- --check` → clean.
- `cargo clippy --workspace --all-targets -- -D warnings` → clean.
- `cargo test --workspace` → all targets passed; the new DIS decoder test and
  the mock-backed `hardware` assertions pass.
- `xvfb-run -a cargo test -p atvv-gui --test ui` → passed; the test injects
  `battery 62 %`, `firmware 2671`, and `serial 250519`, then checks the
  `last known` label with a disconnected status.
- Real RC003 (`C0:5D:39:F8:2D:4C`, 2026-10-31, workspace `atvvd` on a private
  bus, installed unit stopped/restarted): `atvvctl status` returned
  `battery_percent: 62`, `firmware_revision: "2671"`,
  `hardware_revision: "V2.0"`, `software_revision: "A.7.0.6"`,
  `serial_number: "250519"`, `manufacturer: "MIOM"`, `model: "RC003"`;
  `atvvctl doctor` reported
  `hardware ok battery 62% · firmware 2671 · hw V2.0 · sw A.7.0.6 · serial 250519`.

## Risks

- Battery is a cached BlueZ property; it updates when the remote notifies
  `0x2A19`, not on every read. A 60 s poll is enough and avoids GATT churn.
- Some devices omit DIS or require an encrypted read. Each characteristic is
  optional; a failed read leaves that field empty.
- Reading DIS at connect adds one service walk. It is bounded and happens
  only on connect/reconnect, not in the audio loop.
- The GUI must not show a stale battery as current. When `connected` is false,
  the row says so while retaining the last value.

## Progress log

- 2026-10-31 — step planned after reading the Battery1 and DIS facts from the
  real RC003.
- 2026-10-31 — `atvv-bluez` reads the DIS service and cached battery during
  connect/reconnect and exposes `HardwareInfo`; `refresh_battery` updates only
  the cached property.
- 2026-10-31 — `Transport::hardware`, `MockConfig.hardware`, and
  `StatusView.hardware` are implemented; the daemon refreshes on connect and
  every 60 s while connected, and keeps the last known values on disconnect.
- 2026-10-31 — `atvvctl doctor` prints the hardware line; `atvv-mcp status`
  carries the same object.
- 2026-10-31 — GUI sidebar home shows the Battery row and Hardware group; the
  UI smoke test covers connected and last-known states.
- 2026-10-31 — real-device values recorded in
  `docs/research/rc003-hardware-evidence.md`.
