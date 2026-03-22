use std::collections::BTreeMap;

use minijinja::{AutoEscape, Environment};
use waybar_cffi::gtk::prelude::{LabelExt, WidgetExt};

use crate::{
    button::WindowButton,
    settings::{ProcessInfoSource, TitleFormatRule},
};

fn create_template_env() -> Environment<'static> {
    let mut env = Environment::new();
    env.set_auto_escape_callback(|_| AutoEscape::Html);
    env.add_filter("basename", |path: String| -> String {
        std::path::Path::new(&path)
            .file_name()
            .map_or(path.clone(), |n| n.to_string_lossy().into_owned())
    });
    env.add_filter("shorten_home", |path: String| -> String {
        dirs::home_dir()
            .filter(|home| path.starts_with(&*home.to_string_lossy()))
            .map(|home| format!("~{}", &path[home.to_string_lossy().len()..]))
            .unwrap_or(path)
    });
    env
}

thread_local! {
    static TEMPLATE_ENV: Environment<'static> = create_template_env();
}

fn render_with_rule(
    rule: &TitleFormatRule,
    captures: &BTreeMap<&str, &str>,
) -> Option<String> {
    TEMPLATE_ENV.with(|env| {
        match env.render_str(&rule.format, minijinja::context! { ..minijinja::Value::from_serialize(captures) }) {
            Ok(rendered) => Some(rendered),
            Err(e) => {
                tracing::warn!(%e, "template render failed");
                None
            }
        }
    })
}

impl WindowButton {
    #[tracing::instrument(level = "TRACE")]
    pub fn update_title(&self, title: Option<&str>) {
        if let Some(t) = title {
            *self.title.borrow_mut() = Some(t.to_string());
        }

        if self.process_info_enabled {
            let config = self.state.settings().process_info();
            if config.source == ProcessInfoSource::TitleRegex {
                if let Some(text) = title {
                    let rule = self
                        .app_id
                        .as_deref()
                        .and_then(|id| self.state.settings().process_info_rule(id));
                    if let Some(rule) = rule {
                        if let Some(caps) = rule.pattern.captures(text) {
                            let capture_names: BTreeMap<&str, &str> = rule
                                .pattern
                                .capture_names()
                                .flatten()
                                .filter_map(|name| {
                                    caps.name(name).map(|m| (name, m.as_str()))
                                })
                                .collect();

                            if let Some(markup) = render_with_rule(rule, &capture_names) {
                                self.title_label.set_markup(&markup);
                                self.title_label.show();
                                return;
                            }
                        }
                    }
                }
            } else {
                return;
            }
        }

        if self.display_titles {
            if let Some(text) = title {
                let display_text = if self.state.settings().allow_title_linebreaks() {
                    text.to_string()
                } else {
                    text.replace(['\n', '\r'], " ")
                };
                self.title_label.set_text(&display_text);
                self.title_label.show();
            } else {
                self.title_label.set_text("");
                self.title_label.hide();
            }
        }
    }

    pub fn update_process_info(&self, cwd: Option<&str>, command: Option<&str>) {
        if !self.display_titles {
            return;
        }

        let rule = self
            .app_id
            .as_deref()
            .and_then(|id| self.state.settings().process_info_rule(id));

        if let Some(rule) = rule {
            let mut captures = BTreeMap::new();
            if let Some(c) = cwd {
                captures.insert("cwd", c);
            }
            if let Some(c) = command {
                captures.insert("cmd", c);
            }

            if captures.is_empty() {
                let title = self.title.borrow();
                self.title_label.set_text(title.as_deref().unwrap_or(""));
                self.title_label.show();
                return;
            }

            if let Some(markup) = render_with_rule(rule, &captures) {
                self.title_label.set_markup(&markup);
                self.title_label.show();
                return;
            }
        }

        // Fallback: show raw title
        let title = self.title.borrow();
        self.title_label.set_text(title.as_deref().unwrap_or(""));
        self.title_label.show();
    }
}
