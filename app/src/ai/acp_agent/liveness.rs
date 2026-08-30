//! Whether a turn is still saying anything (`.fork/TASKS.md`, T14.10).
//!
//! # The gap this fills, and the one it deliberately does not
//!
//! A parked permission request waits forever **on purpose**: a person is going
//! to answer it, and a deadline would only turn "you were slow" into "the turn
//! died". T14.6 built that and it is right.
//!
//! A turn with no question and no output has no such story. Measured T14.9, one
//! ran for 36 minutes with the agent process alive and sleeping — 74 seconds of
//! CPU against 2176 elapsed — while `agent.list` said `in_progress` throughout.
//! There was no pending approval to answer and no output to read, so the only
//! diagnosis available was `ps` and two screenshots fifteen minutes apart. The
//! information a person needed was *when the agent last said anything*, and
//! nothing kept it.
//!
//! So this is about **noticing**, not repairing. Recovery is already total —
//! `agent cancel` then a next turn's `session/load` restored a conversation
//! including work done in the minutes before it stalled — which is exactly why
//! nothing here decides anything. It reports two numbers and lets a person
//! decide, because "quiet for 18 minutes" is a *symptom*: a long compile and a
//! dead agent look identical from here, and a build that cancelled turns on a
//! timer would eventually cancel a working one.
//!
//! # Keyed on Warp's conversation id
//!
//! Not on the ACP session id, which is the agent's and does not exist until it
//! answers `session/new` — the window in which a turn can already have wedged.
//! `RequestParams::conversation_id` is Warp's own, it is what `agent.list`
//! reports, and it is therefore the only key that lets a person join *this* to
//! the row they were already looking at.
//!
//! # Why the record removes itself
//!
//! The same reason `registry::Parked` does: the guard lives in the driver
//! future, so a turn that ends — or is cancelled, or whose agent dies — drops it
//! on the way past, and nothing has to notice. A liveness map that leaked
//! entries would report a stale quiet time for a conversation that finished
//! hours ago, which is worse than reporting nothing at all.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Instant;

/// One in-flight turn's last sign of life.
struct Turn {
    last_update: Instant,
    /// The last tool call the agent announced, if it announced one.
    ///
    /// Set only by tool calls, never by message chunks, so a wedge that happened
    /// *after* some chatter still names the call it wedged on. That is the field
    /// T14.9 wanted: the panel showed a frozen `grep` and the CLI could not say
    /// so.
    last_tool: Option<String>,
    /// How many permission requests are parked for this conversation.
    ///
    /// **Because a quiet turn has two completely different meanings and the
    /// first version of this module reported them identically.** Found within
    /// the hour by using it: a turn waiting on a person read `quiet_for_seconds:
    /// 171`, which is *true* — the agent had not said anything for 171 seconds —
    /// and reads exactly like the wedge this was built to detect. An agent
    /// blocked on a question is behaving correctly and waiting forever is the
    /// design; a person seeing an alarming number for it learns to discount the
    /// number, which is how a signal stops working.
    ///
    /// A count rather than a flag: an agent may have more than one request
    /// outstanding, and answering one of three should not clear the state.
    waiting: usize,
}

static TURNS: LazyLock<Mutex<HashMap<String, Turn>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn turns() -> std::sync::MutexGuard<'static, HashMap<String, Turn>> {
    TURNS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Registers a turn as in flight until this is dropped.
pub(crate) struct Watch {
    conversation: String,
}

impl Drop for Watch {
    fn drop(&mut self) {
        turns().remove(&self.conversation);
    }
}

/// Starts watching a turn. The clock starts now, because a turn that wedges
/// before its agent says anything is exactly the case with no other signal.
pub(crate) fn watch(conversation: String) -> Watch {
    turns().insert(
        conversation.clone(),
        Turn {
            last_update: Instant::now(),
            last_tool: None,
            waiting: 0,
        },
    );
    Watch { conversation }
}

/// Records that the agent said something, and what if it was a tool call.
///
/// A `None` tool leaves the remembered one alone rather than clearing it: every
/// update is a sign of life, only some of them are a description.
pub(crate) fn note(conversation: &str, tool: Option<String>) {
    if let Some(turn) = turns().get_mut(conversation) {
        turn.last_update = Instant::now();
        if tool.is_some() {
            turn.last_tool = tool;
        }
    }
}

/// What this conversation's in-flight turn looks like from outside: how long
/// since the agent said anything, what it was last seen doing, and whether it is
/// quiet **because it is waiting for an answer**.
///
/// `None` when no turn of this kind is in flight — which is every conversation
/// not being served by the ACP path right now.
pub(crate) fn quiet_for(conversation: &str) -> Option<(u64, Option<String>, bool)> {
    turns().get(conversation).map(|turn| {
        (
            turn.last_update.elapsed().as_secs(),
            turn.last_tool.clone(),
            turn.waiting > 0,
        )
    })
}

/// Records a permission request parking, until the returned guard is dropped.
///
/// Guarded rather than paired with a `stop` call, for the same reason the turn
/// is: a request stops waiting when it is answered, when its turn is cancelled,
/// and when the agent goes away — and only one of those three runs any code of
/// ours.
pub(crate) struct Waiting {
    conversation: String,
}

impl Drop for Waiting {
    fn drop(&mut self) {
        if let Some(turn) = turns().get_mut(&self.conversation) {
            turn.waiting = turn.waiting.saturating_sub(1);
        }
    }
}

pub(crate) fn waiting_on_a_person(conversation: &str) -> Waiting {
    if let Some(turn) = turns().get_mut(conversation) {
        turn.waiting += 1;
    }
    Waiting {
        conversation: conversation.to_owned(),
    }
}

#[cfg(test)]
#[path = "liveness_tests.rs"]
mod tests;
