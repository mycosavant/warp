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
//! | `ToolCall` | a tool row: a tagged `AgentOutput`, appended | **never a `ToolCall` message** — see below |
//! | `ToolCallUpdate` | the same row, updated in place | `UpdateTaskMessage` with a mask; see `crate::ai::tool_row` |
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
use crate::ai::tool_row::{Row, ToolRowState, UPDATE_MASK};
use crate::event_log::Entry;

/// How much of a call's output the row keeps behind its chevron.
///
/// The transcript keeps the detail too, and a `cargo test` run is unbounded.
/// The agent's own store has the whole thing; the row is for a person.
const DETAIL_CHARS: usize = 8_000;

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
    /// One row per tool call, by `toolCallId`, in the order announced.
    ///
    /// Measured: both agents send `tool_call` with a placeholder title and then
    /// correct it on `tool_call_update` — Claude sent *"Preparing file…"* before
    /// *"Write a.txt"*, opencode sent *"read"* before the path. Until
    /// 2026-09-03 the correction was shown only if it rode the *completion*,
    /// and on `claude-agent-acp` 0.73.0 it never does, so the placeholder was
    /// the only thing ever drawn. Now the row is one message, appended at the
    /// announcement and rewritten in place as the call is corrected, finishes,
    /// fails or is refused. See `crate::ai::tool_row`.
    rows: Vec<(String, RowDraft)>,
    /// Tool call ids Warp answered *no* to, so their failure is drawn as a
    /// refusal rather than as a fault of the agent's.
    denied: HashSet<String>,
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
            rows: Vec::new(),
            denied: HashSet::new(),
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
        // update that carries `locations` is a *status* update, and until
        // 2026-09-03 the display path returned early for anything that was not
        // `Completed` — so recording from there would have dropped exactly the
        // one this exists for. The row now absorbs locations too, but this map
        // is the one a parked request is answered from, and it is kept apart so
        // that answering "where does this run" never depends on what is drawn.
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

        // A tool call is a row, not a body: appended once, then rewritten in
        // place. The text before it is still flushed first, so the row never
        // lands mid-sentence.
        let row_event = match update {
            SessionUpdate::ToolCall(call) => Some(self.row_announced(call)),
            SessionUpdate::ToolCallUpdate(update) => self.row_updated(update),
            _ => None,
        };
        if matches!(
            update,
            SessionUpdate::ToolCall(_) | SessionUpdate::ToolCallUpdate(_)
        ) {
            return match row_event {
                Some(event) => {
                    let mut events = self.flush();
                    events.push(event);
                    events
                }
                None => Vec::new(),
            };
        }

        let body = match update {
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

    /// The row for a call the agent has just announced, appended.
    ///
    /// **Deliberately not a `ToolCall` message** — see the module docs. The
    /// agent has already run it; the row says what is happening rather than
    /// asking for it to happen again.
    ///
    /// A re-announcement of a known id is treated as an update, so a second
    /// `tool_call` for the same call rewrites the row instead of adding one.
    fn row_announced(&mut self, call: &ToolCall) -> api::ResponseEvent {
        let id = call.tool_call_id.to_string();
        if self.rows.iter().any(|(seen, _)| seen == &id) {
            let mut draft = self.take_row(&id).expect("just found");
            draft.absorb(
                Some(call.kind),
                tool_name(call.meta.as_ref()),
                call.raw_input.as_ref(),
                &call.locations,
                Some(&call.title),
                &call.content,
                call.raw_output.as_ref(),
                self.cwd.as_deref(),
            );
            let event = self.rewrite(&draft);
            self.rows.push((id, draft));
            return event;
        }
        let message = self.message(api::message::Message::AgentOutput(Default::default()));
        let mut draft = RowDraft::new(message.id.clone());
        draft.absorb(
            Some(call.kind),
            tool_name(call.meta.as_ref()),
            call.raw_input.as_ref(),
            &call.locations,
            Some(&call.title),
            &call.content,
            call.raw_output.as_ref(),
            self.cwd.as_deref(),
        );
        if matches!(
            call.status,
            ToolCallStatus::Completed | ToolCallStatus::Failed
        ) {
            draft.finish(call.status, self.denied.contains(&id));
        }
        let message = draft.row().into_message(message);
        self.rows.push((id, draft));
        self.add(vec![message])
    }

    /// A later update for a call already announced: the row rewritten, if
    /// anything it shows changed.
    ///
    /// An update for an id never announced is dropped — there is no row to
    /// rewrite and inventing one would draw a call whose start Warp never saw.
    /// A terminal row stays terminal; the measured streams echo the final
    /// status, and a second `completed` is the same fact twice.
    fn row_updated(&mut self, update: &ToolCallUpdate) -> Option<api::ResponseEvent> {
        let id = update.tool_call_id.to_string();
        let mut draft = self.take_row(&id)?;
        let before = draft.row();
        if !draft.terminal() {
            draft.absorb(
                update.fields.kind,
                tool_name(update.meta.as_ref()),
                update.fields.raw_input.as_ref(),
                update.fields.locations.as_deref().unwrap_or(&[]),
                update.fields.title.as_deref(),
                update.fields.content.as_deref().unwrap_or(&[]),
                update.fields.raw_output.as_ref(),
                self.cwd.as_deref(),
            );
            if let Some(status @ (ToolCallStatus::Completed | ToolCallStatus::Failed)) =
                update.fields.status
            {
                draft.finish(status, self.denied.contains(&id));
            }
        }
        let event = (draft.row() != before).then(|| self.rewrite(&draft));
        self.rows.push((id, draft));
        event
    }

    fn take_row(&mut self, id: &str) -> Option<RowDraft> {
        let index = self.rows.iter().position(|(seen, _)| seen == id)?;
        Some(self.rows.remove(index).1)
    }

    /// The `UpdateTaskMessage` that rewrites a row's body and tag in place.
    ///
    /// Identity and time are the announcement's, restated so the update finds
    /// its message; the mask names only what changes.
    fn rewrite(&self, draft: &RowDraft) -> api::ResponseEvent {
        let message = draft.row().into_message(api::Message {
            id: draft.message_id.clone(),
            task_id: self.task_id.clone(),
            request_id: self.request_id.clone(),
            timestamp: Some(self.timestamp()),
            ..Default::default()
        });
        actions(vec![api::client_action::Action::UpdateTaskMessage(
            api::client_action::UpdateTaskMessage {
                task_id: self.task_id.clone(),
                message: Some(message),
                mask: Some(prost_types::FieldMask {
                    paths: UPDATE_MASK.iter().map(|path| (*path).to_owned()).collect(),
                }),
            },
        )])
    }

    /// Everything the end of a turn owes the panel: the buffered text, and
    /// every row still `Running` rewritten as interrupted.
    ///
    /// Warp has stopped listening, whichever way the turn ended, so a row left
    /// `Running` would be a spinner over a process nobody is watching — the
    /// one thing a surface must not claim. Called by the driver before
    /// `finished`/`failed`; `finished` itself deliberately does not do this,
    /// for the reason `flush` gives.
    pub(super) fn end_of_turn(&mut self) -> Vec<api::ResponseEvent> {
        let mut events = self.flush();
        let open: Vec<String> = self
            .rows
            .iter()
            .filter(|(_, draft)| draft.state == ToolRowState::Running)
            .map(|(id, _)| id.clone())
            .collect();
        for id in open {
            let mut draft = self.take_row(&id).expect("just listed");
            draft.state = ToolRowState::Interrupted;
            events.push(self.rewrite(&draft));
            self.rows.push((id, draft));
        }
        events
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
        &mut self,
        approval_id: &str,
        tool_call_id: &str,
        decision: &str,
        answered_by: Option<&str>,
    ) {
        // Remembered for the row, not decided here: the answer has already
        // gone to the agent, and this only changes how its failure is drawn.
        if decision == "denied" {
            self.denied.insert(tool_call_id.to_owned());
        }
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

    /// Something Warp says in the conversation, in its own voice.
    ///
    /// **A tagged `AgentOutput`, not a bare one, since 2026-09-03.** This said
    /// *"it is `AgentOutput` because there is no 'the client says' message type
    /// on this protocol, so the text itself has to carry the attribution"* --
    /// and that was the architectural root of the composer's 9.4 : 1
    /// dilution (`.fork/COMPOSER.md`): the renderer could not tell Warp's
    /// words from the agent's because they were the same kind. The protocol
    /// still has no such type; the channel is the message's opaque payload,
    /// which the fork controls end to end. See `crate::ai::warp_note`.
    pub(super) fn note(&mut self, note: crate::ai::warp_note::Note) -> api::ResponseEvent {
        let message = self.message(api::message::Message::AgentOutput(Default::default()));
        let message = note.into_message(message);
        self.add(vec![message])
    }
}

/// What Warp knows about one tool call, from which its row is composed.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RowDraft {
    /// The id of the message the row was announced as; every rewrite names it.
    message_id: String,
    state: ToolRowState,
    verb: Verb,
    /// What the verb acts on -- a command, a path, a pattern -- once known.
    object: Option<String>,
    /// The agent's last title, kept as the object of last resort.
    title: Option<String>,
    /// Text the agent attached to the call, in order, without repeats.
    content: Vec<String>,
    /// The raw output, used only when the agent attached no content.
    raw_output: Option<String>,
}

/// The one verb of a row, in the three forms the states need.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Verb {
    running: &'static str,
    done: &'static str,
    base: &'static str,
}

impl RowDraft {
    fn new(message_id: String) -> Self {
        Self {
            message_id,
            state: ToolRowState::Running,
            verb: verb(ToolKind::Other, None),
            object: None,
            title: None,
            content: Vec::new(),
            raw_output: None,
        }
    }

    fn terminal(&self) -> bool {
        self.state != ToolRowState::Running
    }

    /// Folds one announcement's or update's fields in. Absent fields leave
    /// what was known; a present one is the agent's more recent statement.
    #[allow(clippy::too_many_arguments)]
    fn absorb(
        &mut self,
        kind: Option<ToolKind>,
        tool_name: Option<&str>,
        raw_input: Option<&serde_json::Value>,
        locations: &[ToolCallLocation],
        title: Option<&str>,
        content: &[agent_client_protocol::schema::v1::ToolCallContent],
        raw_output: Option<&serde_json::Value>,
        cwd: Option<&str>,
    ) {
        if kind.is_some() || tool_name.is_some() {
            let kind = kind.unwrap_or(ToolKind::Other);
            self.verb = verb(kind, tool_name);
        }
        if let Some(object) = object_from_input(raw_input, cwd) {
            self.object = Some(object);
        } else if self.object.is_none()
            && let Some(location) = locations.first()
        {
            self.object = Some(relative_to(&location.path.to_string_lossy(), cwd));
        }
        if let Some(title) = title.map(str::trim).filter(|title| !title.is_empty()) {
            self.title = Some(title.to_owned());
        }
        for text in content.iter().filter_map(content_text) {
            if !self.content.contains(&text) {
                self.content.push(text);
            }
        }
        if let Some(raw) = raw_output {
            let text = match raw {
                serde_json::Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            if !text.trim().is_empty() {
                self.raw_output = Some(text);
            }
        }
    }

    fn finish(&mut self, status: ToolCallStatus, denied: bool) {
        self.state = match status {
            ToolCallStatus::Completed => ToolRowState::Done,
            ToolCallStatus::Failed if denied => ToolRowState::Denied,
            ToolCallStatus::Failed => ToolRowState::Failed,
            _ => self.state,
        };
    }

    /// The agent's title, unless it is a placeholder -- the measured ones end
    /// in an ellipsis or are the bare kind (*"Terminal"*, *"read"*).
    fn usable_title(&self) -> Option<&str> {
        let title = self.title.as_deref()?;
        let placeholder = title.ends_with('\u{2026}')
            || title.ends_with("...")
            || title.eq_ignore_ascii_case("terminal")
            || title.eq_ignore_ascii_case(self.verb.base)
            || title.eq_ignore_ascii_case(self.verb.done);
        (!placeholder).then_some(title)
    }

    /// Verb and object, tensed for the state.
    ///
    /// Three shapes, in order of what is known. An object read from the input
    /// gets Warp's verb in the state's tense. Failing that, the agent's own
    /// title is used whole -- it is already a sentence with its own verb
    /// (*"Write a.txt"*, *"Search for callers"*), so it is prefixed for the
    /// states that need saying and left alone for the two that do not, because
    /// re-tensing someone else's sentence is how a row starts lying. Failing
    /// both, the verb stands alone.
    fn headline(&self) -> String {
        let verb = self.verb;
        match (self.object.as_deref(), self.usable_title()) {
            (Some(object), _) => match self.state {
                ToolRowState::Running => format!("{} {object}", verb.running),
                ToolRowState::Done => format!("{} {object}", verb.done),
                ToolRowState::Failed => format!("Failed to {} {object}", verb.base),
                ToolRowState::Denied => format!("Denied: {} {object}", verb.base),
                ToolRowState::Interrupted => {
                    format!("Interrupted while {} {object}", verb.running.to_lowercase())
                }
            },
            (None, Some(title)) => match self.state {
                ToolRowState::Running | ToolRowState::Done => title.to_owned(),
                ToolRowState::Failed => format!("Failed: {title}"),
                ToolRowState::Denied => format!("Denied: {title}"),
                ToolRowState::Interrupted => format!("Interrupted: {title}"),
            },
            (None, None) => match self.state {
                ToolRowState::Running => format!("{}\u{2026}", verb.running),
                ToolRowState::Done => verb.done.to_owned(),
                ToolRowState::Failed => format!("Failed to {}", verb.base),
                ToolRowState::Denied => format!("Denied: {}", verb.base),
                ToolRowState::Interrupted => {
                    format!("Interrupted while {}", verb.running.to_lowercase())
                }
            },
        }
    }

    fn detail(&self) -> String {
        let mut detail = self.content.join("\n\n");
        if detail.trim().is_empty()
            && let Some(raw) = &self.raw_output
        {
            detail = format!("```\n{}\n```", raw.trim_end());
        }
        if detail.chars().count() > DETAIL_CHARS {
            let cut: String = detail.chars().take(DETAIL_CHARS).collect();
            detail = format!("{cut}\n\n\u{2026} (truncated)");
        }
        detail
    }

    fn row(&self) -> Row {
        Row::new(self.state, self.headline(), self.detail())
    }
}

/// The agent's own name for the tool, where it says one.
///
/// `claude-agent-acp` puts it at `_meta.claudeCode.toolName` (measured at
/// 0.73.0: `Bash`, `Write`, `Read`, …). It is read only to pick a verb -- a
/// `Write` and an `Edit` are both `ToolKind::Edit`, and *"Wrote"* is the truer
/// word for the first -- and an agent that names nothing gets the kind's verb.
fn tool_name(meta: Option<&agent_client_protocol::schema::v1::Meta>) -> Option<&str> {
    meta?.get("claudeCode")?.get("toolName")?.as_str()
}

fn verb(kind: ToolKind, tool_name: Option<&str>) -> Verb {
    const fn v(running: &'static str, done: &'static str, base: &'static str) -> Verb {
        Verb {
            running,
            done,
            base,
        }
    }
    match tool_name {
        Some("Write") => return v("Writing", "Wrote", "write"),
        Some("Edit" | "MultiEdit" | "NotebookEdit") => return v("Editing", "Edited", "edit"),
        Some("Task") => return v("Delegating", "Delegated", "delegate"),
        Some("TodoWrite") => return v("Updating the plan", "Updated the plan", "update the plan"),
        _ => {}
    }
    match kind {
        ToolKind::Read => v("Reading", "Read", "read"),
        ToolKind::Edit => v("Editing", "Edited", "edit"),
        ToolKind::Delete => v("Deleting", "Deleted", "delete"),
        ToolKind::Move => v("Moving", "Moved", "move"),
        ToolKind::Search => v("Searching", "Searched", "search"),
        ToolKind::Execute => v("Running", "Ran", "run"),
        ToolKind::Think => v("Thinking", "Thought", "think"),
        ToolKind::Fetch => v("Fetching", "Fetched", "fetch"),
        ToolKind::SwitchMode => v("Switching mode", "Switched mode", "switch mode"),
        _ => v("Using", "Used", "use"),
    }
}

/// The object a call's `rawInput` names, if it names one Warp recognises.
///
/// The keys are the measured agent's, in the order that picks the right one
/// when several are present: a `Grep` carries `pattern` and `path`, and the
/// pattern is what was searched for. A command is cut to its first line.
fn object_from_input(raw_input: Option<&serde_json::Value>, cwd: Option<&str>) -> Option<String> {
    let input = raw_input?.as_object()?;
    for key in [
        "command",
        "file_path",
        "notebook_path",
        "pattern",
        "query",
        "url",
        "path",
        "description",
    ] {
        let Some(value) = input.get(key).and_then(|value| value.as_str()) else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        return Some(match key {
            "command" => match value.split_once('\n') {
                Some((first, _)) => format!("{}\u{2026}", first.trim_end()),
                None => value.to_owned(),
            },
            "file_path" | "notebook_path" | "path" => relative_to(value, cwd),
            _ => value.to_owned(),
        });
    }
    None
}

/// A path under the session directory, said from there.
fn relative_to(path: &str, cwd: Option<&str>) -> String {
    if let Some(cwd) = cwd
        && let Some(rest) = path.strip_prefix(cwd)
        && let Some(rest) = rest.strip_prefix(['/', '\\'])
        && !rest.is_empty()
    {
        return rest.to_owned();
    }
    path.to_owned()
}

/// The text of one piece of attached content, for the detail.
fn content_text(content: &agent_client_protocol::schema::v1::ToolCallContent) -> Option<String> {
    use agent_client_protocol::schema::v1::ToolCallContent;
    match content {
        ToolCallContent::Content(content) => match &content.content {
            ContentBlock::Text(text) => Some(text.text.clone()),
            ContentBlock::ResourceLink(link) => Some(format!("[{}]", link.uri)),
            _ => None,
        },
        ToolCallContent::Diff(diff) => Some(format!(
            "{}\n```\n{}\n```",
            diff.path.to_string_lossy(),
            diff.new_text.trim_end()
        )),
        ToolCallContent::Terminal(terminal) => Some(format!("terminal {}", terminal.terminal_id)),
        _ => None,
    }
    .filter(|text| !text.trim().is_empty())
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
