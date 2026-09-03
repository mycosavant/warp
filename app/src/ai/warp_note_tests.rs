use warp_multi_agent_api as api;

use super::{Note, TAG, is_tagged};

fn blank_message() -> api::Message {
    api::Message {
        id: "m-1".to_owned(),
        task_id: "t-1".to_owned(),
        request_id: "r-1".to_owned(),
        ..Default::default()
    }
}

/// The wire form round-trips: what a translator writes is what the conversion
/// reads back, headline and detail intact.
#[test]
fn a_note_survives_the_wire() {
    let note = Note::new(
        "Waiting for permission: **Write file**",
        "Answer with `warpctrl agent approve 1`.\n\nIt says this acts on `a.txt`.",
    );

    assert_eq!(Note::from_wire(&note.to_wire()), note);
}

/// A note that is only its headline is one line on the wire, not one line and
/// a blank, so a transcript reader sees no gap where nothing was said.
#[test]
fn a_headline_only_note_is_one_line() {
    let note = Note::headline("Answered: **yes**, for this one call.");

    assert_eq!(note.to_wire(), "Answered: **yes**, for this one call.");
    assert_eq!(Note::from_wire(&note.to_wire()), note);
}

/// Text that was never composed as a note still reads as one: the first
/// paragraph is the headline and the rest is the detail. This is what a
/// pre-tag build's text would look like if it were ever tagged, and what
/// `mode.rs`'s prose looks like today.
#[test]
fn untagged_prose_splits_at_its_first_blank_line() {
    let note = Note::from_wire("First sentence.\n\nSecond paragraph.\n\nThird.");

    assert_eq!(note.headline, "First sentence.");
    assert_eq!(note.detail, "Second paragraph.\n\nThird.");
}

/// Leading blank lines are not an empty headline. A note with nothing visible
/// is a note that was not said, and the reader would be left with a chevron
/// and no sentence to attach it to.
#[test]
fn leading_blank_lines_do_not_become_an_empty_headline() {
    let note = Note::from_wire("\n\nOnly this.");

    assert_eq!(note.headline, "Only this.");
    assert!(note.detail.is_empty());
}

/// The message is an ordinary `AgentOutput` with the tag on the opaque field,
/// and the caller's identity fields are untouched. The tag is the whole
/// channel: a build that does not know it renders the text as it always did.
#[test]
fn into_message_tags_an_agent_output_and_keeps_the_callers_ids() {
    let message = Note::new("Head", "Body").into_message(blank_message());

    assert_eq!(message.id, "m-1");
    assert_eq!(message.task_id, "t-1");
    assert_eq!(message.request_id, "r-1");
    assert!(is_tagged(&message.server_message_data));
    assert_eq!(
        message.message,
        Some(api::message::Message::AgentOutput(
            api::message::AgentOutput {
                text: "Head\n\nBody".to_owned()
            }
        ))
    );
}

/// The empty payload every other message carries is not a note, and neither
/// is a near miss. Compared, never parsed.
#[test]
fn only_the_exact_tag_marks_a_note() {
    assert!(!is_tagged(""));
    assert!(!is_tagged("warp-fork/note "));
    assert!(!is_tagged("warp-fork/NOTE"));
    assert!(is_tagged(TAG));
}
