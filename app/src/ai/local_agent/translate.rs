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
    #[serde(rename = "user")]
    User { message: UserMessage },
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
    /// Present on `subtype: "compact_boundary"`, and only there. See
    /// [`Translator::compact`].
    #[serde(default)]
    compact_metadata: Option<CompactMetadata>,
}

/// What Claude reports about a compaction it has just finished.
///
/// The numbers are the whole evidence that `/compact` did anything: the
/// summary itself arrives separately, and could be produced without dropping a
/// single token.
#[derive(Debug, Deserialize)]
struct CompactMetadata {
    /// Context size before, in tokens.
    #[serde(default)]
    pre_tokens: i32,
    /// Context size after. Used as the summary's token count, which it
    /// slightly overstates — the preserved tail of the conversation is in
    /// there too — but it is the only number on offer, and it is the one a
    /// reader of "Conversation summarized" wants: how much is left.
    #[serde(default)]
    post_tokens: i32,
    #[serde(default)]
    duration_ms: u64,
}

#[derive(Debug, Deserialize)]
struct UserMessage {
    #[serde(default)]
    content: UserContent,
}

/// Claude spells a user message's content either way.
///
/// The compaction summary arrives as a bare string; ordinary user turns arrive
/// as blocks. Accepting both costs one enum and means a change of mind
/// upstream does not silently drop the summary.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum UserContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
    /// Anything else at all. Present so that a shape nobody here anticipated
    /// costs an empty summary rather than an unparseable line — an untagged
    /// enum with no arm left to try fails the whole `ClaudeEvent`, and the
    /// event this is reached through is the one carrying the summary.
    Ignored(serde::de::IgnoredAny),
}

impl Default for UserContent {
    fn default() -> Self {
        Self::Ignored(serde::de::IgnoredAny)
    }
}

impl UserContent {
    fn text(&self) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Blocks(blocks) => blocks
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
            Self::Ignored(_) => String::new(),
        }
    }
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

/// Which of the two conversations with Claude this is.
///
/// The two streams do not have the same shape, so this is not decoration. A
/// query opens with `system/init`; a `/compact` emits its `init` *in the
/// middle*, when the compacted session restarts — and emits none at all when
/// the compaction is refused. Reading the second as if it were the first
/// produces a stream with its opening event two thirds of the way through, or
/// missing.
pub(super) enum Mode {
    Query {
        /// Recorded into the transcript as a `UserQuery` message. See
        /// [`Translator::user_query`].
        prompt: String,
    },
    Compact {
        /// The session `--resume` was given. In this mode the id is known
        /// before Claude says anything, which is what lets the stream open on
        /// its own terms rather than on an event that may not arrive.
        ///
        /// Safe because compaction leaves the session id alone: verified by
        /// running it, `pre_tokens: 22060` → `post_tokens: 2337`, same id
        /// either side.
        session: String,
        instructions: Option<String>,
    },
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
    mode: Mode,
    next_message: u64,
    saw_result: bool,
    /// `false` until the [`StreamInit`](api::response_event::StreamInit) has
    /// been emitted. Only [`Mode::Compact`] reads it — a compaction's stream
    /// carries a `system/init` of its own, and a second `StreamInit` would
    /// hand the client a conversation token mid-turn.
    opened: bool,
    /// Set at `compact_boundary`, cleared by the summary that follows it.
    ///
    /// This is how the summary is *identified*. On disk Claude flags it
    /// `isCompactSummary`, but that field does not survive onto the stream, so
    /// the only honest handle is its position: the first user message after
    /// the boundary. Everything after that — Claude's own `Compacted` echo —
    /// belongs to the CLI, not to Warp.
    boundary: Option<CompactMetadata>,
    /// When the turn started, stamped onto every message.
    started_at: DateTime<Utc>,
}

impl Translator {
    pub(super) fn new(
        task_id: String,
        task_needs_announcing: bool,
        request_id: String,
        mode: Mode,
        started_at: DateTime<Utc>,
    ) -> Self {
        Self {
            task_id,
            task_needs_announcing,
            request_id,
            mode,
            next_message: 0,
            saw_result: false,
            opened: false,
            boundary: None,
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

        match self.mode {
            Mode::Query { .. } => self.query(event),
            Mode::Compact { .. } => self.compact(event),
        }
    }

    /// A user's turn.
    fn query(&mut self, event: ClaudeEvent) -> Vec<api::ResponseEvent> {
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
                let mut events = vec![self.init(session_id)];
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
                                description: task_description(self.prompt()),
                                ..Default::default()
                            }),
                        },
                    )]));
                }
                let query = self.user_query();
                events.push(self.add(vec![query]));
                events
            }
            ClaudeEvent::Assistant { message } => self.assistant(message),
            ClaudeEvent::Result(result) => {
                self.saw_result = true;
                vec![self.finished(result)]
            }
            ClaudeEvent::System(_) | ClaudeEvent::User { .. } | ClaudeEvent::Ignored => Vec::new(),
        }
    }

    /// A `/compact`.
    ///
    /// Claude's compaction is a conversation about the conversation, and most
    /// of it is Claude talking to itself: a status event each side of the work,
    /// a fresh `init` for the session it just rewrote, then the summary
    /// injected as the first message of the new context, then the CLI's own
    /// `Compacted` echo. Warp wants exactly one thing out of all that — the
    /// summary — so everything else is dropped rather than relayed.
    ///
    /// A refused compaction ("Not enough messages to compact") comes back as
    /// an ordinary assistant message and is shown as one. It is not an error:
    /// it is the true answer to what was asked.
    fn compact(&mut self, event: ClaudeEvent) -> Vec<api::ResponseEvent> {
        // Nothing here waits for `system/init`. On a refused compaction it
        // never arrives, and a stream that ends without ever having opened is
        // reported to the user as a dropped connection.
        let mut events = self.open();
        match event {
            ClaudeEvent::System(system) if system.subtype == "compact_boundary" => {
                if let Some(metadata) = system.compact_metadata {
                    log::debug!(
                        "local agent: compacted {} -> {} tokens in {}ms",
                        metadata.pre_tokens,
                        metadata.post_tokens,
                        metadata.duration_ms
                    );
                    self.boundary = Some(metadata);
                }
            }
            ClaudeEvent::User { message } => {
                if let Some(metadata) = self.boundary.take() {
                    let summary = self.summarization(&message.content.text(), &metadata);
                    events.push(self.add(vec![summary]));
                }
            }
            ClaudeEvent::Assistant { message } => events.extend(self.assistant(message)),
            ClaudeEvent::Result(result) => {
                self.saw_result = true;
                events.push(self.finished(result));
            }
            ClaudeEvent::System(_) | ClaudeEvent::Ignored => {}
        }
        events
    }

    /// The `StreamInit` that opens a compaction's stream, once.
    fn open(&mut self) -> Vec<api::ResponseEvent> {
        if self.opened {
            return Vec::new();
        }
        let Mode::Compact {
            session,
            instructions,
        } = &self.mode
        else {
            return Vec::new();
        };
        let (session, instructions) = (session.clone(), instructions.clone());
        let mut events = vec![self.init(session)];
        // The request, written into the transcript the way upstream writes it:
        // a system query, which `convert_conversation` deliberately does not
        // render as user input. Without it a restored conversation has a
        // summary that nobody asked for.
        let request = self.message(api::message::Message::SystemQuery(
            api::message::SystemQuery {
                r#type: Some(api::message::system_query::Type::SummarizeConversation(
                    api::message::SummarizeConversation {
                        prompt: instructions.unwrap_or_default(),
                    },
                )),
                context: Some(api::InputContext {
                    current_time: Some(self.timestamp()),
                    ..Default::default()
                }),
            },
        ));
        events.push(self.add(vec![request]));
        events
    }

    fn init(&mut self, session_id: String) -> api::ResponseEvent {
        self.opened = true;
        api::ResponseEvent {
            r#type: Some(api::response_event::Type::Init(
                api::response_event::StreamInit {
                    conversation_id: session_id,
                    request_id: self.request_id.clone(),
                    run_id: String::new(),
                },
            )),
        }
    }

    fn assistant(&mut self, message: AssistantMessage) -> Vec<api::ResponseEvent> {
        let messages: Vec<api::Message> = message
            .content
            .into_iter()
            .filter_map(|block| self.content_block(block))
            .collect();
        if messages.is_empty() {
            return Vec::new();
        }
        vec![self.add(messages)]
    }

    fn add(&self, messages: Vec<api::Message>) -> api::ResponseEvent {
        actions(vec![api::client_action::Action::AddMessagesToTask(
            api::client_action::AddMessagesToTask {
                task_id: self.task_id.clone(),
                messages,
            },
        )])
    }

    fn prompt(&self) -> &str {
        match &self.mode {
            Mode::Query { prompt } => prompt,
            Mode::Compact { .. } => "",
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
            query: self.prompt().to_owned(),
            context: Some(api::InputContext {
                current_time: Some(self.timestamp()),
                ..Default::default()
            }),
            ..Default::default()
        });
        self.message(body)
    }

    /// The summary, as the "Conversation summarized" block.
    ///
    /// A `Summarization` message rather than plain agent output, because the
    /// two are not the same claim: agent output is an answer to the user,
    /// while this is a statement that the conversation behind it has been
    /// replaced. Warp renders the difference — collapsed, headed, and excluded
    /// from a copied transcript — and only if it is told.
    fn summarization(&mut self, summary: &str, metadata: &CompactMetadata) -> api::Message {
        let body = api::message::Message::Summarization(api::message::Summarization {
            finished_duration: Some(prost_types::Duration {
                seconds: (metadata.duration_ms / 1_000) as i64,
                nanos: (metadata.duration_ms % 1_000) as i32 * 1_000_000,
            }),
            summary_type: Some(
                api::message::summarization::SummaryType::ConversationSummary(
                    api::message::summarization::ConversationSummary {
                        summary: readable_summary(summary),
                        token_count: metadata.post_tokens,
                    },
                ),
            ),
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

/// Claude's compaction summary begins with a paragraph addressed to Claude:
///
/// > This session is being continued from a previous conversation that ran out
/// > of context. The summary below covers the earlier portion of the
/// > conversation.
/// >
/// > Summary:
///
/// It is a prompt, not prose — its job is to orient the model that reads it
/// next, and it says "ran out of context" whether or not anything ran out.
/// Under a heading that already reads "Conversation summarized" it is noise,
/// and misleading noise at that.
///
/// Both halves of the preamble have to be present to drop either, so a change
/// of wording upstream costs a stray line rather than a truncated summary.
fn readable_summary(summary: &str) -> String {
    const PREAMBLE: &str = "This session is being continued from a previous conversation";
    const MARKER: &str = "\nSummary:\n";

    let summary = summary.trim();
    if !summary.starts_with(PREAMBLE) {
        return summary.to_owned();
    }
    match summary.split_once(MARKER) {
        Some((_, body)) => body.trim().to_owned(),
        None => summary.to_owned(),
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
