use waybar_cffi::gtk::{self as gtk, prelude::{LabelExt, WidgetExt}};
use crate::settings::{FontStyle, ProcessInfoLayout, ProcessInfoSource};
use crate::button::WindowButton;

fn font_style_to_pango(style: FontStyle) -> (gtk::pango::Weight, gtk::pango::Style) {
    match style {
        FontStyle::Normal => (gtk::pango::Weight::Normal, gtk::pango::Style::Normal),
        FontStyle::Italic => (gtk::pango::Weight::Normal, gtk::pango::Style::Italic),
        FontStyle::Bold => (gtk::pango::Weight::Bold, gtk::pango::Style::Normal),
        FontStyle::BoldItalic => (gtk::pango::Weight::Bold, gtk::pango::Style::Italic),
    }
}

fn apply_styled_range(attrs: &gtk::pango::AttrList, style: FontStyle, start: u32, end: u32) {
    let (weight, pango_style) = font_style_to_pango(style);
    let mut w = gtk::pango::AttrInt::new_weight(weight);
    w.set_start_index(start);
    w.set_end_index(end);
    attrs.insert(w);
    let mut s = gtk::pango::AttrInt::new_style(pango_style);
    s.set_start_index(start);
    s.set_end_index(end);
    attrs.insert(s);
}

fn format_cwd(raw: &str, shorten_home: bool, basename_only: bool) -> String {
    let path = if shorten_home {
        dirs::home_dir()
            .filter(|home| raw.starts_with(&*home.to_string_lossy()))
            .map(|home| format!("~{}", &raw[home.to_string_lossy().len()..]))
            .unwrap_or_else(|| raw.to_string())
    } else {
        raw.to_string()
    };
    if basename_only {
        std::path::Path::new(&path)
            .file_name()
            .map_or(path.clone(), |n| n.to_string_lossy().into_owned())
    } else {
        path
    }
}

fn set_plain_title(label: &gtk::Label, text: &str) {
    label.set_text(text);
    let attrs = gtk::pango::AttrList::new();
    attrs.insert(gtk::pango::AttrInt::new_weight(gtk::pango::Weight::Normal));
    label.set_attributes(Some(&attrs));
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
                    let pattern = self.app_id.as_deref()
                        .and_then(|id| self.state.settings().process_info_pattern(id));
                    if let Some(re) = pattern {
                        if let Some(caps) = re.captures(text) {
                            let cwd = caps.name("cwd").map(|m| m.as_str());
                            let cmd = caps.name("cmd").map(|m| m.as_str());
                            self.update_process_info(cwd, cmd);
                            return;
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
                    text.replace('\n', " ").replace('\r', " ")
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

        let config = self.state.settings().process_info();

        let formatted_cwd = cwd.map(|c| format_cwd(c, config.shorten_home, config.show_basename_only));

        if formatted_cwd.is_none() && command.is_none() {
            let title = self.title.borrow();
            set_plain_title(&self.title_label, title.as_deref().unwrap_or(""));
            return;
        }

        let cwd_part = formatted_cwd.as_deref().unwrap_or("");
        let cmd_part = command.unwrap_or("");

        let separator = match config.layout {
            ProcessInfoLayout::SingleLine => &config.separator,
            ProcessInfoLayout::TwoLines => "\n",
        };

        let text = if !cwd_part.is_empty() && !cmd_part.is_empty() {
            format!("{cwd_part}{separator}{cmd_part}")
        } else if !cwd_part.is_empty() {
            cwd_part.to_string()
        } else {
            cmd_part.to_string()
        };

        self.title_label.set_text(&text);

        let attrs = gtk::pango::AttrList::new();
        let cwd_len = cwd_part.len() as u32;
        let sep_len = if !cwd_part.is_empty() && !cmd_part.is_empty() {
            separator.len() as u32
        } else {
            0
        };
        let cmd_start = cwd_len + sep_len;
        let cmd_end = text.len() as u32;

        if cwd_len > 0 {
            apply_styled_range(&attrs, config.cwd_font_style, 0, cwd_len);
        }
        if cmd_start < cmd_end {
            apply_styled_range(&attrs, config.cmd_font_style, cmd_start, cmd_end);
        }

        self.title_label.set_attributes(Some(&attrs));
        self.title_label.show();
    }
}
