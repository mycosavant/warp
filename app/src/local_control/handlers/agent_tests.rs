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
