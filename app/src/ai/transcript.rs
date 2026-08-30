//! A conversation's history, on disk, so the agent can grep back what it forgot.
//!
//! **The problem this answers, measured before it was written (T14.13).** An ACP
//! agent compacts its own context on its own policy — `opencode` 1.18.25 at
//! roughly 70% of a 200,000-token window, three times in one twenty-one-exchange
//! session — and neither the agent nor the Agent Client Protocol has any way to
//! report that it happened. `SessionNotification` has no update kind for it. So
//! Warp's transcript stays complete while the agent's working context does not,
//! the panel renders Warp's copy, and a person sees turns the agent can no
//! longer see. Measured: 21 exchanges and 159,747 characters still intact at the
//! moment the agent could not recall its own first eleven turns.
//!
//! **Why this routes around detection rather than waiting for it.** The obvious
//! shape is "notice the compaction, then help". That shape cannot be built —
//! there is no signal to notice. But the *recovery* never needed one: Warp holds
//! the whole record either way, so it can put a searchable copy on disk and name
//! the path once. Whether the agent ever compacted, and whether anyone ever
//! found out, stops mattering.
//!
//! **Grep, not read.** Handing a 160,000-character transcript back to an agent
//! would spend exactly the budget that just ran out. The file exists to be
//! *searched* — the pointer says so in those words — so recovery costs the hits
//! rather than the history. This is the whole reason the feature is a file path
//! and not an injected context block.
//!
//! ## What this is not
//!
//! It does not manage the agent's context, and the line matters. Warp writes a
//! file and names it; the agent decides whether to look, what to search for, and
//! what to do with the result. Nothing here re-prompts after a compaction,
//! injects recovered history, or edits the conversation — that would put Warp in
//! the business of authoring someone else's turn, which is the same overreach
//! `acp_permission` refuses when it declines `switch_mode`.
//!
//! It is also not a substitute for the disclosure half of T14.13. Saying in the
//! panel that the two views have diverged is a separate, still-unbuilt thing,
//! and it still wants a protocol signal that does not exist. This makes the
//! divergence *survivable*; it does not make it *visible*.
//!
//! ## Off by default
//!
//! This writes what was said to disk. A fork whose thesis is that nothing leaves
//! the machine should not begin persisting conversation text because it would be
//! convenient, so `WARP_FORK_TRANSCRIPT` has no default — see
//! [`crate::fork::transcript_dir`].

use std::io::Write as _;
use std::path::{Path, PathBuf};

/// One exchange, already rendered to text.
///
/// Kept as plain strings so the rendering below can be tested without a UI
/// context, which is the only part of this module with interesting behaviour.
pub(crate) struct Exchange {
    pub input: String,
    pub output: String,
}

/// Where a conversation's transcript lands.
///
/// Keyed by Warp's conversation id, matching the event log's convention — one
/// turn's evidence should never be split across files named by different ids,
/// which is the defect T14.15 fixed and this deliberately does not reintroduce.
pub(crate) fn path_for(dir: &Path, conversation_id: &str) -> PathBuf {
    dir.join(format!("{conversation_id}.md"))
}

/// The line handed to the agent, naming the file and how to use it.
///
/// **Deliberately imperative about `grep`.** An agent told only that a transcript
/// exists will read it, which costs more context than the compaction that made
/// it necessary. Saying "search" rather than "read" is the difference between
/// recovering a fact and re-spending the window.
///
/// It says nothing about *why* the file might be needed. Warp does not know
/// whether this agent compacts, when, or whether it has; asserting that it has
/// forgotten something would be Warp inventing a fact about someone else's
/// process. The file is offered, not explained.
pub(crate) fn pointer(path: &Path) -> String {
    format!(
        "[Warp] The full transcript of this conversation, including turns older \
         than your current context, is at {}. Search it with grep rather than \
         reading it whole if you need something from earlier.",
        path.display()
    )
}

/// Renders exchanges to the file's text.
///
/// Markdown with stable, greppable headers: a searcher wants to find a turn and
/// then see its boundaries, so each exchange is delimited by a line that carries
/// its number.
pub(crate) fn render(conversation_id: &str, exchanges: &[Exchange]) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Warp conversation {conversation_id}\n\n"));
    out.push_str(
        "Written by Warp for the agent's own recall. Search this file; do not read\n\
         it whole. Exchanges are numbered from 1 and appear oldest first.\n",
    );
    for (index, exchange) in exchanges.iter().enumerate() {
        out.push_str(&format!("\n\n## Exchange {}\n\n### User\n\n", index + 1));
        out.push_str(exchange.input.trim());
        out.push_str("\n\n### Agent\n\n");
        // An exchange still running, or one that errored before saying anything,
        // has no output. Recording that plainly beats an empty section that
        // reads like the agent said nothing when it was never asked.
        if exchange.output.trim().is_empty() {
            out.push_str("(no output recorded)");
        } else {
            out.push_str(exchange.output.trim());
        }
    }
    out.push('\n');
    out
}

/// Writes the transcript, replacing whatever was there.
///
/// **Rewritten whole rather than appended.** An exchange can be edited, retried
/// or cancelled after it first appears, so appending would accumulate a history
/// of drafts that no longer matches what the panel shows — and the file's whole
/// value is that it agrees with Warp's copy. Rewriting is O(n) per turn against
/// an n that is tens of exchanges, which is not a cost worth a correctness
/// argument.
///
/// Written to a temporary file and renamed, so a reader that greps mid-write
/// sees the previous complete version rather than half of this one. The agent
/// reads this file at times Warp does not control.
pub(crate) fn write(
    dir: &Path,
    conversation_id: &str,
    exchanges: &[Exchange],
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = path_for(dir, conversation_id);
    let temporary = path.with_extension("md.partial");
    {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(render(conversation_id, exchanges).as_bytes())?;
        file.sync_all()?;
    }
    std::fs::rename(&temporary, &path)?;
    Ok(path)
}

/// Conversations whose pointer has already been announced in the panel.
static TOLD: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<String>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashSet::new()));

/// Whether the panel still needs to be told that Warp is handing the agent a
/// transcript path, and marks it told.
///
/// **The pointer rides every prompt; the announcement happens once.** Those are
/// different cadences on purpose. The pointer has to be re-sent because the
/// compaction this exists for would eat it along with everything else, and one
/// line per turn is a rounding error against the window. Saying so in the panel
/// every turn would be noise about a thing that has not changed.
///
/// **And it is announced at all because Warp is adding words to someone else's
/// conversation.** The block does not appear in the panel — the panel renders
/// what the person typed — so without this the agent would be reading an
/// instruction the user never saw and cannot account for. Disclosure is the
/// same answer T14.18 gave for session modes, for the same reason.
pub(crate) fn needs_announcing(conversation_id: &str) -> bool {
    let mut told = TOLD.lock().expect("the transcript lock is uncontended");
    told.insert(conversation_id.to_owned())
}

/// What the panel says, once, when the pointer starts riding along.
pub(crate) fn announcement(path: &Path) -> String {
    format!(
        "Warp is keeping a transcript of this conversation at {} and telling the \
         agent it can search it. Nothing else is added to your prompts. Unset \
         `WARP_FORK_TRANSCRIPT` to stop.",
        path.display()
    )
}

#[cfg(test)]
pub(crate) fn forget(conversation_id: &str) {
    TOLD.lock()
        .expect("the transcript lock is uncontended")
        .remove(conversation_id);
}

/// Writes the transcript when a turn ends, if the fork was asked to keep one.
///
/// **Called for every history event and filtered here rather than at the call
/// site**, so the whole policy — whether to write, for which surface, on which
/// status — reads in one place. The subscription that feeds this exists per
/// terminal surface and the history model is global, so without the surface
/// filter one turn would be written once per open pane.
///
/// **Only on a terminal status.** Writing mid-turn would put a half-finished
/// exchange in the file, and the agent that greps it cannot tell a turn still
/// running from one that stopped there.
pub(crate) fn observe(
    terminal_surface_id: warpui::EntityId,
    event: &crate::ai::blocklist::BlocklistAIHistoryEvent,
    ctx: &warpui::AppContext,
) {
    use warpui::SingletonEntity as _;

    use crate::ai::agent::conversation::ConversationStatus;
    use crate::ai::blocklist::{BlocklistAIHistoryEvent, BlocklistAIHistoryModel};

    let Some(dir) = crate::fork::transcript_dir() else {
        return;
    };
    if event
        .terminal_surface_id()
        .is_none_or(|id| id != terminal_surface_id)
    {
        return;
    }
    // Deliberately not exhaustive, for the reason `event_log::warp_agent` gives
    // at length: `BlocklistAIHistoryEvent` has 26 variants that upstream adds
    // to, and a transcript that has not learned about a new one is stale, never
    // wrong.
    let BlocklistAIHistoryEvent::UpdatedConversationStatus {
        conversation_id,
        new_status,
        ..
    } = event
    else {
        return;
    };
    if !matches!(
        new_status,
        ConversationStatus::Success | ConversationStatus::Error | ConversationStatus::Cancelled
    ) {
        return;
    }

    let history = BlocklistAIHistoryModel::as_ref(ctx);
    let Some(conversation) = history.conversation(conversation_id) else {
        return;
    };
    let exchanges = conversation
        .root_task_exchanges()
        .map(|exchange| Exchange {
            input: exchange.format_input_for_copy(),
            // `None` for the action model, so tool results are left out. This
            // file exists to be searched, and a transcript carrying every file
            // read would be larger than the context that ran out.
            output: exchange.format_output_for_copy(None),
        })
        .collect::<Vec<_>>();

    if let Err(error) = write(&dir, &conversation_id.to_string(), &exchanges) {
        // Warned rather than surfaced, for the same reason the event log warns:
        // a full disk should not turn a recovery aid into a second failure on
        // top of the first.
        log::warn!("fork transcript: write for {conversation_id} failed: {error}");
    }
}

#[cfg(test)]
#[path = "transcript_tests.rs"]
mod tests;
