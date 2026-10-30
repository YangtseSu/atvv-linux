# Step 05 — input and uinput

Status: ✅ done
Depends on: 01
Touches: `crates/atvv-input/`, `Cargo.toml`

## Goal

A hardware-free input path: discover an RC003-like evdev device, grab it,
recognize the 12 buttons, and emit synthetic keys through uinput. A fake remote
must exercise the path without a physical device and without touching the real
keyboard.

## Deliverables

- ✅ `atvv-input` crate using `evdev`.
- ✅ Profile-driven device matching (`vendor_id`, `product_id`, names), never a
  bare brand substring.
- ✅ `EVIOCGRAB` on the keyboard/consumer node only; do not grab mouse/absolute
  nodes by default.
- ✅ Key-code map for RC003: voice `KEY_F5`, TV `KEY_GRAVE`, power, OK, back,
  directions, home, menu, volume up/down.
- ✅ Gesture engine with single/double/long/repeat, the RC003 stable-release
  latch (600 ms), double window 300 ms, long press 550 ms, repeat start 350 ms,
  suppression 180 ms, and profile overrides.
- ✅ uinput virtual keyboard: register every keycode the action map can emit;
  tap = press frame + release frame; hold-to-talk needs separate press/release.
- ✅ Action grammar: `none`, `key:<name>`, `combo:<mods>+<name>`,
  `exec:<command>`, `dictation:start|stop|toggle`.
- ✅ Fake remote test device (unique name/VID) plus `evtest`/`libinput` smoke
  instructions; test-only profile, never matched by production profiles.
- ✅ Unit tests for key parsing, gesture timing, repeat gating, and the virtual
  keyboard frame builder.

## Design notes

- Research: `docs/research/landscape.md` §3.2 and §5, and
  `docs/research/protocol-and-device.md` §6.
- uinput is used instead of `wtype` because compositor keybinds must fire. This
  is the same finding recorded by sayall-omarchy; see
  `docs/research/reference-projects.md`.
- The voice key is not in the button map; it drives ATVV. It is still grabbed so
  `KEY_F5` cannot reload a focused browser.
- The virtual keyboard emits on a dedicated blocking thread; the async side
  uses an unbounded channel.
- Permission failure degrades key mapping only; it must not take the daemon
  down.
- The fake device is a test fixture. It must not use the RC003 VID/PID in a way
  that risks grabbing a real device.

## Implementation

- `crates/atvv-input/src/keymap.rs`: profile key map and key-name resolution
  (verbatim kernel names, aliases, synthesized `KEY_*` names).
- `crates/atvv-input/src/action.rs`: action grammar, default pass-through map,
  repeat policy, gesture policy.
- `crates/atvv-input/src/gesture.rs`: clock-free per-button FSM with the latch,
  suppression, double window, long press, and repeat ticks.
- `crates/atvv-input/src/vkbd.rs`: registered uinput keyboard, frame builder,
  blocking writer thread, degraded (no `/dev/uinput`) behavior.
- `crates/atvv-input/src/device.rs`: profile matching, pointer-node exclusion,
  discovery.
- `crates/atvv-input/src/runtime.rs`: per-node non-blocking readers, the engine
  thread, and `RemoteEvent` delivery.
- `crates/atvv-input/src/fake.rs`: `FAKE_REMOTE` (VID `0x1209`, PID `0x0002`,
  name `atvv-fake-remote`) and its uinput device.
- `DeviceProfile` now owns `button_keys` and `gesture_timing`; RC003 uses
  `KEY_HOME` for home, `KEY_COMPOSE` for the application/menu key, and
  `KEY_ENTER`/`KEY_KPENTER` for OK. The old `voice_key`/`tv_key` fields are
  gone. The 2026-10-31 evdev descriptor corrected the earlier
  `KEY_HOMEPAGE`/`KEY_MENU` assumption.

## Out of scope

Real BLE, PipeWire, D-Bus, and per-app profiles.

## Verification

Observed on 2026-10-30, Linux 7.3.0-3-cachyos, `/dev/uinput` writable via the
`input` group:

```text
cargo test -p atvv-input
  → 37 passed; 0 failed; 1 ignored

cargo test --workspace
  → 74 passed; 0 failed; 1 ignored (7 test binaries)

cargo run -q -p atvv-cli -- input fake-remote
  fake remote: /dev/input/event25 (atvv-fake-remote)
  grabbed: /dev/input/event25
  virtual keyboard: /dev/input/event26
  ...
  recognized 12/12 logical buttons
  voice: press
  voice: release
  exit=0

cargo test -p atvv-input hold_and_release_reach_the_kernel -- --ignored
  → 1 passed (hold pressed and release lifted KEY_LEFTALT)

libinput debug-events --device /dev/input/event26   # during --hold 40
  -event26  DEVICE_ADDED     atvv-virtual-keyboard   cap:k
   event26  KEYBOARD_KEY     KEY_UP (103) pressed
   event26  KEYBOARD_KEY     KEY_UP (103) released
   ... every ~1 s
```

The fake remote is the only grabbed node; `FAKE_REMOTE` is not in
`PROFILES`, and its name/ids do not match RC003. `evtest` sees the virtual
keyboard as `atvv-virtual-keyboard`; the grabbed fake node is intentionally not
readable by another client.

## Exit criteria

- ✅ A fake remote produces all 12 logical buttons.
- ✅ Single/double/long/repeat timing is deterministic in tests.
- ✅ uinput tap and hold are both correct (tap observed via `libinput`; hold and
  release observed by the ignored kernel test).
- ✅ A missing `/dev/uinput` degrades mapping without crashing: `vkbd: None`
  reports `virtual keyboard unavailable; key mapping degraded` on each mapped
  action, and no code path panics.
- ✅ No real input device is grabbed by the test: discovery with `FAKE_REMOTE`
  matched only `/dev/input/event25`.

## Risks

- Grabbing all matching nodes can break a remote's air-mouse/gyro; discovery
  rejects nodes with relative/absolute axes or pointer buttons.
- udev `uaccess`/group permissions vary by distribution; step 09 owns the
  policy. The local smoke ran with the `input` group; do not ship that as the
  policy.
- The RC003 stable-release latch is based on field reports; step 07 confirms it
  on hardware.
- The fake fixture has its own short gesture timing; RC003 timing is tested with
  unit tests and stays the production profile.

## Progress log

- 2026-10-29 — step planned; no implementation yet.
- 2026-10-30 — implemented the input path. Key/button facts and latch behavior
  cross-checked against `fan-tastic-z/sayall-omarchy@4d10971`
  (`crates/sayall-hid/src/device.rs`, `crates/sayall-core/src/gesture.rs`,
  `crates/sayall-core/src/button_map.rs`). Added `atvv-input`, moved button keys
  and gesture timing into `DeviceProfile`, and added the `atvvctl input
  fake-remote` diagnostics. Verified 12/12 buttons, voice edges, grabbed-node
  isolation, compositor-visible `KEY_UP` via `libinput`, and uinput hold/release.
