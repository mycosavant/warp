use super::*;
use crate::terminal::cli_agent_sessions::{CLIAgentInputState, CLIAgentSessionContext};

fn session(status: CLIAgentSessionStatus, context: CLIAgentSessionContext) -> CLIAgentSession {
    CLIAgentSession {
        agent: CLIAgent::Claude,
        status,
        session_context: context,
        input_state: CLIAgentInputState::Closed,
        should_auto_toggle_input: false,
        listener: None,
        plugin_version: None,
        remote_host: None,
        draft_text: None,
        custom_command_prefix: None,
        received_rich_notification: true,
    }
}

fn permission_context() -> CLIAgentSessionContext {
    CLIAgentSessionContext {
        cwd: Some("/home/someone/git/warp".to_owned()),
        project: Some("warp".to_owned()),
        session_id: Some("abc".to_owned()),
        tool_name: Some("Bash".to_owned()),
        tool_input_preview: Some("rm -rf build/".to_owned()),
        summary: Some("Wants to run Bash: rm -rf build/".to_owned()),
        query: None,
        response: None,
    }
}

fn blocked() -> CLIAgentSessionStatus {
    CLIAgentSessionStatus::Blocked {
        message: Some("Wants to run Bash: rm -rf build/".to_owned()),
    }
}

/// A session blocked by a permission request, built the way the session model
/// builds one: `Blocked { message }` and `session_context.summary` are the same
/// string, because both come from the same event's `summary` field.
///
/// Tests that vary the context's summary have to vary the block's too, or they
/// are asserting about the *stale-tool-fields* branch rather than the thing they
/// meant to test.
fn blocked_on(context: CLIAgentSessionContext) -> CLIAgentSession {
    session(
        CLIAgentSessionStatus::Blocked {
            message: context.summary.clone(),
        },
        context,
    )
}

/// The whole reason this action exists: a CLI agent that is *not* waiting is not
/// an approval, and reporting one would have a watcher answering a question
/// nobody asked.
#[test]
fn only_a_blocked_session_is_an_approval() {
    for status in [
        CLIAgentSessionStatus::InProgress,
        CLIAgentSessionStatus::Success,
        CLIAgentSessionStatus::Cancelled,
        CLIAgentSessionStatus::Failed {
            error_type: None,
            message: None,
        },
    ] {
        assert!(approval_for(&session(status, permission_context()), "7", "t1").is_none());
    }
    assert!(approval_for(&session(blocked(), permission_context()), "7", "t1").is_some());
}

/// What a person is being asked to decide has to *reach* them. A summary alone
/// would make "allow" mean "allow whatever Warp decided not to show you".
#[test]
fn an_approval_carries_the_command_it_is_asking_about() {
    let approval =
        approval_for(&session(blocked(), permission_context()), "7", "t1").expect("blocked");

    assert_eq!(approval.approval_id, "7");
    assert_eq!(approval.agent, "claude");
    assert_eq!(approval.kind, "permission");
    assert_eq!(approval.tool_name.as_deref(), Some("Bash"));
    assert_eq!(approval.tool_input.as_deref(), Some("rm -rf build/"));
    assert_eq!(approval.cwd.as_deref(), Some("/home/someone/git/warp"));
    assert_eq!(approval.session_id.as_deref(), Some("abc"));
    assert_eq!(approval.tab_id.as_deref(), Some("t1"));
}

/// A blocked session with no tool named is an agent asking a *question*, and the
/// distinction is worth reporting: "allow" on one of these takes the highlighted
/// answer rather than granting a permission.
#[test]
fn a_block_with_no_tool_is_reported_as_a_question() {
    let context = CLIAgentSessionContext {
        tool_name: None,
        tool_input_preview: None,
        summary: None,
        ..permission_context()
    };
    let approval = approval_for(
        &session(
            CLIAgentSessionStatus::Blocked {
                message: Some("Which database should I use?".to_owned()),
            },
            context,
        ),
        "7",
        "t1",
    )
    .expect("blocked");

    assert_eq!(approval.kind, "question");
    assert_eq!(
        approval.summary.as_deref(),
        Some("Which database should I use?")
    );
}

/// The bug the first live run found, pinned.
///
/// A `question_asked` blocks the session without clearing the `tool_name` and
/// `tool_input_preview` a previous `permission_request` left behind. Reading
/// them anyway reported the agent as still asking to run a command it had
/// already been answered about — and, worse, left the digest unchanged, so a
/// remote yes taken from that screen would have been *accepted* onto a question
/// about something else entirely.
#[test]
fn a_question_after_a_permission_does_not_inherit_the_command() {
    let permission = session(blocked(), permission_context());
    let question = session(
        CLIAgentSessionStatus::Blocked {
            message: Some("Which database should I use?".to_owned()),
        },
        // The context a real session is left holding: `question_asked` touches
        // none of these.
        permission_context(),
    );

    let before = approval_for(&permission, "7", "t1").expect("blocked");
    let after = approval_for(&question, "7", "t1").expect("blocked");

    assert_eq!(after.kind, "question");
    assert_eq!(after.tool_name, None, "the stale command must not be shown");
    assert_eq!(after.tool_input, None);
    assert_eq!(
        after.summary.as_deref(),
        Some("Which database should I use?")
    );
    assert_ne!(
        before.digest, after.digest,
        "an answer taken from the permission screen must not fit the question"
    );
}

/// The other half of the same rule: a permission request that carried no summary
/// keeps its tool fields, because `None` matching `None` is a genuine match and
/// not the coincidence the check is guarding against.
#[test]
fn a_permission_with_no_summary_still_reports_its_command() {
    let context = CLIAgentSessionContext {
        summary: None,
        ..permission_context()
    };
    let approval = approval_for(
        &session(CLIAgentSessionStatus::Blocked { message: None }, context),
        "7",
        "t1",
    )
    .expect("blocked");

    assert_eq!(approval.kind, "permission");
    assert_eq!(approval.tool_input.as_deref(), Some("rm -rf build/"));
}

/// The property `agent.approve` is built on. If the digest did not move when the
/// request did, a yes taken from a phone screen would land on whatever the agent
/// is asking a minute later.
#[test]
fn the_digest_moves_when_the_request_does() {
    let original =
        approval_for(&session(blocked(), permission_context()), "7", "t1").expect("blocked");

    for changed in [
        CLIAgentSessionContext {
            tool_input_preview: Some("rm -rf /".to_owned()),
            ..permission_context()
        },
        CLIAgentSessionContext {
            tool_name: Some("Write".to_owned()),
            ..permission_context()
        },
        CLIAgentSessionContext {
            cwd: Some("/etc".to_owned()),
            ..permission_context()
        },
        CLIAgentSessionContext {
            summary: Some("Wants to run Bash: something else".to_owned()),
            ..permission_context()
        },
    ] {
        let other = approval_for(&blocked_on(changed), "7", "t1").expect("blocked");
        assert_ne!(
            original.digest, other.digest,
            "a different request must not share a digest"
        );
    }
}

/// The same request, read twice, has to be answerable the second time — a digest
/// that moved on its own would refuse every answer sent by a client that polled.
#[test]
fn the_digest_holds_still_when_the_request_does() {
    let first =
        approval_for(&session(blocked(), permission_context()), "7", "t1").expect("blocked");
    let second =
        approval_for(&session(blocked(), permission_context()), "7", "t1").expect("blocked");

    assert_eq!(first.digest, second.digest);
}

/// Two panes asking the identical thing are still two requests, and a yes for
/// one must not be spendable on the other.
#[test]
fn a_digest_from_one_pane_does_not_fit_another() {
    let left = approval_for(&session(blocked(), permission_context()), "7", "t1").expect("blocked");
    let right =
        approval_for(&session(blocked(), permission_context()), "9", "t1").expect("blocked");

    assert_ne!(left.digest, right.digest);
}

/// Length prefixes, asserted directly rather than trusted. Concatenation would
/// make `("Bash", "ls")` and `("Bashl", "s")` the same bytes, and a command is
/// arbitrary text so no separator character is safe either.
#[test]
fn adjacent_fields_cannot_be_shifted_into_each_other() {
    let left = CLIAgentSessionContext {
        tool_name: Some("Bash".to_owned()),
        tool_input_preview: Some("ls".to_owned()),
        summary: None,
        ..permission_context()
    };
    let right = CLIAgentSessionContext {
        tool_name: Some("Bashl".to_owned()),
        tool_input_preview: Some("s".to_owned()),
        summary: None,
        ..permission_context()
    };

    assert_ne!(
        approval_for(&blocked_on(left), "7", "t1")
            .expect("blocked")
            .digest,
        approval_for(&blocked_on(right), "7", "t1")
            .expect("blocked")
            .digest,
    );
}

/// An absent field is not an empty one. An agent can genuinely send an empty
/// summary, and a client that could turn "no summary" into "empty summary"
/// without moving the digest would have found a way to change the request
/// without invalidating the answer to it.
#[test]
fn an_absent_field_is_not_an_empty_one() {
    let absent = CLIAgentSessionContext {
        summary: None,
        ..permission_context()
    };
    let empty = CLIAgentSessionContext {
        summary: Some(String::new()),
        ..permission_context()
    };

    assert_ne!(
        approval_for(&blocked_on(absent), "7", "t1")
            .expect("blocked")
            .digest,
        approval_for(&blocked_on(empty), "7", "t1")
            .expect("blocked")
            .digest,
    );
}

/// The two decisions differ by exactly one keystroke, and which one is which is
/// the entire safety story. Pinned so a refactor cannot quietly swap them.
#[test]
fn yes_is_return_and_no_is_escape() {
    assert_eq!(Decision::Allow.bytes(), b"\r");
    assert_eq!(Decision::Allow.keystroke(), "enter");
    assert_eq!(Decision::Allow.action(), ActionKind::AgentApprove);

    assert_eq!(Decision::Deny.bytes(), b"\x1b");
    assert_eq!(Decision::Deny.keystroke(), "escape");
    assert_eq!(Decision::Deny.action(), ActionKind::AgentDeny);
}

/// Deliberately a change-detector. Pressing Return means "take the highlighted
/// option", and which option a given agent highlights is a fact about someone
/// else's TUI that this fork can only learn by watching it. Adding an entry here
/// is a claim that somebody did.
#[test]
fn only_agents_whose_prompt_was_watched_can_be_answered_yes() {
    assert_eq!(ALLOW_VERIFIED_AGENTS, [CLIAgent::Claude].as_slice());

    let error = unverified_agent(CLIAgent::Codex);
    assert_eq!(error.code, ErrorCode::InsufficientPermissions);
    assert!(error.message.contains("Codex"));
    // Names the way out, because the alternative is a caller concluding the
    // agent cannot be answered at all.
    assert!(error.message.contains("deny"));
}

/// The listing and the answer agree about approvability, because they read the
/// same predicate.
///
/// They did not, and the console believed the listing. `agent.approvals` reports
/// every blocked session; `agent.approve` refuses agents outside
/// [`ALLOW_VERIFIED_AGENTS`]; and nothing carried that refusal into the entry —
/// so `console.js`, which took its *Yes* from the paired device's action list,
/// drew one on rows the handler would always reject. Two facts about the same
/// row, disagreeing, with the person holding the phone told the wrong one.
///
/// This is the assertion that keeps them from drifting again: whatever
/// `agent.approve` would refuse, the entry says so up front and in the same
/// words.
#[test]
fn an_entry_reports_the_same_refusal_the_answer_would_give() {
    assert_eq!(
        approve_refusal(CLIAgent::Claude),
        None,
        "a verified agent is approvable and must not carry a reason it is not"
    );

    let refusal = approve_refusal(CLIAgent::Codex).expect("Codex is not verified");
    assert_eq!(
        refusal,
        unverified_agent(CLIAgent::Codex).message,
        "the sentence on the entry and the sentence in the error are the same sentence"
    );
    assert!(refusal.contains("deny"), "it has to name the way out");
}

/// The options the agent offered are part of the question, so an answer is
/// bound to them.
///
/// An ACP agent sends its options typed, and a re-ask offering a *different* set
/// is a different question — measured shapes differ even between agents, with
/// `opencode` putting allow first and `claude-agent-acp` putting deny first. If
/// the digest ignored them, a yes read off one option list could be replayed
/// against another, which is the exact hazard the digest was built for one field
/// over.
///
/// Constructed directly rather than through `approval_for`, because the CLI
/// population never sees options — the prompt is drawn on someone else's PTY.
#[test]
fn an_answer_is_bound_to_the_options_that_were_offered() {
    let base = PendingApproval {
        approval_id: "req-1".to_owned(),
        agent: "opencode".to_owned(),
        kind: "permission".to_owned(),
        summary: Some("echo hello".to_owned()),
        tool_name: Some("execute".to_owned()),
        tool_input: None,
        cwd: None,
        project: None,
        session_id: None,
        tab_id: None,
        digest: String::new(),
        can_approve: false,
        approve_refused_because: None,
        options_offered: vec!["Allow once".to_owned(), "Reject".to_owned()],
    };

    let same = PendingApproval { ..base.clone() };
    assert_eq!(
        digest_of(&base),
        digest_of(&same),
        "the same question hashes the same way"
    );

    for changed in [
        vec![
            "Allow once".to_owned(),
            "Always allow".to_owned(),
            "Reject".to_owned(),
        ],
        vec!["Reject".to_owned(), "Allow once".to_owned()],
        Vec::new(),
    ] {
        let other = PendingApproval {
            options_offered: changed.clone(),
            ..base.clone()
        };
        assert_ne!(
            digest_of(&base),
            digest_of(&other),
            "a different option list is a different question: {changed:?}"
        );
    }
}
