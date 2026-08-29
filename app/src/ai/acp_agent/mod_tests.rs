//! The half of the ACP path that can be decided without an agent.
//!
//! The exchange itself is verified by running it against a real agent — the
//! fork's standard, and the reason `warpctrl acp probe` exists (`.fork/TASKS.md`
//! T14.5).

use agent_client_protocol::schema::v1::{ToolCallUpdate, ToolCallUpdateFields, ToolKind};

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

/// **`deny` may never select an allow**, even when an allow is the only thing on
/// offer.
///
/// This used to say "nothing here says yes", which was true of the whole module
/// until this ticket and is now true only of this function. That is the narrower
/// and more durable claim anyway: a denial is unconditional, so the one path
/// that must never widen anything is the one a person reached by saying *no*. A
/// future edit that "helpfully" picks an `allow_once` when no reject is offered
/// has to come through this test.
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

/// The shape a real request has: a kind whose effect stops at the call, and the
/// `rawInput` that is the only place the specifics survive.
///
/// Transcribed from the T14.6 capture rather than invented — `request()` above
/// is the older fixture and carries neither, which is why it is the *unapprovable*
/// one here.
fn an_executable_request(options: Vec<PermissionOption>) -> RequestPermissionRequest {
    RequestPermissionRequest::new(
        "session-1",
        ToolCallUpdate::new(
            "call_1",
            ToolCallUpdateFields::new()
                .title("echo hello > greeting.txt")
                .kind(ToolKind::Execute)
                .raw_input(serde_json::json!({"command": "echo hello > greeting.txt"})),
        ),
        options,
    )
}

fn park_this(request: &RequestPermissionRequest, acts_on: Vec<String>) -> registry::ParkedRequest {
    parked_request(
        &agent_client_protocol::schema::v1::RequestId::from(0),
        "turn-1",
        request,
        "opencode",
        "/tmp/t146/project".to_owned(),
        Some("ses_1".to_owned()),
        acts_on,
    )
}

fn parked(acts_on: Vec<String>) -> registry::ParkedRequest {
    park_this(&an_executable_request(as_opencode_sent_it()), acts_on)
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

/// **Both measured agents can be approved, and both select the option they
/// themselves typed `allow_once`.**
///
/// The counterpart of `both_measured_agents_are_denied_by_selecting_their_reject_option`,
/// and it carries the same argument in the direction that can actually do
/// damage. `opencode` puts allow first and `claude-agent-acp` puts deny first, so
/// a yes chosen by position would have selected *"Deny"* on one of them — the
/// T14.2 bug, mirrored. Choosing by kind means the id differs and the meaning
/// does not.
#[test]
fn both_measured_agents_can_be_approved_by_the_option_they_typed_allow_once() {
    for (options, expected) in [
        (as_opencode_sent_it(), "once"),
        (as_claude_sent_it(), "allow"),
    ] {
        let parked = park_this(&an_executable_request(options), Vec::new());

        assert_eq!(
            parked.approve_selects.as_deref(),
            Some(expected),
            "a yes selects the agent's own single-shot allow, by kind and never by position"
        );
        assert_eq!(parked.approve_refused_because, None);
    }
}

/// **A request that shows nothing cannot be approved**, whatever its options say.
///
/// With no `rawInput` the only thing any surface can render is the agent's own
/// one-line title, and approving a title is not approving a command. This is the
/// one refusal this module adds on top of `acp_permission`'s, and it is about
/// disclosure rather than scope: the options here are the same ones that are
/// approvable in the test above.
#[test]
fn a_request_that_shows_only_a_title_cannot_be_approved() {
    let parked = park_this(&request(as_opencode_sent_it()), Vec::new());

    assert_eq!(parked.approve_selects, None);
    let why = parked
        .approve_refused_because
        .expect("a refusal has a reason");
    assert!(why.contains("no tool input"), "got: {why}");
    assert!(
        why.contains("deny") || why.contains("agent"),
        "a refusal has to name the way out: {why}"
    );
}

/// **The escalation `acp_permission` exists to refuse, refused through this
/// path too.**
///
/// A `switch_mode` request asks *which policy should apply*, not whether one
/// thing may happen — measured on `claude-agent-acp`, whose five options are the
/// session's mode ids, typed `allow_once` and carrying no `_meta`. A binary yes
/// cannot honestly mean "and also change the session's policy", so no option
/// here is selectable and the entry says which kind it could not bound.
///
/// This is the shared rule doing its job across a crate boundary, which is the
/// reason the module is shared rather than copied.
#[test]
fn a_request_to_change_the_session_policy_is_never_approvable() {
    let switch_mode = RequestPermissionRequest::new(
        "session-1",
        ToolCallUpdate::new(
            "call_1",
            ToolCallUpdateFields::new()
                .title("Exit plan mode?")
                .kind(ToolKind::SwitchMode)
                .raw_input(serde_json::json!({"mode": "default"})),
        ),
        vec![
            option(
                "default",
                "Yes, and manually approve edits",
                PermissionOptionKind::AllowOnce,
            ),
            option(
                "plan",
                "No, keep planning",
                PermissionOptionKind::RejectOnce,
            ),
        ],
    );

    let parked = park_this(&switch_mode, Vec::new());

    assert_eq!(
        parked.approve_selects, None,
        "an `allow_once` that sets the session's permission mode is still not single-shot"
    );
    let why = parked
        .approve_refused_because
        .expect("a refusal has a reason");
    assert!(
        why.contains("policy"),
        "the reason names what is actually being asked: {why}"
    );
}

/// An agent offering no single-shot allow at all cannot be approved, and the
/// refusal lists what it *did* offer.
///
/// Without the list this is indistinguishable from a bug in Warp — the failure
/// is agent-specific and invisible otherwise, which is the argument
/// `no_option_reason` was written under.
#[test]
fn an_agent_offering_only_always_cannot_be_approved() {
    let parked = park_this(
        &an_executable_request(vec![
            option("always", "Always allow", PermissionOptionKind::AllowAlways),
            option("reject", "Reject", PermissionOptionKind::RejectOnce),
        ]),
        Vec::new(),
    );

    assert_eq!(parked.approve_selects, None);
    let why = parked
        .approve_refused_because
        .expect("a refusal has a reason");
    assert!(
        why.contains("allow_always"),
        "it names what was offered: {why}"
    );
}

/// The note offers a yes only where there is one to offer, and says why not
/// otherwise.
///
/// It read "Warp cannot say yes to this yet" on every request, which was true
/// while nothing could be approved and went false the moment something could. A
/// refusal whose stated reason is false is the T14.2 failure — someone concludes
/// the feature is broken — so the sentence is derived from the frozen decision.
#[test]
fn the_note_offers_yes_only_when_there_is_a_yes_to_offer() {
    let approvable = asking_note(&parked(Vec::new()));
    assert!(
        approvable.contains("warpctrl agent approve turn-1:0"),
        "got: {approvable}"
    );
    assert!(
        approvable.contains("this one call"),
        "the scope of the yes is stated, not left to be assumed: {approvable}"
    );

    let refused = asking_note(&park_this(&request(as_opencode_sent_it()), Vec::new()));
    assert!(!refused.contains("agent approve"), "got: {refused}");
    assert!(
        refused.contains("no tool input"),
        "the entry's own reason, not a generic one: {refused}"
    );
}

/// **Every way the wait can end that is not a permitted yes ends in a no.**
///
/// These arms lived inside a `connection.spawn` async block, where nothing could
/// reach them — so the fork's own rule applied and they were extracted rather
/// than described. Three of the four are refusals, and this phase has now
/// produced four separate cases of a hazard written in a comment and shipped
/// undefended.
///
/// The third case is the one worth spelling out: `Allow` on an entry with no
/// option to select cannot be reached, because the control plane refuses it by
/// reading the same frozen field. It denies anyway. Increment 1's collision
/// cascade was two halves of one fix where either alone left the other latent,
/// and this is the same discipline — fail closed at both ends, not only at the
/// end being looked at.
#[test]
fn only_a_permitted_yes_selects_an_allow() {
    let denial = RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
        PermissionOptionId::from("reject".to_owned()),
    ));
    let selected = |outcome: RequestPermissionOutcome| match outcome {
        RequestPermissionOutcome::Selected(selected) => selected.option_id.to_string(),
        RequestPermissionOutcome::Cancelled => "cancelled".to_owned(),
        _ => "unknown".to_owned(),
    };

    assert_eq!(
        selected(outcome_for(
            Ok(registry::Decision::Allow),
            Some("once".to_owned()),
            denial.clone()
        )),
        "once",
        "a permitted yes selects the option that was frozen at park time"
    );
    assert_eq!(
        selected(outcome_for(
            Ok(registry::Decision::Deny),
            Some("once".to_owned()),
            denial.clone()
        )),
        "reject",
        "a no is a no even where a yes was available"
    );
    assert_eq!(
        selected(outcome_for(
            Err(futures::channel::oneshot::Canceled),
            Some("once".to_owned()),
            denial.clone()
        )),
        "reject",
        "a dropped sender is the turn ending, and ending is not consent"
    );
    assert_eq!(
        selected(outcome_for(
            Ok(registry::Decision::Allow),
            None,
            denial.clone()
        )),
        "reject",
        "an Allow for an entry with nothing to select denies rather than guesses"
    );
}

/// The card names the **agent**, not whatever launched it.
///
/// **Measured on T14.6**: a live `claude-agent-acp` request came up as
/// *"npx wants permission"*, because the command that reaches it is
/// `npx -y @agentclientprotocol/claude-agent-acp` and the first token is the
/// runner. Accurate and useless — every agent run through `npx` would say the
/// same, so the one field whose job is *which agent is waiting* had stopped
/// answering it.
///
/// The last case is the guarantee that makes the heuristic acceptable: anything
/// unrecognised falls back to the first token, which is what shipped before, so
/// the failure mode is less information and never wrong information.
#[test]
fn the_agent_is_named_rather_than_its_launcher() {
    for (command, expected) in [
        // The measured one.
        (
            "npx -y @agentclientprotocol/claude-agent-acp",
            "claude-agent-acp",
        ),
        // Pinned versions and scopes both come off.
        (
            "npx -y @agentclientprotocol/claude-agent-acp@0.70.0",
            "claude-agent-acp",
        ),
        ("bunx some-agent@1.2.3", "some-agent"),
        // A direct command is already right and must not be touched.
        ("opencode acp", "opencode"),
        ("/usr/local/bin/my-agent --acp", "/usr/local/bin/my-agent"),
        // Degenerate shapes fall back rather than panic or empty out.
        ("npx", "npx"),
        ("npx -y", "npx"),
        ("", ""),
    ] {
        assert_eq!(agent_name(command), expected, "for {command:?}");
    }
}

/// The refusal reads as sentences, because it is one paragraph a person uses to
/// decide something.
///
/// **Measured on T14.6**, against a live `switch_mode` request: the note read
/// *"…so Warp declines and the session keeps the policy it already had Answer no
/// with `warpctrl agent deny …`"*. The reasons come from a shared module in
/// another crate, so the terminator is added here rather than assumed there —
/// and not doubled when it is already present.
#[test]
fn a_refusal_reads_as_sentences_however_the_reason_was_written() {
    let terminated = asking_note(&registry::ParkedRequest {
        approve_refused_because: Some("it ends in a full stop.".to_owned()),
        ..park_this(&request(as_opencode_sent_it()), Vec::new())
    });
    assert!(
        terminated.contains("full stop. Answer no with"),
        "an already-terminated reason is not given a second stop: {terminated}"
    );

    let bare = asking_note(&registry::ParkedRequest {
        approve_refused_because: Some("it does not".to_owned()),
        ..park_this(&request(as_opencode_sent_it()), Vec::new())
    });
    assert!(
        bare.contains("it does not. Answer no with"),
        "an unterminated reason gets one: {bare}"
    );
}
