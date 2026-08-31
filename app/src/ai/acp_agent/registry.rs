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
//! # Yes, and what it is allowed to mean
//!
//! Increments 1 and 2 shipped deny-only, with every entry refused by one
//! sentence: *"there is no surface that could show you what saying yes would
//! allow."* That sentence has since gone **false**, and a refusal whose stated
//! reason is false is the T14.2 failure — a person concluding the feature is
//! broken. Both surfaces now render the tool call: the title, the kind, the
//! agent's verbatim `rawInput`, the options it offered, and — since the
//! `toolCallId` join — where it says it acts. `acp_permission`'s own rule is
//! that *a single-shot option declares only the tool call, which every surface
//! renders*, so for those entries the condition is met.
//!
//! So the gate is **per entry, not per population**, and it is
//! [`ParkedRequest::approve_selects`]: the shared `acp_permission::choose` picks
//! the option, or writes down why it would not.
//!
//! What that still refuses is unchanged, and none of it is about how attentive
//! the person is. `choose` declines a `switch_mode` request, an unrecognised or
//! absent tool kind, and any option declaring a policy change — because a
//! **binary** yes cannot honestly mean "and also change the session's policy",
//! whoever is looking at it. A surface that could offer those would have to
//! render each option separately; that is a different control and a later
//! ticket. One more condition is added here rather than there: an entry whose
//! `tool_input` is absent shows only the agent's own one-line summary, and
//! approving a summary is not approving a tool call.
//!
//! The asymmetry the fork runs on survives intact — a **no** needs none of this
//! and is still unconditional, because declining can only ever make less happen.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard};

use futures::channel::oneshot;

/// What a person decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Decision {
    /// Select the option [`ParkedRequest::approve_selects`] names.
    ///
    /// **Only meaningful when that field is `Some`**, and a waiter handed this
    /// for an entry without one denies instead. The control plane refuses such
    /// an answer before it gets here; the fallback exists because "fail closed"
    /// has to be true at both ends, not just at the one being looked at.
    Allow,
    Deny,
}

/// Which of Warp's answering surfaces a decision came through.
///
/// **Two values, because [`answer`] has exactly two callers and each knows
/// which it is.** No inference is involved and none is possible: the surface is
/// passed in, so a line in the log says where an answer came from rather than
/// where it probably came from.
///
/// **[`Self::ControlPlane`] must not be read as "a person at this keyboard",
/// and the name is chosen so it cannot be.** It is `agent.approve` /
/// `agent.deny` arriving at the control plane, which is one door with three
/// things behind it: `warpctrl` in a local shell, the T12 console in a browser,
/// and a paired phone answering over the LAN.
///
/// **The distinction to keep: Warp does not *lack* that knowledge, it fails to
/// carry it.** The pairing layer knows perfectly well that a request arrived on
/// a paired credential — that is what `PAIRABLE_ACTIONS` is checked against.
/// What is missing is a channel from there to here: `agent_answer` takes
/// `(instance_id, decision, params, ctx)` and `LocalControlBridge` holds
/// `instance_id`, `control_origin` and `pairing` and no per-request caller
/// identity (read, not run). So this is a plumbing gap a later ticket could
/// close, not a fact about the world — and writing it down that way is what
/// stops the coarse value hardening into a belief that finer is impossible.
///
/// Until then, naming the door is the most this can honestly say. A value like
/// `cli` or `local` would be a claim about which of the three, and this fork's
/// rule is that an unknown is rendered as unknown rather than filled in from
/// the likeliest case.
///
/// **`--approve` is not a third value and never reaches here.**
/// `crates/warp_cli/src/local_control/acp.rs` is a standalone ACP client in its
/// own process, with no Warp behind it and no event log to write to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Surface {
    /// `agent.approve` / `agent.deny` through the control plane — a local
    /// `warpctrl`, the console, or a paired device. See the type's own note on
    /// why these three are one value.
    ControlPlane,
    /// The in-panel approval button (T14.16), which is a person looking at the
    /// conversation that asked.
    Panel,
}

impl Surface {
    /// The wire name, for the event log.
    ///
    /// Written out rather than derived from the variant, so renaming a variant
    /// cannot silently rewrite the history of a log people grep.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ControlPlane => "control_plane",
            Self::Panel => "panel",
        }
    }
}

/// One answer: what was decided, and which surface carried it.
///
/// Travels as a unit down the [`park`] channel rather than as two values,
/// because a decision whose surface went missing somewhere in the plumbing is
/// exactly the audit gap T14.17 exists to close.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Answer {
    pub decision: Decision,
    pub surface: Surface,
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
    /// Warp's own id for the conversation this request belongs to.
    ///
    /// **T14.16, and it is what lets the panel draw the question next to the
    /// conversation that asked it.** The control plane addresses a request by
    /// `approval_id` and never needs this; a panel does, because it is showing
    /// one conversation and has to decide whether *this* request is that
    /// conversation's. Every other id here answers a different question:
    /// `session_id` is the agent's, and `session_directory` is where Warp told
    /// it to work.
    pub conversation_id: String,
    /// The paths this tool call said it would touch, recovered by joining the
    /// permission request to the notification stream on `toolCallId`.
    ///
    /// **Not the same fact as [`Self::session_directory`], and the difference is
    /// the point.** The session directory is Warp's own — it chose it and sent
    /// it. This is the agent's claim about where *this call* acts, and measured
    /// on T14.6 the permission request drops it: the request arrived with
    /// `locations: []` while the `tool_call_update` for the same call, moments
    /// earlier, carried the path. Empty means the agent never said, which a
    /// surface must render as unknown rather than fill in from the session
    /// directory — substituting one for the other is exactly the invented
    /// certainty T14.3 forbids.
    pub acts_on: Vec<String>,
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
    /// The option id a **yes** would select, or `None` if this entry cannot be
    /// approved.
    ///
    /// **Decided once, here, while the real `RequestPermissionRequest` is still
    /// in hand** — by the shared `acp_permission::choose`, plus a check that the
    /// entry actually shows what the call is. Freezing it is what makes the
    /// listing's `can_approve` and the answer path the *same* computation rather
    /// than two that agree today: T14.6's own console bug was a listing and an
    /// answer disagreeing about approvability, and this is that shape removed at
    /// the root rather than patched at both ends.
    ///
    /// It carries the wire id and not the option's name deliberately. The name
    /// is what a person reads and is already in [`Self::options_offered`]; the
    /// id is what actually goes back to the agent, so it is the thing an answer
    /// has to be bound to.
    pub approve_selects: Option<String>,
    /// Why a yes is refused, when [`Self::approve_selects`] is `None`.
    ///
    /// Written by `acp_permission` for a person to read — it names the tool kind
    /// it could not bound, or lists what the agent offered instead.
    pub approve_refused_because: Option<String>,
}

/// A parked request, and the way to wake it.
struct Parked {
    request: ParkedRequest,
    answer: oneshot::Sender<Answer>,
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
pub(crate) fn park(request: ParkedRequest) -> (Waiting, oneshot::Receiver<Answer>) {
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

/// The requests a single conversation is blocked on, oldest first.
///
/// Separate from [`waiting`] rather than filtered by the caller, because the
/// panel asks this on every render of a conversation and the whole-process list
/// is the wrong shape for that question — and because a caller filtering by hand
/// is a caller that can filter by the wrong field. There are three ids on a
/// `ParkedRequest` and only one of them answers "is this mine".
pub(crate) fn waiting_for(conversation_id: &str) -> Vec<ParkedRequest> {
    waiting()
        .into_iter()
        .filter(|parked| parked.conversation_id == conversation_id)
        .collect()
}

/// Everything currently waiting on a person, oldest key first so a caller
/// polling it does not get a reshuffled list between calls.
///
/// **This paragraph spent an unknown period attached to [`waiting_for`]**, one
/// missing blank line above it, which left that function documented as returning
/// everything when it filters to one conversation — and left this one, the
/// whole-process list that `agent.approvals` serves, with no doc at all. Found by
/// an agent in Warp's own panel, scoped to this file. A doc comment can be wrong
/// by being in the wrong place, and nothing in the toolchain says so.
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
pub(crate) fn answer(approval_id: &str, decision: Decision, surface: Surface) -> bool {
    let Some(parked) = registry().remove(approval_id) else {
        return false;
    };
    // The receiver is gone only if the turn ended between the lookup and here,
    // in which case the request is moot and there is nothing to report.
    parked.answer.send(Answer { decision, surface }).is_ok()
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
