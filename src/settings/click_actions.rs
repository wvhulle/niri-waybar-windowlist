use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ClickActions {
    pub left_click_unfocused: ClickAction,
    pub left_click_focused: ClickAction,
    pub double_click: ClickAction,
    pub right_click_unfocused: ClickAction,
    pub right_click_focused: ClickAction,
    pub middle_click_unfocused: ClickAction,
    pub middle_click_focused: ClickAction,
    pub scroll_up: ClickAction,
    pub scroll_down: ClickAction,
}

impl Default for ClickActions {
    fn default() -> Self {
        Self {
            left_click_unfocused: ClickAction::Action(WindowAction::FocusWindow),
            left_click_focused: ClickAction::Action(WindowAction::MaximizeColumn),
            double_click: ClickAction::Action(WindowAction::MaximizeWindowToEdges),
            right_click_unfocused: ClickAction::Action(WindowAction::Menu),
            right_click_focused: ClickAction::Action(WindowAction::Menu),
            middle_click_unfocused: ClickAction::Action(WindowAction::CloseWindow),
            middle_click_focused: ClickAction::Action(WindowAction::CloseWindow),
            scroll_up: ClickAction::Action(WindowAction::None),
            scroll_down: ClickAction::Action(WindowAction::None),
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum WindowAction {
    None,
    FocusWindow,
    CloseWindow,
    MaximizeColumn,
    MaximizeWindowToEdges,
    CenterColumn,
    CenterWindow,
    CenterVisibleColumns,
    ExpandColumnToAvailableWidth,
    FullscreenWindow,
    ToggleWindowedFullscreen,
    ToggleWindowFloating,
    ConsumeWindowIntoColumn,
    ExpelWindowFromColumn,
    ResetWindowHeight,
    SwitchPresetColumnWidth,
    SwitchPresetWindowHeight,
    MoveWindowToWorkspaceDown,
    MoveWindowToWorkspaceUp,
    MoveWindowToMonitorLeft,
    MoveWindowToMonitorRight,
    ToggleColumnTabbedDisplay,
    FocusWorkspacePrevious,
    MoveColumnLeft,
    MoveColumnRight,
    MoveColumnToFirst,
    MoveColumnToLast,
    MoveWindowDown,
    MoveWindowUp,
    MoveWindowDownOrToWorkspaceDown,
    MoveWindowUpOrToWorkspaceUp,
    MoveColumnLeftOrToMonitorLeft,
    MoveColumnRightOrToMonitorRight,
    Menu,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ClickAction {
    Action(WindowAction),
    Command { command: String },
}

impl ClickAction {
    pub fn is_menu(&self) -> bool {
        matches!(self, ClickAction::Action(WindowAction::Menu))
    }

    pub fn is_none(&self) -> bool {
        matches!(self, ClickAction::Action(WindowAction::None))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContextMenuItem {
    pub label: String,
    #[serde(default)]
    pub action: Option<WindowAction>,
    #[serde(default)]
    pub command: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MultiSelectMenuItem {
    pub label: String,
    #[serde(default)]
    pub action: Option<MultiSelectAction>,
    #[serde(default)]
    pub command: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum MultiSelectAction {
    CloseWindows,
    MoveToWorkspaceUp,
    MoveToWorkspaceDown,
    MoveToMonitorLeft,
    MoveToMonitorRight,
    MoveToMonitorUp,
    MoveToMonitorDown,
    MoveColumnLeft,
    MoveColumnRight,
    ToggleFloating,
    FullscreenWindows,
    MaximizeColumns,
    CenterColumns,
    ConsumeIntoColumn,
    ToggleTabbedDisplay,
}

pub fn default_context_menu() -> Vec<ContextMenuItem> {
    vec![
        ContextMenuItem {
            label: "  Maximize Column".to_string(),
            action: Some(WindowAction::MaximizeColumn),
            command: None,
        },
        ContextMenuItem {
            label: "  Maximize to Edges".to_string(),
            action: Some(WindowAction::MaximizeWindowToEdges),
            command: None,
        },
        ContextMenuItem {
            label: "󰉩  Toggle Floating".to_string(),
            action: Some(WindowAction::ToggleWindowFloating),
            command: None,
        },
        ContextMenuItem {
            label: "  Close Window".to_string(),
            action: Some(WindowAction::CloseWindow),
            command: None,
        },
    ]
}

pub fn default_multi_select_menu() -> Vec<MultiSelectMenuItem> {
    vec![
        MultiSelectMenuItem {
            label: "  Close All".to_string(),
            action: Some(MultiSelectAction::CloseWindows),
            command: None,
        },
        MultiSelectMenuItem {
            label: "  Move All to Workspace Up".to_string(),
            action: Some(MultiSelectAction::MoveToWorkspaceUp),
            command: None,
        },
        MultiSelectMenuItem {
            label: "  Move All to Workspace Down".to_string(),
            action: Some(MultiSelectAction::MoveToWorkspaceDown),
            command: None,
        },
    ]
}
