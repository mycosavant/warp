//! The half of the ACP path that can be decided without an agent.
//!
//! The exchange itself is verified by running it against a real agent — the
//! fork's standard, and the reason `warpctrl acp probe` exists (`.fork/TASKS.md`
//! T14.5).

use agent_client_protocol::schema::v1::{ToolCallUpdate, ToolCallUpdateFields, ToolKind};

use super::*;

/// An answer as the panel button would deliver it.
///
/// The surface is arbitrary for every assertion below: `outcome_for` is
/// documented as sending only the decision to the agent, and `answered_note`
/// reports what was answered rather than who answered. A test that varied the
/// surface here would be asserting the opposite of that design.
fn answered_on_the_panel(decision: registry::Decision) -> registry::Answer {
    registry::Answer {
        decision,
        surface: registry::Surface::Panel,
    }
}

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
/// **Only for an agent that says it cannot resume.** T14.7 gave the ones that
/// can a `session/load`, so this is no longer every second turn — which is why
/// the sentence now has to name the agent: it is the thing that differs between
/// the conversation that continues and the one that does not.
#[test]
fn an_agent_that_cannot_resume_is_refused_with_a_reason() {
    let error = continuation_error();

    assert!(
        error.contains("opencode"),
        "the refusal must name the agent it is about, got: {error}"
    );
    assert!(
        error.contains("no memory of what you see above"),
        "the refusal must say what it is refusing and why, got: {error}"
    );
    assert!(
        error.contains("Start a new conversation"),
        "the refusal must say what to do instead, got: {error}"
    );
}

/// …and an agent that *said* it could resume and then failed gets a different
/// sentence, because the advice differs: "use another agent" would be wrong
/// advice for an agent that has simply lost the session.
#[test]
fn an_agent_that_promised_to_resume_and_failed_says_so_instead() {
    let error = resume_failed("opencode", "session not found");

    assert!(error.contains("said it could resume"), "got: {error}");
    assert!(
        error.contains("session not found"),
        "the underlying error is the only thing that distinguishes one of these \
         from another, got: {error}"
    );
    assert!(
        !error.contains("point WARP_FORK_ACP_COMMAND"),
        "this is not an agent-choice problem, got: {error}"
    );
}

/// The refusal text is the only thing a person sees, so it must not name a
/// mechanism they cannot act on.
///
/// **The env var is deducted before the scan rather than removed from the list,
/// and the difference is the whole rule.** `WARP_FORK_ACP_COMMAND` contains the
/// letters `ACP`, so naming the one knob the person actually turned would
/// otherwise trip a check aimed at protocol nouns. Dropping `"ACP"` from the
/// list to make that go away would stop the test noticing a message that talked
/// about the Agent Client Protocol; deducting the variable keeps it noticing,
/// and says out loud that a variable the person set themselves is not jargon.
#[test]
fn the_continuation_refusal_explains_itself_without_protocol_jargon() {
    let error = continuation_error();

    assert!(
        error.contains("WARP_FORK_ACP_COMMAND"),
        "the person chose this agent with that variable and changes it with that \
         variable, got: {error}"
    );
    let prose = error.replace("WARP_FORK_ACP_COMMAND", "");

    for jargon in ["session/load", "loadSession", "ACP", "conversation_token"] {
        assert!(
            !prose.contains(jargon),
            "{jargon} means nothing to a person reading a conversation, got: {error}"
        );
    }
}

/// The sentence that actually ships.
///
/// `Turn::from_request` cannot be called from here — `RequestParams` has a
/// private field, which is the whole reason `Turn` exists — so these pin the
/// function the connection calls rather than a copy of its output. A test that
/// retyped the message would pass while the shipped one said anything at all.
fn continuation_error() -> String {
    cannot_resume("opencode")
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
        "conv-1".to_owned(),
    )
}

fn parked(acts_on: Vec<String>) -> registry::ParkedRequest {
    park_this(&an_executable_request(as_opencode_sent_it()), acts_on)
}

/// Only a tool call describes what a turn is doing (T14.10).
///
/// A message chunk is a sign of life and not a description of one. If chatter
/// set the remembered activity, a wedged turn would be reported as whatever
/// half-sentence the agent emitted last instead of the call it stopped on —
/// which on T14.9 was the one fact the panel had and the CLI did not.
#[test]
fn only_a_tool_call_names_what_a_turn_is_doing() {
    let spoke = SessionUpdate::AgentMessageChunk(
        agent_client_protocol::schema::v1::ContentChunk::new(ContentBlock::from("thinking")),
    );
    assert_eq!(announced_tool(&spoke), None, "chatter describes nothing");

    let called = SessionUpdate::ToolCall(agent_client_protocol::schema::v1::ToolCall::new(
        "call_1",
        "grep -rn kind_name",
    ));
    assert_eq!(
        announced_tool(&called).as_deref(),
        Some("grep -rn kind_name")
    );
}

/// A tool-call update names the call only when it carries a title. Agents send a
/// placeholder first and correct it — Claude sent "Preparing file…" before
/// "Write a.txt" — so an update without one must leave the remembered title
/// alone rather than blank it.
#[test]
fn a_tool_call_update_without_a_title_describes_nothing() {
    let titled = SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
        "call_1",
        ToolCallUpdateFields::new().title("Write a.txt"),
    ));
    assert_eq!(announced_tool(&titled).as_deref(), Some("Write a.txt"));

    let untitled = SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
        "call_1",
        ToolCallUpdateFields::new()
            .status(agent_client_protocol::schema::v1::ToolCallStatus::InProgress),
    ));
    assert_eq!(announced_tool(&untitled), None);
}

/// The request's own locations outrank the remembered ones (T14.8).
///
/// Measured live: one `cat ~/.bashrc` from a pane in the repo produced
/// notifications carrying the *working directory* and a permission request
/// carrying `/home/effatha` — the path it actually wanted to reach. Taking the
/// remembered value told a person the call acted inside the project. The join
/// still covers the T14.6 case below; it just no longer overrides a request that
/// answered the question itself.
#[test]
fn a_request_that_states_its_own_locations_is_believed_over_the_remembered_ones() {
    let mut asked = an_executable_request(as_opencode_sent_it());
    asked.tool_call.fields.locations = Some(vec![
        agent_client_protocol::schema::v1::ToolCallLocation::new("/home/effatha"),
    ]);

    let stated = stated_locations(&asked);

    assert_eq!(
        stated,
        Some(vec!["/home/effatha".to_owned()]),
        "the request said where it would reach, so that is what a person is shown"
    );
}

/// A request that stated nothing falls back to the join, and an empty list is
/// "said nothing" rather than "nowhere" — the distinction T14.6 built
/// `locations_for` around.
#[test]
fn a_request_that_states_no_location_leaves_the_join_in_charge() {
    let mut silent = an_executable_request(as_opencode_sent_it());
    silent.tool_call.fields.locations = None;
    assert_eq!(stated_locations(&silent), None);

    let mut empty = an_executable_request(as_opencode_sent_it());
    empty.tool_call.fields.locations = Some(Vec::new());
    assert_eq!(
        stated_locations(&empty),
        None,
        "an empty list is the agent saying nothing, not the call acting nowhere"
    );
}

/// The note tells a person where the call acts, when the agent said.
///
/// The `toolCallId` join exists for this sentence. Without it the note names a
/// command and a session directory, and a reader has no way to tell that the
/// second is not an answer to "where does this happen" — which on T14.6 was
/// measured to be the question that decided whose permission rules applied.
#[test]
fn the_note_says_where_the_call_acts_when_the_agent_said() {
    let note = asking_note(&parked(vec!["/tmp/t146/project/out.txt".to_owned()]), true);

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
    let note = asking_note(&parked(Vec::new()), true);

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
    let approvable = asking_note(&parked(Vec::new()), true);
    assert!(
        approvable.contains("warpctrl agent approve turn-1:0"),
        "got: {approvable}"
    );
    assert!(
        approvable.contains("this one call"),
        "the scope of the yes is stated, not left to be assumed: {approvable}"
    );

    let refused = asking_note(
        &park_this(&request(as_opencode_sent_it()), Vec::new()),
        true,
    );
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
            Ok(answered_on_the_panel(registry::Decision::Allow)),
            Some("once".to_owned()),
            denial.clone()
        )),
        "once",
        "a permitted yes selects the option that was frozen at park time"
    );
    assert_eq!(
        selected(outcome_for(
            Ok(answered_on_the_panel(registry::Decision::Deny)),
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
            Ok(answered_on_the_panel(registry::Decision::Allow)),
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
    let terminated = asking_note(
        &registry::ParkedRequest {
            approve_refused_because: Some("it ends in a full stop.".to_owned()),
            ..park_this(&request(as_opencode_sent_it()), Vec::new())
        },
        true,
    );
    assert!(
        terminated.contains("full stop. Answer no with"),
        "an already-terminated reason is not given a second stop: {terminated}"
    );

    let bare = asking_note(
        &registry::ParkedRequest {
            approve_refused_because: Some("it does not".to_owned()),
            ..park_this(&request(as_opencode_sent_it()), Vec::new())
        },
        true,
    );
    assert!(
        bare.contains("it does not. Answer no with"),
        "an unterminated reason gets one: {bare}"
    );
}

/// The transcript has to say the question stopped being open.
///
/// The asking note stays where it was written, so without a second note a
/// finished conversation reads as though it is still waiting — and after a
/// cancelled turn it reads worse, because the id it tells the person to type
/// has already left the registry. Measured T14.7: `warpctrl agent approve` on
/// that id answers `missing_target`.
#[test]
fn every_way_a_permission_question_can_end_says_so_in_the_transcript() {
    let allowed = answered_note(&Ok(answered_on_the_panel(registry::Decision::Allow)));
    let denied = answered_note(&Ok(answered_on_the_panel(registry::Decision::Deny)));
    let dropped = answered_note(&Err(oneshot::Canceled));

    assert!(allowed.contains("yes"), "got: {allowed}");
    assert!(
        allowed.contains("one call"),
        "a yes that does not say how far it reaches is the thing T14.6 exists to \
         prevent, got: {allowed}"
    );
    assert!(denied.contains("no"), "got: {denied}");
    assert!(
        !dropped.contains("no"),
        "nobody said no — crediting a person with a decision they did not make is \
         the same error as claiming a policy Warp cannot see, got: {dropped}"
    );
    assert!(dropped.contains("without an answer"), "got: {dropped}");
}

/// …and none of them claims the call then happened.
///
/// A yes is a yes to one request. Whether the tool succeeds is the agent's
/// business and shows up as its own output — the same distinction `approvals.rs`
/// makes by reporting the keystroke it sent rather than `approved: true`.
#[test]
fn the_answer_note_reports_the_answer_and_not_the_outcome() {
    for note in [
        answered_note(&Ok(answered_on_the_panel(registry::Decision::Allow))),
        answered_note(&Ok(answered_on_the_panel(registry::Decision::Deny))),
        answered_note(&Err(oneshot::Canceled)),
    ] {
        for claim in ["ran", "succeeded", "was executed", "completed"] {
            assert!(
                !note.contains(claim),
                "the note may only report what was answered, got: {note}"
            );
        }
    }
}

/// **A process with no local control server must not be told to use one.**
///
/// Measured 2026-08-30 in the TUI, which registers no `LocalControlServer`
/// (`lib.rs` adds it for `LaunchMode::App | Test` only): the note told a person
/// to run `warpctrl agent approve`, an instrument that cannot exist there, and
/// the turn's only exit was Ctrl-C. That is T14.2's failure in one sentence — a
/// person follows the instruction, nothing happens, and they conclude the
/// feature is broken rather than that the surface is absent.
///
/// The bug does not need the TUI's account bypass to occur: the seam keys on
/// `WARP_FORK_ACP_COMMAND` rather than on auth, so a *signed-in* TUI session
/// parks requests the same way.
#[test]
fn an_unanswerable_process_is_not_told_to_run_warpctrl() {
    let note = asking_note(&parked(Vec::new()), false);

    // The bare word is *allowed*, and deliberately: a person who knows
    // `warpctrl` is exactly the person whose first instinct will be to reach for
    // it, and naming it as absent answers them. What must never appear is a
    // runnable instruction, because that is the thing that gets followed and
    // fails silently. The first draft of this test asserted the stronger
    // property and failed against copy that was right.
    for command in [
        "warpctrl agent approve",
        "warpctrl agent deny",
        "warpctrl agent approvals",
    ] {
        assert!(
            !note.contains(command),
            "gave a command this process cannot run ({command}): {note}"
        );
    }
    assert!(
        !note.contains("WARP_FORK_REMOTE_APPROVE"),
        "pairing rides the same absent server and must not be offered either: {note}"
    );
}

/// **And it must say so, rather than going quiet.**
///
/// Dropping the instruction without replacing it would leave a request that
/// looks answerable and simply is not — the worse half of the same failure. This
/// is T14.18's pattern: Warp discloses what is true and names the one exit that
/// exists.
#[test]
fn an_unanswerable_process_says_so_and_names_the_way_out() {
    let note = asking_note(&parked(Vec::new()), false);

    assert!(
        note.contains("Nothing in this session can answer it"),
        "the dead end must be stated: {note}"
    );
    assert!(
        note.contains("Ctrl-C"),
        "the one exit that exists must be named: {note}"
    );
    assert!(
        note.contains("The agent is waiting for permission"),
        "the request itself is still reported: {note}"
    );
}

/// A refusal in an unanswerable process still gives its reason.
///
/// The reason is the more specific truth and it is what T14.6 added; losing it
/// because the *no* has nowhere to be pressed would trade one silence for
/// another.
#[test]
fn a_refusal_keeps_its_reason_when_nothing_can_answer() {
    let refused = asking_note(
        &park_this(&request(as_opencode_sent_it()), Vec::new()),
        false,
    );

    assert!(
        refused.contains("Warp will not say yes to this"),
        "got: {refused}"
    );
    assert!(
        refused.contains("Ctrl-C"),
        "the exit is named here too: {refused}"
    );
    assert!(
        !refused.contains("warpctrl agent deny"),
        "gave a command this process cannot run: {refused}"
    );
}

/// A translator that will log, with no agent and no connection behind it.
///
/// `open` is what gives it a `session_id`; without it the lines still write,
/// but `linked_session_id` is absent and the fixture stops resembling a real
/// one for no benefit.
fn logging_translator() -> Arc<Mutex<Translator>> {
    let mut translator = Translator::new(
        "task-1".to_owned(),
        true,
        "req-1".to_owned(),
        "does this repo build?".to_owned(),
        Utc::now(),
        "test-agent".to_owned(),
        Some("/tmp/project".to_owned()),
        "conv-guard".to_owned(),
    );
    translator.open("ses_guard".to_owned());
    Arc::new(Mutex::new(translator))
}

/// Filtered on `call_id`, never on the event name.
///
/// The event log's broadcast is process-global and tests run in parallel, so
/// `find(|line| line["event"] == …)` reads whatever a neighbouring test happened
/// to write. That already failed once in this phase.
fn line_for(
    events: &mut tokio::sync::broadcast::Receiver<String>,
    call_id: &str,
) -> Option<serde_json::Value> {
    std::iter::from_fn(|| events.try_recv().ok())
        .map(|line| serde_json::from_str::<serde_json::Value>(&line).expect("a JSON line"))
        .find(|line| line["call_id"] == call_id)
}

/// **A question dropped without an answer still says so.**
///
/// This is the case a cancelled turn produces, and until `AsksNothingMore`
/// existed it produced *nothing*: `permission_request` is written synchronously
/// in the request handler, `permission_replied` from a task that cancellation
/// drops mid-`await`, so the trail kept the ask and lost its ending.
///
/// The assertion is on the value, not merely on the line's presence, because
/// `Entry` is `#[skip_serializing_none]` and the whole argument for `unanswered`
/// being a string rather than an absent key is that a reader must be able to
/// tell "nobody answered" from "an older binary".
#[test]
fn a_dropped_question_records_that_nobody_answered() {
    let mut events = crate::event_log::subscribe();
    let translator = logging_translator();

    drop(AsksNothingMore::arm(
        &translator,
        "req-1:9",
        "call_guard_dropped",
    ));

    let line = line_for(&mut events, "call_guard_dropped").expect("the drop was recorded");
    assert_eq!(line["event"], "permission_replied");
    assert_eq!(line["decision"], "unanswered");
    assert!(
        line["answered_by"].is_null(),
        "nobody answered, so no surface can be named: {line}"
    );
}

/// **And a question that *was* answered is not reported twice.**
///
/// The guard rides the same scope as the real logging call, so a disarm that
/// did not take would double every answered permission in the trail — turning
/// an instrument built to be counted into one that inflates. Cheap to pin and
/// silent if it broke.
#[test]
fn an_answered_question_is_not_also_recorded_as_unanswered() {
    let mut events = crate::event_log::subscribe();
    let translator = logging_translator();

    let mut guard = AsksNothingMore::arm(&translator, "req-1:10", "call_guard_disarmed");
    guard.disarm();
    drop(guard);

    assert!(
        line_for(&mut events, "call_guard_disarmed").is_none(),
        "a disarmed guard wrote a line anyway"
    );
}
