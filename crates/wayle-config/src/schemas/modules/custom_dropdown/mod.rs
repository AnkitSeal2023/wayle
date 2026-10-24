use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};

use crate::{
    docs::{ConfigGroup, ModuleInfo, ModuleInfoProvider},
    schemas::modules::{ExecutionMode, RestartDelay, RestartPolicy},
};

/// User-defined dropdown panel opened by any module's click action.
///
/// Panels are named by `id` and opened from any bar module button with
/// `dropdown:<id>` (for example `dropdown:menu`). Content is either declared
/// statically via `items`, or produced by a `command` — the command's output
/// replaces the static items whenever it produces content.
///
/// Full walkthrough with examples at <https://wayle.app/guide/custom-modules>.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub struct CustomDropdownDefinition {
    /// Unique identifier for this dropdown.
    ///
    /// Referenced from module click actions as `dropdown:<id>`. Must be
    /// unique across all custom dropdown definitions.
    ///
    /// ## Example
    ///
    /// ```toml
    /// [[modules.dropdowns]]
    /// id = "menu"
    ///
    /// # Reference in a module:
    /// # left-click = "dropdown:menu"
    /// ```
    pub id: String,

    /// Title shown in the dropdown header.
    ///
    /// A command's JSON output can override this at runtime via its `title`
    /// field. If neither is set, the header is hidden entirely.
    #[serde(default)]
    pub title: Option<String>,

    /// Icon name shown at the start of the dropdown header.
    ///
    /// A command's JSON output can override this at runtime via its `icon`
    /// field. Should be a symbolic icon name from the icon theme
    /// (e.g., `"ld-settings-symbolic"`).
    #[serde(default)]
    pub icon: Option<String>,

    /// Panel width in pixels (before scale).
    ///
    /// Uses built-in default when omitted.
    #[serde(default)]
    pub width: Option<u32>,

    /// Shell command producing the dropdown content.
    ///
    /// The command runs via `sh -c` and should output to stdout. Stderr is
    /// discarded. Commands have a 30-second timeout.
    ///
    /// ## Output Parsing
    ///
    /// - If output starts with `{`: parsed as JSON. Recognized fields are
    ///   `title`, `icon`, `width`, and `items`; anything else is optional.
    /// - Otherwise: each non-empty line becomes a plain item without an
    ///   action.
    ///
    /// ## Behavior by Mode
    ///
    /// - **poll**: Executed every `interval-ms` milliseconds
    /// - **watch**: Spawned once, each stdout line is one complete panel
    ///   payload. Restarts are controlled by `restart-policy`.
    ///
    /// While no command output exists yet, static `items` are shown.
    /// An empty output clears the list (the empty state is shown).
    #[serde(default)]
    pub command: Option<String>,

    /// Execution mode for the command.
    ///
    /// | Mode | Behavior |
    /// |------|----------|
    /// | `poll` | Run command every `interval-ms` (default) |
    /// | `watch` | Spawn long-running process, update on each stdout line |
    ///
    /// Use `poll` for commands that return the panel state and exit.
    /// Use `watch` for commands that stream panel updates.
    #[serde(default)]
    pub mode: ExecutionMode,

    /// Polling interval in milliseconds.
    ///
    /// Only applies to `poll` mode. Ignored in `watch` mode.
    ///
    /// Set to `0` for manual mode: no timer is started. In manual mode, the
    /// command still runs once at startup.
    #[serde(rename = "interval-ms", default = "default_interval")]
    pub interval_ms: u64,

    /// Restart policy for watch mode.
    ///
    /// Only applies to `watch` mode. Ignored in `poll` mode.
    ///
    /// | Policy | Behavior |
    /// |--------|----------|
    /// | `never` | Do not restart after exit |
    /// | `on-exit` | Restart after any exit |
    /// | `on-failure` | Restart only after non-zero/signal exit |
    #[serde(default)]
    pub restart_policy: RestartPolicy,

    /// Base restart delay in milliseconds for watch mode.
    ///
    /// Only applies to `watch` mode. Ignored in `poll` mode.
    ///
    /// Used when `restart-policy` is `on-exit` or `on-failure`.
    /// Delay increases exponentially on rapid failures, capped at 30 seconds.
    #[serde(rename = "restart-interval-ms", default)]
    pub restart_interval_ms: RestartDelay,

    /// Static list of panel items.
    ///
    /// Each item is one row in the panel. Items may run a `command` when
    /// clicked. When `command` output is present it replaces this list; use
    /// static items for dropdowns without a backing process.
    ///
    /// ## Example
    ///
    /// ```toml
    /// [[modules.dropdowns]]
    /// id = "menu"
    /// title = "Quick actions"
    ///
    /// [[modules.dropdowns.items]]
    /// label = "Terminal"
    /// icon = "ld-terminal-symbolic"
    /// command = "foot"
    /// ```
    #[serde(default)]
    pub items: Vec<CustomDropdownItem>,

    /// Close the dropdown after an item's command runs.
    ///
    /// Set to `false` for commands that produce feedback while the panel
    /// stays open (e.g., clipboard pickers that redraw their own state).
    #[serde(rename = "close-on-click", default = "default_close_on_click")]
    pub close_on_click: bool,
}

/// One row of a custom dropdown panel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub struct CustomDropdownItem {
    /// Primary text shown for the item.
    pub label: String,

    /// Leading symbolic icon name.
    #[serde(default)]
    pub icon: Option<String>,

    /// Secondary description below the label.
    #[serde(default)]
    pub subtitle: Option<String>,

    /// Shell command executed when the item is clicked.
    #[serde(default)]
    pub command: Option<String>,

    /// Shows a check mark on the trailing edge, marking the current choice.
    #[serde(default)]
    pub active: bool,

    /// Extra CSS class added to the row for theme-specific styling.
    #[serde(default)]
    pub class: Option<String>,
}

fn default_interval() -> u64 {
    5000
}

fn default_close_on_click() -> bool {
    true
}

impl ModuleInfoProvider for CustomDropdownDefinition {
    fn module_info() -> ModuleInfo {
        ModuleInfo {
            name: String::from("dropdowns"),
            schema: || schema_for!(CustomDropdownDefinition),
            layout_id: None,
            array_entry: true,
        }
    }

    fn groups() -> Vec<ConfigGroup> {
        vec![
            ConfigGroup::general(),
            ConfigGroup::prefix("Restart", "restart-"),
        ]
    }
}

crate::register_module!(CustomDropdownDefinition);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_definition_has_minimal_fields() {
        let toml_input = r#"
id = "menu"
"#;
        let definition: CustomDropdownDefinition = toml::from_str(toml_input).unwrap();
        assert_eq!(definition.id, "menu");
        assert_eq!(definition.mode, ExecutionMode::Poll);
        assert_eq!(definition.interval_ms, 5000);
        assert!(definition.close_on_click);
        assert!(definition.title.is_none());
        assert!(definition.icon.is_none());
        assert!(definition.width.is_none());
        assert!(definition.command.is_none());
        assert!(definition.items.is_empty());
        assert_eq!(definition.restart_policy, RestartPolicy::Never);
    }

    #[test]
    fn parses_static_items() {
        let toml_input = r#"
id = "menu"
title = "Quick actions"
icon = "ld-settings-symbolic"
width = 360

[[items]]
label = "Terminal"
icon = "ld-terminal-symbolic"
subtitle = "foot"
command = "foot"
active = false
class = "accent"
"#;

        let definition: CustomDropdownDefinition = toml::from_str(toml_input).unwrap();
        assert_eq!(definition.title.as_deref(), Some("Quick actions"));
        assert_eq!(definition.width, Some(360));
        assert_eq!(definition.items.len(), 1);
        assert_eq!(definition.items[0].label, "Terminal");
        assert_eq!(definition.items[0].command.as_deref(), Some("foot"));
    }

    #[test]
    fn full_config_array_of_tables() {
        let toml_input = r#"
[[modules.dropdowns]]
id = "menu"
title = "Quick actions"

[[modules.dropdowns.items]]
label = "Terminal"
command = "foot"

[[modules.dropdowns.items]]
label = "Browser"
command = "firefox"

[[modules.dropdowns]]
id = "other"
command = "build-panel"
"#;
        let config: crate::Config = toml::from_str(toml_input).unwrap();
        let dropdowns = config.modules.dropdowns.get();
        assert_eq!(dropdowns.len(), 2);
        assert_eq!(dropdowns[0].id, "menu");
        assert_eq!(dropdowns[0].items.len(), 2);
        assert_eq!(dropdowns[0].items[1].label, "Browser");
        assert_eq!(dropdowns[1].id, "other");
    }
}
