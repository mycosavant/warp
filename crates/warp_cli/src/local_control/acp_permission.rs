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
//! ## A second agent settled it, and the two disagree exactly (T14.5's gate)
//!
//! `opencode` 1.18.25, measured 2026-08-28, on OpenRouter with a Gemini model —
//! a different codebase and a different model from everything above:
//!
//! ```text
//! 1  once    "Allow once"    allow_once
//! 2  always  "Always allow"  allow_always      (no _meta)
//! 3  reject  "Reject"        reject_once
//! ```
//!
//! **Allow is first here; deny is first there.** So `options.first()` — the bug
//! this module was written to fix — would have *approved* on opencode and
//! *denied* on Claude, from the same line of code. Two agents, opposite orders,
//! neither wrong. The argument for choosing by kind stops being an argument and
//! becomes a measurement.
//!
//! opencode's `always` also carries **no `_meta`**, so nothing declares what it
//! does: only the kind gate refuses it. That is the second confirmation that
//! [`declaration`] is an *extra* way to reject and never the load-bearing one.
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
//!
//! **Read that as the *rationale*, because the mechanism is stricter and this
//! paragraph described it wrongly until 2026-08-31.** [`choose`] takes no surface
//! parameter at all — its signature is `(request, decision)` — so it does not ask
//! "can *this* surface show a declaration". It refuses any option carrying a
//! declared change **for every surface, including the panel**, which can render
//! one perfectly well. The stated rule would permit a per-surface answer; the
//! code does not implement one.
//!
//! Recorded rather than reworded because the difference is the crux of I18. A
//! reader taking the rule literally would conclude the panel is already entitled
//! to offer a persistent grant and that some per-surface check has gone missing.
//! Nothing has gone missing: the universal refusal is deliberate and fail-closed,
//! and widening it to a capability check would be a **permission-posture change**,
//! which is exactly the decision that is frozen and the maintainer's to make.
//! Found by an agent in Warp's own panel auditing this file, asked to name
//! anything whose doc claims something the code below it does not do.
//! A single-shot option declares only the tool call, which every surface renders.
//! An option carrying a declared change describes a *transition*, and neither a
//! non-interactive `--approve` nor a phone card can show that before the tap — so
//! selecting it there would authorize something the person was structurally never
//! shown. An interactive approval card that renders the declaration could
//! legitimately offer it; nothing that exists today can.
//!
//! ## …and something does now, so that last clause is retired (T14.16/T14.18)
//!
//! T14.16 built that card. It shows the agent, the verbatim `rawInput`, where
//! the request says it acts, and every option offered, and its own module doc
//! restates this rule. Carrying [`Declaration::Changes`] into the parked
//! request and rendering it verbatim would meet the standard set just above —
//! verbatim is already the doctrine here, because what the agent put on the
//! wire is the only honest thing Warp can say about a policy it cannot see. So
//! **the disclosure objection is satisfiable, and this module must stop resting
//! on it.** A refusal defended by a premise its own repo has falsified is the
//! rot this fork's docs exist to prevent, and that clause was two days from
//! becoming exactly that.
//!
//! What it always was, and remains, is a **necessary** condition and never a
//! sufficient one. `allow_always` stays refused for three reasons that were
//! never about disclosure:
//!
//! * **The population it would serve is empty today.** `opencode`'s `always`
//!   carries no `_meta` (measured above), so there is nothing to render and
//!   *"absence of `_meta` proves nothing"* forbids offering it undeclared — the
//!   card can never offer that one. That leaves `claude-agent-acp` held in
//!   `default`. T14.18 then measured that the panel path never sends
//!   `session/set_mode` at all, so a panel session with that agent runs in
//!   `auto` and raises no permission requests to attach a button to. The
//!   beneficiary set is not small; it has no members.
//! * **Measured demand is zero.** The one recurring refusal in real work was
//!   `opencode`'s `other` precondition, an allowlist refusal `allow_always`
//!   would not have touched. What `allow_always` answers is approval *fatigue*,
//!   and the run that would measure fatigue — T14.11, with the card in place —
//!   has not happened. Building the escalation before the measurement inverts
//!   the ordering that has three times in T14 deleted the work rather than
//!   guided it.
//! * **A declaration is not the grant.** The `_meta` says *"mode →
//!   acceptEdits"*; what a yes actually spends is the future calls, which after
//!   the transition raise no requests and so leave no trace. That thins the
//!   exact property the charter's audit rule and T14.15's one-turn-one-file
//!   exist to provide. A person choosing it knowingly is legitimate consent —
//!   which makes this a posture call rather than a correctness one, and posture
//!   is the maintainer's.
//!
//! **And if fatigue does show up, the answer is probably not this button.** A
//! tired person wants *"stop asking about edits"*, which is a standing policy
//! and belongs on a policy surface — `session/set_mode`, whose absence from the
//! panel is T14.18 — rather than as an escalating third control on a card whose
//! other two answer one call. A mode surface also serves `opencode`-shaped
//! agents, which this button structurally cannot.
//!
//! The cheap half, shelved with the rest: carry `_meta` into the parked request
//! **as data, rendered and never selectable**, so the card can say *"this
//! option, which Warp will not select, declares: …"*. Disclosure without
//! escalation, and consistent with `_meta` already being read only to refuse.
//!
//! A *mode picker* cannot, and the distinction is a channel rather than a
//! quibble: this option lives on a pending `session/request_permission`, which
//! only the surface answering that request can select. A picker acts through
//! `session/set_mode` and can never answer it. T14.4's constraint list named the
//! picker as the successor surface and was wrong about which one.
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
//! **And it is not one agent's quirk.** The spec documents this shape itself, in
//! `docs/protocol/v1/session-modes.mdx` under *"Exiting plan modes"*, down to an
//! option named *"Yes, and manually accept actions"* typed `allow_once` with no
//! `_meta`. So every ACP agent with a plan mode is expected to present a policy
//! change this way, and the finding generalises past the one agent that was
//! watched — which is rare enough here to be worth saying.
//!
//! What the case actually shows is that the *question* can be the problem. These
//! options declare their transition in their **names**, in English, which is
//! disclosure to a person and nothing at all to a flag. When an agent is asking
//! *which policy should apply* rather than *may I do this one thing*, no option
//! is single-shot whatever its kind says.
//!
//! ## The first fix for that was a denylist of one, which is the same trap again
//!
//! It refused `SwitchMode` and allowed everything else — that is *"not the
//! signal, therefore safe"*, one field over from *"no `_meta`, therefore safe"*,
//! and `#[serde(other)]` makes it silent. An agent whose mode switch arrived as
//! `execute`, as a kind added in a later schema, or with no kind at all would
//! have gone straight through. The question is now inverted:
//! [`effect_is_confined_to_this_call`] is an **allowlist** of the kinds whose
//! spec meaning stops at the call, and everything else — `SwitchMode`, `Other`,
//! an absent kind, a variant that does not exist yet — refuses by falling off the
//! end of a `matches!` rather than by being listed.
//!
//! That costs something and the cost is named: an honest agent whose ordinary
//! calls arrive as `other` gets refused under `--approve`. It is refused
//! *loudly*, with the kind in the message, because an allowlist's wrong answers
//! have to be explicable on sight — a person concluding "the flag is broken" is
//! exactly what T14.2 cost. A wrong refusal costs a message; a wrong grant costs
//! the session's policy.
//!
//! Refusing is only the *allow* side. `reject_once` — "No, keep planning" — is
//! still selected and still correct, because declining a change leaves the state
//! where it already was. A no cannot widen anything here either.
//!
//! ## What that cost actually is, measured (T14.8)
//!
//! The cost named just above — *"an honest agent whose ordinary calls arrive as
//! `other` gets refused"* — was a prediction when it was written. Run
//! 2026-08-29 against two live agents, it is real, it belongs to one of them,
//! and it has a shape.
//!
//! `opencode` raises an **extra permission request, before the call itself**,
//! whenever a call would reach outside the project directory. It arrives with
//! `kind: "other"` and a `rawInput` naming the command together with the
//! directories and glob patterns being reached into; the call then arrives as
//! its own `execute`. So `cat .fork/GOAL.md` is one `execute`, while
//! `cat ~/.bashrc` is an `other` followed by an `execute` — and refusing the
//! first means the second is never sent. It resolves paths rather than matching
//! strings: `../warp/.fork/GOAL.md`, which lands back inside, is a plain
//! `execute`.
//!
//! `claude-agent-acp`, same command, same machine, same day: **one** request,
//! `kind: "execute"`, approvable today, no precondition at all. So this is one
//! agent's convention rather than a protocol fact — the same shape as the
//! opposite option orders above, and the same conclusion follows. A rule that
//! special-cased opencode's payload would be reading one vendor's habit as
//! meaning, which is precisely the error `options.first()` already cost.
//!
//! Two measurements decide what can honestly be offered.
//!
//! **The `other` request's `allow_once` really is call-scoped.** Answered
//! `once` for `cat /etc/hostname` (declared pattern `/etc/*`), then asked for
//! `cat /etc/hosts` — same declared pattern, same session — and it asked again.
//! The `patterns` field describes what is being reached into, not what a yes
//! grants. For this agent the effect genuinely stops at the call.
//!
//! **And Warp still cannot know that.** `ToolKind` is `#[serde(other)]`, so an
//! unknown kind string deserializes to exactly the same `Some(Other)` as a
//! deliberate one: there is no value meaning *"this agent said `other` and meant
//! it"*. Admitting `Other` because this agent's `other` happens to be confined
//! would admit every kind nobody has read yet — the denylist trap restated one
//! field over. **The allowlist stands.** What is new is that its cost is
//! measured rather than assumed.
//!
//! **And the refusal is a choice, not an impossibility — say so, because the
//! difference is what a later reader needs.** Warp never manufactures a yes; it
//! relays a person's, and it could relay one here. A person's yes to a *shown*
//! `other` is epistemically no worse than their yes to a shown `execute`, whose
//! actual effect is equally unbounded: this allowlist distinguishes what the
//! spec means, not what a call does. What decides it is size. After the config
//! remedy below, the residual is one agent, outside the project, with no pattern
//! granted, once — and one of those costs a single pasted `agent deny`. That
//! does not pay for widening what every consent surface may say yes to,
//! including a phone tap under `WARP_FORK_REMOTE_APPROVE`. The invariant is
//! worth more: **no Warp surface relays a yes to a call whose spec meaning is
//! unknown.** The shelved design, if the counts ever change, is a digest-bound
//! *person* yes on an entry showing the verbatim call, with `--approve` and
//! every other unattended path left exactly as they are — and its predicate must
//! test that `raw_input` is present and nothing whatever about what is in it.
//!
//! So the refusal changed instead of the rule, in two ways. It states what this
//! build cannot tell rather than what the call does — see
//! [`unconfined_reason`], which is where the old wording overclaimed. And it
//! names a move, because the person's real remedy is not in Warp at all:
//! opencode calls this permission `external_directory`, and one line in the
//! agent's own config stops the ask from being raised. Measured, with
//! `"permission": {"external_directory": {"/etc/*": "allow"}}` only the
//! approvable `execute` survives. That is the mirror of the rule this fork
//! already keeps — **Warp cannot make an agent ask** — and deserves saying in
//! the same breath: the agent's own config also decides whether what it asks
//! can be answered from here.
//!
//! ## This amends a constraint written in `TASKS.md`, rather than rereading it
//!
//! T14.4's constraint list says **"gate on the method, never on
//! `tool_call.kind`"**, as an absolute. There is no method to gate on — every one
//! of these arrives as `session/request_permission` — so as written it forbade
//! the only defence available, and the honest response is to correct the
//! constraint in the doc that carries it, not to find a reading of it that lets
//! this through. What survives, and is sharper: **a kind may disqualify, never
//! qualify — and a kind this build does not recognise must not qualify either.**
//! The second clause is what the denylist got wrong.
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
pub enum Decision {
    Allow,
    Deny,
}

/// What to send back, and what to tell the person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
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

/// Why nothing was allowed for a call this build cannot bound to itself.
///
/// **Names the kind**, and that is load-bearing rather than polite. An allowlist
/// refuses more than a denylist would, so its wrong answers have to be
/// explicable on sight: a person whose `--approve` stopped working needs to read
/// *which* kind was not recognised rather than conclude the flag is broken. That
/// conclusion is exactly what the T14.2 bug cost, and a silent refusal would earn
/// it honestly.
///
/// **Every sentence here is about what this build knows, never about what the
/// call does** — T14.8, and it is a correction. The shipped wording was *"the
/// call's kind is `other`, whose effect this build cannot bound to this one
/// call"*, which the code means epistemically and a person reads
/// ontologically: as Warp saying the call is unbounded, which is to say
/// dangerous. Measured, the commonest real `other` is an agent asking to read a
/// file one directory outside the project. Warp has no idea whether that is
/// dangerous and should not imply that it does; the honest refusal says it
/// cannot tell, and says what the person can do instead.
fn unconfined_reason(request: &RequestPermissionRequest) -> String {
    match request.tool_call.fields.kind {
        Some(ToolKind::SwitchMode) => {
            "the agent is asking which permission policy should apply, not whether one thing may \
             happen; no option there is single-shot whatever its kind says, so Warp declines and \
             the session keeps the policy it already had."
                .to_owned()
        }
        Some(ToolKind::Other) => {
            "the call's kind is `other`, which the protocol leaves without a meaning — and it is \
             also where a kind this build has never heard of arrives, so the word says nothing \
             about what a yes here would cover. This is not a claim that the call is dangerous \
             — Warp cannot tell, so it declines rather than guess. Denying works, so \
             does cancelling the turn, and an agent that asks this for ordinary work can usually \
             be configured to stop asking — its config decides that, not Warp's."
                .to_owned()
        }
        Some(kind) => format!(
            "the call's kind is `{}`, which this build has no meaning for, so it cannot tell \
             whether a yes here stops at this call and declines rather than guess; a kind it \
             knows would have been answered.",
            tool_kind_name(kind)
        ),
        None => {
            "the request says nothing about the call's kind, so this build cannot tell whether \
             saying yes stops at this call, and Warp declines rather than guess."
                .to_owned()
        }
    }
}

/// The wire name of a tool kind, for a person reading a refusal. Written out for
/// the same reason as [`kind_name`], and the `_` arm carries the same weight.
fn tool_kind_name(kind: ToolKind) -> &'static str {
    match kind {
        ToolKind::Read => "read",
        ToolKind::Edit => "edit",
        ToolKind::Delete => "delete",
        ToolKind::Move => "move",
        ToolKind::Search => "search",
        ToolKind::Execute => "execute",
        ToolKind::Think => "think",
        ToolKind::Fetch => "fetch",
        ToolKind::SwitchMode => "switch_mode",
        ToolKind::Other => "other",
        _ => "a kind this build does not know",
    }
}

/// Whether this build knows the effect of saying yes to stop at this call.
///
/// **An allowlist, and the first draft was a denylist of one.** Refusing only
/// `SwitchMode` reads "not the signal, therefore safe" — the same shape as
/// reading an absent `_meta` as safety, one field over, and `#[serde(other)]`
/// makes it silent: an agent whose mode switch arrives as `execute`, as a kind
/// added upstream, or with no kind at all would have sailed through. So the
/// question is inverted. An option may be selected only for a kind the spec
/// gives a meaning that stops at the call — read, edit, delete, move, search,
/// execute, think, fetch. Dangerous is fine; *unbounded* is not, which is why
/// `Delete` is on the list and `SwitchMode` is not.
///
/// Everything else refuses, and by construction rather than by enumeration:
/// `Other`, an absent kind, and any variant a later schema adds all fall through
/// this `matches!` to `false`. That is the same fail-closed as an unknown
/// `_meta.permission.version`, with the same cost — one round trip — and it is
/// the property that makes reading an agent-authored kind admissible at all.
/// A refusal that is wrong costs a loud message; a grant that is wrong costs the
/// session's policy.
pub(super) fn effect_is_confined_to_this_call(request: &RequestPermissionRequest) -> bool {
    matches!(
        request.tool_call.fields.kind,
        Some(
            ToolKind::Read
                | ToolKind::Edit
                | ToolKind::Delete
                | ToolKind::Move
                | ToolKind::Search
                | ToolKind::Execute
                | ToolKind::Think
                | ToolKind::Fetch
        )
    )
}

/// Whether selecting this option would do more than answer this one question.
///
/// The predicate the consent ledger records transitions from, and it is wider
/// than [`changes_policy`] on purpose. A `switch_mode` request's options carry no
/// `_meta` and are typed `allow_once`, so a ledger keyed on declarations alone
/// reported `transitions_offered: []` for a session whose one event was a
/// five-option policy menu — the *offer* went unrecorded, which is the fact worth
/// having even now that nothing here can accept it.
///
/// `reject_once` is excluded, and not for tidiness: declining leaves the session
/// with the policy it already had, so recording a refusal as an offered
/// transition would put *"authorized by Warp: No, keep planning"* in a report,
/// which is the wrong inference this module keeps having to design against.
pub(super) fn is_more_than_an_answer(
    request: &RequestPermissionRequest,
    option: &PermissionOption,
) -> bool {
    if option.kind == PermissionOptionKind::RejectOnce {
        return false;
    }
    changes_policy(option) || !effect_is_confined_to_this_call(request)
}

/// Answer one permission request.
pub fn choose(request: &RequestPermissionRequest, decision: Decision) -> Choice {
    // Before the kind gate, because the kind gate is what got this wrong: the
    // option that changes the session's policy was typed `allow_once` and carried
    // no declaration, so every later test here passes it.
    if decision == Decision::Allow && !effect_is_confined_to_this_call(request) {
        return Choice::Cancel {
            reason: unconfined_reason(request),
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
    // Terminated like the others: these are concatenated into a paragraph by
    // `acp_agent`'s conversation note, and shown on their own by the console and
    // by `warpctrl`. A reason that trails off after a list reads as truncated
    // output rather than as a sentence.
    match decision {
        Decision::Allow => format!(
            "the agent offered no single-shot allow, so the answer is no; it offered: {offered}."
        ),
        Decision::Deny => {
            format!(
                "the agent offered no single-shot reject, so the turn was cancelled, which is still a no; it offered: {offered}."
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
