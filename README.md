# Niri Waybar Windowlist

A Waybar CFFI module for managing windows in the [Niri](https://github.com/YaLTeR/niri) compositor. Hard fork of [niri_waybar_windowlist](https://github.com/csmertx/niri_waybar_windowlist).

<!-- ![screenshot](demo.png) -->

## Features

- Window buttons with icons, titles, and audio indicators
- Configurable click actions per button state (focused/unfocused) and per app
- Context menu, multi-select, drag-and-drop reordering
- Notification urgency hints
- Multi-monitor and per-output sizing
- Scroll overflow with arrow navigation

## Installation

```bash
cargo build --release
# Output: target/release/libniri_window_buttons.so
```

## Configuration

Add to your Waybar config:

```jsonc
{
  "modules-center": ["cffi/niri_window_buttons"],
  "cffi/niri_window_buttons": {
    "module_path": "/path/to/libniri_window_buttons.so"
  }
}
```

All options have sensible defaults. Override only what you need.

### Display

| Option                   | Default  | Description                     |
| ------------------------ | -------- | ------------------------------- |
| `show_all_outputs`       | `false`  | Show windows from all monitors  |
| `only_current_workspace` | `true`   | Limit to current workspace      |
| `show_window_titles`     | `true`   | Show titles next to icons       |
| `truncate_titles`        | `true`   | Ellipsize long titles           |
| `allow_title_linebreaks` | `false`  | Allow `\n` in titles            |
| `show_tooltip`           | `true`   | Tooltip on hover                |
| `tooltip_delay`          | `300`    | Tooltip delay (ms)              |
| `button_alignment`       | `"left"` | `"left"`, `"center"`, `"right"` |
| `drag_hover_focus`       | `true`   | Focus on external drag hover    |
| `drag_hover_focus_delay` | `500`    | Drag hover delay (ms)           |

### Sizing

| Option               | Default | Description                     |
| -------------------- | ------- | ------------------------------- |
| `min_button_width`   | `150`   | Minimum button width (px)       |
| `max_button_width`   | none    | Maximum button width (px)       |
| `max_taskbar_width`  | `1200`  | Total taskbar width limit (px)  |
| `icon_size`          | `24`    | Icon size (px)                  |
| `icon_spacing`       | `6`     | Gap between icon and title (px) |
| `scroll_arrow_left`  | `"◀"`   | Left overflow arrow             |
| `scroll_arrow_right` | `"▶"`   | Right overflow arrow            |

Per-output overrides via `dimensions_per_output`:

```jsonc
"dimensions_per_output": {
  "eDP-1": { "min_button_width": 100, "max_taskbar_width": 800 }
}
```

### Click Actions

```jsonc
"click_actions": {
  "left_click_unfocused": "focus-window",
  "left_click_focused": "maximize-column",
  "double_click": "maximize-window-to-edges",
  "right_click_unfocused": "menu",
  "right_click_focused": "menu",
  "middle_click_unfocused": "close-window",
  "middle_click_focused": "close-window",
  "scroll_up": "none",
  "scroll_down": "none"
}
```

Actions can also be shell commands: `{ "command": "notify-send '{app_id}'" }` with placeholders `{window_id}`, `{app_id}`, `{title}`.

When `scroll_up`/`scroll_down` are `"none"`, the mouse wheel scrolls the taskbar instead.

<details>
<summary>All available actions</summary>

| Action                                                | Description                               |
| ----------------------------------------------------- | ----------------------------------------- |
| `none`                                                | No-op (scroll: enables taskbar scrolling) |
| `menu`                                                | Show context menu                         |
| `focus-window`                                        | Focus the window                          |
| `close-window`                                        | Close the window                          |
| `maximize-column`                                     | Maximize column width                     |
| `maximize-window-to-edges`                            | Maximize to screen edges                  |
| `expand-column-to-available-width`                    | Fill available space                      |
| `reset-window-height`                                 | Reset to default height                   |
| `switch-preset-column-width`                          | Cycle preset widths                       |
| `switch-preset-window-height`                         | Cycle preset heights                      |
| `center-column`                                       | Center column                             |
| `center-window`                                       | Center window                             |
| `center-visible-columns`                              | Center all visible columns                |
| `fullscreen-window`                                   | Toggle fullscreen                         |
| `toggle-windowed-fullscreen`                          | Toggle windowed fullscreen                |
| `toggle-window-floating`                              | Toggle floating                           |
| `consume-window-into-column`                          | Stack into adjacent column                |
| `expel-window-from-column`                            | Unstack from column                       |
| `toggle-column-tabbed-display`                        | Toggle tabbed display                     |
| `move-column-left` / `right` / `to-first` / `to-last` | Move column                               |
| `move-window-up` / `down`                             | Move window in column                     |
| `move-window-to-workspace-up` / `down`                | Move to workspace                         |
| `move-window-up-or-to-workspace-up`                   | Move up or to workspace above             |
| `move-window-down-or-to-workspace-down`               | Move down or to workspace below           |
| `move-window-to-monitor-left` / `right`               | Move to monitor                           |
| `move-column-left-or-to-monitor-left`                 | Move column or to monitor                 |
| `move-column-right-or-to-monitor-right`               | Move column or to monitor                 |
| `focus-workspace-previous`                            | Focus previous workspace                  |

</details>

### Context Menu

```jsonc
"context_menu": [
  {"label": "  Maximize Column", "action": "maximize-column"},
  {"label": "  Close Window", "action": "close-window"},
  {"label": "  Run Script", "command": "my-script.sh {window_id}"}
]
```

### Multi-Select

Hold a modifier key and click to select multiple windows, then right-click for batch actions.

```jsonc
{
  "multi_select_modifier": "ctrl",
  "multi_select_menu": [
    { "label": "  Close All", "action": "close-windows" },
    { "label": "  Move All Up", "action": "move-to-workspace-up" }
  ]
}
```

Modifier options: `ctrl`, `shift`, `alt`, `super`. Modifier + drag moves entire columns instead of individual windows.

Requires membership in the `input` group: `sudo usermod -aG input $USER`

### Per-App Rules

Override click actions based on app ID and title regex:

```jsonc
"apps": {
  "firefox": [
    {
      "match": ".*Picture-in-Picture.*",
      "click_actions": { "left_click_focused": "toggle-window-floating" }
    }
  ]
}
```

### Ignore Rules

Hide windows from the taskbar:

```jsonc
"ignore_rules": [
  {"app_id": "xpad"},
  {"app_id": "firefox", "title_contains": "Picture-in-Picture"},
  {"title_regex": "^Friends List$"},
  {"workspace": 9}
]
```

Matchers: `app_id`, `title`, `title_contains`, `title_regex`, `workspace`. All matchers in a rule use AND logic.

### Notifications

Marks window buttons as urgent when the app sends a notification. Enabled by default.

```jsonc
"notifications": {
  "enabled": true,
  "use_desktop_entry": true,
  "use_fuzzy_matching": false,
  "map_app_ids": { "org.telegram.desktop": "telegram" }
}
```

### Audio Indicator

Shows a speaker icon after the window title when the app is playing audio. Click to toggle mute. Enabled by default. Requires `libpulse`.

```jsonc
"audio_indicator": {
  "enabled": true,
  "playing_icon": "󰕾",
  "muted_icon": "󰖁",
  "clickable": true
}
```

For apps with multiple windows sharing a PID, the indicator is shown only on the focused window.

## Limitations

- Maximized-to-edges state cannot be visually indicated (niri IPC doesn't expose it)
