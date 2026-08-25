use serde_json::Value;

use super::*;
use crate::terminal::CLIAgent;
use crate::terminal::cli_agent_sessions::event::{CLIAgentEventPayload, CLIAgentEventType};

fn event(kind: CLIAgentEventType) -> CLIAgentEvent {
    CLIAgentEvent {
        v: 1,
        agent: CLIAgent::Claude,
        event: kind,
        session_id: Some("abc-123".to_string()),
        cwd: Some("/home/u/p".to_string()),
        project: Some("p".to_string()),
        payload: CLIAgentEventPayload::default(),
        source: CLIAgentEventSource::RichPlugin,
    }
}

/// Renders through the same adapter `record_cli_agent` uses, so world 2's
/// mapping is covered rather than only the writer beneath it.
fn parsed(event: &CLIAgentEvent, applied: bool) -> Value {
    let entry = hosted_agent_entry(event, applied);
    serde_json::from_str(&line(7, "2026-08-24T12:00:00.000Z".to_string(), entry))
        .expect("a record must be valid JSON")
}

/// The line is flat and self-describing, because every reader of this file is a
/// filter and `jq 'select(.event=="…")'` should not have to know the shape.
#[test]
fn a_record_is_one_flat_json_object() {
    let record = parsed(&event(CLIAgentEventType::ToolComplete), true);

    assert_eq!(record["seq"], 7);
    assert_eq!(record["ts"], "2026-08-24T12:00:00.000Z");
    assert_eq!(record["v"], 1);
    assert_eq!(record["agent"], "claude");
    assert_eq!(record["event"], "tool_complete");
    assert_eq!(record["source"], "rich_plugin");
    assert_eq!(record["session_id"], "abc-123");
    assert_eq!(record["applied"], true);
    assert!(
        record
            .as_object()
            .expect("an object")
            .values()
            .all(|v| !v.is_object()),
        "no field may nest: the record is read by filters, not parsers"
    );
}

/// Absent detail is absent, not `null` — otherwise every line carries a dozen
/// nulls and the interesting fields stop being visible by eye.
#[test]
fn fields_with_nothing_to_say_are_omitted() {
    let record = parsed(&event(CLIAgentEventType::Stop), true);

    assert!(record.get("tool_name").is_none());
    assert!(record.get("error_type").is_none());
    assert!(record.get("summary").is_none());
}

/// The case the log exists for: an event that arrived and was thrown away.
#[test]
fn a_dropped_event_is_recorded_as_dropped() {
    let record = parsed(&event(CLIAgentEventType::PermissionRequest), false);

    assert_eq!(record["event"], "permission_request");
    assert_eq!(
        record["applied"], false,
        "an event with no session is the silent failure this log is for"
    );
}

/// An event from a newer plugin than this build understands still reads as
/// itself, rather than as "unknown".
#[test]
fn an_unrecognized_event_keeps_its_own_name() {
    let record = parsed(
        &event(CLIAgentEventType::Unknown("subagent_spawned".to_string())),
        true,
    );

    assert_eq!(record["event"], "subagent_spawned");
}

/// The agent supplies `session_id` and it becomes a filename, so it is not a
/// filename until it has been made one.
#[test]
fn a_session_id_cannot_choose_where_warp_writes() {
    let dir = Path::new("/state/events");

    for hostile in [
        "../../.bashrc",
        "/etc/passwd",
        "..",
        "....//....//x",
        "a/b/c",
    ] {
        let path = path_for(dir, Some(hostile));
        assert_eq!(
            path.parent(),
            Some(dir),
            "{hostile:?} escaped the log directory as {}",
            path.display()
        );
        assert!(
            !path.to_string_lossy().contains(".."),
            "{hostile:?} kept a traversal segment"
        );
    }
}

#[test]
fn a_session_id_that_sanitizes_to_nothing_falls_back() {
    assert_eq!(session_key(None), UNKEYED);
    assert_eq!(session_key(Some("")), UNKEYED);
    assert_eq!(session_key(Some("...")), UNKEYED);
    assert_eq!(session_key(Some("///")), "___");
}

#[test]
fn an_ordinary_session_id_survives_intact() {
    assert_eq!(session_key(Some("abc-123.def_4")), "abc-123.def_4");
}

#[test]
fn a_session_id_cannot_be_arbitrarily_long() {
    let key = session_key(Some(&"a".repeat(4096)));
    assert_eq!(key.len(), MAX_KEY_LEN);
}
