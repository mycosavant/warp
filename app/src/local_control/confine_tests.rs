use ::local_control::protocol::{
    AgentApprovalsResult, AgentConversationSummary, AgentListResult, PendingApproval,
};
use ::local_control::{ActionKind, InstanceId};
use chrono::Duration;
use serde_json::json;

use super::*;

const MINE: &str = "3f2f0e6a-0000-4000-8000-000000000001";
const OTHER: &str = "3f2f0e6a-0000-4000-8000-000000000002";

fn confined(action: ActionKind) -> CredentialGrant {
    CredentialGrant::new(
        InstanceId("inst_test".to_owned()),
        action,
        Duration::minutes(5),
    )
    .confined_to(Some(MINE.to_owned()))
}

fn unconfined(action: ActionKind) -> CredentialGrant {
    CredentialGrant::new(
        InstanceId("inst_test".to_owned()),
        action,
        Duration::minutes(5),
    )
}

fn action(kind: ActionKind, params: Value) -> Action {
    Action::with_params(kind, params).expect("a JSON value is always serializable")
}

fn owner(id: &str) -> Option<String> {
    match id {
        "req-mine" => Some(MINE.to_owned()),
        "req-other" => Some(OTHER.to_owned()),
        _ => None,
    }
}

/// The whole point: a prompt goes to the conversation the device was handed,
/// and to nothing else -- not another conversation, and not a new one.
#[test]
fn a_confined_device_prompts_its_conversation_and_no_other() {
    let grant = confined(ActionKind::AgentPrompt);
    let prompt = |conversation: Option<&str>| {
        let mut params = json!({ "prompt": "carry on" });
        if let Some(id) = conversation {
            params["conversation_id"] = json!(id);
        }
        ensure_within(&action(ActionKind::AgentPrompt, params), &grant, owner)
    };

    assert!(prompt(Some(MINE)).is_ok());
    let other = prompt(Some(OTHER)).expect_err("another conversation is refused");
    assert_eq!(other.code, ErrorCode::InsufficientPermissions);
    assert!(
        other.message.contains(MINE),
        "the refusal names the confinement"
    );
    let fresh = prompt(None).expect_err("a new conversation is not the one handed over");
    assert!(fresh.message.contains("start a new conversation"));
}

/// An approval is answered only if it is this conversation's. A pane agent's
/// request has no conversation and is refused, as is an unknown id: the
/// device is told it is not one of that conversation's rather than
/// `no_such_approval`, so a confined phone cannot probe for what exists.
#[test]
fn a_confined_device_answers_only_its_conversations_requests() {
    for kind in [ActionKind::AgentApprove, ActionKind::AgentDeny] {
        let grant = confined(kind);
        let answer = |id: &str| {
            ensure_within(
                &action(kind, json!({ "approval_id": id, "digest": "d" })),
                &grant,
                owner,
            )
        };
        assert!(answer("req-mine").is_ok());
        assert!(answer("req-other").is_err());
        assert!(answer("pane-7").is_err());
    }
}

#[test]
fn a_confined_device_reads_and_cancels_its_conversation_only() {
    let grant = confined(ActionKind::AgentTrace);
    assert!(
        ensure_within(
            &action(ActionKind::AgentTrace, json!({ "conversation_id": MINE })),
            &grant,
            owner
        )
        .is_ok()
    );
    assert!(
        ensure_within(
            &action(ActionKind::AgentTrace, json!({ "conversation_id": OTHER })),
            &grant,
            owner
        )
        .is_err()
    );
    let grant = confined(ActionKind::AgentCancel);
    assert!(
        ensure_within(
            &action(ActionKind::AgentCancel, json!({ "conversation_id": MINE })),
            &grant,
            owner
        )
        .is_ok()
    );
    assert!(
        ensure_within(
            &action(ActionKind::AgentCancel, json!({ "conversation_id": OTHER })),
            &grant,
            owner
        )
        .is_err()
    );
}

/// The allowlist is closed: an action this module has no rule for is refused
/// under confinement even if the pairable list grows to include it one day.
#[test]
fn a_confined_device_is_refused_anything_without_a_rule() {
    for kind in [
        ActionKind::AgentSpawn,
        ActionKind::InputSubmit,
        ActionKind::SlashRun,
        ActionKind::AgentRead,
    ] {
        let grant = confined(kind);
        assert!(
            ensure_within(&action(kind, json!({})), &grant, owner).is_err(),
            "{} must be refused under confinement",
            kind.as_str()
        );
    }
}

/// Everything above is a no-op for a grant with no confinement, which is every
/// local client and every device paired by `warpctrl pair show`.
#[test]
fn an_unconfined_grant_is_untouched() {
    let grant = unconfined(ActionKind::AgentPrompt);
    assert!(
        ensure_within(
            &action(ActionKind::AgentPrompt, json!({ "prompt": "x" })),
            &grant,
            owner
        )
        .is_ok()
    );
    let mut data = json!({ "conversations": [{ "conversation_id": OTHER }] });
    confine_result(ActionKind::AgentList, &mut data, &grant);
    assert_eq!(data["conversations"].as_array().map(Vec::len), Some(1));
    assert!(line_is_within(r#"{"session_id":"anything"}"#, None));
    assert!(line_is_within("not json", None));
}

/// The two instance-wide reads are filtered on the field the real types
/// serialize, built from those types so a rename fails here.
#[test]
fn instance_wide_reads_are_filtered_to_the_confined_conversation() {
    let summary = |id: &str| AgentConversationSummary {
        conversation_id: id.to_owned(),
        ..serde_json::from_value(json!({
            "conversation_id": id, "status": "success", "settled": false, "is_busy": false,
            "is_hidden": false
        }))
        .expect("a summary with its required fields")
    };
    let mut list = serde_json::to_value(AgentListResult {
        conversations: vec![summary(MINE), summary(OTHER)],
    })
    .expect("serializes");
    confine_result(
        ActionKind::AgentList,
        &mut list,
        &confined(ActionKind::AgentList),
    );
    let ids: Vec<&str> = list["conversations"]
        .as_array()
        .expect("an array")
        .iter()
        .filter_map(|c| c["conversation_id"].as_str())
        .collect();
    assert_eq!(ids, [MINE]);

    let approval = |conversation: Option<&str>| PendingApproval {
        conversation_id: conversation.map(str::to_owned),
        ..serde_json::from_value(json!({
            "approval_id": "r", "agent": "claude", "source": "acp", "kind": "permission",
            "acts_on": [], "digest": "d", "can_approve": true
        }))
        .expect("an approval with its required fields")
    };
    let mut approvals = serde_json::to_value(AgentApprovalsResult {
        approvals: vec![approval(Some(MINE)), approval(Some(OTHER)), approval(None)],
    })
    .expect("serializes");
    confine_result(
        ActionKind::AgentApprovals,
        &mut approvals,
        &confined(ActionKind::AgentApprovals),
    );
    assert_eq!(approvals["approvals"].as_array().map(Vec::len), Some(1));
}

/// The stream is filtered on `session_id`, which is Warp's conversation id on
/// every line the fork writes; a line that names none is withheld.
#[test]
fn the_stream_shows_a_confined_device_its_conversations_lines_only() {
    let mine = format!(r#"{{"ts":"t","event":"tool_start","session_id":"{MINE}"}}"#);
    let other = format!(r#"{{"ts":"t","event":"tool_start","session_id":"{OTHER}"}}"#);
    assert!(line_is_within(&mine, Some(MINE)));
    assert!(!line_is_within(&other, Some(MINE)));
    assert!(!line_is_within(
        r#"{"ts":"t","event":"serialize_failed"}"#,
        Some(MINE)
    ));
    assert!(!line_is_within("not json", Some(MINE)));
}

/// Stop sharing voids the credentials a device already minted for the
/// conversation, and nothing else's. Measured before this existed: the phone
/// kept prompting on a credential minted before the stop.
#[test]
fn taking_a_conversation_back_voids_its_live_credentials_only() {
    let mut credentials = std::collections::HashMap::new();
    credentials.insert("mine-prompt".to_owned(), confined(ActionKind::AgentPrompt));
    credentials.insert("mine-trace".to_owned(), confined(ActionKind::AgentTrace));
    credentials.insert("local".to_owned(), unconfined(ActionKind::AgentPrompt));
    credentials.insert(
        "other".to_owned(),
        unconfined(ActionKind::AgentTrace).confined_to(Some(OTHER.to_owned())),
    );

    assert_eq!(purge_confined(&mut credentials, MINE), 2);

    let mut left: Vec<&str> = credentials.keys().map(String::as_str).collect();
    left.sort();
    assert_eq!(left, ["local", "other"]);
}
