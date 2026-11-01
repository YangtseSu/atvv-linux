# Step 22 — Pointer actions

Status: ✅ done
Depends on: 05, 13, 14, 15
Touches: `crates/atvv-input/`, `crates/atvv-config/`, `crates/atvv-daemon/`,
`crates/atvv-cli/`, `crates/atvv-gui/`, `docs/`, tests

## Goal

Let a remote button emit a mouse click or move the pointer through a dedicated
`atvv-virtual-pointer` uinput device. A relative pointer needs `REL_X`/`REL_Y`
and the mouse button codes; a keyboard-only uinput device cannot produce a
pointer click, so `BTN_*` was previously rejected instead of implemented. The
gesture slots choose the movement distance: a direction key can map Single to
1 unit, Double to 10, and Long to 50. Keyboard-only maps do not create a
pointer device.

## Deliverables

- ✅ Action grammar:
  - `pointer:click:left|right|middle|side|extra|forward|back|task`
  - `pointer:move:up|down|left|right:<units>`, units 1–255.
  - `Action::Pointer(PointerAction)` with parse, display, and validation;
    raw `key:BTN_*`/`combo:BTN_*` is rejected with a message that points at
    `pointer:click:<button>`.
- ✅ `atvv-input` pointer backend:
  - `VirtualPointer` registers `REL_X`, `REL_Y`, and all eight button codes on
    `atvv-virtual-pointer` (vendor `0x1209`, product `0x0003`).
  - Click writes separate press and release frames; movement writes one
    signed relative frame. A blocking writer thread owns the device.
  - `InputRuntime` opens the pointer only when `ActionMap::actions()` contains
    a pointer action, and exposes `pointer_path()`/`pointer_available()`.
  - `Dispatcher` emits pointer actions; Test/capture mode suppresses them
    through the existing `actions_enabled` flag.
  - `InputRuntime::tap_action` supports pointer actions for `RunAction`.
- ✅ Daemon and CLI:
  - `RunAction` accepts pointer actions through the input runtime.
  - `InputView.pointer` reports whether the pointer device is active;
    `atvvctl status` exposes it as `input.pointer`.
  - `atvvctl map test` reports `kind: pointer` and no key program.
- ✅ GUI:
  - New *Pointer* action type with Click/Move, button, direction, and unit
    rows. The editor writes the grammar; the board renders `Left click`,
    `Pointer up 7`, and the other labels.
  - English and Simplified Chinese catalog entries.
- ✅ Docs:
  - `docs/configuration.md` and the Chinese sibling document the grammar,
    the 1–255 relative units, acceleration, dedicated device, and raw
    `BTN_*` rejection.
  - `docs/gui.md` and the Chinese sibling document the editor rows and the
    gesture-distance pattern.
- ✅ Tests:
  - Parser/display/boundary tests, raw `BTN_*` rejection, config validation,
    `KeyResolver::program` returning `NotEmittable`.
  - Runtime dispatch without a pointer reports a degraded error and suppresses
    emission during capture.
  - Kernel test `pointer::kernel_tests::click_and_move_reach_the_kernel` reads
    the pointer node and observes `BTN_LEFT` press/release and `REL_Y=-3`.
  - Fake-remote kernel test dispatches `Up` and `OK` to the pointer node.
  - GUI XTest smoke exercises the pointer click and move rows; i18n test checks
    the Chinese type label and board labels.
- ✅ [hardware] Real RC003 run (2026-11-01, `C0:5D:39:F8:2D:4C`): with
  `up.single = pointer:move:up:5` and `ok.single = pointer:click:left`, the
  ignored `real_pointer` test grabbed the RC003, created
  `/dev/input/event27`, and observed `REL_Y -5`, `BTN_LEFT PRESSED`, and
  `BTN_LEFT RELEASED` from physical presses; the final run with
  `INPUT_PROP_POINTER` passed in 7.93 s and `atvvd` was restarted afterwards.

## Out of scope

- Scroll-wheel actions, click-and-drag/hold, double-click synthesis, and
  absolute pointer positioning.
- Changing the desktop's pointer acceleration or speed settings. Relative
  units are compositor input counts, not guaranteed screen pixels.
- Per-device pointer configuration or a second mapping layer.
- An always-present pointer device; it exists only while the map needs it.

## Verification

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p atvv-input --lib -- --ignored \
  pointer::kernel_tests::click_and_move_reach_the_kernel
cargo test -p atvv-input --lib -- --ignored \
  runtime::kernel_tests::pointer_actions_are_dispatched_to_the_pointer_device
systemctl --user stop atvvd
cargo test -p atvv-input --test real_pointer -- --ignored --nocapture
systemctl --user start atvvd
GDK_BACKEND=x11 xvfb-run -a -s "-screen 0 1280x1024x24" \
  cargo test -p atvv-gui
```

Results on 2026-11-01: `fmt` and `clippy -D warnings` clean; the workspace
suite under Xvfb reported 212 passed and 8 ignored. The two pointer kernel
tests passed, including the fake-remote `RunAction` path (`REL_X=6`), and the
real RC003 test passed in 7.93 s on the final code with
`INPUT_PROP_POINTER`, observing `REL_Y=-5` and `BTN_LEFT` press/release. The GUI suites passed (32 unit tests, XTest `ui`, `narrow`, and
Chinese `i18n`); the XTest suite drives the new pointer rows.

## Exit criteria

- ✅ A saved pointer action produces pointer events on a dedicated uinput
  device; keyboard-only maps stay keyboard-only.
- ✅ Single/Double/Long slots independently choose click or movement actions;
  a direction key can use 1/10/50 units.
- ✅ Test/capture mode never moves or clicks.
- ✅ Raw `BTN_*` is rejected with an actionable error; the GUI offers the
  structural pointer editor instead.
- ✅ The pointer state is visible in `atvvctl status` and both docs pairs
  describe the behavior and the relative-unit limitation.
- ✅ [hardware] The real RC003 run records Up movement and OK click.

## Risks

- Relative units pass through the compositor's pointer acceleration. One unit
  may not be one pixel and small movements can be smoothed; this is a property
  of relative input devices, not a daemon bug.
- A pointer action needs the map to contain at least one pointer action when
  the runtime starts. `atvvctl map test --action pointer:...` only validates;
  it does not create a pointer device for an otherwise keyboard-only map.
- A left click lands at the current cursor position and is subject to the
  desktop's focus and click policy. The action is explicit user configuration,
  like a keyboard shortcut.
- The pointer device is named `atvv-virtual-pointer` and uses product `0x0003`
  so discovery cannot match it as the remote or the virtual keyboard.

## Progress log

- 2026-11-01 — Added pointer grammar, virtual pointer, runtime/daemon/GUI
  wiring, status field, docs, and synthetic/kernel tests.
- 2026-11-01 — Real RC003 acceptance passed: `/dev/input/event27`, REL_Y -5,
  BTN_LEFT press/release, 7.93 s on the final `INPUT_PROP_POINTER` code.
  `atvvd` restarted and active.
