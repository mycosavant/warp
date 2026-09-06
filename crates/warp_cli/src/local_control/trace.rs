//! `warpctrl agent trace`: one trace of a conversation, from the record the
//! agent already keeps and the one Warp keeps (board item 6, phase 1).
//!
//! The frame, from `.fork/docs/observability.md`: the harness's own session
//! file is the record of what was said and done, and Warp's event log is a thin
//! index into it that holds only what no harness file has -- what Warp decided,
//! which surface answered, why a request was refused. Measured before this was
//! built: on the ACP path Warp's tool lines carry the ACP kind and *no* input at
//! all, while Claude Code's file has the tool name and the full input under the
//! same `toolu_…` id. So this reads both files, joins them, orders by time,
//! labels every row by who authored it, and writes nothing. It needs no running
//! Warp: the two inputs are files, and nothing is sent anywhere.
//!
//! # The two files
//!
//! Warp's is `<events dir>/<conversation id>.jsonl`, one line per event, as
//! `WARP_FORK_EVENT_LOG` writes it. Its tool and permission lines carry
//! `linked_session_id`, the agent's own session id, which is the join.
//!
//! The harness's is Claude Code's `~/.claude/projects/<slug>/<session id>.jsonl`.
//! The slug is computed from the `cwd` on Warp's lines by [`slug`], never
//! inverted: measured 2026-09-05, `/`, space, `_` and `.` all become `-`, so
//! the mapping is lossy. On Windows the file is inside the distribution the
//! agent ran in, and this process has no session to ask for the native
//! spelling, so `--harness-dir` names it (`\\wsl.localhost\<distro>\home\…\.claude\projects`).
//! The caller owns reachability, the same rule `WARP_FORK_TRANSCRIPT` follows
//! for a path it was handed.
//!
//! # Parsed like a guest
//!
//! Claude Code's format is not versioned for third parties. A handful of
//! fields are read; everything else rides along in `raw`. A line that does not
//! parse becomes a row with `who: harness` and the raw text and never fails the
//! render. The `version` of every line read is collected into the header, so a
//! shape change is visible in the output before it is visible as a bug. One
//! real file per version seen is kept under `.fork/viewer/fixtures/` and the
//! tests parse it.
//!
//! # Rows
//!
//! ```text
//! {"ts": …, "who": "person"|"agent"|"warp"|"harness", "from": "warp"|"harness",
//!  "kind": …, "call_id"?: …, "text"?: …, "error"?: true, "raw": {…}}
//! ```
//!
//! `who` is provenance and it is the whole point of the labelling: Warp's own
//! words (`session_mode`, a permission decision) never read as the agent's,
//! and the harness's bookkeeping (a compaction boundary, a hook summary) never
//! reads as the person's. `from` says which file the row came from, because a
//! prompt appears in both and the reader deserves to see that rather than have
//! it deduplicated away. A compaction is a boundary row followed by the summary
//! the agent continued from; everything before the boundary is still in the
//! file and still in the trace, which is I21's rule met by construction.
//!
//! The first line of output is a header row (`kind: "trace"`) naming both
//! files, the join key, the versions seen, and how many harness lines were
//! bookkeeping kinds this renders as nothing. When Warp's log names no linked
//! session -- Warp's own agent, or a log written before T14.15 -- the trace is
//! Warp's rows alone and the header says so.
//!
//! The text and HTML forms (phase 2) are in `trace_render.rs`; the global
//! `--output-format` picks text (`pretty`, the default) or the JSON rows
//! (`ndjson` for lines, `json` for one object), and `--html FILE` writes the
//! page beside whichever was printed.

use std::collections::BTreeSet;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use local_control::protocol::{ControlError, ErrorCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::agent::OutputFormat;
use crate::local_control::AgentTraceArgs;
use crate::local_control::output::{write_json, write_json_line};

/// Who authored a row. The four provenances the rendering rules require.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Who {
    Person,
    Agent,
    Warp,
    Harness,
}

/// Which file a row was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Record {
    Warp,
    Harness,
}

/// One merged row.
#[derive(Debug, Serialize, Deserialize)]
pub struct Row {
    /// The timestamp as the file spelled it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ts: Option<String>,
    pub who: Who,
    pub from: Record,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// `true` on a tool result the harness marked `is_error`, and on a Warp
    /// `tool_complete` with an `error_type`. A *denial* is not an error: it is
    /// a `permission_replied` row with `decision: denied`, drawn by Warp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<bool>,
    pub raw: Value,
    /// The parsed timestamp, for ordering. Not written: `ts` already is.
    #[serde(skip)]
    pub at: Option<DateTime<Utc>>,
}

/// The first row of every trace.
#[derive(Debug, Serialize, Deserialize)]
pub struct Header {
    pub kind: String,
    pub conversation_id: String,
    pub events_file: PathBuf,
    pub warp_lines: usize,
    /// The agent's own session id, off Warp's lines. `None` means Warp's log
    /// never named one and the trace is Warp's half alone.
    pub linked_session_id: Option<String>,
    pub harness: Option<String>,
    pub harness_file: Option<PathBuf>,
    /// Every distinct `version` seen on the harness's lines. More than one
    /// means the session was resumed under a different Claude Code.
    pub harness_versions: Vec<String>,
    pub harness_lines: usize,
    /// Lines of bookkeeping kinds this renders as nothing.
    pub harness_lines_skipped: usize,
    /// How many non-empty lines of each file the rows *skip*, when a caller
    /// asked for the tail only (`agent.trace` from the console, polling).
    /// `warp_lines` and `harness_lines` then count the whole file, so the
    /// pair is the cursor for the next poll. Zero -- the CLI's case -- is not
    /// written.
    #[serde(skip_serializing_if = "is_zero")]
    pub warp_after: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub harness_after: usize,
    /// Tool calls found in both files under one `toolu_…` id.
    pub joined_calls: usize,
    /// The harness's clock minus Warp's, the median over the joined calls, in
    /// milliseconds. **Disclosed, not applied.** Measured on the phase 0 run:
    /// the harness stamped each `tool_use` about five seconds *after* Warp
    /// stamped the matching `tool_start`, which cannot be latency (the agent
    /// writes its line before it tells Warp) and is the Windows clock against
    /// the distribution's. So the rows' time order is only as good as the two
    /// clocks, and the `call_id` join is the anchor that is actually true.
    /// `None` when no call was joined.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clock_offset_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Trace {
    pub header: Header,
    pub rows: Vec<Row>,
}

pub(super) fn run(args: AgentTraceArgs, output_format: OutputFormat) -> Result<(), ControlError> {
    if args.live {
        return run_live(args, output_format);
    }
    let events_file = match args.events_file {
        Some(path) => path,
        None => events_dir(args.events_dir).join(format!("{}.jsonl", args.conversation)),
    };
    let events = std::fs::read_to_string(&events_file).map_err(|err| {
        ControlError::new(
            ErrorCode::InvalidParams,
            format!(
                "no event log for conversation {} at {}: {err}. WARP_FORK_EVENT_LOG has to have \
                 been on when the conversation ran; --events-dir or --events-file name another \
                 location.",
                args.conversation,
                events_file.display()
            ),
        )
    })?;

    let (warp_rows, facts) = warp_rows(&events);
    let mut harness_file = args.harness_file;
    let mut note = None;
    if harness_file.is_none() {
        match (&facts.linked_session_id, &facts.cwd) {
            (Some(session), Some(cwd)) => {
                harness_file = Some(
                    harness_dir(args.harness_dir)
                        .join(slug(cwd))
                        .join(format!("{session}.jsonl")),
                );
            }
            (Some(_), None) => {
                note = Some(
                    "Warp's log names a linked session but no cwd, so the harness file's directory \
                     cannot be computed; pass --harness-file."
                        .to_owned(),
                );
            }
            (None, _) => {
                note = Some(
                    "Warp's log names no linked session, so there is no harness file to join: \
                     this is Warp's half alone."
                        .to_owned(),
                );
            }
        }
    }
    let harness = match &harness_file {
        Some(path) => Some(std::fs::read_to_string(path).map_err(|err| {
            ControlError::new(
                ErrorCode::InvalidParams,
                format!(
                    "the harness's session file was not readable at {}: {err}. On Windows the \
                     file is inside the distribution the agent ran in; --harness-dir names its \
                     `.claude/projects` (for example \\\\wsl.localhost\\Ubuntu\\home\\<user>\\.claude\\projects), \
                     or --harness-file names the file.",
                    path.display()
                ),
            )
        })?),
        None => None,
    };

    let trace = build(
        args.conversation,
        events_file,
        warp_rows,
        facts,
        harness_file,
        harness.as_deref(),
        note,
    );
    if let Some(path) = &args.html {
        write_private(path, &super::trace_render::render_html(&trace)).map_err(|err| {
            ControlError::new(
                ErrorCode::InvalidParams,
                format!("could not write {}: {err}", path.display()),
            )
        })?;
    }
    match output_format {
        OutputFormat::Json => write_json(&trace),
        OutputFormat::Ndjson => {
            write_json_line(&trace.header)?;
            for row in &trace.rows {
                write_json_line(row)?;
            }
            Ok(())
        }
        OutputFormat::Pretty | OutputFormat::Text => {
            print!("{}", super::trace_render::render_text(&trace));
            if let Some(path) = &args.html {
                println!("wrote {}", path.display());
            }
            Ok(())
        }
    }
}

/// `--live`: the instance runs the merge (`agent.trace`) and this renders
/// what it sends, so the console and the CLI show one record. The instance's
/// reply is the same `Trace` shape with the action envelope's `action`, `ok`
/// and `instance_id` beside it; those are dropped here because the header
/// already names the conversation.
fn run_live(args: AgentTraceArgs, output_format: OutputFormat) -> Result<(), ControlError> {
    let data = super::commands::send_action(
        &args.target,
        local_control::ActionKind::AgentTrace,
        local_control::protocol::AgentTraceParams {
            conversation_id: args.conversation,
            warp_after: 0,
            harness_after: 0,
        },
    )?;
    let trace: Trace = serde_json::from_value(data).map_err(|err| {
        ControlError::with_details(
            ErrorCode::Internal,
            "the instance's agent.trace reply did not parse as a trace",
            err.to_string(),
        )
    })?;
    if let Some(path) = &args.html {
        write_private(path, &super::trace_render::render_html(&trace)).map_err(|err| {
            ControlError::new(
                ErrorCode::InvalidParams,
                format!("could not write {}: {err}", path.display()),
            )
        })?;
    }
    match output_format {
        OutputFormat::Json => write_json(&trace),
        OutputFormat::Ndjson => {
            write_json_line(&trace.header)?;
            for row in &trace.rows {
                write_json_line(row)?;
            }
            Ok(())
        }
        OutputFormat::Pretty | OutputFormat::Text => {
            print!("{}", super::trace_render::render_text(&trace));
            if let Some(path) = &args.html {
                println!("wrote {}", path.display());
            }
            Ok(())
        }
    }
}

/// Writes the page owner-only from the first byte, the way the fork's
/// transcript and event log are written: the mode goes on the `open`, not on
/// a `chmod` after it, because the window between the two is exactly when the
/// first line lands.
fn write_private(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    options.open(path)?.write_all(contents.as_bytes())
}

/// Everything after the files are in hand, split out so a test can run the
/// whole merge on fixture text without a filesystem.
pub fn build(
    conversation_id: String,
    events_file: PathBuf,
    warp_rows: Vec<Row>,
    facts: WarpFacts,
    harness_file: Option<PathBuf>,
    harness: Option<&str>,
    note: Option<String>,
) -> Trace {
    let warp_lines = warp_rows.len();
    let (harness_rows, harness_facts) = match harness {
        Some(text) => harness_rows(text),
        None => (Vec::new(), HarnessFacts::default()),
    };
    let (joined_calls, clock_offset_ms) = clock_offset(&warp_rows, &harness_rows);
    let rows = merge(warp_rows, harness_rows);
    Trace {
        header: Header {
            kind: "trace".to_owned(),
            conversation_id,
            events_file,
            warp_lines,
            linked_session_id: facts.linked_session_id,
            harness: harness.map(|_| "claude-code".to_owned()),
            harness_file,
            harness_versions: harness_facts.versions.into_iter().collect(),
            harness_lines: harness_facts.lines,
            harness_lines_skipped: harness_facts.skipped,
            joined_calls,
            clock_offset_ms,
            warp_after: 0,
            harness_after: 0,
            note,
        },
        rows,
    }
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

impl Trace {
    /// Marks a trace built from the tails of both files as such: the rows are
    /// what came after `warp_after` and `harness_after` non-empty lines, and
    /// the line counts become totals over the whole file.
    pub fn after(mut self, warp_after: usize, harness_after: usize) -> Self {
        self.header.warp_after = warp_after;
        self.header.harness_after = harness_after;
        self.header.warp_lines += warp_after;
        self.header.harness_lines += harness_after;
        self
    }
}

/// The text after its first `after` non-empty lines. Both files are
/// append-only and both parsers count non-empty lines, so a count is an exact
/// cursor into either.
pub fn tail(text: &str, after: usize) -> String {
    let mut out = String::new();
    for line in text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .skip(after)
    {
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Where Warp's log is, by the same rule the app uses for `WARP_FORK_EVENT_LOG`.
///
/// Restated here rather than imported because the CLI does not depend on the
/// app crate: `on`/`true` means the state directory's `fork/events`, anything
/// else is a directory the caller named, and off means the default too (the
/// log may have been on when the conversation ran and off now).
fn events_dir(flag: Option<PathBuf>) -> PathBuf {
    if let Some(dir) = flag {
        return dir;
    }
    match std::env::var("WARP_FORK_EVENT_LOG")
        .ok()
        .as_deref()
        .map(str::trim)
    {
        None | Some("") | Some("on") | Some("true") | Some("0") | Some("off") | Some("false") => {
            warp_core::paths::secure_state_dir()
                .unwrap_or_else(warp_core::paths::state_dir)
                .join("fork")
                .join("events")
        }
        Some(path) => PathBuf::from(path),
    }
}

fn harness_dir(flag: Option<PathBuf>) -> PathBuf {
    flag.unwrap_or_else(|| {
        std::env::home_dir()
            .unwrap_or_default()
            .join(".claude")
            .join("projects")
    })
}

/// Claude Code's project directory name for a working directory.
///
/// Measured 2026-09-05 (`.fork/runs/viewer-phase0-2026-09-05/`): every
/// character that is not an ASCII letter or digit becomes `-`, case is kept.
/// `/home/effatha/git/warp` is `-home-effatha-git-warp`; a directory named
/// `slug probe/a_b.c` ends `-slug-probe-a-b-c`. Non-ASCII letters were not
/// tried, so they are mapped like punctuation here and that is a guess.
pub fn slug(cwd: &str) -> String {
    cwd.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// What Warp's lines say about where the other file is.
#[derive(Debug, Default)]
pub struct WarpFacts {
    pub linked_session_id: Option<String>,
    pub cwd: Option<String>,
}

/// One row per line of Warp's event log.
pub fn warp_rows(text: &str) -> (Vec<Row>, WarpFacts) {
    let mut rows = Vec::new();
    let mut facts = WarpFacts::default();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let Ok(raw) = serde_json::from_str::<Value>(line) else {
            rows.push(unparsed(Record::Warp, line));
            continue;
        };
        let event = string(&raw, "event").unwrap_or_default();
        if facts.linked_session_id.is_none() {
            facts.linked_session_id = string(&raw, "linked_session_id");
        }
        if facts.cwd.is_none() {
            facts.cwd = string(&raw, "cwd");
        }
        let (who, text, error) = match event.as_str() {
            "session_start" | "prompt_submit" => (Who::Person, string(&raw, "summary"), None),
            "tool_start" => (
                Who::Agent,
                Some(
                    [
                        string(&raw, "tool_name"),
                        string(&raw, "tool_input_preview"),
                    ]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" "),
                ),
                None,
            ),
            "tool_complete" => {
                let error_type = string(&raw, "error_type");
                (Who::Agent, error_type.clone(), error_type.map(|_| true))
            }
            "permission_request" => (Who::Warp, string(&raw, "tool_input_preview"), None),
            "permission_replied" => (
                Who::Warp,
                Some(
                    [string(&raw, "decision"), string(&raw, "answered_by")]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                        .join(" by "),
                ),
                None,
            ),
            _ => (
                Who::Warp,
                string(&raw, "summary").or_else(|| string(&raw, "error_type")),
                None,
            ),
        };
        let ts = string(&raw, "ts");
        rows.push(Row {
            at: ts.as_deref().and_then(parse_ts),
            ts,
            who,
            from: Record::Warp,
            kind: event,
            call_id: string(&raw, "call_id"),
            text: text.filter(|t| !t.is_empty()),
            error,
            raw,
        });
    }
    (rows, facts)
}

#[derive(Debug, Default)]
pub struct HarnessFacts {
    pub versions: BTreeSet<String>,
    pub lines: usize,
    pub skipped: usize,
}

/// Rows from Claude Code's session file.
///
/// `user` and `assistant` lines become one row per content block; `system`
/// lines become one row each; `permission-mode` is kept because it is the one
/// bookkeeping kind that says something about consent. Everything else is
/// counted and dropped. The usage footer is the last row.
pub fn harness_rows(text: &str) -> (Vec<Row>, HarnessFacts) {
    let mut rows = Vec::new();
    let mut facts = HarnessFacts::default();
    let mut usage = Usage::default();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        facts.lines += 1;
        let Ok(raw) = serde_json::from_str::<Value>(line) else {
            rows.push(unparsed(Record::Harness, line));
            continue;
        };
        if let Some(version) = string(&raw, "version") {
            facts.versions.insert(version);
        }
        let ts = string(&raw, "timestamp");
        let at = ts.as_deref().and_then(parse_ts);
        let row =
            |who: Who, kind: &str, call_id: Option<String>, text: Option<String>, error| Row {
                ts: ts.clone(),
                at,
                who,
                from: Record::Harness,
                kind: kind.to_owned(),
                call_id,
                text,
                error,
                raw: raw.clone(),
            };
        match string(&raw, "type").as_deref() {
            Some("user") => {
                if raw.get("isCompactSummary").and_then(Value::as_bool) == Some(true) {
                    rows.push(row(
                        Who::Harness,
                        "compact_summary",
                        None,
                        message_text(&raw),
                        None,
                    ));
                    continue;
                }
                let mut any = false;
                for block in blocks(&raw) {
                    match string(block, "type").as_deref() {
                        Some("tool_result") => {
                            any = true;
                            let is_error = block.get("is_error").and_then(Value::as_bool);
                            rows.push(row(
                                Who::Agent,
                                "tool_result",
                                string(block, "tool_use_id"),
                                content_text(block.get("content")),
                                is_error.filter(|e| *e),
                            ));
                        }
                        Some("text") => {
                            any = true;
                            rows.push(row(
                                Who::Person,
                                "prompt",
                                None,
                                string(block, "text"),
                                None,
                            ));
                        }
                        _ => {}
                    }
                }
                if !any {
                    // A string `content` is the person's prompt in older files.
                    rows.push(row(Who::Person, "prompt", None, message_text(&raw), None));
                }
            }
            Some("assistant") => {
                usage.add(&raw);
                usage.last_ts = ts.clone();
                usage.last_at = at;
                for block in blocks(&raw) {
                    match string(block, "type").as_deref() {
                        Some("text") => {
                            rows.push(row(Who::Agent, "text", None, string(block, "text"), None));
                        }
                        Some("thinking") => rows.push(row(
                            Who::Agent,
                            "thinking",
                            None,
                            string(block, "thinking"),
                            None,
                        )),
                        Some("tool_use") => {
                            let name = string(block, "name").unwrap_or_default();
                            let input = block
                                .get("input")
                                .map(|input| serde_json::to_string(input).unwrap_or_default())
                                .unwrap_or_default();
                            rows.push(row(
                                Who::Agent,
                                "tool_use",
                                string(block, "id"),
                                Some(format!("{name} {input}").trim().to_owned()),
                                None,
                            ));
                        }
                        _ => {}
                    }
                }
            }
            Some("system") => {
                let subtype = string(&raw, "subtype").unwrap_or_default();
                let kind = if subtype.is_empty() {
                    "system".to_owned()
                } else {
                    format!("system/{subtype}")
                };
                rows.push(row(
                    Who::Harness,
                    &kind,
                    None,
                    string(&raw, "content"),
                    None,
                ));
            }
            Some("permission-mode") => rows.push(row(
                Who::Harness,
                "permission_mode",
                None,
                string(&raw, "permissionMode"),
                None,
            )),
            _ => facts.skipped += 1,
        }
    }
    if usage.messages > 0 {
        rows.push(Row {
            ts: usage.last_ts.clone(),
            at: usage.last_at,
            who: Who::Harness,
            from: Record::Harness,
            kind: "usage".to_owned(),
            call_id: None,
            text: Some(format!(
                "{} assistant messages, {} input, {} output, {} cache read, {} cache created",
                usage.messages, usage.input, usage.output, usage.cache_read, usage.cache_creation
            )),
            error: None,
            raw: serde_json::json!({
                "assistant_messages": usage.messages,
                "input_tokens": usage.input,
                "output_tokens": usage.output,
                "cache_read_input_tokens": usage.cache_read,
                "cache_creation_input_tokens": usage.cache_creation,
            }),
        });
    }
    (rows, facts)
}

/// The footer: `message.usage` summed over the assistant's lines.
#[derive(Debug, Default)]
struct Usage {
    messages: u64,
    input: u64,
    output: u64,
    cache_read: u64,
    cache_creation: u64,
    last_ts: Option<String>,
    last_at: Option<DateTime<Utc>>,
}

impl Usage {
    fn add(&mut self, line: &Value) {
        let Some(usage) = line.pointer("/message/usage") else {
            return;
        };
        let n = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0);
        self.messages += 1;
        self.input += n("input_tokens");
        self.output += n("output_tokens");
        self.cache_read += n("cache_read_input_tokens");
        self.cache_creation += n("cache_creation_input_tokens");
    }
}

/// How many calls both files hold, and the median of (harness `tool_use` time
/// minus Warp `tool_start` time) over them. See [`Header::clock_offset_ms`].
fn clock_offset(warp: &[Row], harness: &[Row]) -> (usize, Option<i64>) {
    let starts: std::collections::HashMap<&str, DateTime<Utc>> = warp
        .iter()
        .filter(|row| row.kind == "tool_start")
        .filter_map(|row| Some((row.call_id.as_deref()?, row.at?)))
        .collect();
    let mut offsets: Vec<i64> = harness
        .iter()
        .filter(|row| row.kind == "tool_use")
        .filter_map(|row| {
            let started = starts.get(row.call_id.as_deref()?)?;
            Some((row.at? - *started).num_milliseconds())
        })
        .collect();
    offsets.sort_unstable();
    let median = match offsets.len() {
        0 => None,
        n if n % 2 == 1 => Some(offsets[n / 2]),
        n => Some((offsets[n / 2 - 1] + offsets[n / 2]) / 2),
    };
    (offsets.len(), median)
}

/// Stable by timestamp: Warp's row first when two share one, and rows without
/// a timestamp keep their file order at the end.
pub fn merge(warp: Vec<Row>, harness: Vec<Row>) -> Vec<Row> {
    let mut rows: Vec<Row> = warp.into_iter().chain(harness).collect();
    rows.sort_by_key(|row| match row.at {
        Some(at) => (0, at),
        None => (1, DateTime::<Utc>::MIN_UTC),
    });
    rows
}

fn unparsed(from: Record, line: &str) -> Row {
    Row {
        ts: None,
        at: None,
        who: match from {
            Record::Warp => Who::Warp,
            Record::Harness => Who::Harness,
        },
        from,
        kind: "unparsed".to_owned(),
        call_id: None,
        text: Some(line.to_owned()),
        error: None,
        raw: Value::String(line.to_owned()),
    }
}

fn parse_ts(ts: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(ts)
        .ok()
        .map(|at| at.with_timezone(&Utc))
}

fn string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn blocks(line: &Value) -> impl Iterator<Item = &Value> {
    line.pointer("/message/content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

/// The text of a message whose `content` is a string or an array of blocks.
fn message_text(line: &Value) -> Option<String> {
    content_text(line.pointer("/message/content"))
}

fn content_text(content: Option<&Value>) -> Option<String> {
    match content {
        Some(Value::String(text)) => Some(text.clone()),
        Some(Value::Array(items)) => {
            let parts: Vec<String> = items
                .iter()
                .filter_map(|item| match item {
                    Value::String(text) => Some(text.clone()),
                    other => string(other, "text"),
                })
                .collect();
            (!parts.is_empty()).then(|| parts.join("\n"))
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "trace_tests.rs"]
mod tests;
