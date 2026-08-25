//! Warp's own agent, in the same log as the agents it hosts (T11.1b).
//!
//! T11.1 gave hosted agents a durable record by projecting `CLIAgentEventType`,
//! which they already speak. Warp's own agent speaks something else entirely —
//! `BlocklistAIActionEvent` and `BlocklistAIHistoryEvent`, in-process, richer,
//! and persisted only as conversation *state* in SQLite rather than as a
//! sequence of things that happened. This module is the bridge, and it exists
//! because a log that answers "what did the agent do" for four of the five
//! agents in the window is not an answer.
//!
//! **The mapping is not invented here.**
//! `crates/warp_tui/src/cli_agent_osc_event_publisher.rs` already converts world
//! 1 into world 2's vocabulary so the headless TUI can announce itself to a host
//! terminal, and this follows it wherever the two agree. Where it departs, it is
//! because that publisher is feeding a *notification* and this is feeding a
//! *log*, which want opposite things:
//!
//! * **Every conversation, not the selected one.** The publisher ignores
//!   anything the user is not looking at, which is right for a toast and wrong
//!   for a log — orchestrated child agents are the case this fork cares most
//!   about and are never the selected conversation.
//! * **Every finished action, not just answered questions.** The publisher emits
//!   `tool_complete` only for `AskUserQuestion`, because the rest would be
//!   noise. Noise is what a log is for.
//! * **`tool_start` as well as `tool_complete`.** Not in the wire vocabulary,
//!   and deliberately added: without it, an action that begins and never returns
//!   leaves no trace at all, and "started and never finished" is precisely the
//!   shape of the failure this phase exists to catch. `CLIAgentEventType`
//!   already round-trips names it does not know (`Unknown(String)`), so the log
//!   was always a superset of the protocol.
//!
//! **On this fork's primary path the action half below never fires**, so what
//! this module contributes to a local-agent turn is the turn-level frame:
//! `session_start`, `prompt_submit`, `stop`. Two independent reasons, both
//! measured:
//!
//! * `local_agent` answers from the `claude` CLI, which runs its own tools, and
//!   `translate.rs` turns a `tool_use` block into *text* rather than a
//!   `ToolCall` — deliberately, because a ToolCall is an instruction and Warp
//!   would run the command a second time. So Warp's action model sees nothing.
//! * The plugin's OSC 777 does not arrive either. `local_agent` spawns `claude`
//!   with `Stdio::piped()` and reads its JSON directly; there is no Warp PTY in
//!   that path, so nothing reaches the terminal parser world 2 hangs off.
//!
//! **So on the primary path the log currently carries the frame and not the
//! tools, and that is a gap rather than a design.** Closing it means projecting
//! the stream `translate.rs` is already parsing — it sees every `tool_use`
//! block — but filing those lines under the *run* needs Warp's
//! `AIConversationId`, and `RequestParams` carries only Claude's session token.
//! That plumbing is T11.1c; it was scoped rather than half-built. World 2
//! meanwhile remains what it always was: a CLI agent a person ran in a Warp
//! pane, under its own session id and so its own file.
//!
//! The action table below is reached by Warp's own server-backed agent, which
//! this fork has no account for. It is held by unit tests instead, and that is
//! stated plainly rather than implied: it has not been driven.
//!
//! **World 1 has the per-call id world 2 is missing.** `AIAgentActionId` is
//! stable across an action's whole life, so `permission_request` → `tool_start`
//! → `tool_complete` join on `call_id` — and a `tool_start` with no
//! `permission_request` before it, sharing no id with any ask, is an action that
//! ran unasked. That is the kode-rs failure written as a query. The hosted-agent
//! half of the same idea is still `TR-EVENTS-B` and still needs a protocol
//! version bump, because there the id has to come from the plugin.

use warpui::{AppContext, Entity, EntityId, ModelContext, ModelHandle, SingletonEntity as _};

use super::Entry;
use crate::ai::agent::conversation::ConversationStatus;
use crate::ai::agent::{
    AIAgentActionResultTypeDiscriminants, AIAgentActionType, AIAgentActionTypeDiscriminants,
    CancellationReason,
};
use crate::ai::blocklist::{
    BlocklistAIActionEvent, BlocklistAIActionModel, BlocklistAIHistoryEvent,
    BlocklistAIHistoryModel, ConversationStatusUpdate,
};
use crate::terminal::model::session::active_session::ActiveSession;

/// The `agent` value on every line this module writes.
///
/// The same string `CLIAgent::WarpTui` canonicalises to, because it is the same
/// agent — what separates an in-app turn from a headless TUI one is `source`,
/// not identity.
const AGENT: &str = "warp";

/// The `source` value on every line this module writes: these events never
/// crossed a process boundary.
const SOURCE: &str = "in_process";

/// Longest free text kept on a line. The wire protocol's own limit is 320
/// characters (`MAX_NOTIFICATION_DESCRIPTION_CHARS` in the TUI publisher) and
/// matching it keeps summaries comparable between the two worlds.
const MAX_TEXT_LEN: usize = 320;

/// Starts recording this terminal surface's Warp-agent activity.
///
/// Registered on the host's context rather than a model of its own: the
/// projection holds no state, so a second entity would buy nothing but a
/// lifetime to manage. Called from `BlocklistAIController::new`, which is the
/// one place per terminal surface that already has both models in hand.
pub(crate) fn subscribe<T: Entity>(
    action_model: &ModelHandle<BlocklistAIActionModel>,
    active_session: ModelHandle<ActiveSession>,
    terminal_surface_id: EntityId,
    ctx: &mut ModelContext<T>,
) {
    let session = active_session.clone();
    ctx.subscribe_to_model(action_model, move |_, action_model, event, ctx| {
        record_action_event(&action_model, &session, event, ctx);
    });

    let session = active_session;
    let history_model = BlocklistAIHistoryModel::handle(ctx);
    ctx.subscribe_to_model(&history_model, move |_, _, event, ctx| {
        record_history_event(&session, terminal_surface_id, event, ctx);
    });
}

fn record_action_event(
    action_model: &ModelHandle<BlocklistAIActionModel>,
    active_session: &ModelHandle<ActiveSession>,
    event: &BlocklistAIActionEvent,
    ctx: &AppContext,
) {
    if !super::is_enabled() {
        return;
    }
    let actions = action_model.as_ref(ctx);
    let action_id = event.action_id();

    // `FinishedAction` names its conversation; the rest carry a bare action id,
    // so the model has to be asked. A synchronous action is already gone by the
    // time this runs and cannot be attributed — see the `tool_start` note below.
    let conversation_id = match event {
        BlocklistAIActionEvent::FinishedAction {
            conversation_id, ..
        } => Some(*conversation_id),
        _ => actions.conversation_id_for_action(action_id),
    };

    // The action itself, while Warp still holds it. `FinishedAction` never does
    // — see [`action_event`] — so it is not looked up for one.
    let action_kind = matches!(
        event,
        BlocklistAIActionEvent::ActionBlockedOnUserConfirmation(_)
            | BlocklistAIActionEvent::ExecutingAction(_)
    )
    .then(|| {
        actions
            .in_flight_action(action_id, ctx)
            .map(|action| &action.action)
    })
    .flatten();

    let Some(event_name) = action_event(event, action_kind) else {
        return;
    };
    // A synchronous action is finished by the executor inside the same call that
    // started it, so by the time this runs it is in neither the pending queue
    // nor the running set and there is no conversation to file it under.
    // Dropping its `tool_start` costs nothing: the `tool_complete` that follows
    // carries the conversation id directly, and an action that completes inside
    // one call cannot be the one that hung.
    if event_name == "tool_start" && conversation_id.is_none() {
        return;
    }

    let error_type = match event {
        BlocklistAIActionEvent::FinishedAction {
            cancellation_reason,
            ..
        } => cancellation_reason.map(cancellation_name),
        _ => None,
    };

    // `tool_complete` has no action to read — it has a *result*, whose variant
    // name is the one part safe to put in a log; the payloads carry command
    // output and file contents.
    let result_kind = matches!(event, BlocklistAIActionEvent::FinishedAction { .. })
        .then(|| {
            actions.get_action_result(action_id).map(|result| {
                format!(
                    "{:?}",
                    AIAgentActionResultTypeDiscriminants::from(&result.result)
                )
            })
        })
        .flatten();
    let tool_name = action_kind
        .map(|kind| format!("{:?}", AIAgentActionTypeDiscriminants::from(kind)))
        .or(result_kind);
    let summary = action_kind.map(|kind| excerpt(&kind.user_friendly_name()));
    let preview = action_kind.and_then(tool_input_preview);

    let cwd = active_session
        .as_ref(ctx)
        .current_working_directory()
        .cloned();
    let session_id = conversation_id.map(|id| id.to_string());
    let call_id = action_id.to_string();
    super::record(Entry {
        v: None,
        agent: AGENT,
        event: event_name,
        source: SOURCE,
        session_id: session_id.as_deref(),
        call_id: Some(&call_id),
        cwd: cwd.as_deref(),
        project: cwd.as_deref().and_then(project_name),
        tool_name: tool_name.as_deref(),
        tool_input_preview: preview.as_deref(),
        summary: summary.as_deref(),
        error_type,
        plugin_version: None,
        applied: true,
    });
}

fn record_history_event(
    active_session: &ModelHandle<ActiveSession>,
    terminal_surface_id: EntityId,
    event: &BlocklistAIHistoryEvent,
    ctx: &AppContext,
) {
    if !super::is_enabled() {
        return;
    }
    // The history model is global, so every terminal surface hears every
    // surface's events. Filtering to our own is what keeps one event from
    // landing in the log once per open pane. Events with no surface at all are
    // history bookkeeping and are not projected.
    if event
        .terminal_surface_id()
        .is_none_or(|id| id != terminal_surface_id)
    {
        return;
    }

    let (event_name, conversation_id, error_type) = match event {
        BlocklistAIHistoryEvent::StartedNewConversation {
            new_conversation_id,
            ..
        } => ("session_start", *new_conversation_id, None),
        BlocklistAIHistoryEvent::UpdatedConversationStatus {
            conversation_id,
            update,
            new_status,
            ..
        } => {
            let Some((name, error_type)) = status_event(update, new_status) else {
                return;
            };
            (name, *conversation_id, error_type)
        }
        // Deliberately not exhaustive, against the house rule, and this is the
        // argument. `BlocklistAIHistoryEvent` has 26 variants and upstream adds
        // to it; listing the 24 that are conversation bookkeeping would put a
        // merge conflict in this file every time one appears, to defend a
        // default that is already the safe one — a log that has not yet learned
        // about a new event is incomplete, never wrong. The two projected here
        // are the two that describe a *turn*.
        _ => return,
    };

    // Only for the events that are *about* a turn; `permission_replied` is a
    // resumption of one rather than a new question.
    //
    // `session_start` is included because on a new conversation it *is* the
    // first turn's start marker: `AIConversation::new` sets the status to
    // `InProgress`, so the first request's status write is `InProgress →
    // InProgress` and produces no `prompt_submit` at all. Measured, not
    // assumed — see the note in `.fork/TASKS.md`.
    let query = matches!(
        event_name,
        "session_start" | "prompt_submit" | "stop" | "stop_failure"
    )
    .then(|| {
        BlocklistAIHistoryModel::as_ref(ctx)
            .conversation(&conversation_id)
            .and_then(|conversation| conversation.latest_user_query())
            .map(|query| excerpt(&query))
    })
    .flatten();

    let cwd = active_session
        .as_ref(ctx)
        .current_working_directory()
        .cloned();
    let session_id = conversation_id.to_string();
    super::record(Entry {
        v: None,
        agent: AGENT,
        event: event_name,
        source: SOURCE,
        session_id: Some(&session_id),
        call_id: None,
        cwd: cwd.as_deref(),
        project: cwd.as_deref().and_then(project_name),
        tool_name: None,
        tool_input_preview: None,
        summary: query.as_deref(),
        error_type,
        plugin_version: None,
        applied: true,
    });
}

/// Which wire event, if any, an action event is.
///
/// `kind` is the action itself when Warp still holds it, which decides only
/// whether an ask is a question or a permission gate. Pure and separated from
/// the model lookups for the same reason [`status_event`] is: on this fork's
/// primary path the whole table is unreachable at runtime (see the module note
/// on local-agent turns), so a test is the only thing that can hold it.
fn action_event(
    event: &BlocklistAIActionEvent,
    kind: Option<&AIAgentActionType>,
) -> Option<&'static str> {
    match event {
        // A question is an ask of the *user*, not a permission gate on a tool,
        // and the wire vocabulary keeps them apart. Following it means a reader
        // counting refused permissions does not also count questions.
        BlocklistAIActionEvent::ActionBlockedOnUserConfirmation(_) => Some(match kind {
            Some(AIAgentActionType::AskUserQuestion { .. }) => "question_asked",
            _ => "permission_request",
        }),
        BlocklistAIActionEvent::ExecutingAction(_) => Some("tool_start"),
        BlocklistAIActionEvent::FinishedAction { .. } => Some("tool_complete"),
        // Warp's internal bookkeeping. Queueing is not an act — the action has
        // not been asked about or run, and logging it would put a line between
        // every ask and its answer. The other three are UI plumbing.
        BlocklistAIActionEvent::QueuedAction(_)
        | BlocklistAIActionEvent::InitProject(_)
        | BlocklistAIActionEvent::ToggleCodeReview(_)
        | BlocklistAIActionEvent::InsertCodeReviewComments { .. } => None,
    }
}

/// Which wire event, if any, a conversation-status change is.
///
/// Split out and total over `ConversationStatus` so that a status upstream adds
/// later has to be decided about rather than silently ignored.
fn status_event(
    update: &ConversationStatusUpdate,
    new_status: &ConversationStatus,
) -> Option<(&'static str, Option<&'static str>)> {
    // A restore re-announces a status that was already reached, in a session
    // that may have ended days ago. Logging it would put yesterday's `stop` in
    // today's file.
    let ConversationStatusUpdate::Changed { prev_status } = update else {
        return None;
    };
    // `Changed` does not mean changed. `AIConversation::update_status_with_error`
    // emits unconditionally, and `update_conversation_in_progress_status` calls
    // it as every single action starts — so a busy turn produces a long run of
    // `InProgress → InProgress`. Without this, each one would be logged as the
    // user submitting a fresh prompt, and the log would say a person was typing
    // throughout a turn nobody was watching. The same guard covers a repeated
    // terminal status, which would otherwise be a second `stop` for one ending.
    if prev_status == new_status {
        return None;
    }
    match new_status {
        ConversationStatus::Success => Some(("stop", None)),
        ConversationStatus::Error => Some(("stop_failure", Some("error"))),
        ConversationStatus::Cancelled => Some(("stop_failure", Some("cancelled"))),
        ConversationStatus::InProgress => match prev_status {
            // The user answered the thing that was blocking it.
            ConversationStatus::Blocked { .. } => Some(("permission_replied", None)),
            // A retry or a resumed wait — the turn never stopped being the same
            // turn, so calling either a new prompt would invent one.
            ConversationStatus::TransientError | ConversationStatus::WaitingForEvents => None,
            // Work beginning after the last turn reached an end: someone asked
            // for something. `InProgress` is listed only because the match is
            // total — the equality guard above has already returned for it.
            ConversationStatus::Success
            | ConversationStatus::Error
            | ConversationStatus::Cancelled
            | ConversationStatus::InProgress => Some(("prompt_submit", None)),
        },
        // Non-terminal and reported by their own events: `Blocked` arrives as
        // the `permission_request` the action model already emitted, and the
        // other two are waits that resolve back to `InProgress`.
        ConversationStatus::Blocked { .. }
        | ConversationStatus::TransientError
        | ConversationStatus::WaitingForEvents => None,
    }
}

/// The wire name for a cancellation, so `error_type` on a `tool_complete` says
/// which kind rather than merely that there was one.
fn cancellation_name(reason: CancellationReason) -> &'static str {
    match reason {
        CancellationReason::ManuallyCancelled => "manually_cancelled",
        CancellationReason::AutomaticCloudHandoff => "automatic_cloud_handoff",
        CancellationReason::FollowUpSubmitted { .. } => "follow_up_submitted",
        CancellationReason::UserCommandExecuted => "user_command_executed",
        CancellationReason::Reverted => "reverted",
        CancellationReason::Deleted => "deleted",
        CancellationReason::CommandFinishedDuringInlineAgentView => "command_finished",
        CancellationReason::CLISubagentUserTakeover => "user_takeover",
        CancellationReason::AgentExitedShell => "agent_exited_shell",
    }
}

/// The part of an action worth putting in `tool_input_preview`.
///
/// The field is world 2's, where it holds a plugin's `command` or `file_path`,
/// so this fills it with the same two things and nothing else: a reader greps
/// this field for what was *run*, and widening it to every action's arguments
/// would make that grep unreliable rather than more useful. Hence the catch-all
/// — for the rest, "no preview" is the right answer, not a missing case.
fn tool_input_preview(action: &AIAgentActionType) -> Option<String> {
    match action {
        AIAgentActionType::RequestCommandOutput { command, .. } => Some(excerpt(command)),
        // What the agent typed into a command that was already running — the
        // same question as "what was run", asked of an interactive program.
        AIAgentActionType::WriteToLongRunningShellCommand { input, .. } => {
            Some(excerpt(&String::from_utf8_lossy(input)))
        }
        AIAgentActionType::RequestFileEdits { file_edits, .. } => {
            let files = file_edits
                .iter()
                .filter_map(|edit| edit.file())
                .collect::<Vec<_>>()
                .join(", ");
            (!files.is_empty()).then(|| excerpt(&files))
        }
        _ => None,
    }
}

/// The working directory's last component, matching what the TUI publisher puts
/// in `project`.
fn project_name(cwd: &str) -> Option<&str> {
    std::path::Path::new(cwd)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
}

/// Collapses whitespace and truncates, so one line stays one line.
///
/// A raw command or query can contain newlines, and a newline in a JSONL record
/// would be escaped rather than break the file — but the escaping is what a
/// person reading `tail -f` would have to undo, so it is removed here instead.
fn excerpt(text: &str) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut excerpt: String = normalized.chars().take(MAX_TEXT_LEN).collect();
    if normalized.chars().count() > MAX_TEXT_LEN {
        excerpt.push('…');
    }
    excerpt
}

#[cfg(test)]
#[path = "warp_agent_tests.rs"]
mod tests;
