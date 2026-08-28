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
//! # …and "out loud" turned out to mean "once" (T14.4)
//!
//! T14.3 concluded the agent says its mode out loud. Measured again once Warp
//! could *ask*: it says it out loud **at `session/new`**, and re-announces when a
//! person asks it in prose to switch — but sending `session/set_mode(plan)`
//! produced an acknowledgement, plan-mode behaviour, and **no
//! `CurrentModeUpdate` at all**. The report still read `auto`.
//!
//! So Warp gets *less* sighted the more it participates, and the field that
//! tracked "the mode" was wrong in exactly the direction this module exists to
//! prevent. It is gone. What is left is three separate wire-facts —
//! what was declared at the start, what was announced since, what Warp asked for
//! — and no field that a reader can mistake for the mode the session is in.
//!
//! # A ledger that laundered its own action
//!
//! Worse, and from the same runs. `--approve` selected the option *"Yes, and
//! manually approve edits"* on an `ExitPlanMode` request (see
//! `acp_permission.rs`), and the agent then announced
//! `{"from":"auto","to":"default"}` — which this file recorded as
//! `warp_requested_it: false`, documented as *"the agent widening or narrowing
//! itself"*. Warp caused that change and the ledger attributed it to the agent.
//!
//! The field is now named for the message Warp sent rather than for who moved
//! the mode, because that is the part Warp can actually check. `choose` refusing
//! those options puts the case out of reach from this binary, which is a reason
//! to fix the record rather than a reason not to: the next surface that can
//! answer a policy question will reach it again.
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
    /// What `session/new` said about modes, **as received and never amended**.
    ///
    /// `Option` because `NewSessionResponse.modes` is optional in the schema. An
    /// agent that declares nothing is a **third state**, not an ungoverned one,
    /// and the report says so rather than filling the gap with a guess.
    ///
    /// Kept unmodified since the measurement in the module docs: overwriting
    /// `current_mode_id` on every announcement produced a field that read as *the
    /// mode the session is in*, which Warp does not know. What it knows is the
    /// opening declaration and the announcements since, and those are two facts,
    /// not one.
    mode: Option<SessionModeState>,
    /// Insertion-ordered rather than a map: a prompt turn has a handful of tool
    /// calls, and the order they happened in is the order a person wants to read.
    calls: Vec<Call>,
    /// Every declared policy change that was offered, and what Warp did with it.
    transitions: Vec<Transition>,
    /// Mode changes the agent announced, in order.
    announced_mode_changes: Vec<ModeChange>,
    /// Mode changes Warp asked for, in order.
    mode_requests: Vec<ModeRequest>,
}

/// A `session/set_mode` Warp sent, and everything that came back.
///
/// Kept as its own record because **an acknowledgement is nearly empty**:
/// `SetSessionModeResponse` has no fields at all, so a successful reply carries
/// exactly one bit — no error. Whether the agent then behaved differently is not
/// on the wire, and the only follow-up evidence that can exist is a
/// [`SessionUpdate::CurrentModeUpdate`] naming the mode. So the two are recorded
/// separately and a reader can see which of them happened.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(super) struct ModeRequest {
    pub mode_id: String,
    /// The agent answered without an error. That is the whole content of an
    /// acknowledgement — see the type docs — and it is **not** evidence the mode
    /// took effect.
    pub the_agent_acknowledged: bool,
    /// The agent afterwards announced this mode as current.
    ///
    /// The strongest evidence available, and still only the agent's own claim
    /// about itself. Absent this, an acknowledgement is all there is.
    pub the_agent_announced_it_afterwards: bool,
}

/// A mode change the agent announced mid-session.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub(super) struct ModeChange {
    /// The mode the agent had last claimed, or `null` if it never claimed one.
    pub from: Option<String>,
    pub to: String,
    /// The agent's own description of the mode it moved to, if it gave one.
    pub description_from_the_agent: Option<String>,
    /// Whether this change answers a `session/set_mode` Warp sent and no earlier
    /// announcement has already been credited to.
    ///
    /// **Named for the message Warp sent, not for who caused the change, and that
    /// is a correction.** This field was `warp_requested_it`, documented as
    /// *"a change with this `false` is the agent widening or narrowing itself"* —
    /// and then a run measured `{"from":"auto","to":"default",
    /// "warp_requested_it":false}` arriving immediately after Warp had itself
    /// selected the option *"Yes, and manually approve edits"* on a `switch_mode`
    /// permission request. So `false` was printing the rug-pull sentence over
    /// Warp's own doing. A ledger that launders its own action as the agent's is
    /// worse than no ledger.
    ///
    /// `acp_permission::choose` now refuses those options, which puts that case
    /// out of reach *from this binary* — and the field is still renamed, because
    /// the next surface to answer a policy question will reach it again and the
    /// record has to have been right before it does.
    ///
    /// What remains is an inference, and the only one in this file: nothing on the
    /// wire links a `CurrentModeUpdate` to the request that may have caused it, so
    /// this matches on the mode id and consumes the request — which is why a
    /// second, unsolicited change back to the same mode reads `false`.
    pub answers_a_set_mode_warp_sent: bool,
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

/// How an agent disclosed what an option would do beyond answering the question.
///
/// Three states rather than a `readable` boolean, because a boolean could not
/// tell *"there is a declaration and this build cannot parse it"* from *"there is
/// no declaration at all"* — and after the `switch_mode` measurement the second
/// is the common case, not the rare one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Disclosure {
    /// `_meta.permission.changes`, at a version this build reads. `declared`
    /// holds it verbatim.
    ADeclarationThisBuildCanRead,
    /// A `permission` block in a layout this build does not know. The absence of
    /// detail is the finding, so the entry is kept rather than dropped.
    ADeclarationThisBuildCannotRead,
    /// Nothing structured. The agent said what the option does **in the option's
    /// name**, in English — which is disclosure to a person reading a card and
    /// nothing at all to a program. The measured `ExitPlanMode` menu is entirely
    /// this.
    TheOptionsNameOnly,
}

/// A policy change an agent offered to make, and what Warp did about it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub(super) struct Transition {
    pub tool_call_id: String,
    /// The option that carried it, by the agent's own name for it.
    pub option_name: String,
    /// The declaration verbatim, or `null` when there is nothing structured to
    /// quote. Which of those it is, is `disclosed_as`.
    pub declared: Option<serde_json::Value>,
    /// How the agent said what this option would do.
    pub disclosed_as: Disclosure,
    /// Whether Warp selected the option. Always false today; printed anyway,
    /// because "nothing was authorized" is exactly the claim worth evidencing.
    pub authorized_by_warp: bool,
}

/// The whole of what Warp can honestly say about one session.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub(super) struct Report {
    /// The mode id the agent declared in its `session/new` response, or `null` if
    /// it declared none.
    ///
    /// **The agent's opening claim, not Warp's finding, and not the mode the
    /// session is in.** There used to be a `mode_the_agent_declared` here that
    /// tracked announcements and was read as the current mode. It was measured
    /// wrong: Warp sent `session/set_mode(plan)`, the agent acknowledged and
    /// behaved accordingly — it wrote a plan and then asked to leave plan mode —
    /// and announced nothing, so the field still said `auto` for a session
    /// demonstrably in `plan`.
    ///
    /// So there is no current-mode field any more. Warp knows what was declared at
    /// the start, what was announced since, and what it asked for; the current
    /// mode is not among them, and a reader composing it from these three can at
    /// least see where it is uncertain.
    pub mode_the_agent_declared_at_session_start: Option<String>,
    /// The agent's own description of that opening mode, quoted.
    pub its_description_from_the_agent: Option<String>,
    /// Mode changes the agent announced after the session started.
    ///
    /// Printed even when empty, since "the agent did not re-declare itself" is the
    /// claim this field exists to evidence. **An empty list is not evidence the
    /// mode never moved** — see the field above, where it moved silently.
    pub mode_changes_the_agent_announced: Vec<ModeChange>,
    /// Mode changes Warp asked for, and what evidence came back that they landed.
    ///
    /// Printed even when empty, because a reader has no other way to tell a
    /// session Warp stayed out of from one where it asked and was ignored.
    pub mode_requests_warp_sent: Vec<ModeRequest>,
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
                      shell command. Nor is it necessarily current: an agent that honours a \
                      `session/set_mode` need not announce it, and one measured session ran in \
                      `plan` while its last declaration still said `auto`.";

impl Ledger {
    pub(super) fn new() -> Self {
        Self::default()
    }

    /// Record what `session/new` said about modes.
    pub(super) fn observe_session(&mut self, modes: Option<SessionModeState>) {
        self.mode = modes;
    }

    /// Record that Warp is about to send `session/set_mode`.
    ///
    /// Before rather than after, and that is not tidiness. Nothing orders a
    /// notification against a response, so the agent's `CurrentModeUpdate` may
    /// arrive while the `set_mode` call is still outstanding; a request recorded
    /// afterwards would miss its own answer and the change Warp asked for would be
    /// reported as the agent widening itself. Recording first cannot lose that
    /// race in either direction.
    pub(super) fn observe_mode_request(&mut self, mode_id: &str) {
        self.mode_requests.push(ModeRequest {
            mode_id: mode_id.to_owned(),
            the_agent_acknowledged: false,
            the_agent_announced_it_afterwards: false,
        });
    }

    /// Record that the agent answered a `session/set_mode` without an error.
    pub(super) fn observe_mode_acknowledgement(&mut self, mode_id: &str) {
        if let Some(request) = self
            .mode_requests
            .iter_mut()
            .find(|request| request.mode_id == mode_id && !request.the_agent_acknowledged)
        {
            request.the_agent_acknowledged = true;
        }
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
                let to = update.current_mode_id.to_string();
                let from = self
                    .announced_mode_changes
                    .last()
                    .map(|change| change.to.clone())
                    .or_else(|| {
                        self.mode
                            .as_ref()
                            .map(|mode| mode.current_mode_id.to_string())
                    });
                let description = self.description_of(&to);
                // Credited to the oldest un-answered request for this mode, if
                // there is one. Telling the two apart is the point: "you were
                // asked and you agreed" is a different sentence from "the agent
                // widened itself and said so", and only the second is a rug pull.
                let answers_a_request = self
                    .mode_requests
                    .iter_mut()
                    .find(|request| {
                        request.mode_id == to && !request.the_agent_announced_it_afterwards
                    })
                    .map(|request| {
                        request.the_agent_announced_it_afterwards = true;
                    })
                    .is_some();
                self.announced_mode_changes.push(ModeChange {
                    from,
                    to,
                    description_from_the_agent: description,
                    answers_a_set_mode_warp_sent: answers_a_request,
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
            .filter(|option| acp_permission::is_more_than_an_answer(request, option))
            .map(|option| {
                let (declared, disclosed_as) = match acp_permission::declaration(option) {
                    Declaration::Changes(changes) => (
                        Some(changes.clone()),
                        Disclosure::ADeclarationThisBuildCanRead,
                    ),
                    Declaration::UnknownVersion => {
                        (None, Disclosure::ADeclarationThisBuildCannotRead)
                    }
                    Declaration::None => (None, Disclosure::TheOptionsNameOnly),
                };
                Transition {
                    tool_call_id: request.tool_call.tool_call_id.to_string(),
                    option_name: option.name.clone(),
                    declared,
                    disclosed_as,
                    authorized_by_warp: false,
                }
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
        let opening = self
            .mode
            .as_ref()
            .map(|mode| mode.current_mode_id.to_string());
        Report {
            its_description_from_the_agent: opening
                .as_deref()
                .and_then(|opening| self.description_of(opening)),
            mode_the_agent_declared_at_session_start: opening,
            mode_changes_the_agent_announced: self.announced_mode_changes.clone(),
            mode_requests_warp_sent: self.mode_requests.clone(),
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

    /// The agent's own description of one of the modes it listed, if it gave one.
    fn description_of(&self, mode_id: &str) -> Option<String> {
        self.mode
            .as_ref()?
            .available_modes
            .iter()
            .find(|mode| mode.id.to_string() == mode_id)?
            .description
            .clone()
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
