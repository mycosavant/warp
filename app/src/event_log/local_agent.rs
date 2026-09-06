//! The fork's own agent path, into the same log (T11.1c).
//!
//! T11.1 gave the agents Warp *hosts* a durable record; T11.1b added Warp's own.
//! Neither covers the path this fork exists for. With
//! `WARP_FORK_LOCAL_AGENT=1` a turn is answered by the `claude` CLI, and running
//! T11.1b showed the log carrying that turn's frame — `session_start`,
//! `prompt_submit`, `stop` — and **none of its tools**. Two independent reasons,
//! both measured:
//!
//! * Warp's action model never sees the call. `translate.rs` turns a `tool_use`
//!   block into *text* rather than a `ToolCall`, deliberately, because a
//!   ToolCall is an instruction and Warp would run the command a second time.
//!   So [`super::warp_agent`], which watches that model, has nothing to report.
//! * The plugin's OSC 777 does not arrive either. `local_agent` spawns `claude`
//!   on `Stdio::piped()` and reads its JSON directly, so there is no Warp PTY
//!   and nothing reaches the terminal parser world 2 hangs off.
//!
//! So this is a **third source**, and it says so: `source` is `local_agent`,
//! next to `in_process` and `rich_plugin`. The events land under Warp's
//! `AIConversationId` rather than Claude's session id, which is what puts them
//! in the same file as the frame — and is the whole of the plumbing T11.1c
//! needed, since `RequestParams` used to drop that id on the floor.
//!
//! **`call_id` is Claude's `tool_use.id`**, so `tool_start` → `tool_complete`
//! join here exactly as they do for Warp's own agent, and a call that starts and
//! never returns is a `tool_start` with no partner. That is the shape this phase
//! exists to catch, and this path is now the only one of the three where it can
//! be seen for a *third-party* agent — world 2 still has no per-call id, because
//! there it would have to come from the plugin (`TR-EVENTS-B`).
//!
//! **And it is the only source that reports nesting.** Claude's stream carries
//! `parent_tool_use_id`, so a subagent's tools name the `Task` call that spawned
//! them and land here as `parent_call_id`. That was not planned — running the
//! rest of T11.1c produced a `Task` turn whose inner `Read` was visibly a child
//! only because it happened to fall *between* the parent's `tool_start` and
//! `tool_complete`. Interleaving is not attribution: two subagents running at
//! once turn it into a soup, and fan-out is the case this fork most wants to
//! watch. So the field the stream already had is recorded rather than inferred.
//!
//! **`linked_session_id` is Claude's session id** (viewer phase 0, 2026-09-05).
//! Until then this path wrote `None` there, on the reasoning that it had "no
//! second id space to join to" — which was wrong: Claude's session id is the
//! conversation token this path round-trips for `--resume`, and it names the
//! file Claude itself keeps, `~/.claude/projects/<slug>/<id>.jsonl`. That file
//! holds the full input of every call where `tool_input_preview` here holds
//! 320 characters, so the viewer (`.fork/docs/observability.md`) treats it as
//! the record and these lines as the index into it. The id is taken from the
//! stream's `system/init`, not from the spawn arguments, so that a `--resume`
//! that missed names the session Claude actually ran.
//!
//! What this does **not** carry is a permission event. Claude in `--print` mode
//! does not report one: a refused tool comes back as an ordinary `tool_result`
//! with `is_error`, indistinguishable on the wire from a tool that ran and
//! failed. Both are logged as `tool_complete` with `error_type: "error"`, which
//! is what the stream actually said. Claiming a `permission_request` from it
//! would be an invention.

use super::{Entry, excerpt, project_name};
use crate::ai::local_agent::ToolEvent;

/// The `agent` value on every line this module writes.
///
/// The same string world 2 canonicalises the Claude CLI to
/// (`CLIAgent::command_prefixes`), because it is the same program; `source` is
/// what separates a pane a person ran it in from Warp driving it on a pipe.
const AGENT: &str = "claude";

/// The `source` value on every line this module writes.
///
/// Neither `in_process` (this crossed a process boundary) nor `rich_plugin`
/// (nothing was parsed out of a terminal). A reader who sees this knows the turn
/// was answered locally by the CLI rather than by Warp's server.
const SOURCE: &str = "local_agent";

/// What every line of one turn shares.
///
/// Held for the life of the turn by the stream in `ai::local_agent`, so the
/// conversation id and working directory are resolved once at spawn rather than
/// looked up per tool call — there is no model to ask on this path anyway.
pub(crate) struct TurnContext {
    /// Warp's `AIConversationId`, stringified. **Not** Claude's session id: that
    /// is what `conversation_token` carries here, and filing under it would put
    /// a turn's tools in a different file from its frame.
    session_id: String,
    /// Claude's session id, once the stream has named it; the join key to
    /// Claude's own session file. `None` only before `system/init`, which is
    /// the first line of the stream, so no tool event is written without it.
    linked_session_id: Option<String>,
    cwd: Option<String>,
}

impl TurnContext {
    pub(crate) fn new(conversation_id: String, cwd: Option<String>) -> Self {
        Self {
            session_id: conversation_id,
            linked_session_id: None,
            cwd,
        }
    }

    /// Names the Claude session this turn's lines belong to.
    ///
    /// Called with whatever the translator has seen, on every line, so it has
    /// to be cheap when nothing changed: it clones only when the id is new.
    pub(crate) fn link(&mut self, session_id: &str) {
        if self.linked_session_id.as_deref() != Some(session_id) {
            self.linked_session_id = Some(session_id.to_owned());
        }
    }
}

/// Appends one tool event from a local-agent turn.
pub(crate) fn record(context: &TurnContext, event: &ToolEvent) {
    if !super::is_enabled() {
        return;
    }
    let projected = project(event);
    super::record(Entry {
        // Absent, and that is the claim: this did not come off the OSC 777 wire,
        // so there is no protocol version to report. Claude's stream is versioned
        // by its own CLI, independently of anything Warp negotiates.
        v: None,
        agent: AGENT,
        event: projected.event,
        source: SOURCE,
        session_id: Some(&context.session_id),
        linked_session_id: context.linked_session_id.as_deref(),
        call_id: Some(&projected.call_id),
        parent_call_id: projected.parent_call_id.as_deref(),
        cwd: context.cwd.as_deref(),
        project: context.cwd.as_deref().and_then(project_name),
        tool_name: projected.tool_name.as_deref(),
        tool_input_preview: projected.tool_input_preview.as_deref(),
        summary: None,
        error_type: projected.error_type,
        plugin_version: None,
        decision: None,
        answered_by: None,
        via: None,
        can_approve: None,
        applied: true,
    });
}

/// One tool event in the log's vocabulary.
///
/// Split from [`record`] and owned rather than borrowed for the same reason
/// `warp_agent::action_event` is split out: the naming is the part worth
/// asserting, and a test should not need a log directory to do it.
struct Projected {
    event: &'static str,
    call_id: String,
    parent_call_id: Option<String>,
    tool_name: Option<String>,
    tool_input_preview: Option<String>,
    error_type: Option<&'static str>,
}

fn project(event: &ToolEvent) -> Projected {
    match event {
        ToolEvent::Started {
            call_id,
            parent_call_id,
            name,
            input_preview,
        } => Projected {
            event: "tool_start",
            call_id: call_id.clone(),
            parent_call_id: parent_call_id.clone(),
            tool_name: Some(name.clone()),
            tool_input_preview: input_preview.as_deref().map(excerpt),
            error_type: None,
        },
        ToolEvent::Completed {
            call_id,
            parent_call_id,
            name,
            failed,
        } => Projected {
            event: "tool_complete",
            call_id: call_id.clone(),
            parent_call_id: parent_call_id.clone(),
            tool_name: name.clone(),
            // The stream says only that it failed, so the log says only that.
            // World 1 puts a cancellation *kind* here because it has one;
            // inventing a finer word for this would make the two look
            // comparable when they are not.
            tool_input_preview: None,
            error_type: failed.then_some("error"),
        },
    }
}

#[cfg(test)]
#[path = "local_agent_tests.rs"]
mod tests;
