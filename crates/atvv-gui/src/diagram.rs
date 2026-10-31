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

use crate::i18n::{tr, tr_args};
use crate::model::button_label;

const VIEWBOX_WIDTH: f64 = 320.0;
const VIEWBOX_HEIGHT: f64 = 950.0;

#[derive(Debug, Clone, Copy)]
struct Hotspot {
    button: RemoteButton,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

const HOTSPOTS: [Hotspot; 13] = [
    Hotspot {
        button: RemoteButton::Power,
        x: 0.222813,
        y: 0.036632,
        width: 0.1875,
        height: 0.063158,
    },
    Hotspot {
        button: RemoteButton::Voice,
        x: 0.589688,
        y: 0.036632,
        width: 0.1875,
        height: 0.063158,
    },
    Hotspot {
        button: RemoteButton::Tv,
        x: 0.519375,
        y: 0.497158,
        width: 0.25,
        height: 0.084211,
    },
    Hotspot {
        button: RemoteButton::Up,
        x: 0.353125,
        y: 0.115789,
        width: 0.29375,
        height: 0.048421,
    },
    Hotspot {
        button: RemoteButton::Down,
        x: 0.353125,
        y: 0.258947,
        width: 0.29375,
        height: 0.048421,
    },
    Hotspot {
        button: RemoteButton::Left,
        x: 0.20625,
        y: 0.162105,
        width: 0.146875,
        height: 0.098947,
    },
    Hotspot {
        button: RemoteButton::Right,
        x: 0.646875,
        y: 0.162105,
        width: 0.146875,
        height: 0.098947,
    },
    Hotspot {
        button: RemoteButton::Ok,
        x: 0.353125,
        y: 0.162105,
        width: 0.29375,
        height: 0.098947,
    },
    Hotspot {
        button: RemoteButton::Back,
        x: 0.229063,
        y: 0.315789,
        width: 0.25,
        height: 0.084211,
    },
    Hotspot {
        button: RemoteButton::Home,
        x: 0.229063,
        y: 0.405474,
        width: 0.25,
        height: 0.084211,
    },
    Hotspot {
        button: RemoteButton::Menu,
        x: 0.229063,
        y: 0.496526,
        width: 0.25,
        height: 0.084211,
    },
    Hotspot {
        button: RemoteButton::VolumeUp,
        x: 0.51875,
        y: 0.315789,
        width: 0.257813,
        height: 0.087474,
    },
    Hotspot {
        button: RemoteButton::VolumeDown,
        x: 0.51875,
        y: 0.402105,
        width: 0.257813,
        height: 0.088105,
    },
];

const HOTSPOT_CSS: [&str; 13] = [
    "hotspot-power",
    "hotspot-voice",
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

const DIAGRAM_WIDTH: f64 = 200.0;
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
            .width_request(DIAGRAM_WIDTH as i32)
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
                let label = gtk::Label::new(Some(tr("Remote diagram unavailable")));
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
            let tooltip = if hotspot.button == RemoteButton::Voice {
                tr("Voice key: drives the ATVV microphone and is not remappable").to_string()
            } else {
                tr_args("{label}: click to edit", &[("label", label)])
            };
            let button = gtk::Button::builder()
                .has_frame(false)
                .focusable(true)
                .tooltip_text(tooltip)
                .build();
            button.add_css_class("remote-hotspot");
            button.add_css_class(HOTSPOT_CSS[index]);
            button.set_widget_name(&format!("hotspot-{}", hotspot.button.name()));
            button.update_property(&[gtk::accessible::Property::Label(&tr_args(
                "{label} key{suffix}",
                &[("label", label), ("suffix", "")],
            ))]);
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
    pub fn refresh(
        &self,
        model: &crate::model::GuiModel,
        pressed: Option<RemoteButton>,
        voice_selected: bool,
    ) {
        let conflicts = model.conflicts();
        for (button, widget) in &self.buttons {
            for class in [
                "selected", "bound", "unbound", "conflict", "pressed", "voice",
            ] {
                widget.remove_css_class(class);
            }
            let editable = *button != RemoteButton::Voice;
            if editable {
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
            } else {
                widget.add_css_class("voice");
                if voice_selected {
                    widget.add_css_class("selected");
                }
            }
            if pressed == Some(*button) {
                widget.add_css_class("pressed");
            }
            let suffix = if !editable {
                tr(", not remappable")
            } else if conflicts.contains(button) {
                tr(", conflicting action")
            } else {
                ""
            };
            widget.update_property(&[gtk::accessible::Property::Label(&tr_args(
                "{label} key{suffix}",
                &[("label", button_label(*button)), ("suffix", suffix)],
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
        assert!(seen.contains(&RemoteButton::Voice));
        assert_eq!(seen.len(), RemoteButton::LOGICAL.len() + 1);
    }
}
