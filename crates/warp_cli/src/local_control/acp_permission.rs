//! Choosing an option on an ACP `session/request_permission` (`.fork/TASKS.md`,
//! T14.2).
//!
//! # The bug this module exists because of
//!
//! T14.1's probe answered an approval by taking `options.first()`. Measured
//! 2026-08-27 against live `claude-agent-acp`, the options arrive in this order:
//!
//! ```text
//! 1  reject        "Deny"          reject_once
//! 2  allow         "Allow Once"    allow_once
//! 3  allow_always  "Always Allow"  allow_always   + _meta.permission.changes
//! ```
//!
//! **Deny is first.** So `--approve` denied: the run wrote no file and the agent
//! reported *"I wasn't able to create the file — the write was denied."* The flag
//! did the opposite of its name, silently, against the flagship agent. Nothing in
//! the protocol orders this list, and nothing ever will — the order is a fact
//! about one agent's UI, exactly like which option a TUI highlights
//! (`app/src/local_control/handlers/approvals.rs`, `ALLOW_VERIFIED_AGENTS`).
//! ACP's improvement over that path is not that the order is knowable; it is that
//! **the order does not have to be known**, because every option is typed.
//!
//! So: choose by [`PermissionOptionKind`], never by position. That is the whole
//! idea, and it is the reason a typed permission channel beats a screen-scraped
//! one for all 39 agents at once.
//!
//! # Failing closed, in both directions
//!
//! An allow that cannot be expressed safely becomes a denial, because a denial is
//! the answer that can only ever make less happen — the asymmetry T11.5
//! established for `agent.approve`/`agent.deny` and T13.3 restated for review
//! verdicts. A denial that cannot be expressed as `reject_once` is still
//! answerable: [`RequestPermissionOutcome::Cancelled`] is a no, and the live run
//! above confirms an agent treats it as one.
//!
//! # Why `allow_always` is refused rather than offered
//!
//! The third option above carries, in its `_meta`, a declaration that selecting
//! it sets Claude Code's permission mode to `acceptEdits` for the rest of the
//! session. Composed with `WARP_FORK_REMOTE_APPROVE`, one tap on a phone would
//! authorize every subsequent call the person will never be shown. The scope of a
//! remote *yes* should be one call, so the always-variants are not selectable
//! here at all.
//!
//! `_meta` is read only to *refuse*, never to grant, which is why reading it does
//! not violate the spec's "implementations MUST NOT make assumptions about
//! values at these keys". No assumption is made about what a change means; the
//! presence of the key is treated as "this option does more than answer the
//! question", and that is true whatever it says. An agent that mislabels
//! `allow_always` as `allow_once` is still caught, and an agent that attaches a
//! change to a genuinely single-shot option loses nothing but one round trip.

use agent_client_protocol::schema::v1::{
    PermissionOption, PermissionOptionId, PermissionOptionKind, RequestPermissionRequest,
};

/// Which way the caller wants to answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Decision {
    Allow,
    Deny,
}

/// What to send back, and what to tell the person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Choice {
    /// Answer with this option id.
    Select(PermissionOptionId),
    /// Nothing could be selected. Answer `Cancelled` — still a no — and say why.
    ///
    /// Carried as a message rather than an error because the exchange continues:
    /// the agent gets a well-formed refusal and the person gets a reason.
    Cancel { reason: String },
}

/// The `_meta` key an option uses to declare that selecting it changes policy.
const PERMISSION_META_KEY: &str = "permission";

/// Answer one permission request.
pub(super) fn choose(request: &RequestPermissionRequest, decision: Decision) -> Choice {
    let wanted = match decision {
        Decision::Allow => PermissionOptionKind::AllowOnce,
        Decision::Deny => PermissionOptionKind::RejectOnce,
    };

    match request
        .options
        .iter()
        .find(|option| option.kind == wanted && !changes_policy(option))
    {
        Some(option) => Choice::Select(option.option_id.clone()),
        None => Choice::Cancel {
            reason: no_option_reason(request, decision),
        },
    }
}

/// Whether selecting this option would do more than answer the question.
///
/// Two independent signals, either of which is enough: the kind says the choice
/// is remembered, or the option declares a policy change in its `_meta`. Neither
/// is trusted to grant anything — both only ever remove an option from
/// consideration.
pub(super) fn changes_policy(option: &PermissionOption) -> bool {
    matches!(
        option.kind,
        PermissionOptionKind::AllowAlways | PermissionOptionKind::RejectAlways
    ) || declared_changes(option).is_some()
}

/// The policy change an option declares, verbatim.
///
/// Returned rather than summarised. Warp cannot see an agent's permission policy
/// — T14.3 — so the only honest thing it can ever say about one is what the agent
/// itself put on the wire, quoted rather than interpreted.
pub(super) fn declared_changes(option: &PermissionOption) -> Option<&serde_json::Value> {
    option.meta.as_ref()?.get(PERMISSION_META_KEY)
}

/// Why nothing was selected, naming what the agent actually offered.
///
/// The list is included because the failure is agent-specific and invisible
/// otherwise: an agent that offers only `allow_always` is not broken, it just
/// cannot be answered yes from here, and a caller who is not told what was on
/// offer has no way to tell that from a bug in this code.
fn no_option_reason(request: &RequestPermissionRequest, decision: Decision) -> String {
    let offered = request
        .options
        .iter()
        .map(|option| {
            let suffix = if changes_policy(option) {
                " (changes policy)"
            } else {
                ""
            };
            format!("{}{suffix}", kind_name(option.kind))
        })
        .collect::<Vec<_>>()
        .join(", ");
    let offered = if offered.is_empty() {
        "nothing".to_owned()
    } else {
        offered
    };
    match decision {
        Decision::Allow => format!(
            "the agent offered no single-shot allow, so the answer is no; it offered: {offered}"
        ),
        Decision::Deny => {
            format!(
                "the agent offered no single-shot reject, so the turn was cancelled, which is still a no; it offered: {offered}"
            )
        }
    }
}

/// The wire name of a kind.
///
/// Written out rather than derived through `serde`, because this is for a person
/// reading an error and a serialization round trip to produce four constants
/// would be the more surprising code. The `_` arm is load-bearing:
/// `PermissionOptionKind` is `#[non_exhaustive]`, so a kind added upstream must
/// have a name here rather than a panic.
fn kind_name(kind: PermissionOptionKind) -> &'static str {
    match kind {
        PermissionOptionKind::AllowOnce => "allow_once",
        PermissionOptionKind::AllowAlways => "allow_always",
        PermissionOptionKind::RejectOnce => "reject_once",
        PermissionOptionKind::RejectAlways => "reject_always",
        _ => "an option kind this build does not know",
    }
}

#[cfg(test)]
#[path = "acp_permission_tests.rs"]
mod tests;
