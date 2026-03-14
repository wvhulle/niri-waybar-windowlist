use std::sync::{Arc, Mutex};

use wayland_client::{
    delegate_noop, event_created_child,
    protocol::{wl_registry, wl_seat},
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1::{self, ZwlrForeignToplevelHandleV1},
    zwlr_foreign_toplevel_manager_v1::{self, ZwlrForeignToplevelManagerV1},
};

struct ToplevelInfo {
    handle: ZwlrForeignToplevelHandleV1,
    app_id: Option<String>,
    title: Option<String>,
}

struct SharedData {
    seat: Option<wl_seat::WlSeat>,
    toplevels: Vec<ToplevelInfo>,
}

/// Thread-safe handle for activating windows via the Wayland
/// `wlr-foreign-toplevel-management` protocol. Sends `activate` requests
/// directly over the Wayland connection instead of niri IPC.
#[derive(Clone)]
pub struct WaylandActivator {
    shared: Arc<Mutex<SharedData>>,
    conn: Arc<Connection>,
}

impl std::fmt::Debug for WaylandActivator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WaylandActivator").finish_non_exhaustive()
    }
}

impl WaylandActivator {
    pub fn connect() -> Option<Self> {
        let conn = match Connection::connect_to_env() {
            Ok(c) => Arc::new(c),
            Err(e) => {
                tracing::warn!(%e, "failed to connect to Wayland display");
                return None;
            }
        };

        let shared = Arc::new(Mutex::new(SharedData {
            seat: None,
            toplevels: Vec::new(),
        }));

        let mut state = DispatchState {
            shared: shared.clone(),
            pending: Vec::new(),
        };

        let mut event_queue = conn.new_event_queue::<DispatchState>();
        let qh = event_queue.handle();

        conn.display().get_registry(&qh, ());

        if let Err(e) = event_queue.roundtrip(&mut state) {
            tracing::warn!(%e, "Wayland roundtrip failed");
            return None;
        }

        {
            let data = shared.lock().ok()?;
            if data.seat.is_none() {
                tracing::warn!("no wl_seat found");
            }
        }

        // Flush the manager bind request enqueued during the roundtrip
        if let Err(e) = conn.flush() {
            tracing::warn!(%e, "Wayland flush failed");
        }

        std::thread::Builder::new()
            .name("wayland-toplevel".into())
            .spawn(move || {
                // Roundtrip to receive initial toplevel events from the manager
                if let Err(e) = event_queue.roundtrip(&mut state) {
                    tracing::error!(%e, "initial toplevel roundtrip failed");
                    return;
                }

                loop {
                    if let Err(e) = event_queue.blocking_dispatch(&mut state) {
                        tracing::error!(%e, "Wayland dispatch error");
                        break;
                    }
                }
            })
            .ok()?;

        tracing::info!("Wayland toplevel activator connected");
        Some(Self { shared, conn })
    }

    /// Activate a window matching the given app_id and title.
    pub fn activate(&self, app_id: Option<&str>, title: Option<&str>) -> bool {
        let data = match self.shared.lock() {
            Ok(d) => d,
            Err(_) => return false,
        };

        let Some(seat) = &data.seat else {
            return false;
        };

        let matched = data.toplevels.iter().find(|tl| {
            let app_match = match (app_id, &tl.app_id) {
                (Some(a), Some(b)) => a == b,
                (None, None) => true,
                _ => false,
            };
            let title_match = match (title, &tl.title) {
                (Some(a), Some(b)) => a == b,
                (None, None) => true,
                _ => false,
            };
            app_match && title_match
        });

        if let Some(tl) = matched {
            tl.handle.activate(seat);
            drop(data);
            let _ = self.conn.flush();
            true
        } else {
            tracing::debug!(?app_id, ?title, "no Wayland toplevel matched");
            false
        }
    }
}

struct DispatchState {
    shared: Arc<Mutex<SharedData>>,
    pending: Vec<PendingToplevel>,
}

struct PendingToplevel {
    handle: ZwlrForeignToplevelHandleV1,
    app_id: Option<String>,
    title: Option<String>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for DispatchState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "wl_seat" => {
                    let seat = registry.bind::<wl_seat::WlSeat, _, _>(name, version.min(8), qh, ());
                    if let Ok(mut data) = state.shared.lock() {
                        data.seat = Some(seat);
                    }
                }
                "zwlr_foreign_toplevel_manager_v1" => {
                    registry.bind::<ZwlrForeignToplevelManagerV1, _, _>(
                        name,
                        version.min(3),
                        qh,
                        (),
                    );
                    tracing::info!("bound zwlr_foreign_toplevel_manager_v1");
                }
                _ => {}
            }
        }
    }
}

delegate_noop!(DispatchState: ignore wl_seat::WlSeat);

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for DispatchState {
    event_created_child!(DispatchState, ZwlrForeignToplevelManagerV1, [
        zwlr_foreign_toplevel_manager_v1::EVT_TOPLEVEL_OPCODE => (ZwlrForeignToplevelHandleV1, ())
    ]);

    fn event(
        state: &mut Self,
        _proxy: &ZwlrForeignToplevelManagerV1,
        event: zwlr_foreign_toplevel_manager_v1::Event,
        _: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let zwlr_foreign_toplevel_manager_v1::Event::Toplevel { toplevel } = event {
            state.pending.push(PendingToplevel {
                handle: toplevel,
                app_id: None,
                title: None,
            });
        }
    }
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for DispatchState {
    fn event(
        state: &mut Self,
        handle: &ZwlrForeignToplevelHandleV1,
        event: zwlr_foreign_toplevel_handle_v1::Event,
        _: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_foreign_toplevel_handle_v1::Event::Title { title } => {
                if let Some(pending) = state.pending.iter_mut().find(|p| p.handle == *handle) {
                    pending.title = Some(title);
                } else if let Ok(mut data) = state.shared.lock() {
                    if let Some(tl) = data.toplevels.iter_mut().find(|t| t.handle == *handle) {
                        tl.title = Some(title);
                    }
                }
            }
            zwlr_foreign_toplevel_handle_v1::Event::AppId { app_id } => {
                if let Some(pending) = state.pending.iter_mut().find(|p| p.handle == *handle) {
                    pending.app_id = Some(app_id);
                } else if let Ok(mut data) = state.shared.lock() {
                    if let Some(tl) = data.toplevels.iter_mut().find(|t| t.handle == *handle) {
                        tl.app_id = Some(app_id);
                    }
                }
            }
            zwlr_foreign_toplevel_handle_v1::Event::Done => {
                if let Some(idx) = state.pending.iter().position(|p| p.handle == *handle) {
                    let pending = state.pending.swap_remove(idx);
                    tracing::debug!(app_id = ?pending.app_id, title = ?pending.title, "toplevel registered");
                    if let Ok(mut data) = state.shared.lock() {
                        data.toplevels.push(ToplevelInfo {
                            handle: pending.handle,
                            app_id: pending.app_id,
                            title: pending.title,
                        });
                    }
                }
            }
            zwlr_foreign_toplevel_handle_v1::Event::Closed => {
                state.pending.retain(|p| p.handle != *handle);
                if let Ok(mut data) = state.shared.lock() {
                    data.toplevels.retain(|t| t.handle != *handle);
                }
            }
            _ => {}
        }
    }
}
