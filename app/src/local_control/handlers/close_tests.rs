//! What `window.close` is allowed to claim.

use super::*;

/// **`ok` must never be the whole story for a cancellable close.**
///
/// The failure this guards is silent and measured: `window close` answered
/// `ok: true` while the process stayed up, three times in one session, and
/// stale instances then made every later `warpctrl` call answer
/// `ambiguous_instance`. A caller — or a shell script — that greps for `"ok"`
/// sails straight past it.
///
/// So the assertion is not that some extra field exists; it is that the payload
/// carries a **qualifier a reader has to account for**, while leaving `ok`
/// itself intact for every caller already reading it.
#[test]
fn a_cancellable_close_does_not_report_itself_as_a_closed_window() {
    let response = close_requested(&Some(InstanceId("inst-1".to_owned())));

    assert_eq!(
        response["ok"], true,
        "the request was accepted and dispatched, which is what `ok` has always meant here"
    );
    assert_eq!(
        response["close"], "requested",
        "…but the verb is *requested*, because this handler returns before the \
         window has had a chance to refuse"
    );
    assert_eq!(
        response["cancellable"], true,
        "the close is sent with TerminationMode::Cancellable, whose own doc says \
         the termination can be interrupted"
    );
    assert_eq!(response["action"], "window.close");
}

/// The result tells a caller what to do about the uncertainty, rather than
/// merely admitting to it.
///
/// A field saying "this might not have worked" and nothing else moves the
/// problem to the reader. The two remedies named are the measured ones — a CLI
/// agent alive in a pane, and an in-flight agent turn — and `instance list` is
/// the check that actually settles it, since a closed instance leaves no
/// discovery record.
#[test]
fn the_result_names_the_check_that_settles_it() {
    let response = close_requested(&None);
    let verify = response["verify"].as_str().expect("a verify sentence");

    assert!(
        verify.contains("instance list"),
        "the caller is pointed at the check that answers the question, got: {verify}"
    );
    // Not `"turn"`: a four-character substring that "returned", "turns" and
    // "turned" all satisfy is close to unfalsifiable, and this assertion exists
    // to catch the sentence being reworded into uselessness.
    for remedy in ["CLI agent", "in-flight agent turn"] {
        assert!(
            verify.contains(remedy),
            "the two measured blockers are named so a caller is not left guessing, \
             missing {remedy:?} in: {verify}"
        );
    }
}

/// **It must not claim to know *why* a close would be refused.**
///
/// The mechanism has not been established by running it, and one candidate was
/// ruled out by reading — `CloseSessionConfirmationDialog` covers pane and tab
/// closes and has no window arm. Asserting a cause on that evidence would be
/// the invented certainty `unconfined_reason` was corrected for in T14.8, and a
/// control-plane result is read with less context than a panel message, not
/// more.
#[test]
fn it_does_not_invent_a_reason_the_close_might_be_refused() {
    let response = close_requested(&None);
    let rendered = response.to_string();

    for invented in ["because", "dialog", "confirmation", "blocked by"] {
        assert!(
            !rendered.contains(invented),
            "the payload may say a close can be refused and what to check; it may \
             not name a cause nobody has measured — found {invented:?} in: {rendered}"
        );
    }
}
