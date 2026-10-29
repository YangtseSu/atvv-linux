# Step 05 — input and uinput

Status: ⬜ not-started
Depends on: 01
Touches: `crates/atvv-input/`, `Cargo.toml`

## Goal

A hardware-free input path: discover an RC003-like evdev device, grab it,
recognize the 12 buttons, and emit synthetic keys through uinput. A fake remote
must exercise the path without a physical device and without touching the real
keyboard.

## Deliverables

- ⬜ `atvv-input` crate using `evdev`.
- ⬜ Profile-driven device matching (`vendor_id`, `product_id`, names), never a
  bare brand substring.
- ⬜ `EVIOCGRAB` on the keyboard/consumer node only; do not grab mouse/absolute
  nodes by default.
- ⬜ Key-code map for RC003: voice `KEY_F5`, TV `KEY_GRAVE`, power, OK, back,
  directions, home, menu, volume up/down.
- ⬜ Gesture engine with single/double/long/repeat, the RC003 stable-release
  latch (600 ms), double window 300 ms, long press 550 ms, repeat start 350 ms,
  suppression 180 ms, and profile overrides.
- ⬜ uinput virtual keyboard: register every keycode the action map can emit;
  tap = press frame + release frame; hold-to-talk needs separate press/release.
- ⬜ Action grammar: `none`, `key:<name>`, `combo:<mods>+<name>`,
  `exec:<command>`, `dictation:start|stop|toggle`.
- ⬜ Fake remote test device (unique name/VID) plus `evtest`/`libinput` smoke
  instructions; test-only profile, never matched by production profiles.
- ⬜ Unit tests for key parsing, gesture timing, repeat gating, and the virtual
  keyboard frame builder.

## Design notes

- uinput is used instead of `wtype` because compositor keybinds must fire. This
  is the same finding recorded by sayall.
- The voice key is not in the button map; it drives ATVV. It is still grabbed so
  `KEY_F5` cannot reload a focused browser.
- The virtual keyboard emits on a dedicated blocking thread; the async side
  uses an unbounded channel.
- Permission failure degrades key mapping only; it must not take the daemon
  down.
- The fake device is a test fixture. It must not use the RC003 VID/PID in a way
  that risks grabbing a real device.

## Out of scope

Real BLE, PipeWire, D-Bus, and per-app profiles.

## Verification

```text
cargo test -p atvv-input
cargo run -q -p atvv-cli -- input fake-remote
evtest                # keys from the fake device
libinput debug-events # compositor-visible keys
```

Also verify that no real keyboard is grabbed and that `KEY_F5` is swallowed
when the fake remote is the grabbed device.

## Exit criteria

- ⬜ A fake remote produces all 12 logical buttons.
- ⬜ Single/double/long/repeat timing is deterministic in tests.
- ⬜ uinput tap and hold are both correct.
- ⬜ A missing `/dev/uinput` degrades mapping without crashing.
- ⬜ No real input device is grabbed by the test.

## Risks

- Grabbing all matching nodes can break a remote's air-mouse/gyro; restrict to
  the keyboard node.
- udev `uaccess`/group permissions vary by distribution; step 09 owns the
  policy.
- The RC003 stable-release latch is based on field reports; step 07 confirms it
  on hardware.

## Progress log

- 2026-10-29 — step planned; no implementation yet.
