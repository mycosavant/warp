//! Warp's own voice in a conversation, as a message *kind* rather than as text.
//!
//! Until 2026-09-03 everything Warp said in the agent panel -- the mode
//! disclosure, the transcript announcement, the asking note, the answered
//! note -- went out as `Message::AgentOutput`, the same type as the agent's
//! prose. The renderer could not tell them apart, so it could not style them,
//! collapse them, or count them, and measured during a turn with three asks
//! Warp's words outweighed the agent's **9.4 : 1** while the agent's narration
//! never reached the screen (`.fork/COMPOSER.md`). The `[Warp]` marker did not
//! help: it is text chrome for `transcript::strip_chrome`, not a channel.
//!
//! **The carrier is a field, not a new proto variant.** `api::Message` comes
//! from the `warp-proto-apis` git dependency and the fork cannot add to its
//! `oneof`. It does carry `server_message_data`, documented in `task.proto` as
//! *"an opaque payload that the client should simply roundtrip"*, and nothing
//! in this workspace reads it (measured: every reference sets it to the empty
//! string). A note is an `AgentOutput` whose `server_message_data` is [`TAG`];
//! `convert_from` turns that into [`AIAgentOutputMessageType::WarpNote`], and
//! everything downstream of the conversion sees a distinct kind. Because the
//! proto shape is unchanged the note survives persistence -- conversations are
//! stored as `api::Task` and rebuilt through the same conversion on restore --
//! and a build that predates the tag renders it as text, which is exactly what
//! it did before.
//!
//! **Wire form: a headline, a blank line, the detail.** The panel draws the
//! headline as a row and the detail behind a chevron, so the sentence a person
//! needs is always visible and the paragraphs that justify it are one click
//! away. `format_for_copy` writes both back out in that order, so the
//! transcript and the clipboard carry the same words they always did.
//!
//! **What this does not change: which of Warp's words reach the transcript.**
//! `strip_chrome` removes lines starting with `[Warp]` and keeps everything
//! else; permission prose is deliberately kept there, because a refusal is the
//! one thing the transcript holds that the agent's own store does not. That
//! decision is the transcript's and is made on the text, as before. A note
//! that must stay out of the file still starts with `transcript::CHROME`, and
//! the renderer hides that prefix rather than the writer dropping it.
//!
//! [`AIAgentOutputMessageType::WarpNote`]: crate::ai::agent::AIAgentOutputMessageType::WarpNote

use warp_multi_agent_api as api;

/// The value of `api::Message::server_message_data` that marks a note as
/// Warp's. Chosen to be unmistakable rather than short: this string is
/// compared, never parsed, and a collision with an opaque payload set by
/// somebody's server would misfile one of their messages as ours.
pub(crate) const TAG: &str = "warp-fork/note";

/// One thing Warp says in a conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Note {
    /// The sentence a person needs. Always shown.
    pub(crate) headline: String,
    /// The paragraphs behind it. Shown on request; empty for a note that is
    /// only its headline.
    pub(crate) detail: String,
}

impl Note {
    pub(crate) fn new(headline: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            headline: headline.into(),
            detail: detail.into(),
        }
    }

    /// A note that is only its headline.
    pub(crate) fn headline(headline: impl Into<String>) -> Self {
        Self::new(headline, String::new())
    }

    /// The text that travels in `AgentOutput::text`: headline, blank line,
    /// detail. A headline-only note is just the headline, so a reader of the
    /// transcript sees one line and not one line and a gap.
    pub(crate) fn to_wire(&self) -> String {
        let detail = self.detail.trim();
        if detail.is_empty() {
            self.headline.trim().to_owned()
        } else {
            format!("{}\n\n{detail}", self.headline.trim())
        }
    }

    /// The inverse of [`Note::to_wire`]: the first paragraph is the headline,
    /// whatever follows the first blank line is the detail.
    ///
    /// Total: text with no blank line is a headline-only note. Leading blank
    /// lines are skipped rather than read as an empty headline, because a
    /// note with nothing visible is a note that was not said.
    pub(crate) fn from_wire(text: &str) -> Self {
        let text = text.trim_start_matches(['\n', '\r']);
        match text.split_once("\n\n") {
            Some((headline, detail)) => Self::new(headline.trim_end(), detail.trim()),
            None => Self::headline(text.trim_end()),
        }
    }

    /// The proto message for this note, marked as Warp's.
    ///
    /// Everything but the body and the tag is the caller's: id, task,
    /// request and timestamp are what make it *this* turn's message, and the
    /// translators own that numbering.
    pub(crate) fn into_message(self, mut message: api::Message) -> api::Message {
        message.message = Some(api::message::Message::AgentOutput(
            api::message::AgentOutput {
                text: self.to_wire(),
            },
        ));
        message.server_message_data = TAG.to_owned();
        message
    }
}

/// Whether a message's opaque payload marks it as one of Warp's notes.
pub(crate) fn is_tagged(server_message_data: &str) -> bool {
    server_message_data == TAG
}

#[cfg(test)]
#[path = "warp_note_tests.rs"]
mod tests;
