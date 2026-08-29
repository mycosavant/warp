//! The mapping table, checked without an agent.
//!
//! The fixtures are transcribed from two measured sessions on 2026-08-28 —
//! `claude-agent-acp` and `opencode` 1.18.25 — because the point of
//! `warpctrl acp probe` was to produce this table by running agents rather than
//! by reading the schema.

use agent_client_protocol::schema::v1::{
    AvailableCommandsUpdate, ContentChunk, CurrentModeUpdate, TextContent, ToolCall,
    ToolCallLocation, ToolCallUpdateFields, ToolKind, UsageUpdate,
};

use super::*;

fn translator() -> Translator {
    Translator::new(
        "task-1".to_owned(),
        true,
        "req-1".to_owned(),
        "what is in this directory?".to_owned(),
        DateTime::from_timestamp(1_700_000_000, 0).expect("a valid fixture timestamp"),
    )
}

fn text_chunk(text: &str) -> ContentChunk {
    ContentChunk::new(ContentBlock::Text(TextContent::new(text)))
}

/// Every message body a batch of events carries, in order.
fn bodies(events: &[api::ResponseEvent]) -> Vec<api::message::Message> {
    events
        .iter()
        .filter_map(|event| match &event.r#type {
            Some(api::response_event::Type::ClientActions(actions)) => Some(&actions.actions),
            _ => None,
        })
        .flatten()
        .filter_map(|action| match &action.action {
            Some(api::client_action::Action::AddMessagesToTask(add)) => Some(&add.messages),
            _ => None,
        })
        .flatten()
        .filter_map(|message| message.message.clone())
        .collect()
}

fn output(events: &[api::ResponseEvent]) -> Vec<String> {
    bodies(events)
        .into_iter()
        .filter_map(|body| match body {
            api::message::Message::AgentOutput(output) => Some(output.text),
            _ => None,
        })
        .collect()
}

/// **The hazard, pinned.** A `ToolCall` message is an *instruction*: Warp's
/// action model executes it and returns a result. The agent has already run the
/// tool, so emitting one would run it a second time.
///
/// `local_agent/translate.rs` learned this and wrote it down. T14 then produced
/// three separate instances of a hazard being recorded in prose and built
/// against anyway, so this one gets a test rather than a paragraph.
#[test]
fn a_tool_call_is_never_emitted_as_a_tool_call_message() {
    let mut translator = translator();

    let events = translator.on_update(&SessionUpdate::ToolCall(
        ToolCall::new("call_1", "Write a.txt").kind(ToolKind::Edit),
    ));

    assert!(
        !bodies(&events)
            .iter()
            .any(|body| matches!(body, api::message::Message::ToolCall(_))),
        "a ToolCall message would ask Warp to run a tool the agent already ran"
    );
    assert_eq!(output(&events), vec!["`Write a.txt`".to_owned()]);
}

/// **The defect the first live turn produced.** ACP streams tokens, so one
/// message per chunk rendered `"notes.txt doesn"`, `"'t exist in this"`,
/// `" directory"` as three messages in Warp's panel. Text accumulates and is
/// emitted at a boundary instead.
#[test]
fn token_chunks_are_joined_into_one_message() {
    let mut translator = translator();

    for chunk in ["notes.txt doesn", "'t exist in this", " directory"] {
        assert!(
            translator
                .on_update(&SessionUpdate::AgentMessageChunk(text_chunk(chunk)))
                .is_empty(),
            "a chunk on its own is not a message"
        );
    }
    let events = translator.flush();

    assert_eq!(
        output(&events),
        vec!["notes.txt doesn't exist in this directory".to_owned()]
    );
}

/// A tool call mid-sentence must not be buried inside it, so it is a boundary.
#[test]
fn a_tool_call_flushes_the_text_before_it() {
    let mut translator = translator();
    translator.on_update(&SessionUpdate::AgentMessageChunk(text_chunk("looking now")));

    let events = translator.on_update(&SessionUpdate::ToolCall(
        ToolCall::new("call_1", "read").kind(ToolKind::Read),
    ));

    assert_eq!(
        output(&events),
        vec!["looking now".to_owned(), "`read`".to_owned()],
        "the sentence, then the tool, in that order"
    );
}

/// Answer and reasoning are shown differently, so a run of one ends a run of the
/// other rather than merging into it.
#[test]
fn switching_between_output_and_reasoning_flushes() {
    let mut translator = translator();
    translator.on_update(&SessionUpdate::AgentThoughtChunk(text_chunk("hmm")));

    let events = translator.on_update(&SessionUpdate::AgentMessageChunk(text_chunk("beta")));

    assert!(matches!(
        bodies(&events).as_slice(),
        [api::message::Message::AgentReasoning(_)]
    ));
    assert_eq!(output(&translator.flush()), vec!["beta".to_owned()]);
}

/// Reasoning is rendered as reasoning, not folded into the answer — Warp shows
/// the two differently and merging them would put thinking in the transcript.
#[test]
fn a_thought_chunk_becomes_reasoning_rather_than_output() {
    let mut translator = translator();

    translator.on_update(&SessionUpdate::AgentThoughtChunk(text_chunk(
        "considering the options",
    )));
    let events = translator.flush();

    assert!(matches!(
        bodies(&events).as_slice(),
        [api::message::Message::AgentReasoning(_)]
    ));
    assert!(output(&events).is_empty());
}

/// Empty chunks arrive on both measured streams. Rendering them puts blank
/// messages in the conversation.
#[test]
fn an_empty_chunk_produces_nothing() {
    let mut translator = translator();

    translator.on_update(&SessionUpdate::AgentMessageChunk(text_chunk("   ")));

    assert!(
        translator.flush().is_empty(),
        "whitespace alone is not a message"
    );
}

/// Measured on both agents: `tool_call` carries a placeholder title and a later
/// `tool_call_update` corrects it — Claude sent "Preparing file…" then
/// "Write a.txt", opencode sent "read" then the path. The corrected one is the
/// useful one, and printing both is noise.
#[test]
fn a_corrected_title_is_shown_once_more_and_only_when_it_changed() {
    let mut translator = translator();
    translator.on_update(&SessionUpdate::ToolCall(
        ToolCall::new("call_1", "Preparing file…").kind(ToolKind::Edit),
    ));

    let corrected = translator.on_update(&SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
        "call_1",
        ToolCallUpdateFields::new()
            .title("Write a.txt".to_owned())
            .status(ToolCallStatus::Completed),
    )));
    let repeated = translator.on_update(&SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
        "call_1",
        ToolCallUpdateFields::new()
            .title("Write a.txt".to_owned())
            .status(ToolCallStatus::Completed),
    )));

    assert_eq!(output(&corrected), vec!["`Write a.txt`".to_owned()]);
    assert!(
        repeated.is_empty(),
        "the same title twice is the same fact twice"
    );
}

/// An unfinished update is the common case — the measured streams send several
/// per call — and each one rendered would bury the answer.
#[test]
fn an_in_progress_update_produces_nothing() {
    let mut translator = translator();

    let events = translator.on_update(&SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
        "call_1",
        ToolCallUpdateFields::new()
            .title("read".to_owned())
            .status(ToolCallStatus::InProgress),
    )));

    assert!(events.is_empty());
}

/// **An update that renders nothing must still be recorded**, and this is the
/// pairing that makes the `toolCallId` join work at all.
///
/// Measured on T14.6: a `session/request_permission` arrives with
/// `locations: []`, while the `tool_call_update` carrying the path came moments
/// earlier — *in progress*, not completed. `tool_update_text` returns early for
/// anything that is not `Completed`, so recording from the display path would
/// drop the one update this exists for. Showing and remembering are separate
/// concerns and this test is the seam between them: no events, and the location
/// nonetheless known.
#[test]
fn a_location_is_remembered_from_an_update_that_shows_nothing() {
    let mut translator = translator();

    let events = translator.on_update(&SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
        "call_1",
        ToolCallUpdateFields::new()
            .status(ToolCallStatus::InProgress)
            .locations(vec![ToolCallLocation::new("/tmp/t146/project")]),
    )));

    assert!(
        events.is_empty(),
        "an in-progress update still shows nothing"
    );
    assert_eq!(
        translator.locations_for("call_1"),
        Some(vec!["/tmp/t146/project".to_owned()]),
        "…and the path is nonetheless available to answer 'where does this run'"
    );
}

/// A later update that names no location does not erase one that did.
///
/// Agents send `locations` on the update that has them and omit the field
/// elsewhere, so treating every silent update as "nowhere" would lose the answer
/// in the gap between the tool call and the permission request — which is
/// precisely the interval the join has to survive.
#[test]
fn a_silent_update_does_not_erase_a_location_already_known() {
    let mut translator = translator();

    translator.on_update(&SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
        "call_1",
        ToolCallUpdateFields::new()
            .status(ToolCallStatus::InProgress)
            .locations(vec![ToolCallLocation::new("/tmp/t146/project")]),
    )));
    translator.on_update(&SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
        "call_1",
        ToolCallUpdateFields::new()
            .title("echo hello".to_owned())
            .status(ToolCallStatus::Completed),
    )));

    assert_eq!(
        translator.locations_for("call_1"),
        Some(vec!["/tmp/t146/project".to_owned()])
    );
    assert_eq!(
        translator.locations_for("call_2"),
        None,
        "a call that never named one reports nothing rather than another call's path"
    );
}

/// The variants that are deliberately silent. `CurrentModeUpdate` is the one
/// that matters: T14.3 and T14.4 established that the mode is the agent's claim
/// and does not predict per-call gating, so rendering it in a conversation would
/// be Warp restating a governance fact it cannot check.
#[test]
fn the_updates_warp_deliberately_does_not_render_produce_nothing() {
    let mut translator = translator();

    for update in [
        SessionUpdate::CurrentModeUpdate(CurrentModeUpdate::new("plan")),
        SessionUpdate::AvailableCommandsUpdate(AvailableCommandsUpdate::new(vec![])),
        SessionUpdate::UsageUpdate(UsageUpdate::new(1, 2)),
        SessionUpdate::UserMessageChunk(text_chunk("what is in this directory?")),
    ] {
        assert!(
            translator.on_update(&update).is_empty(),
            "{update:?} should render nothing"
        );
    }
}

/// The stream has to open with the session id, because the client stores it as
/// the conversation token and hands it back — that round-tripping is the whole
/// session store.
#[test]
fn opening_reports_the_session_id_and_announces_the_task_once() {
    let mut translator = translator();

    let first = translator.open("ses_abc".to_owned());
    let second = translator.open("ses_abc".to_owned());

    assert!(matches!(
        first[0].r#type,
        Some(api::response_event::Type::Init(ref init)) if init.conversation_id == "ses_abc"
    ));
    assert_eq!(
        first.iter().filter(|event| creates_a_task(event)).count(),
        1
    );
    assert_eq!(
        second.iter().filter(|event| creates_a_task(event)).count(),
        0,
        "a client told twice about one task rejects the second"
    );
}

/// **The flag that decides how a failure gets reported.** A `StreamFinished`
/// sent before any `StreamInit` is addressed to a stream Warp was never told
/// about, and it was measured to vanish — no message in the panel, none in the
/// log. `drive` reads this to choose between finishing the stream and failing
/// the item; if it ever reports `true` too early, that silence comes back.
#[test]
fn a_stream_is_not_open_until_it_has_been_opened() {
    let mut translator = translator();

    assert!(
        !translator.stream_was_opened(),
        "nothing has been emitted yet, so there is no stream to finish"
    );

    translator.open("ses_abc".to_owned());

    assert!(translator.stream_was_opened());
}

fn creates_a_task(event: &api::ResponseEvent) -> bool {
    match &event.r#type {
        Some(api::response_event::Type::ClientActions(actions)) => {
            actions.actions.iter().any(|action| {
                matches!(
                    action.action,
                    Some(api::client_action::Action::CreateTask(_))
                )
            })
        }
        _ => false,
    }
}

/// The user's question is written into the transcript, or a restored
/// conversation is missing the half a person actually wrote.
#[test]
fn opening_writes_the_users_question_into_the_transcript() {
    let mut translator = translator();

    let events = translator.open("ses_abc".to_owned());

    assert!(bodies(&events).iter().any(|body| matches!(
        body,
        api::message::Message::UserQuery(query) if query.query == "what is in this directory?"
    )));
}

/// A refusal is a finished turn, not a crash — the client synthesizes an
/// "unexpected EOF" for a stream that stops without a `StreamFinished`, which
/// reads as a Warp bug rather than as the agent declining.
#[test]
fn every_stop_reason_finishes_the_stream() {
    let translator = translator();

    for stop in [
        StopReason::EndTurn,
        StopReason::Cancelled,
        StopReason::MaxTokens,
        StopReason::MaxTurnRequests,
        StopReason::Refusal,
    ] {
        assert!(
            matches!(
                translator.finished(stop).r#type,
                Some(api::response_event::Type::Finished(_))
            ),
            "{stop:?} must still finish the stream"
        );
    }
}

/// A refusal and a completion must not read the same. A conversation that simply
/// stops gives a person no way to tell which happened.
#[test]
fn a_refusal_is_distinguishable_from_a_completed_turn() {
    let translator = translator();
    use api::response_event::stream_finished;

    let done = translator.finished(StopReason::EndTurn);
    let refused = translator.finished(StopReason::Refusal);

    assert!(matches!(
        done.r#type,
        Some(api::response_event::Type::Finished(
            api::response_event::StreamFinished {
                reason: Some(stream_finished::Reason::Done(_)),
                ..
            }
        ))
    ));
    assert!(matches!(
        refused.r#type,
        Some(api::response_event::Type::Finished(
            api::response_event::StreamFinished {
                reason: Some(stream_finished::Reason::InternalError(_)),
                ..
            }
        ))
    ));
}

/// Every message needs a distinct id and a timestamp: `convert_conversation`
/// derives a restored exchange's times from them, so an unstamped message is a
/// conversation that happened in 1970.
#[test]
fn messages_are_stamped_and_uniquely_identified() {
    let mut translator = translator();

    let mut events = translator.open("ses_abc".to_owned());
    translator.on_update(&SessionUpdate::AgentMessageChunk(text_chunk("beta")));
    events.extend(translator.flush());

    let ids: Vec<String> = events
        .iter()
        .filter_map(|event| match &event.r#type {
            Some(api::response_event::Type::ClientActions(actions)) => Some(&actions.actions),
            _ => None,
        })
        .flatten()
        .filter_map(|action| match &action.action {
            Some(api::client_action::Action::AddMessagesToTask(add)) => Some(&add.messages),
            _ => None,
        })
        .flatten()
        .map(|message| {
            assert!(
                message.timestamp.is_some(),
                "an unstamped message dates to 1970"
            );
            message.id.clone()
        })
        .collect();

    let mut unique = ids.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(ids.len(), unique.len(), "message ids collided: {ids:?}");
}

/// A long prompt names the conversation in the history panel, and a prompt that
/// ends mid-glyph must not panic on the byte boundary.
#[test]
fn a_task_description_is_truncated_on_a_character_boundary() {
    let long = "é".repeat(200);

    let description = task_description(&long);

    assert!(description.ends_with('…'));
    assert_eq!(description.chars().count(), TASK_DESCRIPTION_CHARS + 1);
}
