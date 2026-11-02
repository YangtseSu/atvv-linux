// SPDX-License-Identifier: GPL-3.0-or-later

//! Remote diagram: the SVG picture plus one real `GtkButton` per hotspot.
//!
//! Hotspot positions are fractions of the SVG view box, so the buttons and the
//! picture share one coordinate system. `GtkAspectFrame` keeps the picture at
//! the view-box ratio; a size change only recomputes the scaled rectangles.
//! Each device family supplies its own vector asset and hotspot table.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::rc::Rc;

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

struct RemoteLayout {
    profile_id: &'static str,
    svg: &'static [u8],
    hotspots: &'static [Hotspot],
}

const RC003_SVG: &[u8] = include_bytes!("../resources/remote-miremote-rc003.svg");
const CHROMECAST_SVG: &[u8] = include_bytes!("../resources/remote-google-chromecast.svg");

const RC003_LAYOUT: RemoteLayout = RemoteLayout {
    profile_id: "miremote-rc003",
    svg: RC003_SVG,
    hotspots: &[
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
    ],
};

const CHROMECAST_LAYOUT: RemoteLayout = RemoteLayout {
    profile_id: "google-chromecast",
    svg: CHROMECAST_SVG,
    hotspots: &[
        Hotspot {
            button: RemoteButton::Up,
            x: 0.3625,
            y: 0.035789,
            width: 0.275,
            height: 0.073684,
        },
        Hotspot {
            button: RemoteButton::Down,
            x: 0.3625,
            y: 0.204211,
            width: 0.275,
            height: 0.073684,
        },
        Hotspot {
            button: RemoteButton::Left,
            x: 0.125,
            y: 0.117895,
            width: 0.21875,
            height: 0.077895,
        },
        Hotspot {
            button: RemoteButton::Right,
            x: 0.65625,
            y: 0.117895,
            width: 0.21875,
            height: 0.077895,
        },
        Hotspot {
            button: RemoteButton::Ok,
            x: 0.365625,
            y: 0.111579,
            width: 0.26875,
            height: 0.090526,
        },
        Hotspot {
            button: RemoteButton::VolumeUp,
            x: 0.89375,
            y: 0.172632,
            width: 0.10625,
            height: 0.104211,
        },
        Hotspot {
            button: RemoteButton::VolumeDown,
            x: 0.89375,
            y: 0.28,
            width: 0.10625,
            height: 0.096842,
        },
        Hotspot {
            button: RemoteButton::Back,
            x: 0.134375,
            y: 0.305789,
            width: 0.284375,
            height: 0.095789,
        },
        Hotspot {
            button: RemoteButton::Home,
            x: 0.134375,
            y: 0.431052,
            width: 0.284375,
            height: 0.095789,
        },
        Hotspot {
            button: RemoteButton::Voice,
            x: 0.58125,
            y: 0.305789,
            width: 0.284375,
            height: 0.095789,
        },
        Hotspot {
            button: RemoteButton::Power,
            x: 0.192188,
            y: 0.690526,
            width: 0.16875,
            height: 0.056842,
        },
        Hotspot {
            button: RemoteButton::Tv,
            x: 0.639062,
            y: 0.690526,
            width: 0.16875,
            height: 0.056842,
        },
    ],
};

const DEFAULT_LAYOUT: &RemoteLayout = &RC003_LAYOUT;

fn layout_for(profile_id: &str) -> &'static RemoteLayout {
    match profile_id {
        "google-chromecast" => &CHROMECAST_LAYOUT,
        _ => DEFAULT_LAYOUT,
    }
}

fn hotspot_css(button: RemoteButton) -> &'static str {
    match button {
        RemoteButton::Power => "hotspot-power",
        RemoteButton::Voice => "hotspot-voice",
        RemoteButton::Tv => "hotspot-tv",
        RemoteButton::Up => "hotspot-up",
        RemoteButton::Down => "hotspot-down",
        RemoteButton::Left => "hotspot-left",
        RemoteButton::Right => "hotspot-right",
        RemoteButton::Ok => "hotspot-ok",
        RemoteButton::Back => "hotspot-back",
        RemoteButton::Home => "hotspot-home",
        RemoteButton::Menu => "hotspot-menu",
        RemoteButton::VolumeUp => "hotspot-volume-up",
        RemoteButton::VolumeDown => "hotspot-volume-down",
    }
}

/// Smallest width the diagram claims; the board around it sets the height, so
/// the picture shrinks to fit a 1040x760 window instead of forcing the board
/// to scroll vertically.
const DIAGRAM_WIDTH: f64 = 130.0;
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
    base: gtk::Stack,
    picture: gtk::Picture,
    buttons: BTreeMap<RemoteButton, gtk::Button>,
    layout: Rc<Cell<&'static RemoteLayout>>,
    profile: Cell<&'static str>,
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

        let picture = gtk::Picture::new();
        picture.set_content_fit(gtk::ContentFit::Contain);
        picture.set_can_shrink(true);
        picture.set_can_target(false);

        let fallback = gtk::Label::new(Some(tr("Remote diagram unavailable")));
        fallback.add_css_class("dim-label");
        fallback.set_can_target(false);

        let base = gtk::Stack::new();
        base.set_can_target(false);
        base.set_size_request(DIAGRAM_WIDTH as i32, DIAGRAM_HEIGHT as i32);
        base.add_named(&picture, Some("picture"));
        base.add_named(&fallback, Some("fallback"));
        base.set_visible_child_name("picture");
        fixed.put(&base, 0.0, 0.0);

        let buttons = RemoteButton::ALL
            .iter()
            .map(|&button| (button, hotspot_button(button)))
            .collect::<BTreeMap<_, _>>();
        for button in buttons.values() {
            fixed.put(button, 0.0, 0.0);
        }

        let layout = Rc::new(Cell::new(DEFAULT_LAYOUT));
        let tick_layout = Rc::clone(&layout);
        let tick_base = base.clone();
        let tick_buttons = buttons.clone();
        let last = Cell::new((0, 0));
        fixed.add_tick_callback(move |fixed, _clock| {
            let width = fixed.width();
            let height = fixed.height();
            if width > 0 && height > 0 && (width, height) != last.get() {
                last.set((width, height));
                tick_base.set_size_request(width, height);
                for hotspot in tick_layout.get().hotspots {
                    let Some(button) = tick_buttons.get(&hotspot.button) else {
                        continue;
                    };
                    let (x, y, w, h) = scaled(*hotspot, f64::from(width), f64::from(height));
                    button.set_size_request(w, h);
                    fixed.move_(button, x, y);
                }
            }
            glib::ControlFlow::Continue
        });

        let diagram = Self {
            root,
            fixed,
            base,
            picture,
            buttons,
            layout,
            profile: Cell::new(DEFAULT_LAYOUT.profile_id),
        };
        diagram.apply(DEFAULT_LAYOUT);
        diagram
    }

    /// Swap the illustration and hotspot table for the active profile.
    pub fn set_profile(&self, profile_id: &str) {
        let layout = layout_for(profile_id);
        if self.profile.get() == layout.profile_id {
            return;
        }
        self.profile.set(layout.profile_id);
        self.apply(layout);
    }

    fn apply(&self, layout: &'static RemoteLayout) {
        self.layout.set(layout);
        match texture(layout.svg) {
            Some(texture) => {
                self.picture.set_paintable(Some(&texture));
                self.base.set_visible_child_name("picture");
            }
            None => {
                self.base.set_visible_child_name("fallback");
            }
        }

        let width = if self.fixed.width() > 0 {
            f64::from(self.fixed.width())
        } else {
            DIAGRAM_WIDTH
        };
        let height = if self.fixed.height() > 0 {
            f64::from(self.fixed.height())
        } else {
            DIAGRAM_HEIGHT
        };
        self.base.set_size_request(width as i32, height as i32);
        for button in RemoteButton::ALL {
            let Some(hotspot) = layout
                .hotspots
                .iter()
                .find(|hotspot| hotspot.button == button)
                .copied()
            else {
                if let Some(widget) = self.buttons.get(&button) {
                    widget.set_visible(false);
                }
                continue;
            };
            let Some(widget) = self.buttons.get(&button) else {
                continue;
            };
            widget.set_visible(true);
            let (x, y, w, h) = scaled(hotspot, width, height);
            widget.set_size_request(w, h);
            self.fixed.move_(widget, x, y);
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
        let handler = Rc::new(handler);
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
            for class in ["selected", "conflict", "pressed", "voice"] {
                widget.remove_css_class(class);
            }
            let editable = *button != RemoteButton::Voice;
            if editable {
                if *button == model.selected {
                    widget.add_css_class("selected");
                }
                if conflicts.contains(button) {
                    widget.add_css_class("conflict");
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

fn hotspot_button(button: RemoteButton) -> gtk::Button {
    let label = button_label(button);
    let tooltip = if button == RemoteButton::Voice {
        tr("Voice key: drives the ATVV microphone and is not remappable").to_string()
    } else {
        tr_args("{label}: click to edit", &[("label", label)])
    };
    let widget = gtk::Button::builder()
        .has_frame(false)
        .focusable(true)
        .tooltip_text(tooltip)
        .build();
    widget.add_css_class("remote-hotspot");
    widget.add_css_class(hotspot_css(button));
    widget.set_widget_name(&format!("hotspot-{}", button.name()));
    widget.update_property(&[gtk::accessible::Property::Label(&tr_args(
        "{label} key{suffix}",
        &[("label", label), ("suffix", "")],
    ))]);
    widget
}

/// Decode an embedded SVG through GDK's texture loader.
fn texture(svg: &'static [u8]) -> Option<gdk::Texture> {
    let bytes = glib::Bytes::from_static(svg);
    gdk::Texture::from_bytes(&bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_cover_expected_buttons_inside_the_view_box() {
        for layout in [DEFAULT_LAYOUT, &CHROMECAST_LAYOUT] {
            let mut seen = std::collections::BTreeSet::new();
            for hotspot in layout.hotspots {
                assert!(
                    seen.insert(hotspot.button),
                    "{} has a duplicate {} hotspot",
                    layout.profile_id,
                    hotspot.button
                );
                assert!((0.0..=1.0).contains(&hotspot.x));
                assert!((0.0..=1.0).contains(&hotspot.y));
                assert!(hotspot.x + hotspot.width <= 1.0 + f64::EPSILON);
                assert!(hotspot.y + hotspot.height <= 1.0 + f64::EPSILON);
            }
            let google = layout.profile_id == "google-chromecast";
            for button in RemoteButton::ALL {
                let expected = !(google && button == RemoteButton::Menu);
                assert_eq!(
                    seen.contains(&button),
                    expected,
                    "{} hotspot state for {button}",
                    layout.profile_id
                );
            }
            assert!(layout.svg.len() > 1_000);
        }
        assert_ne!(RC003_LAYOUT.svg, CHROMECAST_LAYOUT.svg);
    }

    #[test]
    fn profile_selection_uses_the_matching_asset() {
        assert_eq!(
            layout_for("google-chromecast").profile_id,
            CHROMECAST_LAYOUT.profile_id
        );
        assert_eq!(
            layout_for("miremote-rc003").profile_id,
            RC003_LAYOUT.profile_id
        );
        assert_eq!(layout_for("unknown").profile_id, DEFAULT_LAYOUT.profile_id);
    }
}
