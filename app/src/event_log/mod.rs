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
use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use serde_with::skip_serializing_none;

use crate::terminal::cli_agent_sessions::event::{CLIAgentEvent, CLIAgentEventSource};

pub(crate) mod warp_agent;

/// Longest session id accepted as a filename stem. Long enough for a UUID and
/// then some; short enough that no `PATH_MAX` on any platform is in play.
const MAX_KEY_LEN: usize = 64;

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
    /// A stable id for the tool call this event belongs to, so a `tool_complete`
    /// can be tied to the `permission_request` that preceded it.
    ///
    /// Present for Warp's own agent, which has always had one
    /// (`AIAgentActionId`), and absent for hosted agents, whose protocol carries
    /// no such field — that half is `TR-EVENTS-B` and needs a version bump,
    /// because the id has to come from the plugin.
    pub call_id: Option<&'a str>,
    pub cwd: Option<&'a str>,
    pub project: Option<&'a str>,
    pub tool_name: Option<&'a str>,
    pub tool_input_preview: Option<&'a str>,
    pub summary: Option<&'a str>,
    pub error_type: Option<&'a str>,
    pub plugin_version: Option<&'a str>,
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

/// Open files, keyed by sanitized session key, plus the sequence source.
struct Sink {
    dir: PathBuf,
    seq: AtomicU64,
    files: Mutex<HashMap<String, File>>,
}

static SINK: OnceLock<Option<Sink>> = OnceLock::new();

fn sink() -> Option<&'static Sink> {
    SINK.get_or_init(|| {
        let dir = crate::fork::event_log_dir()?;
        if let Err(err) = std::fs::create_dir_all(&dir) {
            log::warn!("fork event log: cannot create {}: {err}", dir.display());
            return None;
        }
        log::info!("fork event log: writing to {}", dir.display());
        Some(Sink {
            dir,
            seq: AtomicU64::new(0),
            files: Mutex::new(HashMap::new()),
        })
    })
    .as_ref()
}

/// Whether anything is being logged at all.
///
/// For callers that would do work to build an [`Entry`] — the world-1
/// projection looks conversations up in the history model — and should not do
/// it when the answer goes nowhere.
pub(crate) fn is_enabled() -> bool {
    sink().is_some()
}

/// Appends one event. A no-op unless `WARP_FORK_EVENT_LOG` asked for a log.
pub(crate) fn record(entry: Entry<'_>) {
    let Some(sink) = sink() else {
        return;
    };
    let seq = sink.seq.fetch_add(1, Ordering::Relaxed);
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let key = session_key(entry.session_id);
    let line = line(seq, now, entry);

    if let Err(err) = sink.append(&key, &line) {
        // Warn once per failure rather than reporting: a full disk should not
        // turn observability into a second outage on top of the first.
        log::warn!("fork event log: append to {key} failed: {err}");
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
        call_id: None,
        cwd: event.cwd.as_deref(),
        project: event.project.as_deref(),
        tool_name: payload.tool_name.as_deref(),
        tool_input_preview: payload.tool_input_preview.as_deref(),
        summary: payload.summary.as_deref(),
        error_type: payload.error_type.as_deref(),
        plugin_version: payload.plugin_version.as_deref(),
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
                let file = OpenOptions::new().create(true).append(true).open(path)?;
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

/// Where a session's file lands, for callers that want to read one back.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn path_for(dir: &Path, session_id: Option<&str>) -> PathBuf {
    dir.join(format!("{}.jsonl", session_key(session_id)))
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
