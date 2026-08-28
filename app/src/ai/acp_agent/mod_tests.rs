//! The half of the ACP path that can be decided without an agent.
//!
//! The exchange itself is verified by running it against a real agent — the
//! fork's standard, and the reason `warpctrl acp probe` exists (`.fork/TASKS.md`
//! T14.5).

use agent_client_protocol::schema::v1::{ToolCallUpdate, ToolCallUpdateFields};

use super::*;

fn request(options: Vec<PermissionOption>) -> RequestPermissionRequest {
    RequestPermissionRequest::new(
        "session-1",
        ToolCallUpdate::new("call_1", ToolCallUpdateFields::new().title("Write out.txt")),
        options,
    )
}

fn option(id: &'static str, name: &'static str, kind: PermissionOptionKind) -> PermissionOption {
    PermissionOption::new(id, name, kind)
}

/// The measured `opencode` list — **allow first**.
fn as_opencode_sent_it() -> Vec<PermissionOption> {
    vec![
        option("once", "Allow once", PermissionOptionKind::AllowOnce),
        option("always", "Always allow", PermissionOptionKind::AllowAlways),
        option("reject", "Reject", PermissionOptionKind::RejectOnce),
    ]
}

/// The measured `claude-agent-acp` list — **deny first**.
fn as_claude_sent_it() -> Vec<PermissionOption> {
    vec![
        option("reject", "Deny", PermissionOptionKind::RejectOnce),
        option("allow", "Allow Once", PermissionOptionKind::AllowOnce),
        option(
            "allow_always",
            "Always Allow",
            PermissionOptionKind::AllowAlways,
        ),
    ]
}

/// Two agents, opposite orders, the same answer. The reason this picks by kind
/// and never by position — `options.first()` would deny on one and **approve**
/// on the other, which is the T14.2 bug exactly.
#[test]
fn both_measured_agents_are_denied_by_selecting_their_reject_option() {
    for options in [as_opencode_sent_it(), as_claude_sent_it()] {
        let (outcome, _) = deny(&request(options));

        assert!(
            matches!(
                outcome,
                RequestPermissionOutcome::Selected(ref selected)
                    if selected.option_id.to_string() == "reject"
            ),
            "a denial must select the single-shot reject, wherever it sits in the list"
        );
    }
}

/// An agent that offers no way to say no is still answered no. `Cancelled` is a
/// refusal, and the `warpctrl` probe confirmed against a live agent that it is
/// treated as one.
#[test]
fn an_agent_offering_no_reject_is_still_refused() {
    let (outcome, _) = deny(&request(vec![option(
        "once",
        "Allow once",
        PermissionOptionKind::AllowOnce,
    )]));

    assert!(matches!(outcome, RequestPermissionOutcome::Cancelled));
}

/// **Nothing here may ever select an allow.** This is the property the whole
/// spike rests on: there is no surface that can show a person what saying yes
/// would permit, so nothing says yes. A future edit that "helpfully" picks an
/// `allow_once` when no reject is offered has to come through this test.
#[test]
fn no_option_that_permits_anything_is_ever_selected() {
    let permissive = vec![
        option("once", "Allow once", PermissionOptionKind::AllowOnce),
        option("always", "Always allow", PermissionOptionKind::AllowAlways),
    ];

    let (outcome, _) = deny(&request(permissive));

    assert!(
        matches!(outcome, RequestPermissionOutcome::Cancelled),
        "with only allows on offer the answer is still no"
    );
}

/// The person is told what was refused and why, in the conversation. A turn
/// where the agent silently could not act reads as the agent being broken.
#[test]
fn the_refusal_names_what_was_denied_and_says_it_was_warp() {
    let (_, note) = deny(&request(as_opencode_sent_it()));

    assert!(note.contains("Warp denied this"), "got: {note}");
    assert!(note.contains("Write out.txt"), "got: {note}");
}

/// A request with no title still produces a usable sentence rather than an
/// empty one — `ToolCallUpdateFields` is a delta type and every field is
/// optional.
#[test]
fn a_request_without_a_title_still_reads_as_a_sentence() {
    let bare = RequestPermissionRequest::new(
        "session-1",
        ToolCallUpdate::new("call_1", ToolCallUpdateFields::new()),
        as_opencode_sent_it(),
    );

    let (_, note) = deny(&bare);

    assert!(note.contains("a request to act"), "got: {note}");
}
