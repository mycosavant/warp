//! A tool call the agent ran, drawn as one row that changes state.
//!
//! Until 2026-09-03 a tool call reached the panel as `` `title` `` -- the
//! agent's first title, in backticks, as `Message::AgentOutput`. Measured on
//! `claude-agent-acp` 0.73.0 that first title is a placeholder (*"Preparing
//! file…"*, *"Terminal"*): the real one arrives on a later `tool_call_update`
//! that carries no status, and the completion carries no title, so the
//! correction `tool_update_text` waited for never came and the transcript of
//! the measured turn reads *"Preparing file…"* three times with nothing else.
//! The panel was drawing the one line the agent had not meant anyone to read.
//!
//! **One row per call, updated in place.** The row is an `AgentOutput` whose
//! `server_message_data` is [`tag`] for its state -- the same channel
//! `warp_note` opened, with the state in the tag rather than in the text so
//! nothing is parsed to learn it. It is appended once, when the agent announces
//! the call, and every later change -- the real title, the completion, a
//! failure, a denial -- is an `UpdateTaskMessage` for the same message id with
//! a field mask naming the body and the tag. `acp_agent/mod.rs` declined that
//! path twice as a guess about a `FieldMask` into the `Message` oneof; it is
//! not a guess any more. `Task::upsert_message` applies it through
//! `crates/field_mask`, `Exchange::upsert_output_for_message` re-converts the
//! result in place while the exchange streams, and a test in this module's
//! neighbour pins the two path names against `api::MESSAGE_DESCRIPTOR`.
//!
//! **The headline says the verb in the tense of the state.** *Running `cargo
//! test`* becomes *Ran `cargo test`*; a failure is *Failed to run*, a refusal
//! is *Denied: run*, and a row the turn ended around is *Interrupted while
//! running*. That last one exists so the row never claims more than the code
//! does: a `Running` row after the stream has closed would be a spinner over a
//! process nobody is watching. The translator sweeps open rows at the end of a
//! turn, and the renderer draws a `Running` row in a settled exchange as
//! interrupted regardless, because a turn cancelled from outside ends without
//! the sweep.
//!
//! **Never `Message::ToolCall`.** That type is an instruction Warp executes.
//! The agent has already run this; the row reports it. The constraint is
//! restated in `acp_agent/translate.rs` with a test under it.
//!
//! What the detail holds is the agent's own description of the call, when it
//! gave one, and the content it attached on completion -- command output as
//! the agent formatted it, a written file's text, an error. It is what a
//! person opens the chevron for and what the transcript keeps.

use warp_multi_agent_api as api;

use super::warp_note::Note;

/// Where a tool call is, as far as Warp has heard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolRowState {
    /// Announced, not yet reported finished.
    Running,
    /// `ToolCallStatus::Completed`.
    Done,
    /// `ToolCallStatus::Failed`, and Warp did not refuse it.
    Failed,
    /// `ToolCallStatus::Failed` after Warp answered *no* to this call.
    Denied,
    /// The turn ended -- finished, failed or cancelled -- while the row was
    /// still `Running`. Warp stopped listening; it does not know the outcome.
    Interrupted,
}

/// Whether an agent's title for a tool call names *this* call, or is the
/// placeholder it sends before it knows.
///
/// **Measured on `claude-agent-acp` 0.73.0** (`146265e37`): the first
/// `tool_call` carries *"Preparing file…"* or *"Terminal"*, and the real title
/// arrives on a later update. A row that took the first one at face value drew
/// the one line the agent had not meant anyone to read.
///
/// **Shared with the approval card on purpose.** The row distrusted the title
/// and [`acp_approval::layered`] trusted it, so the same request could headline
/// as *"Terminal"* above the buttons while its row said *"Running `cargo
/// test`"* -- two surfaces disagreeing about the same string, with the
/// credulous one being the one a person answers.
///
/// `also` carries whatever else counts as bare for the caller: the row passes
/// its own verb, the card passes the tool kind the agent typed.
///
/// [`acp_approval::layered`]: crate::ai::blocklist::inline_action::acp_approval::layered
pub(crate) fn is_placeholder_title(title: &str, also: &[&str]) -> bool {
    let title = title.trim();
    title.is_empty()
        || title.ends_with('\u{2026}')
        || title.ends_with("...")
        || title.eq_ignore_ascii_case("terminal")
        || also
            .iter()
            .any(|bare| !bare.is_empty() && title.eq_ignore_ascii_case(bare))
}

/// How a row's headline reads once the renderer alone has demoted it -- a row
/// left `Running` in an exchange that has stopped streaming, which the wire
/// never got round to rewriting.
///
/// **Prefixed, never re-tensed.** The first cut rewrote any headline whose
/// first word ended in *"ing"* into *"Interrupted while {word} …"*. That is
/// right for the verbs Warp writes itself and wrong for the ones it does not:
/// a row with no object falls back to the agent's title used whole, so *"Ping
/// the host"* came out as *"Interrupted while ping the host"*. Re-tensing
/// someone else's sentence is how a row starts lying, which the translator
/// says where it picks that fallback -- and this was the same mistake one file
/// over, in the half that cannot tell the two kinds of headline apart.
pub(crate) fn demoted_headline(headline: &str) -> String {
    format!("Interrupted: {headline}")
}

const TAG_PREFIX: &str = "warp-fork/tool/";

/// The value of `server_message_data` for a row in `state`.
pub(crate) fn tag(state: ToolRowState) -> &'static str {
    match state {
        ToolRowState::Running => "warp-fork/tool/running",
        ToolRowState::Done => "warp-fork/tool/done",
        ToolRowState::Failed => "warp-fork/tool/failed",
        ToolRowState::Denied => "warp-fork/tool/denied",
        ToolRowState::Interrupted => "warp-fork/tool/interrupted",
    }
}

/// The state a message's opaque payload marks it as being in, if it is a
/// tool row at all. Compared, never parsed: an unknown suffix is not a row.
pub(crate) fn state_of(server_message_data: &str) -> Option<ToolRowState> {
    if !server_message_data.starts_with(TAG_PREFIX) {
        return None;
    }
    [
        ToolRowState::Running,
        ToolRowState::Done,
        ToolRowState::Failed,
        ToolRowState::Denied,
        ToolRowState::Interrupted,
    ]
    .into_iter()
    .find(|state| tag(*state) == server_message_data)
}

/// One tool call, as the panel shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Row {
    pub(crate) state: ToolRowState,
    /// Verb and object, tensed for the state. Always shown.
    pub(crate) headline: String,
    /// The agent's description and the call's output. Shown on request.
    pub(crate) detail: String,
}

impl Row {
    pub(crate) fn new(
        state: ToolRowState,
        headline: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            state,
            headline: headline.into(),
            detail: detail.into(),
        }
    }

    /// The text that travels in `AgentOutput::text`: the same headline, blank
    /// line, detail shape a note uses, so a build that predates the tag draws
    /// the headline as prose and the transcript reads it as a sentence.
    pub(crate) fn to_wire(&self) -> String {
        Note::new(self.headline.clone(), self.detail.clone()).to_wire()
    }

    /// The inverse of [`Row::to_wire`], given the state the tag carried.
    pub(crate) fn from_wire(state: ToolRowState, text: &str) -> Self {
        let note = Note::from_wire(text);
        Self::new(state, note.headline, note.detail)
    }

    /// The proto message for this row, marked with its state.
    ///
    /// Identity is the caller's, as for a note -- and here it matters more,
    /// because every update for a call must carry the id the announcement
    /// carried or it lands as a second row instead of a change to the first.
    pub(crate) fn into_message(self, mut message: api::Message) -> api::Message {
        message.message = Some(api::message::Message::AgentOutput(
            api::message::AgentOutput {
                text: self.to_wire(),
            },
        ));
        message.server_message_data = tag(self.state).to_owned();
        message
    }
}

/// The field-mask paths an in-place update of a row names: the body and the
/// tag, nothing else. The identity and timestamp fields stay as announced.
///
/// `agent_output` is the `oneof` member's own field name in `task.proto`, which
/// is what `prost_reflect` resolves a path segment against; the `oneof`'s name
/// (`message`) is not a field and a path through it would be silently skipped
/// -- `apply_path` returns `Ok(())` for a name the descriptor lacks. Pinned by
/// test against the real descriptor for exactly that reason.
pub(crate) const UPDATE_MASK: [&str; 2] = ["agent_output", "server_message_data"];

#[cfg(test)]
#[path = "tool_row_tests.rs"]
mod tests;
