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
//! values at these keys". No assumption is made about what a change means; a
//! declared change is treated as "this option does more than answer the
//! question", and that is true whatever it says. An agent that mislabels
//! `allow_always` as `allow_once` is still caught, and an agent that attaches a
//! change to a genuinely single-shot option loses nothing but one round trip.
//!
//! The rule that makes this a line rather than a rationalisation: **an option may
//! only be selected by a surface capable of showing what that option declares.**
//! A single-shot option declares only the tool call, which every surface renders.
//! An option carrying a declared change describes a *transition*, and neither a
//! non-interactive `--approve` nor a phone card can show that before the tap — so
//! selecting it there would authorize something the person was structurally never
//! shown. An in-app picker that renders the declaration could legitimately offer
//! it; nothing that exists today can.
//!
//! # The hole that argument left, measured 2026-08-27
//!
//! All of the above assumes a policy change announces itself in `_meta`. It does
//! not have to. Asked to leave plan mode, `claude-agent-acp` sends a
//! `session/request_permission` whose tool call has stable-v1
//! `kind: "switch_mode"` and whose five options are the session's **mode ids**:
//!
//! ```text
//! bypassPermissions  "Yes, and bypass permissions"       allow_always
//! auto               "Yes, and use \"auto\" mode"         allow_always
//! acceptEdits        "Yes, and auto-accept edits"        allow_always
//! default            "Yes, and manually approve edits"   allow_once    ← selected
//! plan               "No, keep planning"                 reject_once
//! ```
//!
//! **Not one of the five carries `_meta`.** So the kind gate admitted `default`,
//! the declaration gate found nothing to object to, and `--approve` answered
//! *"Yes, and manually approve edits"* — setting the session's permission mode
//! for every call after it. Watched on the wire: `{"outcome":"selected",
//! "optionId":"default"}`, followed by the agent leaving plan mode and writing a
//! file. The person had asked for `--mode plan`.
//!
//! The docs below already said why this would happen — *"absence of `_meta`
//! proves nothing"* — and `choose` read exactly that absence as safety anyway.
//! Writing a hazard down does not implement it; this is the third time in T14,
//! after the re-ask hazard T14.2 recorded and T14.3 built against, and the count
//! pin in T8.6. So it stops being an observation and becomes a rule: **a hazard
//! in a doc comment with no test under it is a hazard that is not defended.**
//! *"Absence of `_meta` proves nothing"* had none for as long as it was true and
//! undefended; the `switch_mode` tests are the first.
//!
//! What the case actually shows is that the *question* can be the problem. These
//! options declare their transition in their **names**, in English, which is
//! disclosure to a person and nothing at all to a flag. When an agent is asking
//! *which policy should apply* rather than *may I do this one thing*, no option
//! is single-shot whatever its kind says — so `--approve` has no business
//! answering yes to any of them, and [`asks_which_policy_applies`] is where that
//! is decided.
//!
//! Refusing is only the *allow* side. `reject_once` — "No, keep planning" — is
//! still selected and still correct, because declining a change leaves the state
//! where it already was. A no cannot widen anything here either.
//!
//! ## This reverses a constraint written in `TASKS.md`, deliberately
//!
//! T14.4's constraint list says **"gate on the method, never on
//! `tool_call.kind`"**. There is no method to gate on: every one of these arrives
//! as `session/request_permission`, so the constraint as written forbids the only
//! defence available. It is sound about *granting* — `ToolKind` is
//! `#[serde(other)]`, so an unrecognised kind silently becomes `Other`, and
//! anything that grants on a kind grants on the default. Used to **refuse**, that
//! same degradation runs the safe way: an unknown kind is not `SwitchMode`, so it
//! is not refused, which is today's behaviour rather than a new hole. The rule
//! survives with its direction named: never grant on `tool_call.kind`.
//!
//! # Two things this is not
//!
//! **Absence of `_meta` proves nothing.** The moment any path here reads "no
//! declared change, therefore safe", the forbidden assumption has been made in
//! the granting direction. The kind gate is what admits an option; the declared
//! change is only ever an extra way to reject one.
//!
//! **This is not a boundary against a hostile agent.** `PermissionOption.kind` is
//! exactly as agent-authored as `tool_call.kind`, which the ticket forbids gating
//! on — and a hostile agent does not ask permission at all, which T14.1 measured:
//! with `defaultMode: auto` the agent read files, wrote files and ran commands
//! and asked nobody. What this defends against is honest agents: an arbitrary
//! option order, an escalating option offered by default, and a spec-sloppy
//! option whose kind understates what it does. Saying so is the point — the fork
//! does not claim protection it does not have.

use agent_client_protocol::schema::v1::{
    PermissionOption, PermissionOptionId, PermissionOptionKind, RequestPermissionRequest, ToolKind,
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

/// The only `_meta.permission.version` whose layout this build claims to know.
///
/// Measured: `claude-agent-acp` sends `1`. Anything else — including a missing
/// version — means the `changes` list could be spelled somewhere this code does
/// not look, so an absent list is not evidence of an absent declaration.
const KNOWN_PERMISSION_VERSION: u64 = 1;

/// Said when the agent asks which policy should apply and the answer is a flag.
///
/// Names the surface rather than the option, because the refusal is not about
/// this agent doing anything wrong — it asked clearly and in English. It is about
/// `--approve` being unable to read English.
const POLICY_QUESTION_REASON: &str = "the agent is asking which permission policy should apply, not whether one thing may \
     happen; no option there is single-shot, so --approve declines and the session keeps \
     the policy it already had";

/// Whether the agent is asking which policy should apply, rather than whether one
/// thing may happen.
///
/// Read only to **refuse**, exactly like [`declaration`], and for the same reason:
/// `ToolKind` is agent-authored, so an agent that labels a mode switch `edit` is
/// not caught. That is not a regression — it is where this already stood — and it
/// is the same honesty `changes_policy` is written with. What this does catch is
/// the agent that says plainly what it is asking, which is the one worth
/// answering carefully.
pub(super) fn asks_which_policy_applies(request: &RequestPermissionRequest) -> bool {
    request.tool_call.fields.kind == Some(ToolKind::SwitchMode)
}

/// Answer one permission request.
pub(super) fn choose(request: &RequestPermissionRequest, decision: Decision) -> Choice {
    // Before the kind gate, because the kind gate is what got this wrong: the
    // option that changes the session's policy was typed `allow_once` and carried
    // no declaration, so every later test here passes it.
    if decision == Decision::Allow && asks_which_policy_applies(request) {
        return Choice::Cancel {
            reason: POLICY_QUESTION_REASON.to_owned(),
        };
    }

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
    ) || !matches!(declaration(option), Declaration::None)
}

/// What an option's `_meta` says about widening policy.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Declaration<'a> {
    /// Nothing relevant. **Not a guarantee of safety** — see the module docs.
    None,
    /// A version this build knows how to read, declaring these changes verbatim.
    ///
    /// Verbatim rather than summarised. Warp cannot see an agent's permission
    /// policy — T14.3 — so the only honest thing it can ever say about one is
    /// what the agent itself put on the wire.
    Changes(&'a serde_json::Value),
    /// A `permission` block in a layout this build does not know.
    ///
    /// Refused, because the reason no changes were found may simply be that this
    /// code looked in the wrong place. Failing closed on an unknown version costs
    /// one round trip; failing open costs the escalation the whole rule exists to
    /// stop.
    UnknownVersion,
}

/// Read an option's policy declaration.
///
/// Keyed on a **non-empty `changes` list**, not on the presence of the
/// `permission` block. `_meta` is a free-form map and an agent may reasonably put
/// benign permission metadata on every option it sends; refusing all of those
/// would make ordinary approvals fail for no reason, which is how a safety rule
/// gets switched off. Whether any agent actually does that is unmeasured — one
/// agent has been watched — so the narrow rule is the one to hold.
pub(super) fn declaration(option: &PermissionOption) -> Declaration<'_> {
    let Some(permission) = option
        .meta
        .as_ref()
        .and_then(|meta| meta.get(PERMISSION_META_KEY))
    else {
        return Declaration::None;
    };
    if permission
        .get("version")
        .and_then(serde_json::Value::as_u64)
        != Some(KNOWN_PERMISSION_VERSION)
    {
        return Declaration::UnknownVersion;
    }
    match permission.get("changes") {
        Some(changes) if changes.as_array().is_none_or(|list| !list.is_empty()) => {
            Declaration::Changes(changes)
        }
        _ => Declaration::None,
    }
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
            let suffix = match declaration(option) {
                Declaration::Changes(_) => " (declares a policy change)",
                Declaration::UnknownVersion => " (declares something this build cannot read)",
                Declaration::None => "",
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
