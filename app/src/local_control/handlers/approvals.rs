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

/// The two values of [`PendingApproval::source`], written once so the server and
/// its tests cannot drift.
///
/// Stated rather than derived, because a client that inferred the population
/// from the shape of an `approval_id` or from whether `tab_id` is set would be
/// reading a structural fact off an incidental field — and it needs the answer
/// to label `cwd`, which means two different things in the two populations.
const SOURCE_PANE: &str = "pane";
const SOURCE_ACP: &str = "acp";

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

    // The ACP population first, because its ids are JSON-RPC request ids and
    // would never match a pane — so a miss here costs one map lookup and falling
    // through is correct, while the other order would report `no_such_approval`
    // for a request that is sitting right there.
    if let Some(answered) = answer_acp(instance_id, decision, &params, ctx)? {
        return Ok(answered);
    }

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
    approvals.extend(acp_approvals());
    // Stable across calls, because a `HashMap` walk is not and a caller
    // rendering a list should not have it reshuffle under them between polls.
    approvals.sort_by(|left, right| left.approval_id.cmp(&right.approval_id));
    approvals
}

/// Answers an ACP permission request, or reports that this id is not one.
///
/// `Ok(None)` means "not mine, try the panes". Everything else is a final
/// answer, including the refusal of a yes.
#[cfg(not(target_family = "wasm"))]
fn answer_acp(
    instance_id: &Option<InstanceId>,
    decision: Decision,
    params: &AgentApproveParams,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<Option<serde_json::Value>, ControlError> {
    let _ = ctx;
    let Some(current) = acp_approvals()
        .into_iter()
        .find(|approval| approval.approval_id == params.approval_id)
    else {
        return Ok(None);
    };

    // Same order as the pane path, and for the same reason: a caller holding a
    // stale digest is asking about a request that is gone, and telling them
    // "Warp cannot say yes yet" would send them off to solve the wrong problem.
    if current.digest != params.digest {
        return Err(ControlError::new(
            ErrorCode::InvalidParams,
            format!(
                "the request `{}` is not the one this digest was taken from; \
                 read `agent.approvals` again and answer what it reports now",
                params.approval_id
            ),
        ));
    }

    // **The entry's own refusal, not the population's.** This used to reject
    // every ACP request with one sentence; the sentence has since gone false for
    // the requests `acp_permission` can bound to a single call. `can_approve` is
    // frozen at park time from that same decision, so what is refused here and
    // what the listing showed cannot disagree — which is the bug this module
    // already fixed once, one population over.
    if decision == Decision::Allow && !current.can_approve {
        return Err(ControlError::new(
            ErrorCode::InsufficientPermissions,
            current
                .approve_refused_because
                .clone()
                .unwrap_or_else(|| ACP_APPROVE_NOT_APPROVABLE.to_owned()),
        ));
    }

    if !crate::ai::acp_agent::registry::answer(
        &params.approval_id,
        match decision {
            Decision::Allow => crate::ai::acp_agent::registry::Decision::Allow,
            Decision::Deny => crate::ai::acp_agent::registry::Decision::Deny,
        },
    ) {
        // Between the read above and here the turn ended — cancelled, or the
        // agent went away. The question is gone rather than unanswered.
        return Err(no_such_approval(&params.approval_id));
    }

    let mut response = ack(instance_id, decision.action());
    merge(
        &mut response,
        serde_json::to_value(AgentApproveResult {
            approval_id: params.approval_id.clone(),
            decision: decision.as_str().to_owned(),
            agent: current.agent,
            // **Not a keystroke — the option id that actually went back to the
            // agent.** The CLI path presses a key because pressing a key is all
            // it can do, and reports *which* key for the same reason this
            // reports which option: a result claiming `approved: true` would
            // assert an effect this process cannot observe. Here the answer is
            // typed and carries an id, so naming it is both more precise and
            // more honest than naming a keystroke that was never sent.
            keystroke: match decision {
                Decision::Allow => current
                    .approve_selects
                    .clone()
                    .unwrap_or_else(|| "allow".to_owned()),
                Decision::Deny => "reject_once".to_owned(),
            },
        })
        .map_err(|error| ControlError::new(ErrorCode::Internal, error.to_string()))?,
    )?;
    Ok(Some(response))
}

#[cfg(target_family = "wasm")]
fn answer_acp(
    _instance_id: &Option<InstanceId>,
    _decision: Decision,
    _params: &AgentApproveParams,
    _ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<Option<serde_json::Value>, ControlError> {
    Ok(None)
}

/// Why `agent.approve` refuses an ACP request, in the sentence a person reads.
///
/// Not the same refusal as an unverified CLI agent's, and deliberately so: that
/// one is about *this fork not knowing what Enter would select on someone
/// else's TUI*, which ACP does not have — its options are typed and carry ids.
/// This one is about there being no surface that could show what a yes would
/// allow, which is what T14.6 is for.
/// The fallback sentence for an unapprovable ACP entry that carries no reason of
/// its own.
///
/// Should be unreachable: `acp_permission` writes a reason for every refusal it
/// makes, and the one refusal this fork adds writes its own. It exists because
/// an empty explanation on a screen showing only *No* is worse than a vague one
/// — the person cannot tell a setting from a fault, which is the whole argument
/// for `approve_refused_because` existing.
const ACP_APPROVE_NOT_APPROVABLE: &str =
    "Warp will not say yes to this request. `deny` works, and so does cancelling the turn.";

/// The ACP permission requests currently waiting on a person (T14.6).
///
/// The second population `agent.approvals` reports, after the CLI-agent panes
/// this module was written for. They arrive by a completely different route — a
/// JSON-RPC request parked mid-turn rather than a prompt drawn on a PTY — and
/// the only thing they share is that a person is what unblocks them, which is
/// exactly what this action is for.
#[cfg(not(target_family = "wasm"))]
fn acp_approvals() -> Vec<PendingApproval> {
    crate::ai::acp_agent::registry::waiting()
        .into_iter()
        .map(|parked| {
            let mut approval = PendingApproval {
                approval_id: parked.approval_id,
                agent: parked.agent,
                source: SOURCE_ACP.to_owned(),
                kind: "permission".to_owned(),
                summary: parked.title,
                tool_name: parked.tool_name,
                tool_input: parked.tool_input,
                // Warp's own fact: it chose this directory from the pane and sent
                // it in `session/new`. It is **not** a claim about whose rules
                // governed the call — measured on T14.6, nothing on the wire says
                // that, and the directory only decides which config the agent
                // *looked* for.
                cwd: parked.session_directory,
                project: None,
                session_id: parked.session_id,
                tab_id: None,
                // The agent's own claim about where this call acts, recovered by
                // the `toolCallId` join — and left empty rather than defaulted to
                // `cwd` when it never said one.
                acts_on: parked.acts_on,
                digest: String::new(),
                // Read off the decision `acp_permission` froze at park time, so
                // the listing cannot promise a yes the answer path would refuse.
                can_approve: parked.approve_selects.is_some(),
                approve_selects: parked.approve_selects,
                approve_refused_because: parked.approve_refused_because,
                options_offered: parked.options_offered,
            };
            approval.digest = digest_of(&approval);
            approval
        })
        .collect()
}

#[cfg(target_family = "wasm")]
fn acp_approvals() -> Vec<PendingApproval> {
    Vec::new()
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
        source: SOURCE_PANE.to_owned(),
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
        // An OSC notification carries a tool name and a command preview and
        // never a location list, so there is nothing to report here.
        acts_on: Vec::new(),
        digest: String::new(),
        // Set before the digest is taken, and excluded from it — see
        // `PendingApproval::can_approve`. Whether Warp would accept a yes is not
        // part of the question the agent asked.
        can_approve: refusal.is_none(),
        approve_refused_because: refusal,
        // A keystroke has no option id; this path presses Return.
        approve_selects: None,
        // A CLI agent's prompt is drawn on its own terminal; Warp sees a status
        // and a tool name over OSC 777 and never the options themselves.
        options_offered: Vec::new(),
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
        // In the hash because it changes what the person was *shown*: the
        // console labels `cwd` from it, and the two labels are different claims.
        Some(approval.source.as_str()),
        Some(approval.kind.as_str()),
        approval.summary.as_deref(),
        approval.tool_name.as_deref(),
        approval.tool_input.as_deref(),
        approval.cwd.as_deref(),
        approval.project.as_deref(),
        approval.session_id.as_deref(),
        // **In the hash, unlike its neighbours `can_approve` and
        // `approve_refused_because`**, and what separates them is what the field
        // describes. Those two are Warp's policy — folding them in would move a
        // digest without the agent's question having changed. This is the
        // *answer*: the id a yes sends back. An entry that would select a
        // different option is a different thing to agree to, so a digest taken
        // before that changed must not still fit.
        approval.approve_selects.as_deref(),
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
    // Two lists, hashed the same way and for the same reason as the fields
    // above: each is part of what the person was shown, so an answer is bound to
    // it. The options because an agent that re-asks offering a different set is
    // asking something else; the paths because "run this" and "run this *here*"
    // are different questions, and where a call acts is the fact T14.6 measured
    // as deciding whether anyone was asked at all. Counted first, then each
    // entry length-prefixed, so no arrangement of one list can collide with
    // another.
    for list in [&approval.options_offered, &approval.acts_on] {
        hasher.update((list.len() as u64).to_le_bytes());
        for entry in list {
            hasher.update((entry.len() as u64).to_le_bytes());
            hasher.update(entry.as_bytes());
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
