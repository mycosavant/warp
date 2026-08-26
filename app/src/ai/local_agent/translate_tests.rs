//! Every fixture line here is a real line, copied from
//! `claude --print --output-format stream-json --verbose`, trimmed to the
//! fields this fork reads. Inventing the shape would have tested the reading of
//! my own guess.

use chrono::{DateTime, Utc};
use warp_multi_agent_api as api;

use super::*;

/// A fixed instant, so the timestamp assertions are about the code rather than
/// about when the suite ran.
fn started_at() -> DateTime<Utc> {
    DateTime::from_timestamp(1_787_186_964, 0).expect("a valid instant")
}

fn translator() -> Translator {
    Translator::new(
        "task-1".to_owned(),
        true,
        "req-1".to_owned(),
        Mode::Query {
            prompt: "what is the capital of France?".to_owned(),
        },
        started_at(),
    )
}

fn continuing_translator() -> Translator {
    Translator::new(
        "task-1".to_owned(),
        false,
        "req-1".to_owned(),
        Mode::Query {
            prompt: "what is the capital of France?".to_owned(),
        },
        started_at(),
    )
}

const INIT: &str = r#"{"type":"system","subtype":"init","cwd":"/tmp","session_id":"43ce5cd9-e7ff-4b39-afc4-6e828a726e3b","model":"claude-haiku-4-5","permissionMode":"default","apiKeySource":"none"}"#;

fn assistant(content: &str) -> String {
    format!(
        r#"{{"type":"assistant","message":{{"model":"claude-haiku-4-5","role":"assistant","content":[{content}]}},"session_id":"43ce5cd9"}}"#
    )
}

fn client_actions(event: &api::ResponseEvent) -> &[api::ClientAction] {
    match &event.r#type {
        Some(api::response_event::Type::ClientActions(actions)) => &actions.actions,
        other => panic!("expected client actions, got {other:?}"),
    }
}

fn messages(event: &api::ResponseEvent) -> &[api::Message] {
    match &client_actions(event)[0].action {
        Some(api::client_action::Action::AddMessagesToTask(add)) => &add.messages,
        other => panic!("expected AddMessagesToTask, got {other:?}"),
    }
}

#[test]
fn claudes_session_id_becomes_the_conversation_token() {
    // The whole session store. Warp keeps `StreamInit.conversation_id` as the
    // conversation's server token and hands it back next turn, which is what
    // `--resume` is given.
    let events = translator().on_line(INIT);

    let Some(api::response_event::Type::Init(init)) = &events[0].r#type else {
        panic!("expected StreamInit, got {:?}", events[0].r#type);
    };
    assert_eq!(init.conversation_id, "43ce5cd9-e7ff-4b39-afc4-6e828a726e3b");
    assert_eq!(init.request_id, "req-1");
}

#[test]
fn a_new_conversation_is_told_about_its_task() {
    let events = translator().on_line(INIT);

    assert_eq!(
        events.len(),
        3,
        "expected StreamInit, CreateTask, then the user's own turn"
    );
    let Some(api::client_action::Action::CreateTask(create)) =
        &client_actions(&events[1])[0].action
    else {
        panic!("expected CreateTask");
    };
    let task = create.task.as_ref().unwrap();
    assert_eq!(task.id, "task-1");
    assert!(
        task.messages.is_empty(),
        "messages arrive via AddMessagesToTask; including them here would double them"
    );
    assert_eq!(
        task.description, "what is the capital of France?",
        "`AIConversation::title` reads the description first, and an empty one \
         is a conversation the history panel calls \"Untitled\""
    );
}

#[test]
fn a_conversation_that_already_has_a_task_is_not_told_again() {
    let events = continuing_translator().on_line(INIT);

    assert_eq!(
        events.len(),
        2,
        "StreamInit and the user's turn, but no CreateTask: {events:?}"
    );
}

#[test]
fn the_task_is_announced_once_even_if_claude_reinitializes() {
    let mut translator = translator();
    let first = translator.on_line(INIT);
    let second = translator.on_line(INIT);

    assert_eq!(first.len(), 3);
    assert_eq!(second.len(), 2, "a second CreateTask would be rejected");
}

#[test]
fn assistant_text_becomes_agent_output() {
    let mut translator = continuing_translator();
    let events = translator.on_line(&assistant(r#"{"type":"text","text":"pong"}"#));

    let messages = messages(&events[0]);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].task_id, "task-1");
    let Some(api::message::Message::AgentOutput(output)) = &messages[0].message else {
        panic!("expected AgentOutput, got {:?}", messages[0].message);
    };
    assert_eq!(output.text, "pong");
}

#[test]
fn thinking_becomes_reasoning_not_output() {
    let mut translator = continuing_translator();
    let events = translator.on_line(&assistant(
        r#"{"type":"thinking","thinking":"They want the word pong.","signature":"Et4DCq8B"}"#,
    ));

    let Some(api::message::Message::AgentReasoning(reasoning)) = &messages(&events[0])[0].message
    else {
        panic!("expected AgentReasoning");
    };
    assert_eq!(reasoning.reasoning, "They want the word pong.");
}

#[test]
fn a_tool_claude_ran_is_reported_but_never_requested() {
    // The dangerous mistake this fork could make. A `ToolCall` message is an
    // instruction: Warp's action model executes it. Claude has already run the
    // tool, so emitting one would run it twice — a second `rm`, a second push.
    let mut translator = continuing_translator();
    let events = translator.on_line(&assistant(
        r#"{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"rm -rf build"}}"#,
    ));

    let message = &messages(&events[0])[0];
    assert!(
        matches!(message.message, Some(api::message::Message::AgentOutput(_))),
        "tool activity must be reported as text, got {:?}",
        message.message
    );
    assert!(
        !matches!(message.message, Some(api::message::Message::ToolCall(_))),
        "a ToolCall would ask Warp to run `rm -rf build` a second time"
    );
}

#[test]
fn several_content_blocks_arrive_as_several_messages_with_distinct_ids() {
    let mut translator = continuing_translator();
    let events = translator.on_line(&assistant(
        r#"{"type":"thinking","thinking":"first"},{"type":"text","text":"second"}"#,
    ));

    let messages = messages(&events[0]);
    assert_eq!(messages.len(), 2);
    assert_ne!(
        messages[0].id, messages[1].id,
        "ids collide and the client would treat the second as an edit of the first"
    );
}

#[test]
fn empty_text_is_dropped_rather_than_shown_as_a_blank_message() {
    let mut translator = continuing_translator();
    let events = translator.on_line(&assistant(r#"{"type":"text","text":"   "}"#));

    assert!(events.is_empty(), "{events:?}");
}

#[test]
fn a_successful_result_finishes_the_stream() {
    let mut translator = continuing_translator();
    assert!(!translator.saw_result());

    let events = translator.on_line(
        r#"{"type":"result","subtype":"success","is_error":false,"result":"pong","usage":{"input_tokens":10,"output_tokens":64,"cache_read_input_tokens":16591,"cache_creation_input_tokens":7833}}"#,
    );

    assert!(translator.saw_result());
    let Some(api::response_event::Type::Finished(finished)) = &events[0].r#type else {
        panic!("expected StreamFinished");
    };
    assert!(matches!(
        finished.reason,
        Some(api::response_event::stream_finished::Reason::Done(_))
    ));
    let usage = &finished.token_usage[0];
    assert_eq!(usage.output, 64);
    assert_eq!(usage.total_input, 10 + 16591 + 7833);
    assert_eq!(
        usage.cost_in_cents, 0.0,
        "this field is what Warp charged, and Warp charged nothing"
    );
}

#[test]
fn a_failed_result_finishes_the_stream_with_claudes_own_words() {
    let mut translator = continuing_translator();
    let events = translator.on_line(
        r#"{"type":"result","subtype":"error_during_execution","is_error":true,"result":"Credit balance is too low"}"#,
    );

    let Some(api::response_event::Type::Finished(finished)) = &events[0].r#type else {
        panic!("expected StreamFinished");
    };
    let Some(api::response_event::stream_finished::Reason::InternalError(error)) = &finished.reason
    else {
        panic!("expected InternalError, got {:?}", finished.reason);
    };
    assert_eq!(error.message, "Credit balance is too low");
    assert!(
        translator.saw_result(),
        "a reported failure is still a finished turn; treating it as a dead \
         stream would retry it three times"
    );
}

#[test]
fn the_lines_warp_has_no_use_for_are_ignored_rather_than_fatal() {
    // Claude's stream carries plenty Warp cannot render, and it is versioned
    // independently of this fork. Any of these taking the turn down would make
    // a Claude Code update break the agent.
    let mut translator = continuing_translator();

    for line in [
        r#"{"type":"system","subtype":"thinking_tokens","estimated_tokens":43}"#,
        r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed"}}"#,
        r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"ok"}]}}"#,
        r#"{"type":"something_invented_next_release"}"#,
        r#"{"type":"assistant","message":{"content":[{"type":"redacted_thinking","data":"x"}]}}"#,
        "not json at all",
        "",
        "   ",
    ] {
        assert!(
            translator.on_line(line).is_empty(),
            "unexpectedly translated: {line}"
        );
    }

    assert!(!translator.saw_result(), "none of those finished the turn");
}

#[test]
fn the_users_own_turn_is_written_into_the_transcript() {
    // Upstream the *server* echoes the query back as a message, and the whole
    // of a restored conversation's user side comes from it. Live it draws
    // nothing — `convert_from` maps `UserQuery` to `NoClientRepresentation` —
    // so this cannot double up the prompt the input already showed.
    let events = translator().on_line(INIT);

    let messages = messages(&events[2]);
    assert_eq!(messages.len(), 1);
    let Some(api::message::Message::UserQuery(query)) = &messages[0].message else {
        panic!("expected UserQuery, got {:?}", messages[0].message);
    };
    assert_eq!(query.query, "what is the capital of France?");
}

#[test]
fn the_users_turn_carries_the_time_the_exchange_started() {
    // `convert_conversation` looks here first for a restored exchange's
    // `start_time`, and its last resort is `unwrap_or_default()` — the Unix
    // epoch. That is what showed in the history panel as "58 years ago".
    let events = translator().on_line(INIT);

    let Some(api::message::Message::UserQuery(query)) = &messages(&events[2])[0].message else {
        panic!("expected UserQuery");
    };
    let current_time = query
        .context
        .as_ref()
        .and_then(|context| context.current_time.as_ref())
        .expect("the user query must carry the time the turn started");
    assert_eq!(current_time.seconds, started_at().timestamp());
}

#[test]
fn every_message_is_stamped_with_the_time_of_the_turn() {
    // A restored exchange takes its `finish_time` from these. Unstamped, a
    // conversation that happened today is filed under 1970 and sorts to the
    // bottom of a history panel ordered by `last_updated`.
    let mut translator = continuing_translator();
    let mut events = translator.on_line(INIT);
    events.extend(translator.on_line(&assistant(r#"{"type":"text","text":"Paris."}"#)));

    let stamped: Vec<i64> = events
        .iter()
        .filter(|event| {
            matches!(
                event.r#type,
                Some(api::response_event::Type::ClientActions(_))
            )
        })
        .flat_map(|event| messages(event).to_vec())
        .map(|message| {
            message
                .timestamp
                .unwrap_or_else(|| panic!("unstamped message {}", message.id))
                .seconds
        })
        .collect();

    assert_eq!(stamped.len(), 2, "the user's turn and the agent's reply");
    assert!(
        stamped
            .iter()
            .all(|seconds| *seconds == started_at().timestamp()),
        "one turn is one exchange, so its messages share one time: {stamped:?}"
    );
}

#[test]
fn a_long_prompt_is_cut_to_a_readable_title_without_splitting_a_glyph() {
    // The cut is by character, not by byte: `String::truncate` on a byte index
    // inside a multi-byte glyph panics, and a pasted prompt is exactly where
    // one turns up.
    let prompt = "🌍".repeat(200);
    let mut translator = Translator::new(
        "task-1".to_owned(),
        true,
        "req-1".to_owned(),
        Mode::Query { prompt },
        started_at(),
    );
    let events = translator.on_line(INIT);

    let Some(api::client_action::Action::CreateTask(create)) =
        &client_actions(&events[1])[0].action
    else {
        panic!("expected CreateTask");
    };
    let description = &create.task.as_ref().unwrap().description;
    assert!(description.ends_with('…'), "{description}");
    assert_eq!(
        description.chars().count(),
        61,
        "60 glyphs plus the ellipsis"
    );
}

// A `/compact`. Every line below is verbatim from
// `claude --print --output-format stream-json --verbose --resume <id>` with the
// summary body cut short — the shape is Claude's, not a guess at Claude's.

fn compactor() -> Translator {
    Translator::new(
        "task-1".to_owned(),
        false,
        "req-1".to_owned(),
        Mode::Compact {
            session: "30461b56-4238-4d93-9acd-443eae43e5a1".to_owned(),
            instructions: None,
        },
        started_at(),
    )
}

const COMPACTING: &str = r#"{"type":"system","subtype":"status","status":"compacting","session_id":"30461b56-4238-4d93-9acd-443eae43e5a1"}"#;
const COMPACTED: &str = r#"{"type":"system","subtype":"status","status":null,"compact_result":"success","session_id":"30461b56-4238-4d93-9acd-443eae43e5a1"}"#;
const COMPACT_INIT: &str = r#"{"type":"system","subtype":"init","cwd":"/tmp","session_id":"30461b56-4238-4d93-9acd-443eae43e5a1","model":"claude-opus-5"}"#;
const BOUNDARY: &str = r#"{"type":"system","subtype":"compact_boundary","session_id":"30461b56-4238-4d93-9acd-443eae43e5a1","compact_metadata":{"trigger":"manual","pre_tokens":22988,"post_tokens":2156,"cumulative_dropped_tokens":62561,"duration_ms":18962}}"#;
const SUMMARY: &str = r#"{"type":"user","message":{"role":"user","content":"This session is being continued from a previous conversation that ran out of context. The summary below covers the earlier portion of the conversation.\n\nSummary:\n1. Primary Request and Intent:\n   The user asked for the codewords back."},"session_id":"30461b56-4238-4d93-9acd-443eae43e5a1"}"#;
const ECHO: &str = r#"{"type":"user","message":{"role":"user","content":"<local-command-stdout>Compacted </local-command-stdout>"},"session_id":"30461b56-4238-4d93-9acd-443eae43e5a1"}"#;
const COMPACT_RESULT: &str = r#"{"type":"result","subtype":"success","is_error":false,"result":"","session_id":"30461b56-4238-4d93-9acd-443eae43e5a1","usage":{"input_tokens":0,"output_tokens":0}}"#;

fn summarizations(events: &[api::ResponseEvent]) -> Vec<&api::message::Summarization> {
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
        .filter_map(|message| match &message.message {
            Some(api::message::Message::Summarization(summarization)) => Some(summarization),
            _ => None,
        })
        .collect()
}

fn conversation_summary(
    summarization: &api::message::Summarization,
) -> &api::message::summarization::ConversationSummary {
    match &summarization.summary_type {
        Some(api::message::summarization::SummaryType::ConversationSummary(summary)) => summary,
        other => panic!("expected a conversation summary, got {other:?}"),
    }
}

/// A whole compaction, in the order Claude sends it.
#[test]
fn a_compaction_becomes_one_summarization_message() {
    let mut translator = compactor();
    let mut events = Vec::new();
    for line in [
        COMPACTING,
        COMPACTED,
        COMPACT_INIT,
        BOUNDARY,
        SUMMARY,
        ECHO,
        COMPACT_RESULT,
    ] {
        events.extend(translator.on_line(line));
    }

    let summaries = summarizations(&events);
    assert_eq!(
        summaries.len(),
        1,
        "the summary is one message; the CLI's `Compacted` echo is not a second one"
    );
    let summary = conversation_summary(summaries[0]);
    assert!(
        summary.summary.starts_with("1. Primary Request"),
        "the preamble addressed to the next model should be gone: {:?}",
        summary.summary
    );
    assert_eq!(
        summary.token_count, 2156,
        "the post-compaction context size is what is left"
    );
    assert_eq!(
        summaries[0].finished_duration,
        Some(prost_types::Duration {
            seconds: 18,
            nanos: 962_000_000
        }),
        "18962ms, kept to the millisecond"
    );
}

/// The stream opens once, on the first line, whatever Claude does next.
///
/// A compaction's own `system/init` arrives in the middle — after the work is
/// done, for the session it has just rewritten. Relaying that as the client's
/// `StreamInit` would put the opening event two thirds of the way through the
/// stream, and relaying *both* would hand the client a second conversation
/// token mid-turn.
#[test]
fn a_compaction_opens_its_stream_once_and_at_the_start() {
    let mut translator = compactor();
    let opening = translator.on_line(COMPACTING);

    let Some(api::response_event::Type::Init(init)) = &opening[0].r#type else {
        panic!("expected StreamInit, got {:?}", opening[0]);
    };
    assert_eq!(
        init.conversation_id, "30461b56-4238-4d93-9acd-443eae43e5a1",
        "compaction leaves the session id alone, so the token must not move"
    );

    let later: Vec<_> = [COMPACTED, COMPACT_INIT, BOUNDARY, SUMMARY]
        .into_iter()
        .flat_map(|line| translator.on_line(line))
        .filter(|event| matches!(event.r#type, Some(api::response_event::Type::Init(_))))
        .collect();
    assert!(later.is_empty(), "a second StreamInit: {later:?}");
}

/// The request is recorded, so a restored conversation is not a summary that
/// nobody asked for.
#[test]
fn a_compaction_records_what_was_asked() {
    let mut translator = Translator::new(
        "task-1".to_owned(),
        false,
        "req-1".to_owned(),
        Mode::Compact {
            session: "30461b56-4238-4d93-9acd-443eae43e5a1".to_owned(),
            instructions: Some("keep only the codewords".to_owned()),
        },
        started_at(),
    );
    let opening = translator.on_line(COMPACTING);

    let Some(api::message::Message::SystemQuery(query)) = &messages(&opening[1])[0].message else {
        panic!(
            "expected a system query, got {:?}",
            messages(&opening[1])[0]
        );
    };
    let Some(api::message::system_query::Type::SummarizeConversation(summarize)) = &query.r#type
    else {
        panic!("expected SummarizeConversation, got {:?}", query.r#type);
    };
    assert_eq!(summarize.prompt, "keep only the codewords");
    assert!(
        summarizations(&opening).is_empty(),
        "nothing has been summarized yet"
    );
}

/// A refusal is an answer, not an error.
///
/// Claude declines to compact a conversation that has barely started, and says
/// so through a synthetic assistant message. There is no `compact_boundary`,
/// no summary, and — the part that matters — no `system/init` either, so a
/// stream that waited for one would end without ever having opened and be
/// reported to the user as a dropped connection.
#[test]
fn a_refused_compaction_still_opens_and_still_finishes() {
    const REFUSED: &str = r#"{"type":"system","subtype":"status","status":null,"compact_result":"failed","compact_error":"Not enough messages to compact.","session_id":"30461b56-4238-4d93-9acd-443eae43e5a1"}"#;
    const EXPLANATION: &str = r#"{"type":"assistant","message":{"model":"<synthetic>","role":"assistant","content":[{"type":"text","text":"Not enough messages to compact."}]},"session_id":"30461b56-4238-4d93-9acd-443eae43e5a1"}"#;

    let mut translator = compactor();
    let mut events = Vec::new();
    for line in [COMPACTING, REFUSED, EXPLANATION, COMPACT_RESULT] {
        events.extend(translator.on_line(line));
    }

    assert!(
        matches!(
            events.first().and_then(|event| event.r#type.as_ref()),
            Some(api::response_event::Type::Init(_))
        ),
        "first event was {:?}",
        events.first()
    );
    assert!(translator.saw_result(), "the turn ended, and said so");
    assert!(summarizations(&events).is_empty(), "nothing was summarized");

    let explanation: Vec<_> = events
        .iter()
        .filter(|event| {
            matches!(
                event.r#type,
                Some(api::response_event::Type::ClientActions(_))
            )
        })
        .flat_map(|event| messages(event))
        .filter_map(|message| match &message.message {
            Some(api::message::Message::AgentOutput(output)) => Some(output.text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        explanation,
        vec!["Not enough messages to compact."],
        "the reason belongs on screen"
    );
}

/// The summary is found by position, not by prose.
///
/// On disk Claude flags it `isCompactSummary`; on the stream that field is
/// gone, so the only handle is that it is the first user message after the
/// boundary. A summary arriving before one would be some other user message
/// entirely.
#[test]
fn a_user_message_outside_the_boundary_is_not_a_summary() {
    let mut translator = compactor();
    let events: Vec<_> = [COMPACTING, SUMMARY, ECHO]
        .into_iter()
        .flat_map(|line| translator.on_line(line))
        .collect();

    assert!(
        summarizations(&events).is_empty(),
        "a user message with no compaction behind it summarizes nothing"
    );
}

/// The preamble is dropped only when it is really there.
#[test]
fn only_a_recognized_preamble_is_stripped() {
    assert_eq!(
        readable_summary(
            "This session is being continued from a previous conversation that ran out of \
             context.\n\nSummary:\nThe body."
        ),
        "The body."
    );
    // Reworded upstream: the marker is gone, so nothing is cut.
    assert_eq!(
        readable_summary("This session is being continued from a previous conversation. The body."),
        "This session is being continued from a previous conversation. The body."
    );
    // A summary that simply does not have one.
    assert_eq!(readable_summary("  The body.  "), "The body.");
    // And a `Summary:` that is part of the summary rather than the preamble.
    assert_eq!(
        readable_summary("What we did.\nSummary:\nNot a preamble."),
        "What we did.\nSummary:\nNot a preamble."
    );
}

// ---- tool events for the log (T11.1c) ---------------------------------------
//
// Every fixture below is a real line from
// `claude --print --output-format stream-json --verbose`, captured 2026-08-25
// and trimmed to the fields this fork reads. Two of them exist because the
// capture contradicted what would otherwise have been written from memory:
// `is_error` is *absent* on some successful results and `false` on others from
// the same binary, and a `tool_use` block carries a `caller` object that no
// remembered version of this shape had.

const TOOL_USE: &str = r#"{"type":"tool_use","id":"toolu_01QQ7Nn3ZtF8tjGwhmWf7K1Y","name":"Read","input":{"file_path":"/tmp/t111c/sample.txt"},"caller":{"type":"direct"}}"#;

fn tool_result(body: &str) -> String {
    format!(
        r#"{{"type":"user","message":{{"role":"user","content":[{body}]}},"parent_tool_use_id":null,"session_id":"eb60ccee"}}"#
    )
}

/// The join the log is for: both halves of one call carry Claude's
/// `tool_use.id`, so a `tool_start` with no `tool_complete` is a call that hung.
#[test]
fn a_tool_call_and_its_result_are_recorded_as_a_matched_pair() {
    let mut translator = continuing_translator();

    translator.on_line(&assistant(TOOL_USE));
    assert_eq!(
        translator.take_tool_events(),
        vec![ToolEvent::Started {
            call_id: "toolu_01QQ7Nn3ZtF8tjGwhmWf7K1Y".to_owned(),
            parent_call_id: None,
            name: "Read".to_owned(),
            input_preview: Some("/tmp/t111c/sample.txt".to_owned()),
        }]
    );

    let events = translator.on_line(&tool_result(
        r#"{"tool_use_id":"toolu_01QQ7Nn3ZtF8tjGwhmWf7K1Y","type":"tool_result","content":"1\thello\n"}"#,
    ));
    assert!(
        events.is_empty(),
        "a tool result still renders nothing: Claude has already said in prose what it meant"
    );
    assert_eq!(
        translator.take_tool_events(),
        vec![ToolEvent::Completed {
            call_id: "toolu_01QQ7Nn3ZtF8tjGwhmWf7K1Y".to_owned(),
            parent_call_id: None,
            name: Some("Read".to_owned()),
            failed: false,
        }],
        "the completion carries the name its `tool_use` had"
    );
}

/// Absent `is_error` and `"is_error":false` were both observed from the same
/// CLI build, so defaulting is required rather than defensive.
#[test]
fn a_missing_is_error_is_a_success() {
    let mut translator = continuing_translator();
    translator.on_line(&tool_result(
        r#"{"tool_use_id":"toolu_1","type":"tool_result","content":"ok"}"#,
    ));

    assert_eq!(
        translator.take_tool_events(),
        vec![ToolEvent::Completed {
            call_id: "toolu_1".to_owned(),
            parent_call_id: None,
            name: None,
            failed: false,
        }]
    );
}

#[test]
fn a_failed_tool_result_is_recorded_as_failed() {
    let mut translator = continuing_translator();
    translator.on_line(&tool_result(
        r#"{"type":"tool_result","content":"File does not exist.","is_error":true,"tool_use_id":"toolu_01WNT6kU9hUQ9WWPUoS3wZjG"}"#,
    ));

    assert_eq!(
        translator.take_tool_events(),
        vec![ToolEvent::Completed {
            call_id: "toolu_01WNT6kU9hUQ9WWPUoS3wZjG".to_owned(),
            parent_call_id: None,
            name: None,
            failed: true,
        }]
    );
}

/// The preview answers "what ran", the same question Warp's own agent's does,
/// so it is those two keys and not the whole input object — which is also where
/// a tool's secrets are.
#[test]
fn the_preview_is_the_command_or_the_file_path_and_nothing_else() {
    let mut translator = continuing_translator();
    translator.on_line(&assistant(
        r#"{"type":"tool_use","id":"a","name":"Bash","input":{"command":"rm -rf build","description":"Clean"}},
           {"type":"tool_use","id":"b","name":"Grep","input":{"pattern":"secret","path":"/etc"}}"#,
    ));

    let previews: Vec<Option<String>> = translator
        .take_tool_events()
        .into_iter()
        .map(|event| match event {
            ToolEvent::Started { input_preview, .. } => input_preview,
            other => panic!("expected two starts, got {other:?}"),
        })
        .collect();
    assert_eq!(
        previews,
        vec![Some("rm -rf build".to_owned()), None],
        "`description` and `pattern` are not what was run"
    );
}

/// `input` is whatever a tool's schema says, and an MCP server can declare
/// `command` as any shape at all. A typed field here would fail the whole
/// `ClaudeEvent`, and `on_line` drops a line it cannot parse — so an
/// unrecognized input would have cost the **answer**, not just the preview.
#[test]
fn an_input_of_an_unexpected_shape_costs_the_preview_and_not_the_message() {
    let mut translator = continuing_translator();
    let events = translator.on_line(&assistant(
        r#"{"type":"tool_use","id":"a","name":"mcp__thing__run","input":{"command":["sh","-c","ls"]}}"#,
    ));

    assert_eq!(
        messages(&events[0]).len(),
        1,
        "the tool is still reported to the conversation"
    );
    assert_eq!(
        translator.take_tool_events(),
        vec![ToolEvent::Started {
            call_id: "a".to_owned(),
            parent_call_id: None,
            name: "mcp__thing__run".to_owned(),
            input_preview: None,
        }]
    );
}

/// Drained, not accumulated: the caller writes each batch as it arrives, and a
/// second read must not write the same lines again.
#[test]
fn taking_the_tool_events_empties_them() {
    let mut translator = continuing_translator();
    translator.on_line(&assistant(TOOL_USE));

    assert_eq!(translator.take_tool_events().len(), 1);
    assert!(translator.take_tool_events().is_empty());
}

/// A compaction summary's content is a bare string rather than blocks. It
/// carries no tool results, and asking it for some must not be an error.
#[test]
fn a_summary_is_not_mistaken_for_a_tool_result() {
    let mut compactor = compactor();
    compactor.on_line(BOUNDARY);
    compactor.on_line(SUMMARY);

    assert!(compactor.take_tool_events().is_empty());
}

/// Captured 2026-08-25 from a real `Task` turn. The nested `Read` names the
/// `Agent` call that spawned it; the `Agent` call itself names nothing.
///
/// Found by running the rest of T11.1c, not planned: without this the only
/// evidence of containment is that a subagent's tools fall *between* the
/// parent's start and completion, and that stops being evidence the moment two
/// subagents run at once.
#[test]
fn a_subagents_tools_name_the_call_that_spawned_them() {
    let mut translator = continuing_translator();

    translator.on_line(
        r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_outer","name":"Agent","input":{}}]},"parent_tool_use_id":null,"session_id":"s"}"#,
    );
    translator.on_line(
        r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_inner","name":"Read","input":{"file_path":"/tmp/x"}}]},"parent_tool_use_id":"toolu_outer","session_id":"s"}"#,
    );
    translator.on_line(
        r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_inner","content":"ok"}]},"parent_tool_use_id":"toolu_outer","session_id":"s"}"#,
    );

    let parents: Vec<(String, Option<String>)> = translator
        .take_tool_events()
        .into_iter()
        .map(|event| match event {
            ToolEvent::Started {
                call_id,
                parent_call_id,
                ..
            }
            | ToolEvent::Completed {
                call_id,
                parent_call_id,
                ..
            } => (call_id, parent_call_id),
        })
        .collect();

    assert_eq!(
        parents,
        vec![
            ("toolu_outer".to_owned(), None),
            ("toolu_inner".to_owned(), Some("toolu_outer".to_owned())),
            ("toolu_inner".to_owned(), Some("toolu_outer".to_owned())),
        ]
    );
}
