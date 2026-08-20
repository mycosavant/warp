//! `agent.*` and `slash.*` — the half of `warpctrl` that lets an agent drive an
//! agent (`.fork/TASKS.md`, T6.5).
//!
//! Before this, `warpctrl` could open every surface and type into exactly one of
//! them. `input.submit` puts its text in the terminal input and *runs* it, so a
//! caller reaching for the agent got a shell command instead:
//!
//! ```text
//! warpctrl input submit '/agent what is 6 times 7'
//!     bash: /agent: No such file or directory
//! ```
//!
//! That was not a Linux-build problem or a profile problem, both of which were
//! believed in turn — the identical call fails the identical way on Windows. The
//! keyboard route is `ctrl`+`shift`+`Return`, which no local-control action
//! could reach.

use ::local_control::protocol::{
    AgentConversationSummary, AgentListResult, AgentPromptParams, AgentPromptResult,
    SlashCommandSummary, SlashListResult, SlashRunParams, TargetSelector,
};
use ::local_control::{ActionKind, ControlError, ErrorCode, InstanceId};
use warpui::{ModelContext, SingletonEntity, ViewHandle};

use crate::ai::agent::conversation::{AIConversationId, ConversationStatus};
use crate::ai::blocklist::history_model::BlocklistAIHistoryModel;
use crate::local_control::LocalControlBridge;
use crate::local_control::handlers::ack;
use crate::local_control::resolver::{input_target_pane_id, target_pane_group};
use crate::search::slash_command_menu::static_commands::SlashCommandKind;
use crate::search::slash_command_menu::static_commands::commands::COMMAND_REGISTRY;
use crate::terminal::input::slash_commands::slash_command_is_submitted_as_prompt;
use crate::terminal::view::TerminalView;

/// Whether a slash command is part of managing agents and conversations, and so
/// runs from `warpctrl` without `--force`.
///
/// **An allowlist rather than a deny-list, deliberately.** The registry is
/// upstream's and grows; a deny-list would silently admit every command added
/// after it was written. Getting that wrong here means an agent driving
/// `warpctrl` can end its own session — `/exit` and `/logout` sit in the same
/// registry as `/compact` and are one typo apart from it.
///
/// What is on the list is "verbs for running work", not "verbs that happen to be
/// safe". `/index` and `/init` do real work and are here; `/theme` and
/// `/vim-mode` are harmless and are not, because an orchestrator has no business
/// restyling the user's terminal.
///
/// Notable exclusions, each for a reason:
///
/// * `/clear` discards conversation state with no undo.
/// * `/auto-approve` changes the approval policy. An agent widening its own
///   permissions is exactly the thing a person should type themselves.
/// * `/exit`, `/logout`, `/manage-billing`, `/upgrade`, `/connect-grok`,
///   `/api-keys` are account and lifecycle, not orchestration.
pub fn slash_command_is_orchestration(kind: SlashCommandKind) -> bool {
    matches!(
        kind,
        // Starting and shaping conversations.
        SlashCommandKind::Agent
            | SlashCommandKind::CloudAgent
            | SlashCommandKind::New
            | SlashCommandKind::Plan
            | SlashCommandKind::Orchestrate
            | SlashCommandKind::Queue
            // Context management — the reason T6.5 was asked for.
            | SlashCommandKind::Compact
            | SlashCommandKind::CompactAnd
            | SlashCommandKind::Fork
            | SlashCommandKind::ForkAndCompact
            | SlashCommandKind::ForkFrom
            | SlashCommandKind::Rewind
            // Handing work between conversations and machines.
            | SlashCommandKind::ContinueLocally
            | SlashCommandKind::MoveToCloud
            | SlashCommandKind::Conversations
            | SlashCommandKind::RenameConversation
            // Choosing what answers.
            | SlashCommandKind::Model
            | SlashCommandKind::Harness
            | SlashCommandKind::Profile
            | SlashCommandKind::Environment
            | SlashCommandKind::Host
            // Reading the state of the work.
            | SlashCommandKind::Status
            | SlashCommandKind::Usage
            | SlashCommandKind::Cost
            // Carrying context out of a conversation and into a handoff.
            | SlashCommandKind::ExportToClipboard
            | SlashCommandKind::ExportToFile
            // Preparing a repository to be worked on.
            | SlashCommandKind::Index
            | SlashCommandKind::Init
            | SlashCommandKind::InvokeSkill
    )
}

/// `agent.list` — every live conversation, with the one field a caller polling
/// for "is it my turn yet" needs.
pub fn agent_list(
    instance_id: &Option<InstanceId>,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    let conversations = BlocklistAIHistoryModel::as_ref(ctx)
        .all_live_conversations()
        .into_iter()
        .map(|(_terminal_surface_id, conversation)| {
            let status = conversation.status();
            AgentConversationSummary {
                conversation_id: conversation.id().to_string(),
                title: conversation.title(),
                status: status_name(status).to_owned(),
                blocked_action: match status {
                    ConversationStatus::Blocked { blocked_action } => Some(blocked_action.clone()),
                    _ => None,
                },
                // `InProgress` alone. `WaitingForEvents` is quiescent by
                // design — the agent yielded and is listening — and `Blocked`
                // is waiting on a person, so reporting either as busy would
                // make a poller wait for something that is already waiting for
                // it.
                is_busy: matches!(status, ConversationStatus::InProgress),
                // Left unresolved for now: mapping a conversation back to the
                // pane showing it needs the terminal-surface id translated
                // through the pane group, and `agent.prompt` addresses
                // conversations rather than panes precisely so callers do not
                // need it. See `.fork/TASKS.md` T6.6.
                pane_id: None,
                tab_id: None,
            }
        })
        .collect::<Vec<_>>();

    let mut response = ack(instance_id, ActionKind::AgentList);
    merge(
        &mut response,
        serde_json::to_value(AgentListResult { conversations })
            .map_err(|error| ControlError::new(ErrorCode::Internal, error.to_string()))?,
    );
    Ok(response)
}

fn status_name(status: &ConversationStatus) -> &'static str {
    match status {
        ConversationStatus::InProgress => "in_progress",
        ConversationStatus::Success => "success",
        ConversationStatus::Error => "error",
        ConversationStatus::TransientError => "transient_error",
        ConversationStatus::Cancelled => "cancelled",
        ConversationStatus::Blocked { .. } => "blocked",
        ConversationStatus::WaitingForEvents => "waiting_for_events",
    }
}

/// `slash.list` — the registry, with the allowlist decision attached to each
/// entry so a caller can see what it may run before it tries.
///
/// Worth listing rather than documenting: the registry is assembled behind
/// feature flags at first use, so which commands exist is a property of the
/// running build, not of the source. `/compact`, `/queue`, `/fork-from`,
/// `/rewind`, `/profile`, `/host`, `/harness` and `/environment` are each
/// gated. A caller that hardcodes a name will eventually be wrong; one that
/// reads this will not.
pub fn slash_list(
    instance_id: &Option<InstanceId>,
    target: &TargetSelector,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    let terminal_view = terminal_view_for(ActionKind::SlashList, target, ctx)?;
    let mut commands = terminal_view.read(ctx, |terminal_view, ctx| {
        COMMAND_REGISTRY
            .all_commands()
            .map(|command| SlashCommandSummary {
                name: command.name.to_owned(),
                is_orchestration: slash_command_is_orchestration(command.kind),
                submits_prompt: slash_command_is_submitted_as_prompt(command),
                is_available: terminal_view
                    .slash_command_is_available_for_local_control(command, ctx),
            })
            .collect::<Vec<_>>()
    });
    commands.sort_by(|left, right| left.name.cmp(&right.name));

    let mut response = ack(instance_id, ActionKind::SlashList);
    merge(
        &mut response,
        serde_json::to_value(SlashListResult { commands })
            .map_err(|error| ControlError::new(ErrorCode::Internal, error.to_string()))?,
    );
    Ok(response)
}

fn merge(response: &mut serde_json::Value, extra: serde_json::Value) {
    if let (Some(object), Some(extra)) = (response.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            object.insert(key.clone(), value.clone());
        }
    }
}

/// Resolves the conversation a caller named, or explains why it could not be.
///
/// Kept separate from the send so the failure modes are distinguishable: a
/// malformed id is the caller's mistake, an unknown id usually means the
/// conversation ended, and the two want different responses from an
/// orchestrator.
pub fn resolve_conversation(
    params: &AgentPromptParams,
    ctx: &ModelContext<LocalControlBridge>,
) -> Result<Option<AIConversationId>, ControlError> {
    let Some(raw) = params.conversation_id.as_deref() else {
        return Ok(None);
    };
    let id = AIConversationId::try_from(raw.to_owned()).map_err(|_| {
        ControlError::new(
            ErrorCode::InvalidParams,
            format!("conversation_id `{raw}` is not a UUID"),
        )
    })?;
    if BlocklistAIHistoryModel::as_ref(ctx)
        .conversation(&id)
        .is_none()
    {
        return Err(ControlError::new(
            ErrorCode::MissingTarget,
            format!(
                "no live conversation `{raw}`; `agent.list` reports the ones that exist right now"
            ),
        ));
    }
    Ok(Some(id))
}

/// `agent.prompt` — the action T6.5 was opened for.
pub fn agent_prompt(
    instance_id: &Option<InstanceId>,
    params: &serde_json::Value,
    target: &TargetSelector,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    let params: AgentPromptParams = serde_json::from_value(params.clone())
        .map_err(|error| ControlError::new(ErrorCode::InvalidParams, error.to_string()))?;
    if params.prompt.trim().is_empty() {
        return Err(ControlError::new(
            ErrorCode::InvalidParams,
            "agent.prompt requires a non-empty prompt",
        ));
    }
    let conversation_id = resolve_conversation(&params, ctx)?;
    let terminal_view = terminal_view_for(ActionKind::AgentPrompt, target, ctx)?;

    let created = conversation_id.is_none();
    let started = terminal_view.update(ctx, |terminal_view, ctx| {
        terminal_view.start_agent_conversation_from_local_control(
            params.prompt.clone(),
            conversation_id,
            ctx,
        )
    });

    let started = started.ok_or_else(|| {
        ControlError::new(
            ErrorCode::TargetStateConflict,
            "the agent is monitoring a long-running command; it cannot take a new conversation \
             until that finishes",
        )
    })?;

    let mut response = ack(instance_id, ActionKind::AgentPrompt);
    merge(
        &mut response,
        serde_json::to_value(AgentPromptResult {
            conversation_id: started.to_string(),
            created,
        })
        .map_err(|error| ControlError::new(ErrorCode::Internal, error.to_string()))?,
    );
    Ok(response)
}

/// `slash.run` — the registry, behind the allowlist.
pub fn slash_run(
    instance_id: &Option<InstanceId>,
    params: &serde_json::Value,
    target: &TargetSelector,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    let params: SlashRunParams = serde_json::from_value(params.clone())
        .map_err(|error| ControlError::new(ErrorCode::InvalidParams, error.to_string()))?;
    // Accept `/compact` as well as `compact`. The registry stores the name
    // *with* the slash — `StaticCommand::name` is `"/compact"` — so both sides
    // are stripped rather than the caller's input alone, which would have made
    // every lookup miss.
    let name = params.command.trim().trim_start_matches('/');
    let command = COMMAND_REGISTRY
        .all_commands()
        .find(|command| command.name.trim_start_matches('/') == name)
        .ok_or_else(|| {
            ControlError::new(
                ErrorCode::InvalidParams,
                // Not "no such command": the registry is assembled behind
                // feature flags, so a real command can be genuinely absent from
                // this build. `slash.list` reports what this build has.
                format!("`{name}` is not available in this build; `slash.list` reports what is"),
            )
        })?;

    if !params.force && !slash_command_is_orchestration(command.kind) {
        return Err(ControlError::new(
            ErrorCode::InsufficientPermissions,
            format!(
                "refused: `{name}` is not an orchestration command. Re-run with force if you \
                 meant it."
            ),
        ));
    }

    let terminal_view = terminal_view_for(ActionKind::SlashRun, target, ctx)?;

    // Availability is per-pane, and refusing here rather than reporting
    // `handled: false` is the difference between a caller that can act on the
    // answer and one that has to guess. `/compact` needs an agent view with an
    // active conversation; asking for it from a shell pane is a target problem,
    // not a command problem, and the error says which.
    let available = terminal_view.read(ctx, |terminal_view, ctx| {
        terminal_view.slash_command_is_available_for_local_control(command, ctx)
    });
    if !available {
        return Err(ControlError::new(
            ErrorCode::TargetStateConflict,
            format!(
                "`{name}` is not available in this pane right now; `slash.list` reports \
                 `is_available` for each command against the pane you target"
            ),
        ));
    }

    let argument = params.argument.clone();
    let handled = terminal_view.update(ctx, |terminal_view, ctx| {
        terminal_view.run_slash_command_from_local_control(command, argument, ctx)
    });

    let mut response = ack(instance_id, ActionKind::SlashRun);
    if let Some(object) = response.as_object_mut() {
        object.insert(
            "command".to_owned(),
            serde_json::Value::String(name.to_owned()),
        );
        // `execute_slash_command` reports whether it took the command, which is
        // not the same as whether the command succeeded — a command can be
        // handled and then show the user an error toast. Reported as-is rather
        // than translated into a success this cannot vouch for.
        object.insert("handled".to_owned(), serde_json::Value::Bool(handled));
    }
    Ok(response)
}

fn terminal_view_for(
    action_kind: ActionKind,
    target: &TargetSelector,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<ViewHandle<TerminalView>, ControlError> {
    let pane_group = target_pane_group(action_kind, target, ctx)?;
    let pane_id = input_target_pane_id(action_kind, target, &pane_group, ctx)?;
    pane_group
        .read(ctx, |pane_group, ctx| {
            pane_group.terminal_view_from_pane_id(pane_id, ctx)
        })
        .ok_or_else(|| {
            ControlError::new(
                ErrorCode::MissingTarget,
                format!("{} requires a terminal target", action_kind.as_str()),
            )
        })
}

#[cfg(test)]
#[path = "agent_tests.rs"]
mod tests;
