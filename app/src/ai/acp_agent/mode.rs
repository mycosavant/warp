//! What mode a session is in, said out loud (`.fork/TASKS.md`, T14.18).
//!
//! # The hole this fills
//!
//! Warp sent `NewSessionRequest::new(cwd)` and never sent `session/set_mode`,
//! so a panel session ran in whatever mode the agent chose for itself, for the
//! session's whole life, with no surface to see it or change it. Measured
//! 2026-08-30 against `claude-agent-acp` 0.70.0, calibrated in both directions
//! with the same prompt and a client that denied everything it was asked:
//!
//! ```text
//! panel's session shape, no set_mode   started `auto`   0 requests   file written
//! calibration, set_mode(default) first started `auto`   2 requests   no file
//! ```
//!
//! Zero. The agent's default mode is `auto`, which it describes as *"Use a
//! model classifier to approve/deny permission prompts"*, and in it the fork's
//! entire consent architecture — the allowlist, the parked request, the panel
//! button T14.16 built — is never consulted, because nothing ever asks. Run two
//! is what makes run one's zero evidence rather than silence: it fires on the
//! known-present, and both requests it raised are kinds the allowlist admits.
//!
//! So the fork's permission model was not too tight. It was **unreached**.
//!
//! # Why Warp does not simply pick a mode
//!
//! The obvious fix — request `default`, the mode where the agent asks — is the
//! error this fork documented hours earlier, in the commit that surveyed both
//! agents' modes: **do not generalise a mode id across agents.** `SessionModeId`
//! is an opaque `Arc<str>`, and the protocol's own example set for
//! `session/set_mode` is `"ask"`, `"architect"`, `"code"` — not one of which is
//! `default`. `opencode` advertises no modes at all, and its nearest control
//! selects a *persona* (`build`/`plan`) while `claude-agent-acp`'s selects
//! permission policy. Warp cannot tell which mode makes an agent ask, and a
//! built-in default would be one vendor's vocabulary imposed on every other.
//!
//! It is worth naming what this gives up, because the argument for picking one
//! was not stupid. Moving a session from `auto` to `default` **narrows** — it
//! can only cause more requests to be raised, never fewer — and this fork
//! already uses that asymmetry to let `agent.deny` work with no switch while
//! `agent.approve` needs `WARP_FORK_REMOTE_APPROVE`. The asymmetry is real. It
//! just does not apply, because Warp cannot identify the narrowing move without
//! reading a vendor's word as if it were the protocol's. A safe direction is no
//! help when you cannot tell which way you are facing.
//!
//! # So: disclose always, request only when told
//!
//! [`Decision::of`] is the whole policy. Warp reports the mode a session
//! started in, **quoting the agent's own `description` verbatim** — the same
//! rule `acp_permission::Declaration::Changes` keeps, and for the same reason:
//! Warp cannot see an agent's permission policy, so the only honest thing it
//! can say about one is what the agent itself put on the wire. It never
//! paraphrases a mode, never ranks modes, and never says a mode is safe or
//! unsafe.
//!
//! A person who wants a different mode names it in `WARP_FORK_ACP_MODE`, and
//! Warp sends `session/set_mode` only for an id the agent actually advertised.
//! An id it did not advertise is reported rather than sent: the spec requires
//! the mode be one of `availableModes`, and a JSON-RPC error naming a method is
//! not a sentence anyone can act on — the same judgement `mod.rs` already makes
//! about asking a non-resuming agent to resume.
//!
//! # An agent may change its own mode, and that is disclosed too
//!
//! The spec says agents "may also change modes autonomously and notify the
//! client via `current_mode_update`". A mode change Warp did not ask for is the
//! same hazard returning by a different door — the session's policy moves and
//! nobody is told — so [`changed`] writes the same kind of note from the
//! notification. This is the `switch_mode` lesson in a third place: T14.4
//! measured a policy change arriving as an ordinary-looking permission option,
//! and the general rule it produced is that a transition must be visible
//! wherever it can happen, not only where it was expected.
//!
//! # What this is not
//!
//! **It is not a claim that Warp is now in the loop.** In a mode where the
//! agent does not ask, Warp still sees nothing and still decides nothing; the
//! only thing that changed is that the person is told so. Reporting a mode is
//! disclosure, not control, and a note that read as reassurance would be worse
//! than no note at all.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use agent_client_protocol::schema::v1::{SessionMode, SessionModeId, SessionModeState};

/// The mode each conversation was last told about.
///
/// **Process-global, like [`super::liveness`]'s map and for the same reason:**
/// a turn's translator lives and dies with the turn, and this fact has to
/// outlive one. Keyed on Warp's conversation id.
static DISCLOSED: LazyLock<Mutex<HashMap<String, SessionModeId>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Whether this conversation still needs telling, recording that it was told.
///
/// **Every turn after the first resumes with `session/load`, whose reply
/// carries `modes` exactly like `session/new`'s.** Disclosing from that
/// unconditionally would put the same paragraph above every turn of a long
/// conversation — which is the failure [`Decision::NothingToSay`] already
/// argues against one case over: a note that always appears is a note nobody
/// reads, and this one has to be read on the turn where the answer changes.
///
/// So the rule is *say it when it is news*: the first turn of a conversation,
/// and any turn where the mode is not the one last disclosed. A mode that
/// changed back is news again, because what matters is what is true now and
/// whether the person has been told it.
fn needs_telling(conversation_id: &str, current: &SessionModeId) -> bool {
    let Ok(mut disclosed) = DISCLOSED.lock() else {
        // A poisoned lock means some other turn panicked mid-update. Saying it
        // again is the harmless direction; going quiet about a session's policy
        // because of an unrelated panic is not.
        return true;
    };
    match disclosed.get(conversation_id) {
        Some(told) if told == current => false,
        _ => {
            disclosed.insert(conversation_id.to_owned(), current.clone());
            true
        }
    }
}

/// Forget a conversation, so a test does not inherit another's disclosure.
#[cfg(test)]
fn forget(conversation_id: &str) {
    if let Ok(mut disclosed) = DISCLOSED.lock() {
        disclosed.remove(conversation_id);
    }
}

/// What to do about a session's modes, and what to say about them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Decision {
    /// The agent advertises no modes, so there is nothing to disclose and
    /// nothing to ask for.
    ///
    /// **Silent on purpose.** `opencode` is this case, and a note on every
    /// session saying an agent has no modes is noise that trains a person to
    /// skip the notes that matter. Absence of a mode system is not a hazard; an
    /// undisclosed mode is.
    NothingToSay,
    /// Modes exist. Warp asked for nothing, and says which one is in force.
    Disclose { note: String },
    /// Modes exist and the person named one the agent advertises. Send it, then
    /// say what was asked for.
    Request { mode: SessionModeId, note: String },
    /// The same request, on a later turn of a conversation already told.
    ///
    /// Separate from [`Self::Request`] rather than a `note: Option<String>`,
    /// because the two differ in what they mean and not only in what they
    /// print: this one is the steady state of a long conversation, and reading
    /// it in a match should say so.
    RequestQuietly { mode: SessionModeId },
}

impl Decision {
    /// The note to show, if any.
    pub(crate) fn note(&self) -> Option<&str> {
        match self {
            Self::NothingToSay | Self::RequestQuietly { .. } => None,
            Self::Disclose { note } | Self::Request { note, .. } => Some(note),
        }
    }

    /// The mode to send `session/set_mode` for, if any.
    pub(crate) fn mode(&self) -> Option<&SessionModeId> {
        match self {
            Self::Request { mode, .. } | Self::RequestQuietly { mode } => Some(mode),
            _ => None,
        }
    }

    /// Decide from what the agent advertised and what the person asked for.
    ///
    /// `modes` is `None` when the agent sent no `modes` field at all, which is
    /// the same case as advertising an empty list: nothing to disclose.
    /// `wanted` is [`crate::fork::acp_mode`], threaded rather than read here so
    /// this is testable without touching the process environment.
    pub(crate) fn of(
        conversation_id: &str,
        modes: Option<&SessionModeState>,
        wanted: Option<&str>,
    ) -> Self {
        let Some(state) = modes.filter(|state| !state.available_modes.is_empty()) else {
            return Self::NothingToSay;
        };
        // Asked before the sentences are built, and it *records* -- so two
        // decisions for one unchanged mode disclose once. The mode request
        // below is not gated on this: `session/set_mode` is idempotent and a
        // resumed session may have come back in a different mode than it left,
        // so it is re-sent every turn while only the telling is rationed.
        let news = needs_telling(conversation_id, &state.current_mode_id);
        let current = describe_current(state);
        let offered = offered_list(state);
        match wanted {
            None if !news => Self::NothingToSay,
            None => Self::Disclose {
                note: format!(
                    "{current} Warp did not choose it and cannot tell what it permits — a mode is \
                     the agent's own idea, so only the agent's description above says anything \
                     about it. To ask for a different one, set `WARP_FORK_ACP_MODE` to its id \
                     before starting Warp. This agent offers: {offered}."
                ),
            },
            Some(asked) => match state
                .available_modes
                .iter()
                .find(|mode| mode.id.0.as_ref() == asked)
            {
                Some(mode) if !news => Self::RequestQuietly {
                    mode: mode.id.clone(),
                },
                Some(mode) => Self::Request {
                    mode: mode.id.clone(),
                    note: format!(
                        "{current} `WARP_FORK_ACP_MODE` asks for {}, so Warp is requesting it. \
                         Whether the agent honours the request is the agent's to answer.",
                        described(mode)
                    ),
                },
                // Reported rather than sent. The spec requires the id be one of
                // `availableModes`, so sending it anyway buys a protocol error
                // in place of a sentence; and failing *silently* would leave a
                // person believing a mode was requested when it was not, which
                // is the worse of the two by the margin this whole module is
                // about.
                None if !news => Self::NothingToSay,
                None => Self::Disclose {
                    note: format!(
                        "{current} `WARP_FORK_ACP_MODE` asks for `{asked}`, which this agent does \
                         not offer, so nothing was requested and the mode above is the one in \
                         force. This agent offers: {offered}."
                    ),
                },
            },
        }
    }
}

/// The note for a mode change the agent made on its own.
///
/// Deliberately says *the agent changed it*, because that is the fact worth
/// having: the same sentence about a change Warp requested would let a person
/// read their own instruction back as the agent acting unprompted. Warp's own
/// requests are announced by [`Decision::of`] at the point they are made.
///
/// **`current_mode_update` carries the id and nothing else** — no name, no
/// description — so the description has to come from what the agent advertised
/// when the session opened, which is why the translator remembers that list.
/// Without it this note could only say *"the mode is now `dontAsk`"*, and an
/// id is exactly the thing a person cannot interpret: `dontAsk` and `auto` and
/// `bypassPermissions` are all just words until the agent's own sentence about
/// them is attached.
///
/// `advertised` is `None` when the session opened without a mode list, in which
/// case the id is all there is and the note says so rather than pretending
/// otherwise.
pub(crate) fn changed(advertised: Option<&SessionModeState>, now: &SessionModeId) -> String {
    let described = advertised
        .and_then(|state| state.available_modes.iter().find(|mode| &mode.id == now))
        .map(described)
        .unwrap_or_else(|| {
            format!(
                "the agent's `{}` mode, which it did not list when this session opened, so there                  is no description of it to show",
                now.0
            )
        });
    format!("The agent changed this session's mode on its own. It is now {described}.")
}

/// *"in `auto` mode, which the agent describes as \"…\""* — or without the
/// description, when the agent gave none.
///
/// A mode the agent advertises no description for is reported by id and nothing
/// else. Inventing one from the id would be Warp explaining a policy it cannot
/// see, and an empty pair of quotes reads as though the agent said something
/// blank.
fn describe_current(state: &SessionModeState) -> String {
    let current = state
        .available_modes
        .iter()
        .find(|mode| mode.id == state.current_mode_id);
    match current {
        Some(mode) => format!("This session is in {}.", described(mode)),
        // The advertised list did not contain the current mode. Nothing in the
        // spec forbids that, and guessing which of the others it resembles
        // would be worse than saying only what arrived.
        None => format!(
            "This session is in the agent's `{}` mode, which the agent did not include in the \
             modes it listed, so there is no description of it to show.",
            state.current_mode_id.0
        ),
    }
}

/// One mode, as *``id`` mode, which the agent describes as "…"*.
fn described(mode: &SessionMode) -> String {
    match mode.description.as_deref().map(str::trim) {
        Some(description) if !description.is_empty() => format!(
            "the agent's `{}` mode, which the agent describes as \"{description}\"",
            mode.id.0
        ),
        _ => format!(
            "the agent's `{}` mode, for which the agent gave no description",
            mode.id.0
        ),
    }
}

/// Every id the agent offered, so a person choosing one does not have to guess
/// its spelling — and so the note carries the evidence for its own claim about
/// what is available.
fn offered_list(state: &SessionModeState) -> String {
    state
        .available_modes
        .iter()
        .map(|mode| format!("`{}`", mode.id.0))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
#[path = "mode_tests.rs"]
mod tests;
