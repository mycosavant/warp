use super::*;
use crate::event_log::MAX_TEXT_LEN;

fn started(name: &str, preview: Option<&str>) -> ToolEvent {
    ToolEvent::Started {
        call_id: "toolu_01QQ7Nn3ZtF8tjGwhmWf7K1Y".to_owned(),
        parent_call_id: None,
        name: name.to_owned(),
        input_preview: preview.map(str::to_owned),
    }
}

fn completed(name: Option<&str>, failed: bool) -> ToolEvent {
    ToolEvent::Completed {
        call_id: "toolu_01QQ7Nn3ZtF8tjGwhmWf7K1Y".to_owned(),
        parent_call_id: None,
        name: name.map(str::to_owned),
        failed,
    }
}

#[test]
fn a_started_call_is_a_tool_start_carrying_what_ran() {
    let projected = project(&started("Bash", Some("cargo test -p warp")));

    assert_eq!(projected.event, "tool_start");
    assert_eq!(projected.tool_name.as_deref(), Some("Bash"));
    assert_eq!(
        projected.tool_input_preview.as_deref(),
        Some("cargo test -p warp")
    );
    assert_eq!(projected.error_type, None);
}

/// The join that makes the log answerable: `tool_start` and `tool_complete`
/// share Claude's `tool_use.id`, so a call that starts and never returns is a
/// `tool_start` with no partner. That is the failure this phase exists to catch.
#[test]
fn both_halves_of_one_call_carry_the_same_id() {
    let start = project(&started("Read", None));
    let end = project(&completed(Some("Read"), false));

    assert_eq!(start.call_id, end.call_id);
    assert_eq!(end.event, "tool_complete");
}

/// Claude in `--print` mode does not report a refusal as its own event: a denied
/// tool comes back as an ordinary `tool_result` with `is_error`, which is
/// indistinguishable on the wire from a tool that ran and failed. Both are
/// recorded as what the stream actually said.
#[test]
fn a_failed_call_says_so_without_claiming_to_know_why() {
    let projected = project(&completed(Some("Read"), true));

    assert_eq!(projected.event, "tool_complete");
    assert_eq!(projected.error_type, Some("error"));
}

/// `None` is a real state, not a missing case: a turn resumed part-way through a
/// call has a result whose `tool_use` was never on this stream. Recorded without
/// a name rather than dropped, so the orphan is visible.
#[test]
fn a_result_with_no_matching_call_is_recorded_anyway() {
    let projected = project(&completed(None, false));

    assert_eq!(projected.event, "tool_complete");
    assert_eq!(projected.tool_name, None);
    assert!(!projected.call_id.is_empty());
}

/// The field is shared with two other sources and read by grepping for what ran,
/// so it truncates on the same rule they do.
#[test]
fn a_long_command_is_excerpted_like_every_other_source() {
    let command = "x".repeat(MAX_TEXT_LEN * 3);
    let projected = project(&started("Bash", Some(&command)));
    let preview = projected.tool_input_preview.expect("a preview");

    assert_eq!(preview.chars().count(), MAX_TEXT_LEN + 1);
    assert!(preview.ends_with('…'));
}

/// A `tool_complete` describes an ending, and repeating the command on it would
/// double every command in the file for no new information — the `call_id` join
/// already reaches the `tool_start` that has it.
#[test]
fn a_completion_does_not_repeat_the_input() {
    assert_eq!(
        project(&completed(Some("Bash"), false)).tool_input_preview,
        None
    );
}

/// Filing under Claude's session id instead would put a turn's tools in a
/// different file from the frame `warp_agent` writes for the same turn, which is
/// the entire reason `RequestParams` had to start carrying this.
#[test]
fn the_context_files_events_under_warps_conversation_id() {
    let context = TurnContext::new(
        "af35bf30-0000-0000-0000-000000000000".to_owned(),
        Some("/home/effatha/git/warp".to_owned()),
    );

    assert_eq!(context.session_id, "af35bf30-0000-0000-0000-000000000000");
    assert_eq!(context.cwd.as_deref(), Some("/home/effatha/git/warp"));
    assert_eq!(
        context.cwd.as_deref().and_then(project_name),
        Some("warp"),
        "the log's `project` field is the working directory's last component"
    );
}

/// Nesting is reported, not inferred. Interleaving alone stops distinguishing
/// two concurrent subagents' tools, and concurrent subagents are the case.
#[test]
fn a_subagents_tool_names_the_call_that_spawned_it() {
    let nested = ToolEvent::Started {
        call_id: "toolu_inner".to_owned(),
        parent_call_id: Some("toolu_outer".to_owned()),
        name: "Read".to_owned(),
        input_preview: None,
    };

    assert_eq!(
        project(&nested).parent_call_id.as_deref(),
        Some("toolu_outer")
    );
    assert_eq!(
        project(&started("Agent", None)).parent_call_id,
        None,
        "the spawning call itself has no parent"
    );
}
