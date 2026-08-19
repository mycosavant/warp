//! Claude Code's `stream-json` events, said in Warp's vocabulary.
//!
//! Pure functions over strings: one JSON line in, zero or more
//! [`api::ResponseEvent`] out. No process, no clock, no network — which is what
//! makes the interesting half of this feature testable without either end.

use chrono::{DateTime, Utc};
use serde::Deserialize;
use warp_multi_agent_api as api;

/// How much of the prompt becomes the conversation's name in the history panel.
///
/// Upstream the server writes a summarised title; there is no summariser here,
/// so the prompt itself is the honest stand-in. Long enough to tell two
/// conversations apart, short enough for the panel.
const TASK_DESCRIPTION_CHARS: usize = 60;

fn task_description(prompt: &str) -> String {
    let prompt = prompt.trim();
    // `char_indices` rather than slicing by byte: a prompt can end mid-glyph
    // and `String::truncate` would panic on the boundary.
    match prompt.char_indices().nth(TASK_DESCRIPTION_CHARS) {
        Some((cut, _)) => format!("{}…", prompt[..cut].trim_end()),
        None => prompt.to_owned(),
    }
}

/// A line of Claude's `--output-format stream-json`.
///
/// Only the variants Warp can render are named. `Ignored` is load-bearing:
/// Claude emits `system/thinking_tokens`, `rate_limit_event` and others that
/// arrive on the same stream and would otherwise fail the whole turn.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ClaudeEvent {
    #[serde(rename = "system")]
    System(SystemEvent),
    #[serde(rename = "assistant")]
    Assistant { message: AssistantMessage },
    #[serde(rename = "result")]
    Result(ResultEvent),
    #[serde(other)]
    Ignored,
}

#[derive(Debug, Deserialize)]
struct SystemEvent {
    #[serde(default)]
    subtype: String,
    #[serde(default)]
    session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AssistantMessage {
    #[serde(default)]
    content: Vec<ContentBlock>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "thinking")]
    Thinking { thinking: String },
    #[serde(rename = "tool_use")]
    ToolUse { name: String },
    #[serde(other)]
    Ignored,
}

#[derive(Debug, Deserialize)]
struct ResultEvent {
    #[serde(default)]
    is_error: bool,
    #[serde(default)]
    result: Option<String>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Debug, Default, Deserialize)]
struct Usage {
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
    #[serde(default)]
    cache_read_input_tokens: u32,
    #[serde(default)]
    cache_creation_input_tokens: u32,
}

/// Turns one Claude turn into one Warp response stream.
///
/// Holds only what the protocol needs carried between lines: which task the
/// messages belong to, whether that task has been announced yet, and a counter
/// so every message gets a distinct id.
pub(super) struct Translator {
    task_id: String,
    /// `true` until the [`CreateTask`](api::client_action::CreateTask) that
    /// tells the client this task exists has been emitted. A conversation that
    /// already has tasks skips it — the client would reject a task it already
    /// holds.
    task_needs_announcing: bool,
    request_id: String,
    /// The prompt this turn was asked, recorded into the transcript as a
    /// `UserQuery` message. See [`Self::user_query`].
    prompt: String,
    next_message: u64,
    saw_result: bool,
    /// When the turn started, stamped onto every message.
    started_at: DateTime<Utc>,
}

impl Translator {
    pub(super) fn new(
        task_id: String,
        task_needs_announcing: bool,
        request_id: String,
        prompt: String,
        started_at: DateTime<Utc>,
    ) -> Self {
        Self {
            task_id,
            task_needs_announcing,
            request_id,
            prompt,
            next_message: 0,
            saw_result: false,
            started_at,
        }
    }

    /// Whether Claude reported the turn finished.
    ///
    /// The client synthesizes `UnexpectedEof` when a stream ends without a
    /// `StreamFinished`, so the caller needs to know whether the stream ended
    /// because the turn ended or because the process died.
    pub(super) fn saw_result(&self) -> bool {
        self.saw_result
    }

    /// Translates one line. Unparseable lines are dropped rather than failing
    /// the turn: Claude's stream is versioned independently of this fork, and a
    /// field added upstream should not take the conversation down with it.
    pub(super) fn on_line(&mut self, line: &str) -> Vec<api::ResponseEvent> {
        let line = line.trim();
        if line.is_empty() {
            return Vec::new();
        }
        let Ok(event) = serde_json::from_str::<ClaudeEvent>(line) else {
            log::debug!("local agent: unrecognized claude stream line");
            return Vec::new();
        };

        match event {
            ClaudeEvent::System(system) if system.subtype == "init" => {
                // Claude's session id becomes Warp's conversation token, and
                // Warp hands it back on the next request. Its own
                // round-tripping is the whole session store — nothing here
                // keeps state between turns.
                //
                // Read from the event rather than reused from the spawn
                // arguments on purpose: if `--resume` misses, Claude starts a
                // fresh session and says so, and the token must follow the
                // session that actually exists.
                let session_id = system.session_id.unwrap_or_default();
                let mut events = vec![api::ResponseEvent {
                    r#type: Some(api::response_event::Type::Init(
                        api::response_event::StreamInit {
                            conversation_id: session_id,
                            request_id: self.request_id.clone(),
                            run_id: String::new(),
                        },
                    )),
                }];
                if self.task_needs_announcing {
                    self.task_needs_announcing = false;
                    events.push(actions(vec![api::client_action::Action::CreateTask(
                        api::client_action::CreateTask {
                            task: Some(api::Task {
                                id: self.task_id.clone(),
                                // The task's name. `AIConversation::title`
                                // reads it first and only falls back to the
                                // initial query, so a task with no description
                                // is a conversation called "Untitled" in the
                                // history panel.
                                description: task_description(&self.prompt),
                                ..Default::default()
                            }),
                        },
                    )]));
                }
                events.push(actions(vec![
                    api::client_action::Action::AddMessagesToTask(
                        api::client_action::AddMessagesToTask {
                            task_id: self.task_id.clone(),
                            messages: vec![self.user_query()],
                        },
                    ),
                ]));
                events
            }
            ClaudeEvent::Assistant { message } => {
                let messages: Vec<api::Message> = message
                    .content
                    .into_iter()
                    .filter_map(|block| self.content_block(block))
                    .collect();
                if messages.is_empty() {
                    return Vec::new();
                }
                vec![actions(vec![
                    api::client_action::Action::AddMessagesToTask(
                        api::client_action::AddMessagesToTask {
                            task_id: self.task_id.clone(),
                            messages,
                        },
                    ),
                ])]
            }
            ClaudeEvent::Result(result) => {
                self.saw_result = true;
                vec![self.finished(result)]
            }
            ClaudeEvent::System(_) | ClaudeEvent::Ignored => Vec::new(),
        }
    }

    /// The user's own turn, written into the transcript.
    ///
    /// Upstream the *server* echoes the query back as a message, and a great
    /// deal hangs off that rather than off the input the client already holds.
    /// Live it is deliberately inert — `convert_from` maps it to
    /// `NoClientRepresentation`, so it does not double-render the prompt the
    /// input already drew. It matters when the conversation is read back from
    /// the database, where `convert_conversation` turns it into the exchange's
    /// `AIAgentInput::UserQuery`. Without it a restored conversation has the
    /// agent's half and not the user's, no `initial_query`, no title, and an
    /// exchange whose `start_time` falls through `unwrap_or_default()` to the
    /// Unix epoch — which is what "58 years ago" in the history panel was.
    fn user_query(&mut self) -> api::Message {
        let body = api::message::Message::UserQuery(api::message::UserQuery {
            query: self.prompt.clone(),
            context: Some(api::InputContext {
                current_time: Some(self.timestamp()),
                ..Default::default()
            }),
            ..Default::default()
        });
        self.message(body)
    }

    fn content_block(&mut self, block: ContentBlock) -> Option<api::Message> {
        let body = match block {
            ContentBlock::Text { text } if !text.trim().is_empty() => {
                api::message::Message::AgentOutput(api::message::AgentOutput { text })
            }
            ContentBlock::Thinking { thinking } if !thinking.trim().is_empty() => {
                api::message::Message::AgentReasoning(api::message::AgentReasoning {
                    reasoning: thinking,
                    finished_duration: None,
                })
            }
            // Deliberately *not* a `ToolCall` message. A ToolCall is an
            // instruction — the client's action model executes it and returns a
            // result. Claude has already run this tool itself, so emitting one
            // would run it a second time. Reporting it as text says what
            // happened without asking for it to happen again.
            ContentBlock::ToolUse { name } => {
                api::message::Message::AgentOutput(api::message::AgentOutput {
                    text: format!("`{name}`"),
                })
            }
            ContentBlock::Text { .. } | ContentBlock::Thinking { .. } | ContentBlock::Ignored => {
                return None;
            }
        };

        Some(self.message(body))
    }

    /// Wraps a message body with the identity and time every message needs.
    ///
    /// The timestamp is not decoration. `convert_conversation` derives a
    /// restored exchange's `finish_time` from it and falls back to it for
    /// `start_time`, so an unstamped message becomes a conversation that
    /// happened in 1970.
    fn message(&mut self, body: api::message::Message) -> api::Message {
        self.next_message += 1;
        api::Message {
            id: format!("{}-{}", self.request_id, self.next_message),
            task_id: self.task_id.clone(),
            request_id: self.request_id.clone(),
            timestamp: Some(self.timestamp()),
            message: Some(body),
            ..Default::default()
        }
    }

    /// One time for the whole turn, taken when it started.
    ///
    /// Deliberately not `now()` per message: these all belong to one exchange,
    /// and a turn that took a minute should not look like a minute of
    /// conversation history.
    fn timestamp(&self) -> prost_types::Timestamp {
        prost_types::Timestamp {
            seconds: self.started_at.timestamp(),
            nanos: self.started_at.timestamp_subsec_nanos() as i32,
        }
    }

    fn finished(&self, result: ResultEvent) -> api::ResponseEvent {
        use api::response_event::stream_finished;

        let reason = if result.is_error {
            stream_finished::Reason::InternalError(stream_finished::InternalError {
                message: result
                    .result
                    .unwrap_or_else(|| "Claude Code reported a failure.".to_owned()),
            })
        } else {
            stream_finished::Reason::Done(stream_finished::Done {})
        };

        let usage = result.usage.unwrap_or_default();
        api::ResponseEvent {
            r#type: Some(api::response_event::Type::Finished(
                api::response_event::StreamFinished {
                    reason: Some(reason),
                    token_usage: vec![stream_finished::TokenUsage {
                        model_id: String::new(),
                        total_input: usage.input_tokens
                            + usage.cache_read_input_tokens
                            + usage.cache_creation_input_tokens,
                        output: usage.output_tokens,
                        input_cache_read: usage.cache_read_input_tokens,
                        input_cache_write: usage.cache_creation_input_tokens,
                        // Left at zero rather than filled from Claude's
                        // `total_cost_usd`. That number is what the *model*
                        // costs; this field is what Warp charged, and under
                        // fork policy Warp charged nothing.
                        cost_in_cents: 0.0,
                    }],
                    ..Default::default()
                },
            )),
        }
    }
}

fn actions(actions: Vec<api::client_action::Action>) -> api::ResponseEvent {
    api::ResponseEvent {
        r#type: Some(api::response_event::Type::ClientActions(
            api::response_event::ClientActions {
                actions: actions
                    .into_iter()
                    .map(|action| api::ClientAction {
                        action: Some(action),
                    })
                    .collect(),
            },
        )),
    }
}

#[cfg(test)]
#[path = "translate_tests.rs"]
mod tests;
