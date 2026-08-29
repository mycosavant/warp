//! `agent.approvals` and `agent.approve` — answering the thing that is actually
//! blocked (`.fork/TASKS.md`, T11.5).
//!
//! # The population `agent.list` cannot see
//!
//! T11.2 built a read surface and T11.4 let a phone reach it, and both of them
//! report *Warp's* conversations: `agent.list` walks
//! [`BlocklistAIHistoryModel::all_live_conversations`], which knows about
//! `AIConversation`s and nothing else.
//!
//! A `claude` running in a Warp pane is not one of those. It is a process on a
//! PTY that Warp watches over OSC 777, tracked in
//! [`CLIAgentSessionsModel`](crate::terminal::cli_agent_sessions::CLIAgentSessionsModel)
//! — a different map, keyed by terminal view, holding a status, a tool name and
//! a command. On this fork that is the agent path a person actually uses, so the
//! thing most likely to be waiting on them was the one thing `warpctrl` could
//! not see at all. A phone that connected *after* the request arrived saw an
//! empty snapshot; only the live event stream carried it, and only if you were
//! already watching.
//!
//! # Approval is a keystroke here, and saying otherwise would be a lie
//!
//! Warp has no channel to tell a CLI agent "approved". The agent drew a prompt
//! on its own terminal and is reading its own stdin; the only thing a person in
//! the chair can do is press a key. So `agent.approve` presses Return and
//! `agent.deny` presses Escape, and the result says which, because a result that
//! said `approved: true` would be claiming knowledge this process does not have.
//! Confirm by reading `agent.approvals` again: an answer that landed makes the
//! entry disappear.
//!
//! This is also why `agent.approve` is refused for agents not named in
//! [`ALLOW_VERIFIED_AGENTS`]. Return means "take the highlighted option", and
//! which option that is, is a fact about someone else's TUI; for the rest the
//! honest answer is that nobody here knows, and pressing Return to find out is
//! not a thing to do on someone's behalf from across a network. `agent.deny` is
//! allowed for all of them, because Escape's worst case is that nothing happens
//! — and the caller can see from the next `agent.approvals` that nothing
//! happened.
//!
//! # What binds an answer to the question
//!
//! Between a phone rendering "run `rm -rf build/`?" and a thumb landing on
//! *approve*, the agent may have been answered locally, moved on, and asked
//! something else. The pane id alone would carry the yes onto whatever is there
//! now. So every approval carries a digest of exactly what was shown, and both
//! answering actions require it back: a stale answer is refused instead of
//! misapplied. That is the same shape as `tool_digest.rs`, for the same reason —
//! "did you approve *this*?" is only answerable if you kept what *this* was.
//!
//! # What is deliberately not here
//!
//! **Warp's own blocked conversations.** `BlocklistAIActionModel` has a real
//! accept path (`execute_action` / `cancel_action_with_id`) and it is what the
//! TUI's permission prompt calls. It is also unreachable on this fork: the
//! agent panel is served by `ai::local_agent`, which — by its own module
//! docs — never returns a `ToolCall`, because Claude runs its own tools. No
//! `ToolCall` means no queued action, which means no confirmation, which means
//! `ConversationStatus::Blocked` never arrives on that path. Building the
//! branch anyway would produce exactly the artefact this fork was started over:
//! a feature that exists, is tested, and is never reached. `agent.list` already
//! reports `blocked` for those conversations if they ever do appear.
use ::local_control::protocol::{
    AgentApprovalsResult, AgentApproveParams, AgentApproveResult, PendingApproval,
};
use ::local_control::{ActionKind, ControlError, ErrorCode, InstanceId};
use sha2::{Digest as _, Sha256};
use warpui::{ModelContext, SingletonEntity};

use crate::local_control::LocalControlBridge;
use crate::local_control::handlers::ack;
use crate::local_control::handlers::agent::{SurfaceLocation, surface_locations};
use crate::terminal::CLIAgent;
use crate::terminal::cli_agent_sessions::{
    CLIAgentSession, CLIAgentSessionStatus, CLIAgentSessionsModel,
};

/// Bytes written for each decision.
///
/// A carriage return rather than a line feed because that is what a terminal
/// sends for the Return key; a TUI reading raw stdin gets `\r`, and `\n` is a
/// different byte that some line editors treat as a literal newline in the
/// buffer.
const ALLOW_BYTES: &[u8] = b"\r";
const DENY_BYTES: &[u8] = b"\x1b";

/// Agents whose permission prompt is known to highlight *yes*, so pressing
/// Return means what a caller asking for `agent.approve` meant.
///
/// One entry, and it should stay hard to add to: the cost of being wrong is
/// approving something on a person's behalf that they would have refused.
///
/// **What "known" rests on, stated because it is weaker than the rest of this
/// module.** Claude Code's prompt documents option 1 as *Yes* and preselects it,
/// and labels its reject option `(esc)`. T11.5's live runs exercised the whole
/// path — the byte `0d` was observed reaching a pane's PTY — against a
/// *synthesised* permission notification rather than a real `claude`, so what
/// this entry asserts is Claude's documented prompt, not a prompt this fork
/// watched. It is the first claim here to re-check, and the cheapest check is to
/// answer one real prompt with `warpctrl agent approve` and see whether the tool
/// runs.
const ALLOW_VERIFIED_AGENTS: &[CLIAgent] = &[CLIAgent::Claude];

/// Which key `agent.approve` and `agent.deny` press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Decision {
    Allow,
    Deny,
}

impl Decision {
    fn bytes(self) -> &'static [u8] {
        match self {
            Self::Allow => ALLOW_BYTES,
            Self::Deny => DENY_BYTES,
        }
    }

    fn keystroke(self) -> &'static str {
        match self {
            Self::Allow => "enter",
            Self::Deny => "escape",
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
        }
    }

    fn action(self) -> ActionKind {
        match self {
            Self::Allow => ActionKind::AgentApprove,
            Self::Deny => ActionKind::AgentDeny,
        }
    }
}

/// `agent.approvals` — everything waiting on a person right now.
pub fn agent_approvals(
    instance_id: &Option<InstanceId>,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    let approvals = pending_approvals(ctx);
    let mut response = ack(instance_id, ActionKind::AgentApprovals);
    merge(
        &mut response,
        serde_json::to_value(AgentApprovalsResult { approvals })
            .map_err(|error| ControlError::new(ErrorCode::Internal, error.to_string()))?,
    )?;
    Ok(response)
}

/// `agent.approve` / `agent.deny` — press the key a person in the chair would
/// have pressed.
pub fn agent_answer(
    instance_id: &Option<InstanceId>,
    decision: Decision,
    params: &serde_json::Value,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    let params: AgentApproveParams = serde_json::from_value(params.clone())
        .map_err(|error| ControlError::new(ErrorCode::InvalidParams, error.to_string()))?;

    let locations = surface_locations(ctx);
    let Some((&terminal_view_id, location)) = locations
        .iter()
        .find(|(_, location)| location.pane_id.to_string() == params.approval_id)
    else {
        return Err(no_such_approval(&params.approval_id));
    };
    let terminal_view = location.terminal_view.clone();
    let tab_id = location.tab_id.clone();
    let pane_id = location.pane_id.to_string();

    let session = CLIAgentSessionsModel::as_ref(ctx)
        .session(terminal_view_id)
        .cloned();
    let Some(current) = session
        .as_ref()
        .and_then(|session| approval_for(session, &pane_id, &tab_id))
    else {
        return Err(no_such_approval(&params.approval_id));
    };

    // Before the agent check, because a caller holding a stale digest is asking
    // about a request that is gone, and telling them "this agent is unverified"
    // would send them off to solve the wrong problem.
    if current.digest != params.digest {
        return Err(ControlError::new(
            ErrorCode::InvalidParams,
            format!(
                "the request on pane `{}` is not the one this digest was taken from; \
                 read `agent.approvals` again and answer what it reports now",
                params.approval_id
            ),
        ));
    }

    if decision == Decision::Allow {
        let agent = session
            .as_ref()
            .map(|session| session.agent)
            .expect("a pending approval implies a session");
        if !ALLOW_VERIFIED_AGENTS.contains(&agent) {
            return Err(unverified_agent(agent));
        }
    }

    let written = terminal_view.update(ctx, |terminal_view, ctx| {
        terminal_view.press_key_for_local_control(decision.bytes(), ctx)
    });
    if !written {
        return Err(ControlError::new(
            ErrorCode::Internal,
            format!(
                "pane `{}` refused the keystroke; its active block is under Warp's own agent \
                 control, so nothing was sent and the CLI agent is still waiting",
                params.approval_id
            ),
        ));
    }

    let mut response = ack(instance_id, decision.action());
    merge(
        &mut response,
        serde_json::to_value(AgentApproveResult {
            approval_id: params.approval_id,
            decision: decision.as_str().to_owned(),
            agent: current.agent,
            keystroke: decision.keystroke().to_owned(),
        })
        .map_err(|error| ControlError::new(ErrorCode::Internal, error.to_string()))?,
    )?;
    Ok(response)
}

fn pending_approvals(ctx: &mut ModelContext<LocalControlBridge>) -> Vec<PendingApproval> {
    let locations = surface_locations(ctx);
    let sessions = CLIAgentSessionsModel::as_ref(ctx);
    let mut approvals = locations
        .iter()
        .filter_map(|(&terminal_view_id, location): (_, &SurfaceLocation)| {
            let session = sessions.session(terminal_view_id)?;
            approval_for(session, &location.pane_id.to_string(), &location.tab_id)
        })
        .collect::<Vec<_>>();
    // Stable across calls, because a `HashMap` walk is not and a caller
    // rendering a list should not have it reshuffle under them between polls.
    approvals.sort_by(|left, right| left.approval_id.cmp(&right.approval_id));
    approvals
}

/// The pending approval a session represents, if it is waiting on a person.
///
/// Split from the walk so the shape of an approval can be asserted without a
/// pane group, a window, or a running app.
///
/// # The trap this function exists to avoid, found by running it
///
/// The obvious implementation reads `tool_name` and `tool_input_preview` off the
/// session context whenever the status is `Blocked`. That is wrong, and the
/// first live run proved it: a `permission_request` sets those three fields, and
/// a `question_asked` arriving afterwards sets `Blocked` **without clearing
/// them** — `clear_permission_scoped_state` runs on `tool_complete`,
/// `permission_replied`, `prompt_submit` and `stop`, and a question is none of
/// those.
///
/// So an agent that had asked to run `rm -rf build/` and then asked "which
/// database should I use?" was reported as still asking to run `rm -rf build/`
/// — *with an unchanged digest*, so the stale-answer check was satisfied and a
/// remote yes would have been accepted onto the wrong question. The digest was
/// hashing a field that had gone stale underneath it, which is the one failure
/// a digest exists to make impossible.
///
/// The fix is to take the summary from the *block* rather than from the context.
/// `Blocked { message }` is set by whichever event caused the current wait, so
/// it is the one field here guaranteed to describe **now**; the retained tool
/// fields are reported only when they agree with it.
fn approval_for(session: &CLIAgentSession, pane_id: &str, tab_id: &str) -> Option<PendingApproval> {
    let CLIAgentSessionStatus::Blocked { message } = &session.status else {
        return None;
    };
    let context = &session.session_context;
    // Compared rather than merely checked for presence, because a permission
    // request may carry no summary at all: `None == None` is a match and keeps
    // its tool fields, while a question — whose message falls back to "Waiting
    // for your answer" and so is never `None` — cannot collide with one.
    let tool_fields_describe_this_block = message.as_deref() == context.summary.as_deref();
    let (tool_name, tool_input) = if tool_fields_describe_this_block {
        (
            context.tool_name.clone(),
            context.tool_input_preview.clone(),
        )
    } else {
        (None, None)
    };
    let refusal = approve_refusal(session.agent);
    let mut approval = PendingApproval {
        approval_id: pane_id.to_owned(),
        agent: agent_name(session.agent).to_owned(),
        kind: if tool_name.is_some() {
            "permission"
        } else {
            "question"
        }
        .to_owned(),
        summary: message.clone(),
        tool_name,
        tool_input,
        cwd: context.cwd.clone(),
        project: context.project.clone(),
        session_id: context.session_id.clone(),
        tab_id: Some(tab_id.to_owned()),
        digest: String::new(),
        // Set before the digest is taken, and excluded from it — see
        // `PendingApproval::can_approve`. Whether Warp would accept a yes is not
        // part of the question the agent asked.
        can_approve: refusal.is_none(),
        approve_refused_because: refusal,
    };
    approval.digest = digest_of(&approval);
    Some(approval)
}

/// Hashes what a person was shown.
///
/// **Field-separated, not concatenated.** `tool_name: "Bash", tool_input: "ls"`
/// and `tool_name: "Bashl", tool_input: "s"` are different requests and must not
/// hash alike, so every field is length-prefixed. The alternative — joining with
/// a separator — only moves the problem to whichever separator is chosen, since
/// a command is arbitrary text and can contain it.
///
/// `approval_id` is in the hash even though `agent.approve` also matches on it,
/// so that a digest taken from one pane cannot be replayed against another.
fn digest_of(approval: &PendingApproval) -> String {
    let mut hasher = Sha256::new();
    for field in [
        Some(approval.approval_id.as_str()),
        Some(approval.agent.as_str()),
        Some(approval.kind.as_str()),
        approval.summary.as_deref(),
        approval.tool_name.as_deref(),
        approval.tool_input.as_deref(),
        approval.cwd.as_deref(),
        approval.project.as_deref(),
        approval.session_id.as_deref(),
    ] {
        match field {
            Some(value) => {
                hasher.update((value.len() as u64).to_le_bytes());
                hasher.update(value.as_bytes());
            }
            // Distinct from an empty string, which is a value the agent could
            // actually send.
            None => hasher.update(u64::MAX.to_le_bytes()),
        }
    }
    format!("{:x}", hasher.finalize())
}

/// The same name `WARP_FORK_EVENT_LOG` writes, so an approval and the log lines
/// for the session it came from can be lined up without a translation table.
fn agent_name(agent: CLIAgent) -> &'static str {
    agent.command_prefixes().first().copied().unwrap_or("?")
}

fn no_such_approval(approval_id: &str) -> ControlError {
    ControlError::new(
        ErrorCode::MissingTarget,
        format!(
            "nothing is waiting on pane `{approval_id}`; `agent.approvals` reports the requests \
             that exist right now"
        ),
    )
}

/// Why `agent.approve` would refuse this agent, or `None` if it would not.
///
/// **The single source of truth for approvability, and it exists because there
/// were two.** The listing reported every blocked session, `agent_answer`
/// refused unverified agents at the point of answering, and nothing carried that
/// refusal to whatever was drawing the list — so `console.js`, which decides its
/// *Yes* from the paired device's action list, drew one on rows that could never
/// be approved. Both callers now read this, so they cannot drift again.
fn approve_refusal(agent: CLIAgent) -> Option<String> {
    (!ALLOW_VERIFIED_AGENTS.contains(&agent)).then(|| {
        format!(
            "`allow` presses Enter, and this fork has not verified what {} highlights by default; \
             answer it at the keyboard, or use `deny`, which presses Escape",
            agent.display_name()
        )
    })
}

fn unverified_agent(agent: CLIAgent) -> ControlError {
    ControlError::new(
        ErrorCode::InsufficientPermissions,
        approve_refusal(agent).unwrap_or_else(|| {
            "this agent is verified; nothing should have called this".to_owned()
        }),
    )
}

/// Folds a typed result into the acknowledgement envelope.
///
/// A local copy rather than a shared helper for the same reason `agent.rs` has
/// one: it is four lines and exporting it would make two modules share a private
/// detail of how responses are shaped.
fn merge(response: &mut serde_json::Value, extra: serde_json::Value) -> Result<(), ControlError> {
    let (Some(response), Some(extra)) = (response.as_object_mut(), extra.as_object()) else {
        return Err(ControlError::new(
            ErrorCode::Internal,
            "local-control response envelope is not an object",
        ));
    };
    for (key, value) in extra {
        response.insert(key.clone(), value.clone());
    }
    Ok(())
}

#[cfg(test)]
#[path = "approvals_tests.rs"]
mod tests;
