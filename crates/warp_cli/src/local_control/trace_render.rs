//! The viewer's two renderings of a [`Trace`]: text for a terminal and a
//! self-contained HTML page (board item 6, phase 2).
//!
//! Both draw from the rows `trace.rs` merged, and both apply the same rules,
//! from `.fork/docs/observability.md`:
//!
//! * **Provenance first.** A one-character gutter says who wrote each line:
//!   `P` the person, `A` the agent, `W` Warp, `H` the harness itself. Warp's
//!   words never sit in the agent's column.
//! * **A tool call is one item.** The `tool_use` from the harness (the name
//!   and the full input), the `tool_result` that answered it, and whatever
//!   Warp logged under the same `toolu_…` id (`tool_start`, `tool_complete`,
//!   a permission request and its answer) are drawn together, once, at the
//!   call's earliest stamp. The item carries *both* files' timestamps, because
//!   the two clocks disagree (phase 1 measured 5.5 s on Windows) and the join
//!   is what makes the call one thing, not the clocks. `is_error` from the
//!   harness and `denied` from Warp are different states and get different
//!   marks.
//! * **A compaction is a boundary**, drawn as a rule, with the summary the
//!   agent continued from folded beneath it. Everything above the rule is still
//!   there.
//! * **Thinking is shown if present**, marked as such.
//! * **Usage is a footer.**
//!
//! One deliberate departure from the JSON rows: the person's prompt appears in
//! both files, and the JSON keeps both. Here the harness's copy is dropped
//! when Warp's `session_start`/`prompt_submit` carries the same text, because
//! a transcript that says everything twice is one nobody reads. Warp's copy is
//! the one kept because it is the one in the right place: the first live
//! render dropped it instead, and the prompt then appeared *after* both tool
//! calls, seven seconds down the page on the harness's skewed clock. When the
//! texts differ (Warp's `summary` is an excerpt of a long prompt) both stay,
//! and the reader sees why.
//!
//! The HTML is a file, not a route. It loads nothing: no script, no font, no
//! stylesheet beyond its own `<style>`, per the maintainer's local-first rule
//! for `.fork/board.html`. Every piece of text passes through [`esc`] at
//! generation time, so the page takes on none of the console's DOM-sink
//! discipline -- there is no DOM code to discipline. The one interaction it
//! has is `<details>`, which the browser owns.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use chrono::{DateTime, Utc};
use serde_json::Value;

use super::trace::{Record, Row, Trace, Who};

/// The gutter character for a provenance.
fn gutter(who: Who) -> char {
    match who {
        Who::Person => 'P',
        Who::Agent => 'A',
        Who::Warp => 'W',
        Who::Harness => 'H',
    }
}

fn who_name(who: Who) -> &'static str {
    match who {
        Who::Person => "person",
        Who::Agent => "agent",
        Who::Warp => "warp",
        Who::Harness => "harness",
    }
}

/// `HH:MM:SS.mmm`, UTC, or blanks for a row with no time.
fn clock(at: Option<DateTime<Utc>>) -> String {
    match at {
        Some(at) => at.format("%H:%M:%S%.3f").to_string(),
        None => " ".repeat(12),
    }
}

/// What one tool call looks like once both files have been read.
#[derive(Debug, Default)]
struct Call<'a> {
    id: &'a str,
    /// The harness's `tool_use`: the tool's real name and its full input.
    used: Option<&'a Row>,
    result: Option<&'a Row>,
    started: Option<&'a Row>,
    completed: Option<&'a Row>,
    asked: Option<&'a Row>,
    replied: Option<&'a Row>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Started and never answered by either file.
    Open,
    Done,
    Failed,
    Denied,
    /// Warp asked and the answer never came: what a cancelled turn leaves.
    Unanswered,
}

impl<'a> Call<'a> {
    fn state(&self) -> State {
        if self
            .replied
            .and_then(|r| r.raw.get("decision"))
            .and_then(Value::as_str)
            == Some("denied")
        {
            return State::Denied;
        }
        if self.asked.is_some() && self.replied.is_none() && self.result.is_none() {
            return State::Unanswered;
        }
        if self.result.and_then(|r| r.error) == Some(true)
            || self.completed.and_then(|r| r.error) == Some(true)
        {
            return State::Failed;
        }
        if self.result.is_some() || self.completed.is_some() {
            return State::Done;
        }
        State::Open
    }

    /// The tool's name: the harness's if it wrote one, else Warp's kind.
    fn name(&self) -> String {
        self.used
            .and_then(|row| tool_use_block(row, self.id))
            .and_then(|block| block.get("name").and_then(Value::as_str))
            .map(str::to_owned)
            .or_else(|| {
                self.started
                    .or(self.asked)
                    .and_then(|row| row.raw.get("tool_name").and_then(Value::as_str))
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| "tool".to_owned())
    }

    /// The input, as the harness recorded it: a shell command as itself,
    /// anything else as compact JSON. Warp's preview when the harness has none.
    fn input(&self) -> Option<String> {
        if let Some(block) = self.used.and_then(|row| tool_use_block(row, self.id)) {
            let input = block.get("input")?;
            if let Some(command) = input.get("command").and_then(Value::as_str) {
                return Some(command.to_owned());
            }
            // An edit is drawn as a diff by `edit`; its input line is the file.
            if self.edit().is_some() {
                return input
                    .get("file_path")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
            }
            return serde_json::to_string(input).ok();
        }
        self.started
            .or(self.asked)
            .and_then(|row| row.raw.get("tool_input_preview").and_then(Value::as_str))
            .map(str::to_owned)
    }

    /// An edit tool's input as hunks (2026-09-06): Claude Code's `Edit`
    /// carries `old_string`/`new_string`, `MultiEdit` a list of them, `Write`
    /// the whole new content. Two strings side by side are a diff drawn
    /// badly; this draws it as one. Only from the harness's own `tool_use`
    /// block, since the ACP path writes no input to Warp's log.
    fn edit(&self) -> Option<Edit> {
        let input = self
            .used
            .and_then(|row| tool_use_block(row, self.id))?
            .get("input")?;
        let string =
            |value: &Value, key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
        let mut hunks = Vec::new();
        if let (Some(old), Some(new)) = (string(input, "old_string"), string(input, "new_string")) {
            hunks.push((old, new));
        } else if let Some(edits) = input.get("edits").and_then(Value::as_array) {
            for edit in edits {
                if let (Some(old), Some(new)) =
                    (string(edit, "old_string"), string(edit, "new_string"))
                {
                    hunks.push((old, new));
                }
            }
        } else if let (Some(content), Some(_)) =
            (string(input, "content"), string(input, "file_path"))
        {
            hunks.push((String::new(), content));
        }
        if hunks.is_empty() {
            return None;
        }
        Some(Edit {
            file: string(input, "file_path").unwrap_or_default(),
            hunks,
        })
    }

    /// The call's earliest stamp, which is where it is drawn.
    fn first(&self) -> Option<DateTime<Utc>> {
        [
            self.used,
            self.result,
            self.started,
            self.completed,
            self.asked,
            self.replied,
        ]
        .into_iter()
        .flatten()
        .filter_map(|row| row.at)
        .min()
    }

    fn who(&self) -> Who {
        Who::Agent
    }
}

/// The `tool_use` block with this id inside an assistant line.
/// An edit's input: the file and its (old, new) pairs.
struct Edit {
    file: String,
    hunks: Vec<(String, String)>,
}

/// One diff line: the mark, and the line without its newline.
enum DiffLine {
    Removed(String),
    Added(String),
    Kept(String),
}

impl Edit {
    /// Line by line, in order, hunks separated by `None`.
    fn lines(&self) -> Vec<Option<DiffLine>> {
        let mut out = Vec::new();
        for (index, (old, new)) in self.hunks.iter().enumerate() {
            if index > 0 {
                out.push(None);
            }
            let diff = similar::TextDiff::from_lines(old.as_str(), new.as_str());
            for change in diff.iter_all_changes() {
                let line = change.value().trim_end_matches('\n').to_owned();
                out.push(Some(match change.tag() {
                    similar::ChangeTag::Delete => DiffLine::Removed(line),
                    similar::ChangeTag::Insert => DiffLine::Added(line),
                    similar::ChangeTag::Equal => DiffLine::Kept(line),
                }));
            }
        }
        out
    }

    /// The text form: `-`/`+`/space marks, one line each.
    fn text(&self) -> String {
        let mut out = String::new();
        for line in self.lines() {
            let _ = match line {
                None => writeln!(out, "  …"),
                Some(DiffLine::Removed(line)) => writeln!(out, "- {line}"),
                Some(DiffLine::Added(line)) => writeln!(out, "+ {line}"),
                Some(DiffLine::Kept(line)) => writeln!(out, "  {line}"),
            };
        }
        out
    }

    /// The HTML form: a `<pre class="diff">` of spans, everything escaped.
    fn html(&self) -> String {
        let mut out = String::from("<pre class=\"diff\">");
        if !self.file.is_empty() {
            let _ = write!(out, "<span class=\"file\">{}</span>\n", esc(&self.file));
        }
        for line in self.lines() {
            let _ = match line {
                None => write!(out, "<span class=\"file\">…</span>\n"),
                Some(DiffLine::Removed(line)) => {
                    write!(out, "<span class=\"del\">- {}</span>\n", esc(&line))
                }
                Some(DiffLine::Added(line)) => {
                    write!(out, "<span class=\"add\">+ {}</span>\n", esc(&line))
                }
                Some(DiffLine::Kept(line)) => {
                    write!(out, "<span class=\"ctx\">  {}</span>\n", esc(&line))
                }
            };
        }
        out.push_str("</pre>");
        out
    }
}

fn tool_use_block<'a>(row: &'a Row, id: &str) -> Option<&'a Value> {
    row.raw
        .pointer("/message/content")?
        .as_array()?
        .iter()
        .find(|block| block.get("id").and_then(Value::as_str) == Some(id))
}

/// The rows, with each call folded into one item at its first appearance and
/// Warp's duplicate of the person's prompt dropped.
enum Item<'a> {
    Row(&'a Row),
    Call(Call<'a>),
}

fn items(trace: &Trace) -> Vec<Item<'_>> {
    let mut calls: HashMap<&str, Call<'_>> = HashMap::new();
    for row in &trace.rows {
        let Some(id) = row.call_id.as_deref() else {
            continue;
        };
        let call = calls.entry(id).or_insert_with(|| Call {
            id,
            ..Call::default()
        });
        let slot = match (row.from, row.kind.as_str()) {
            (Record::Harness, "tool_use") => &mut call.used,
            (Record::Harness, "tool_result") => &mut call.result,
            (Record::Warp, "tool_start") => &mut call.started,
            (Record::Warp, "tool_complete") => &mut call.completed,
            (Record::Warp, "permission_request") => &mut call.asked,
            (Record::Warp, "permission_replied") => &mut call.replied,
            _ => continue,
        };
        if slot.is_none() {
            *slot = Some(row);
        }
    }
    let warp_prompts: HashSet<&str> = trace
        .rows
        .iter()
        .filter(|row| {
            row.from == Record::Warp
                && matches!(row.kind.as_str(), "session_start" | "prompt_submit")
        })
        .filter_map(|row| row.text.as_deref())
        .collect();

    let mut drawn: HashSet<&str> = HashSet::new();
    let mut out = Vec::new();
    for row in &trace.rows {
        if let Some(id) = row.call_id.as_deref() {
            if drawn.insert(id)
                && let Some(call) = calls.remove(id)
            {
                out.push(Item::Call(call));
            }
            continue;
        }
        if row.from == Record::Harness
            && row.kind == "prompt"
            && row
                .text
                .as_deref()
                .is_some_and(|text| warp_prompts.contains(text))
        {
            continue;
        }
        out.push(Item::Row(row));
    }
    out
}

/// What a non-call row says, in words a reader wants: Warp's frame lines get
/// a label, the harness's bookkeeping gets its subtype, the rest is the text.
fn caption(row: &Row) -> (String, Option<&str>) {
    let text = row.text.as_deref();
    match (row.from, row.kind.as_str()) {
        (Record::Warp, "session_agent") => ("agent".to_owned(), text),
        (Record::Warp, "session_mode") => ("mode".to_owned(), text),
        (Record::Warp, "session_model") => ("model".to_owned(), text),
        // A prompt from a phone (T19) is labelled so; the words are the
        // person's own either way.
        (Record::Warp, "session_start" | "prompt_submit") => (
            via_label(&row.raw).map(str::to_owned).unwrap_or_default(),
            text,
        ),
        // `stop` repeats the prompt as its summary; the state is what matters.
        (Record::Warp, "stop") => (
            "stop".to_owned(),
            row.raw.get("error_type").and_then(Value::as_str),
        ),
        (Record::Warp, kind) if kind.starts_with("stop") => (kind.to_owned(), None),
        (Record::Harness, "prompt" | "text") => (String::new(), text),
        (Record::Harness, "thinking") => ("thinking".to_owned(), text),
        (Record::Harness, "permission_mode") => ("permission mode".to_owned(), text),
        (Record::Harness, "compact_summary") => {
            ("summary the agent continued from".to_owned(), text)
        }
        (Record::Harness, "usage") => ("usage".to_owned(), text),
        (_, "unparsed") => ("unparsed line".to_owned(), text),
        (_, kind) => (kind.to_owned(), text),
    }
}

/// The compaction rule's words, from `compactMetadata`.
fn compaction_caption(row: &Row) -> String {
    let meta = row.raw.get("compactMetadata");
    let field = |key: &str| meta.and_then(|m| m.get(key));
    let mut parts = vec!["compaction".to_owned()];
    if let Some(trigger) = field("trigger").and_then(Value::as_str) {
        parts.push(trigger.to_owned());
    }
    if let Some(tokens) = field("preTokens").and_then(Value::as_u64) {
        parts.push(format!("{tokens} tokens before"));
    }
    if let Some(ms) = field("durationMs").and_then(Value::as_u64) {
        parts.push(format!("{:.1} s", ms as f64 / 1000.0));
    }
    parts.join(", ")
}

fn state_word(state: State) -> &'static str {
    match state {
        State::Open => "no result recorded",
        State::Done => "done",
        State::Failed => "failed",
        State::Denied => "denied",
        State::Unanswered => "asked, never answered",
    }
}

fn state_mark(state: State) -> char {
    match state {
        State::Open => '…',
        State::Done => '✓',
        State::Failed => '✗',
        State::Denied => '⊘',
        State::Unanswered => '?',
    }
}

/// Warp's decision on a call, in Warp's words, or nothing.
///
/// `via: paired_device` on the line (T19) becomes "from the phone": the door
/// was the control plane either way, and this is the record saying who was
/// at it.
fn decision(call: &Call<'_>) -> Option<String> {
    let replied = call.replied?;
    let decision = replied.raw.get("decision").and_then(Value::as_str)?;
    let by = replied.raw.get("answered_by").and_then(Value::as_str);
    let mut text = match by {
        Some(by) => format!("Warp: {decision} by {by}"),
        None => format!("Warp: {decision}"),
    };
    if let Some(via) = via_label(&replied.raw) {
        text.push(' ');
        text.push_str(via);
    }
    Some(text)
}

/// The stamp for a line that came from a paired phone, or nothing.
pub(crate) fn via_label(raw: &Value) -> Option<&'static str> {
    match raw.get("via").and_then(Value::as_str)? {
        "paired_device" => Some("from the phone"),
        _ => Some("from another device"),
    }
}

/// The stamps from each file, so a reader sees the two clocks side by side.
fn stamps(call: &Call<'_>) -> String {
    let span = |open: Option<&Row>, close: Option<&Row>| -> Option<String> {
        let open = open?.at?;
        Some(match close.and_then(|row| row.at) {
            Some(close) => format!("{} → {}", clock(Some(open)), clock(Some(close))),
            None => clock(Some(open)),
        })
    };
    let mut parts = Vec::new();
    if let Some(s) = span(call.started, call.completed) {
        parts.push(format!("warp {s}"));
    }
    if let Some(s) = span(call.used, call.result) {
        parts.push(format!("harness {s}"));
    }
    parts.join(" · ")
}

// ---------------------------------------------------------------- text

/// Lines of a block of text after the first, indented under the gutter.
const INDENT: &str = "                ";

/// How many lines of a tool result the text form shows before folding.
const RESULT_LINES: usize = 3;

fn push_block(out: &mut String, at: Option<DateTime<Utc>>, who: Who, text: &str) {
    let mut lines = text.lines();
    let first = lines.next().unwrap_or("");
    let _ = writeln!(out, "{} {}  {first}", clock(at), gutter(who));
    for line in lines {
        let _ = writeln!(out, "{INDENT}{line}");
    }
}

fn folded(text: &str, keep: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= keep {
        return text.to_owned();
    }
    let mut shown = lines[..keep].join("\n");
    let _ = write!(shown, "\n(+{} lines)", lines.len() - keep);
    shown
}

/// The trace as text for a terminal.
pub(super) fn render_text(trace: &Trace) -> String {
    let h = &trace.header;
    let mut out = String::new();
    let _ = write!(
        out,
        "trace {} · warp {} lines",
        h.conversation_id, h.warp_lines
    );
    if let Some(harness) = &h.harness {
        let _ = write!(
            out,
            " · {harness} {}, {} lines ({} bookkeeping)",
            h.harness_versions.join("/"),
            h.harness_lines,
            h.harness_lines_skipped
        );
    }
    if let Some(offset) = h.clock_offset_ms {
        let _ = write!(
            out,
            " · {} calls joined · harness clock {offset:+} ms",
            h.joined_calls
        );
    }
    out.push('\n');
    let _ = writeln!(out, "  events   {}", h.events_file.display());
    if let Some(file) = &h.harness_file {
        let _ = writeln!(out, "  harness  {}", file.display());
    }
    if let Some(note) = &h.note {
        let _ = writeln!(out, "  note     {note}");
    }
    out.push('\n');

    for item in items(trace) {
        match item {
            Item::Row(row) if row.kind == "system/compact_boundary" => {
                let _ = writeln!(
                    out,
                    "{} {}  ──── {} ────",
                    clock(row.at),
                    gutter(row.who),
                    compaction_caption(row)
                );
            }
            Item::Row(row) if row.kind == "compact_summary" => {
                let (label, text) = caption(row);
                let body = format!("{label}:\n{}", folded(text.unwrap_or(""), 1));
                push_block(&mut out, row.at, row.who, &body);
            }
            Item::Row(row) => {
                let (label, text) = caption(row);
                let body = match (label.is_empty(), text) {
                    (true, Some(text)) => text.to_owned(),
                    (true, None) => row.kind.clone(),
                    (false, Some(text)) => format!("{label}: {text}"),
                    (false, None) => label,
                };
                push_block(&mut out, row.at, row.who, &body);
            }
            Item::Call(call) => {
                let state = call.state();
                let mut body = format!("▸ {}", call.name());
                if let Some(input) = call.input() {
                    let _ = write!(body, " {}", folded(&input, RESULT_LINES));
                }
                if let Some(edit) = call.edit() {
                    let _ = write!(body, "\n{}", edit.text().trim_end_matches('\n'));
                }
                let _ = write!(body, "\n{} {}", state_mark(state), state_word(state));
                if let Some(decision) = decision(&call) {
                    let _ = write!(body, " · {decision}");
                }
                let stamps = stamps(&call);
                if !stamps.is_empty() {
                    let _ = write!(body, " · {stamps}");
                }
                if let Some(result) = call.result.and_then(|row| row.text.as_deref()) {
                    let _ = write!(body, "\n{}", folded(result, RESULT_LINES));
                }
                push_block(&mut out, call.first(), call.who(), &body);
            }
        }
    }
    out
}

// ---------------------------------------------------------------- html

/// Escapes text for an HTML text node or a quoted attribute.
pub(super) fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// Lines of a result above which the HTML folds it.
const HTML_FOLD_LINES: usize = 6;

fn html_result(text: &str) -> String {
    let lines = text.lines().count();
    if lines > HTML_FOLD_LINES {
        format!(
            "<details><summary>{} lines</summary><pre>{}</pre></details>",
            lines,
            esc(text)
        )
    } else {
        format!("<pre>{}</pre>", esc(text))
    }
}

const STYLE: &str = r#"
:root{color-scheme:light dark;--bg:#fbfaf7;--fg:#1d1c1a;--dim:#6f6a62;--rule:#d9d4cb;
--p:#2b5fb4;--a:#7a4ea8;--w:#b3651a;--h:#5d7a4a;--ok:#3a7d44;--bad:#b23a3a;--den:#8a5a00;--mono:ui-monospace,Menlo,Consolas,monospace}
@media(prefers-color-scheme:dark){:root{--bg:#161513;--fg:#e8e4dc;--dim:#9a948a;--rule:#3a3733;--p:#7ea6ea;--a:#c2a1e6;--w:#e6a36b;--h:#a3c28b;--ok:#7dc48a;--bad:#e07a7a;--den:#e0b25a}}
html{background:var(--bg);color:var(--fg)}body{margin:0;font:14px/1.45 system-ui,sans-serif;max-width:64rem;padding:1.5rem}
header h1{font-size:1.1rem;margin:0 0 .5rem}header dl{display:grid;grid-template-columns:max-content 1fr;gap:.15rem 1rem;margin:0;color:var(--dim);font-size:.85rem}header dt{font-weight:600}header dd{margin:0;word-break:break-all}
main{margin-top:1.5rem}
.row{display:grid;grid-template-columns:6.5rem 1.4rem 1fr;gap:.5rem;padding:.3rem 0;border-top:1px solid var(--rule);align-items:baseline}
.row time{font:.8rem var(--mono);color:var(--dim)}.g{font:700 .8rem var(--mono);text-align:center;border-radius:3px;color:#fff}
.who-person .g{background:var(--p)}.who-agent .g{background:var(--a)}.who-warp .g{background:var(--w)}.who-harness .g{background:var(--h)}
.t{white-space:pre-wrap;word-break:break-word;min-width:0}.label{color:var(--dim);font-size:.85rem}.thinking .t{font-style:italic;color:var(--dim)}
.call .head b{font-family:var(--mono)}.call code{font:.85rem var(--mono);background:color-mix(in srgb,var(--fg) 7%,transparent);padding:0 .25rem;border-radius:3px}
.badge{font:.75rem var(--mono);padding:0 .35rem;border-radius:3px;border:1px solid currentColor;margin-left:.4rem}
.state-done .badge{color:var(--ok)}.state-failed .badge{color:var(--bad)}.state-denied .badge{color:var(--den)}.state-unanswered .badge,.state-open .badge{color:var(--dim)}
.stamps{font:.75rem var(--mono);color:var(--dim);margin:.15rem 0}
.diff .del{color:var(--bad)}.diff .add{color:var(--ok)}.diff .ctx,.diff .file{color:var(--dim)}
pre{margin:.3rem 0 0;font:.8rem var(--mono);white-space:pre-wrap;word-break:break-word;background:color-mix(in srgb,var(--fg) 5%,transparent);padding:.4rem .6rem;border-radius:4px;max-height:24rem;overflow:auto}
details summary{cursor:pointer;color:var(--dim);font-size:.85rem}
.boundary{border-top:2px dashed var(--h);margin:1rem 0 .5rem;padding-top:.3rem;color:var(--h);font-size:.85rem;text-align:center}
footer{margin-top:1.5rem;padding-top:.5rem;border-top:1px solid var(--rule);color:var(--dim);font-size:.85rem}
"#;

fn html_row(out: &mut String, at: Option<DateTime<Utc>>, who: Who, class: &str, inner: &str) {
    let _ = write!(
        out,
        "<div class=\"row who-{} {class}\"><time>{}</time><span class=\"g\">{}</span><div class=\"t\">{inner}</div></div>\n",
        who_name(who),
        clock(at).trim(),
        gutter(who)
    );
}

/// The trace as a self-contained page.
pub(super) fn render_html(trace: &Trace) -> String {
    let h = &trace.header;
    let mut out = String::new();
    let _ = write!(
        out,
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>trace {}</title><style>{STYLE}</style></head>\n<body>\n<header><h1>trace <code>{}</code></h1><dl>",
        esc(&h.conversation_id),
        esc(&h.conversation_id)
    );
    let mut dt = |k: &str, v: String| {
        let _ = write!(out, "<dt>{}</dt><dd>{}</dd>", esc(k), v);
    };
    dt(
        "warp",
        format!(
            "{} lines, {}",
            h.warp_lines,
            esc(&h.events_file.display().to_string())
        ),
    );
    match (&h.harness, &h.harness_file) {
        (Some(harness), Some(file)) => dt(
            harness,
            format!(
                "{} · {} lines ({} bookkeeping) · {}",
                esc(&h.harness_versions.join("/")),
                h.harness_lines,
                h.harness_lines_skipped,
                esc(&file.display().to_string())
            ),
        ),
        _ => {}
    }
    if let Some(session) = &h.linked_session_id {
        dt("session", esc(session));
    }
    if let Some(offset) = h.clock_offset_ms {
        dt(
            "clocks",
            esc(&format!(
                "{} calls joined; the harness's clock reads {offset:+} ms against Warp's, disclosed and not applied",
                h.joined_calls
            )),
        );
    }
    if let Some(note) = &h.note {
        dt("note", esc(note));
    }
    out.push_str("</dl></header>\n<main>\n");

    let mut footer = None;
    for item in items(trace) {
        match item {
            Item::Row(row) if row.kind == "system/compact_boundary" => {
                let _ = writeln!(
                    out,
                    "<div class=\"boundary\">{} · {}</div>",
                    esc(&compaction_caption(row)),
                    clock(row.at).trim()
                );
            }
            Item::Row(row) if row.kind == "compact_summary" => {
                let inner = format!(
                    "<details><summary>summary the agent continued from</summary><pre>{}</pre></details>",
                    esc(row.text.as_deref().unwrap_or(""))
                );
                html_row(&mut out, row.at, row.who, "summary", &inner);
            }
            Item::Row(row) if row.kind == "usage" => {
                footer = row.text.clone();
            }
            Item::Row(row) => {
                let (label, text) = caption(row);
                let mut inner = String::new();
                if !label.is_empty() {
                    let _ = write!(inner, "<span class=\"label\">{}</span> ", esc(&label));
                }
                inner.push_str(&esc(text.unwrap_or(if label.is_empty() {
                    &row.kind
                } else {
                    ""
                })));
                let class = if row.kind == "thinking" {
                    "thinking"
                } else {
                    ""
                };
                html_row(&mut out, row.at, row.who, class, &inner);
            }
            Item::Call(call) => {
                let state = call.state();
                let class = match state {
                    State::Open => "call state-open",
                    State::Done => "call state-done",
                    State::Failed => "call state-failed",
                    State::Denied => "call state-denied",
                    State::Unanswered => "call state-unanswered",
                };
                let mut inner = format!("<div class=\"head\"><b>{}</b>", esc(&call.name()));
                if let Some(input) = call.input() {
                    let _ = write!(inner, " <code>{}</code>", esc(&input));
                }
                let _ = write!(inner, "<span class=\"badge\">{}</span>", state_word(state));
                if let Some(decision) = decision(&call) {
                    let _ = write!(inner, " <span class=\"label\">{}</span>", esc(&decision));
                }
                inner.push_str("</div>");
                if let Some(edit) = call.edit() {
                    inner.push_str(&edit.html());
                }
                let stamps = stamps(&call);
                if !stamps.is_empty() {
                    let _ = write!(inner, "<div class=\"stamps\">{}</div>", esc(&stamps));
                }
                if let Some(result) = call.result.and_then(|row| row.text.as_deref()) {
                    inner.push_str(&html_result(result));
                }
                html_row(&mut out, call.first(), call.who(), class, &inner);
            }
        }
    }
    out.push_str("</main>\n");
    if let Some(usage) = footer {
        let _ = writeln!(out, "<footer>usage: {}</footer>", esc(&usage));
    }
    out.push_str("</body></html>\n");
    out
}

#[cfg(test)]
#[path = "trace_render_tests.rs"]
mod tests;
