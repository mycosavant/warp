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
//! ## Where it goes decides whether it works at all
//!
//! **Measured 2026-08-30, and it is the finding that shapes this feature.** With
//! the transcript written outside the pane's directory — which the `on` default
//! under [`crate::fork::state_dir`] always is — `opencode` asking to read it
//! arrives as `tool: other`, the kind `acp_permission` **cannot say yes to**.
//! Not "asks and waits": Warp offers no yes at all, so a person at the panel
//! could not approve it either. The recovery is unreachable by construction.
//!
//! With the same file inside the pane's directory it is an ordinary read, the
//! agent's native search tool finds it, and it works with **zero** permission
//! requests — verified end to end by planting a passphrase in one turn and
//! having the agent grep it back out of the file in the next.
//!
//! So the useful default is *not* the tidy one, and this is left as the caller's
//! choice rather than guessed at: writing into someone's repository without
//! being asked is worse than a path they had to name. What the docs owe the
//! reader is the sentence above, not a clever default.
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
    /// What the agent did, in order: one `name → outcome` per call.
    ///
    /// **Name and outcome, never the payload.** The measured loss this file
    /// exists for was *"three audited claims with their file:line citations"* —
    /// a record of what was checked and what came back. "What have I already
    /// tried" is the single most useful thing for an agent that just lost its
    /// history, because without it the failure mode is silently redoing work.
    /// Full results are the opposite trade: they are the largest thing here and
    /// the most cheaply recovered, since the agent can simply read the file
    /// again.
    ///
    /// **Empty on both fork transports, and structurally so — the paragraph
    /// above describes what this field is *for*, not what it currently
    /// delivers.** The collector matches `AIAgentOutputMessageType::Action`,
    /// and neither `acp_agent` nor `local_agent` ever emits one: an `Action`
    /// is an *instruction* to Warp's action model, so emitting one for a tool
    /// the agent has already run would run it a second time
    /// (`acp_agent/translate.rs` names this trap and counts four attempts at
    /// it). On those paths `### Tools used` never renders, and on the one path
    /// where `Action` messages do exist, `get_action_result`'s only writer is
    /// the collaboration path, so every line would read `-> no result
    /// recorded`.
    ///
    /// Recorded here rather than only in `../CLAUDE.md`, which has said it for
    /// some time, because the next person to wonder why the section is missing
    /// will be reading this field and not that file. **Do not close the gap by
    /// synthesising `Action` messages.** What the fork keeps instead is the
    /// prose: on these transports the outcome, the refusal and its reason are
    /// all in `output`, which is where the record actually lives.
    pub tools: Vec<String>,
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

/// Marks a line as Warp's own voice rather than the agent's.
///
/// **This exists because of a misattribution the first cut shipped.** Warp's
/// announcement is emitted as agent output — that is how it reaches the panel —
/// so it landed in the transcript under `### Agent`, and an agent grepping its
/// own history read Warp's words as its own. In a file whose entire purpose is
/// to be trusted as a record, that is worse than noise.
///
/// **Only Warp's editorial asides are stripped, never the record of what
/// happened.** Permission prose stays: that a call was requested and refused is
/// the one thing this transcript holds that the agent's own store does not —
/// measured T14.19, `opencode` records a denied call as `status=error` with no
/// notion that anything said no. Dropping it would leave a history in which
/// refusals look like failures, and an agent reading that would reasonably retry.
pub(crate) const CHROME: &str = "[Warp]";

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
        let spoken = strip_chrome(&exchange.output);
        // An exchange still running, or one that errored before saying anything,
        // has no output. Recording that plainly beats an empty section that
        // reads like the agent said nothing when it was never asked.
        if spoken.trim().is_empty() {
            out.push_str("(no output recorded)");
        } else {
            out.push_str(spoken.trim());
        }
        // **Appended rather than interleaved, and the trade is deliberate.**
        // Placing each call where it happened would mean re-implementing
        // upstream's section rendering; an ordered list keeps the sequence of
        // what was tried, which is the part that answers "have I done this
        // already", and loses only its position relative to the prose.
        if !exchange.tools.is_empty() {
            out.push_str("\n\n### Tools used\n");
            for tool in &exchange.tools {
                out.push_str(&format!("\n- {tool}"));
            }
        }
    }
    out.push('\n');
    out
}

/// Drops Warp's own announcements, keeping everything the agent actually said.
///
/// Line-based rather than clever: the marker starts a line, so a paragraph the
/// agent wrote that happens to quote the marker mid-sentence is untouched.
///
/// **But it leaves a mark rather than deleting, and the first cut did not.**
/// The doc above anticipated the mid-sentence case and not the line-start one:
/// an agent that quotes Warp's announcement at the start of a line — which is
/// exactly what an agent summarising its own context does — had that line
/// removed with nothing to say so. Silent deletion is a worse failure than the
/// misattribution this exists to prevent, in a file whose entire value is that
/// it can be trusted as a record. Found in review 2026-08-31. Warp's own asides
/// still never reach the file; what changed is that a reader can tell a line was
/// taken out.
fn strip_chrome(output: &str) -> String {
    // An exchange that was *only* Warp's note keeps reading as no output at all,
    // because there the agent genuinely said nothing and a marker would invent a
    // gap where none was lost. The mark is for the mixed case, which is the one
    // that loses something a reader needs.
    let kept_anything = output
        .lines()
        .any(|line| !line.trim().is_empty() && !line.trim_start().starts_with(CHROME));
    output
        .lines()
        .filter_map(|line| {
            if !line.trim_start().starts_with(CHROME) {
                Some(line)
            } else if kept_anything {
                Some(ELIDED)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Stands in for a line `strip_chrome` removed.
///
/// Deliberately not the marker itself, so this cannot be confused with the thing
/// it replaces, and deliberately visible: the reader of a transcript is an agent
/// reconstructing what happened, and a gap it cannot see is a gap it will fill
/// in with a guess.
const ELIDED: &str = "> [line removed: Warp's own note, not the agent's]";

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
    // Owner-only, because this file holds the user's prompts verbatim and this
    // fork's whole claim is that what is said here stays here. It was `0644` in
    // a `0755` directory until 2026-08-31, inherited from the umask.
    crate::fork::create_private_dir(dir)?;
    let path = path_for(dir, conversation_id);
    let temporary = path.with_extension("md.partial");
    {
        // The mode is on the create, not a chmod after it: the rename below
        // publishes this file under a name the agent is told to read, and a
        // window where it exists at the umask is a window someone can open it.
        let mut file = crate::fork::create_private_file(&temporary, false)?;
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
        "{CHROME} Warp is keeping a transcript of this conversation at {} and \
         telling the agent it can search it. Nothing else is added to your \
         prompts. Unset `WARP_FORK_TRANSCRIPT` to stop.",
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
    action_model: &warpui::ModelHandle<crate::ai::blocklist::BlocklistAIActionModel>,
    active_session: &warpui::ModelHandle<
        crate::terminal::model::session::active_session::ActiveSession,
    >,
    terminal_surface_id: warpui::EntityId,
    event: &crate::ai::blocklist::BlocklistAIHistoryEvent,
    ctx: &warpui::AppContext,
) {
    use warpui::SingletonEntity as _;

    use crate::ai::agent::conversation::ConversationStatus;
    use crate::ai::agent::{
        AIAgentActionResultTypeDiscriminants, AIAgentActionTypeDiscriminants,
        AIAgentOutputMessageType,
    };
    use crate::ai::blocklist::{
        BlocklistAIHistoryEvent, BlocklistAIHistoryModel, ConversationStatusUpdate,
    };

    let Some(location) = crate::fork::transcript_dir() else {
        return;
    };
    // **Asked for only when the location needs it.** The first cut looked the
    // session's directory up unconditionally, which silently gated `Fixed` too:
    // a caller who named an absolute directory got nothing whenever the pane
    // reported no cwd. Found by running, not by a test -- every unit test hands
    // `resolve` a directory, so none of them can see this.
    //
    // For `InSessionProject` a missing cwd is a real stop: Warp does not know
    // which project this is, and guessing would write the conversation
    // somewhere nobody named.
    let dir = match &location {
        crate::fork::TranscriptLocation::Fixed(path) => path.clone(),
        crate::fork::TranscriptLocation::InSessionProject => {
            let Some(cwd) = active_session
                .as_ref(ctx)
                .current_working_directory()
                .cloned()
            else {
                return;
            };
            location.resolve(std::path::Path::new(&cwd))
        }
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
        update,
        new_status,
        ..
    } = event
    else {
        return;
    };
    // **A restore is not a turn ending, and skipping it is not cosmetic.**
    // Measured 2026-08-30: relaunching Warp rewrote a transcript for a
    // conversation from the previous run, because a restore re-announces a
    // status that was reached before the process started. On a history of any
    // size that is a write storm at startup, for turns that ended days ago.
    // `event_log::warp_agent::status_event` guards the same way for the same
    // reason -- "logging it would put yesterday's `stop` in today's file".
    let ConversationStatusUpdate::Changed { prev_status } = update else {
        return;
    };
    // `Changed` does not mean changed: the status is re-emitted as every action
    // starts, so without this a busy turn rewrites the file once per action.
    if prev_status == new_status {
        return;
    }
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
    let actions = action_model.as_ref(ctx);
    let exchanges = conversation
        .root_task_exchanges()
        .map(|exchange| Exchange {
            input: exchange.format_input_for_copy(),
            // `None` for the action model, so tool *results* are left out --
            // those are the largest thing available and the most cheaply
            // recovered. The calls themselves are collected separately below.
            output: exchange.format_output_for_copy(None),
            tools: exchange
                .output_status
                .output()
                .map(|output| {
                    output
                        .get()
                        .messages
                        .iter()
                        .filter_map(|message| match &message.message {
                            AIAgentOutputMessageType::Action(action) => {
                                let name = format!(
                                    "{:?}",
                                    AIAgentActionTypeDiscriminants::from(&action.action)
                                );
                                // The outcome, by the same discriminant the event
                                // log uses, so one vocabulary describes a call in
                                // both places. A call with no result recorded is
                                // said plainly rather than guessed at -- it may
                                // still be running, or have been refused.
                                let outcome = actions
                                    .get_action_result(&action.id)
                                    .map(|result| {
                                        format!(
                                            "{:?}",
                                            AIAgentActionResultTypeDiscriminants::from(
                                                &result.result
                                            )
                                        )
                                    })
                                    .unwrap_or_else(|| "no result recorded".to_owned());
                                Some(format!("{name} -> {outcome}"))
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
        })
        .collect::<Vec<_>>();

    // Before the first write, not after it, so there is no moment when the
    // user's prompts sit in a repository with nothing in front of `git add -A`.
    // Only for the location the fork chose -- see `fork::keep_dir_out_of_git`.
    if matches!(location, crate::fork::TranscriptLocation::InSessionProject) {
        crate::fork::keep_dir_out_of_git(&dir);
    }

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
