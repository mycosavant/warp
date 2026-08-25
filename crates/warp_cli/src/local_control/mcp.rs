//! An MCP server exposing the local-control action catalog as tools.
//!
//! Turns `warpctrl` into something an agent can call directly: every
//! implemented catalog action becomes one MCP tool, so Claude Code (or any MCP
//! client) can drive windows, tabs, panes, sessions and the input buffer of a
//! running Warp instance.
//!
//! Tools are **generated from the catalog**, not hardcoded. Adding an action to
//! `catalog.rs` publishes a tool here with no work, and an action that is only
//! a `Stub` upstream is never advertised.
//!
//! # Why this is written by hand
//!
//! MCP over stdio is newline-delimited JSON-RPC 2.0, and the local-control
//! client is entirely blocking (`reqwest::blocking`). A synchronous read-eval
//! loop over stdin is therefore the whole implementation, and it needs no MCP
//! framework and no async runtime — `serde_json` was already a dependency.
//!
//! Input schemas are also written by hand rather than derived from the
//! parameter structs. A derived schema describes Rust; these descriptions are
//! read by a model deciding which tool to call, and are the difference between
//! an agent that targets the right pane and one that guesses.

use std::io::{BufRead as _, Write as _};

use local_control::protocol::{
    Action, ActionKind, ActionMetadata, ActionParameterSpec, ControlError, ControlResponse,
    RequestEnvelope, TargetScope, TargetSelector,
};
use local_control::selection::InstanceSelector;
use serde_json::{Value, json};

/// The MCP revision this server implements.
const PROTOCOL_VERSION: &str = "2024-11-05";

/// Runs the stdio MCP server until stdin closes, then exits the process.
pub fn run_and_exit() -> ! {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut line = String::new();
    loop {
        line.clear();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => std::process::exit(0),
            Ok(_) => {}
            Err(err) => {
                // stderr is free for diagnostics; stdout carries the protocol.
                eprintln!("warpctrl mcp: failed to read stdin: {err}");
                std::process::exit(1);
            }
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some(response) = handle_line(trimmed) else {
            continue;
        };
        if writeln!(stdout, "{response}")
            .and_then(|()| stdout.flush())
            .is_err()
        {
            std::process::exit(0);
        }
    }
}

/// Handles one JSON-RPC message, returning a response when one is owed.
///
/// Notifications carry no `id` and must not be answered, so they map to
/// `None` rather than to an empty response.
fn handle_line(line: &str) -> Option<String> {
    let request: Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(err) => {
            return Some(error_response(
                Value::Null,
                -32700,
                &format!("parse error: {err}"),
            ));
        }
    };
    let id = request.get("id").cloned();
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let params = request.get("params").cloned().unwrap_or(Value::Null);

    // Notifications have no id and expect no reply.
    let id = id?;

    match method {
        "initialize" => Some(result_response(id, initialize_result())),
        "ping" => Some(result_response(id, json!({}))),
        "tools/list" => Some(result_response(id, json!({ "tools": tool_definitions() }))),
        "tools/call" => Some(match call_tool(&params) {
            Ok(value) => result_response(id, value),
            // Tool failures are reported as a result with `isError`, not as a
            // JSON-RPC error: the model needs to read the message and adapt,
            // and a transport-level error would not reach it.
            Err(message) => result_response(
                id,
                json!({
                    "content": [{ "type": "text", "text": message }],
                    "isError": true,
                }),
            ),
        }),
        _ => Some(error_response(
            id,
            -32601,
            &format!("method not found: {method}"),
        )),
    }
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "warpctrl", "version": env!("CARGO_PKG_VERSION") },
        "instructions": "Controls a running Warp terminal. Call warp_instance_list \
    first to confirm an instance is reachable. Mutating actions need a focused \
    window that has a workspace; if warp_window_list reports is_active false, call \
    warp_app_focus before mutating. Use warp_input_submit to run a command and \
    warp_input_insert to stage text without running it.",
    })
}

/// MCP tool name for a catalog action: `tab.create` -> `warp_tab_create`.
///
/// The `warp_` prefix keeps these distinguishable in a client that has many
/// servers connected at once.
fn tool_name(action: ActionKind) -> String {
    format!("warp_{}", action.as_str().replace('.', "_"))
}

fn action_for_tool_name(name: &str) -> Option<ActionKind> {
    ActionKind::implemented_metadata()
        .into_iter()
        .find(|metadata| tool_name(metadata.kind) == name)
        .map(|metadata| metadata.kind)
}

/// One tool per implemented catalog action.
fn tool_definitions() -> Vec<Value> {
    ActionKind::implemented_metadata()
        .into_iter()
        .map(|metadata| {
            json!({
                "name": tool_name(metadata.kind),
                "description": describe(&metadata),
                "inputSchema": input_schema(&metadata),
            })
        })
        .collect()
}

/// Human-readable description handed to the model.
fn describe(metadata: &ActionMetadata) -> String {
    let mut description = match metadata.kind.as_str() {
        // Actions whose behaviour or hazards are not obvious from the name.
        "input.insert" => "Insert text at the cursor of the target pane's input \
buffer WITHOUT running it. Existing buffer content is kept."
            .to_string(),
        "input.replace" => "Replace the target pane's entire input buffer WITHOUT \
running it."
            .to_string(),
        "input.submit" => "Replace the target pane's input buffer with this text \
and RUN it. This executes a command in the user's terminal. Rejects newlines \
and control characters, so exactly one command runs per call. The result \
reports `executed: true` when it ran immediately, or `queued: true` when the \
pane's shell is still starting or busy — a queued command runs as soon as the \
pane is ready, so wait before reading its output."
            .to_string(),
        "app.active" => "Report the active window/tab/pane/session chain.".to_string(),
        "app.focus" => "Focus the Warp app. Required before mutating actions when \
the target window is not active."
            .to_string(),
        "instance.list" => "List running Warp instances that have local control \
enabled and are reachable. Start here."
            .to_string(),
        "window.list" => "List windows. `has_workspace` false means the window \
cannot accept tab or pane mutations yet."
            .to_string(),
        "setting.set" => "Set a Warp setting by dotted key. Use warp_setting_list \
to discover keys and their current values first."
            .to_string(),
        "drive.sync.status" => "Report where Warp Drive would be mirrored on disk \
and how many objects would go there, WITHOUT writing anything, plus any \
mirrored files left half-merged by git. Run this before \
warp_drive_sync_export to check the destination."
            .to_string(),
        "drive.sync.export" => "Write the whole of Warp Drive into the directory \
set by `warp_drive.local_sync.path`, for the user to keep under git. Warp never \
runs git itself. This PRUNES: files in that directory that Warp wrote and that \
no longer correspond to an object are deleted, and directories are removed once \
empty. Files Warp did not write are never touched. The destination comes from \
settings and cannot be passed in. Refuses, writing nothing at all, if a file it \
would overwrite still has git conflict markers in it. A workflow's aliases \
travel inside its own file."
            .to_string(),
        "drive.sync.import" => "Read the directory set by \
`warp_drive.local_sync.path` back into Warp Drive, after the user has pulled. \
The FILES WIN: an object is overwritten by its file, and an object whose file \
is gone is moved to the trash — recoverable from the Warp Drive panel, but a \
visible change to the user's data. Refuses a tree with no Warp Drive objects \
in it, since that would read as \"everything was deleted\". Also refuses while \
any mirrored file still has git conflict markers in it: DO NOT resolve those \
yourself, report them to the user, because the two versions are theirs to \
choose between. Run warp_drive_sync_status first."
            .to_string(),
        other => format!("Warp local control action `{other}`."),
    };
    description.push_str(&format!(
        " (scope: {}, returns: {})",
        scope_label(metadata.target_scope),
        serde_json::to_value(metadata.result_spec)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| "result".to_owned()),
    ));
    description
}

fn scope_label(scope: TargetScope) -> &'static str {
    match scope {
        TargetScope::Instance => "instance",
        TargetScope::Window => "window",
        TargetScope::Tab => "tab",
        TargetScope::Pane => "pane",
        TargetScope::Session => "session",
        TargetScope::Input => "input",
        TargetScope::Settings => "settings",
        TargetScope::Appearance => "appearance",
        TargetScope::Surface => "surface",
        TargetScope::File => "file",
        TargetScope::Keybinding => "keybinding",
        TargetScope::Action => "action",
        TargetScope::Capability => "capability",
        TargetScope::Drive => "drive",
        TargetScope::Agent => "agent",
        TargetScope::Slash => "slash",
        TargetScope::Events => "events",
    }
}

/// Whether an action accepts window/tab/pane/session selectors.
///
/// Only hierarchy-scoped actions do. Advertising selectors on the rest would
/// invite a model to send targets the app rejects — `surface.list` explicitly
/// refuses them.
fn accepts_target(scope: TargetScope) -> bool {
    matches!(
        scope,
        TargetScope::Window
            | TargetScope::Tab
            | TargetScope::Pane
            | TargetScope::Session
            | TargetScope::Input
    )
}

fn input_schema(metadata: &ActionMetadata) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required: Vec<Value> = Vec::new();

    add_parameter_properties(metadata.parameter_spec, &mut properties, &mut required);

    properties.insert(
        "instance".to_owned(),
        json!({
            "type": "string",
            "description": "Instance id from warp_instance_list. Omit to use the active instance.",
        }),
    );

    if accepts_target(metadata.target_scope) {
        add_target_properties(metadata.target_scope, &mut properties);
    }

    json!({
        "type": "object",
        "properties": Value::Object(properties),
        "required": required,
        "additionalProperties": false,
    })
}

/// Selector properties, widened as the scope deepens.
fn add_target_properties(scope: TargetScope, properties: &mut serde_json::Map<String, Value>) {
    let depth_includes = |target: TargetScope| -> bool {
        let rank = |scope: TargetScope| match scope {
            TargetScope::Window => 1,
            TargetScope::Tab => 2,
            TargetScope::Pane | TargetScope::Input => 3,
            TargetScope::Session => 4,
            _ => 0,
        };
        rank(target) <= rank(scope)
    };

    if depth_includes(TargetScope::Window) {
        properties.insert(
            "window".to_owned(),
            json!({
                "type": "string",
                "description": "Window id from warp_window_list, or \"active\".",
            }),
        );
        properties.insert(
            "window_index".to_owned(),
            json!({ "type": "integer", "description": "Zero-based window index." }),
        );
    }
    if depth_includes(TargetScope::Tab) {
        properties.insert(
            "tab".to_owned(),
            json!({
                "type": "string",
                "description": "Tab id from warp_tab_list, or \"active\".",
            }),
        );
        properties.insert(
            "tab_index".to_owned(),
            json!({ "type": "integer", "description": "Zero-based tab index." }),
        );
    }
    if depth_includes(TargetScope::Pane) {
        properties.insert(
            "pane".to_owned(),
            json!({
                "type": "string",
                "description": "Pane id from warp_pane_list, or \"active\".",
            }),
        );
        properties.insert(
            "pane_index".to_owned(),
            json!({ "type": "integer", "description": "Zero-based pane index." }),
        );
    }
    if depth_includes(TargetScope::Session) {
        properties.insert(
            "session".to_owned(),
            json!({
                "type": "string",
                "description": "Session id from warp_session_list, or \"active\".",
            }),
        );
    }
}

/// Schema fragment for each catalog parameter spec.
///
/// Mirrors the structs in `local_control::protocol`; a mismatch surfaces as a
/// `deny_unknown_fields` rejection from the app rather than silent misbehaviour.
fn add_parameter_properties(
    spec: ActionParameterSpec,
    properties: &mut serde_json::Map<String, Value>,
    required: &mut Vec<Value>,
) {
    let mut require = |name: &str, schema: Value| {
        properties.insert(name.to_owned(), schema);
        required.push(json!(name));
    };

    match spec {
        ActionParameterSpec::None => {}
        ActionParameterSpec::ActionName => require(
            "action",
            json!({ "type": "string", "description": "Canonical action name, e.g. \"tab.create\"." }),
        ),
        ActionParameterSpec::BindingName => require(
            "binding_name",
            json!({ "type": "string", "description": "Keybinding name." }),
        ),
        ActionParameterSpec::BooleanValue => require(
            "value",
            json!({ "type": "boolean", "description": "New boolean value." }),
        ),
        ActionParameterSpec::ColorValue => require(
            "color",
            json!({ "type": "string", "description": "Tab color name or hex value." }),
        ),
        ActionParameterSpec::Direction => require("direction", direction_schema()),
        ActionParameterSpec::DriveObjectList => {
            properties.insert(
                "include_trashed".to_owned(),
                json!({ "type": "boolean", "description": "Include objects in the trash." }),
            );
            properties.insert(
                "object_type".to_owned(),
                json!({
                    "type": "string",
                    "description": "Only this type: \"workflow\", \"notebook\", \"folder\", \"prompt\" or \"env-vars\".",
                }),
            );
        }
        ActionParameterSpec::RemoteWslConnect => {
            properties.insert(
                "distro".to_owned(),
                json!({
                    "type": "string",
                    "description": "WSL distribution name, from remote.wsl.list. Defaults to the target pane's own distribution when it is already running a WSL shell.",
                }),
            );
        }
        ActionParameterSpec::DriveObjectGet => require(
            "id",
            json!({ "type": "string", "description": "Object id, from drive.object.list." }),
        ),
        ActionParameterSpec::DriveObjectCreate => {
            require(
                "object_type",
                json!({
                    "type": "string",
                    "description": "\"workflow\", \"notebook\", \"folder\", \"prompt\" or \"env-vars\".",
                }),
            );
            require(
                "name",
                json!({ "type": "string", "description": "Display name." }),
            );
            properties.insert(
                "body".to_owned(),
                json!({
                    "type": "string",
                    "description": "Markdown for a notebook, JSON otherwise, omitted for a folder. Call drive.object.get on an object of the same type to see the shape.",
                }),
            );
            properties.insert(
                "folder".to_owned(),
                json!({ "type": "string", "description": "Id of the folder to create it in." }),
            );
        }
        ActionParameterSpec::DriveObjectTrash => require(
            "id",
            json!({ "type": "string", "description": "Object id, from drive.object.list." }),
        ),
        ActionParameterSpec::FileOpen => {
            require(
                "path",
                json!({ "type": "string", "description": "Absolute path to open." }),
            );
            properties.insert(
                "line".to_owned(),
                json!({ "type": "integer", "description": "One-based line to reveal." }),
            );
            properties.insert(
                "column".to_owned(),
                json!({ "type": "integer", "description": "One-based column to reveal." }),
            );
            properties.insert(
                "new_tab".to_owned(),
                json!({ "type": "boolean", "description": "Open in a new tab." }),
            );
        }
        ActionParameterSpec::Key => require(
            "key",
            json!({ "type": "string", "description": "Dotted setting key, e.g. \"appearance.text.font_size\"." }),
        ),
        ActionParameterSpec::KeyValue => {
            require(
                "key",
                json!({ "type": "string", "description": "Dotted setting key." }),
            );
            require(
                "value",
                json!({ "description": "New value, typed to match the setting." }),
            );
        }
        ActionParameterSpec::Namespace => {
            properties.insert(
                "namespace".to_owned(),
                json!({
                    "type": "string",
                    "description": "Restrict to a dotted key prefix, e.g. \"appearance\".",
                }),
            );
        }
        ActionParameterSpec::PageQuery => {
            properties.insert(
                "page".to_owned(),
                json!({ "type": "string", "description": "Settings page to open." }),
            );
            properties.insert(
                "query".to_owned(),
                json!({ "type": "string", "description": "Search query to seed." }),
            );
        }
        ActionParameterSpec::Query => {
            properties.insert(
                "query".to_owned(),
                json!({ "type": "string", "description": "Search query to seed." }),
            );
        }
        ActionParameterSpec::Rename => require(
            "title",
            json!({ "type": "string", "description": "New title." }),
        ),
        ActionParameterSpec::Resize => {
            require("direction", direction_schema());
            properties.insert(
                "amount".to_owned(),
                json!({ "type": "integer", "description": "Resize step count." }),
            );
        }
        ActionParameterSpec::TabActivate => require(
            "mode",
            json!({
                "type": "string",
                "enum": ["target", "previous", "next", "last"],
                "description": "Which tab to activate. \"target\" uses the tab selector.",
            }),
        ),
        ActionParameterSpec::TabClose => require(
            "mode",
            json!({
                "type": "string",
                "enum": ["target", "active", "others", "right_of"],
                "description": "Which tabs to close.",
            }),
        ),
        ActionParameterSpec::TabCreate => {
            properties.insert(
                "tab_type".to_owned(),
                json!({
                    "type": "string",
                    "enum": ["terminal", "agent", "cloud_agent", "default"],
                    "description": "Kind of tab to create.",
                }),
            );
        }
        ActionParameterSpec::Text => require(
            "text",
            json!({
                "type": "string",
                "description": "Text for the input buffer. Must not contain newlines or control characters.",
            }),
        ),
        ActionParameterSpec::ThemeName => require(
            "theme_name",
            json!({ "type": "string", "description": "Theme name from warp_theme_list." }),
        ),
        ActionParameterSpec::AgentPrompt => {
            require(
                "prompt",
                json!({
                    "type": "string",
                    "description": "Prompt to send to the agent. Newlines are allowed, unlike input text.",
                }),
            );
            properties.insert(
                "conversation_id".to_owned(),
                json!({
                    "type": "string",
                    "description": "Conversation to continue, from warp_agent_list. Omit to start a new one.",
                }),
            );
        }
        ActionParameterSpec::AgentRead => {
            require(
                "conversation_id",
                json!({
                    "type": "string",
                    "description": "Conversation to read, from warp_agent_list.",
                }),
            );
            properties.insert(
                "last".to_owned(),
                json!({
                    "type": "integer",
                    "description": "Return only the last N exchanges. Omit for the whole transcript.",
                }),
            );
            properties.insert(
                "include_tool_results".to_owned(),
                json!({
                    "type": "boolean",
                    "description": "Include tool-call results — every file read and command output — in the returned text.",
                }),
            );
        }
        ActionParameterSpec::AgentSpawn => {
            require(
                "prompt",
                json!({
                    "type": "string",
                    "description": "Self-contained prompt for the child agent. It does not inherit the parent's transcript, so include everything it needs.",
                }),
            );
            properties.insert(
                "name".to_owned(),
                json!({
                    "type": "string",
                    "description": "Name for the child, shown on its pill.",
                }),
            );
            properties.insert(
                "parent_conversation_id".to_owned(),
                json!({
                    "type": "string",
                    "description": "Conversation to parent it to. Defaults to the one in front of the targeted pane.",
                }),
            );
            properties.insert(
                "allow_tools".to_owned(),
                json!({
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Tools the child may use: the preset read-only, or ToolType names such as READ_FILES or RUN_SHELL_COMMAND. Omit for no restriction; an empty list means no tools.",
                }),
            );
        }
        ActionParameterSpec::AgentCancel => require(
            "conversation_id",
            json!({
                "type": "string",
                "description": "Conversation whose turn should be stopped, from warp_agent_list.",
            }),
        ),
        ActionParameterSpec::AgentSettle => {
            require(
                "conversation_id",
                json!({
                    "type": "string",
                    "description": "Conversation to settle or unsettle, from warp_agent_list.",
                }),
            );
            properties.insert(
                "settled".to_owned(),
                json!({
                    "type": "boolean",
                    "description": "True to settle the thread (default), false to bring it back.",
                }),
            );
        }
        ActionParameterSpec::AgentReveal => {
            require(
                "conversation_id",
                json!({
                    "type": "string",
                    "description": "Background child agent conversation to put on screen, from warp_agent_list.",
                }),
            );
            properties.insert(
                "target".to_owned(),
                json!({
                    "type": "string",
                    "enum": ["pane", "tab", "swap"],
                    "description": "Where to put it: split off beside its parent (default), a new tab, or swapped into the targeted pane.",
                }),
            );
        }
        ActionParameterSpec::SlashRun => {
            require(
                "command",
                json!({
                    "type": "string",
                    "description": "Slash command name from warp_slash_list, with or without the leading slash.",
                }),
            );
            properties.insert(
                "argument".to_owned(),
                json!({
                    "type": "string",
                    "description": "Argument for commands that take one, such as the instructions to /compact-and.",
                }),
            );
            properties.insert(
                "force".to_owned(),
                json!({
                    "type": "boolean",
                    "description": "Run a command outside the orchestration allowlist. Refused without this.",
                }),
            );
        }
    }
}

fn direction_schema() -> Value {
    json!({
        "type": "string",
        "enum": ["left", "right", "up", "down", "previous", "next"],
        "description": "Layout direction.",
    })
}

/// Property names consumed as routing rather than forwarded as action params.
const ROUTING_KEYS: &[&str] = &[
    "instance",
    "window",
    "window_index",
    "tab",
    "tab_index",
    "pane",
    "pane_index",
    "session",
];

fn call_tool(params: &Value) -> Result<Value, String> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| "tools/call requires a tool name".to_owned())?;
    let action = action_for_tool_name(name).ok_or_else(|| format!("unknown tool: {name}"))?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let instance_selector = match arguments.get("instance").and_then(Value::as_str) {
        Some(id) => InstanceSelector::Id(local_control::discovery::InstanceId(id.to_owned())),
        None => InstanceSelector::Active,
    };
    let target = target_from_arguments(&arguments);

    // Everything that is not routing is an action parameter. The app validates
    // the shape with `deny_unknown_fields`, so a wrong key fails loudly there
    // rather than being silently dropped here.
    let mut action_params = serde_json::Map::new();
    if let Some(object) = arguments.as_object() {
        for (key, value) in object {
            if !ROUTING_KEYS.contains(&key.as_str()) {
                action_params.insert(key.clone(), value.clone());
            }
        }
    }

    let records = local_control::discovery::list_instances(
        &warp_core::channel::ChannelState::channel().to_string(),
    );
    let instance = local_control::selection::select_instance(&records, &instance_selector)
        .map_err(describe_error)?;

    let mut request = RequestEnvelope::new(
        Action::with_params(action, Value::Object(action_params)).map_err(describe_error)?,
    );
    request.target = target;

    let response =
        local_control::client::send_request(&instance, &request).map_err(describe_error)?;
    let ControlResponse::Ok { data } = response.response else {
        return Err("local-control request failed without an error payload".to_owned());
    };

    let text =
        serde_json::to_string_pretty(&data).unwrap_or_else(|_| "action completed".to_owned());
    Ok(json!({ "content": [{ "type": "text", "text": text }] }))
}

/// Renders a `ControlError` for a model rather than for a terminal.
///
/// The error code is retained because it is the actionable part: `missing_target`
/// means focus a window first, `local_control_disabled` means the user must
/// enable Scripting.
fn describe_error(error: ControlError) -> String {
    let code = serde_json::to_value(error.code)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "error".to_owned());
    let mut message = format!("{code}: {}", error.message);
    if let Some(details) = &error.details {
        message.push_str(&format!(" ({details})"));
    }
    message
}

fn target_from_arguments(arguments: &Value) -> TargetSelector {
    use local_control::protocol::{
        PaneSelector, PaneTarget, SessionSelector, SessionTarget, TabSelector, TabTarget,
        WindowSelector, WindowTarget,
    };

    let string = |key: &str| arguments.get(key).and_then(Value::as_str);
    let index = |key: &str| {
        arguments
            .get(key)
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
    };

    let window = match string("window") {
        Some("active") => Some(WindowTarget::Active),
        Some(id) => Some(WindowTarget::Id {
            id: WindowSelector(id.to_owned()),
        }),
        None => index("window_index").map(|index| WindowTarget::Index { index }),
    };
    let tab = match string("tab") {
        Some("active") => Some(TabTarget::Active),
        Some(id) => Some(TabTarget::Id {
            id: TabSelector(id.to_owned()),
        }),
        None => index("tab_index").map(|index| TabTarget::Index { index }),
    };
    let pane = match string("pane") {
        Some("active") => Some(PaneTarget::Active),
        Some(id) => Some(PaneTarget::Id {
            id: PaneSelector(id.to_owned()),
        }),
        None => index("pane_index").map(|index| PaneTarget::Index { index }),
    };
    let session = match string("session") {
        Some("active") => Some(SessionTarget::Active),
        Some(id) => Some(SessionTarget::Id {
            id: SessionSelector(id.to_owned()),
        }),
        None => None,
    };

    TargetSelector {
        window,
        tab,
        pane,
        session,
    }
}

fn result_response(id: Value, result: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

fn error_response(id: Value, code: i32, message: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
    .to_string()
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
