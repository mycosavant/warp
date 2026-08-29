use super::*;

/// Every kind the allowlist admits, stated as data.
///
/// Asserted against [`SlashCommandKind`] rather than against the registry
/// on purpose. The registry is assembled behind feature flags at first use —
/// `/compact` is gated on `SummarizationConversationCommand`, `/queue` on
/// `QueueSlashCommand`, and so on — and flags are off in a unit-test process,
/// so a registry-driven assertion would silently stop covering the exact
/// commands this feature exists for.
const ORCHESTRATION_KINDS: &[SlashCommandKind] = &[
    SlashCommandKind::Agent,
    SlashCommandKind::CloudAgent,
    SlashCommandKind::New,
    SlashCommandKind::Plan,
    SlashCommandKind::Orchestrate,
    SlashCommandKind::Queue,
    SlashCommandKind::Compact,
    SlashCommandKind::CompactAnd,
    SlashCommandKind::Fork,
    SlashCommandKind::ForkAndCompact,
    SlashCommandKind::ForkFrom,
    SlashCommandKind::Rewind,
    SlashCommandKind::ContinueLocally,
    SlashCommandKind::MoveToCloud,
    SlashCommandKind::Conversations,
    SlashCommandKind::RenameConversation,
    SlashCommandKind::Model,
    SlashCommandKind::Harness,
    SlashCommandKind::Profile,
    SlashCommandKind::Environment,
    SlashCommandKind::Host,
    SlashCommandKind::Status,
    SlashCommandKind::Usage,
    SlashCommandKind::Cost,
    SlashCommandKind::ExportToClipboard,
    SlashCommandKind::ExportToFile,
    SlashCommandKind::Index,
    SlashCommandKind::Init,
    SlashCommandKind::InvokeSkill,
];

#[test]
fn every_orchestration_kind_is_admitted() {
    for kind in ORCHESTRATION_KINDS {
        assert!(
            slash_command_is_orchestration(*kind),
            "{kind:?} is on the allowlist and must run without force"
        );
    }
}

/// The commands an agent must not reach by mistyping one it may.
///
/// `/exit` is one character from `/edit`; `/clear` discards a conversation with
/// no undo; `/auto-approve` widens the agent's own permissions, which is
/// precisely the decision a person should make at a keyboard. These are why
/// `slash.run` refuses by default rather than permits by default.
#[test]
fn the_session_ending_kinds_are_refused() {
    for kind in [
        SlashCommandKind::Exit,
        SlashCommandKind::Logout,
        SlashCommandKind::Clear,
        SlashCommandKind::AutoApprove,
        SlashCommandKind::ManageBilling,
        SlashCommandKind::Upgrade,
        SlashCommandKind::ApiKeys,
        SlashCommandKind::ConnectGrok,
        SlashCommandKind::Feedback,
        SlashCommandKind::Theme,
        SlashCommandKind::VimMode,
        SlashCommandKind::CopyDebuggingId,
    ] {
        assert!(
            !slash_command_is_orchestration(kind),
            "{kind:?} must not run from warpctrl without force"
        );
    }
}

/// The classification is a closed set: nothing outside [`ORCHESTRATION_KINDS`]
/// is admitted, checked against every command the running build actually has.
///
/// This is the direction that matters. `slash_command_is_orchestration` matches
/// on named variants, so a command upstream adds tomorrow is excluded by
/// default and cannot widen the surface by accident. This asserts that property
/// rather than trusting it.
#[test]
fn nothing_outside_the_list_is_admitted() {
    for command in COMMAND_REGISTRY.all_commands() {
        if slash_command_is_orchestration(command.kind) {
            assert!(
                ORCHESTRATION_KINDS.contains(&command.kind),
                "{} ({:?}) is admitted but is not on the list",
                command.name,
                command.kind
            );
        }
    }
}

/// The registry stores names *with* the leading slash — `"/agent"`, not
/// `"agent"` — so `slash.run` has to strip both sides. Stripping only the
/// caller's input made every lookup miss, which is how this was found.
#[test]
fn a_command_resolves_with_or_without_its_slash() {
    fn resolve(name: &str) -> Option<&'static str> {
        let name = name.trim().trim_start_matches('/');
        COMMAND_REGISTRY
            .all_commands()
            .find(|command| command.name.trim_start_matches('/') == name)
            .map(|command| command.name)
    }

    // `/agent` is in the registry's unconditional list, so this holds whatever
    // the feature flags say.
    assert_eq!(resolve("agent"), Some("/agent"));
    assert_eq!(resolve("/agent"), Some("/agent"));
    assert_eq!(resolve("  /agent  "), Some("/agent"));
    assert_eq!(resolve("no-such-command"), None);
}

/// `--last N` counts from the newest end, and neither end of the range panics.
///
/// The saturating case is the one that matters in use: an orchestrator asking
/// for the last 10 turns of a conversation that has had 2 wants both of them,
/// not an error and not an empty list. The zero case is the other direction —
/// a caller whose arithmetic produced 0 asked for nothing, and returning the
/// whole transcript instead would be the most expensive possible way to be
/// wrong about that.
#[test]
fn last_counts_back_from_the_newest_exchange() {
    assert_eq!(exchange_window_start(5, None), 0);
    assert_eq!(exchange_window_start(5, Some(1)), 4);
    assert_eq!(exchange_window_start(5, Some(5)), 0);
    assert_eq!(exchange_window_start(2, Some(10)), 0);
    assert_eq!(exchange_window_start(0, Some(1)), 0);
    assert_eq!(exchange_window_start(5, Some(0)), 5);
}

/// A misspelled tool name is refused rather than dropped.
///
/// In an allowlist, silently dropping a token always errs toward *fewer* tools
/// than the caller meant — so the child gets a policy nobody wrote, and the
/// symptom is a delegated agent that will not do the work, discovered later
/// and somewhere else. The refusal names the vocabulary so the fix is in the
/// error.
#[test]
fn an_unknown_tool_name_is_refused() {
    let error = resolve_allowed_tools(&["READ_FILES".to_owned(), "Bash".to_owned()])
        .expect_err("`Bash` is Claude's name for it, not Warp's");
    assert_eq!(error.code, ErrorCode::InvalidParams);
    assert!(
        error.message.contains("RUN_SHELL_COMMAND"),
        "the error should name the vocabulary: {}",
        error.message
    );
}

/// Presets expand, names resolve, and the two do not double up.
///
/// `read-only` overlapping an explicitly named read tool is the ordinary case
/// — a caller adds one tool to a preset — and a duplicated entry would reach
/// the policy and then the `--allowedTools` argument, where it is noise at
/// best.
#[test]
fn presets_and_names_resolve_into_one_list() {
    let resolved = resolve_allowed_tools(&["read-only".to_owned(), "READ_FILES".to_owned()])
        .expect("both tokens resolve");
    assert_eq!(
        resolved
            .iter()
            .filter(|t| **t == ToolType::ReadFiles)
            .count(),
        1,
        "a tool named twice should appear once"
    );
    assert!(resolved.contains(&ToolType::Grep));
    assert!(!resolved.contains(&ToolType::RunShellCommand));

    // The strictest policy a caller can express, and it has to survive being
    // expressed: an empty list is "no tools", not "no policy".
    assert_eq!(resolve_allowed_tools(&[]), Ok(Vec::new()));
}

/// Only `swap` reveals a conversation that was never a background child.
///
/// The default is `pane`, which is the strict one, and that is the right way
/// round: an orchestrator revealing its own child gets the non-destructive
/// target without asking, and a caller pointing `reveal` at an ordinary
/// conversation is told so instead of watching a warning get logged where it
/// cannot see it.
#[test]
fn only_swap_reveals_a_conversation_that_is_not_a_child_agent() {
    assert!(reveal_target_requires_child_pane(AgentRevealTarget::Pane));
    assert!(reveal_target_requires_child_pane(AgentRevealTarget::Tab));
    assert!(!reveal_target_requires_child_pane(AgentRevealTarget::Swap));
    assert_eq!(AgentRevealTarget::default(), AgentRevealTarget::Pane);
}

/// A malformed id and an unknown one are different failures.
///
/// An orchestrator polling a child it spawned needs to tell "I built this
/// string wrong" from "the conversation I was watching has gone", because the
/// second is a normal outcome of a child finishing and being closed, and the
/// first is a bug in the caller. One error code for both would make the normal
/// case look like a bug.
#[test]
fn a_bad_id_and_a_missing_one_are_told_apart() {
    let malformed = parse_conversation_id("not-a-uuid").expect_err("should not parse");
    assert_eq!(malformed.code, ErrorCode::InvalidParams);

    let missing = unknown_conversation("f3f2e0a6-0000-4000-8000-000000000000");
    assert_eq!(missing.code, ErrorCode::MissingTarget);
    assert!(
        missing.message.contains("agent.list"),
        "the error should name the action that lists what exists: {}",
        missing.message
    );
}

/// Every conversation state is distinguishable, and only one of them is busy.
///
/// A poller that treats `waiting_for_events` or `blocked` as busy waits for
/// something that is already waiting for *it* — the agent has yielded, or is
/// asking a person for permission. That deadlock is why `is_busy` is its own
/// field rather than something the caller derives from `status`.
#[test]
fn only_in_progress_is_busy() {
    let statuses = [
        (ConversationStatus::InProgress, "in_progress", true),
        (ConversationStatus::Success, "success", false),
        (ConversationStatus::Error, "error", false),
        (ConversationStatus::TransientError, "transient_error", false),
        (ConversationStatus::Cancelled, "cancelled", false),
        (
            ConversationStatus::Blocked {
                blocked_action: "run rm -rf".to_owned(),
            },
            "blocked",
            false,
        ),
        (
            ConversationStatus::WaitingForEvents,
            "waiting_for_events",
            false,
        ),
    ];

    for (status, expected_name, expected_busy) in statuses {
        assert_eq!(status_name(&status), expected_name);
        assert_eq!(
            matches!(status, ConversationStatus::InProgress),
            expected_busy,
            "{expected_name} busy-ness"
        );
    }
}

/// A failed turn reports its reason, and **nothing else does**.
///
/// The first half is why `error` exists: measured on T14.6, a conversation whose
/// panel was showing a full error paragraph read back through `agent.read` as an
/// exchange with no `output` and no reason, because
/// `FinishedAIAgentOutput::output()` returns `None` for the `Error` variant.
/// A caller could only conclude the agent had answered with silence.
///
/// The second half is the hazard the fix introduces, and is the reason this is a
/// test rather than a sentence in the doc comment. A **cancelled** turn is not a
/// failed one — `agent.list` already reports `cancelled`, and unlike the error
/// case its partial output survives — so reporting an error for it would make
/// every deliberate stop look like a malfunction. Streaming has not failed yet
/// either.
#[test]
fn only_a_failed_exchange_reports_an_error() {
    use crate::ai::agent::{
        AIAgentOutputStatus, CancellationReason, FinishedAIAgentOutput, RenderableAIError,
    };

    let failed = AIAgentOutputStatus::Finished {
        finished_output: FinishedAIAgentOutput::Error {
            output: None,
            error: RenderableAIError::other("the agent could not be started", false),
        },
    };
    assert_eq!(
        exchange_error(&failed).as_deref(),
        Some("the agent could not be started"),
        "a failed turn has to say why, or it is indistinguishable from a silent one"
    );

    let cancelled = AIAgentOutputStatus::Finished {
        finished_output: FinishedAIAgentOutput::Cancelled {
            output: None,
            reason: CancellationReason::ManuallyCancelled,
        },
    };
    assert_eq!(
        exchange_error(&cancelled),
        None,
        "a turn someone stopped on purpose is not a failure"
    );

    assert_eq!(
        exchange_error(&AIAgentOutputStatus::Streaming { output: None }),
        None,
        "a turn still running has not failed"
    );
}
