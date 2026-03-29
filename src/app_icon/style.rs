use std::{cell::Cell, path::PathBuf, rc::Rc};

use waybar_cffi::gtk::{
    self as gtk,
    gdk_pixbuf::Pixbuf,
    glib::source::idle_add_local_once,
    prelude::{BoxExt, GdkPixbufExt, IconThemeExt, WidgetExt},
    EventBox, IconLookupFlags, IconSize, IconTheme,
};

thread_local! {
    static ICON_THEME_INSTANCE: IconTheme = IconTheme::default().unwrap_or_default();
}

pub struct IconRenderingParams {
    pub display_titles: bool,
    pub icon_size: i32,
    pub icon_path: Option<PathBuf>,
}

pub fn setup_icon_rendering(
    event_box: &gtk::EventBox,
    layout_box: &gtk::Box,
    title_label: &gtk::Label,
    audio_event_box: &EventBox,
    audio_visible: &Rc<Cell<bool>>,
    params: IconRenderingParams,
) {
    let display_titles = params.display_titles;
    let icon_size = params.icon_size;
    let icon_path = params.icon_path;
    let container = layout_box.clone();
    let label = title_label.clone();
    let audio_eb = audio_event_box.clone();
    let audio_vis = audio_visible.clone();

    // Pack label and audio immediately — they don't need the scale factor.
    if display_titles {
        container.pack_start(&label, true, true, 0);
    }
    container.pack_start(&audio_eb, false, false, 0);

    // Load and insert the icon once the widget is realized (so scale_factor is
    // available).
    let icon_inserted = Rc::new(Cell::new(false));
    event_box.connect_size_allocate(move |button, _allocation| {
        if icon_inserted.get() {
            return;
        }
        icon_inserted.set(true);
        tracing::debug!("icon insertion triggered for size_allocate (one-time)");

        let dimension = icon_size;

        let icon_image =
            load_icon_image(icon_path.as_ref(), button, dimension).unwrap_or_else(|| {
                static FALLBACK: &str = "application-x-executable";

                ICON_THEME_INSTANCE
                    .with(|theme| {
                        theme.lookup_icon_for_scale(
                            FALLBACK,
                            dimension,
                            button.scale_factor(),
                            IconLookupFlags::empty(),
                        )
                    })
                    .and_then(|info| load_icon_image(info.filename().as_ref(), button, dimension))
                    .unwrap_or_else(|| gtk::Image::from_icon_name(Some(FALLBACK), IconSize::Button))
            });

        // Insert icon at the front, before the label.
        let container_copy = container.clone();
        let audio_copy = audio_eb.clone();
        let audio_vis_copy = audio_vis.clone();
        let button_copy = button.clone();
        idle_add_local_once(move || {
            container_copy.pack_start(&icon_image, false, false, 0);
            container_copy.reorder_child(&icon_image, 0);

            container_copy.show_all();
            button_copy.show_all();

            if audio_vis_copy.get() {
                audio_copy.show();
            }
        });
    });
}

pub fn load_icon_image(
    path: Option<&PathBuf>,
    widget: &impl WidgetExt,
    size: i32,
) -> Option<gtk::Image> {
    let scaled_size = size * widget.scale_factor();

    path.and_then(
        |p| match Pixbuf::from_file_at_scale(p, scaled_size, scaled_size, true) {
            Ok(pixbuf) => Some(pixbuf),
            Err(e) => {
                tracing::warn!(%e, path = %p.display(), "icon load failed");
                None
            }
        },
    )
    .and_then(|pixbuf| pixbuf.create_surface(0, widget.window().as_ref()))
    .map(|surface| gtk::Image::from_surface(Some(&surface)))
}
