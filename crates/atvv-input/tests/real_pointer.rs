// SPDX-License-Identifier: GPL-3.0-or-later

//! Real-device acceptance for pointer actions.
//!
//! Run only with the RC003 connected, `/dev/uinput` and evdev access, and the
//! installed daemon stopped so this runtime can grab the remote node:
//!
//! ```text
//! systemctl --user stop atvvd
//! cargo test -p atvv-input --test real_pointer -- --ignored --nocapture
//! systemctl --user start atvvd
//! ```
//!
//! The test waits for one Up press and one OK press. Up moves the pointer five
//! units up, and OK emits a left click at the current cursor position.

use std::time::{Duration, Instant};

use atvv_input::{ActionMap, InputRuntime, Trigger, parse_action};
use atvv_protocol::profile::{RC003, RemoteButton};
use evdevil::Evdev;
use evdevil::event::{EventKind, Key, KeyState, Rel};

fn wait_timeout() -> Duration {
    Duration::from_secs(90)
}

fn open_reader(path: &std::path::Path) -> evdevil::EventReader {
    let deadline = Instant::now() + Duration::from_secs(3);
    let device = loop {
        match Evdev::open(path) {
            Ok(device) => break device,
            Err(_) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("cannot open {}: {error}", path.display()),
        }
    };
    device.set_nonblocking(true).unwrap();
    device.into_reader().unwrap()
}

#[test]
#[ignore = "needs a connected RC003, /dev/uinput, and manual Up/OK presses"]
fn rc003_pointer_actions_reach_the_kernel() {
    let mut map = ActionMap::defaults();
    map.set(
        RemoteButton::Up,
        Trigger::Single,
        parse_action("pointer:move:up:5").unwrap(),
    )
    .unwrap();
    map.set(
        RemoteButton::Ok,
        Trigger::Single,
        parse_action("pointer:click:left").unwrap(),
    )
    .unwrap();

    let runtime = InputRuntime::start(RC003, map).expect("start the RC003 input runtime");
    let path = runtime
        .pointer_path()
        .expect("the map must create a pointer device")
        .to_path_buf();
    println!("pointer device: {}", path.display());
    println!("1. Press and release Up; expect REL_Y = -5");
    println!("2. Press and release OK; expect BTN_LEFT press and release");
    println!("waiting up to {} s…", wait_timeout().as_secs());

    let mut reader = open_reader(&path);
    let mut moved = false;
    let mut clicked = false;
    let mut released = false;
    let deadline = Instant::now() + wait_timeout();
    while Instant::now() < deadline && !(moved && clicked && released) {
        for event in reader.events() {
            let Ok(event) = event else {
                panic!("pointer node read failed");
            };
            match event.kind() {
                EventKind::Rel(event) if event.rel() == Rel::Y => {
                    println!("REL_Y {}", event.value());
                    if event.value() == -5 {
                        moved = true;
                    }
                }
                EventKind::Key(event) if event.key() == Key::BTN_LEFT => {
                    println!("BTN_LEFT {:?}", event.state());
                    if event.state() == KeyState::PRESSED {
                        clicked = true;
                    }
                    if event.state() == KeyState::RELEASED {
                        released = true;
                    }
                }
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }

    assert!(moved, "Up did not emit REL_Y=-5");
    assert!(clicked, "OK did not press BTN_LEFT");
    assert!(released, "OK did not release BTN_LEFT");
    assert!(runtime.pointer_available());
}
