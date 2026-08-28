//! Answering a permission request, decided without an agent.
//!
//! The fixture is not invented. [`as_claude_sent_it`] is the option list
//! `claude-agent-acp` put on the wire on 2026-08-27, transcribed field for field
//! including the order — which is the part that matters, because the order is
//! what the old code trusted.

use agent_client_protocol::schema::v1::{ToolCallUpdate, ToolCallUpdateFields};

use super::*;

/// An ordinary file write, carrying `kind: edit` because the measured one did —
/// `{"toolCallId":"toolu_01Fs…","kind":"edit","title":"Write hello.txt"}`.
///
/// It did not carry one when these tests were first written, and the allowlist is
/// what exposed that as a fixture bug rather than a test failure: a request with
/// no kind is a real case, but it is not *this* case, and using it here quietly
/// tested the wrong thing.
fn request(options: Vec<PermissionOption>) -> RequestPermissionRequest {
    RequestPermissionRequest::new(
        "session-1",
        ToolCallUpdate::new(
            "toolu_1",
            ToolCallUpdateFields::new()
                .title("Write probe.txt")
                .kind(ToolKind::Edit),
        ),
        options,
    )
}

fn option(id: &'static str, name: &'static str, kind: PermissionOptionKind) -> PermissionOption {
    PermissionOption::new(id, name, kind)
}

fn allow_once_with_meta(meta: serde_json::Value) -> PermissionOption {
    PermissionOption::new("allow", "Allow Once", PermissionOptionKind::AllowOnce).meta(
        meta.as_object()
            .expect("a _meta fixture is an object")
            .clone(),
    )
}

/// The measured list, in the measured order: deny first, then allow once, then
/// an always-variant carrying a session-wide mode change.
fn as_claude_sent_it() -> Vec<PermissionOption> {
    vec![
        option("reject", "Deny", PermissionOptionKind::RejectOnce),
        option("allow", "Allow Once", PermissionOptionKind::AllowOnce),
        PermissionOption::new(
            "allow_always",
            "Always Allow",
            PermissionOptionKind::AllowAlways,
        )
        .meta(
            serde_json::json!({
                "permission": {
                    "version": 1,
                    "changes": [{
                        "type": "permission_mode",
                        "operation": "set",
                        "provider": "claudeCode",
                        "mode": "acceptEdits",
                        "description": "Set Claude Code permission mode to acceptEdits",
                        "lifetime": { "scope": "session" }
                    }]
                }
            })
            .as_object()
            .expect("the fixture is an object")
            .clone(),
        ),
    ]
}

/// `opencode` 1.18.25 on 2026-08-28, transcribed the same way — and the reason
/// this module's rule is a measurement rather than an argument. **Allow is first
/// here and deny is first in [`as_claude_sent_it`]**, so one line taking
/// `options.first()` approves on one agent and denies on the other.
fn as_opencode_sent_it() -> Vec<PermissionOption> {
    vec![
        option("once", "Allow once", PermissionOptionKind::AllowOnce),
        // No `_meta`. Only the kind gate refuses this one.
        option("always", "Always allow", PermissionOptionKind::AllowAlways),
        option("reject", "Reject", PermissionOptionKind::RejectOnce),
    ]
}

/// Two agents, opposite orders, the same answers. This is the property the whole
/// module exists for, and it could not be written until a second agent was run.
#[test]
fn two_agents_order_their_options_oppositely_and_both_are_answered_correctly() {
    assert_eq!(
        as_claude_sent_it()[0].kind,
        PermissionOptionKind::RejectOnce,
        "claude-agent-acp puts deny first"
    );
    assert_eq!(
        as_opencode_sent_it()[0].kind,
        PermissionOptionKind::AllowOnce,
        "opencode puts allow first — so first() would approve here and deny there"
    );

    let opencode = request(as_opencode_sent_it());
    assert_eq!(
        choose(&opencode, Decision::Allow),
        Choice::Select(PermissionOptionId::new("once"))
    );
    assert_eq!(
        choose(&opencode, Decision::Deny),
        Choice::Select(PermissionOptionId::new("reject"))
    );
}

/// opencode's always-variant declares nothing at all, so the kind gate is the
/// only thing standing between `--approve` and a remembered yes. Second
/// confirmation that the `_meta` rule is an extra refusal and never the
/// load-bearing one.
#[test]
fn an_always_variant_that_declares_nothing_is_still_refused() {
    let always = &as_opencode_sent_it()[1];

    assert_eq!(declaration(always), Declaration::None);
    assert!(changes_policy(always), "the kind alone must be enough");
    assert_eq!(
        choose(&request(as_opencode_sent_it()), Decision::Allow),
        Choice::Select(PermissionOptionId::new("once")),
        "the single-shot allow is taken, never the always-variant beside it"
    );
}

/// The second measured list, transcribed the same way: `ExitPlanMode` on
/// 2026-08-27. Every option is a session mode id, every name is a sentence about
/// policy, and **none of them carries `_meta`** — which is the whole reason the
/// `_meta` rule alone was not enough.
fn as_claude_asked_to_leave_plan_mode() -> RequestPermissionRequest {
    RequestPermissionRequest::new(
        "session-1",
        ToolCallUpdate::new(
            "toolu_switch",
            ToolCallUpdateFields::new()
                .title("Ready to code?")
                .kind(ToolKind::SwitchMode),
        ),
        vec![
            option(
                "bypassPermissions",
                "Yes, and bypass permissions",
                PermissionOptionKind::AllowAlways,
            ),
            option(
                "auto",
                "Yes, and use \"auto\" mode",
                PermissionOptionKind::AllowAlways,
            ),
            option(
                "acceptEdits",
                "Yes, and auto-accept edits",
                PermissionOptionKind::AllowAlways,
            ),
            option(
                "default",
                "Yes, and manually approve edits",
                PermissionOptionKind::AllowOnce,
            ),
            option(
                "plan",
                "No, keep planning",
                PermissionOptionKind::RejectOnce,
            ),
        ],
    )
}

/// The live hole, pinned. `--approve` selected `default` on the wire — a
/// session-wide permission mode, typed `allow_once`, carrying no declaration —
/// and the agent then left plan mode and wrote a file the person had asked it to
/// only plan.
#[test]
fn an_allow_refuses_a_question_about_which_policy_applies() {
    let choice = choose(&as_claude_asked_to_leave_plan_mode(), Decision::Allow);

    assert!(
        matches!(choice, Choice::Cancel { .. }),
        "answering yes here sets the policy for every later call, got: {choice:?}"
    );
}

/// The asymmetry survives the fix: declining a policy change leaves the session
/// with the policy it already had, so a no is still expressible and still safe.
#[test]
fn a_deny_still_answers_a_question_about_which_policy_applies() {
    let choice = choose(&as_claude_asked_to_leave_plan_mode(), Decision::Deny);

    assert_eq!(
        choice,
        Choice::Select(PermissionOptionId::new("plan")),
        "\"No, keep planning\" is a well-formed no and refusing to send it would help nobody"
    );
}

/// The ordinary path, unchanged. The measured `Write hello.txt` request carried
/// `kind: "edit"`, which is on the allowlist, so `--approve` still says yes.
#[test]
fn an_ordinary_edit_is_still_allowed() {
    let request = request(as_claude_sent_it());

    assert!(effect_is_confined_to_this_call(&request));
    assert_eq!(
        choose(&request, Decision::Allow),
        Choice::Select(PermissionOptionId::new("allow"))
    );
}

/// Every kind the spec gives a meaning that stops at the call. `delete` and
/// `execute` are on the list on purpose: the test is whether the effect is
/// *bounded*, not whether it is gentle, and confusing the two would turn this
/// into a danger rating `--approve` has no way to make.
#[test]
fn every_kind_whose_effect_stops_at_the_call_is_allowed() {
    for kind in [
        ToolKind::Read,
        ToolKind::Edit,
        ToolKind::Delete,
        ToolKind::Move,
        ToolKind::Search,
        ToolKind::Execute,
        ToolKind::Think,
        ToolKind::Fetch,
    ] {
        let mut request = request(as_claude_sent_it());
        request.tool_call.fields.kind = Some(kind);

        assert!(
            effect_is_confined_to_this_call(&request),
            "{kind:?} should be answerable"
        );
    }
}

/// The correction to the first fix. A denylist of `SwitchMode` read "not the
/// signal, therefore safe", and `#[serde(other)]` makes an unrecognised kind
/// arrive silently as `Other` — so a mode switch labelled anything else would
/// have gone through. Refusing by falling off the end of an allowlist is what
/// makes that impossible rather than merely unlikely.
#[test]
fn a_kind_this_build_does_not_recognise_is_refused() {
    for kind in [ToolKind::Other, ToolKind::SwitchMode] {
        let mut request = as_claude_asked_to_leave_plan_mode();
        request.tool_call.fields.kind = Some(kind);

        assert!(
            matches!(choose(&request, Decision::Allow), Choice::Cancel { .. }),
            "{kind:?} is not known to stop at the call, so it cannot be approved by a flag"
        );
    }
}

/// A request that says nothing about its kind. Fail-closed, same as an unknown
/// `_meta.permission.version`: the reason no bound was found may be that this
/// code was never told one.
#[test]
fn a_request_with_no_kind_is_refused() {
    let mut request = as_claude_asked_to_leave_plan_mode();
    request.tool_call.fields.kind = None;

    assert!(!effect_is_confined_to_this_call(&request));
    assert!(matches!(
        choose(&request, Decision::Allow),
        Choice::Cancel { .. }
    ));
}

/// The reason reaches the person, because a `--approve` run that quietly stops
/// approving is the failure this whole module was built out of — and an allowlist
/// refuses more than a denylist, so it owes more of an explanation.
#[test]
fn the_refusal_says_what_was_being_asked() {
    let Choice::Cancel { reason } = choose(&as_claude_asked_to_leave_plan_mode(), Decision::Allow)
    else {
        panic!("a policy question is refused");
    };

    assert!(
        reason.contains("which permission policy should apply"),
        "the reason should name the question, got: {reason}"
    );
}

/// An unrecognised kind produces a *different* sentence, naming the kind, so the
/// person can tell "this build does not know that kind" from "that agent asked
/// something a flag may not answer". Conflating them is how an allowlist earns
/// the reputation of being broken.
#[test]
fn a_refusal_for_an_unrecognised_kind_names_it() {
    let mut request = as_claude_asked_to_leave_plan_mode();
    request.tool_call.fields.kind = Some(ToolKind::Other);

    let Choice::Cancel { reason } = choose(&request, Decision::Allow) else {
        panic!("an unrecognised kind is refused");
    };

    assert!(
        reason.contains("`other`"),
        "the reason should name the kind, got: {reason}"
    );
}

/// The regression. `options.first()` here is **Deny**, so the old code answered
/// an approval by refusing it — and did so silently, reporting success.
#[test]
fn an_allow_does_not_take_the_first_option() {
    let request = request(as_claude_sent_it());

    let choice = choose(&request, Decision::Allow);

    assert_eq!(
        choice,
        Choice::Select(PermissionOptionId::new("allow")),
        "allow must select the allow_once option, not whatever came first"
    );
}

#[test]
fn a_deny_selects_the_single_shot_reject() {
    let request = request(as_claude_sent_it());

    let choice = choose(&request, Decision::Deny);

    assert_eq!(choice, Choice::Select(PermissionOptionId::new("reject")));
}

/// One phone tap must not widen the session. The always-variant is the only
/// allow on offer here, and the answer is still no.
#[test]
fn an_allow_never_selects_an_always_variant() {
    let request = request(vec![
        option("reject", "Deny", PermissionOptionKind::RejectOnce),
        option(
            "allow_always",
            "Always Allow",
            PermissionOptionKind::AllowAlways,
        ),
    ]);

    let choice = choose(&request, Decision::Allow);

    let Choice::Cancel { reason } = choice else {
        panic!("an always-variant is not selectable, got: {choice:?}");
    };
    assert!(
        reason.contains("allow_always"),
        "the reason should name what was offered, got: {reason}"
    );
}

/// Kind and `_meta` are independent signals and either one is disqualifying, so
/// an agent that labels a policy-widening option `allow_once` is still caught.
#[test]
fn an_allow_refuses_an_option_that_declares_a_policy_change_whatever_its_kind() {
    let request = request(vec![
        option("reject", "Deny", PermissionOptionKind::RejectOnce),
        allow_once_with_meta(serde_json::json!({
            "permission": { "version": 1, "changes": [{ "type": "permission_mode" }] }
        })),
    ]);

    let choice = choose(&request, Decision::Allow);

    assert!(
        matches!(choice, Choice::Cancel { .. }),
        "a declared policy change disqualifies the option, got: {choice:?}"
    );
}

/// `_meta` is a free-form map, so an unrelated key must not cost the person a
/// perfectly ordinary approval.
#[test]
fn an_unrelated_meta_key_does_not_disqualify_an_option() {
    let request = request(vec![allow_once_with_meta(
        serde_json::json!({ "claudeCode": { "toolName": "Write" } }),
    )]);

    assert_eq!(
        choose(&request, Decision::Allow),
        Choice::Select(PermissionOptionId::new("allow"))
    );
}

/// The rule is keyed on a non-empty `changes` list, not on the `permission` block
/// existing. An agent that decorates every option with benign metadata must not
/// find that ordinary approvals stop working — that is how a safety rule gets
/// switched off by whoever it inconveniences.
#[test]
fn a_permission_block_declaring_no_changes_does_not_disqualify_an_option() {
    let request = request(vec![allow_once_with_meta(serde_json::json!({
        "permission": { "version": 1, "changes": [] }
    }))]);

    assert_eq!(
        choose(&request, Decision::Allow),
        Choice::Select(PermissionOptionId::new("allow"))
    );
}

/// The narrowing above is only safe if an unreadable block still fails closed:
/// finding no `changes` may just mean this code looked in the wrong place.
#[test]
fn a_permission_block_in_an_unknown_version_is_refused_even_with_no_changes() {
    let request = request(vec![allow_once_with_meta(serde_json::json!({
        "permission": { "version": 7 }
    }))]);

    let Choice::Cancel { reason } = choose(&request, Decision::Allow) else {
        panic!("an unreadable declaration must fail closed");
    };
    assert!(
        reason.contains("cannot read"),
        "the reason should say the declaration was unreadable, got: {reason}"
    );
}

#[test]
fn a_permission_block_with_no_version_at_all_is_refused() {
    let request = request(vec![allow_once_with_meta(serde_json::json!({
        "permission": { "changes": [] }
    }))]);

    assert!(matches!(
        choose(&request, Decision::Allow),
        Choice::Cancel { .. }
    ));
}

/// An agent that offers only always-variants can still be told no — cancelling
/// is a refusal, and the live run confirms the agent reads it as one.
#[test]
fn a_deny_with_no_single_shot_reject_still_answers_no() {
    let request = request(vec![option(
        "allow_always",
        "Always Allow",
        PermissionOptionKind::AllowAlways,
    )]);

    let Choice::Cancel { reason } = choose(&request, Decision::Deny) else {
        panic!("there is no reject_once here to select");
    };
    assert!(
        reason.contains("still a no"),
        "the reason should say a cancel is a refusal, got: {reason}"
    );
}

#[test]
fn an_empty_option_list_is_answered_rather_than_panicking() {
    let request = request(Vec::new());

    let Choice::Cancel { reason } = choose(&request, Decision::Allow) else {
        panic!("nothing can be selected from an empty list");
    };
    assert!(
        reason.contains("nothing"),
        "the reason should say nothing was offered, got: {reason}"
    );
}

/// What T14.3 will report is the agent's own words, so nothing may be lost on
/// the way through.
#[test]
fn a_declared_change_is_returned_verbatim() {
    let options = as_claude_sent_it();
    let always = options
        .iter()
        .find(|option| option.kind == PermissionOptionKind::AllowAlways)
        .expect("the fixture has an always-variant");

    let Declaration::Changes(declared) = declaration(always) else {
        panic!("the always-variant declares a readable change");
    };

    assert_eq!(declared[0]["mode"], "acceptEdits");
    assert_eq!(declared[0]["lifetime"]["scope"], "session");
}

#[test]
fn an_option_with_no_meta_declares_nothing() {
    assert_eq!(
        declaration(&option(
            "allow",
            "Allow Once",
            PermissionOptionKind::AllowOnce
        )),
        Declaration::None
    );
}
