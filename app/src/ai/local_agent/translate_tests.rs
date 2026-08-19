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
        "what is the capital of France?".to_owned(),
        started_at(),
    )
}

fn continuing_translator() -> Translator {
    Translator::new(
        "task-1".to_owned(),
        false,
        "req-1".to_owned(),
        "what is the capital of France?".to_owned(),
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
        prompt,
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
