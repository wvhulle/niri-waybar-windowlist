use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use waybar_cffi::gtk::{self as gtk, prelude::{LabelExt, WidgetExt}};

use super::parse::render_with_rule;
use crate::settings::Settings;

pub fn update_title(
    title_label: &gtk::Label,
    title_store: &Rc<RefCell<Option<String>>>,
    window_id: u64,
    app_id: Option<&str>,
    settings: &Settings,
    display_titles: bool,
    title: Option<&str>,
) {
    if let Some(t) = title {
        *title_store.borrow_mut() = Some(t.to_string());
    }

    if let Some(text) = title {
        let rule = app_id.and_then(|id| settings.title_format_rule(id));

        if let Some(rule) = rule {
            if let Some(caps) = rule.pattern.captures(text) {
                let capture_names: BTreeMap<&str, &str> = rule
                    .pattern
                    .capture_names()
                    .flatten()
                    .filter_map(|name| caps.name(name).map(|m| (name, m.as_str())))
                    .collect();

                if let Some(markup) = render_with_rule(rule, &capture_names) {
                    tracing::info!(
                        window_id,
                        markup = %markup,
                        has_parent = title_label.parent().is_some(),
                        "set_markup on title label"
                    );
                    title_label.set_markup(&markup);
                    title_label.show();
                    return;
                }
            }
        }
    }

    if display_titles {
        if let Some(text) = title {
            let display_text = if settings.allow_title_linebreaks() {
                text.to_string()
            } else {
                text.replace(['\n', '\r'], " ")
            };
            tracing::info!(
                window_id,
                display_text = %display_text,
                has_parent = title_label.parent().is_some(),
                "set_text on title label"
            );
            title_label.set_text(&display_text);
            title_label.show();
        } else {
            tracing::info!(window_id, "clearing title label");
            title_label.set_text("");
            title_label.hide();
        }
    }
}

pub fn update_process_info(
    title_label: &gtk::Label,
    title_store: &Rc<RefCell<Option<String>>>,
    app_id: Option<&str>,
    settings: &Settings,
    display_titles: bool,
    cwd: Option<&str>,
    command: Option<&str>,
) {
    if !display_titles {
        return;
    }

    let rule = app_id.and_then(|id| settings.title_format_rule(id));

    if let Some(rule) = rule {
        let mut captures = BTreeMap::new();
        if let Some(c) = cwd {
            captures.insert("cwd", c);
        }
        if let Some(c) = command {
            captures.insert("cmd", c);
        }

        if captures.is_empty() {
            let title = title_store.borrow();
            title_label.set_text(title.as_deref().unwrap_or(""));
            title_label.show();
            return;
        }

        if let Some(markup) = render_with_rule(rule, &captures) {
            title_label.set_markup(&markup);
            title_label.show();
            return;
        }
    }

    let title = title_store.borrow();
    title_label.set_text(title.as_deref().unwrap_or(""));
    title_label.show();
}
