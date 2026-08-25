use super::*;
use crate::ai::agent::AIAgentActionId;
use crate::ai::agent::conversation::AIConversationId;

fn command_action(command: &str) -> AIAgentActionType {
    AIAgentActionType::RequestCommandOutput {
        command: command.to_string(),
        is_read_only: None,
        is_risky: None,
        wait_until_completion: true,
        uses_pager: None,
        rationale: None,
        citations: Vec::new(),
    }
}

fn action_id() -> AIAgentActionId {
    AIAgentActionId::from("action-1".to_string())
}

/// The three moments in an action's life that the log is for, and the id that
/// ties them together. A `tool_start` sharing no `call_id` with any preceding
/// ask is an action that ran unasked — the query this whole phase exists to
/// make answerable.
#[test]
fn an_action_is_asked_about_started_and_finished() {
    let command = command_action("ls -la");

    assert_eq!(
        action_event(
            &BlocklistAIActionEvent::ActionBlockedOnUserConfirmation(action_id()),
            Some(&command)
        ),
        Some("permission_request")
    );
    assert_eq!(
        action_event(
            &BlocklistAIActionEvent::ExecutingAction(action_id()),
            Some(&command)
        ),
        Some("tool_start")
    );
    assert_eq!(
        action_event(
            &BlocklistAIActionEvent::FinishedAction {
                action_id: action_id(),
                conversation_id: AIConversationId::new(),
                cancellation_reason: None,
            },
            None
        ),
        Some("tool_complete")
    );
}

/// Asking the user a question is not a tool asking for permission. Collapsing
/// them would make "how many permission requests did this run produce" count
/// interruptions that granted nothing.
#[test]
fn a_question_to_the_user_is_not_a_permission_request() {
    let question = AIAgentActionType::AskUserQuestion {
        questions: Vec::new(),
    };

    assert_eq!(
        action_event(
            &BlocklistAIActionEvent::ActionBlockedOnUserConfirmation(action_id()),
            Some(&question)
        ),
        Some("question_asked")
    );
}

/// An action Warp no longer holds still has to be *named* — otherwise a
/// permission request whose action vanished would silently become a question,
/// which is the more permissive of the two readings.
#[test]
fn an_ask_with_no_action_in_hand_is_still_a_permission_request() {
    assert_eq!(
        action_event(
            &BlocklistAIActionEvent::ActionBlockedOnUserConfirmation(action_id()),
            None
        ),
        Some("permission_request")
    );
}

/// Queueing is not an act: the action has not been asked about or run. Logging
/// it would put a line between every ask and its answer.
#[test]
fn bookkeeping_is_not_an_event() {
    assert_eq!(
        action_event(&BlocklistAIActionEvent::QueuedAction(action_id()), None),
        None
    );
    assert_eq!(
        action_event(&BlocklistAIActionEvent::InitProject(action_id()), None),
        None
    );
    assert_eq!(
        action_event(&BlocklistAIActionEvent::ToggleCodeReview(action_id()), None),
        None
    );
}

/// A restore re-announces a status the conversation already reached, possibly in
/// a session that ended days ago. It must not put yesterday's `stop` in today's
/// file.
#[test]
fn a_restored_status_is_not_an_event() {
    assert_eq!(
        status_event(
            &ConversationStatusUpdate::Restored,
            &ConversationStatus::Success
        ),
        None
    );
}

#[test]
fn a_terminal_status_says_how_the_turn_ended() {
    let changed = |prev| ConversationStatusUpdate::Changed { prev_status: prev };

    assert_eq!(
        status_event(
            &changed(ConversationStatus::InProgress),
            &ConversationStatus::Success
        ),
        Some(("stop", None))
    );
    assert_eq!(
        status_event(
            &changed(ConversationStatus::InProgress),
            &ConversationStatus::Error
        ),
        Some(("stop_failure", Some("error")))
    );
    assert_eq!(
        status_event(
            &changed(ConversationStatus::InProgress),
            &ConversationStatus::Cancelled
        ),
        Some(("stop_failure", Some("cancelled")))
    );
}

/// The pair the log exists for: `Blocked` is the agent asking, and leaving
/// `Blocked` is the person answering. Reading it as a fresh prompt would lose
/// the only record that a permission gate was ever cleared.
#[test]
fn leaving_blocked_is_an_answer_and_not_a_new_prompt() {
    assert_eq!(
        status_event(
            &ConversationStatusUpdate::Changed {
                prev_status: ConversationStatus::Blocked {
                    blocked_action: "Run command: rm -rf /".to_string(),
                },
            },
            &ConversationStatus::InProgress,
        ),
        Some(("permission_replied", None))
    );
}

/// A retry and a resumed wait return to `InProgress` without anyone having
/// typed anything, so neither is a `prompt_submit`.
#[test]
fn resuming_the_same_turn_is_not_a_new_prompt() {
    for prev in [
        ConversationStatus::TransientError,
        ConversationStatus::WaitingForEvents,
    ] {
        assert_eq!(
            status_event(
                &ConversationStatusUpdate::Changed { prev_status: prev },
                &ConversationStatus::InProgress
            ),
            None,
        );
    }
}

/// The one that would have flooded the log. `update_status_with_error` emits
/// whether or not the status moved, and `update_conversation_in_progress_status`
/// runs as every action starts — so a single busy turn produces a long run of
/// `InProgress → InProgress`. Read as prompts, they would claim a person was
/// typing throughout a turn nobody was watching.
#[test]
fn a_status_that_did_not_change_is_not_an_event() {
    for status in [
        ConversationStatus::InProgress,
        ConversationStatus::Success,
        ConversationStatus::Error,
        ConversationStatus::Cancelled,
        ConversationStatus::WaitingForEvents,
    ] {
        assert_eq!(
            status_event(
                &ConversationStatusUpdate::Changed {
                    prev_status: status.clone()
                },
                &status
            ),
            None,
            "{status:?} did not move, so nothing happened"
        );
    }
}

#[test]
fn starting_work_after_a_finished_turn_is_a_new_prompt() {
    for prev in [
        ConversationStatus::Success,
        ConversationStatus::Error,
        ConversationStatus::Cancelled,
    ] {
        assert_eq!(
            status_event(
                &ConversationStatusUpdate::Changed { prev_status: prev },
                &ConversationStatus::InProgress
            ),
            Some(("prompt_submit", None)),
        );
    }
}

/// `Blocked` reaches the log through the action model's
/// `ActionBlockedOnUserConfirmation`, which knows *which* action is blocked.
/// Emitting from the status change as well would double every permission
/// request, and the duplicate would be the one with no tool name.
#[test]
fn a_status_reported_by_another_event_is_not_reported_twice() {
    for status in [
        ConversationStatus::Blocked {
            blocked_action: "Edit src/main.rs".to_string(),
        },
        ConversationStatus::TransientError,
        ConversationStatus::WaitingForEvents,
    ] {
        assert_eq!(
            status_event(
                &ConversationStatusUpdate::Changed {
                    prev_status: ConversationStatus::InProgress
                },
                &status
            ),
            None,
        );
    }
}

/// `tool_input_preview` is world 2's field for "what was run", and a reader
/// greps it expecting a command. Filling it with a command is the point.
#[test]
fn a_command_is_previewed_as_the_command() {
    let action = command_action("rm -rf /");

    assert_eq!(tool_input_preview(&action).as_deref(), Some("rm -rf /"));
}

/// An action with no command and no files has no preview, rather than a
/// plausible-looking one derived from something else — a reader filtering on
/// this field must be able to trust that a value is a command or a path.
#[test]
fn an_action_with_nothing_to_preview_has_none() {
    assert_eq!(tool_input_preview(&AIAgentActionType::InitProject), None);
}

/// A record is one line, and a command spanning several must not become
/// several — nor arrive full of `\n` escapes that a person reading `tail -f`
/// has to undo.
#[test]
fn free_text_is_flattened_to_one_line() {
    assert_eq!(
        excerpt("echo one\necho two\n\techo three"),
        "echo one echo two echo three"
    );
}

#[test]
fn free_text_is_bounded() {
    let long = "x".repeat(MAX_TEXT_LEN * 4);
    let excerpt = excerpt(&long);

    assert_eq!(excerpt.chars().count(), MAX_TEXT_LEN + 1);
    assert!(excerpt.ends_with('…'), "truncation should be visible");
}

/// Counting *characters* rather than bytes: a multi-byte character truncated
/// halfway is not a character, and `String::truncate` would panic on the
/// boundary.
#[test]
fn truncation_does_not_split_a_character() {
    let long = "é".repeat(MAX_TEXT_LEN * 2);
    let excerpt = excerpt(&long);

    assert_eq!(excerpt.chars().count(), MAX_TEXT_LEN + 1);
}

#[test]
fn a_cancellation_says_which_kind_it_was() {
    assert_eq!(
        cancellation_name(CancellationReason::ManuallyCancelled),
        "manually_cancelled"
    );
    assert_eq!(
        cancellation_name(CancellationReason::AgentExitedShell),
        "agent_exited_shell"
    );
}

/// The name written for an action is the variant, not the user-facing string:
/// `user_friendly_name` interpolates the command itself, so grouping by tool
/// would group by *invocation* and every shell call would be its own kind.
#[test]
fn a_tool_name_is_the_kind_and_not_the_invocation() {
    let action = command_action("ls -la");

    assert_eq!(
        format!("{:?}", AIAgentActionTypeDiscriminants::from(&action)),
        "RequestCommandOutput"
    );
    assert!(action.user_friendly_name().contains("ls -la"));
}

#[test]
fn a_project_is_the_last_component_of_the_working_directory() {
    assert_eq!(project_name("/home/u/git/warp"), Some("warp"));
    assert_eq!(project_name("/"), None);
}
