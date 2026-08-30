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

use std::collections::HashMap;

use ::local_control::protocol::{
    AgentCancelParams, AgentCancelResult, AgentConversationSummary, AgentExchangeSummary,
    AgentListResult, AgentPromptParams, AgentPromptResult, AgentReadParams, AgentReadResult,
    AgentRevealParams, AgentRevealResult, AgentRevealTarget, AgentSettleParams, AgentSettleResult,
    AgentSpawnParams, AgentSpawnResult, SlashCommandSummary, SlashListResult, SlashRunParams,
    TargetSelector,
};
use ::local_control::{ActionKind, ControlError, ErrorCode, InstanceId};
use warp_multi_agent_api::ToolType;
use warpui::{EntityId, ModelContext, SingletonEntity, ViewHandle};

use crate::ai::agent::conversation::{AIConversation, AIConversationId, ConversationStatus};
use crate::ai::blocklist::child_agent_tool_policy::{READ_ONLY_PRESET, resolve_tool_token};
use crate::ai::blocklist::history_model::BlocklistAIHistoryModel;
use crate::local_control::LocalControlBridge;
use crate::local_control::handlers::ack;
use crate::local_control::handlers::metadata::select_tab_entries;
use crate::local_control::resolver::{input_target_pane_id, target_pane_group};
use crate::pane_group::{PaneGroup, PaneId};
use crate::search::slash_command_menu::static_commands::SlashCommandKind;
use crate::search::slash_command_menu::static_commands::commands::COMMAND_REGISTRY;
use crate::terminal::input::slash_commands::slash_command_is_submitted_as_prompt;
use crate::terminal::view::{LocalControlRevealTarget, TerminalView};

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

/// Where a conversation's terminal surface is, if it is anywhere.
///
/// Built by walking every tab rather than asked of one, because the panes a
/// conversation can be in are not restricted to the window the caller targeted
/// — an orchestrator's children can be scattered across tabs, and a hidden one
/// is in a tab nobody is looking at.
pub(super) struct SurfaceLocation {
    pub(super) pane_id: PaneId,
    pub(super) tab_id: String,
    is_hidden: bool,
    pub(super) terminal_view: ViewHandle<TerminalView>,
    pane_group: ViewHandle<PaneGroup>,
}

/// Maps terminal-surface ids to the pane showing them, hidden panes included.
///
/// `pane.list` reports `visible_pane_ids` and is right to: a hidden pane is not
/// addressable as a pane. This is the other question — *which* pane holds a
/// conversation — and for a background child agent the answer is a real pane
/// that happens to be hidden. Hence `pane_ids()`, which is every pane in the
/// group, with visibility reported as a field instead of a filter.
pub(super) fn surface_locations(
    ctx: &mut ModelContext<LocalControlBridge>,
) -> HashMap<EntityId, SurfaceLocation> {
    let mut locations = HashMap::new();
    // Degrades to "location unknown" rather than failing the call: every
    // consumer of this map treats a missing entry as a closed surface, which
    // is the same answer a caller gets for a window that has gone away
    // mid-request.
    let tabs = select_tab_entries(&TargetSelector::default(), ActionKind::AgentList, ctx)
        .unwrap_or_default();
    for tab in tabs {
        let tab_id = tab.pane_group.id().to_string();
        let handle = tab.pane_group.clone();
        tab.pane_group.read(ctx, |pane_group, ctx| {
            let visible = pane_group.visible_pane_ids();
            for pane_id in pane_group.pane_ids() {
                let Some(terminal_view) = pane_group.terminal_view_from_pane_id(pane_id, ctx)
                else {
                    continue;
                };
                locations.insert(
                    terminal_view.id(),
                    SurfaceLocation {
                        pane_id,
                        tab_id: tab_id.clone(),
                        is_hidden: !visible.contains(&pane_id),
                        terminal_view,
                        pane_group: handle.clone(),
                    },
                );
            }
        });
    }
    locations
}

fn conversation_summary(
    conversation: &AIConversation,
    location: Option<&SurfaceLocation>,
    settled: bool,
) -> AgentConversationSummary {
    let status = conversation.status();
    // Read once, here, rather than in two field initialisers: the two halves of
    // a liveness report have to describe the same instant, or a listing could
    // pair a quiet time with a tool call from a turn that ended between them.
    let session_mode = crate::ai::acp_agent::mode::current_for(&conversation.id().to_string());
    let (quiet, last_activity, waiting_for_you) =
        match crate::ai::acp_agent::liveness::quiet_for(&conversation.id().to_string()) {
            Some((quiet, tool, waiting)) => (Some(quiet), tool, waiting),
            None => (None, None, false),
        };
    AgentConversationSummary {
        conversation_id: conversation.id().to_string(),
        title: conversation.title(),
        status: status_name(status).to_owned(),
        blocked_action: match status {
            ConversationStatus::Blocked { blocked_action } => Some(blocked_action.clone()),
            _ => None,
        },
        // `InProgress` alone. `WaitingForEvents` is quiescent by design — the
        // agent yielded and is listening — and `Blocked` is waiting on a
        // person, so reporting either as busy would make a poller wait for
        // something that is already waiting for it.
        //
        // And it must have a turn to be busy *with*. `AIConversation` is
        // constructed `InProgress` (`conversation.rs:420`) and only leaves that
        // state when a turn finishes, so a conversation nobody has asked
        // anything reports `in_progress` for as long as it exists — measured at
        // a minute and counting on a freshly opened agent tab. `status` still
        // says what upstream says; `is_busy` is the fork's derived answer and
        // it should mean a turn is actually running.
        is_busy: matches!(status, ConversationStatus::InProgress) && !conversation.is_empty(),
        settled,
        pane_id: location.map(|location| location.pane_id.to_string()),
        tab_id: location.map(|location| location.tab_id.clone()),
        is_hidden: location.is_some_and(|location| location.is_hidden),
        // Only the ACP path keeps this, and only while a turn is in flight, so
        // both fields are absent for every other conversation — see
        // `AgentConversationSummary::quiet_for_seconds` for why absent is not
        // zero.
        quiet_for_seconds: quiet,
        last_activity,
        waiting_for_you,
        session_mode,
    }
}

/// `agent.list` — every live conversation, with the one field a caller polling
/// for "is it my turn yet" needs.
pub fn agent_list(
    instance_id: &Option<InstanceId>,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    let locations = surface_locations(ctx);
    let history = BlocklistAIHistoryModel::as_ref(ctx);
    let conversations = history
        .all_live_conversations()
        .into_iter()
        .map(|(terminal_surface_id, conversation)| {
            conversation_summary(
                conversation,
                locations.get(&terminal_surface_id),
                history.is_conversation_settled(&conversation.id()),
            )
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

/// The first exchange `last` asks for, counting from the end.
///
/// From the end because that is the direction an orchestrator reads: the answer
/// it is waiting for is the newest turn. Saturating rather than clamping so
/// `--last 50` on a two-turn conversation returns both turns instead of
/// nothing, and `--last 0` returns nothing rather than everything — a caller
/// that computed a zero meant zero.
fn exchange_window_start(exchange_count: usize, last: Option<u32>) -> usize {
    match last {
        Some(last) => exchange_count.saturating_sub(last as usize),
        None => 0,
    }
}

/// `agent.read` — what a conversation actually said.
///
/// The gap that made the rest of the surface hard to use: `agent.list` reports
/// *that* a child finished and never *what it produced*, so an orchestrator
/// could dispatch work and watch it complete without being able to collect the
/// result. Handing work along a chain needs the answer, not the status.
/// The error a finished exchange failed with, if it failed.
///
/// **Fork (T14.6).** `format_output_for_copy` goes through
/// [`AIAgentOutputStatus::output`], and its `Error` arm returns `None` — so an
/// exchange that failed reads back with no output and no reason, which is
/// exactly what a successful-but-silent turn looks like. Measured: a
/// conversation displaying a full error paragraph in the panel read back through
/// `agent.read` as an exchange with neither.
///
/// Only `Finished { Error }` produces a string. A cancelled turn is not an error
/// — `agent.list` already reports `cancelled`, and its partial output survives
/// `output()` — and a still-streaming turn has not failed yet.
fn exchange_error(status: &crate::ai::agent::AIAgentOutputStatus) -> Option<String> {
    use crate::ai::agent::{AIAgentOutputStatus, FinishedAIAgentOutput};

    match status {
        AIAgentOutputStatus::Finished {
            finished_output: FinishedAIAgentOutput::Error { error, .. },
        } => Some(error.to_string()),
        _ => None,
    }
}

pub fn agent_read(
    instance_id: &Option<InstanceId>,
    params: &serde_json::Value,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    let params: AgentReadParams = serde_json::from_value(params.clone())
        .map_err(|error| ControlError::new(ErrorCode::InvalidParams, error.to_string()))?;
    let conversation_id = parse_conversation_id(&params.conversation_id)?;
    let locations = surface_locations(ctx);

    let history = BlocklistAIHistoryModel::as_ref(ctx);
    let Some(conversation) = history.conversation(&conversation_id) else {
        return Err(unknown_conversation(&params.conversation_id));
    };
    let location = history
        .terminal_surface_id_for_conversation(&conversation_id)
        .and_then(|id| locations.get(&id));

    // Tool results need the action model of the surface that owns the
    // conversation, and that surface can be closed while the conversation
    // survives. Asking for them and getting text is a smaller failure than
    // refusing the read, so this reports what it managed rather than erroring —
    // `included_tool_results` is the field that says which happened.
    let action_model = params
        .include_tool_results
        .then(|| location.map(|location| location.terminal_view.as_ref(ctx).ai_action_model()))
        .flatten()
        .map(|action_model| action_model.as_ref(ctx));

    let exchanges = conversation.root_task_exchanges().collect::<Vec<_>>();
    let exchange_count = exchanges.len();
    let skip = exchange_window_start(exchange_count, params.last);
    let exchanges = exchanges
        .into_iter()
        .enumerate()
        .skip(skip)
        .map(|(index, exchange)| {
            let input = exchange.format_input_for_copy();
            let output = exchange.format_output_for_copy(action_model);
            AgentExchangeSummary {
                index: index as u32,
                input: (!input.is_empty()).then_some(input),
                output: (!output.is_empty()).then_some(output),
                is_complete: exchange.finish_time.is_some(),
                error: exchange_error(&exchange.output_status),
            }
        })
        .collect::<Vec<_>>();

    let result = AgentReadResult {
        conversation: conversation_summary(
            conversation,
            location,
            history.is_conversation_settled(&conversation_id),
        ),
        exchanges,
        exchange_count: exchange_count as u32,
        included_tool_results: action_model.is_some(),
    };

    let mut response = ack(instance_id, ActionKind::AgentRead);
    merge(
        &mut response,
        serde_json::to_value(result)
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
    let id = parse_conversation_id(raw)?;
    if BlocklistAIHistoryModel::as_ref(ctx)
        .conversation(&id)
        .is_none()
    {
        return Err(unknown_conversation(raw));
    }
    Ok(Some(id))
}

fn parse_conversation_id(raw: &str) -> Result<AIConversationId, ControlError> {
    AIConversationId::try_from(raw.to_owned()).map_err(|_| {
        ControlError::new(
            ErrorCode::InvalidParams,
            format!("conversation_id `{raw}` is not a UUID"),
        )
    })
}

fn unknown_conversation(raw: &str) -> ControlError {
    ControlError::new(
        ErrorCode::MissingTarget,
        format!("no live conversation `{raw}`; `agent.list` reports the ones that exist right now"),
    )
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

/// How deep a conversation sits under the one a person started.
///
/// Walks parents rather than trusting a stored depth, because the chain is
/// what a cap is about and a stored number is one more thing that can be wrong.
/// Bounded so a parent cycle — which the data model does not forbid — costs a
/// refusal rather than a hang.
fn conversation_depth(
    conversation_id: AIConversationId,
    ctx: &ModelContext<LocalControlBridge>,
) -> u32 {
    const MAX_WALK: u32 = 64;
    let history = BlocklistAIHistoryModel::as_ref(ctx);
    let mut depth = 0;
    let mut current = Some(conversation_id);
    while let Some(id) = current {
        if depth >= MAX_WALK {
            break;
        }
        current = history
            .conversation(&id)
            .and_then(|conversation| conversation.parent_conversation_id());
        if current.is_some() {
            depth += 1;
        }
    }
    depth
}

/// Resolves `allow_tools` into the list the policy is enforced in.
///
/// Every token has to resolve. A caller that misspells one and is answered
/// with silence has been handed a policy it did not ask for, and in an
/// allowlist the direction of that mistake is always *fewer* tools than
/// intended — which shows up as a child that will not work, at some later
/// point, for no visible reason.
fn resolve_allowed_tools(tokens: &[String]) -> Result<Vec<ToolType>, ControlError> {
    let mut resolved: Vec<ToolType> = Vec::new();
    for token in tokens {
        let tools = resolve_tool_token(token).ok_or_else(|| {
            ControlError::new(
                ErrorCode::InvalidParams,
                format!(
                    "`{token}` is not a tool. Use `{READ_ONLY_PRESET}`, or a ToolType name such \
                     as READ_FILES or RUN_SHELL_COMMAND."
                ),
            )
        })?;
        for tool in tools {
            if !resolved.contains(&tool) {
                resolved.push(tool);
            }
        }
    }
    Ok(resolved)
}

/// `agent.spawn` — a child agent in a hidden pane.
///
/// The fourth handoff target, and the only one T6.5 could not compose:
/// `pane.split` or `tab.create` followed by `agent.prompt` starts an agent
/// that is visible and unrelated, while this starts a *child* — parented,
/// hidden, and reachable afterwards through `agent.reveal`.
pub fn agent_spawn(
    instance_id: &Option<InstanceId>,
    params: &serde_json::Value,
    target: &TargetSelector,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    let params: AgentSpawnParams = serde_json::from_value(params.clone())
        .map_err(|error| ControlError::new(ErrorCode::InvalidParams, error.to_string()))?;
    if params.prompt.trim().is_empty() {
        return Err(ControlError::new(
            ErrorCode::InvalidParams,
            "agent.spawn requires a non-empty prompt; a child does not inherit its parent's \
             transcript, so the prompt is everything it knows",
        ));
    }
    let allowed_tools = params
        .allow_tools
        .as_deref()
        .map(resolve_allowed_tools)
        .transpose()?;

    // The parent, either named or taken from the pane the caller targeted.
    // Resolved to a conversation and then to *its* pane, rather than spawning
    // beside the targeted pane: a child is inserted relative to its parent's
    // pane and tracked in that pane group, so naming a parent in another tab
    // has to move the spawn there too.
    let parent_conversation_id = match params.parent_conversation_id.as_deref() {
        Some(raw) => {
            let id = parse_conversation_id(raw)?;
            if BlocklistAIHistoryModel::as_ref(ctx)
                .conversation(&id)
                .is_none()
            {
                return Err(unknown_conversation(raw));
            }
            id
        }
        None => {
            let host = terminal_view_for(ActionKind::AgentSpawn, target, ctx)?;
            host.read(ctx, |terminal_view, ctx| {
                terminal_view.selected_conversation_for_local_control(ctx)
            })
            .ok_or_else(|| {
                ControlError::new(
                    ErrorCode::TargetStateConflict,
                    "the targeted pane has no agent conversation to parent a child to; start one \
                     with `agent.prompt`, or name a parent",
                )
            })?
        }
    };

    let depth = conversation_depth(parent_conversation_id, ctx) + 1;
    let limit = crate::fork::agent_spawn_depth_limit();
    if depth > limit {
        return Err(ControlError::new(
            ErrorCode::InsufficientPermissions,
            format!(
                "refused: this child would sit at depth {depth} and the limit is {limit}. Set \
                 WARP_FORK_AGENT_SPAWN_DEPTH to change it."
            ),
        ));
    }

    let locations = surface_locations(ctx);
    let parent = BlocklistAIHistoryModel::as_ref(ctx)
        .terminal_surface_id_for_conversation(&parent_conversation_id)
        .and_then(|id| locations.get(&id))
        .ok_or_else(|| {
            ControlError::new(
                ErrorCode::MissingTarget,
                "the parent conversation has no pane; a child is spawned beside its parent, so \
                 there has to be one",
            )
        })?;
    let pane_group = parent.pane_group.clone();
    let parent_pane_id = parent.pane_id;

    let name = params
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .unwrap_or_default();

    let conversation_id = pane_group.update(ctx, |pane_group, ctx| {
        pane_group.spawn_hidden_child_agent_for_local_control(
            parent_pane_id,
            parent_conversation_id,
            name,
            params.prompt.clone(),
            allowed_tools.clone(),
            ctx,
        )
    });
    let conversation_id = conversation_id.ok_or_else(|| {
        ControlError::new(
            ErrorCode::Internal,
            "could not create a hidden pane for the child agent",
        )
    })?;

    let mut response = ack(instance_id, ActionKind::AgentSpawn);
    merge(
        &mut response,
        serde_json::to_value(AgentSpawnResult {
            conversation_id: conversation_id.to_string(),
            parent_conversation_id: parent_conversation_id.to_string(),
            depth,
            allowed_tools: allowed_tools.map(|tools| {
                tools
                    .into_iter()
                    .map(|tool| tool.as_str_name().to_owned())
                    .collect()
            }),
        })
        .map_err(|error| ControlError::new(ErrorCode::Internal, error.to_string()))?,
    );
    Ok(response)
}

/// `agent.cancel` — stop a turn.
///
/// An orchestrator that cannot stop a runaway child is not in charge of it.
/// This is Stop and not Kill: the conversation survives, its transcript stays
/// readable through `agent.read`, and the pane is not discarded. Killing a
/// child is a heavier thing and stays where it is, behind a person and a menu.
pub fn agent_cancel(
    instance_id: &Option<InstanceId>,
    params: &serde_json::Value,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    let params: AgentCancelParams = serde_json::from_value(params.clone())
        .map_err(|error| ControlError::new(ErrorCode::InvalidParams, error.to_string()))?;
    let conversation_id = parse_conversation_id(&params.conversation_id)?;
    let locations = surface_locations(ctx);

    let history = BlocklistAIHistoryModel::as_ref(ctx);
    let Some(conversation) = history.conversation(&conversation_id) else {
        return Err(unknown_conversation(&params.conversation_id));
    };
    let status = status_name(conversation.status()).to_owned();
    let was_running = matches!(conversation.status(), ConversationStatus::InProgress);
    let terminal_view = history
        .terminal_surface_id_for_conversation(&conversation_id)
        .and_then(|id| locations.get(&id))
        .map(|location| location.terminal_view.clone());

    if was_running {
        // Only required when there is something to stop. A finished
        // conversation outlives the pane that showed it, and refusing to
        // acknowledge a no-op cancel because the pane has gone would make the
        // ordinary end of a child's life look like an error.
        let terminal_view = terminal_view.ok_or_else(|| {
            ControlError::new(
                ErrorCode::MissingTarget,
                format!(
                    "conversation `{}` is running but the pane that owns it is gone; there is \
                     nothing here to stop",
                    params.conversation_id
                ),
            )
        })?;
        terminal_view.update(ctx, |terminal_view, ctx| {
            terminal_view.stop_agent_conversation_from_local_control(conversation_id, ctx);
        });
    }

    let mut response = ack(instance_id, ActionKind::AgentCancel);
    merge(
        &mut response,
        serde_json::to_value(AgentCancelResult {
            conversation_id: conversation_id.to_string(),
            was_running,
            status,
        })
        .map_err(|error| ControlError::new(ErrorCode::Internal, error.to_string()))?,
    );
    Ok(response)
}

/// `agent.settle` — mark a thread dealt with, or bring it back (T8.3).
///
/// Settling keeps a thread and moves it to the bottom of the inbox; it is not
/// a delete, and the promise that it will still be there later is enforced in
/// `select_conversations_to_evict`, which exempts settled rows from the
/// 200-conversation cap.
///
/// Unlike `agent.cancel`, this accepts a conversation that is **not loaded** —
/// checked against the metadata cache as well as the live map. That is the
/// normal case rather than an edge case: the threads worth settling are the
/// ones nobody has opened this session, and requiring a load to change one bit
/// would make the verb useless for exactly its intended target.
pub fn agent_settle(
    instance_id: &Option<InstanceId>,
    params: &serde_json::Value,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    let params: AgentSettleParams = serde_json::from_value(params.clone())
        .map_err(|error| ControlError::new(ErrorCode::InvalidParams, error.to_string()))?;
    let conversation_id = parse_conversation_id(&params.conversation_id)?;

    {
        let history = BlocklistAIHistoryModel::as_ref(ctx);
        let known = history.conversation(&conversation_id).is_some()
            || history
                .get_conversation_metadata(&conversation_id)
                .is_some();
        if !known {
            return Err(unknown_conversation(&params.conversation_id));
        }
    }

    let settled = params.settled;
    let changed = BlocklistAIHistoryModel::handle(ctx).update(ctx, |history, ctx| {
        history.set_conversation_settled(conversation_id, settled, ctx)
    });

    let mut response = ack(instance_id, ActionKind::AgentSettle);
    merge(
        &mut response,
        serde_json::to_value(AgentSettleResult {
            conversation_id: conversation_id.to_string(),
            settled,
            changed,
        })
        .map_err(|error| ControlError::new(ErrorCode::Internal, error.to_string()))?,
    );
    Ok(response)
}

/// Whether a reveal target needs the conversation to have a child-agent pane.
///
/// `pane` and `tab` both go through the child-agent machinery — they reuse the
/// hidden pane rather than building a new one, which is what preserves an
/// in-flight turn and its transcript. A conversation that was never spawned as
/// a child has no such pane, and the events would find nothing and log. `swap`
/// is the general one: it navigates to any conversation, falling back to
/// workspace-level focus when the conversation is in another tab.
fn reveal_target_requires_child_pane(target: AgentRevealTarget) -> bool {
    match target {
        AgentRevealTarget::Pane | AgentRevealTarget::Tab => true,
        AgentRevealTarget::Swap => false,
    }
}

/// `agent.reveal` — put a background child agent on screen.
///
/// The other half of spawning one hidden. Every failure the reveal events
/// report by logging a warning is checked here first, because they are events:
/// nothing comes back from emitting one, so a caller told "revealed" after a
/// warning was logged would have no way to find out otherwise.
pub fn agent_reveal(
    instance_id: &Option<InstanceId>,
    params: &serde_json::Value,
    target: &TargetSelector,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    let params: AgentRevealParams = serde_json::from_value(params.clone())
        .map_err(|error| ControlError::new(ErrorCode::InvalidParams, error.to_string()))?;
    let conversation_id = parse_conversation_id(&params.conversation_id)?;
    let locations = surface_locations(ctx);

    let surface_id = {
        let history = BlocklistAIHistoryModel::as_ref(ctx);
        if history.conversation(&conversation_id).is_none() {
            return Err(unknown_conversation(&params.conversation_id));
        }
        history.terminal_surface_id_for_conversation(&conversation_id)
    };
    let location = surface_id
        .and_then(|id| locations.get(&id))
        .ok_or_else(|| {
            ControlError::new(
                ErrorCode::MissingTarget,
                format!(
                    "no pane hosts conversation `{}`; `agent.list` reports `pane_id` for the ones \
                 that have one",
                    params.conversation_id
                ),
            )
        })?;
    let was_hidden = location.is_hidden;
    let conversation_tab_id = location.tab_id.clone();
    let conversation_pane_group = location.pane_group.clone();
    let is_child_agent = conversation_pane_group.read(ctx, |pane_group, _| {
        pane_group
            .child_agent_pane_for_conversation(&conversation_id)
            .is_some()
    });

    if !is_child_agent && reveal_target_requires_child_pane(params.target) {
        return Err(ControlError::new(
            ErrorCode::TargetStateConflict,
            format!(
                "conversation `{}` is not a background child agent, so there is no hidden pane \
                 to move; `swap` navigates to a conversation that already has one",
                params.conversation_id
            ),
        ));
    }

    // With no selector, the host is the focused pane of the tab that already
    // holds the conversation — not the app's active pane.
    //
    // Every other action defaults to the active pane because it has no better
    // idea; this one knows exactly where the conversation is. Deferring to the
    // active pane instead would make `agent reveal <id>` fail whenever the
    // person had looked at another tab since, and the fix — passing both
    // `--tab` and `--pane` — would be something a caller has to learn by
    // hitting it, since the pane selector resolves inside the active tab.
    let host = if target.window.is_none()
        && target.tab.is_none()
        && target.pane.is_none()
        && target.session.is_none()
    {
        conversation_pane_group
            .read(ctx, |pane_group, ctx| {
                pane_group.terminal_view_from_pane_id(pane_group.focused_pane_id(ctx), ctx)
            })
            .ok_or_else(|| {
                ControlError::new(
                    ErrorCode::MissingTarget,
                    "the tab holding this conversation has no terminal pane to reveal it from",
                )
            })?
    } else {
        terminal_view_for(ActionKind::AgentReveal, target, ctx)?
    };
    // The reveal events are scoped to one pane group: they ask *this* group
    // for the child's hidden pane. Emitting from a pane in another tab finds
    // nothing and logs, so the mismatch is refused here where it can be
    // explained. For `swap` the targeted pane is also the pane being replaced,
    // which is the other reason the caller gets to name it.
    let host_tab_id = locations
        .get(&host.id())
        .map(|location| location.tab_id.clone());
    if host_tab_id.as_deref() != Some(conversation_tab_id.as_str()) {
        return Err(ControlError::new(
            ErrorCode::TargetStateConflict,
            format!(
                "conversation `{}` lives in tab {conversation_tab_id} and the targeted pane is \
                 not in it; target a pane in that tab",
                params.conversation_id
            ),
        ));
    }

    let reveal_target = match params.target {
        AgentRevealTarget::Pane => LocalControlRevealTarget::Split,
        AgentRevealTarget::Tab => LocalControlRevealTarget::Tab,
        AgentRevealTarget::Swap => LocalControlRevealTarget::Swap,
    };
    host.update(ctx, |terminal_view, ctx| {
        terminal_view.reveal_agent_conversation_from_local_control(
            conversation_id,
            reveal_target,
            ctx,
        );
    });

    let mut response = ack(instance_id, ActionKind::AgentReveal);
    merge(
        &mut response,
        serde_json::to_value(AgentRevealResult {
            conversation_id: conversation_id.to_string(),
            was_hidden,
            target: params.target,
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
        terminal_view.run_slash_command_from_local_control(command, argument.clone(), ctx)
    });

    // `false` here is not failure. `/compact`, `/plan` and `/orchestrate`
    // return it *deliberately*: they are not actions, they are prompts, and the
    // meaning is "send my text to the agent and let the downstream handler
    // recognise the prefix". The keyboard path says as much in a comment and
    // falls through to `send_queued_user_query_in_conversation`; this is the
    // same fall-through.
    //
    // Reconstructing `/compact <argument>` rather than passing the argument
    // alone is the point — the prefix *is* the instruction.
    let submitted_as_prompt = !handled && slash_command_is_submitted_as_prompt(command);
    let conversation_id = if submitted_as_prompt {
        let prompt = match argument.as_deref().map(str::trim).filter(|a| !a.is_empty()) {
            Some(argument) => format!("{} {argument}", command.name),
            None => command.name.to_owned(),
        };
        // Into the conversation in front of the pane, not a new one: compacting
        // a fresh conversation would compact nothing.
        let conversation_id = terminal_view.read(ctx, |terminal_view, ctx| {
            terminal_view.selected_conversation_for_local_control(ctx)
        });
        terminal_view.update(ctx, |terminal_view, ctx| {
            terminal_view.start_agent_conversation_from_local_control(prompt, conversation_id, ctx)
        })
    } else {
        None
    };

    let mut response = ack(instance_id, ActionKind::SlashRun);
    if let Some(object) = response.as_object_mut() {
        object.insert(
            "command".to_owned(),
            serde_json::Value::String(name.to_owned()),
        );
        // Reported as-is rather than translated into a success this cannot
        // vouch for: a command can be handled and then raise an error toast.
        object.insert("handled".to_owned(), serde_json::Value::Bool(handled));
        object.insert(
            "submitted_as_prompt".to_owned(),
            serde_json::Value::Bool(submitted_as_prompt),
        );
        if let Some(conversation_id) = conversation_id {
            object.insert(
                "conversation_id".to_owned(),
                serde_json::Value::String(conversation_id.to_string()),
            );
        }
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
