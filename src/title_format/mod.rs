pub mod rules;

use std::collections::BTreeMap;

use minijinja::{AutoEscape, Environment};
pub use rules::TitleFormatRule;

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

pub fn render_with_rule(rule: &TitleFormatRule, captures: &BTreeMap<&str, &str>) -> Option<String> {
    TEMPLATE_ENV.with(|env| {
        match env.render_str(
            &rule.format,
            minijinja::context! { ..minijinja::Value::from_serialize(captures) },
        ) {
            Ok(rendered) => Some(rendered),
            Err(e) => {
                tracing::warn!(%e, "template render failed");
                None
            }
        }
    })
}
