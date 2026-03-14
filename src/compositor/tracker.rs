use std::collections::{BTreeMap, BTreeSet, HashMap};

use niri_ipc::{Event, Workspace};

use super::{
    event_stream::CompositorEvent,
    window_info::{WindowInfo, WindowSnapshot},
};

#[derive(Debug)]
pub(super) struct WindowTracker {
    state: Option<TrackerState>,
    focused_id: Option<u64>,
}

#[derive(Debug)]
enum TrackerState {
    WindowsOnly(Vec<niri_ipc::Window>),
    WorkspacesOnly(Vec<Workspace>),
    Ready {
        windows: BTreeMap<u64, niri_ipc::Window>,
        workspaces: BTreeMap<u64, Workspace>,
        active_per_workspace: BTreeMap<u64, u64>,
        last_focused_per_workspace: BTreeMap<u64, u64>,
    },
}

impl WindowTracker {
    pub(super) fn new() -> Self {
        Self {
            state: None,
            focused_id: None,
        }
    }

    #[tracing::instrument(level = "TRACE", skip(self))]
    pub(super) fn process_event(
        &mut self,
        event: Event,
        filter_workspace: bool,
    ) -> Vec<CompositorEvent> {
        use TrackerState::*;

        match event {
            Event::WindowsChanged { windows } => {
                self.state = match self.state.take() {
                    Some(WorkspacesOnly(ws)) => Some(Ready {
                        windows: windows.iter().map(|w| (w.id, w.clone())).collect(),
                        workspaces: ws.into_iter().map(|w| (w.id, w)).collect(),
                        active_per_workspace: BTreeMap::new(),
                        last_focused_per_workspace: BTreeMap::new(),
                    }),
                    Some(Ready {
                        workspaces,
                        active_per_workspace,
                        last_focused_per_workspace,
                        ..
                    }) => Some(Ready {
                        windows: windows.iter().map(|w| (w.id, w.clone())).collect(),
                        workspaces,
                        active_per_workspace,
                        last_focused_per_workspace,
                    }),
                    _ => Some(WindowsOnly(windows)),
                };
                self.maybe_full_snapshot(filter_workspace)
            }
            Event::WorkspacesChanged { workspaces } => {
                self.state = match self.state.take() {
                    Some(WindowsOnly(wins)) => Some(Ready {
                        windows: wins.iter().map(|w| (w.id, w.clone())).collect(),
                        workspaces: workspaces.into_iter().map(|w| (w.id, w)).collect(),
                        active_per_workspace: BTreeMap::new(),
                        last_focused_per_workspace: BTreeMap::new(),
                    }),
                    Some(Ready {
                        windows,
                        active_per_workspace,
                        last_focused_per_workspace,
                        ..
                    }) => Some(Ready {
                        windows,
                        workspaces: workspaces.into_iter().map(|w| (w.id, w)).collect(),
                        active_per_workspace,
                        last_focused_per_workspace,
                    }),
                    _ => Some(WorkspacesOnly(workspaces)),
                };
                self.maybe_full_snapshot(filter_workspace)
            }
            Event::WindowClosed { id } => {
                if let Some(Ready { windows, .. }) = &mut self.state {
                    windows.remove(&id);
                }
                if self.focused_id == Some(id) {
                    self.focused_id = None;
                }
                self.maybe_full_snapshot(filter_workspace)
            }
            Event::WindowOpenedOrChanged { window } => {
                let Some(Ready {
                    windows,
                    last_focused_per_workspace,
                    ..
                }) = &mut self.state
                else {
                    return vec![];
                };

                if window.is_focused {
                    Self::record_last_focused(windows, last_focused_per_workspace);
                    for w in windows.values_mut() {
                        w.is_focused = false;
                    }
                    self.focused_id = Some(window.id);
                }

                let previous = windows.insert(window.id, window);

                match previous {
                    None => self.maybe_full_snapshot(filter_workspace),
                    Some(prev) => {
                        let current = &windows[&prev.id];
                        if Self::only_title_differs(&prev, current) {
                            vec![CompositorEvent::WindowTitleChanged {
                                id: prev.id,
                                title: current.title.clone(),
                            }]
                        } else {
                            self.maybe_full_snapshot(filter_workspace)
                        }
                    }
                }
            }
            Event::WindowFocusChanged { id } => {
                let old = self.focused_id;
                if let Some(Ready {
                    windows,
                    last_focused_per_workspace,
                    ..
                }) = &mut self.state
                {
                    Self::record_last_focused(windows, last_focused_per_workspace);

                    for window in windows.values_mut() {
                        window.is_focused = Some(window.id) == id;
                    }

                    if let Some(focused_id) = id {
                        Self::record_scrolling_window(
                            windows,
                            last_focused_per_workspace,
                            focused_id,
                        );
                    }
                }
                self.focused_id = id;
                vec![CompositorEvent::FocusChanged { old, new: id }]
            }
            Event::WorkspaceActivated { id, .. } => {
                if let Some(Ready { workspaces, .. }) = &mut self.state {
                    let activated_output = workspaces.get(&id).and_then(|ws| ws.output.clone());

                    workspaces
                        .values_mut()
                        .filter(|ws| ws.output == activated_output)
                        .for_each(|ws| ws.is_active = ws.id == id);
                }
                self.maybe_full_snapshot(filter_workspace)
            }
            Event::WorkspaceActiveWindowChanged {
                workspace_id,
                active_window_id,
            } => {
                tracing::info!(
                    "workspace {} active window changed to {:?}",
                    workspace_id,
                    active_window_id
                );
                if let Some(Ready {
                    active_per_workspace,
                    ..
                }) = &mut self.state
                {
                    if let Some(win_id) = active_window_id {
                        active_per_workspace.insert(workspace_id, win_id);
                    } else {
                        active_per_workspace.remove(&workspace_id);
                    }
                    tracing::info!("active window map: {:?}", active_per_workspace);
                }
                self.maybe_full_snapshot(filter_workspace)
            }
            Event::WindowLayoutsChanged { changes } => {
                if let Some(Ready { windows, .. }) = &mut self.state {
                    for (win_id, layout) in changes {
                        if let Some(window) = windows.get_mut(&win_id) {
                            window.layout = layout;
                        } else {
                            tracing::warn!(win_id, ?layout, "layout update for unknown window");
                        }
                    }
                }
                self.maybe_full_snapshot(filter_workspace)
            }
            _ => vec![],
        }
    }

    fn only_title_differs(prev: &niri_ipc::Window, current: &niri_ipc::Window) -> bool {
        prev.title != current.title
            && prev.is_urgent == current.is_urgent
            && prev.is_focused == current.is_focused
            && prev.workspace_id == current.workspace_id
            && prev.app_id == current.app_id
    }

    fn record_last_focused(
        windows: &BTreeMap<u64, niri_ipc::Window>,
        last_focused_per_workspace: &mut BTreeMap<u64, u64>,
    ) {
        let Some(focused_id) = windows.values().find(|w| w.is_focused).map(|w| w.id) else {
            return;
        };
        Self::record_scrolling_window(windows, last_focused_per_workspace, focused_id);
    }

    fn record_scrolling_window(
        windows: &BTreeMap<u64, niri_ipc::Window>,
        last_focused_per_workspace: &mut BTreeMap<u64, u64>,
        window_id: u64,
    ) {
        if let Some(window) = windows.get(&window_id) {
            if window.layout.pos_in_scrolling_layout.is_some() {
                if let Some(ws_id) = window.workspace_id {
                    last_focused_per_workspace.insert(ws_id, window_id);
                }
            }
        }
    }

    fn maybe_full_snapshot(&self, filter_workspace: bool) -> Vec<CompositorEvent> {
        if let Some(TrackerState::Ready {
            windows,
            workspaces,
            active_per_workspace,
            last_focused_per_workspace,
        }) = &self.state
        {
            vec![CompositorEvent::FullSnapshot(self.generate_snapshot(
                windows,
                workspaces,
                active_per_workspace,
                last_focused_per_workspace,
                filter_workspace,
            ))]
        } else {
            vec![]
        }
    }

    fn generate_snapshot(
        &self,
        windows: &BTreeMap<u64, niri_ipc::Window>,
        workspaces: &BTreeMap<u64, Workspace>,
        active_per_workspace: &BTreeMap<u64, u64>,
        last_focused_per_workspace: &BTreeMap<u64, u64>,
        filter_workspace: bool,
    ) -> WindowSnapshot {
        struct WindowWithWorkspace<'a> {
            window: &'a niri_ipc::Window,
            workspace: &'a Workspace,
        }

        let active_workspace_per_output: HashMap<_, _> = workspaces
            .values()
            .filter(|ws| ws.is_active)
            .filter_map(|ws| ws.output.as_ref().map(|output| (output.clone(), ws.id)))
            .collect();

        let mut window_workspace_pairs: Vec<_> = windows
            .values()
            .filter_map(|window| {
                window.workspace_id.and_then(|ws_id| {
                    workspaces.get(&ws_id).and_then(|ws| {
                        if filter_workspace {
                            let is_active_on_output = ws
                                .output
                                .as_ref()
                                .and_then(|output| active_workspace_per_output.get(output))
                                .map(|active_ws_id| *active_ws_id == ws.id)
                                .unwrap_or(false);

                            if !is_active_on_output {
                                return None;
                            }
                        }
                        Some(WindowWithWorkspace {
                            window,
                            workspace: ws,
                        })
                    })
                })
            })
            .collect();

        let mut position_map: HashMap<u64, (usize, usize)> = HashMap::new();

        let workspace_ids: BTreeSet<_> = window_workspace_pairs
            .iter()
            .map(|p| p.workspace.id)
            .collect();
        for ws_id in workspace_ids {
            let anchor_pos = last_focused_per_workspace
                .get(&ws_id)
                .and_then(|win_id| {
                    window_workspace_pairs
                        .iter()
                        .find(|p| p.window.id == *win_id)
                        .and_then(|p| p.window.layout.pos_in_scrolling_layout)
                })
                .unwrap_or_else(|| {
                    window_workspace_pairs
                        .iter()
                        .filter(|p| {
                            p.workspace.id == ws_id
                                && p.window.layout.pos_in_scrolling_layout.is_some()
                        })
                        .filter_map(|p| p.window.layout.pos_in_scrolling_layout)
                        .max_by_key(|pos| (pos.0, pos.1))
                        .unwrap_or((0, 0))
                });

            window_workspace_pairs
                .iter()
                .filter(|p| {
                    p.workspace.id == ws_id && p.window.layout.pos_in_scrolling_layout.is_none()
                })
                .for_each(|pair| {
                    position_map.insert(pair.window.id, (anchor_pos.0, anchor_pos.1 + 1));
                });
        }

        window_workspace_pairs.sort_by(|a, b| {
            a.workspace
                .idx
                .cmp(&b.workspace.idx)
                .then_with(|| {
                    let a_pos = a
                        .window
                        .layout
                        .pos_in_scrolling_layout
                        .or_else(|| position_map.get(&a.window.id).copied())
                        .unwrap_or((usize::MAX, 0));
                    let b_pos = b
                        .window
                        .layout
                        .pos_in_scrolling_layout
                        .or_else(|| position_map.get(&b.window.id).copied())
                        .unwrap_or((usize::MAX, 0));
                    a_pos.0.cmp(&b_pos.0).then_with(|| a_pos.1.cmp(&b_pos.1))
                })
                .then_with(|| a.window.id.cmp(&b.window.id))
        });

        for pair in &window_workspace_pairs {
            let pos = pair
                .window
                .layout
                .pos_in_scrolling_layout
                .or_else(|| position_map.get(&pair.window.id).copied());
            tracing::debug!(
                window_id = pair.window.id,
                app_id = ?pair.window.app_id,
                workspace_idx = pair.workspace.idx,
                pos = ?pos,
                "snapshot order"
            );
        }

        let active_workspace = workspaces.values().find(|ws| ws.is_active).map(|ws| ws.id);
        let overview_active =
            active_workspace.and_then(|ws_id| active_per_workspace.get(&ws_id).copied());
        let has_focused = window_workspace_pairs
            .iter()
            .any(|pair| pair.window.is_focused);

        let highlight_window = if !has_focused {
            overview_active
                .or_else(|| {
                    active_workspace
                        .and_then(|ws_id| last_focused_per_workspace.get(&ws_id).copied())
                })
                .or_else(|| {
                    active_workspace.and_then(|active_ws| {
                        window_workspace_pairs
                            .iter()
                            .find(|pair| pair.workspace.id == active_ws)
                            .map(|pair| pair.window.id)
                    })
                })
        } else {
            None
        };

        tracing::info!(
            "snapshot: active_ws={:?}, overview={:?}, last_focused={:?}, highlight={:?}",
            active_workspace,
            overview_active,
            last_focused_per_workspace,
            highlight_window
        );

        window_workspace_pairs
            .into_iter()
            .map(|pair| {
                let mut window_copy = pair.window.clone();
                if !window_copy.is_focused && Some(window_copy.id) == highlight_window {
                    tracing::info!("highlighting window {}", window_copy.id);
                    window_copy.is_focused = true;
                }
                WindowInfo {
                    inner: window_copy,
                    output_name: pair.workspace.output.clone(),
                }
            })
            .collect()
    }
}
