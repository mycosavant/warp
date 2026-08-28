//! Answering a permission request, decided without an agent.
//!
//! The fixture is not invented. [`as_claude_sent_it`] is the option list
//! `claude-agent-acp` put on the wire on 2026-08-27, transcribed field for field
//! including the order — which is the part that matters, because the order is
//! what the old code trusted.

use agent_client_protocol::schema::v1::{ToolCallUpdate, ToolCallUpdateFields};

use super::*;

fn request(options: Vec<PermissionOption>) -> RequestPermissionRequest {
    RequestPermissionRequest::new(
        "session-1",
        ToolCallUpdate::new(
            "toolu_1",
            ToolCallUpdateFields::new().title("Write probe.txt"),
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
