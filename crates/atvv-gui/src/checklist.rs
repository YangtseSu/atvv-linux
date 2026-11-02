// SPDX-License-Identifier: GPL-3.0-or-later

//! First-run checklist and recovery guidance.
//!
//! The state comes from `GetConfig` and `GetStatus` data only. The dialog
//! never starts a service, opens Bluetooth, or writes the configuration; a
//! command can be copied for the user to run. The consumer row is
//! informational because the provider cannot know whether a consumer is
//! configured.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use atvv_config::ConfigDocument;
use gtk::prelude::*;
use libadwaita as adw;

use adw::prelude::*;

use crate::client::DaemonStatus;
use crate::i18n::{tr, tr_args};

/// Consumer setup guide linked from the checklist.
pub const INTEGRATIONS_URL: &str =
    "https://github.com/YangtseSu/atvv-linux/tree/main/docs/integrations";

/// One layer between the remote and a consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Daemon,
    Device,
    Source,
    Input,
}

impl Layer {
    pub const ALL: [Self; 4] = [Self::Daemon, Self::Device, Self::Source, Self::Input];

    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Self::Daemon => 0,
            Self::Device => 1,
            Self::Source => 2,
            Self::Input => 3,
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Daemon => tr("Daemon"),
            Self::Device => tr("Device"),
            Self::Source => tr("Source"),
            Self::Input => tr("Input and trigger"),
        }
    }
}

/// Result of one layer check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckState {
    /// The layer answered and is usable.
    Ready,
    /// The layer is configured but not usable.
    Missing,
    /// A lower layer did not answer, so this check cannot be decided.
    Unknown,
}

/// Computed checklist and the data its commands need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checklist {
    states: [CheckState; 4],
    active_device: Option<String>,
    document_loaded: bool,
    source: String,
    recipe: Option<String>,
}

impl Checklist {
    #[must_use]
    pub fn state(&self, layer: Layer) -> CheckState {
        self.states[layer.index()]
    }

    /// Configured remote address, if the document names one.
    #[must_use]
    pub fn active_device(&self) -> Option<&str> {
        self.active_device.as_deref()
    }

    /// Source name to look up, falling back to `atvv-<profile>`.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Daemon-supplied consumer instruction; `None` without a status.
    #[must_use]
    pub fn recipe(&self) -> Option<&str> {
        self.recipe.as_deref()
    }

    /// Commands for a layer, in check order. The dialog shows them when the
    /// layer is not ready.
    #[must_use]
    pub fn commands(&self, layer: Layer) -> Vec<String> {
        match layer {
            Layer::Daemon => vec![
                "systemctl --user status atvvd".to_string(),
                "atvvctl doctor".to_string(),
            ],
            Layer::Device => vec!["atvvctl pair <MAC>".to_string()],
            Layer::Source => vec![
                "atvvctl status".to_string(),
                format!("pw-cli info {}", self.source),
            ],
            Layer::Input => vec![
                "atvvctl status".to_string(),
                "ls -l /dev/uinput".to_string(),
            ],
        }
    }
}

/// Evaluate every layer from the configuration document and the daemon status.
///
/// A missing document or status leaves the dependent checks `Unknown` instead
/// of reporting a failure the GUI cannot observe.
#[must_use]
pub fn evaluate(document: Option<&ConfigDocument>, status: Option<&DaemonStatus>) -> Checklist {
    let active_device = document
        .and_then(|document| document.active_device.clone())
        .filter(|address| !address.is_empty());
    let profile = status
        .map(|status| status.contract.profile.trim())
        .filter(|profile| !profile.is_empty())
        .map(str::to_string)
        .or_else(|| {
            document
                .map(|document| document.profile.clone())
                .filter(|profile| !profile.is_empty())
        })
        .unwrap_or_default();
    let source = status
        .map(|status| status.contract.source.trim())
        .filter(|source| !source.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            if profile.is_empty() {
                "atvv-<profile>".to_string()
            } else {
                format!("atvv-{profile}")
            }
        });
    let recipe = status
        .map(|status| status.contract.recipe.trim())
        .filter(|recipe| !recipe.is_empty())
        .map(str::to_string);

    let daemon = if status.is_some() {
        CheckState::Ready
    } else {
        CheckState::Missing
    };
    let device = match document {
        None => CheckState::Unknown,
        Some(_) => match status {
            None => CheckState::Unknown,
            Some(status) => match active_device.as_deref() {
                Some(address) if status.device.address == address && status.connected => {
                    CheckState::Ready
                }
                _ => CheckState::Missing,
            },
        },
    };
    let source_state = match status {
        None => CheckState::Unknown,
        Some(status) => {
            if status.audio.available && !status.contract.source.is_empty() {
                CheckState::Ready
            } else {
                CheckState::Missing
            }
        }
    };
    let input = match status {
        None => CheckState::Unknown,
        Some(status) => {
            if status.input.enabled && status.input.keyboard && status.contract.active_key.is_some()
            {
                CheckState::Ready
            } else {
                CheckState::Missing
            }
        }
    };

    Checklist {
        states: [daemon, device, source_state, input],
        active_device,
        document_loaded: document.is_some(),
        source,
        recipe,
    }
}

struct CommandRow {
    row: gtk::Box,
    label: gtk::Label,
    copy: gtk::Button,
}

struct CheckRow {
    dot: gtk::Image,
    body: gtk::Label,
    commands: Vec<CommandRow>,
    state: Cell<CheckState>,
}

/// Modal **Getting started** dialog.
pub struct ChecklistDialog {
    dialog: adw::Dialog,
    rows: Vec<CheckRow>,
    consumer_body: gtk::Label,
    consumer_link: gtk::LinkButton,
    open: Rc<Cell<bool>>,
    copied: Rc<RefCell<Option<String>>>,
}

impl ChecklistDialog {
    #[must_use]
    pub fn new() -> Self {
        let copied = Rc::new(RefCell::new(None));
        let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
        content.set_margin_top(16);
        content.set_margin_bottom(16);
        content.set_margin_start(16);
        content.set_margin_end(16);

        let intro = gtk::Label::new(Some(tr(
            "Check each layer from the daemon down to the trigger. Copy a command when a check is missing.",
        )));
        intro.add_css_class("check-body");
        intro.set_wrap(true);
        intro.set_xalign(0.0);
        content.append(&intro);

        let mut rows = Vec::new();
        for layer in Layer::ALL {
            let (row, widgets) = check_row(layer, Rc::clone(&copied));
            content.append(&row);
            rows.push(widgets);
        }

        let consumer = gtk::Box::new(gtk::Orientation::Vertical, 4);
        consumer.add_css_class("check-row");
        let title = gtk::Label::new(Some(tr("Consumer")));
        title.add_css_class("check-title");
        title.set_halign(gtk::Align::Start);
        let consumer_body = gtk::Label::new(None);
        consumer_body.add_css_class("check-body");
        consumer_body.set_wrap(true);
        consumer_body.set_xalign(0.0);
        let consumer_link = gtk::LinkButton::with_label(INTEGRATIONS_URL, tr("Integration guides"));
        consumer_link.add_css_class("check-link");
        consumer_link.set_halign(gtk::Align::Start);
        consumer.append(&title);
        consumer.append(&consumer_body);
        consumer.append(&consumer_link);
        content.append(&consumer);

        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        footer.set_margin_top(10);
        footer.set_margin_bottom(10);
        footer.set_margin_start(16);
        footer.set_margin_end(16);
        let close = gtk::Button::with_label(tr("Close"));
        close.add_css_class("flat");
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        footer.append(&spacer);
        footer.append(&close);

        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&adw::WindowTitle::new(
            tr("Getting started"),
            tr("Daemon, remote, source, and trigger"),
        )));
        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        let scroller = gtk::ScrolledWindow::new();
        scroller.set_child(Some(&content));
        scroller.set_vexpand(true);
        toolbar.set_content(Some(&scroller));
        toolbar.add_bottom_bar(&footer);

        let dialog = adw::Dialog::builder()
            .title(tr("Getting started"))
            .content_width(560)
            .content_height(620)
            .child(&toolbar)
            .build();

        let checklist = Self {
            dialog,
            rows,
            consumer_body,
            consumer_link,
            open: Rc::new(Cell::new(false)),
            copied,
        };
        let open = Rc::clone(&checklist.open);
        checklist.dialog.connect_closed(move |_| open.set(false));
        let close_dialog = checklist.dialog.clone();
        close.connect_clicked(move |_| {
            close_dialog.close();
        });
        checklist
    }

    #[must_use]
    pub fn dialog(&self) -> &adw::Dialog {
        &self.dialog
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open.get()
    }

    pub fn present(&self, parent: &impl IsA<gtk::Widget>) {
        self.open.set(true);
        self.dialog.present(Some(parent));
    }

    pub fn close(&self) {
        self.dialog.close();
    }

    /// Render one evaluation. Cheap enough for a 1 s refresh.
    pub fn refresh(&self, checklist: &Checklist) {
        for layer in Layer::ALL {
            let row = &self.rows[layer.index()];
            let state = checklist.state(layer);
            row.state.set(state);
            row.dot.set_icon_name(Some(match state {
                CheckState::Ready => "object-select-symbolic",
                CheckState::Missing => "dialog-warning-symbolic",
                CheckState::Unknown => "content-loading-symbolic",
            }));
            for class in ["success", "warning", "dim-label"] {
                row.dot.remove_css_class(class);
            }
            row.dot.add_css_class(match state {
                CheckState::Ready => "success",
                CheckState::Missing => "warning",
                CheckState::Unknown => "dim-label",
            });
            row.body.set_text(&explanation(layer, state, checklist));
            let commands = checklist.commands(layer);
            for (index, command) in row.commands.iter().enumerate() {
                match commands.get(index) {
                    Some(text) => {
                        command.label.set_text(text);
                        command.row.set_visible(state != CheckState::Ready);
                    }
                    None => command.row.set_visible(false),
                }
            }
        }
        self.consumer_body.set_text(checklist.recipe().unwrap_or(tr(
            "Select the ATVV source in your consumer and bind a declared recording key.",
        )));
    }

    #[must_use]
    pub fn state(&self, layer: Layer) -> CheckState {
        self.rows[layer.index()].state.get()
    }

    #[must_use]
    pub fn body_text(&self, layer: Layer) -> String {
        self.rows[layer.index()].body.text().to_string()
    }

    #[must_use]
    pub fn commands(&self, layer: Layer) -> Vec<String> {
        self.rows[layer.index()]
            .commands
            .iter()
            .filter(|command| command.row.is_visible())
            .map(|command| command.label.text().to_string())
            .collect()
    }

    #[must_use]
    pub fn consumer_text(&self) -> String {
        self.consumer_body.text().to_string()
    }

    #[must_use]
    pub fn link_url(&self) -> String {
        self.consumer_link.uri().to_string()
    }

    /// Click a command's copy button, as the user would.
    #[must_use]
    pub fn copy(&self, layer: Layer, index: usize) -> bool {
        let Some(command) = self.rows[layer.index()].commands.get(index) else {
            return false;
        };
        if !command.row.is_visible() {
            return false;
        }
        command.copy.emit_clicked();
        true
    }

    /// Last text placed on the clipboard, for the UI test.
    #[must_use]
    pub fn last_copied(&self) -> Option<String> {
        self.copied.borrow().clone()
    }
}

impl Default for ChecklistDialog {
    fn default() -> Self {
        Self::new()
    }
}

fn check_row(layer: Layer, copied: Rc<RefCell<Option<String>>>) -> (gtk::Box, CheckRow) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("check-row");
    let dot = gtk::Image::from_icon_name("content-loading-symbolic");
    dot.add_css_class("check-dot");
    dot.add_css_class("dim-label");
    dot.set_valign(gtk::Align::Start);
    row.append(&dot);

    let text = gtk::Box::new(gtk::Orientation::Vertical, 4);
    text.set_hexpand(true);
    let title = gtk::Label::new(Some(layer.title()));
    title.add_css_class("check-title");
    title.set_halign(gtk::Align::Start);
    let body = gtk::Label::new(None);
    body.add_css_class("check-body");
    body.set_wrap(true);
    body.set_xalign(0.0);
    text.append(&title);
    text.append(&body);

    let mut commands = Vec::new();
    for _ in 0..2 {
        let command_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        command_row.set_visible(false);
        let label = gtk::Label::new(None);
        label.add_css_class("check-command");
        label.set_xalign(0.0);
        label.set_selectable(true);
        label.set_hexpand(true);
        let copy = gtk::Button::from_icon_name("edit-copy-symbolic");
        copy.add_css_class("flat");
        copy.set_tooltip_text(Some(tr("Copy command")));
        copy.set_valign(gtk::Align::Center);
        let copied = Rc::clone(&copied);
        let label_for_copy = label.clone();
        copy.connect_clicked(move |_| {
            let text = label_for_copy.text().to_string();
            if let Some(display) = gtk::gdk::Display::default() {
                display.clipboard().set_text(&text);
            }
            *copied.borrow_mut() = Some(text);
        });
        command_row.append(&label);
        command_row.append(&copy);
        text.append(&command_row);
        commands.push(CommandRow {
            row: command_row,
            label,
            copy,
        });
    }
    row.append(&text);

    (
        row,
        CheckRow {
            dot,
            body,
            commands,
            state: Cell::new(CheckState::Unknown),
        },
    )
}

fn explanation(layer: Layer, state: CheckState, checklist: &Checklist) -> String {
    match (layer, state) {
        (Layer::Daemon, CheckState::Ready) => tr("atvvd answered on the session bus.").to_string(),
        (Layer::Daemon, _) => {
            tr("atvvd is not running or the session bus is not reachable.").to_string()
        }
        (Layer::Device, CheckState::Ready) => tr("The configured remote is connected.").to_string(),
        (Layer::Device, CheckState::Missing) if checklist.active_device().is_some() => {
            tr("The configured remote is not connected.").to_string()
        }
        (Layer::Device, CheckState::Missing) => {
            tr("No remote is configured yet. Pair one with atvvctl.").to_string()
        }
        (Layer::Device, CheckState::Unknown) if !checklist.document_loaded => {
            tr("The configuration is not loaded yet.").to_string()
        }
        (Layer::Device, CheckState::Unknown) => tr("Waiting for atvvd.").to_string(),
        (Layer::Source, CheckState::Ready) => tr_args(
            "The source {source} is available.",
            &[("source", checklist.source())],
        ),
        (Layer::Source, CheckState::Missing) => tr_args(
            "The source {source} is missing from PipeWire.",
            &[("source", checklist.source())],
        ),
        (Layer::Source, CheckState::Unknown) => tr("Waiting for atvvd.").to_string(),
        (Layer::Input, CheckState::Ready) => {
            tr("The grab is active and the trigger key can be emitted.").to_string()
        }
        (Layer::Input, CheckState::Missing) => {
            tr("The input runtime is not ready or the trigger key is not emitted.").to_string()
        }
        (Layer::Input, CheckState::Unknown) => tr("Waiting for atvvd.").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use atvv_config::{Config, ConfigDocument};
    use atvv_protocol::RC003;

    use super::*;
    use crate::client::{AudioInfo, ContractInfo, DeviceInfo, InputInfo};

    fn document(active: Option<&str>) -> ConfigDocument {
        let mut document = ConfigDocument::from_config(&Config::defaults(&RC003));
        document.active_device = active.map(str::to_string);
        document
    }

    fn status(address: &str, connected: bool) -> DaemonStatus {
        DaemonStatus {
            connected,
            device: DeviceInfo {
                name: "RC003".to_string(),
                address: address.to_string(),
            },
            audio: AudioInfo {
                available: true,
                ..AudioInfo::default()
            },
            input: InputInfo {
                enabled: true,
                keyboard: true,
            },
            contract: ContractInfo {
                profile: "miremote-rc003".to_string(),
                source: "atvv-miremote-rc003".to_string(),
                active_key: Some("DICTATE".to_string()),
                recipe: "Select the source.".to_string(),
                ..ContractInfo::default()
            },
            ..DaemonStatus::default()
        }
    }

    const ADDRESS: &str = "C0:5D:39:F8:2D:4C";

    #[test]
    fn no_status_and_no_document_reports_only_daemon_missing() {
        let checklist = evaluate(None, None);
        assert_eq!(checklist.state(Layer::Daemon), CheckState::Missing);
        assert_eq!(checklist.state(Layer::Device), CheckState::Unknown);
        assert_eq!(checklist.state(Layer::Source), CheckState::Unknown);
        assert_eq!(checklist.state(Layer::Input), CheckState::Unknown);
        assert_eq!(checklist.source(), "atvv-<profile>");
        assert!(checklist.recipe().is_none());
    }

    #[test]
    fn configured_connected_remote_is_ready_at_every_layer() {
        let checklist = evaluate(Some(&document(Some(ADDRESS))), Some(&status(ADDRESS, true)));
        for layer in Layer::ALL {
            assert_eq!(checklist.state(layer), CheckState::Ready, "{layer:?}");
        }
        assert_eq!(checklist.recipe(), Some("Select the source."));
        assert_eq!(checklist.source(), "atvv-miremote-rc003");
    }

    #[test]
    fn device_requires_a_configured_matching_and_connected_address() {
        let live = status(ADDRESS, true);
        assert_eq!(
            evaluate(Some(&document(None)), Some(&live)).state(Layer::Device),
            CheckState::Missing
        );
        assert_eq!(
            evaluate(Some(&document(Some(ADDRESS))), Some(&live)).state(Layer::Device),
            CheckState::Ready
        );
        assert_eq!(
            evaluate(Some(&document(Some("00:00:00:00:00:00"))), Some(&live)).state(Layer::Device),
            CheckState::Missing
        );
        assert_eq!(
            evaluate(
                Some(&document(Some(ADDRESS))),
                Some(&status(ADDRESS, false))
            )
            .state(Layer::Device),
            CheckState::Missing
        );
        assert_eq!(
            evaluate(Some(&document(Some(ADDRESS))), None).state(Layer::Device),
            CheckState::Unknown
        );
    }

    #[test]
    fn source_requires_an_available_nonempty_source() {
        let mut status = status(ADDRESS, true);
        status.audio.available = false;
        assert_eq!(
            evaluate(Some(&document(Some(ADDRESS))), Some(&status)).state(Layer::Source),
            CheckState::Missing
        );
        status.audio.available = true;
        status.contract.source.clear();
        let checklist = evaluate(Some(&document(Some(ADDRESS))), Some(&status));
        assert_eq!(checklist.state(Layer::Source), CheckState::Missing);
        assert_eq!(checklist.source(), "atvv-miremote-rc003");
    }

    #[test]
    fn input_requires_a_live_runtime_and_an_active_key() {
        let mut status = status(ADDRESS, true);
        status.input.enabled = false;
        assert_eq!(
            evaluate(Some(&document(Some(ADDRESS))), Some(&status)).state(Layer::Input),
            CheckState::Missing
        );
        status.input.enabled = true;
        status.input.keyboard = false;
        assert_eq!(
            evaluate(Some(&document(Some(ADDRESS))), Some(&status)).state(Layer::Input),
            CheckState::Missing
        );
        status.input.keyboard = true;
        status.contract.active_key = None;
        assert_eq!(
            evaluate(Some(&document(Some(ADDRESS))), Some(&status)).state(Layer::Input),
            CheckState::Missing
        );
        status.contract.active_key = Some("DICTATE".to_string());
        assert_eq!(
            evaluate(Some(&document(Some(ADDRESS))), Some(&status)).state(Layer::Input),
            CheckState::Ready
        );
    }

    #[test]
    fn commands_are_the_documented_recovery_commands() {
        let checklist = evaluate(Some(&document(Some(ADDRESS))), Some(&status(ADDRESS, true)));
        assert_eq!(
            checklist.commands(Layer::Daemon),
            ["systemctl --user status atvvd", "atvvctl doctor"]
        );
        assert_eq!(checklist.commands(Layer::Device), ["atvvctl pair <MAC>"]);
        assert_eq!(
            checklist.commands(Layer::Source),
            ["atvvctl status", "pw-cli info atvv-miremote-rc003"]
        );
        assert_eq!(
            checklist.commands(Layer::Input),
            ["atvvctl status", "ls -l /dev/uinput"]
        );
    }
}
