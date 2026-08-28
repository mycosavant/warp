//! What Warp says it knows, decided without an agent.
//!
//! The fixtures reproduce a session measured on 2026-08-27: mode `default`,
//! whose own description is *"Standard behavior, prompts for dangerous
//! operations"*, in which a file write was put to Warp and a shell command was
//! not. That run is the whole argument for reporting per call, so it is the
//! thing the tests are built out of.

use agent_client_protocol::schema::v1::{
    CurrentModeUpdate, PermissionOption, PermissionOptionKind, SessionMode, ToolCall,
    ToolCallUpdate, ToolCallUpdateFields,
};

use super::*;

const WRITE: &str = "toolu_write";
const SHELL: &str = "toolu_shell";

fn modes(current: &'static str) -> SessionModeState {
    SessionModeState::new(
        current,
        vec![
            SessionMode::new("default", "Manual")
                .description("Standard behavior, prompts for dangerous operations"),
            SessionMode::new("auto", "Auto")
                .description("Use a model classifier to approve/deny permission prompts"),
        ],
    )
}

fn tool_call(id: &'static str, title: &'static str, kind: ToolKind) -> SessionUpdate {
    SessionUpdate::ToolCall(ToolCall::new(id, title).kind(kind))
}

fn permission_for(id: &'static str, title: &'static str) -> RequestPermissionRequest {
    RequestPermissionRequest::new(
        "session-1",
        ToolCallUpdate::new(id, ToolCallUpdateFields::new().title(title)),
        vec![PermissionOption::new(
            "allow",
            "Allow Once",
            PermissionOptionKind::AllowOnce,
        )],
    )
}

/// The measured session, replayed: same mode, one call asked about and one not.
fn measured_session() -> Ledger {
    let mut ledger = Ledger::new();
    ledger.observe_session(Some(modes("default")));
    ledger.observe_update(&tool_call(WRITE, "Write a.txt", ToolKind::Edit));
    ledger.observe_request(&permission_for(WRITE, "Write a.txt"));
    ledger.observe_answer(&ToolCallId::new(WRITE), "selected", Some("Allow Once"));
    ledger.observe_update(&tool_call(SHELL, "Terminal", ToolKind::Execute));
    ledger
}

/// The finding T14.3 exists for: one mode, two calls, opposite gating. A report
/// that could not show this would be the report that invites the wrong
/// inference.
#[test]
fn one_mode_can_cover_a_call_that_was_asked_about_and_one_that_was_not() {
    let report = measured_session().report();

    assert_eq!(
        report.mode_the_agent_declared_at_session_start.as_deref(),
        Some("default")
    );
    assert_eq!(report.calls.len(), 2);
    assert_eq!(
        report.calls[0].permission_requests_received, 1,
        "the write was put to Warp"
    );
    assert_eq!(
        report.calls[1].permission_requests_received, 0,
        "the shell command was not put to Warp"
    );
    assert_eq!(report.calls_warp_was_not_asked_about, 1);
}

/// The mode is quoted from the agent, description and all, because paraphrasing
/// it would make Warp the author of a claim it cannot check.
#[test]
fn the_mode_description_is_the_agents_own_words() {
    let report = measured_session().report();

    assert_eq!(
        report.its_description_from_the_agent.as_deref(),
        Some("Standard behavior, prompts for dangerous operations")
    );
}

/// `NewSessionResponse.modes` is optional. An agent that declares nothing is a
/// third state, and filling it in would be inventing the very fact the module
/// refuses to infer.
#[test]
fn an_agent_that_declares_no_mode_is_reported_as_declaring_none() {
    let mut ledger = Ledger::new();
    ledger.observe_session(None);
    ledger.observe_update(&tool_call(SHELL, "Terminal", ToolKind::Execute));

    let report = ledger.report();

    assert_eq!(report.mode_the_agent_declared_at_session_start, None);
    assert_eq!(report.its_description_from_the_agent, None);
    assert_eq!(
        report.calls_warp_was_not_asked_about, 1,
        "a missing mode does not change what Warp was or was not asked"
    );
}

/// The caveat travels with the numbers rather than living in a document nobody
/// reading a transcript will open.
#[test]
fn the_report_carries_the_sentence_that_stops_the_wrong_reading() {
    let report = measured_session().report();

    assert!(
        report.caveat.contains("does not read"),
        "the caveat should say Warp does not read the user's rules, got: {}",
        report.caveat
    );
    assert!(
        report.caveat.contains("does not predict"),
        "the caveat should refuse the mode-to-gating inference, got: {}",
        report.caveat
    );
}

/// A permission request can arrive before any `tool_call` notification, and the
/// call Warp *was* asked about is the last one that may go missing.
#[test]
fn a_request_before_any_notification_still_produces_a_call() {
    let mut ledger = Ledger::new();
    ledger.observe_request(&permission_for(WRITE, "Write a.txt"));

    let report = ledger.report();

    assert_eq!(report.calls.len(), 1);
    assert_eq!(report.calls[0].permission_requests_received, 1);
    assert_eq!(report.calls_warp_was_not_asked_about, 0);
}

/// The hazard T14.2 wrote down and this struct first shipped against: nothing in
/// the schema forbids an agent re-asking on the same `toolCallId` after a
/// refusal. A boolean would record the second ask as the first and lose its
/// answer, which is the whole reason this is a count and a list.
#[test]
fn a_second_request_on_the_same_id_is_counted_and_its_answer_kept() {
    let mut ledger = Ledger::new();
    ledger.observe_request(&permission_for(WRITE, "Write a.txt"));
    ledger.observe_answer(&ToolCallId::new(WRITE), "selected", Some("Deny"));
    ledger.observe_request(&permission_for(WRITE, "Write a.txt, smaller"));
    ledger.observe_answer(&ToolCallId::new(WRITE), "cancelled", None);

    let report = ledger.report();

    assert_eq!(
        report.calls.len(),
        1,
        "it is one tool call, asked about twice"
    );
    assert_eq!(report.calls[0].permission_requests_received, 2);
    assert_eq!(
        report.calls[0].answers_warp_sent,
        vec!["selected".to_owned(), "cancelled".to_owned()],
        "both answers survive, in order"
    );
}

/// Later updates refine a call rather than duplicating it — the measured stream
/// sends `tool_call` with a placeholder title and then corrects it.
#[test]
fn updates_for_the_same_id_refine_one_record() {
    let mut ledger = Ledger::new();
    ledger.observe_update(&tool_call(WRITE, "Preparing file…", ToolKind::Edit));
    ledger.observe_update(&SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
        WRITE,
        ToolCallUpdateFields::new()
            .title("Write a.txt".to_owned())
            .status(ToolCallStatus::Completed),
    )));

    let report = ledger.report();

    assert_eq!(report.calls.len(), 1);
    assert_eq!(report.calls[0].title.as_deref(), Some("Write a.txt"));
    assert_eq!(report.calls[0].status.as_deref(), Some("completed"));
}

/// An announcement is a *transition Warp watched*, and it does not overwrite the
/// opening declaration — that used to be a `mode_the_agent_declared` field which
/// a reader took for the current mode, and the current mode is the one thing here
/// Warp does not know.
#[test]
fn an_announced_mode_change_does_not_rewrite_what_was_declared_at_the_start() {
    let mut ledger = Ledger::new();
    ledger.observe_session(Some(modes("default")));
    ledger.observe_update(&SessionUpdate::CurrentModeUpdate(CurrentModeUpdate::new(
        "auto",
    )));

    let report = ledger.report();

    assert_eq!(
        report.mode_the_agent_declared_at_session_start.as_deref(),
        Some("default"),
        "the opening claim is a wire-fact and stays one"
    );
    let change = &report.mode_changes_the_agent_announced[0];
    assert_eq!(change.to, "auto");
    assert_eq!(
        change.description_from_the_agent.as_deref(),
        Some("Use a model classifier to approve/deny permission prompts"),
        "the mode moved to is quoted in the agent's own words, where the move is recorded"
    );
}

/// Chained announcements read from the last one, not from the opening
/// declaration, or the second change would claim to start where the first did.
#[test]
fn a_second_announced_change_starts_from_the_first() {
    let mut ledger = Ledger::new();
    ledger.observe_session(Some(modes("default")));
    ledger.observe_update(&SessionUpdate::CurrentModeUpdate(CurrentModeUpdate::new(
        "auto",
    )));
    ledger.observe_update(&SessionUpdate::CurrentModeUpdate(CurrentModeUpdate::new(
        "default",
    )));

    let changes = ledger.report().mode_changes_the_agent_announced;

    assert_eq!(changes[1].from.as_deref(), Some("auto"));
    assert_eq!(changes[1].to, "default");
}

/// An agent re-declaring itself mid-session is the rug-pull shape, and applying
/// the change without keeping the announcement would lose the only fact that
/// matters: that nobody asked for it.
#[test]
fn an_announced_mode_change_is_kept_as_well_as_applied() {
    let mut ledger = Ledger::new();
    ledger.observe_session(Some(modes("default")));
    ledger.observe_update(&SessionUpdate::CurrentModeUpdate(CurrentModeUpdate::new(
        "auto",
    )));

    let report = ledger.report();

    assert_eq!(report.mode_changes_the_agent_announced.len(), 1);
    let change = &report.mode_changes_the_agent_announced[0];
    assert_eq!(change.from.as_deref(), Some("default"));
    assert_eq!(change.to, "auto");
    assert!(
        !change.answers_a_set_mode_warp_sent,
        "this probe never sends session/set_mode, so it asked for nothing"
    );
}

/// The list is printed even when empty, because "the agent did not re-declare
/// itself" is a claim worth evidencing rather than assuming.
#[test]
fn a_session_with_no_announced_change_reports_an_empty_list() {
    let report = measured_session().report();

    assert!(report.mode_changes_the_agent_announced.is_empty());
    assert!(
        report.mode_requests_warp_sent.is_empty(),
        "a session Warp stayed out of must be distinguishable from one where it asked"
    );
}

/// The one change Warp participated in. Without this the probe's own
/// `session/set_mode` would be reported as the agent widening itself, which is
/// the rug-pull sentence and would be false.
#[test]
fn a_change_answering_warps_request_is_reported_as_requested() {
    let mut ledger = Ledger::new();
    ledger.observe_session(Some(modes("default")));
    ledger.observe_mode_request("auto");
    ledger.observe_mode_acknowledgement("auto");
    ledger.observe_update(&SessionUpdate::CurrentModeUpdate(CurrentModeUpdate::new(
        "auto",
    )));

    let report = ledger.report();

    assert!(report.mode_changes_the_agent_announced[0].answers_a_set_mode_warp_sent);
    assert_eq!(
        report.mode_requests_warp_sent,
        vec![ModeRequest {
            mode_id: "auto".to_owned(),
            the_agent_acknowledged: true,
            the_agent_announced_it_afterwards: true,
        }]
    );
}

/// `SetSessionModeResponse` has no fields, so an acknowledgement is one bit: no
/// error. Reporting it as though the mode were in force would be the mode
/// picker's version of claiming protection the fork does not have.
#[test]
fn an_acknowledgement_alone_is_not_reported_as_the_mode_taking_effect() {
    let mut ledger = Ledger::new();
    ledger.observe_session(Some(modes("default")));
    ledger.observe_mode_request("auto");
    ledger.observe_mode_acknowledgement("auto");

    let report = ledger.report();

    assert!(report.mode_requests_warp_sent[0].the_agent_acknowledged);
    assert!(!report.mode_requests_warp_sent[0].the_agent_announced_it_afterwards);
    assert_eq!(
        report.mode_the_agent_declared_at_session_start.as_deref(),
        Some("default"),
        "the opening declaration is untouched by a mode Warp asked for and was never told about"
    );
    assert!(report.mode_changes_the_agent_announced.is_empty());
}

/// The measured case, and the reason the current-mode field was deleted. Warp
/// asked for `plan`, the agent acknowledged and behaved as though in plan mode,
/// and announced nothing — so the only two mode facts in the report disagree with
/// the session, and the report has to be readable as *that* rather than as `auto`.
#[test]
fn a_silently_honoured_request_leaves_the_report_saying_only_what_it_saw() {
    let mut ledger = Ledger::new();
    ledger.observe_session(Some(modes("auto")));
    ledger.observe_mode_request("plan");
    ledger.observe_mode_acknowledgement("plan");

    let value = serde_json::to_value(ledger.report()).expect("the report should render");

    assert!(
        value.get("mode_the_agent_declared").is_none(),
        "no field may read as the mode the session is in, got: {value}"
    );
    assert_eq!(
        value["mode_the_agent_declared_at_session_start"], "auto",
        "named for the moment it was true"
    );
    assert_eq!(value["mode_requests_warp_sent"][0]["mode_id"], "plan");
    assert_eq!(
        value["mode_changes_the_agent_announced"],
        serde_json::json!([]),
        "nothing was announced, and the empty list is the evidence of that"
    );
}

/// One request buys credit for one announcement. An agent that acknowledges,
/// announces, and then announces the same mode again unprompted is doing
/// something Warp did not ask for the second time, and the record has to show it.
#[test]
fn a_second_change_to_the_same_mode_is_not_credited_to_the_one_request() {
    let mut ledger = Ledger::new();
    ledger.observe_session(Some(modes("default")));
    ledger.observe_mode_request("auto");
    ledger.observe_mode_acknowledgement("auto");
    ledger.observe_update(&SessionUpdate::CurrentModeUpdate(CurrentModeUpdate::new(
        "auto",
    )));
    ledger.observe_update(&SessionUpdate::CurrentModeUpdate(CurrentModeUpdate::new(
        "auto",
    )));

    let changes = ledger.report().mode_changes_the_agent_announced;

    assert!(changes[0].answers_a_set_mode_warp_sent);
    assert!(!changes[1].answers_a_set_mode_warp_sent);
}

/// The announcement may overtake the response — nothing orders a notification
/// against a reply — and a request recorded afterwards would miss its own answer.
#[test]
fn a_change_that_arrives_before_the_acknowledgement_is_still_credited() {
    let mut ledger = Ledger::new();
    ledger.observe_session(Some(modes("default")));
    ledger.observe_mode_request("auto");
    ledger.observe_update(&SessionUpdate::CurrentModeUpdate(CurrentModeUpdate::new(
        "auto",
    )));
    ledger.observe_mode_acknowledgement("auto");

    let report = ledger.report();

    assert!(report.mode_changes_the_agent_announced[0].answers_a_set_mode_warp_sent);
    assert!(report.mode_requests_warp_sent[0].the_agent_acknowledged);
    assert!(report.mode_requests_warp_sent[0].the_agent_announced_it_afterwards);
}

/// An agent that never answers is a case the report must be able to show, since
/// the probe stops on a refusal and the record is the only account of why.
#[test]
fn an_unanswered_request_is_reported_rather_than_dropped() {
    let mut ledger = Ledger::new();
    ledger.observe_session(Some(modes("default")));
    ledger.observe_mode_request("plan");

    let report = ledger.report();

    assert_eq!(report.mode_requests_warp_sent.len(), 1);
    assert!(!report.mode_requests_warp_sent[0].the_agent_acknowledged);
    assert!(!report.mode_requests_warp_sent[0].the_agent_announced_it_afterwards);
}

/// Warp refuses every option that declares a change, so the offered list fills
/// and the authorized list stays empty — and printing an empty list is the
/// point, because it is the claim a person would otherwise take on trust.
#[test]
fn a_declared_transition_is_reported_as_offered_and_not_authorized() {
    let mut ledger = Ledger::new();
    let request = RequestPermissionRequest::new(
        "session-1",
        ToolCallUpdate::new(
            WRITE,
            ToolCallUpdateFields::new()
                .title("Write a.txt")
                .kind(ToolKind::Edit),
        ),
        vec![
            PermissionOption::new("allow", "Allow Once", PermissionOptionKind::AllowOnce),
            PermissionOption::new(
                "allow_always",
                "Always Allow",
                PermissionOptionKind::AllowAlways,
            )
            .meta(
                serde_json::json!({
                    "permission": {
                        "version": 1,
                        "changes": [{ "type": "permission_mode", "mode": "acceptEdits" }]
                    }
                })
                .as_object()
                .expect("the fixture is an object")
                .clone(),
            ),
        ],
    );
    ledger.observe_request(&request);
    ledger.observe_answer(&ToolCallId::new(WRITE), "selected", Some("Allow Once"));

    let report = ledger.report();

    assert_eq!(
        report.transitions_offered.len(),
        1,
        "on a call whose effect is bounded, only the option that declares more than an answer"
    );
    assert_eq!(report.transitions_offered[0].option_name, "Always Allow");
    assert_eq!(
        report.transitions_offered[0].disclosed_as,
        Disclosure::ADeclarationThisBuildCanRead
    );
    assert_eq!(
        report.transitions_offered[0]
            .declared
            .as_ref()
            .map(|declared| declared[0]["mode"].clone()),
        Some(serde_json::json!("acceptEdits")),
        "the declaration is quoted verbatim, not summarised"
    );
    assert!(
        report.transitions_authorized_by_warp.is_empty(),
        "nothing carrying a declared change is ever selected"
    );
}

/// A declaration this build cannot read is still reported — the absence of
/// detail is the finding, and dropping the entry would hide it.
#[test]
fn an_unreadable_declaration_is_reported_as_one_this_build_cannot_read() {
    let mut ledger = Ledger::new();
    ledger.observe_request(&RequestPermissionRequest::new(
        "session-1",
        ToolCallUpdate::new(WRITE, ToolCallUpdateFields::new().kind(ToolKind::Edit)),
        vec![
            PermissionOption::new("allow", "Allow Once", PermissionOptionKind::AllowOnce).meta(
                serde_json::json!({ "permission": { "version": 99 } })
                    .as_object()
                    .expect("the fixture is an object")
                    .clone(),
            ),
        ],
    ));

    let report = ledger.report();

    assert_eq!(report.transitions_offered.len(), 1);
    assert_eq!(
        report.transitions_offered[0].disclosed_as,
        Disclosure::ADeclarationThisBuildCannotRead
    );
    assert_eq!(report.transitions_offered[0].declared, None);
}

/// The measured `ExitPlanMode` menu, and the falsehood it produced: the shipped
/// report said `transitions_offered: []` for a session whose one event was a
/// five-option policy menu, because the list was fed only from `_meta` and none
/// of the five had any. The offer is the fact worth having, now that nothing
/// here can accept it.
#[test]
fn a_transition_disclosed_only_in_the_option_names_is_still_recorded() {
    let mut ledger = Ledger::new();
    ledger.observe_request(&RequestPermissionRequest::new(
        "session-1",
        ToolCallUpdate::new(
            WRITE,
            ToolCallUpdateFields::new()
                .title("Ready to code?")
                .kind(ToolKind::SwitchMode),
        ),
        vec![
            PermissionOption::new(
                "bypassPermissions",
                "Yes, and bypass permissions",
                PermissionOptionKind::AllowAlways,
            ),
            PermissionOption::new(
                "default",
                "Yes, and manually approve edits",
                PermissionOptionKind::AllowOnce,
            ),
            PermissionOption::new(
                "plan",
                "No, keep planning",
                PermissionOptionKind::RejectOnce,
            ),
        ],
    ));

    let offered = ledger.report().transitions_offered;

    assert_eq!(
        offered.len(),
        2,
        "both ways of saying yes are transitions; the refusal is not"
    );
    assert!(
        offered
            .iter()
            .all(|transition| transition.disclosed_as == Disclosure::TheOptionsNameOnly),
        "the agent said what these do in English and nowhere else"
    );
    assert!(
        !offered
            .iter()
            .any(|transition| transition.option_name == "No, keep planning"),
        "recording a refusal here would put \"authorized by Warp: No, keep planning\" in a report"
    );
}

/// The field name is about Warp's inbox, and the serialized form is what a
/// reader and `jq` see — so it is pinned rather than left to a rename.
#[test]
fn the_serialized_report_names_warp_rather_than_the_agent() {
    let report = measured_session().report();

    let value = serde_json::to_value(&report).expect("the report should render");

    assert_eq!(value["calls"][1]["permission_requests_received"], 0);
    assert_eq!(value["calls_warp_was_not_asked_about"], 1);
    assert!(
        value.get("approved").is_none() && value.get("unapproved").is_none(),
        "nothing here may be named as an approval verdict, got: {value}"
    );
}
