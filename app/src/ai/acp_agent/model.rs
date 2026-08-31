//! Which model the agent says a person may pick (`.fork/TASKS.md`, T14.14).
//!
//! # The hole this fills
//!
//! `session/new` and `session/load` each reply with `configOptions`, and the
//! fork read them for nothing: the reply's session id was taken, its `modes`
//! were captured for `mode`, and `config_options` was dropped. Measured
//! against live agents the list is real — `opencode` 1.18.25 advertises a
//! model select whose options run to hundreds, `claude-agent-acp` 0.70.0 a
//! `model` select too — so the protocol already offers a model picker and the
//! fork simply did not read it.
//!
//! # It is a generic channel, and the filter is the whole point
//!
//! `configOptions` is not a model picker. It is a generic config channel, and
//! one agent puts the session's *permission mode* in it: the first entry of
//! `claude-agent-acp`'s list is `{"id": "mode", "category": "mode",
//! "type": "select"}` whose options include `dontAsk` and
//! `bypassPermissions`. Rendering it wholesale would ship a control that can
//! set `bypassPermissions` — the exact escalation `acp_permission` refuses as
//! `switch_mode`, arriving through a different door, with no permission
//! request involved at all because a config write is not a
//! `session/request_permission`.
//!
//! So the scope is explicit: **render `category: "model"` and nothing else.**
//! [`Catalog::of`] is the single seam, and a surface holding a `Catalog` is a
//! surface that cannot touch anything the filter rejected: what may be shown
//! comes through [`Catalog::options`], and what may be sent comes through
//! [`Catalog::request`]. Both doors are on the same filtered value, so the
//! send cannot reach an option the render could not show — the `switch_mode`
//! through a generic picker, from the other side.
//!
//! # `None`, and the survey's one live case
//!
//! `category` is optional and `#[serde_as(deserialize_as =
//! "DefaultOnError")]`, so a malformed category deserialises to `None` rather
//! than erroring. The filter is therefore barer than a `matches!` reads:
//! `Some(_)` only ever means a category that actually arrived, and any
//! `None` — absent, or garbage that became absent — is excluded by the same
//! comparison.
//!
//! And absence is not hypothetical. Measured 2026-08-30, `claude-agent-acp`
//! ships an option with **no category at all** (`id: "agent"`). The
//! unknown-must-not-qualify rule was written for a category this build had not
//! read; the measured case is a category that is *absent*. Same answer, and
//! pinned by a test rather than inferred.
//!
//! # What this is not
//!
//! **It is not the whole of `configOptions`, and it must not drift there.**
//! The enum also carries `ModelConfig` ("Model-related configuration
//! parameter") and `ThoughtLevel`; both are deliberately excluded. `Model` is
//! the *selector* — the thing a person chooses between — and `ModelConfig` is
//! a knob behind the selector, which this seam exists not to turn. A filter
//! that admitted it would be a different product, argued into existence one
//! category at a time.
//!
//! **It is not a validation of the choices.** The list is the agent's claim
//! about itself, so it is rendered, never checked, and a selection is passed
//! back verbatim; the only honesty required of Warp is that it show what the
//! agent put on the wire, not that it agree with it.

use agent_client_protocol::schema::v1::{
    SessionConfigId, SessionConfigKind, SessionConfigOption, SessionConfigOptionCategory,
    SessionConfigOptionValue, SessionId, SetSessionConfigOptionRequest,
};

/// The agent's claim about which model is selected and what else may be.
///
/// The only way into this type is [`Catalog::of`], and it is the only source a
/// renderer or a writer is allowed to draw from. It is deliberately small:
/// everything the seam exists for is that a non-model option cannot reach
/// either door.
#[derive(Debug)]
pub(crate) struct Catalog {
    options: Vec<SessionConfigOption>,
}

impl Catalog {
    /// The seam. Only options declaring `category: "model"` survive.
    ///
    /// `None` (absent or malformed — same value after serde's
    /// `DefaultOnError`), `Mode`, `ModelConfig`, `ThoughtLevel` and an unknown
    /// `Other(..)` all do not, by the same allowlist logic as the tool kinds:
    /// unknown must not qualify.
    pub(crate) fn of(config_options: Option<&[SessionConfigOption]>) -> Self {
        let options = config_options
            .map(|options| {
                options
                    .iter()
                    .filter(|option| {
                        matches!(option.category, Some(SessionConfigOptionCategory::Model))
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        Self { options }
    }

    /// The render door: the only list any surface may show.
    pub(crate) fn options(&self) -> &[SessionConfigOption] {
        &self.options
    }

    /// The send door: the only way a model selection becomes a protocol write.
    ///
    /// Refuses any `config_id` this catalog does not hold, so a caller that
    /// names an option the filter rejected gets `None` rather than a write the
    /// render could never have shown. The value is passed back verbatim — the
    /// choices belong to the agent's claim, and Warp does not check them.
    ///
    /// Dead in this build only because no surface reaches for it yet: the
    /// model picker (T14.14's second half) is the caller, and it arrives after
    /// this seam. Until then the construction is pinned by the tests in this
    /// file rather than by a live render.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn request(
        &self,
        session_id: &SessionId,
        config_id: &SessionConfigId,
        value: SessionConfigOptionValue,
    ) -> Option<SetSessionConfigOptionRequest> {
        self.options
            .iter()
            .any(|option| &option.id == config_id)
            .then(|| {
                SetSessionConfigOptionRequest::new(session_id.clone(), config_id.clone(), value)
            })
    }
}

/// The `agent` value on the line this module writes, matching `translate.rs`.
const SOURCE: &str = "acp_agent";

/// Write one `session_model` line into the event log (T14.14, serving T14.17).
///
/// **The audit reader's answer to "which model was this agent on and what did
/// it offer?", the way `mode::log` answers "could this session have asked
/// me?".** Written on every turn whether or not anything was shown to a
/// person: a conversation note is rationed because a human reader tires, and a
/// log is read by something that does not.
///
/// Compact on purpose. `opencode`'s single model option alone holds hundreds
/// of selectable values, so the line names each model option and its current
/// selection rather than enumerating the choices.
pub(crate) fn log(conversation_id: &str, agent: &str, cwd: &str, catalog: &Catalog) {
    let options = catalog.options();
    if options.is_empty() {
        return;
    }
    let summary = options
        .iter()
        .map(|option| match &option.kind {
            // The protocol is open-ended (`#[non_exhaustive]`), so an unknown
            // kind is still the agent's claim about its model option and is
            // named, just without a current selection to report.
            SessionConfigKind::Select(select) => {
                format!(
                    "`{}` “{}”, current `{}`",
                    option.id.0, option.name, select.current_value.0
                )
            }
            _ => format!("`{}` “{}”", option.id.0, option.name),
        })
        .collect::<Vec<_>>()
        .join("; ");
    crate::event_log::record(crate::event_log::Entry {
        v: None,
        agent,
        event: "session_model",
        source: SOURCE,
        session_id: Some(conversation_id),
        linked_session_id: None,
        call_id: None,
        parent_call_id: None,
        cwd: Some(cwd),
        project: crate::event_log::project_name(cwd),
        tool_name: None,
        tool_input_preview: None,
        summary: Some(&summary),
        error_type: None,
        plugin_version: None,
        decision: None,
        answered_by: None,
        can_approve: None,
        applied: true,
    });
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
