use serde::Deserialize;

use crate::window_button::settings::{MultiSelectAction, WindowAction};

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
            label: "\u{f0269}  Toggle Floating".to_string(),
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
