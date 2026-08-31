//! Claude Code's `stream-json` events, said in Warp's vocabulary.
//!
//! Pure functions over strings: one JSON line in, zero or more
//! [`api::ResponseEvent`] out. No process, no clock, no network — which is what
//! makes the interesting half of this feature testable without either end.
//!
//! It also produces a second, smaller output: [`ToolEvent`], the tool calls
//! Claude ran, for the fork's event log (T11.1c). Those are *accumulated*, not
//! written — the caller drains them — because a file here would cost this file
//! the property in the paragraph above.

use std::collections::HashMap;

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
    Assistant {
        message: AssistantMessage,
        /// The `tool_use.id` of the call this message was produced *inside*, so
        /// non-null means a subagent. Lives on the event rather than on the
        /// block, which is why it has to be carried down by hand.
        #[serde(default)]
        parent_tool_use_id: Option<String>,
    },
    #[serde(rename = "user")]
    User {
        message: UserMessage,
        #[serde(default)]
        parent_tool_use_id: Option<String>,
    },
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
    ToolUse {
        /// Claude's per-call id, which is what ties this to the `tool_result`
        /// that answers it. It becomes the log's `call_id`.
        #[serde(default)]
        id: String,
        #[serde(default)]
        name: String,
        /// Whatever the tool's schema says, so deliberately untyped — see
        /// [`input_preview`].
        #[serde(default)]
        input: serde_json::Value,
    },
    /// The answer to a `tool_use`, which arrives on a `user` message rather
    /// than an assistant one. Warp renders nothing for it — Claude has already
    /// said in prose whatever the result meant — but it is the only place the
    /// stream says a tool *finished*, so the log needs it.
    #[serde(rename = "tool_result")]
    ToolResult {
        #[serde(default)]
        tool_use_id: String,
        /// Absent on success in some versions of the CLI and `false` in others;
        /// both were observed on 2026-08-25 from the same binary. Defaulting is
        /// not defensive here, it is required.
        #[serde(default)]
        is_error: bool,
    },
    #[serde(other)]
    Ignored,
}

/// A tool call as Claude reported it, on its way to the fork's event log.
///
/// Data, not an action: the translator accumulates these and the caller writes
/// them, which is what keeps a filesystem out of this file. The log's own
/// vocabulary — event names, truncation — belongs to
/// `event_log::local_agent`, not here; this says only what Claude said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ToolEvent {
    Started {
        call_id: String,
        /// The call this one ran inside, i.e. `parent_tool_use_id`. `Some` means
        /// a subagent is doing the work — verified against a real `Task` turn on
        /// 2026-08-25, where the nested `Read` named the `Agent` call that
        /// spawned it.
        parent_call_id: Option<String>,
        name: String,
        input_preview: Option<String>,
    },
    Completed {
        call_id: String,
        parent_call_id: Option<String>,
        /// The name carried by the matching [`ToolEvent::Started`].
        ///
        /// `None` means no `tool_use` with this id was seen on this stream. It
        /// is not an error — a turn can be resumed part-way through one — but
        /// it is worth being able to see, so the result is recorded without a
        /// name rather than dropped.
        name: Option<String>,
        failed: bool,
    },
}

/// The part of a tool's input worth putting in the log's `tool_input_preview`.
///
/// Two keys and not the whole object, matching what Warp's own agent puts there
/// (`event_log::warp_agent::tool_input_preview`): a reader greps this field for
/// what was *run*, and widening it to every argument would make that grep
/// unreliable across sources as well as putting more of a tool's payload —
/// which is where its secrets are — in a file.
///
/// Untyped on purpose, which a struct would say better. `input` is whatever the
/// tool's schema declares, so `command` is a string for `Bash` and could be any
/// shape at all for an MCP tool — and a deserialize failure here would take the
/// **whole assistant message** with it, because [`Translator::on_line`] drops a
/// line it cannot parse. A missing preview is worth far less than a missing
/// answer.
fn input_preview(input: &serde_json::Value) -> Option<String> {
    ["command", "file_path"]
        .into_iter()
        .find_map(|key| input.get(key).and_then(serde_json::Value::as_str))
        .map(str::to_owned)
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
    /// Tool calls seen since the caller last drained them (T11.1c).
    tool_events: Vec<ToolEvent>,
    /// `tool_use.id` → tool name, so a `tool_result` can say *what* finished
    /// rather than only that something with that id did.
    ///
    /// Bounded by the calls in flight, not by the turn: an entry is removed by
    /// the result that answers it.
    /// Warp's one disclosure sentence, held until the task it would attach to
    /// exists.
    ///
    /// **This is a `None` after it has been said, and the reason it is held at
    /// all is a bug that compiled and unit-tested cleanly.** The first cut
    /// seeded the note into the turn's pending queue *before* the stream
    /// started, reasoning that the sentence should reach the panel ahead of the
    /// agent's first token. But a note is an `AddMessagesToTask`, and the task
    /// is created by `CreateTask` further down this very function -- so the
    /// message named a task that did not exist yet and went nowhere. Measured:
    /// zero `[Warp]` lines in the panel, on a build whose unit test for the note
    /// passed. Ordering against the stream is not something a unit test on the
    /// note can see.
    pending_announcement: Option<String>,
    tool_names: HashMap<String, String>,
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
            tool_events: Vec::new(),
            tool_names: HashMap::new(),
            pending_announcement: None,
        }
    }

    /// Takes the tool calls seen since the last call.
    ///
    /// Drained rather than returned from [`Self::on_line`] because they are a
    /// different audience: `ResponseEvent`s are for the client's task model and
    /// these are for the log, and threading a second return value through every
    /// arm would have made the interesting code harder to read to save one
    /// call site.
    pub(super) fn take_tool_events(&mut self) -> Vec<ToolEvent> {
        std::mem::take(&mut self.tool_events)
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
                // After `CreateTask` and before the user's own turn: the task
                // now exists to attach to, and Warp's sentence reads first.
                if let Some(text) = self.pending_announcement.take() {
                    events.push(self.note(text));
                }
                let query = self.user_query();
                events.push(self.add(vec![query]));
                events
            }
            ClaudeEvent::Assistant {
                message,
                parent_tool_use_id,
            } => self.assistant(message, parent_tool_use_id.as_deref()),
            ClaudeEvent::Result(result) => {
                self.saw_result = true;
                vec![self.finished(result)]
            }
            // A user message on a query is Claude feeding itself a tool result.
            // Warp renders nothing for it — this arm returns no events, as it
            // always has — but it is where the stream says a tool finished, so
            // it is no longer *ignored*.
            ClaudeEvent::User {
                message,
                parent_tool_use_id,
            } => {
                self.observe(&message.content, parent_tool_use_id.as_deref());
                Vec::new()
            }
            ClaudeEvent::System(_) | ClaudeEvent::Ignored => Vec::new(),
        }
    }

    /// Records the tool results in a user message, and nothing else.
    ///
    /// Takes the whole [`UserContent`] rather than the blocks so the two shapes
    /// it can arrive in stay this function's problem: a compaction summary is a
    /// bare string and carries no tool results, which is the correct answer
    /// rather than a case to handle.
    fn observe(&mut self, content: &UserContent, parent_call_id: Option<&str>) {
        let UserContent::Blocks(blocks) = content else {
            return;
        };
        for block in blocks {
            let ContentBlock::ToolResult {
                tool_use_id,
                is_error,
            } = block
            else {
                continue;
            };
            self.tool_events.push(ToolEvent::Completed {
                name: self.tool_names.remove(tool_use_id),
                call_id: tool_use_id.clone(),
                parent_call_id: parent_call_id.map(str::to_owned),
                failed: *is_error,
            });
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
            ClaudeEvent::User {
                message,
                parent_tool_use_id,
            } => {
                // A compaction is not supposed to run tools, and in practice it
                // does not. Observed anyway rather than assumed not to happen:
                // the cost is one match and the alternative is a silence that
                // would be indistinguishable from the gap this task closed.
                self.observe(&message.content, parent_tool_use_id.as_deref());
                if let Some(metadata) = self.boundary.take() {
                    let summary = self.summarization(&message.content.text(), &metadata);
                    events.push(self.add(vec![summary]));
                }
            }
            ClaudeEvent::Assistant {
                message,
                parent_tool_use_id,
            } => events.extend(self.assistant(message, parent_tool_use_id.as_deref())),
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

    fn assistant(
        &mut self,
        message: AssistantMessage,
        parent_call_id: Option<&str>,
    ) -> Vec<api::ResponseEvent> {
        let messages: Vec<api::Message> = message
            .content
            .into_iter()
            .filter_map(|block| self.content_block(block, parent_call_id))
            .collect();
        if messages.is_empty() {
            return Vec::new();
        }
        vec![self.add(messages)]
    }

    /// Holds Warp's disclosure until there is a task to attach it to.
    ///
    /// Set before the stream starts; spent on the `init` event, immediately
    /// after `CreateTask`.
    pub(super) fn announce_transcript(&mut self, text: String) {
        self.pending_announcement = Some(text);
    }

    /// One sentence from Warp, in the panel, in Warp's own voice.
    ///
    /// The mirror of `acp_agent`'s `note`, and it exists for the same reason:
    /// the transcript pointer rides on the prompt where the person cannot see
    /// it, so without this they would be watching an agent act on an
    /// instruction they were never shown. Marked with the transcript module's
    /// `[Warp]` chrome, which `strip_chrome` then keeps out of the transcript
    /// file — an agent must never read Warp's asides back as its own words.
    pub(super) fn note(&mut self, text: String) -> api::ResponseEvent {
        let message = self.message(api::message::Message::AgentOutput(
            api::message::AgentOutput { text },
        ));
        self.add(vec![message])
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

    fn content_block(
        &mut self,
        block: ContentBlock,
        parent_call_id: Option<&str>,
    ) -> Option<api::Message> {
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
            //
            // That decision is also why the log needs this line: because no
            // `ToolCall` is emitted, Warp's action model never sees the call,
            // so `event_log::warp_agent` — which watches that model — cannot
            // report it either. T11.1c.
            ContentBlock::ToolUse { id, name, input } => {
                self.tool_events.push(ToolEvent::Started {
                    call_id: id.clone(),
                    parent_call_id: parent_call_id.map(str::to_owned),
                    name: name.clone(),
                    input_preview: input_preview(&input),
                });
                self.tool_names.insert(id, name.clone());
                api::message::Message::AgentOutput(api::message::AgentOutput {
                    text: format!("`{name}`"),
                })
            }
            // Only ever arrives on a `user` message, where [`Self::observe`]
            // has it. Listed rather than folded into the catch-all so that it
            // is visibly decided about.
            ContentBlock::ToolResult { .. }
            | ContentBlock::Text { .. }
            | ContentBlock::Thinking { .. }
            | ContentBlock::Ignored => {
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
