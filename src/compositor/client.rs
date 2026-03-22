use std::collections::HashMap;

use niri_ipc::{Action, Output, Request};

use super::{event_stream::NiriEventStream, send_request, validate_handled};
use crate::{settings::Settings, CompositorIpcError};

#[derive(Debug, Clone)]
pub struct CompositorClient {
    settings: Settings,
}

impl CompositorClient {
    pub fn create(settings: Settings) -> Self {
        Self { settings }
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn focus_window(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        let response = send_request(Request::Action(Action::FocusWindow { id: window_id }))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn close_window(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        let response = send_request(Request::Action(Action::CloseWindow {
            id: Some(window_id),
        }))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn maximize_window_column(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::MaximizeColumn {}))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn maximize_window_to_edges(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::MaximizeWindowToEdges {
            id: Some(window_id),
        }))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn center_column(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::CenterColumn {}))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn fullscreen_window(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        let response = send_request(Request::Action(Action::FullscreenWindow {
            id: Some(window_id),
        }))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn toggle_floating(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        let response = send_request(Request::Action(Action::ToggleWindowFloating {
            id: Some(window_id),
        }))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn center_window(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        let response = send_request(Request::Action(Action::CenterWindow {
            id: Some(window_id),
        }))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn center_visible_columns(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::CenterVisibleColumns {}))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn expand_column_to_available_width(
        &self,
        window_id: u64,
    ) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::ExpandColumnToAvailableWidth {}))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn toggle_windowed_fullscreen(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        let response = send_request(Request::Action(Action::ToggleWindowedFullscreen {
            id: Some(window_id),
        }))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn consume_window_into_column(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::ConsumeWindowIntoColumn {}))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn expel_window_from_column(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::ExpelWindowFromColumn {}))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn reset_window_height(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::ResetWindowHeight { id: None }))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn switch_preset_column_width(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::SwitchPresetColumnWidth {}))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn switch_preset_window_height(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::SwitchPresetWindowHeight {
            id: None,
        }))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn move_window_to_workspace_down(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::MoveWindowToWorkspaceDown {
            focus: false,
        }))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn move_window_to_workspace_up(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::MoveWindowToWorkspaceUp {
            focus: false,
        }))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn move_window_to_monitor_left(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::MoveWindowToMonitorLeft {}))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn move_window_to_monitor_right(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::MoveWindowToMonitorRight {}))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn move_window_to_monitor_up(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::MoveWindowToMonitorUp {}))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn move_window_to_monitor_down(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::MoveWindowToMonitorDown {}))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn toggle_column_tabbed_display(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::ToggleColumnTabbedDisplay {}))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn focus_workspace_previous(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::FocusWorkspacePrevious {}))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn move_column_left(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::MoveColumnLeft {}))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn move_column_right(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::MoveColumnRight {}))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn move_column_to_first(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::MoveColumnToFirst {}))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn move_column_to_last(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::MoveColumnToLast {}))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn move_window_down(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::MoveWindowDown {}))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn move_window_up(&self, window_id: u64) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::MoveWindowUp {}))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn move_window_down_or_to_workspace_down(
        &self,
        window_id: u64,
    ) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::MoveWindowDownOrToWorkspaceDown {}))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn move_window_up_or_to_workspace_up(
        &self,
        window_id: u64,
    ) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::MoveWindowUpOrToWorkspaceUp {}))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn move_column_left_or_to_monitor_left(
        &self,
        window_id: u64,
    ) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::MoveColumnLeftOrToMonitorLeft {}))?;
        validate_handled(response)
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn move_column_right_or_to_monitor_right(
        &self,
        window_id: u64,
    ) -> Result<(), CompositorIpcError> {
        self.focus_window(window_id)?;
        let response = send_request(Request::Action(Action::MoveColumnRightOrToMonitorRight {}))?;
        validate_handled(response)
    }

    pub fn query_outputs() -> Result<HashMap<String, Output>, CompositorIpcError> {
        let response = send_request(Request::Outputs)?;
        match response {
            Ok(niri_ipc::Response::Outputs(outputs)) => Ok(outputs),
            Ok(other) => Err(CompositorIpcError::unexpected_response("Outputs", other)),
            Err(msg) => Err(CompositorIpcError::Reply(msg)),
        }
    }

    pub fn create_event_stream(&self) -> NiriEventStream {
        NiriEventStream::start(self.settings.only_current_workspace())
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn reposition_window(
        &self,
        window_id: u64,
        position_delta: i32,
        keep_stacked: bool,
    ) -> Result<(), CompositorIpcError> {
        if position_delta == 0 {
            return Ok(());
        }

        tracing::info!(
            "repositioning window {} by {} columns (keep_stacked: {})",
            window_id,
            position_delta,
            keep_stacked
        );

        let response = send_request(Request::Windows)?;
        let all_windows: Vec<niri_ipc::Window> = match response {
            Ok(niri_ipc::Response::Windows(windows)) => windows,
            Ok(other) => return Err(CompositorIpcError::unexpected_response("Windows", other)),
            Err(msg) => return Err(CompositorIpcError::Reply(msg)),
        };

        let currently_focused = all_windows.iter().find(|w| w.is_focused).map(|w| w.id);

        let target_window = all_windows.iter().find(|w| w.id == window_id);
        let Some(target) = target_window else {
            tracing::warn!("target window not found in window list");
            return Ok(());
        };

        let (current_col, tile_position) = target.layout.pos_in_scrolling_layout.unwrap_or((1, 1));
        let is_stacked = tile_position > 1;

        self.focus_window(window_id)?;

        let effective_col = if is_stacked && !keep_stacked {
            tracing::trace!("expelling stacked window from column");
            let response = send_request(Request::Action(Action::ExpelWindowFromColumn {}))?;
            validate_handled(response)?;

            let response = send_request(Request::Windows)?;
            let windows: Vec<niri_ipc::Window> = match response {
                Ok(niri_ipc::Response::Windows(w)) => w,
                Ok(other) => return Err(CompositorIpcError::unexpected_response("Windows", other)),
                Err(msg) => return Err(CompositorIpcError::Reply(msg)),
            };

            windows
                .iter()
                .find(|w| w.id == window_id)
                .and_then(|w| w.layout.pos_in_scrolling_layout)
                .map_or(current_col, |(col, _)| col)
        } else {
            current_col
        };

        let target_index: usize = (i32::try_from(effective_col).unwrap_or(i32::MAX)
            + position_delta)
            .max(1)
            .try_into()
            .expect("target index is positive after .max(1)");
        tracing::trace!("moving column from {} to {}", effective_col, target_index);

        let response = send_request(Request::Action(Action::MoveColumnToIndex {
            index: target_index,
        }))?;
        validate_handled(response)?;

        if let Some(original_focus) = currently_focused {
            if original_focus != window_id {
                self.focus_window(original_focus)?;
            }
        }

        Ok(())
    }
}
