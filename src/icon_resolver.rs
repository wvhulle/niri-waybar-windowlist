use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, LazyLock, Mutex},
};

use waybar_cffi::gtk::{
    gio::DesktopAppInfo,
    prelude::{AppInfoExt, Cast, FileExt, IconExt, IconThemeExt},
};

#[derive(Debug, Clone, Default)]
pub struct IconResolver(Arc<Mutex<HashMap<String, PathBuf>>>);

impl IconResolver {
    pub fn new() -> Self {
        Self::default()
    }

    #[tracing::instrument(level = "TRACE", ret)]
    pub fn resolve(&self, app_id: &str) -> Option<PathBuf> {
        let mut cache = self.0.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("icon resolver lock was poisoned, clearing cache");
            let mut guard = poisoned.into_inner();
            guard.clear();
            guard
        });

        if !cache.contains_key(app_id) {
            if let Some(path) = search_for_icon(app_id) {
                cache.insert(app_id.to_string(), path);
            }
        }

        cache.get(app_id).cloned()
    }
}

fn search_for_icon(app_id: &str) -> Option<PathBuf> {
    let from_desktop_files = DATA_DIRECTORIES.iter().find_map(|directory| {
        ["", ".desktop"]
            .iter()
            .map(|suffix| directory.join(format!("applications/{app_id}{suffix}")))
            .chain(
                ["applications/kde/", "applications/org.kde."]
                    .iter()
                    .flat_map(|prefix| {
                        ["", ".desktop"]
                            .iter()
                            .map(move |suffix| directory.join(format!("{prefix}{app_id}{suffix}")))
                    }),
            )
            .find_map(|path| {
                DesktopAppInfo::from_filename(&path).and_then(|info| extract_icon_path(&info))
            })
    });

    from_desktop_files
        .or_else(|| {
            DesktopAppInfo::search(app_id)
                .into_iter()
                .flatten()
                .find_map(|candidate| {
                    DesktopAppInfo::new(&candidate).and_then(|info| extract_icon_path(&info))
                })
        })
        .or_else(|| query_icon_theme(app_id))
}

fn query_icon_theme(icon_name: &str) -> Option<PathBuf> {
    use waybar_cffi::gtk::{IconLookupFlags, IconTheme};

    let icon_theme = IconTheme::default()?;

    let icon_info = icon_theme.lookup_icon(icon_name, 512, IconLookupFlags::empty())?;

    icon_info.filename()
}

fn extract_icon_path(info: &DesktopAppInfo) -> Option<PathBuf> {
    use waybar_cffi::gtk::gio::FileIcon;

    info.icon().and_then(|icon| {
        if let Some(file_icon) = icon.downcast_ref::<FileIcon>() {
            return file_icon.file().path();
        }

        IconExt::to_string(&icon).and_then(|name| query_icon_theme(&name))
    })
}

static DATA_DIRECTORIES: LazyLock<Vec<PathBuf>> = LazyLock::new(|| {
    let home_dirs = std::env::var("HOME").into_iter().flat_map(|home| {
        let home_path = PathBuf::from(home);
        [
            home_path.join(".local/share"),
            home_path.join(".local/share/flatpak/exports/share"),
        ]
    });

    let xdg_dirs: Box<dyn Iterator<Item = PathBuf>> =
        if let Ok(xdg_data) = std::env::var("XDG_DATA_DIRS") {
            Box::new(
                xdg_data
                    .split(':')
                    .map(PathBuf::from)
                    .collect::<Vec<_>>()
                    .into_iter(),
            )
        } else {
            Box::new(
                [
                    PathBuf::from("/usr/local/share"),
                    PathBuf::from("/usr/share"),
                ]
                .into_iter(),
            )
        };

    home_dirs
        .chain(xdg_dirs)
        .chain(std::iter::once(PathBuf::from(
            "/var/lib/flatpak/exports/share",
        )))
        .collect()
});
