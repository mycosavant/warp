use local_control::protocol::{ActionKind, ActionParameterSpec, TargetScope};
use serde_json::{Value, json};

use super::*;

fn parse(response: String) -> Value {
    serde_json::from_str(&response).expect("response is valid JSON")
}

#[test]
fn initialize_advertises_tools_capability() {
    let response = parse(handle_line(r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#).unwrap());
    assert_eq!(response["result"]["protocolVersion"], PROTOCOL_VERSION);
    assert!(response["result"]["capabilities"]["tools"].is_object());
    assert_eq!(response["result"]["serverInfo"]["name"], "warpctrl");
}

/// Notifications carry no `id`; answering one corrupts the stream.
#[test]
fn notifications_are_not_answered() {
    assert!(handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none());
}

#[test]
fn malformed_json_reports_a_parse_error() {
    let response = parse(handle_line("{not json").unwrap());
    assert_eq!(response["error"]["code"], -32700);
}

#[test]
fn unknown_method_reports_method_not_found() {
    let response =
        parse(handle_line(r#"{"jsonrpc":"2.0","id":7,"method":"resources/list"}"#).unwrap());
    assert_eq!(response["error"]["code"], -32601);
    assert_eq!(response["id"], 7);
}

#[test]
fn every_implemented_action_is_published_as_a_tool() {
    let tools = tool_definitions();
    assert_eq!(tools.len(), ActionKind::implemented_metadata().len());
    assert!(tools.iter().all(|tool| {
        tool["name"]
            .as_str()
            .is_some_and(|name| name.starts_with("warp_"))
            && tool["description"].as_str().is_some_and(|d| !d.is_empty())
            && tool["inputSchema"]["type"] == "object"
    }));
}

/// Tool names must round-trip, or a model's call cannot be routed back to an
/// action.
#[test]
fn tool_names_round_trip_and_are_unique() {
    let metadata = ActionKind::implemented_metadata();
    let mut names = std::collections::HashSet::new();
    for entry in &metadata {
        let name = tool_name(entry.kind);
        assert!(names.insert(name.clone()), "duplicate tool name {name}");
        assert!(
            name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "{name} must be an identifier-safe MCP tool name"
        );
        assert_eq!(action_for_tool_name(&name), Some(entry.kind));
    }
}

#[test]
fn unknown_tool_names_do_not_resolve() {
    assert_eq!(action_for_tool_name("warp_not_a_real_action"), None);
    assert_eq!(action_for_tool_name("tab.create"), None);
}

#[test]
fn dotted_action_names_become_underscored_tools() {
    assert_eq!(tool_name(ActionKind::TabCreate), "warp_tab_create");
    assert_eq!(
        tool_name(ActionKind::SurfaceSettingsOpen),
        "warp_surface_settings_open"
    );
}

/// `surface.list` rejects target selectors app-side, so advertising them would
/// invite guaranteed failures.
#[test]
fn only_hierarchy_scoped_actions_advertise_selectors() {
    for entry in ActionKind::implemented_metadata() {
        let schema = input_schema(&entry);
        let has_window = schema["properties"].get("window").is_some();
        assert_eq!(
            has_window,
            accepts_target(entry.target_scope),
            "{} selector exposure does not match its scope",
            entry.kind.as_str()
        );
    }
}

#[test]
fn selector_depth_widens_with_scope() {
    let window = input_schema(&metadata_for(ActionKind::WindowFocus));
    assert!(window["properties"].get("window").is_some());
    assert!(window["properties"].get("pane").is_none());

    let pane = input_schema(&metadata_for(ActionKind::PaneSplit));
    assert!(pane["properties"].get("window").is_some());
    assert!(pane["properties"].get("tab").is_some());
    assert!(pane["properties"].get("pane").is_some());
}

#[test]
fn every_action_accepts_an_instance_selector() {
    for entry in ActionKind::implemented_metadata() {
        let schema = input_schema(&entry);
        assert!(
            schema["properties"].get("instance").is_some(),
            "{} must accept an instance selector",
            entry.kind.as_str()
        );
    }
}

#[test]
fn text_actions_require_text_and_warn_about_newlines() {
    let schema = input_schema(&metadata_for(ActionKind::InputSubmit));
    assert_eq!(schema["required"], json!(["text"]));
    let description = schema["properties"]["text"]["description"]
        .as_str()
        .unwrap();
    assert!(description.contains("newlines"));
}

/// `input.submit` executes in the user's terminal; its description must say so,
/// or a model cannot weigh the action correctly. It must also explain the
/// queued case, since a queued command runs later and its output is not ready
/// when the call returns.
#[test]
fn submit_is_described_as_executing_and_explains_queueing() {
    let description = describe(&metadata_for(ActionKind::InputSubmit));
    assert!(description.contains("RUN it"));
    assert!(description.contains("queued"));
    let insert = describe(&metadata_for(ActionKind::InputInsert));
    assert!(insert.contains("WITHOUT running it"));
}

#[test]
fn parameterless_actions_require_nothing() {
    let schema = input_schema(&metadata_for(ActionKind::AppPing));
    assert_eq!(schema["required"], json!([]));
}

#[test]
fn enum_parameters_publish_their_variants() {
    let schema = input_schema(&metadata_for(ActionKind::TabClose));
    assert_eq!(
        schema["properties"]["mode"]["enum"],
        json!(["target", "active", "others", "right_of"])
    );
}

/// Every parameter spec must produce a schema; a missing arm would silently
/// publish a tool that cannot be called correctly.
#[test]
fn every_parameter_spec_is_mapped() {
    for entry in ActionKind::implemented_metadata() {
        let schema = input_schema(&entry);
        let properties = schema["properties"].as_object().unwrap();
        if entry.parameter_spec != ActionParameterSpec::None {
            let non_routing = properties
                .keys()
                .filter(|key| !ROUTING_KEYS.contains(&key.as_str()))
                .count();
            assert!(
                non_routing > 0,
                "{} has spec {:?} but publishes no parameters",
                entry.kind.as_str(),
                entry.parameter_spec
            );
        }
    }
}

#[test]
fn routing_arguments_are_not_forwarded_as_action_parameters() {
    // `instance` and the selectors route the request; only `text` is a param.
    let arguments = json!({ "text": "ls", "instance": "inst_x", "pane": "active" });
    let forwarded: Vec<&String> = arguments
        .as_object()
        .unwrap()
        .keys()
        .filter(|key| !ROUTING_KEYS.contains(&key.as_str()))
        .collect();
    assert_eq!(forwarded, vec!["text"]);
}

#[test]
fn target_selectors_are_parsed_from_arguments() {
    let target = target_from_arguments(&json!({ "window": "active", "tab_index": 2 }));
    assert!(matches!(
        target.window,
        Some(local_control::protocol::WindowTarget::Active)
    ));
    assert!(matches!(
        target.tab,
        Some(local_control::protocol::TabTarget::Index { index: 2 })
    ));
    assert!(target.pane.is_none());
}

#[test]
fn absent_selectors_produce_an_empty_target() {
    let target = target_from_arguments(&json!({ "text": "ls" }));
    assert!(target.window.is_none());
    assert!(target.tab.is_none());
    assert!(target.pane.is_none());
    assert!(target.session.is_none());
}

#[test]
fn calling_an_unknown_tool_is_reported_as_a_tool_error() {
    let response = parse(
        handle_line(
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"warp_nope"}}"#,
        )
        .unwrap(),
    );
    // A tool failure must be a result with isError, not a JSON-RPC error, so
    // the model can read it and adapt.
    assert!(response["error"].is_null());
    assert_eq!(response["result"]["isError"], true);
}

fn metadata_for(kind: ActionKind) -> local_control::protocol::ActionMetadata {
    ActionKind::implemented_metadata()
        .into_iter()
        .find(|entry| entry.kind == kind)
        .expect("action is implemented")
}

#[test]
fn scope_labels_cover_every_scope() {
    for scope in [
        TargetScope::Instance,
        TargetScope::Window,
        TargetScope::Tab,
        TargetScope::Pane,
        TargetScope::Session,
        TargetScope::Input,
        TargetScope::Settings,
        TargetScope::Appearance,
        TargetScope::Surface,
        TargetScope::File,
        TargetScope::Keybinding,
        TargetScope::Action,
        TargetScope::Capability,
    ] {
        assert!(!scope_label(scope).is_empty());
    }
}
