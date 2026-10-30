// SPDX-License-Identifier: GPL-3.0-or-later

//! Remote diagram: the SVG picture plus one real `GtkButton` per hotspot.
//!
//! Hotspot positions are fractions of the SVG view box, so the buttons and the
//! picture share one coordinate system. `GtkAspectFrame` keeps the picture at
//! the view-box ratio; a size change only recomputes the scaled rectangles.

use std::collections::BTreeMap;

use atvv_protocol::profile::RemoteButton;
use gtk::prelude::*;
use gtk::{gdk, glib};

use crate::model::button_label;

const VIEWBOX_WIDTH: f64 = 320.0;
const VIEWBOX_HEIGHT: f64 = 820.0;

#[derive(Debug, Clone, Copy)]
struct Hotspot {
    button: RemoteButton,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

const HOTSPOTS: [Hotspot; 12] = [
    Hotspot {
        button: RemoteButton::Power,
        x: 0.275,
        y: 0.060976,
        width: 0.15,
        height: 0.058537,
    },
    Hotspot {
        button: RemoteButton::Tv,
        x: 0.56875,
        y: 0.592683,
        width: 0.175,
        height: 0.068293,
    },
    Hotspot {
        button: RemoteButton::Up,
        x: 0.3375,
        y: 0.153659,
        width: 0.325,
        height: 0.07561,
    },
    Hotspot {
        button: RemoteButton::Down,
        x: 0.3375,
        y: 0.331707,
        width: 0.325,
        height: 0.07561,
    },
    Hotspot {
        button: RemoteButton::Left,
        x: 0.175,
        y: 0.229268,
        width: 0.19375,
        height: 0.102439,
    },
    Hotspot {
        button: RemoteButton::Right,
        x: 0.63125,
        y: 0.229268,
        width: 0.19375,
        height: 0.102439,
    },
    Hotspot {
        button: RemoteButton::Ok,
        x: 0.36875,
        y: 0.229268,
        width: 0.2625,
        height: 0.102439,
    },
    Hotspot {
        button: RemoteButton::Back,
        x: 0.2625,
        y: 0.417073,
        width: 0.175,
        height: 0.068293,
    },
    Hotspot {
        button: RemoteButton::Home,
        x: 0.2625,
        y: 0.504878,
        width: 0.175,
        height: 0.068293,
    },
    Hotspot {
        button: RemoteButton::Menu,
        x: 0.2625,
        y: 0.592683,
        width: 0.175,
        height: 0.068293,
    },
    Hotspot {
        button: RemoteButton::VolumeUp,
        x: 0.55625,
        y: 0.421951,
        width: 0.2,
        height: 0.068293,
    },
    Hotspot {
        button: RemoteButton::VolumeDown,
        x: 0.55625,
        y: 0.490244,
        width: 0.2,
        height: 0.068293,
    },
];

const HOTSPOT_CSS: [&str; 12] = [
    "hotspot-power",
    "hotspot-tv",
    "hotspot-up",
    "hotspot-down",
    "hotspot-left",
    "hotspot-right",
    "hotspot-ok",
    "hotspot-back",
    "hotspot-home",
    "hotspot-menu",
    "hotspot-volume-up",
    "hotspot-volume-down",
];

const DIAGRAM_WIDTH: f64 = 220.0;
const DIAGRAM_HEIGHT: f64 = DIAGRAM_WIDTH * VIEWBOX_HEIGHT / VIEWBOX_WIDTH;

fn scaled(hotspot: Hotspot, width: f64, height: f64) -> (f64, f64, i32, i32) {
    let x = (hotspot.x * width).round();
    let y = (hotspot.y * height).round();
    let w = (hotspot.width * width).round().max(18.0) as i32;
    let h = (hotspot.height * height).round().max(18.0) as i32;
    (x, y, w, h)
}

/// Callback type for a hotspot click.
pub type SelectHandler = Box<dyn Fn(RemoteButton)>;

pub struct RemoteDiagram {
    root: gtk::AspectFrame,
    fixed: gtk::Fixed,
    buttons: BTreeMap<RemoteButton, gtk::Button>,
}

impl RemoteDiagram {
    #[must_use]
    pub fn new() -> Self {
        let root = gtk::AspectFrame::builder()
            .ratio((VIEWBOX_WIDTH / VIEWBOX_HEIGHT) as f32)
            .obey_child(false)
            .hexpand(true)
            .vexpand(true)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .width_request(220)
            .build();

        let fixed = gtk::Fixed::new();
        fixed.add_css_class("remote-canvas");
        fixed.set_hexpand(true);
        fixed.set_vexpand(true);
        fixed.set_size_request(DIAGRAM_WIDTH as i32, DIAGRAM_HEIGHT as i32);
        root.set_child(Some(&fixed));

        let base: gtk::Widget = match texture() {
            Some(texture) => {
                let picture = gtk::Picture::for_paintable(&texture);
                picture.set_content_fit(gtk::ContentFit::Contain);
                picture.set_can_shrink(true);
                picture.set_can_target(false);
                picture.upcast()
            }
            None => {
                let label = gtk::Label::new(Some("Remote diagram unavailable"));
                label.add_css_class("dim-label");
                label.set_can_target(false);
                label.upcast()
            }
        };
        base.set_size_request(DIAGRAM_WIDTH as i32, DIAGRAM_HEIGHT as i32);
        fixed.put(&base, 0.0, 0.0);

        let mut buttons = BTreeMap::new();
        for (index, hotspot) in HOTSPOTS.iter().enumerate() {
            let label = button_label(hotspot.button);
            let button = gtk::Button::builder()
                .has_frame(false)
                .focusable(true)
                .tooltip_text(format!("{label}: click to edit"))
                .build();
            button.add_css_class("remote-hotspot");
            button.add_css_class(HOTSPOT_CSS[index]);
            button.set_widget_name(&format!("hotspot-{}", hotspot.button.name()));
            button.update_property(&[gtk::accessible::Property::Label(&format!("{label} key"))]);
            fixed.put(&button, 0.0, 0.0);
            buttons.insert(hotspot.button, button);
        }

        for hotspot in HOTSPOTS {
            let Some(button) = buttons.get(&hotspot.button) else {
                continue;
            };
            let (x, y, w, h) = scaled(hotspot, DIAGRAM_WIDTH, DIAGRAM_HEIGHT);
            button.set_size_request(w, h);
            fixed.move_(button, x, y);
        }

        let layout_base = base.clone();
        let layout_buttons = buttons.clone();
        let last = std::cell::Cell::new((0, 0));
        fixed.add_tick_callback(move |fixed, _clock| {
            let width = fixed.width();
            let height = fixed.height();
            if width > 0 && height > 0 && (width, height) != last.get() {
                last.set((width, height));
                layout_base.set_size_request(width, height);
                for hotspot in HOTSPOTS {
                    let Some(button) = layout_buttons.get(&hotspot.button) else {
                        continue;
                    };
                    let (x, y, w, h) = scaled(hotspot, f64::from(width), f64::from(height));
                    button.set_size_request(w, h);
                    fixed.move_(button, x, y);
                }
            }
            glib::ControlFlow::Continue
        });

        Self {
            root,
            fixed,
            buttons,
        }
    }

    #[must_use]
    pub fn root(&self) -> &gtk::AspectFrame {
        &self.root
    }

    #[must_use]
    pub fn fixed(&self) -> &gtk::Fixed {
        &self.fixed
    }

    #[must_use]
    pub fn button(&self, button: RemoteButton) -> Option<&gtk::Button> {
        self.buttons.get(&button)
    }

    /// Connect every hotspot; the handler runs on the GTK main thread.
    pub fn connect_selected<F: Fn(RemoteButton) + 'static>(&self, handler: F) {
        let handler = std::rc::Rc::new(handler);
        for (button, widget) in &self.buttons {
            let handler = handler.clone();
            let button = *button;
            widget.connect_clicked(move |_| handler(button));
        }
    }

    /// Refresh the visual state classes from the model.
    pub fn refresh(&self, model: &crate::model::GuiModel, pressed: Option<RemoteButton>) {
        let conflicts = model.conflicts();
        for (button, widget) in &self.buttons {
            for class in ["selected", "bound", "unbound", "conflict", "pressed"] {
                widget.remove_css_class(class);
            }
            if *button == model.selected {
                widget.add_css_class("selected");
            }
            if conflicts.contains(button) {
                widget.add_css_class("conflict");
            } else if model.is_bound(*button) {
                widget.add_css_class("bound");
            } else {
                widget.add_css_class("unbound");
            }
            if pressed == Some(*button) {
                widget.add_css_class("pressed");
            }
            widget.update_property(&[gtk::accessible::Property::Label(&format!(
                "{} key{}",
                button_label(*button),
                if conflicts.contains(button) {
                    ", conflicting action"
                } else {
                    ""
                }
            ))]);
        }
    }
}

impl Default for RemoteDiagram {
    fn default() -> Self {
        Self::new()
    }
}

/// Decode the embedded SVG through GDK's texture loader.
fn texture() -> Option<gdk::Texture> {
    let bytes = glib::Bytes::from_static(include_bytes!("../resources/remote.svg"));
    gdk::Texture::from_bytes(&bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotspots_cover_every_logical_button_inside_the_view_box() {
        let mut seen = std::collections::BTreeSet::new();
        for hotspot in HOTSPOTS {
            assert!(seen.insert(hotspot.button), "duplicate {}", hotspot.button);
            assert!((0.0..=1.0).contains(&hotspot.x));
            assert!((0.0..=1.0).contains(&hotspot.y));
            assert!(hotspot.x + hotspot.width <= 1.0 + f64::EPSILON);
            assert!(hotspot.y + hotspot.height <= 1.0 + f64::EPSILON);
        }
        for button in RemoteButton::LOGICAL {
            assert!(seen.contains(&button), "{button} has no hotspot");
        }
        assert_eq!(seen.len(), RemoteButton::LOGICAL.len());
    }
}
