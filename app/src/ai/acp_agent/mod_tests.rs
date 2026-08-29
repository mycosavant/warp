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

/// A second turn is refused rather than answered by an agent that remembers
/// nothing. Measured: it answered *"I haven't written to or modified any files
/// yet in this session"* directly below the turn where it wrote one.
///
/// The predicate is here rather than in `handles` on purpose — `handles` says
/// "this path serves user queries", which is still true, and a request it
/// declines would fall through to an implementation that would answer it wrongly
/// in a different way.
#[test]
fn a_conversation_that_already_has_a_session_is_refused_with_a_reason() {
    let error = continuation_error();

    assert!(
        error.contains("cannot continue"),
        "the refusal must say what it is refusing, got: {error}"
    );
    assert!(
        error.contains("Start a new conversation"),
        "the refusal must say what to do instead, got: {error}"
    );
}

/// The refusal text is the only thing a person sees, so it must not name a
/// mechanism they cannot act on.
#[test]
fn the_continuation_refusal_explains_itself_without_protocol_jargon() {
    let error = continuation_error();

    for jargon in ["session/load", "loadSession", "ACP", "conversation_token"] {
        assert!(
            !error.contains(jargon),
            "{jargon} means nothing to a person reading a conversation, got: {error}"
        );
    }
}

/// The sentence that actually ships.
///
/// `Turn::from_request` cannot be called from here — `RequestParams` has a
/// private field, which is the whole reason `Turn` exists — so these pin the
/// constant it returns rather than a copy of it. A test that retyped the message
/// would pass while the shipped one said anything at all.
fn continuation_error() -> String {
    CANNOT_CONTINUE.to_owned()
}

/// An agent that is not on `PATH` produces a sentence naming the variable, the
/// command and `PATH` — not a crate line number and an errno.
///
/// Measured before this existed: the raw error was
/// `Internal error: {"spawned_at": "…/jsonrpc.rs:1732:39", "data": "No such file
/// or directory (os error 2)"}`, and the app dropped even that.
#[test]
fn an_agent_that_is_not_on_path_is_reported_as_being_not_on_path() {
    let error = spawn_failure_or(
        "Internal error: {\"spawned_at\": \"jsonrpc.rs:1732:39\", \
         \"data\": \"No such file or directory (os error 2)\"}",
        "opencode acp",
    )
    .to_string();

    assert!(error.contains("WARP_FORK_ACP_COMMAND"), "got: {error}");
    assert!(error.contains("opencode acp"), "got: {error}");
    assert!(error.contains("PATH"), "got: {error}");
}

/// …and every *other* failure keeps the generic wording, because guessing `PATH`
/// at an error that has nothing to do with it would send a person to the wrong
/// place. The rule that earned this test is T14.4's: a fix shaped like the hole
/// it fixes.
#[test]
fn a_failure_that_is_not_a_missing_file_does_not_blame_path() {
    let error = spawn_failure_or("connection closed before initialize", "opencode acp").to_string();

    assert!(!error.contains("PATH"), "got: {error}");
    assert!(error.contains("connection closed"), "got: {error}");
}

fn parked(acts_on: Vec<String>) -> registry::ParkedRequest {
    parked_request(
        &agent_client_protocol::schema::v1::RequestId::from(0),
        "turn-1",
        &request(as_opencode_sent_it()),
        "opencode",
        "/tmp/t146/project".to_owned(),
        Some("ses_1".to_owned()),
        acts_on,
    )
}

/// The note tells a person where the call acts, when the agent said.
///
/// The `toolCallId` join exists for this sentence. Without it the note names a
/// command and a session directory, and a reader has no way to tell that the
/// second is not an answer to "where does this happen" — which on T14.6 was
/// measured to be the question that decided whose permission rules applied.
#[test]
fn the_note_says_where_the_call_acts_when_the_agent_said() {
    let note = asking_note(&parked(vec!["/tmp/t146/project/out.txt".to_owned()]));

    assert!(note.contains("/tmp/t146/project/out.txt"), "got: {note}");
    assert!(
        note.contains("acts on"),
        "the path is labelled as the call's, not left as a bare string: {note}"
    );
}

/// **And says nothing at all when the agent named no location.**
///
/// The tempting fallback is the session directory, which is right there and
/// usually correct. It is still Warp's own fact rather than the agent's, and
/// presenting it as where the call acts would manufacture the one certainty this
/// fork has repeatedly measured itself not to have. An absent claim stays absent.
#[test]
fn a_call_that_named_no_location_is_not_given_one() {
    let note = asking_note(&parked(Vec::new()));

    assert!(!note.contains("acts on"), "got: {note}");
    assert!(
        note.contains("This session runs in `/tmp/t146/project`"),
        "the session directory is still said, as Warp's own fact: {note}"
    );
}
