//! An append-only record of agent events, for a run nobody is watching (T11.1).
//!
//! Warp already has the taxonomy this needs. `CLIAgentEventType` is a versioned
//! protocol carried as OSC 777 on the PTY, negotiated through
//! `WARP_CLI_AGENT_PROTOCOL_VERSION`, with `PermissionRequest` and
//! `PermissionReplied` as first-class events. What it has no answer for is
//! *keeping* one: `CLIAgentSessionsModel` is three in-memory `HashMap`s, so an
//! event is parsed, updates a session, paints, and is gone. Warp's own agent
//! history persists to SQLite; the agents it merely hosts do not persist at all.
//!
//! So this module is a **projection, not a taxonomy** — the smallest thing that
//! is still the idea. It decides nothing about what an event *means*; it is
//! handed events that were already understood and appends them to a file. Policy
//! lives in [`crate::fork::event_log_dir`].
//!
//! **Two sources, one vocabulary.** Warp has two event worlds and they do not
//! meet anywhere else: `CLIAgentEventType` for the agents Warp merely *hosts*
//! (this file's original caller, world 2), and `BlocklistAIHistoryEvent` /
//! `BlocklistAIActionEvent` for Warp's own agent (world 1, projected by
//! [`warp_agent`] as T11.1b). They arrive as an [`Entry`], which is the only
//! shape this module writes, so a reader filtering for `permission_request` gets
//! every agent's answer without knowing there were ever two enums. The `source`
//! field says which world a line came from; nothing else needs to.
//!
//! **One line per event, JSON, one file per session.** JSONL because the reader
//! is `tail -f`, `jq`, and eventually an SSE endpoint that re-broadcasts lines
//! it did not have to parse. Per session because that is the unit a person asks
//! about ("what happened in that run"), and because it keeps concurrent agents
//! from interleaving into an unreadable single file.
//!
//! **Writes are synchronous and flushed.** These events are human-paced — a
//! tool call costs tens of milliseconds of real work at minimum — so the cost
//! is far below anything `WARP_FORK_FRAME_LOG` would report, and a log you
//! cannot `tail` while the thing is running is not much of a log. If that ever
//! stops being true, the frame log is the instrument that will say so.

use std::collections::HashMap;
use std::fs::File;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use serde_with::skip_serializing_none;
use tokio::sync::broadcast;

use crate::terminal::cli_agent_sessions::event::{CLIAgentEvent, CLIAgentEventSource};

// Spawns a local `claude`, so it follows `ai::local_agent`'s own gate.
#[cfg(not(target_family = "wasm"))]
pub(crate) mod local_agent;
pub(crate) mod warp_agent;

/// Longest session id accepted as a filename stem. Long enough for a UUID and
/// then some; short enough that no `PATH_MAX` on any platform is in play.
const MAX_KEY_LEN: usize = 64;

/// Longest free text kept on a line. The wire protocol's own limit is 320
/// characters (`MAX_NOTIFICATION_DESCRIPTION_CHARS` in the TUI publisher) and
/// matching it keeps summaries comparable between the worlds.
///
/// Shared by every adapter rather than owned by one: three sources whose
/// `tool_input_preview` truncated at three different lengths would make the
/// field useless for exactly the comparison it exists to support.
const MAX_TEXT_LEN: usize = 320;

/// Where events for a session with no id are collected.
const UNKEYED: &str = "unkeyed";

/// One event, as its source describes it. Everything a caller supplies; `ts` and
/// `seq` are the writer's and are stamped in [`record`].
///
/// Borrowed throughout, because every field already exists somewhere the caller
/// is holding — this type is an argument list with names, not a value anyone
/// keeps.
#[skip_serializing_none]
#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct Entry<'a> {
    /// Protocol version the event was parsed under, when it came off a wire.
    ///
    /// **Absent means it did not.** Warp's own agent emits in-process and has no
    /// protocol version; inventing a `1` for it would claim a compatibility
    /// guarantee that does not exist. So `v` present is exactly "this line
    /// crossed the OSC 777 boundary".
    pub v: Option<u32>,
    pub agent: &'a str,
    pub event: &'a str,
    /// How the event reached the log: `rich_plugin` or `codex_osc9_fallback`
    /// for a hosted agent, `in_process` for Warp's own.
    ///
    /// Worth keeping for the first two, because a session silently running on
    /// the degraded fallback path explains a lot of missing detail downstream —
    /// and worth keeping for the third, because `agent` alone cannot separate
    /// Warp's in-app agent from its headless TUI, which announces itself over
    /// the wire under the same name.
    pub source: &'a str,
    pub session_id: Option<&'a str>,
    /// The *other* id for this same turn, when a turn has two.
    ///
    /// **Fork (T14.15), and it exists because a log with two id spaces and no
    /// link between them reads as a log that is missing half its events.**
    /// [`session_key`] below already names the two worlds: Warp's own ids, and
    /// the ones that cross a process boundary before Warp sees them. An ACP turn
    /// has one of each — Warp's conversation id and the agent's session id — and
    /// each keys a *different file*. Measured 2026-08-29, one turn wrote
    /// `<conversation>.jsonl` holding `session_start` and `stop`, and
    /// `<acp-session>.jsonl` holding `tool_start` and `tool_complete`, with
    /// nothing in either naming the other. Opening the first alone shows a
    /// session with nothing between its ends, which is exactly the false belief
    /// `CLAUDE.md` carried for a day after it stopped being true.
    ///
    /// So this is the join key, written from both ends: a line keyed by one id
    /// names the other. Absent for every source with only one id, which is all
    /// of them except the ACP path.
    pub linked_session_id: Option<&'a str>,
    /// A stable id for the tool call this event belongs to, so a `tool_complete`
    /// can be tied to the `permission_request` that preceded it.
    ///
    /// Present for Warp's own agent, which has always had one
    /// (`AIAgentActionId`), and absent for hosted agents, whose protocol carries
    /// no such field — that half is `TR-EVENTS-B` and needs a version bump,
    /// because the id has to come from the plugin.
    pub call_id: Option<&'a str>,
    /// The `call_id` of the tool call this one ran *inside*, when it ran inside
    /// one. A subagent's work, in other words.
    ///
    /// **Present only for `local_agent` lines** (T11.1c), because Claude's
    /// stream is the only one of the three sources that says so
    /// (`parent_tool_use_id`). Warp's own agent has no nesting to report and
    /// the hosted-agent protocol carries no such field.
    ///
    /// Without it, containment is only inferable from interleaving — a
    /// subagent's tools happen to fall between its own `tool_start` and
    /// `tool_complete` — and that inference stops working the moment two
    /// subagents run at once, which is the case this fork most wants to watch.
    pub parent_call_id: Option<&'a str>,
    pub cwd: Option<&'a str>,
    pub project: Option<&'a str>,
    pub tool_name: Option<&'a str>,
    pub tool_input_preview: Option<&'a str>,
    pub summary: Option<&'a str>,
    pub error_type: Option<&'a str>,
    pub plugin_version: Option<&'a str>,
    /// What a person answered a permission request, on a `permission_replied`
    /// line: `allowed`, `denied`, or `unanswered`.
    ///
    /// **`unanswered` is an explicit value and never an absent field**, which
    /// looks like redundancy and is not. This struct is
    /// `#[skip_serializing_none]`, so a `None` does not serialize as `null` —
    /// it *vanishes from the line*. A `permission_replied` with no `decision`
    /// key would therefore be indistinguishable from a line written by a build
    /// from before this field existed, which collapses "nobody answered" into
    /// "old binary" — a versioning ambiguity in the one record that exists to
    /// be believed later. The absence-grammar this fork uses elsewhere
    /// (`ParkedRequest::acts_on`'s "empty means the agent never said") is
    /// readable because those fields are always present; this one would not be.
    ///
    /// **Present only on `acp_agent` lines.** Warp's own agent path reaches
    /// `permission_replied` through a conversation-status transition that
    /// carries no decision, so it cannot fill this in — the same per-source
    /// fidelity `parent_call_id` documents one field up. A reader counting
    /// denials across sources reads this on ACP lines and infers it on
    /// `warp_agent` ones from a `permission_request` followed by a cancelled
    /// `tool_complete`. That is a fidelity gap to know about, not two
    /// vocabularies.
    ///
    /// **That cross-source claim has two halves with different evidence, and
    /// they are worth keeping apart** — an earlier draft of this comment
    /// labelled the whole thing "read, not run", which *understated* the first
    /// half and overstated the difficulty of settling the second.
    ///
    /// - **The mapping is test-held.** `status_event` is a pure function and
    ///   `leaving_blocked_is_an_answer_and_not_a_new_prompt`
    ///   (`warp_agent_tests.rs`) pins `Blocked → InProgress` →
    ///   `permission_replied`. That is as verified as anything on that path can
    ///   be here, which is exactly why `action_event`'s own note says a test is
    ///   the only thing that can hold it.
    /// - **The flow is conditional, and an earlier version of this comment
    ///   stated one branch of it as the whole answer.** Rejecting a proposed
    ///   command calls `cancel_action_with_id(…, ManuallyCancelled)` on one
    ///   *action*; `cancel_pending_action` turns it into a cancelled action
    ///   *result* and hands it to `handle_action_result`. What happens next
    ///   depends on what else is in the drained phase
    ///   (`action_model.rs:1451-1480`): once no pending actions remain and the
    ///   reason maps to `CancellationOutcome::Cancelled` — which
    ///   `ManuallyCancelled` does (`ai/agent/mod.rs:208`) — the status becomes
    ///   `Cancelled` **if every finished result is cancelled**, and
    ///   `InProgress` otherwise.
    ///
    ///   So rejecting the *only* pending action ends the conversation as
    ///   `Cancelled`, which `status_event` maps to `stop_failure` and **not**
    ///   to `permission_replied`. Rejecting one call among others that
    ///   succeeded resumes the turn and does emit `permission_replied`. Both
    ///   readings this repo has recorded were half of that: the first said a
    ///   denial never emits the event, the second said it always does.
    ///
    ///   **The practical consequence for a reader counting denials across
    ///   sources**: on `warp_agent` lines a lone rejected call leaves no
    ///   `permission_replied` at all, so its `permission_request` is the only
    ///   trace and the answer is inferable solely from the turn ending as
    ///   `stop_failure`. The ACP source has no such hole — it writes
    ///   `decision: denied` in every case, which is the fidelity gap this
    ///   field exists to close.
    ///
    /// **And "unverified" here means not-yet, not cannot.** What is unavailable
    /// on this fork is the *live server path* — reaching Warp's own agent needs
    /// the account the fork exists without. That is not the only instrument: the
    /// flow half is model-layer behaviour, `action_model_tests.rs` exists beside
    /// the code that implements it, and `integration_testing/agent_mode` already
    /// asserts on `ConversationStatus::Blocked`. A test driving
    /// `cancel_pending_action` and observing that the conversation is not
    /// cancelled would move this half to test-held. Recorded that way because
    /// this fork has filed "impossible here" confidently and wrongly before —
    /// T12's "no browser on this machine", three times, when it meant the WSL
    /// userland.
    pub decision: Option<&'a str>,
    /// Which of Warp's surfaces carried the answer — `control_plane` or
    /// `panel`. See `acp_agent::registry::Surface`, which owns the names and
    /// the argument for why `control_plane` is one value covering three doors.
    ///
    /// Absent when `decision` is `unanswered`, and that absence is safe here
    /// precisely because the value on the same line explains it: nobody
    /// answered, so no surface carried anything.
    pub answered_by: Option<&'a str>,
    /// Whether Warp could have offered a *yes* at all, on a
    /// `permission_request` line.
    ///
    /// **This is the field that lets the log detect its own falsifier.**
    /// Without it a `denied` line is ambiguous between a person saying no and
    /// Warp never having offered a yes to say — and telling those apart is the
    /// entire question T14.17 was built to make answerable. The reason, when
    /// this is `false`, is prose and rides `summary`.
    pub can_approve: Option<bool>,
    /// Whether Warp acted on the event rather than discarding it. False when a
    /// hosted agent's event arrived for a terminal the sessions model has no
    /// session for.
    ///
    /// **Recorded rather than filtered, deliberately.** A dropped event is
    /// exactly the silent failure this phase exists to catch, and a log that
    /// only contains what succeeded cannot show you the one that did not.
    pub applied: bool,
}

/// One line. Flat rather than an envelope wrapping the entry, because every
/// reader of this file is a filter — `jq 'select(.event=="permission_request")'`
/// should not have to know which fields live one level down.
#[derive(Debug, Serialize, PartialEq, Eq)]
struct Record<'a> {
    /// Warp's clock at the moment the event was understood, not the agent's.
    /// The agent's clock is not ours to trust and the protocol does not carry
    /// one anyway; what this answers is "when did Warp know".
    ts: String,
    /// Monotonic across the process, so two events in the same millisecond
    /// still have an order, and a gap is visible as a gap.
    seq: u64,
    #[serde(flatten)]
    entry: Entry<'a>,
}

/// Open files, keyed by sanitized session key.
struct Sink {
    dir: PathBuf,
    files: Mutex<HashMap<String, File>>,
}

static SINK: OnceLock<Option<Sink>> = OnceLock::new();

/// The sequence source.
///
/// Process-global rather than a field of [`Sink`], because a subscriber
/// (T11.2) is a consumer with no file behind it, and `seq` is documented as
/// process-global — it should not quietly restart or vanish depending on
/// whether `WARP_FORK_EVENT_LOG` named a directory.
static SEQ: AtomicU64 = AtomicU64::new(0);

/// Live fan-out of rendered lines, for the SSE endpoint (T11.2).
///
/// Carries the **same string** that goes to the file, so a subscriber
/// re-broadcasts bytes it never had to parse and cannot drift from the on-disk
/// format. Capacity is bounded: a subscriber that stops reading is lagged and
/// told so by `RecvError::Lagged` rather than being allowed to grow the buffer
/// without limit. Dropping events for a slow reader is the correct trade here —
/// the file is the durable record, and this channel is the live view.
static BROADCAST: OnceLock<broadcast::Sender<String>> = OnceLock::new();

/// Capacity in events. Sized for a burst of tool calls, not for a backlog:
/// anything that falls this far behind wants the file, not the stream.
const BROADCAST_CAPACITY: usize = 256;

fn broadcast() -> &'static broadcast::Sender<String> {
    BROADCAST.get_or_init(|| broadcast::channel(BROADCAST_CAPACITY).0)
}

/// Subscribes to live events. Every subscriber makes [`is_enabled`] true.
pub(crate) fn subscribe() -> broadcast::Receiver<String> {
    broadcast().subscribe()
}

fn sink() -> Option<&'static Sink> {
    SINK.get_or_init(|| {
        let dir = crate::fork::event_log_dir()?;
        // Owner-only: these lines carry `tool_input_preview`, which is the
        // command an agent asked to run and the file it asked to touch.
        if let Err(err) = crate::fork::create_private_dir(&dir) {
            log::warn!("fork event log: cannot create {}: {err}", dir.display());
            return None;
        }
        log::info!("fork event log: writing to {}", dir.display());
        Some(Sink {
            dir,
            files: Mutex::new(HashMap::new()),
        })
    })
    .as_ref()
}

/// Whether anything is listening at all — a file, a live subscriber, or both.
///
/// For callers that would do work to build an [`Entry`] — the world-1
/// projection looks conversations up in the history model — and should not do
/// it when the answer goes nowhere.
///
/// **This is now dynamic.** Before T11.2 it meant "`WARP_FORK_EVENT_LOG` named a
/// directory" and was fixed for the life of the process. An SSE subscriber can
/// arrive and leave at any time, so it can flip either way, and a caller must
/// not cache it.
pub(crate) fn is_enabled() -> bool {
    sink().is_some() || broadcast().receiver_count() > 0
}

/// Appends one event to the file, and hands it to any live subscriber.
///
/// A no-op unless `WARP_FORK_EVENT_LOG` asked for a log or something is
/// subscribed — checked again here rather than trusted from the caller, because
/// the last subscriber can leave between their [`is_enabled`] and this call.
pub(crate) fn record(entry: Entry<'_>) {
    let sink = sink();
    let live = broadcast().receiver_count() > 0;
    if sink.is_none() && !live {
        return;
    }
    // Stamped once and shared, so the line a subscriber sees is byte-identical
    // to the line on disk — including `seq`.
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let key = session_key(entry.session_id);
    let line = line(seq, now, entry);

    if let Some(sink) = sink {
        if let Err(err) = sink.append(&key, &line) {
            // Warn once per failure rather than reporting: a full disk should not
            // turn observability into a second outage on top of the first.
            log::warn!("fork event log: append to {key} failed: {err}");
        }
    }
    if live {
        // The only error is "no receivers left", which is not one: the race
        // between the check above and here is expected and means nobody cared.
        let _ = broadcast().send(line);
    }
}

/// Appends an event from an agent Warp is hosting (world 2).
///
/// `applied` is whether the sessions model acted on the event; see
/// [`Entry::applied`].
pub(crate) fn record_cli_agent(event: &CLIAgentEvent, applied: bool) {
    if !is_enabled() {
        return;
    }
    record(hosted_agent_entry(event, applied));
}

/// The world-2 adapter, split from the writing so it can be asserted without a
/// filesystem.
fn hosted_agent_entry(event: &CLIAgentEvent, applied: bool) -> Entry<'_> {
    let payload = &event.payload;
    Entry {
        v: Some(event.v),
        agent: event
            .agent
            .command_prefixes()
            .first()
            .copied()
            .unwrap_or("?"),
        event: event.event.wire_name(),
        source: match event.source {
            CLIAgentEventSource::RichPlugin => "rich_plugin",
            CLIAgentEventSource::CodexOsc9Fallback => "codex_osc9_fallback",
        },
        session_id: event.session_id.as_deref(),
        // One id only: this source has no second id space to join to.
        linked_session_id: None,
        call_id: None,
        parent_call_id: None,
        cwd: event.cwd.as_deref(),
        project: event.project.as_deref(),
        tool_name: payload.tool_name.as_deref(),
        tool_input_preview: payload.tool_input_preview.as_deref(),
        summary: payload.summary.as_deref(),
        error_type: payload.error_type.as_deref(),
        plugin_version: payload.plugin_version.as_deref(),
        decision: None,
        answered_by: None,
        can_approve: None,
        applied,
    }
}

impl Sink {
    fn append(&self, key: &str, line: &str) -> std::io::Result<()> {
        let mut files = self.files.lock().expect("fork event log mutex poisoned");
        let file = match files.get_mut(key) {
            Some(file) => file,
            None => {
                let path = self.dir.join(format!("{key}.jsonl"));
                let file = crate::fork::create_private_file(&path, true)?;
                // `open` ignores the mode on a file that already exists, so a log
                // an earlier build left world-readable would stay that way for as
                // long as this session kept appending to it.
                crate::fork::tighten_existing(&path);
                files.entry(key.to_string()).or_insert(file)
            }
        };
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;
        file.flush()
    }
}

/// Renders one record as a JSON line.
///
/// Split out from the writing so the format can be asserted without a
/// filesystem, which is most of what is worth testing here.
fn line(seq: u64, ts: String, entry: Entry<'_>) -> String {
    let record = Record { ts, seq, entry };
    // A record that cannot be serialized is a bug in this file, not a runtime
    // condition, but it must not take the session down to prove it.
    serde_json::to_string(&record).unwrap_or_else(|err| {
        format!(r#"{{"ts":"?","seq":{seq},"event":"serialize_failed","error":"{err}"}}"#)
    })
}

/// Turns an agent-supplied session id into a filename stem.
///
/// **The agent controls this string**, so it is not a filename until it has
/// been made one: anything outside `[A-Za-z0-9._-]` is replaced, leading dots
/// are stripped, and the result is truncated. Without that, a `session_id` of
/// `../../.bashrc` is a plugin choosing where Warp writes.
///
/// World 1's ids are Warp's own — a `AIConversationId` is a UUID — so nothing
/// here can fire on them. It exists for world 2, where the string crosses a
/// process boundary before Warp sees it.
fn session_key(session_id: Option<&str>) -> String {
    let Some(raw) = session_id else {
        return UNKEYED.to_string();
    };
    let cleaned: String = raw
        .chars()
        .take(MAX_KEY_LEN)
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    // Order matters here, and the tests pin it. Leading dots go first, so a name
    // that is *only* dots collapses to nothing and takes the fallback rather
    // than becoming the plausible-looking `_.`. Then `..` is collapsed: the
    // character filter leaves it intact, so `../../x` arrives as `_.._x`, which
    // cannot escape — no separator survives — but leaving a traversal segment in
    // a filename means every future reader has to re-derive that argument. One
    // non-overlapping pass suffices, because each replacement consumes both dots.
    let cleaned = cleaned.trim_start_matches('.').replace("..", "_");
    if cleaned.is_empty() {
        UNKEYED.to_string()
    } else {
        cleaned
    }
}

/// The working directory's last component, matching what the TUI publisher puts
/// in `project`.
pub(crate) fn project_name(cwd: &str) -> Option<&str> {
    std::path::Path::new(cwd)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
}

/// Collapses whitespace and truncates, so one line stays one line.
///
/// A raw command or query can contain newlines, and a newline in a JSONL record
/// would be escaped rather than break the file — but the escaping is what a
/// person reading `tail -f` would have to undo, so it is removed here instead.
pub(crate) fn excerpt(text: &str) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut excerpt: String = normalized.chars().take(MAX_TEXT_LEN).collect();
    if normalized.chars().count() > MAX_TEXT_LEN {
        excerpt.push('…');
    }
    excerpt
}

/// Where a session's file lands, for callers that want to read one back.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn path_for(dir: &Path, session_id: Option<&str>) -> PathBuf {
    dir.join(format!("{}.jsonl", session_key(session_id)))
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
