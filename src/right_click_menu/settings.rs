use serde::Deserialize;

use crate::window_button::settings::{MultiSelectAction, WindowAction};

#[derive(Debug, Clone, Deserialize)]
pub struct ContextMenuItem {
    pub label: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub action: Option<WindowAction>,
    #[serde(default)]
    pub command: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MultiSelectMenuItem {
    pub label: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub action: Option<MultiSelectAction>,
    #[serde(default)]
    pub command: Option<String>,
}

fn action_item(icon: &str, label: &str, action: WindowAction) -> ContextMenuItem {
    ContextMenuItem {
        label: label.to_string(),
        icon: Some(icon.to_string()),
        action: Some(action),
        command: None,
    }
}

pub fn default_context_menu() -> Vec<Vec<ContextMenuItem>> {
    vec![
        vec![
            action_item("view-fullscreen-symbolic", "Maximize Column", WindowAction::MaximizeColumn),
            action_item("view-fullscreen-symbolic", "Maximize to Edges", WindowAction::MaximizeWindowToEdges),
        ],
        vec![
            action_item("object-flip-vertical-symbolic", "Toggle Floating", WindowAction::ToggleWindowFloating),
        ],
        vec![
            action_item("window-close-symbolic", "Close Window", WindowAction::CloseWindow),
        ],
    ]
}

pub fn default_multi_select_menu() -> Vec<MultiSelectMenuItem> {
    vec![
        MultiSelectMenuItem {
            label: "Close All".to_string(),
            icon: Some("window-close-symbolic".to_string()),
            action: Some(MultiSelectAction::CloseWindows),
            command: None,
        },
        MultiSelectMenuItem {
            label: "Move All to Workspace Up".to_string(),
            icon: Some("go-up-symbolic".to_string()),
            action: Some(MultiSelectAction::MoveToWorkspaceUp),
            command: None,
        },
        MultiSelectMenuItem {
            label: "Move All to Workspace Down".to_string(),
            icon: Some("go-down-symbolic".to_string()),
            action: Some(MultiSelectAction::MoveToWorkspaceDown),
            command: None,
        },
    ]
}
