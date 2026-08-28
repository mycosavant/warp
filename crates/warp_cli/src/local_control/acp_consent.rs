//! What Warp actually knows about who consented (`.fork/TASKS.md`, T14.3).
//!
//! # The ticket said Warp is blind. It is not, and running it is what showed that
//!
//! T14.3 was filed on the finding that `claude-agent-acp` resolves its permission
//! mode from the user's own `~/.claude/settings.json` — a file Warp never reads —
//! and concluded that Warp cannot observe an agent's permission policy at all.
//! That was read off the agent's source. Measured 2026-08-27 on the wire, the
//! agent **says the mode out loud**: `session/new` came back with
//! `modes: {currentModeId: "auto", availableModes: [...]}`, six modes each with
//! the agent's own description, and a matching `configOptions` entry.
//!
//! That is stable v1 — `NewSessionResponse.modes`, its own spec page — not vendor
//! `_meta`, so it is not a Claude-only affordance. The spec's own sentence is
//! *"Modes often affect the system prompts used, the availability of tools, and
//! **whether they request permission before running**."* There is a
//! `SessionUpdate::CurrentModeUpdate` for changes, too.
//!
//! # What is actually invisible is the rules, and the gap is measurable
//!
//! A mode is a coarse dial with the user's own rules underneath it, and the rules
//! are what Warp cannot see. That distinction is not theoretical. In one session
//! at `currentModeId: "default"` — whose own description is *"Standard behavior,
//! prompts for dangerous operations"* — a prompt to write a file and run
//! `echo done` produced **two tool calls and one permission request**: the write
//! was put to Warp, the command was not. Same session, same mode, opposite
//! outcomes.
//!
//! So the mode **does not predict whether any given call will be gated**, and any
//! report that let a reader draw that inference would be the error T14.3 exists
//! to prevent, merely relocated. Hence:
//!
//! - The mode is reported as **the agent's claim**, quoted, never as a Warp
//!   finding and never as a prediction.
//! - Gating is reported **per call**, and only ever as a count of the requests
//!   this process received and a list of the answers it sent — facts about
//!   Warp's own inbox, which are the only things here that are certainly true.
//!   A count rather than a label, because a label is a verdict: *"unasked"* and
//!   *"ungoverned"* are inferences about the agent, and only the arithmetic is an
//!   observation.
//! - The two are never combined into a verdict.
//!
//! # "Not asked" is not "not approved", and the difference is the whole point
//!
//! The user's settings are the user's own expressed policy, and an agent
//! honouring them is precisely this fork's thesis — the `echo done` above was
//! allowed by a rule its user wrote. A call Warp was not asked about was very
//! probably consented to in advance, by the person, deliberately. What Warp can
//! say is only that **it was not the one who was asked**. Saying more would claim
//! knowledge of a file it never opened, which is the third instance of one
//! principle: `approvals.rs` reports which keystroke it sent rather than
//! `approved: true`, and `local_agent/tools.rs` refuses an allowlist it cannot
//! enforce.
//!
//! # Transitions, which are the one thing Warp is genuinely sighted on
//!
//! Warp cannot know the policy state, but it knows exactly what it was *asked to
//! authorize*, because that arrives as an option it either selected or refused.
//! Today `acp_permission::choose` refuses every option carrying a declared change,
//! so the authorized list is always empty and the refused list is the interesting
//! one — and an empty authorized list is worth printing precisely because it is
//! the claim a person would otherwise have to take on trust.

use agent_client_protocol::schema::v1::{
    RequestPermissionRequest, SessionModeState, SessionUpdate, ToolCallId, ToolCallStatus, ToolKind,
};

use super::acp_permission::{self, Declaration};

/// What Warp observed, in the order it observed it.
#[derive(Debug, Default)]
pub(super) struct Ledger {
    /// The mode the agent declared, if it declared one.
    ///
    /// `Option` because `NewSessionResponse.modes` is optional in the schema. An
    /// agent that declares nothing is a **third state**, not an ungoverned one,
    /// and the report says so rather than filling the gap with a guess.
    mode: Option<SessionModeState>,
    /// Insertion-ordered rather than a map: a prompt turn has a handful of tool
    /// calls, and the order they happened in is the order a person wants to read.
    calls: Vec<Call>,
    /// Every declared policy change that was offered, and what Warp did with it.
    transitions: Vec<Transition>,
    /// Mode changes the agent announced, in order.
    announced_mode_changes: Vec<ModeChange>,
}

/// A mode change the agent announced mid-session.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(super) struct ModeChange {
    /// The mode the agent had last claimed, or `null` if it never claimed one.
    pub from: Option<String>,
    pub to: String,
    /// Whether this change is one Warp asked for.
    ///
    /// `session/set_mode` exists in the schema, so a future surface can request a
    /// mode; nothing here does. A change with this `false` is the agent widening
    /// or narrowing itself and saying so, which is a wire-fact worth keeping
    /// separate from a change Warp participated in.
    pub warp_requested_it: bool,
}

/// One tool call, and whether this process was asked about it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(super) struct Call {
    pub tool_call_id: String,
    /// The agent's own human-readable title. Free text, and reported as such.
    pub title: Option<String>,
    /// The agent's own category for the call.
    ///
    /// Reported because a person reading a list wants it, and never branched on:
    /// `ToolKind` is agent-authored and its unknown case silently becomes `Other`
    /// through `#[serde(other)]`, so anything hung on it degrades silently.
    pub kind: Option<String>,
    pub status: Option<String>,
    /// How many `session/request_permission` calls for this id reached this
    /// process.
    ///
    /// **A count and not a boolean, and that is a correction rather than a
    /// preference.** T14.2 withdrew the claim that a `toolCallId` binds an answer
    /// to its question, on the grounds that nothing in the schema forbids an
    /// agent re-asking on the same id after a refusal — and then this struct
    /// shipped a `bool`, which would have recorded the second ask as the first
    /// and dropped its answer. Writing a hazard down does not implement it.
    ///
    /// Named for Warp's inbox on purpose. A zero is a count of what this process
    /// received; grammatically it cannot be a statement about the agent, and it
    /// does **not** mean unapproved, unauthorized or bypassed. See the module
    /// docs.
    pub permission_requests_received: usize,
    /// What Warp sent back, in order, one entry per request received.
    pub answers_warp_sent: Vec<String>,
}

/// A policy change an agent offered to make, and what Warp did about it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub(super) struct Transition {
    pub tool_call_id: String,
    /// The option that carried it, by the agent's own name for it.
    pub option_name: String,
    /// The declaration verbatim, or `null` when the agent used a layout this
    /// build cannot read — in which case `readable` is false and the absence of
    /// detail is the finding.
    pub declared: Option<serde_json::Value>,
    pub readable: bool,
    /// Whether Warp selected the option. Always false today; printed anyway,
    /// because "nothing was authorized" is exactly the claim worth evidencing.
    pub authorized_by_warp: bool,
}

/// The whole of what Warp can honestly say about one session.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub(super) struct Report {
    /// The mode id the agent declared, or `null` if it declared none.
    ///
    /// **The agent's claim, not Warp's finding.** Warp cannot verify it and does
    /// not act on it.
    pub mode_the_agent_declared: Option<String>,
    /// The agent's own description of that mode, quoted.
    pub mode_description_from_the_agent: Option<String>,
    /// Mode changes the agent announced after the session started.
    ///
    /// Empty in every run measured so far — which is why it is printed rather
    /// than omitted, since "the agent did not re-declare itself" is the claim
    /// this field exists to evidence.
    pub mode_changes_the_agent_announced: Vec<ModeChange>,
    pub calls: Vec<Call>,
    /// How many calls this process was never asked about.
    ///
    /// A count of Warp's own silence. It is **not** a count of unapproved
    /// actions: a call Warp was not asked about was most likely permitted in
    /// advance by a rule its user wrote, which Warp cannot see and does not
    /// second-guess.
    pub calls_warp_was_not_asked_about: usize,
    pub transitions_offered: Vec<Transition>,
    /// Always empty while `acp_permission::choose` refuses every declared change.
    pub transitions_authorized_by_warp: Vec<Transition>,
    /// The sentence to print above the rest, so the numbers are read the way they
    /// are meant.
    pub caveat: &'static str,
}

/// Why the counts above mean less than they look like they mean.
const CAVEAT: &str = "`permission_requests_received: 0` means only that no permission request for \
                      this call reached Warp. The agent's permission rules live in the user's own \
                      configuration, which Warp does not read; a call it was not asked about was \
                      most likely allowed by a rule the user wrote deliberately. The declared mode \
                      is the agent's claim and does not predict per-call gating — one measured \
                      session at mode `default` asked about a file write and did not ask about a \
                      shell command.";

impl Ledger {
    pub(super) fn new() -> Self {
        Self::default()
    }

    /// Record what `session/new` said about modes.
    pub(super) fn observe_session(&mut self, modes: Option<SessionModeState>) {
        self.mode = modes;
    }

    /// Record one `SessionUpdate`.
    ///
    /// Only the variants that carry consent-relevant facts are read. The rest are
    /// deliberately ignored rather than matched exhaustively: `SessionUpdate` is
    /// `#[non_exhaustive]`, and a probe that failed to build every time upstream
    /// added a variant would be a probe nobody keeps current.
    pub(super) fn observe_update(&mut self, update: &SessionUpdate) {
        match update {
            SessionUpdate::ToolCall(call) => {
                let entry = self.entry(&call.tool_call_id);
                entry.title = Some(call.title.clone());
                entry.kind = Some(kind_name(call.kind).to_owned());
                entry.status = Some(status_name(call.status).to_owned());
            }
            SessionUpdate::ToolCallUpdate(update) => {
                let fields = update.fields.clone();
                let entry = self.entry(&update.tool_call_id);
                if let Some(title) = fields.title {
                    entry.title = Some(title);
                }
                if let Some(kind) = fields.kind {
                    entry.kind = Some(kind_name(kind).to_owned());
                }
                if let Some(status) = fields.status {
                    entry.status = Some(status_name(status).to_owned());
                }
            }
            // A mode change Warp never asked for is the rug-pull channel, and it
            // is the shape `tool_digest.rs` already watches for: something
            // re-declaring itself after the point at which it was accepted.
            // Overwriting the mode silently would lose the only fact that matters
            // — that the agent widened *itself* — so the announcement is kept as
            // well as applied.
            SessionUpdate::CurrentModeUpdate(update) => {
                let from = self
                    .mode
                    .as_ref()
                    .map(|mode| mode.current_mode_id.to_string());
                let to = update.current_mode_id.to_string();
                if let Some(mode) = self.mode.as_mut() {
                    mode.current_mode_id = update.current_mode_id.clone();
                }
                self.announced_mode_changes.push(ModeChange {
                    from,
                    to,
                    // This probe never sends `session/set_mode`, so every change
                    // it sees is one it did not ask for. The field exists because
                    // a surface that *can* request one must be able to tell the
                    // two apart: "you were asked and you agreed" is a different
                    // sentence from "the agent widened itself and said so".
                    warp_requested_it: false,
                });
            }
            _ => {}
        }
    }

    /// Record that a permission request for a call reached this process, and what
    /// the agent offered to change if it were answered yes.
    pub(super) fn observe_request(&mut self, request: &RequestPermissionRequest) {
        let transitions = request
            .options
            .iter()
            .filter_map(|option| {
                let (declared, readable) = match acp_permission::declaration(option) {
                    Declaration::None => return None,
                    Declaration::Changes(changes) => (Some(changes.clone()), true),
                    Declaration::UnknownVersion => (None, false),
                };
                Some(Transition {
                    tool_call_id: request.tool_call.tool_call_id.to_string(),
                    option_name: option.name.clone(),
                    declared,
                    readable,
                    authorized_by_warp: false,
                })
            })
            .collect::<Vec<_>>();
        self.transitions.extend(transitions);
        self.entry(&request.tool_call.tool_call_id)
            .permission_requests_received += 1;
    }

    /// Record what Warp answered, and which option it picked if it picked one.
    pub(super) fn observe_answer(
        &mut self,
        tool_call_id: &ToolCallId,
        answer: &str,
        selected_option_name: Option<&str>,
    ) {
        self.entry(tool_call_id)
            .answers_warp_sent
            .push(answer.to_owned());
        let Some(name) = selected_option_name else {
            return;
        };
        for transition in &mut self.transitions {
            if transition.tool_call_id == tool_call_id.to_string() && transition.option_name == name
            {
                transition.authorized_by_warp = true;
            }
        }
    }

    /// Everything Warp can honestly say, and nothing else.
    pub(super) fn report(&self) -> Report {
        let current = self.mode.as_ref().map(|mode| mode.current_mode_id.clone());
        let description = current.as_ref().and_then(|current| {
            self.mode
                .as_ref()?
                .available_modes
                .iter()
                .find_map(|mode| (&mode.id == current).then(|| mode.description.clone())?)
        });
        Report {
            mode_the_agent_declared: current.map(|id| id.to_string()),
            mode_description_from_the_agent: description,
            mode_changes_the_agent_announced: self.announced_mode_changes.clone(),
            calls_warp_was_not_asked_about: self
                .calls
                .iter()
                .filter(|call| call.permission_requests_received == 0)
                .count(),
            calls: self.calls.clone(),
            transitions_authorized_by_warp: self
                .transitions
                .iter()
                .filter(|transition| transition.authorized_by_warp)
                .cloned()
                .collect(),
            transitions_offered: self.transitions.clone(),
            caveat: CAVEAT,
        }
    }

    /// The record for a call, created on first sight.
    ///
    /// A permission request can arrive before any `tool_call` notification for the
    /// same id, so this must create as readily as it finds — otherwise the call
    /// Warp *was* asked about is the one missing from the report.
    fn entry(&mut self, tool_call_id: &ToolCallId) -> &mut Call {
        let id = tool_call_id.to_string();
        if let Some(index) = self.calls.iter().position(|call| call.tool_call_id == id) {
            return &mut self.calls[index];
        }
        self.calls.push(Call {
            tool_call_id: id,
            title: None,
            kind: None,
            status: None,
            permission_requests_received: 0,
            answers_warp_sent: Vec::new(),
        });
        self.calls.last_mut().expect("a record was just pushed")
    }
}

/// The wire name of a tool kind. The `_` arm is load-bearing: `ToolKind` is
/// `#[non_exhaustive]`, so an upstream addition must get a name, not a panic.
fn kind_name(kind: ToolKind) -> &'static str {
    match kind {
        ToolKind::Read => "read",
        ToolKind::Edit => "edit",
        ToolKind::Delete => "delete",
        ToolKind::Move => "move",
        ToolKind::Search => "search",
        ToolKind::Execute => "execute",
        ToolKind::Think => "think",
        ToolKind::Fetch => "fetch",
        ToolKind::SwitchMode => "switch_mode",
        ToolKind::Other => "other",
        _ => "a kind this build does not know",
    }
}

fn status_name(status: ToolCallStatus) -> &'static str {
    match status {
        ToolCallStatus::Pending => "pending",
        ToolCallStatus::InProgress => "in_progress",
        ToolCallStatus::Completed => "completed",
        ToolCallStatus::Failed => "failed",
        _ => "a status this build does not know",
    }
}

#[cfg(test)]
#[path = "acp_consent_tests.rs"]
mod tests;
