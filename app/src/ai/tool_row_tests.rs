use field_mask::FieldMaskOperation;
use warp_multi_agent_api as api;

use super::{Row, ToolRowState, UPDATE_MASK, state_of, tag};

fn announced() -> api::Message {
    Row::new(ToolRowState::Running, "Running cargo --version", "").into_message(api::Message {
        id: "m-7".to_owned(),
        task_id: "t-1".to_owned(),
        request_id: "r-1".to_owned(),
        timestamp: Some(prost_types::Timestamp {
            seconds: 1_700_000_000,
            nanos: 0,
        }),
        ..Default::default()
    })
}

/// Every state has a tag and every tag names its state; nothing else does.
#[test]
fn each_state_round_trips_through_its_tag_and_nothing_else_is_a_row() {
    for state in [
        ToolRowState::Running,
        ToolRowState::Done,
        ToolRowState::Failed,
        ToolRowState::Denied,
        ToolRowState::Interrupted,
    ] {
        assert_eq!(state_of(tag(state)), Some(state));
    }
    assert_eq!(state_of(""), None);
    assert_eq!(state_of("warp-fork/note"), None);
    assert_eq!(state_of("warp-fork/tool/"), None);
    assert_eq!(state_of("warp-fork/tool/Done"), None);
    assert_eq!(state_of("warp-fork/tool/done "), None);
}

/// The row's text is a note's text, so the transcript and a pre-tag build
/// read it the way they read everything else.
#[test]
fn a_row_survives_the_wire_with_its_state_from_the_tag() {
    let row = Row::new(
        ToolRowState::Done,
        "Ran cargo --version",
        "Print cargo version\n\n```console\ncargo 1.92.0\n```",
    );
    let message = row.clone().into_message(announced());

    let state = state_of(&message.server_message_data).expect("a tagged row");
    let text = match message.message {
        Some(api::message::Message::AgentOutput(output)) => output.text,
        other => panic!("not an AgentOutput: {other:?}"),
    };
    assert_eq!(Row::from_wire(state, &text), row);
    assert_eq!(message.id, "m-7");
}

/// **The path the module twice declined to guess, run against the real
/// descriptor.** An update for the announced message with [`UPDATE_MASK`]
/// replaces the body and the tag and leaves identity and time as announced.
/// Calibrated: masking the `oneof`'s name instead (`message`) is a no-op, so
/// a wrong path here would not error, it would leave every row `Running`.
#[test]
fn the_update_mask_replaces_the_body_and_the_tag_and_nothing_else() {
    let existing = announced();
    let patch = Row::new(ToolRowState::Done, "Ran cargo --version", "cargo 1.92.0").into_message(
        api::Message {
            id: "m-7".to_owned(),
            // Deliberately different, to show the mask does not carry them.
            task_id: "t-other".to_owned(),
            request_id: "r-other".to_owned(),
            timestamp: None,
            ..Default::default()
        },
    );
    let mask = prost_types::FieldMask {
        paths: UPDATE_MASK.iter().map(|path| (*path).to_owned()).collect(),
    };

    let merged = FieldMaskOperation::update(&api::MESSAGE_DESCRIPTOR, &existing, &patch, mask)
        .apply()
        .expect("the mask applies");

    assert_eq!(
        state_of(&merged.server_message_data),
        Some(ToolRowState::Done)
    );
    assert_eq!(
        merged.message,
        Some(api::message::Message::AgentOutput(
            api::message::AgentOutput {
                text: "Ran cargo --version\n\ncargo 1.92.0".to_owned()
            }
        ))
    );
    assert_eq!(merged.task_id, "t-1");
    assert_eq!(merged.request_id, "r-1");
    assert_eq!(merged.timestamp, existing.timestamp);

    // The wrong path, so the reason the right one is pinned stays visible.
    let wrong = prost_types::FieldMask {
        paths: vec!["message".to_owned(), "server_message_data".to_owned()],
    };
    let merged = FieldMaskOperation::update(&api::MESSAGE_DESCRIPTOR, &existing, &patch, wrong)
        .apply()
        .expect("an unknown path is skipped, not refused");
    assert_eq!(
        state_of(&merged.server_message_data),
        Some(ToolRowState::Done)
    );
    assert_eq!(merged.message, existing.message, "the body was not touched");
}

/// **The renderer may not re-tense a headline it did not write.** A row that
/// falls back to the agent's own title carries a sentence Warp has no grammar
/// for; the guard is that demotion only ever prefixes.
#[test]
fn a_demoted_row_is_prefixed_and_never_re_tensed() {
    // Warp's own headline: still legible, and still true.
    assert_eq!(
        super::demoted_headline("Running cargo test"),
        "Interrupted: Running cargo test"
    );
    // The agent's title used whole. The first word ends in "ing" and is not a
    // verb, which is the case the sniffing version mangled.
    assert_eq!(
        super::demoted_headline("Ping the host"),
        "Interrupted: Ping the host"
    );
}
