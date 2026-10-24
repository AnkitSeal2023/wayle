//! Panel content parsing for user-defined dropdowns.
//!
//! A dropdown's `command` may output JSON describing the whole panel
//! (title, icon, width, item rows) or plain text (one item per line).
//! Either way the result is a [`ParsedPanel`] that replaces the static
//! config items while new output keeps arriving.

use serde_json::Value;
use tracing::warn;
use wayle_config::schemas::modules::CustomDropdownDefinition;

use super::item::DropdownListItem;

const MAX_JSON_PARSE_BYTES: usize = 64 * 1024;
const MAX_ITEMS: usize = 100;
const MAX_WIDTH_PX: u32 = 8192;

/// Panel content parsed from a command's output.
pub(crate) struct ParsedPanel {
    /// Header title; `None` when the output does not provide one.
    pub title: Option<String>,
    /// Header icon; `None` when the output does not provide one.
    pub icon: Option<String>,
    /// Panel width in pixels; `None` when the output does not provide one.
    pub width: Option<u32>,
    /// Panel rows.
    pub items: Vec<DropdownListItem>,
}

/// Finds a dropdown definition by ID.
pub(crate) fn find_definition(
    definitions: &[CustomDropdownDefinition],
    id: &str,
) -> Option<CustomDropdownDefinition> {
    definitions.iter().find(|def| def.id == id).cloned()
}

/// Parses command output into panel content.
///
/// JSON output (an object) provides title, icon, width, and items. A
/// malformed or non-object JSON payload falls back to plain-text parsing
/// so partial output still renders something useful instead of wiping
/// the panel.
pub(crate) fn parse_panel(output: &str) -> ParsedPanel {
    let trimmed = output.trim();

    let looks_like_json = matches!(trimmed.as_bytes().first(), Some(b'{') | Some(b'['))
        && trimmed.len() <= MAX_JSON_PARSE_BYTES;

    if looks_like_json && let Ok(payload) = serde_json::from_str::<Value>(trimmed) {
        return parse_json_panel(&payload);
    }

    parse_text_panel(trimmed)
}

fn parse_json_panel(payload: &Value) -> ParsedPanel {
    let Some(object) = payload.as_object() else {
        // A top-level JSON array is a bare list of items.
        let items = payload
            .as_array()
            .map(|nodes| collect_items(nodes.iter()))
            .unwrap_or_default();

        return ParsedPanel {
            title: None,
            icon: None,
            width: None,
            items,
        };
    };

    let items = object
        .get("items")
        .and_then(Value::as_array)
        .map(|nodes| collect_items(nodes.iter()))
        .unwrap_or_default();

    ParsedPanel {
        title: string_field(object.get("title")),
        icon: string_field(object.get("icon")),
        width: u32_field(object.get("width")),
        items,
    }
}

/// Parses one `"items"` element. Objects must carry a non-empty `label`
/// string; unknown shapes are skipped rather than failing the whole panel.
fn parse_item(node: &Value) -> Option<DropdownListItem> {
    match node {
        Value::String(label) => (!label.trim().is_empty()).then(|| DropdownListItem {
            label: label.trim().to_owned(),
            icon: None,
            subtitle: None,
            command: None,
            active: false,
            class: None,
        }),
        Value::Object(object) => object
            .get("label")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|label| !label.is_empty())
            .map(|label| DropdownListItem {
                label: label.to_owned(),
                icon: string_field(object.get("icon")),
                subtitle: string_field(object.get("subtitle")),
                command: string_field(object.get("command")),
                active: object
                    .get("active")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                class: string_field(object.get("class")),
            }),
        _ => None,
    }
}

/// Collects up to [`MAX_ITEMS`] parsable items, warning when truncated.
fn collect_items<'a>(nodes: impl Iterator<Item = &'a Value>) -> Vec<DropdownListItem> {
    let mut items = Vec::new();

    for node in nodes {
        if items.len() >= MAX_ITEMS {
            warn!(shown = MAX_ITEMS, "dropdown items capped");
            break;
        }

        if let Some(item) = parse_item(node) {
            items.push(item);
        }
    }

    items
}

/// Plain-text output: every non-empty line becomes a label-only item.
fn parse_text_panel(trimmed: &str) -> ParsedPanel {
    let mut items = Vec::new();

    for line in trimmed.lines() {
        let label = line.trim();
        if label.is_empty() {
            continue;
        }
        if items.len() >= MAX_ITEMS {
            warn!(shown = MAX_ITEMS, "dropdown text items capped");
            break;
        }

        items.push(DropdownListItem {
            label: label.to_owned(),
            icon: None,
            subtitle: None,
            command: None,
            active: false,
            class: None,
        });
    }

    ParsedPanel {
        title: None,
        icon: None,
        width: None,
        items,
    }
}

fn string_field(node: Option<&Value>) -> Option<String> {
    node.and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
}

fn u32_field(node: Option<&Value>) -> Option<u32> {
    node.and_then(Value::as_u64)
        .filter(|value| *value > 0 && *value <= u64::from(MAX_WIDTH_PX))
        .map(|value| value as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_plain_text_lines() {
        let panel = parse_panel("Alpha\n\nBeta  \nGamma");
        assert_eq!(panel.items.len(), 3);
        assert_eq!(panel.items[0].label, "Alpha");
        assert_eq!(panel.items[2].label, "Gamma");
        assert!(panel.title.is_none());
        assert!(panel.items.iter().all(|item| item.command.is_none()));
    }

    #[test]
    fn empty_output_clears_items() {
        let panel = parse_panel("   \n  ");
        assert!(panel.items.is_empty());
        assert!(panel.title.is_none());
    }

    #[test]
    fn parse_json_full_panel() {
        let output = r#"{
            "title": "Quick actions",
            "icon": "ld-settings-symbolic",
            "width": 360,
            "items": [
                {"label": "Terminal", "icon": "ld-terminal-symbolic",
                 "subtitle": "foot", "command": "foot", "active": true},
                {"label": "Browser", "command": "firefox", "class": "web"}
            ]
        }"#;

        let panel = parse_panel(output);
        assert_eq!(panel.title.as_deref(), Some("Quick actions"));
        assert_eq!(panel.icon.as_deref(), Some("ld-settings-symbolic"));
        assert_eq!(panel.width, Some(360));
        assert_eq!(panel.items.len(), 2);

        let first = &panel.items[0];
        assert_eq!(first.label, "Terminal");
        assert_eq!(first.icon.as_deref(), Some("ld-terminal-symbolic"));
        assert_eq!(first.subtitle.as_deref(), Some("foot"));
        assert_eq!(first.command.as_deref(), Some("foot"));
        assert!(first.active);

        let second = &panel.items[1];
        assert_eq!(second.command.as_deref(), Some("firefox"));
        assert!(!second.active);
        assert_eq!(second.class.as_deref(), Some("web"));
    }

    #[test]
    fn parse_json_string_items() {
        let output = r#"{"items": ["Alpha", "  Beta  "]}"#;
        let panel = parse_panel(output);
        assert_eq!(panel.items.len(), 2);
        assert_eq!(panel.items[0].label, "Alpha");
        assert_eq!(panel.items[1].label, "Beta");
    }

    #[test]
    fn parse_json_missing_labels_skipped() {
        let output = r#"{"items": [{"label": ""}, {"command": "no-label"}, {"label": "Ok", "command": "true"}]}"#;
        let panel = parse_panel(output);
        assert_eq!(panel.items.len(), 1);
        assert_eq!(panel.items[0].label, "Ok");
    }

    #[test]
    fn malformed_json_falls_back_to_text_lines() {
        let panel = parse_panel("{\"title\": \"cut off");
        assert!(panel.items.contains(&DropdownListItem {
            label: String::from("{\"title\": \"cut off"),
            icon: None,
            subtitle: None,
            command: None,
            active: false,
            class: None,
        }));
    }

    #[test]
    fn non_object_json_array_is_bare_item_list() {
        let panel = parse_panel(r#"["Alpha", {"label": "Beta", "command": "true"}]"#);
        assert_eq!(panel.items.len(), 2);
        assert_eq!(panel.items[0].label, "Alpha");
        assert_eq!(panel.items[1].command.as_deref(), Some("true"));
        assert!(panel.title.is_none());
    }

    #[test]
    fn non_item_json_leaves_panel_empty() {
        let panel = parse_panel("{}");
        assert!(panel.items.is_empty());
        assert!(panel.title.is_none());
    }

    #[test]
    fn width_ignored_when_unreasonable() {
        assert_eq!(parse_panel(r#"{"width": 0}"#).width, None);
        assert_eq!(parse_panel(r#"{"width": 99999}"#).width, None);
        assert_eq!(parse_panel(r#"{"width": 400}"#).width, Some(400));
    }

    #[test]
    fn giant_items_capped_via_text() {
        let big = "x".repeat(MAX_JSON_PARSE_BYTES + 1);
        let output = format!(r#"{{"items": ["{big}"]}}"#);
        // Oversized payloads are treated as plain text, so exactly one huge
        // label arrives but the panel survives.
        let panel = parse_panel(&output);
        assert!(panel.items.len() == 1);
    }

    #[test]
    fn items_cap_warns_and_truncates() {
        let items: Vec<String> = (0..MAX_ITEMS + 5).map(|_| "\"A\"".to_owned()).collect();
        let output = format!(r#"{{"items": [{}]}}"#, items.join(","));
        let panel = parse_panel(&output);
        assert_eq!(panel.items.len(), MAX_ITEMS);
    }
}
