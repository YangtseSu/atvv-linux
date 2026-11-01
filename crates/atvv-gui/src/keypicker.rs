// SPDX-License-Identifier: GPL-3.0-or-later

//! Searchable key-name picker for the action editor.
//!
//! The list comes from [`atvv_input::KEY_NAMES`], so every row is a name the
//! action grammar resolves. Free-form entry still accepts kernel names that
//! are not in the curated list.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use libadwaita as adw;

use adw::prelude::*;

use crate::i18n::tr;
use crate::model::{friendly_key, friendly_key_english};

/// One picker row: canonical name, localized label, and English label.
struct PickerRow {
    name: &'static str,
    label: String,
    english: String,
}

pub struct KeyPicker {
    dialog: adw::Dialog,
    search: gtk::SearchEntry,
    list: gtk::ListBox,
    rows: Rc<Vec<PickerRow>>,
    activate: Rc<dyn Fn(usize)>,
    open: Rc<Cell<bool>>,
}

impl KeyPicker {
    /// Build the picker. `choose` receives the canonical key name.
    #[must_use]
    pub fn new(choose: impl Fn(&'static str) + 'static) -> Self {
        let search = gtk::SearchEntry::builder()
            .placeholder_text(tr("Search keys"))
            .hexpand(true)
            .build();
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .build();
        list.add_css_class("boxed-list");

        let mut rows = Vec::with_capacity(atvv_input::KEY_NAMES.len());
        for name in atvv_input::KEY_NAMES {
            let name: &'static str = name;
            let label = friendly_key(name);
            let english = friendly_key_english(name);
            let row = gtk::ListBoxRow::new();
            row.set_widget_name(name);
            row.set_activatable(true);
            row.set_tooltip_text(Some(name));
            let copy = gtk::Box::new(gtk::Orientation::Vertical, 0);
            copy.set_margin_top(6);
            copy.set_margin_bottom(6);
            copy.set_margin_start(6);
            copy.set_margin_end(6);
            let title = gtk::Label::new(Some(&label));
            title.set_xalign(0.0);
            let subtitle = gtk::Label::new(Some(name));
            subtitle.add_css_class("caption");
            subtitle.add_css_class("dim-label");
            subtitle.set_xalign(0.0);
            copy.append(&title);
            copy.append(&subtitle);
            row.set_child(Some(&copy));
            let accessible = format!("{label} ({name})");
            row.update_property(&[gtk::accessible::Property::Label(&accessible)]);
            list.append(&row);
            rows.push(PickerRow {
                name,
                label,
                english,
            });
        }
        let rows = Rc::new(rows);

        let search_for_filter = search.clone();
        let rows_for_filter = Rc::clone(&rows);
        list.set_filter_func(move |row| {
            let query = search_for_filter.text().trim().to_lowercase();
            if query.is_empty() {
                return true;
            }
            let Some(row) = rows_for_filter.get(row.index() as usize) else {
                return false;
            };
            // Match the canonical name, the localized label, and the English
            // label, so an English query works in every locale.
            row.name.to_lowercase().contains(&query)
                || row.label.to_lowercase().contains(&query)
                || row.english.to_lowercase().contains(&query)
        });
        // `search-changed` is delayed by GtkSearchEntry; filter on every
        // keystroke instead so the list and the tests stay immediate.
        let list_for_search = list.clone();
        search.connect_changed(move |_| list_for_search.invalidate_filter());

        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        content.set_margin_top(8);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.append(&search);
        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .child(&list)
            .build();
        content.append(&scroller);

        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&adw::WindowTitle::new(tr("Choose key"), "")));
        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&content));
        let dialog = adw::Dialog::builder()
            .title(tr("Choose key"))
            .content_width(420)
            .content_height(480)
            .child(&toolbar)
            .build();
        dialog.set_default_widget(Some(&search));
        dialog.set_focus(Some(&search));

        let choose = Rc::new(choose);
        let activate: Rc<dyn Fn(usize)> = {
            let rows = Rc::clone(&rows);
            let choose = Rc::clone(&choose);
            let weak = dialog.downgrade();
            Rc::new(move |index| {
                let Some(row) = rows.get(index) else {
                    return;
                };
                choose(row.name);
                if let Some(dialog) = weak.upgrade() {
                    dialog.close();
                }
            })
        };

        let activate_for_row = Rc::clone(&activate);
        list.connect_row_activated(move |_, row| activate_for_row(row.index() as usize));

        // Enter in the search field chooses the first row that passes the
        // filter; otherwise the search entry swallows the key.
        let activate_for_search = Rc::clone(&activate);
        let list_for_search = list.clone();
        let rows_for_search = Rc::clone(&rows);
        search.connect_activate(move |_| {
            let first = (0..rows_for_search.len()).find(|index| {
                list_for_search
                    .row_at_index(*index as i32)
                    .is_some_and(|row| row.is_child_visible())
            });
            if let Some(index) = first {
                activate_for_search(index);
            }
        });

        let open = Rc::new(Cell::new(false));
        let open_for_close = Rc::clone(&open);
        dialog.connect_closed(move |_| open_for_close.set(false));

        Self {
            dialog,
            search,
            list,
            rows,
            activate,
            open,
        }
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
        self.search.set_text("");
        self.list.invalidate_filter();
        self.open.set(true);
        self.dialog.present(Some(parent));
        self.search.grab_focus();
    }

    pub fn close(&self) {
        self.open.set(false);
        self.dialog.close();
    }

    /// The search field, exposed for tests.
    #[must_use]
    pub fn search(&self) -> &gtk::SearchEntry {
        &self.search
    }

    /// The list, exposed for tests.
    #[must_use]
    pub fn list(&self) -> &gtk::ListBox {
        &self.list
    }

    /// Canonical names whose rows pass the current filter.
    #[must_use]
    pub fn visible_names(&self) -> Vec<&'static str> {
        self.rows
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                self.list
                    .row_at_index(*index as i32)
                    .is_some_and(|row| row.is_child_visible())
            })
            .map(|(_, row)| row.name)
            .collect()
    }

    /// Choose a key by canonical name, as a row click would.
    pub fn select(&self, name: &str) -> bool {
        let Some(index) = self.rows.iter().position(|row| row.name == name) else {
            return false;
        };
        (self.activate)(index);
        true
    }
}
