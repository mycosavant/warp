//! Permission requests an ACP agent is waiting on, and the way to answer them
//! (`.fork/TASKS.md`, T14.6).
//!
//! # Why this exists at all
//!
//! An ACP `session/request_permission` arrives **mid-turn**: the agent is
//! blocked, `session/prompt` is still outstanding, and the stream Warp is
//! serving is open and being polled. So the answer does not need the connection
//! to outlive the turn — it needs a way *in* to a turn that is already running.
//! T11.5 built one for CLI agents (`agent.approvals` / `agent.approve` /
//! `agent.deny`), and this is the second population answered through it.
//!
//! # Why the answer arrives on a channel instead of being applied here
//!
//! [`agent_client_protocol::Responder`] is `Send`, so it *could* be moved to
//! whatever thread the control-plane handler runs on and replied to there.
//! It is not, deliberately: T14.6's spike proved that parking a responder and
//! replying to it **from the connection's own task** works — held 180s, answered
//! within 5s, survived cancellation — and replying from somewhere else is a
//! thing nothing has run. So the handler only ever sends a [`Decision`] down a
//! oneshot, and the reply happens exactly where it was measured happening.
//!
//! That also gets the teardown right for free. The waiting future lives in the
//! connection task, so a cancelled turn drops it along with everything else, and
//! [`Parked`]'s `Drop` takes the entry out of the map on the way past. Nothing
//! has to notice that a turn ended.
//!
//! # What is deliberately not here
//!
//! **Yes.** Every entry reports `can_approve: false`, and `agent.approve` refuses
//! this population by name. Saying yes needs the shared `acp_permission::choose`
//! and a surface that can show what an option would allow; saying no needs
//! neither, because refusing is unconditional (T14.5) and a denial can only ever
//! make less happen. Shipping the half that is honest is not a shortcut — it is
//! the asymmetry this fork runs on.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard};

use futures::channel::oneshot;

/// What a person decided. Only one variant today — see the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Decision {
    Deny,
}

/// One request an agent is blocked on, as the control plane needs to describe
/// it.
///
/// Everything here is either Warp's own fact or the agent's, and the two are
/// kept apart on purpose — see [`Self::session_directory`].
#[derive(Debug, Clone)]
pub(crate) struct ParkedRequest {
    /// The JSON-RPC request id, stringified.
    ///
    /// **Keyed on the request, not on the tool call.** An agent may ask about
    /// the same `toolCallId` more than once, and a key that collapsed those
    /// would let an answer to the first land on the second — the stale-answer
    /// hazard T11.5's digest exists for, reintroduced through the key.
    ///
    /// **And it is scoped to the turn, because a JSON-RPC id is not unique on
    /// its own.** Found by running two ACP conversations at once: the ids are
    /// per-connection and both agents opened with `0`, so the second `park`
    /// evicted the first from this process-global map. That dropped the first
    /// request's sender, which its waiter reads as "answered" — so *both* turns
    /// denied instantly while their panels still said they were waiting for a
    /// person. The caller supplies the scope; see `mod.rs`.
    pub approval_id: String,
    /// The program `WARP_FORK_ACP_COMMAND` names, so an entry says which agent
    /// is waiting rather than just that one is.
    pub agent: String,
    /// The agent's own one-line description of what it wants to do.
    pub title: Option<String>,
    /// The tool kind as the agent typed it — `execute`, `edit`, `read`, …
    pub tool_name: Option<String>,
    /// The command, or whatever `rawInput` carried, as text.
    pub tool_input: Option<String>,
    /// The directory Warp put this session in, which it knows first-hand
    /// because it chose it from the pane and sent it in `session/new`.
    ///
    /// Worth showing on its own account: measured on T14.6, this directory
    /// decides whether the user's own agent configuration — including their
    /// permission rules — is loaded at all. It is **not** a claim about whose
    /// rules governed this call, and nothing here may present it as one.
    pub session_directory: Option<String>,
    /// The ACP session id, so an entry can be lined up with the lines
    /// `WARP_FORK_EVENT_LOG` wrote for the same session.
    pub session_id: Option<String>,
    /// Every option the agent offered, by name, as **data rather than as
    /// controls**.
    ///
    /// Recorded because "the offer went unrecorded" is the finding
    /// `acp_permission::is_more_than_an_answer` exists for. Rendering a control
    /// for one of these is a separate decision and mostly a refusal: an
    /// `allow_always` carrying no declaration — measured, that is what `opencode`
    /// sends — cannot be shown to a person in terms of what it would widen,
    /// because there is nothing to show but the name.
    pub options_offered: Vec<String>,
}

/// A parked request, and the way to wake it.
struct Parked {
    request: ParkedRequest,
    answer: oneshot::Sender<Decision>,
    /// Distinguishes this entry from any later one that reuses its key.
    token: u64,
}

static NEXT_TOKEN: AtomicU64 = AtomicU64::new(0);

static PARKED: LazyLock<Mutex<HashMap<String, Parked>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn registry() -> MutexGuard<'static, HashMap<String, Parked>> {
    PARKED
        .lock()
        // The lock is held only for map operations, none of which can panic
        // while holding it, so a poisoned lock would mean a bug elsewhere had
        // already taken the process somewhere undefined.
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Registers a request as waiting on a person, and hands back the future that
/// resolves when one answers.
///
/// The returned guard **must** be held by whatever is waiting: dropping it
/// removes the entry, which is how a cancelled or failed turn stops advertising
/// a question nobody can answer any more.
pub(crate) fn park(request: ParkedRequest) -> (Waiting, oneshot::Receiver<Decision>) {
    let (answer, wait) = oneshot::channel();
    let approval_id = request.approval_id.clone();
    let token = NEXT_TOKEN.fetch_add(1, Ordering::Relaxed);
    registry().insert(
        approval_id.clone(),
        Parked {
            request,
            answer,
            token,
        },
    );
    (Waiting { approval_id, token }, wait)
}

/// Keeps a parked request listed for exactly as long as something is waiting on
/// it.
pub(crate) struct Waiting {
    approval_id: String,
    token: u64,
}

impl Drop for Waiting {
    fn drop(&mut self) {
        let mut registry = registry();
        // **Only if this entry is still ours.** Removing by key alone is what
        // turned a key collision into a cascade: the first waiter's cleanup
        // evicted the *second* request that had taken its key, denying a turn
        // nobody had answered. Keys are unique now, so this is belt and braces —
        // but it is the cheap half of the fix and the failure it prevents is
        // silent.
        if registry
            .get(&self.approval_id)
            .is_some_and(|parked| parked.token == self.token)
        {
            registry.remove(&self.approval_id);
        }
    }
}

/// Everything currently waiting on a person, oldest key first so a caller
/// polling it does not get a reshuffled list between calls.
pub(crate) fn waiting() -> Vec<ParkedRequest> {
    let registry = registry();
    let mut requests = registry
        .values()
        .map(|parked| parked.request.clone())
        .collect::<Vec<_>>();
    requests.sort_by(|left, right| left.approval_id.cmp(&right.approval_id));
    requests
}

/// Answers one parked request, if it is still parked.
///
/// `false` means there is nothing by that id — which is the same answer a
/// caller gets for a request that was already answered, or whose turn was
/// cancelled while they were deciding. Both are "the question is gone", and
/// distinguishing them would mean keeping a record of answered requests for no
/// one to read.
pub(crate) fn answer(approval_id: &str, decision: Decision) -> bool {
    let Some(parked) = registry().remove(approval_id) else {
        return false;
    };
    // The receiver is gone only if the turn ended between the lookup and here,
    // in which case the request is moot and there is nothing to report.
    parked.answer.send(decision).is_ok()
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
