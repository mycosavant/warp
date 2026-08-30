use super::*;

/// A conversation nothing is serving reports nothing — not zero.
///
/// The distinction is the whole reason the field is optional. A conversation
/// that finished an hour ago is not "quiet for 0 seconds", and a caller polling
/// for a wedge must not see one everywhere it looks.
#[test]
fn a_conversation_with_no_turn_in_flight_reports_nothing() {
    assert!(quiet_for("t1410-absent").is_none());
}

/// The clock starts at the turn, not at the first thing the agent says.
///
/// A turn that wedges before its agent has said anything is the case with the
/// least other signal — no output, no tool call, no approval — so it is the one
/// that most needs a number.
#[test]
fn a_turn_is_watched_from_the_moment_it_starts() {
    let _watch = watch("t1410-silent".to_owned());

    let (quiet, tool, waiting) = quiet_for("t1410-silent").expect("a watched turn reports");

    assert!(quiet < 5, "a fresh turn is not already stale: {quiet}");
    assert_eq!(tool, None, "nothing has been announced yet");
    assert!(!waiting, "nothing has been asked yet either");
}

/// A tool call is remembered; later chatter refreshes the clock without erasing
/// it. That pairing is what T14.9 needed and could not get: the panel showed a
/// frozen `grep` while the CLI could say only `in_progress`.
#[test]
fn the_last_tool_survives_updates_that_are_not_tool_calls() {
    let _watch = watch("t1410-tool".to_owned());

    note("t1410-tool", Some("grep -rn kind_name".to_owned()));
    note("t1410-tool", None);

    let (_, tool, _) = quiet_for("t1410-tool").expect("a watched turn reports");
    assert_eq!(tool.as_deref(), Some("grep -rn kind_name"));
}

/// A later tool call replaces an earlier one, because the question is what it
/// wedged on, not what it started with.
#[test]
fn a_newer_tool_call_replaces_the_remembered_one() {
    let _watch = watch("t1410-second".to_owned());

    note("t1410-second", Some("first".to_owned()));
    note("t1410-second", Some("second".to_owned()));

    let (_, tool, _) = quiet_for("t1410-second").expect("a watched turn reports");
    assert_eq!(tool.as_deref(), Some("second"));
}

/// The record removes itself, so a finished conversation cannot report a stale
/// quiet time — the failure that would make this field worse than absent.
#[test]
fn the_record_is_gone_when_the_turn_ends() {
    {
        let _watch = watch("t1410-ends".to_owned());
        assert!(quiet_for("t1410-ends").is_some());
    }

    assert!(quiet_for("t1410-ends").is_none(), "the guard cleans up");
}

/// Noting against a turn that has ended is a no-op rather than a resurrection.
/// A notification can arrive as a turn is being torn down, and re-inserting
/// there would leak the entry the guard just removed.
#[test]
fn noting_after_the_turn_ended_does_not_recreate_it() {
    {
        let _watch = watch("t1410-late".to_owned());
    }

    note("t1410-late", Some("too late".to_owned()));

    assert!(quiet_for("t1410-late").is_none());
}

/// Quiet because it is asking is not the same as quiet because it is gone.
///
/// The distinction this file lacked for an hour. A turn parked on an approval
/// reported 171 seconds of quiet — true, and indistinguishable from the wedge
/// the number exists to reveal. Waiting forever on a question is the design, so
/// a reader who sees an alarm for it learns to discount the alarm.
#[test]
fn a_turn_waiting_on_a_person_says_so() {
    let _watch = watch("t1410-asking".to_owned());
    assert_eq!(quiet_for("t1410-asking").map(|(_, _, w)| w), Some(false));

    let asking = waiting_on_a_person("t1410-asking");
    assert_eq!(quiet_for("t1410-asking").map(|(_, _, w)| w), Some(true));

    drop(asking);
    assert_eq!(
        quiet_for("t1410-asking").map(|(_, _, w)| w),
        Some(false),
        "answering the question stops the claim"
    );
}

/// Two questions at once, and answering one does not clear the other.
///
/// A count rather than a flag, because an agent may have more than one request
/// outstanding — measured on the CLI-agent path, where two concurrent turns both
/// opened with JSON-RPC id 0 and one evicted the other from a map keyed too
/// loosely. Same failure shape, one field over.
#[test]
fn answering_one_of_two_questions_does_not_clear_the_other() {
    let _watch = watch("t1410-two".to_owned());

    let first = waiting_on_a_person("t1410-two");
    let second = waiting_on_a_person("t1410-two");
    drop(first);

    assert_eq!(
        quiet_for("t1410-two").map(|(_, _, w)| w),
        Some(true),
        "one answered, one still outstanding"
    );

    drop(second);
    assert_eq!(quiet_for("t1410-two").map(|(_, _, w)| w), Some(false));
}

/// A guard outliving its turn does not resurrect the record, and does not
/// panic. Teardown order is not something a caller controls.
#[test]
fn a_waiting_guard_outliving_its_turn_is_harmless() {
    let asking = {
        let _watch = watch("t1410-order".to_owned());
        waiting_on_a_person("t1410-order")
    };

    drop(asking);

    assert!(quiet_for("t1410-order").is_none());
}
