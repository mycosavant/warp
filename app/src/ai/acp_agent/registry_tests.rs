//! The registry's own rules. The exchange it serves is verified by running it
//! against a real agent — the fork's standard.
//!
//! These share one process-global map, so each test uses ids of its own rather
//! than asserting on the whole list.

use super::*;

fn request(id: &str) -> ParkedRequest {
    conversation_request(id, "conv-1")
}

fn conversation_request(id: &str, conversation: &str) -> ParkedRequest {
    ParkedRequest {
        approval_id: id.to_owned(),
        agent: "opencode".to_owned(),
        title: Some("echo hello > greeting.txt".to_owned()),
        conversation_id: conversation.to_owned(),
        tool_name: Some("execute".to_owned()),
        tool_input: Some("echo hello > greeting.txt".to_owned()),
        session_directory: Some("/tmp/project".to_owned()),
        session_id: Some("ses_1".to_owned()),
        acts_on: vec!["/tmp/project".to_owned()],
        options_offered: vec!["Allow once".to_owned(), "Reject".to_owned()],
        approve_selects: Some("once".to_owned()),
        approve_refused_because: None,
    }
}

fn listed(id: &str) -> Option<ParkedRequest> {
    waiting().into_iter().find(|req| req.approval_id == id)
}

/// The whole point: a request is visible while something waits on it, and the
/// wait ends when a decision is sent.
#[test]
fn a_parked_request_is_listed_until_it_is_answered() {
    let id = "registry-answered";
    let (_waiting, mut wait) = park(request(id));

    assert!(
        listed(id).is_some(),
        "a parked request is waiting on a person"
    );

    assert!(
        answer(id, Decision::Deny, Surface::ControlPlane),
        "answering a live request works"
    );
    assert_eq!(
        wait.try_recv().ok().flatten(),
        Some(Answer {
            decision: Decision::Deny,
            surface: Surface::ControlPlane
        })
    );
    assert!(
        listed(id).is_none(),
        "an answer that landed makes the entry disappear — `agent.approvals`' own contract"
    );
}

/// **The cleanup that stops a cancelled turn advertising a dead question.**
///
/// The guard is held by the connection task that is waiting, so a turn that is
/// cancelled — measured on T14.6 as killing the agent within 2s — drops it along
/// with everything else. Without this the map would keep offering a request no
/// one can answer, and answering it would send a decision into a closed channel.
#[test]
fn dropping_the_waiter_stops_advertising_the_request() {
    let id = "registry-dropped";
    let (waiting_guard, _wait) = park(request(id));
    assert!(listed(id).is_some());

    drop(waiting_guard);

    assert!(
        listed(id).is_none(),
        "nothing is waiting, so nothing is asked"
    );
    assert!(
        !answer(id, Decision::Deny, Surface::ControlPlane),
        "and a late answer finds nothing rather than landing somewhere"
    );
}

/// Answering something that is not there is `false`, not a panic and not a
/// silent success — a phone can hold a stale entry, and the honest reply is
/// that the question is gone.
#[test]
fn answering_an_unknown_request_reports_that_it_is_gone() {
    assert!(!answer(
        "registry-never-existed",
        Decision::Deny,
        Surface::ControlPlane
    ));
}

/// Two requests parked at once stay distinguishable, and an answer reaches only
/// the one it names.
///
/// This is the key-choice under test. Keying on the JSON-RPC request id rather
/// than the tool call id is what keeps a second ask about the same call from
/// being answered by the first one's decision.
#[test]
fn an_answer_reaches_only_the_request_it_names() {
    let (first_guard, mut first) = park(request("registry-pair-a"));
    let (second_guard, mut second) = park(request("registry-pair-b"));

    assert!(answer(
        "registry-pair-a",
        Decision::Deny,
        Surface::ControlPlane
    ));

    assert_eq!(
        first.try_recv().ok().flatten(),
        Some(Answer {
            decision: Decision::Deny,
            surface: Surface::ControlPlane
        })
    );
    assert_eq!(
        second.try_recv().ok().flatten(),
        None,
        "the other request is still waiting on its own answer"
    );
    assert!(listed("registry-pair-b").is_some());

    drop(first_guard);
    drop(second_guard);
}

/// The offered options are carried, because an offer that goes unrecorded is
/// the finding `acp_permission::is_more_than_an_answer` exists for. Carrying
/// them is not offering them: nothing here turns one into a control.
#[test]
fn the_options_the_agent_offered_are_kept_as_data() {
    let id = "registry-options";
    let (_guard, _wait) = park(request(id));

    let entry = listed(id).expect("just parked");
    assert_eq!(entry.options_offered, ["Allow once", "Reject"]);
}

/// **The cascade this registry shipped with for one build, measured live.**
///
/// JSON-RPC request ids are per-connection, and two concurrent `opencode`
/// sessions both opened with `0`. Against a process-global map keyed on that id
/// alone the failure was not a lost entry, it was two wrong answers:
///
/// 1. turn B parks over turn A, dropping A's sender;
/// 2. A's waiter reads a dropped sender as "answered" and denies at once, while
///    the panel still says it is waiting for a person;
/// 3. A's cleanup then removes the key — now B's entry — and B denies too.
///
/// Both turns reported success with the tool never run and nobody ever asked.
/// The key is scoped to the turn now, so this test builds the collision by hand
/// to check the second half of the fix: a waiter must never evict an entry that
/// is not its own.
#[test]
fn a_reused_key_cannot_make_one_waiter_answer_another() {
    let id = "registry-collision";
    let (first_guard, mut first) = park(request(id));
    let (_second_guard, mut second) = park(request(id));

    // Parking over the first is what a colliding key does; the eviction itself
    // is unavoidable once two entries claim one key.
    assert!(
        first.try_recv().is_err(),
        "the evicted waiter sees a closed channel — this is why keys must be unique"
    );

    // …but the evicted waiter's cleanup must not reach the entry that replaced
    // it. This is the half that turned one collision into two denials.
    drop(first_guard);

    assert!(
        listed(id).is_some(),
        "the surviving request is still waiting on a person"
    );
    assert_eq!(
        second.try_recv().ok().flatten(),
        None,
        "and nothing has answered it"
    );

    assert!(answer(id, Decision::Deny, Surface::ControlPlane));
    assert_eq!(
        second.try_recv().ok().flatten(),
        Some(Answer {
            decision: Decision::Deny,
            surface: Surface::ControlPlane
        })
    );
}

/// A panel showing one conversation gets that conversation's questions only.
///
/// There are three ids on a `ParkedRequest` and only one of them answers "is
/// this mine": the agent's `session_id` and Warp's `session_directory` are both
/// wrong for this, and both are the kind of near-miss a caller filtering by hand
/// would reach for. So the filter lives here rather than at the call site.
#[test]
fn a_conversations_questions_are_its_own() {
    let (_mine, _m) = park(conversation_request("registry-conv-mine", "conv-mine"));
    let (_theirs, _t) = park(conversation_request("registry-conv-theirs", "conv-theirs"));

    let mine = waiting_for("conv-mine");

    assert_eq!(mine.len(), 1, "one question belongs to this conversation");
    assert_eq!(mine[0].approval_id, "registry-conv-mine");
    assert!(
        waiting_for("conv-nobody").is_empty(),
        "a conversation with no question waiting gets an empty list, not everyone else's"
    );
}

/// **The surface a caller passed is the surface the waiter receives** — the
/// whole point of T14.17's threading, and the one thing about it that could
/// silently regress.
///
/// It is worth a test rather than being obvious, because the failure it guards
/// is a *quiet* one: a default surface substituted anywhere in the plumbing
/// would compile, would answer the request correctly, and would write an audit
/// line naming the wrong door. Nothing downstream would notice, because nothing
/// downstream knows what the right answer was.
///
/// Both values are exercised against both decisions, so a mapping that
/// collapsed to a constant cannot pass.
#[test]
fn an_answer_carries_the_surface_it_came_from() {
    for (index, (decision, surface)) in [
        (Decision::Allow, Surface::Panel),
        (Decision::Allow, Surface::ControlPlane),
        (Decision::Deny, Surface::Panel),
        (Decision::Deny, Surface::ControlPlane),
    ]
    .into_iter()
    .enumerate()
    {
        let id = format!("registry-surface-{index}");
        let (_waiting, mut wait) = park(request(&id));

        assert!(answer(&id, decision, surface));
        assert_eq!(
            wait.try_recv().ok().flatten(),
            Some(Answer { decision, surface }),
            "the answer must carry the surface it was given, unchanged"
        );
    }
}

/// The wire names are written down, so a variant rename cannot rewrite the
/// history of a log people grep.
///
/// `Surface::as_str` spells them out rather than deriving them for exactly this
/// reason; this is the test that makes that choice load-bearing instead of
/// merely stated.
#[test]
fn the_surface_wire_names_are_stable() {
    assert_eq!(Surface::ControlPlane.as_str(), "control_plane");
    assert_eq!(Surface::Panel.as_str(), "panel");
}
