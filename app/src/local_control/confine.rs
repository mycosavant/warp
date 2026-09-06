//! What a credential confined to one conversation may do (2026-09-05, the
//! fork's `/remote-control`).
//!
//! A device paired by a code minted *for one conversation* holds credentials
//! whose grant names it (`CredentialGrant::conversation`). That is what lets
//! such a device be granted `agent.prompt` and `agent.approve` at all; this
//! module is the check that the name is honoured. Two halves:
//!
//! * [`ensure_within`] runs before a handler. An action that names a
//!   conversation must name *this* one; an action that names an approval must
//!   name one of this conversation's; an action that starts a new conversation
//!   is refused, because a new conversation is not the one that was handed
//!   over; and anything outside the small set a confined device could have
//!   been granted is refused, so a future widening of the pairable list does
//!   not widen this by accident.
//! * [`confine_result`] runs after. The reads a confined device holds
//!   (`agent.list`, `agent.approvals`) answer for every agent in the instance,
//!   and a phone handed one conversation should not learn the titles of the
//!   others; the arrays are filtered to that one. The event stream is filtered
//!   the same way by [`line_is_within`], on the line's `session_id`, which is
//!   Warp's conversation id.
//!
//! An unconfined grant -- every local client, every device paired by
//! `warpctrl pair show` -- passes through both untouched.
use ::local_control::auth::CredentialGrant;
use ::local_control::protocol::{
    Action, ActionKind, AgentApproveParams, AgentCancelParams, AgentPromptParams, AgentTraceParams,
    ControlError, ErrorCode,
};
use serde_json::Value;

/// Refuses an action a confined credential may not perform.
///
/// `approval_owner` answers which conversation a pending approval belongs to,
/// or `None` for one that is not a conversation's (a pane agent's) or does not
/// exist; passed in so the rule can be asserted without an app.
pub(super) fn ensure_within(
    action: &Action,
    grant: &CredentialGrant,
    approval_owner: impl Fn(&str) -> Option<String>,
) -> Result<(), ControlError> {
    let Some(conversation) = grant.conversation.as_deref() else {
        return Ok(());
    };
    let refuse = |what: String| {
        Err(ControlError::new(
            ErrorCode::InsufficientPermissions,
            format!(
                "this device was paired for conversation {conversation} only and may not {what}"
            ),
        ))
    };
    match action.kind {
        // Instance-wide reads a confined device holds; their results are
        // filtered by `confine_result` and the stream by `line_is_within`.
        ActionKind::AppPing
        | ActionKind::AgentList
        | ActionKind::EventsSubscribe
        | ActionKind::AgentApprovals => Ok(()),
        ActionKind::AgentPrompt => {
            let params: AgentPromptParams = action.params_as()?;
            match params.conversation_id.as_deref() {
                Some(named) if named == conversation => Ok(()),
                Some(other) => refuse(format!("prompt conversation {other}")),
                None => refuse("start a new conversation".to_owned()),
            }
        }
        ActionKind::AgentTrace => {
            let params: AgentTraceParams = action.params_as()?;
            if params.conversation_id == conversation {
                Ok(())
            } else {
                refuse(format!("read conversation {}", params.conversation_id))
            }
        }
        ActionKind::AgentCancel => {
            let params: AgentCancelParams = action.params_as()?;
            if params.conversation_id == conversation {
                Ok(())
            } else {
                refuse(format!("cancel conversation {}", params.conversation_id))
            }
        }
        ActionKind::AgentApprove | ActionKind::AgentDeny => {
            let params: AgentApproveParams = action.params_as()?;
            match approval_owner(&params.approval_id) {
                Some(owner) if owner == conversation => Ok(()),
                Some(other) => refuse(format!(
                    "answer request {}, which belongs to conversation {other}",
                    params.approval_id
                )),
                None => refuse(format!(
                    "answer request {}, which is not one of that conversation's",
                    params.approval_id
                )),
            }
        }
        other => refuse(format!("invoke {}", other.as_str())),
    }
}

/// Filters an instance-wide read's answer down to the confined conversation.
///
/// Touches only the two arrays a confined device can receive; anything else is
/// returned as the handler built it. Written against the field names the
/// protocol types serialize to, and pinned by a test that builds the real
/// types, so a rename there fails here rather than silently filtering nothing.
pub(super) fn confine_result(kind: ActionKind, data: &mut Value, grant: &CredentialGrant) {
    let Some(conversation) = grant.conversation.as_deref() else {
        return;
    };
    let key = match kind {
        ActionKind::AgentList => "conversations",
        ActionKind::AgentApprovals => "approvals",
        _ => return,
    };
    if let Some(items) = data.get_mut(key).and_then(Value::as_array_mut) {
        items.retain(|item| {
            item.get("conversation_id").and_then(Value::as_str) == Some(conversation)
        });
    }
}

/// Whether one line of the event log belongs to the confined conversation.
///
/// Unconfined grants see everything. A line that does not parse, or names no
/// `session_id`, is withheld from a confined device rather than shown: the
/// only lines such a device is entitled to are the ones that say whose they
/// are.
pub(super) fn line_is_within(line: &str, conversation: Option<&str>) -> bool {
    let Some(conversation) = conversation else {
        return true;
    };
    serde_json::from_str::<Value>(line)
        .ok()
        .and_then(|value| {
            value
                .get("session_id")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .is_some_and(|session| session == conversation)
}

#[cfg(test)]
#[path = "confine_tests.rs"]
mod tests;
