//! ACP `SessionUpdate`s, said in Warp's vocabulary.
//!
//! Pure functions over protocol values: one update in, zero or more
//! [`api::ResponseEvent`] out. No process, no clock, no network — the same
//! property `local_agent/translate.rs` has, and for the same reason: it is what
//! makes the interesting half testable without either end.
//!
//! # The mapping table, and where it came from
//!
//! `warpctrl acp probe` exists to produce this table by running real agents
//! rather than by reading the schema. Both columns are measured — Claude
//! through `claude-agent-acp`, and `opencode` 1.18.25 over OpenRouter:
//!
//! | `SessionUpdate` | becomes | why |
//! |---|---|---|
//! | `AgentMessageChunk` | `AgentOutput` | the answer |
//! | `AgentThoughtChunk` | `AgentReasoning` | rendered as thinking, not as output |
//! | `ToolCall` | `AgentOutput`, as text | **never a `ToolCall` message** — see below |
//! | `ToolCallUpdate` | `AgentOutput`, on completion only | a title that arrives late is the useful one |
//! | `UserMessageChunk` | nothing | Warp already holds the prompt it sent |
//! | `Plan`, `PlanUpdate`, `PlanRemoved` | nothing | Warp has a todo model; wiring it is not this step |
//! | `AvailableCommandsUpdate` | nothing | a menu for a UI that is not here |
//! | `CurrentModeUpdate` | nothing | T14.3/T14.4: the mode is a claim and must not be rendered as governance |
//! | `ConfigOptionUpdate`, `SessionInfoUpdate` | nothing | measured: opencode sends 356 models here |
//! | `UsageUpdate` | nothing | Warp's own accounting is on the `StreamFinished` |
//! | anything else | nothing | `#[non_exhaustive]`; a new variant must not fail a turn |
//!
//! # A tool call is reported, never requested
//!
//! **`ToolCall` never becomes `api::message::Message::ToolCall`.** That type is
//! an *instruction*: Warp's action model executes it and returns a result. The
//! ACP agent has already run the tool itself, so emitting one would run it a
//! second time.
//!
//! This is inherited verbatim from `local_agent/translate.rs`, which found it
//! the hard way, and it is written out again here rather than assumed because
//! T14 produced three separate instances of a hazard being recorded in prose and
//! then built against anyway. There is a test under this paragraph.
//!
//! # Nothing here *answers* a permission
//!
//! Permission requests are a *request* on the connection, not an update on this
//! stream, so none arrives through the mapping below. What answers them is
//! `warp_cli`'s `acp_permission`, reached from `mod.rs`. Keeping the two apart
//! is deliberate: this file decides how something is *shown*, and showing must
//! not be able to authorize.
//!
//! **The heading said "reads" and the body said they "never reach this file"
//! until 2026-08-31, and T14.17 had made both false that same morning.**
//! `log_permission_request` and `log_permission_replied` take a
//! `registry::ParkedRequest` and write the audit lines from it, so a permission
//! request very much reaches this file — it is handed here deliberately, so the
//! record of what was shown is written by whatever did the showing.
//!
//! What is still exactly true is the part that matters, and it is why the
//! separation exists: **nothing here can change an outcome.** Both functions are
//! observation only, `log_permission_request`'s own doc says so, and a test pins
//! that logging emits no client action. Written down rather than quietly
//! reworded because the author of the stale sentence and the author of the code
//! that staled it were the same person, hours apart, and neither noticed —
//! which is this repo's most-repeated failure with the shortest possible fuse.

use std::collections::{HashMap, HashSet};

use agent_client_protocol::schema::v1::{
    ContentBlock, ContentChunk, SessionUpdate, StopReason, ToolCall, ToolCallLocation,
    ToolCallStatus, ToolCallUpdate, ToolKind,
};
use chrono::{DateTime, Utc};
use warp_multi_agent_api as api;

use super::registry;
use crate::event_log::Entry;

/// How much of the prompt becomes the conversation's name in the history panel.
///
/// Same constant and same reasoning as the local agent: there is no summariser
/// on this path either, so the prompt itself is the honest stand-in.
const TASK_DESCRIPTION_CHARS: usize = 60;

fn task_description(prompt: &str) -> String {
    let prompt = prompt.trim();
    // `char_indices` rather than byte slicing: a prompt can end mid-glyph and
    // `String::truncate` would panic on the boundary.
    match prompt.char_indices().nth(TASK_DESCRIPTION_CHARS) {
        Some((cut, _)) => format!("{}…", prompt[..cut].trim_end()),
        None => prompt.to_owned(),
    }
}

/// Which kind of message buffered text is on its way to becoming.
///
/// Answer and reasoning are shown differently, so a run of one must be flushed
/// before a run of the other starts rather than merged into it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pending {
    Output,
    Reasoning,
}

/// Turns one ACP turn into one Warp response stream.
pub(super) struct Translator {
    task_id: String,
    task_needs_announcing: bool,
    request_id: String,
    prompt: String,
    next_message: u64,
    started_at: DateTime<Utc>,
    /// Text seen since the last flush, and which kind of message it becomes.
    ///
    /// **Found by running it.** ACP's `agent_message_chunk` is a *token* stream,
    /// not a paragraph stream: the first live turn through Warp's panel rendered
    /// `"notes.txt doesn"`, `"'t exist in this"`, `" directory"` as separate
    /// messages, because one chunk was becoming one message. Claude's path never
    /// showed this — `stream-json` delivers whole content blocks — so nothing in
    /// the fork had met it before.
    ///
    /// Buffering to a natural boundary gives exactly the granularity
    /// `local_agent` already produces. The alternative is
    /// `AppendToMessageContent`, which the protocol has and which is built for
    /// precisely this — but its `FieldMask` path into the `Message` oneof is not
    /// used anywhere in this repo, so shipping it would mean guessing an input
    /// and finding out from a silent failure. Named on T14.6 instead.
    pending: Option<(Pending, String)>,
    /// Tool calls whose title has already been shown, so a late correction is
    /// not printed twice.
    ///
    /// Measured: both agents send `tool_call` with a placeholder title and then
    /// correct it on `tool_call_update` — Claude sent *"Preparing file…"* before
    /// *"Write a.txt"*, opencode sent *"read"* before the path. So the first
    /// title is usually the useless one, and this is what lets the second be
    /// shown without showing both.
    announced: Vec<(String, String)>,
    /// Where each tool call said it would act, by `toolCallId`.
    ///
    /// Accumulated from the notification stream because the permission request
    /// for the same call does not carry it — see [`Self::note_locations`].
    locations: Vec<(String, Vec<String>)>,
    /// Whether [`Self::open`] has run, i.e. whether Warp has seen a `StreamInit`
    /// for this turn.
    ///
    /// **This is a correctness flag, not bookkeeping.** Every other event this
    /// type produces is addressed to a stream Warp is already tracking, and a
    /// failure before `open` has no such stream to be reported into — it was
    /// measured to vanish completely, panel and log alike. See
    /// [`Self::stream_was_opened`].
    opened: bool,
    /// The agent's own session id, once it has named one.
    ///
    /// Carried so a parked permission request can be lined up with the lines
    /// `WARP_FORK_EVENT_LOG` wrote for the same session, the way
    /// `PendingApproval::session_id` already is for CLI agents.
    ///
    /// **It no longer names the event log's file** — see [`Self::conversation_id`].
    session_id: Option<String>,
    /// Warp's own id for this conversation, which is what the event log files
    /// under.
    ///
    /// **T14.15, and `local_agent` had already written down why.** Its
    /// `TurnContext::session_id` carries this same value with the comment *"Not
    /// Claude's session id … filing under it would put a turn's tools in a
    /// different file from its frame"* — and the ACP path then did exactly that.
    /// Measured: one turn wrote `session_start`/`stop` into
    /// `<conversation>.jsonl` and `tool_start`/`tool_complete` into
    /// `<acp-session>.jsonl`, with neither naming the other, so opening the
    /// obvious file showed a session with nothing between its ends.
    ///
    /// Filing under Warp's id also takes an agent-controlled string out of the
    /// filename, which [`event_log::session_key`] otherwise has to sanitise
    /// precisely because a session id of `../../.bashrc` would be a plugin
    /// choosing where Warp writes. The agent's id is still recorded, as
    /// `linked_session_id`, where it cannot pick a path.
    conversation_id: String,
    /// The program name the event log's `agent` field carries, resolved once from
    /// `WARP_FORK_ACP_COMMAND` rather than per tool call.
    ///
    /// `source` is what separates this path from the others; `agent` is which
    /// program the pane is talking to. Threaded in because there is no model to
    /// ask on this path, the same way `TurnContext` resolves its fields at spawn.
    agent: String,
    /// The session's working directory, on the log as `cwd`/`project`.
    ///
    /// This is the pane directory, which is what decides where the agent resolves
    /// its own permission rules — the security-relevant input T14.6 measured.
    cwd: Option<String>,
    /// Tool call ids whose `tool_start` has been logged, mapped to the kind
    /// named at the announcement, so a re-announcement is not double-logged and
    /// a completion that does not repeat the kind can still be named.
    started: HashMap<String, &'static str>,
    /// Tool call ids whose `tool_complete` has been logged.
    ///
    /// ACP streams several updates per call; the wire names the terminal ones
    /// (`Completed`/`Failed`), but may echo them, so each id is remembered to
    /// keep the log from echoing it.
    completed: HashSet<String>,
    /// Whether the updates arriving right now are history rather than news.
    ///
    /// `session/load` replays the whole conversation as ordinary
    /// `session/update` notifications before it answers — measured against
    /// `opencode`, which sent `user_message_chunk` then `agent_message_chunk`
    /// for a one-turn history and sent both *before* the reply. Warp already
    /// holds that transcript (the client sends its whole task list back on
    /// every request), so replaying it into the stream would draw the
    /// conversation twice.
    ///
    /// **Framed as "until the reply" rather than "the first N", deliberately.**
    /// The ordering above is measured for one agent and the spec does not
    /// appear to require it, so a count would be a guess about every other
    /// agent. A window that closes when the request answers is right whatever
    /// the agent sends, including nothing at all.
    replaying: bool,
    /// What the agent said its modes were when this session opened (T14.18).
    ///
    /// Held only so a `current_mode_update` — which carries an id and nothing
    /// else — can be reported in the agent's own words rather than as a bare
    /// identifier. Never read to decide anything.
    modes: Option<agent_client_protocol::schema::v1::SessionModeState>,
}

impl Translator {
    pub(super) fn new(
        task_id: String,
        task_needs_announcing: bool,
        request_id: String,
        prompt: String,
        started_at: DateTime<Utc>,
        agent: String,
        cwd: Option<String>,
        conversation_id: String,
    ) -> Self {
        Self {
            task_id,
            task_needs_announcing,
            request_id,
            prompt,
            next_message: 0,
            started_at,
            pending: None,
            announced: Vec::new(),
            locations: Vec::new(),
            opened: false,
            session_id: None,
            conversation_id,
            agent,
            cwd,
            started: HashMap::new(),
            completed: HashSet::new(),
            replaying: false,
            modes: None,
        }
    }

    /// Start ignoring updates, because `session/load` is about to replay them.
    pub(super) fn begin_replay(&mut self) {
        self.replaying = true;
    }

    /// Stop ignoring updates: everything from here is this turn's own.
    pub(super) fn end_replay(&mut self) {
        self.replaying = false;
    }

    /// Warp's own id for this turn — unique per turn, and available from the
    /// moment the translator is built rather than only after `session/new`.
    pub(super) fn request_id(&self) -> String {
        self.request_id.clone()
    }

    /// The agent's session id, once `session/new` has answered.
    pub(super) fn session_id(&self) -> Option<String> {
        self.session_id.clone()
    }

    /// Whether a `StreamInit` has been emitted, which decides how a failure has
    /// to be reported. See [`Self::opened`].
    pub(super) fn stream_was_opened(&self) -> bool {
        self.opened
    }

    /// The events that open the stream, emitted once the agent has named its
    /// session.
    ///
    /// The ACP session id becomes Warp's conversation token, exactly as Claude's
    /// session id does on the other path: the client stores it and hands it back
    /// as `params.conversation_token`, so Warp's own round-tripping is the
    /// session store and this module keeps no state between turns.
    /// Remember what the agent advertised, for [`super::mode::changed`].
    pub(super) fn remember_modes(
        &mut self,
        modes: Option<agent_client_protocol::schema::v1::SessionModeState>,
    ) {
        self.modes = modes;
    }

    pub(super) fn open(&mut self, session_id: String) -> Vec<api::ResponseEvent> {
        self.opened = true;
        self.session_id = Some(session_id.clone());
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
                        // `AIConversation::title` reads this first and only
                        // falls back to the initial query, so a task with no
                        // description is a conversation called "Untitled" in
                        // the history panel.
                        description: task_description(&self.prompt),
                        ..Default::default()
                    }),
                },
            )]));
        }
        let query = self.user_query();
        events.push(self.add(vec![query]));
        events
    }

    /// Translates one update. Anything not in the table above yields nothing:
    /// `SessionUpdate` is `#[non_exhaustive]` and an agent is versioned
    /// independently of this fork, so a variant added upstream must not take the
    /// conversation down with it.
    pub(super) fn on_update(&mut self, update: &SessionUpdate) -> Vec<api::ResponseEvent> {
        // History, not news. Dropped whole rather than merely not emitted: the
        // bookkeeping below — the accumulating text, the announced titles, the
        // tool-call locations — all describes *this* turn, and seeding it from a
        // replay would make a previous turn's tool call the one a permission
        // request gets lined up against.
        if self.replaying {
            return Vec::new();
        }
        // Text accumulates; anything else is a boundary that flushes it first,
        // so a tool call never lands in the middle of a sentence.
        let text = match update {
            SessionUpdate::AgentMessageChunk(chunk) => Some((Pending::Output, chunk_text(chunk))),
            SessionUpdate::AgentThoughtChunk(chunk) => {
                Some((Pending::Reasoning, chunk_text(chunk)))
            }
            _ => None,
        };
        if let Some((kind, text)) = text {
            if text.is_empty() {
                return Vec::new();
            }
            return match &mut self.pending {
                Some((pending, buffer)) if *pending == kind => {
                    buffer.push_str(&text);
                    Vec::new()
                }
                _ => {
                    let flushed = self.flush();
                    self.pending = Some((kind, text));
                    flushed
                }
            };
        }

        // **Before the display dispatch, and deliberately not inside it.** The
        // update that carries `locations` is a *status* update, and
        // `tool_update_text` returns early for anything that is not `Completed`
        // — so recording from there would drop exactly the one this exists for.
        // Recording is not showing: nothing below reads this map to draw
        // anything, only to answer "where does this run" for a parked request.
        match update {
            SessionUpdate::ToolCall(call) => {
                self.log_tool_start(call);
                self.note_locations(call.tool_call_id.to_string(), &call.locations);
            }
            SessionUpdate::ToolCallUpdate(update) => {
                if matches!(
                    update.fields.status,
                    Some(ToolCallStatus::Completed | ToolCallStatus::Failed)
                ) {
                    self.log_tool_complete(update);
                }
                if let Some(locations) = &update.fields.locations {
                    self.note_locations(update.tool_call_id.to_string(), locations);
                }
            }
            _ => {}
        }

        let body = match update {
            SessionUpdate::ToolCall(call) => match self.tool_text(call) {
                Some(text) => {
                    api::message::Message::AgentOutput(api::message::AgentOutput { text })
                }
                None => return Vec::new(),
            },
            SessionUpdate::ToolCallUpdate(update) => match self.tool_update_text(update) {
                Some(text) => {
                    api::message::Message::AgentOutput(api::message::AgentOutput { text })
                }
                None => return Vec::new(),
            },
            // The agent moved its own session's policy. The spec permits this
            // outright -- "Agents may also change modes autonomously and notify
            // the client via `current_mode_update`" -- so a mode Warp disclosed
            // at session start is not a mode that stays true, and a silent
            // change is T14.18's hazard returning through a door nobody
            // watched. Drawn as an ordinary note, in the agent's own words.
            SessionUpdate::CurrentModeUpdate(update) => {
                api::message::Message::AgentOutput(api::message::AgentOutput {
                    text: super::mode::changed(
                        &self.conversation_id,
                        self.modes.as_ref(),
                        &update.current_mode_id,
                    ),
                })
            }
            _ => return Vec::new(),
        };
        let mut events = self.flush();
        let message = self.message(body);
        events.push(self.add(vec![message]));
        events
    }

    /// Emits whatever text has accumulated, as one message.
    ///
    /// Must be called before the turn ends, or the agent's last sentence — which
    /// is usually its whole answer — is never shown. The driver in `mod.rs` does
    /// that; `finished` deliberately does not, because a caller that forgets is
    /// better caught by a test than by silence.
    pub(super) fn flush(&mut self) -> Vec<api::ResponseEvent> {
        let Some((kind, text)) = self.pending.take() else {
            return Vec::new();
        };
        if text.trim().is_empty() {
            return Vec::new();
        }
        let body = match kind {
            Pending::Output => {
                api::message::Message::AgentOutput(api::message::AgentOutput { text })
            }
            Pending::Reasoning => {
                api::message::Message::AgentReasoning(api::message::AgentReasoning {
                    reasoning: text,
                    finished_duration: None,
                })
            }
        };
        let message = self.message(body);
        vec![self.add(vec![message])]
    }

    /// What a newly announced tool call is shown as, if anything.
    ///
    /// **Deliberately not a `ToolCall` message** — see the module docs. The
    /// agent has already run it; this says what happened rather than asking for
    /// it to happen again.
    fn tool_text(&mut self, call: &ToolCall) -> Option<String> {
        let title = call.title.trim();
        if title.is_empty() {
            return None;
        }
        self.remember(call.tool_call_id.to_string(), title.to_owned());
        Some(format!("`{title}`"))
    }

    /// A later update for a call already seen.
    ///
    /// Only a *changed* title on a *finished* call is worth a second line. The
    /// measured streams send several updates per call — status transitions,
    /// content, raw output — and printing each would bury the answer.
    fn tool_update_text(&mut self, update: &ToolCallUpdate) -> Option<String> {
        if !matches!(update.fields.status, Some(ToolCallStatus::Completed)) {
            return None;
        }
        let title = update.fields.title.as_deref()?.trim();
        if title.is_empty() {
            return None;
        }
        let id = update.tool_call_id.to_string();
        let shown = self
            .announced
            .iter()
            .find(|(seen, _)| seen == &id)
            .map(|(_, title)| title.clone());
        if shown.as_deref() == Some(title) {
            return None;
        }
        self.remember(id, title.to_owned());
        Some(format!("`{title}`"))
    }

    /// Records the file paths a tool call said it would touch.
    ///
    /// **The join T14.6 measured the need for.** A `session/request_permission`
    /// carries a narrower view of the call than the notification stream did:
    /// captured live, the request had `locations: []` and a `rawInput` with no
    /// `cwd`, while the `tool_call_update` for the *same* `toolCallId` moments
    /// earlier carried `locations: [{"path": "/tmp/t146/project"}]`. So a card
    /// rendering the request alone shows a shell command and cannot say where it
    /// runs — and where it runs is the fact that decided, in that same session,
    /// whether the user's own permission rules were loaded at all.
    ///
    /// An empty list is not recorded over a known one: agents send `locations`
    /// on the update that has them and omit it elsewhere, and overwriting with
    /// nothing would lose the answer between the update and the request.
    fn note_locations(&mut self, id: String, locations: &[ToolCallLocation]) {
        if locations.is_empty() {
            return;
        }
        let paths = locations
            .iter()
            .map(|location| location.path.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        match self.locations.iter_mut().find(|(seen, _)| seen == &id) {
            Some(entry) => entry.1 = paths,
            None => self.locations.push((id, paths)),
        }
    }

    /// Where a tool call said it would act, if it ever said.
    ///
    /// `None` means the agent never sent a location for this call, which is a
    /// different thing from it acting nowhere — so a caller must render the
    /// absence as *unknown* rather than substitute a directory of Warp's own.
    pub(super) fn locations_for(&self, tool_call_id: &str) -> Option<Vec<String>> {
        self.locations
            .iter()
            .find(|(seen, _)| seen == tool_call_id)
            .map(|(_, paths)| paths.clone())
    }

    /// Appends a `tool_start` for a call the agent has announced.
    ///
    /// A `ToolCall` notification arrives once per call, but a re-announcement
    /// must not double-post, so the id is remembered. The kind is remembered
    /// with the id because a completion does not repeat it.
    fn log_tool_start(&mut self, call: &ToolCall) {
        if self.started.contains_key(&call.tool_call_id.to_string()) {
            return;
        }
        let kind = kind_name(call.kind);
        self.started.insert(call.tool_call_id.to_string(), kind);
        self.log(Entry {
            v: None,
            agent: &self.agent,
            event: "tool_start",
            source: "acp_agent",
            session_id: Some(&self.conversation_id),
            linked_session_id: self.session_id.as_deref(),
            call_id: Some(&call.tool_call_id.to_string()),
            parent_call_id: None,
            cwd: self.cwd.as_deref(),
            project: self.cwd.as_deref().and_then(crate::event_log::project_name),
            tool_name: Some(kind),
            // ACP's notification stream has no `rawInput`; that lives on the
            // permission request, which `mod.rs` handles and which this
            // module deliberately does not reach. So the preview stays
            // absent and the log says nothing rather than the wrong thing.
            tool_input_preview: None,
            summary: None,
            // A start is not a failure; a failure, when the wire reports one,
            // is on the completion.
            error_type: None,
            plugin_version: None,
            decision: None,
            answered_by: None,
            can_approve: None,
            applied: true,
        });
    }

    /// Appends a `tool_complete` for a call whose status has turned terminal —
    /// `Completed` or `Failed`.
    ///
    /// ACP streams several updates per call; only a terminal one matters here,
    /// and only once. The completion may not repeat the kind (found by running
    /// it: the kind rides the announcement and is absent here), so the kind
    /// remembered at `tool_start` is the fallback; the update's own kind wins
    /// when it is present, as it is the more recent statement.
    fn log_tool_complete(&mut self, update: &ToolCallUpdate) {
        if !self.completed.insert(update.tool_call_id.to_string()) {
            return;
        }
        let tool_name = update
            .fields
            .kind
            .map(kind_name)
            .or_else(|| self.started.get(&update.tool_call_id.to_string()).copied());
        self.log(Entry {
            v: None,
            agent: &self.agent,
            event: "tool_complete",
            source: "acp_agent",
            session_id: Some(&self.conversation_id),
            linked_session_id: self.session_id.as_deref(),
            call_id: Some(&update.tool_call_id.to_string()),
            parent_call_id: None,
            cwd: self.cwd.as_deref(),
            project: self.cwd.as_deref().and_then(crate::event_log::project_name),
            tool_name,
            tool_input_preview: None,
            summary: None,
            // The wire names failure (`ToolCallStatus::Failed`, "the tool call
            // failed with an error"), so the log says `failed`. local_agent
            // says `error` only because its stream carries a bare `is_error`;
            // a reader filtering for a failure must not grep for one word that
            // this source never writes. A call that failed is a completion,
            // never a `tool_start` with no partner — which would read as a hang.
            error_type: (update.fields.status == Some(ToolCallStatus::Failed)).then_some("failed"),
            plugin_version: None,
            decision: None,
            answered_by: None,
            can_approve: None,
            applied: true,
        });
    }

    /// Appends a `permission_request` for a question the agent has raised
    /// (T14.17).
    ///
    /// **Derived from [`registry::ParkedRequest`] rather than from the wire,
    /// and that coupling is the point.** `.fork/GOAL.md`'s second unattended
    /// rule asks for *"the `tool_input` that was shown"* — and the parked
    /// request is verifiably the shown thing: it is what every surface renders
    /// and what `agent.approvals` reports. Re-reading the raw
    /// `RequestPermissionRequest` here would log what *arrived*, which is the
    /// same string today and one refactor away from silently not being. A test
    /// pins the preview against `excerpt(parked.tool_input)` for exactly this
    /// reason.
    ///
    /// The `tool_call_id` is threaded separately rather than added to
    /// `ParkedRequest`, because it is a fact only this log consumes and the
    /// registry's type exists to serve surfaces.
    ///
    /// **Observation only.** Nothing here can change an outcome: it is called
    /// after the request has been parked and the answer is decided elsewhere.
    pub(super) fn log_permission_request(
        &self,
        request: &registry::ParkedRequest,
        tool_call_id: &str,
    ) {
        // Truncated by the same helper and to the same length as every other
        // source, because a preview cut at a different length would make the
        // field useless for the cross-source comparison it exists to support.
        let preview = request.tool_input.as_deref().map(crate::event_log::excerpt);
        let summary = ask_summary(request);
        self.log(Entry {
            v: None,
            agent: &self.agent,
            event: "permission_request",
            source: "acp_agent",
            session_id: Some(&self.conversation_id),
            linked_session_id: self.session_id.as_deref(),
            call_id: Some(tool_call_id),
            parent_call_id: None,
            cwd: self.cwd.as_deref(),
            project: self.cwd.as_deref().and_then(crate::event_log::project_name),
            tool_name: request.tool_name.as_deref(),
            tool_input_preview: preview.as_deref(),
            summary: Some(&summary),
            // An ask is not a failure. Warp having no yes to offer is reported
            // by `can_approve`, which is a fact about this build's allowlist
            // and never a judgement about the call — the overreach
            // `unconfined_reason`'s wording was corrected for in T14.8.
            error_type: None,
            plugin_version: None,
            decision: None,
            answered_by: None,
            can_approve: Some(request.approve_selects.is_some()),
            applied: true,
        });
    }

    /// Appends a `permission_replied` for a question that has stopped being
    /// open (T14.17).
    ///
    /// **One event with a `decision` field, not three event names.** The name
    /// is `warp_agent`'s, and it is literally true on all three paths: Warp
    /// replies to the agent in every case, including the one where nobody
    /// answered — `outcome_for` relays the prepared denial when the sender was
    /// dropped. So the event records *Warp's act* and the field records *the
    /// person's*, which is the more honest split as well as the one that keeps
    /// a cross-source `jq` counting the same kind of thing from both sources.
    ///
    /// **`decision` is always present**, `unanswered` included — see the
    /// field's own note on why an absent key would be unreadable here.
    pub(super) fn log_permission_replied(
        &self,
        approval_id: &str,
        tool_call_id: &str,
        decision: &str,
        answered_by: Option<&str>,
    ) {
        // The approval id, because `call_id` alone cannot pair a re-asked call:
        // an agent may ask about the same `toolCallId` more than once, which is
        // the stale-answer hazard `ParkedRequest::approval_id` is keyed to
        // avoid. Carrying it on both lines is what makes an ask and its answer
        // joinable without that ambiguity.
        let summary = format!("approval {approval_id}");
        self.log(Entry {
            v: None,
            agent: &self.agent,
            event: "permission_replied",
            source: "acp_agent",
            session_id: Some(&self.conversation_id),
            linked_session_id: self.session_id.as_deref(),
            call_id: Some(tool_call_id),
            parent_call_id: None,
            cwd: self.cwd.as_deref(),
            project: self.cwd.as_deref().and_then(crate::event_log::project_name),
            tool_name: None,
            tool_input_preview: None,
            summary: Some(&summary),
            error_type: None,
            plugin_version: None,
            decision: Some(decision),
            answered_by,
            can_approve: None,
            applied: true,
        });
    }

    /// Appends one line to the event log, unless this is a replay of history.
    ///
    /// Replayed updates date from a previous turn; logging them would credit
    /// this turn with tools it never ran (the same reason `on_update` drops
    /// them before display). `record` itself no-ops when nothing is listening,
    /// so the work of building the `Entry` is the only thing gated here.
    fn log(&self, entry: Entry<'_>) {
        if self.replaying {
            return;
        }
        crate::event_log::record(entry);
    }

    fn remember(&mut self, id: String, title: String) {
        match self.announced.iter_mut().find(|(seen, _)| seen == &id) {
            Some(entry) => entry.1 = title,
            None => self.announced.push((id, title)),
        }
    }

    /// The user's own turn, written into the transcript.
    ///
    /// Upstream the server echoes the query back as a message and a great deal
    /// hangs off that; live it is inert, but a restored conversation without it
    /// is missing the question.
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

    fn add(&self, messages: Vec<api::Message>) -> api::ResponseEvent {
        actions(vec![api::client_action::Action::AddMessagesToTask(
            api::client_action::AddMessagesToTask {
                task_id: self.task_id.clone(),
                messages,
            },
        )])
    }

    /// Wraps a body with the identity and time every message needs.
    ///
    /// The timestamp is not decoration: `convert_conversation` derives a
    /// restored exchange's `finish_time` from it, so an unstamped message
    /// becomes a conversation that happened in 1970.
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

    /// One time for the whole turn, taken when it started — these all belong to
    /// one exchange.
    fn timestamp(&self) -> prost_types::Timestamp {
        prost_types::Timestamp {
            seconds: self.started_at.timestamp(),
            nanos: self.started_at.timestamp_subsec_nanos() as i32,
        }
    }

    /// How the turn ended, in Warp's terms.
    ///
    /// `Refusal` and `MaxTokens` are reported as what they are rather than
    /// flattened into `Done`: a turn that stopped because the agent declined is
    /// a different event from one that finished, and a person reading a
    /// conversation that simply stops has no way to tell.
    pub(super) fn finished(&self, stop: StopReason) -> api::ResponseEvent {
        use api::response_event::stream_finished;

        let reason = match stop {
            StopReason::EndTurn => stream_finished::Reason::Done(stream_finished::Done {}),
            StopReason::Cancelled => stream_finished::Reason::Done(stream_finished::Done {}),
            StopReason::MaxTokens => {
                stream_finished::Reason::InternalError(stream_finished::InternalError {
                    message: "The agent stopped: it reached its token limit.".to_owned(),
                })
            }
            StopReason::MaxTurnRequests => {
                stream_finished::Reason::InternalError(stream_finished::InternalError {
                    message: "The agent stopped: it reached its limit on requests for this turn."
                        .to_owned(),
                })
            }
            StopReason::Refusal => {
                stream_finished::Reason::InternalError(stream_finished::InternalError {
                    message: "The agent declined to continue.".to_owned(),
                })
            }
            // `StopReason` is `#[non_exhaustive]`. An unknown reason is still a
            // finished turn — the alternative is a stream the client reports as
            // an unexpected EOF, which is a worse lie than "done".
            _ => stream_finished::Reason::Done(stream_finished::Done {}),
        };
        api::ResponseEvent {
            r#type: Some(api::response_event::Type::Finished(
                api::response_event::StreamFinished {
                    reason: Some(reason),
                    ..Default::default()
                },
            )),
        }
    }

    /// A failure, said in the conversation rather than swallowed.
    pub(super) fn failed(&self, message: String) -> api::ResponseEvent {
        use api::response_event::stream_finished;

        api::ResponseEvent {
            r#type: Some(api::response_event::Type::Finished(
                api::response_event::StreamFinished {
                    reason: Some(stream_finished::Reason::InternalError(
                        stream_finished::InternalError { message },
                    )),
                    ..Default::default()
                },
            )),
        }
    }

    /// One line of plain text in the conversation, from Warp rather than the
    /// agent.
    ///
    /// Used for the refusal notice. It is `AgentOutput` because there is no
    /// "the client says" message type on this protocol, so the text itself has
    /// to carry the attribution — see `mod.rs`.
    pub(super) fn note(&mut self, text: String) -> api::ResponseEvent {
        let message = self.message(api::message::Message::AgentOutput(
            api::message::AgentOutput { text },
        ));
        self.add(vec![message])
    }
}

/// The text of a content chunk, ignoring the parts Warp cannot render inline.
///
/// Images and audio arrive here too. Dropping them silently is wrong and
/// naming them is cheap, so an unrenderable block becomes a short line saying
/// what was there.
fn chunk_text(chunk: &ContentChunk) -> String {
    match &chunk.content {
        ContentBlock::Text(text) => text.text.clone(),
        ContentBlock::Image(_) => "[image]".to_owned(),
        ContentBlock::Audio(_) => "[audio]".to_owned(),
        ContentBlock::ResourceLink(link) => format!("[{}]", link.uri),
        _ => String::new(),
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

/// What a `permission_request` line says in prose.
///
/// Carries three things no machine field on the line does: the **approval id**,
/// which is the join key a `call_id` cannot supply on its own — an agent may
/// ask about the same `toolCallId` twice, which is the stale-answer hazard
/// `ParkedRequest::approval_id` is keyed against; the agent's own one-line
/// **title**; and, where Warp had no *yes* to offer, the **reason**, which
/// `acp_permission` already wrote for a person to read rather than for a filter.
///
/// The title earns its place in exactly the case the preview is empty. A
/// request carrying no `rawInput` is unapprovable *because* the title is all
/// there is (`approvable`'s second gate), so a line that dropped it would
/// record nothing whatever about what had been asked — the one case where this
/// log would be silent about its own subject.
fn ask_summary(request: &registry::ParkedRequest) -> String {
    let mut summary = format!("approval {}", request.approval_id);
    if let Some(title) = request.title.as_deref() {
        summary.push_str(" · ");
        summary.push_str(title);
    }
    // Phrased as what Warp could not offer, never as what the call deserves.
    // T14.8 corrected `unconfined_reason` for implying the second, and a log
    // line is read later with less context than a panel message, not more.
    if let Some(reason) = request.approve_refused_because.as_deref() {
        summary.push_str(" · Warp had no yes to offer: ");
        summary.push_str(reason);
    }
    summary
}

/// The wire name of a tool kind, for the log's `tool_name`.
///
/// `ToolKind` is a stable enum; the title is a display string the measured
/// agents correct late, which is the same finding `acp_consent.rs` records, so
/// the kind is what names a call here. The `_` arm is load-bearing: `ToolKind`
/// is `#[non_exhaustive]`, so an upstream addition must get a name, not a panic.
///
/// This is a near-copy of `acp_consent.rs`'s `kind_name` and it is deliberately
/// not shared. Those two (`acp_consent`, `acp_permission`) name a kind *for a
/// person* reading a consent card, where the `_` arm is a sentence; this one
/// names a kind *for a log*, where the `_` arm must be a token a grep or a join
/// can match, distinct from the real `ToolKind::Other` ("other"). A shared
/// function would have to pick one `_` arm for both audiences.
///
/// **This paragraph spent T14.17 attached to [`ask_summary`]**, which was
/// inserted above this function with no blank line between the two doc blocks.
/// It left `ask_summary` prefaced by an argument about `ToolKind` and `_` arms
/// it has neither of, and left the reason these two `kind_name`s stay apart
/// stranded exactly where someone would delete one of them. A blank line does
/// not fix this -- doc attributes accumulate across one onto the next item --
/// so the paragraph has to move. Found in review, the same class as
/// `registry.rs` and `fork.rs`.
fn kind_name(kind: ToolKind) -> &'static str {
    match kind {
        ToolKind::Read => "read",
        ToolKind::Edit => "edit",
        ToolKind::Delete => "delete",
        ToolKind::Move => "move",
        ToolKind::Search => "search",
        ToolKind::Execute => "execute",
        ToolKind::Think => "think",
        ToolKind::Fetch => "fetch",
        ToolKind::SwitchMode => "switch_mode",
        ToolKind::Other => "other",
        _ => "unknown",
    }
}

#[cfg(test)]
#[path = "translate_tests.rs"]
mod tests;
