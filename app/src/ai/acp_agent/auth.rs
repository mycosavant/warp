//! Why the agent would not open a session, when the reason is a credential.
//!
//! # What this is for
//!
//! `claude-agent-acp` reads Claude Code's own login off the disk, so the panel
//! has never had an authentication step and never needed one. Point
//! `WARP_FORK_ACP_COMMAND` at an agent that does not — Codex, Gemini — and
//! every turn dies at `session/new`, and what the person saw before this module
//! existed was the raw JSON-RPC error and nothing else. Measured 2026-09-07 and
//! again 2026-09-10, `.fork/runs/auth-2026-09-10/`:
//!
//! ```text
//! @zed-industries/codex-acp@0.16.0  →  -32000 "Authentication required"
//! @google/gemini-cli@0.58.0 --acp   →  -32000 "Gemini API key is missing or not configured."
//! ```
//!
//! # What this is not
//!
//! **Warp does not send `authenticate`.** Every one of those agents advertises
//! several ways to sign in, and picking one is choosing where a credential
//! comes from. That is the person's, for the same reason `WARP_FORK_ACP_MODE`
//! has no default: the ids are opaque vendor strings and Warp has no standing
//! to rank them. So this module explains a refusal and offers no way past it.
//!
//! Nothing here reads `WARP_FORK_ACP_AUTH`. The sentence below **names** that
//! variable as one that does not exist yet, which is a different thing from
//! parsing one that does nothing.
//!
//! # Why this keys on the error code and not on the advertised list
//!
//! The obvious rule — *the agent advertised ways to sign in and then refused a
//! session, so it must want one* — is wrong in a case this fork has already
//! measured, and it fails in the direction that hurts. `initialize` is answered
//! before the agent knows anything about the request, so `authMethods` is a
//! menu, not a verdict: Codex names all three of its methods whether or not
//! `OPENAI_API_KEY` is set. Once a credential exists on this machine, the very
//! next `session/new` failure is the one `CLAUDE.md` has recorded since
//! 2026-09-02 — a WSL pane's Unix cwd handed to a Windows-side agent — and that
//! rule would answer it with *"sign in"*.
//!
//! The code is the agent's own verdict about this request.
//! [`ErrorCode::AuthRequired`] is `-32000`, and **both refusing agents send it**,
//! including Gemini, whose message is its own prose. Measured on the wire with a
//! raw JSON-RPC probe, because `warpctrl acp probe` prints an [`Error`]'s
//! `Display`, which is its `message` alone and drops the code
//! (`.fork/runs/auth-2026-09-10/rawprobe.py`).
//!
//! The control is in the same run: `claude-agent-acp` 0.73.0, signed in, refused
//! `session/new` for a missing directory with `-32602` and an `authMethods` of
//! `[]`. Both halves of that answer are needed — the empty list is what makes
//! the advertised-list rule *look* safe on this machine today, and the code is
//! what keeps it safe on the machine that has a Codex key.

use agent_client_protocol::ErrorCode;
use agent_client_protocol::schema::v1::AuthMethod;

/// The `agent` value on the lines this module writes, matching `translate.rs`.
const SOURCE: &str = "acp_agent";

/// What a person is told when `session/new` was refused, or `None` when the
/// refusal was about something else.
///
/// `None` is not silence — it hands the error back to
/// [`super::spawn_failure_or`], which already explains the two non-credential
/// failures worth explaining and relays the rest verbatim. A second generic
/// sentence here would displace the one that names the WSL boundary, which is
/// the `session/new` failure that actually happens on the Windows build.
pub(crate) fn refused_for_a_credential(
    agent: &str,
    code: ErrorCode,
    said: &str,
    methods: &[AuthMethod],
) -> Option<String> {
    if code != ErrorCode::AuthRequired {
        return None;
    }
    Some(if methods.is_empty() {
        sign_in_somehow(agent, said)
    } else {
        sign_in_with_one_of(agent, said, methods)
    })
}

/// The agent wants a credential and said how one is supplied.
///
/// The list is the agent's own, rendered from the wire in the agent's own
/// words — id, name and description — and Warp adds nothing to it and ranks
/// nothing in it.
fn sign_in_with_one_of(agent: &str, said: &str, methods: &[AuthMethod]) -> String {
    format!(
        "{agent} would not start until you are signed in. It said: “{said}”\n\n\
         Ways it named to sign in:\n{}\n\n\
         Warp holds no credential and sends none, and there is no WARP_FORK_ACP_AUTH yet to \
         give it one. Sign in with {agent}'s own command-line tool and ask again, or point \
         WARP_FORK_ACP_COMMAND at an agent that is already signed in.",
        offered_list(methods)
    )
}

/// The agent wants a credential and named no way to supply one.
///
/// Separate from [`sign_in_with_one_of`] because the sentence it replaces would
/// otherwise read *"Ways it named to sign in:"* followed by nothing, which is
/// the shape of a bug rather than of a fact. The advice is also genuinely
/// different: with no list there is nothing to look up, and the agent's own
/// documentation is the only place left to go.
///
/// **Reachable by the wire rather than by any agent measured here.**
/// `auth_methods` is `#[serde(default)]`, so an agent that omits the field
/// entirely arrives with an empty list — this arm does not depend on any agent
/// choosing to answer `-32000` with nothing to offer, only on the field being
/// optional, which it is.
fn sign_in_somehow(agent: &str, said: &str) -> String {
    format!(
        "{agent} would not start until you are signed in. It said: “{said}”\n\n\
         It named no way to sign in, so there is nothing here to list and nothing for Warp to \
         offer. Sign in with {agent}'s own command-line tool and ask again, or point \
         WARP_FORK_ACP_COMMAND at an agent that is already signed in."
    )
}

/// One line per method, quoting whatever the agent chose to say about it.
///
/// The description is included when there is one because it is where the
/// actionable half lives — Codex's `codex-api-key` is unusable knowledge until
/// you read *"Requires setting the `CODEX_API_KEY` environment variable."* —
/// and because paraphrasing it would be Warp inventing instructions for a
/// login flow it does not implement.
fn offered_list(methods: &[AuthMethod]) -> String {
    methods
        .iter()
        .map(|method| match method.description() {
            Some(description) => format!("  {} — {}: {description}", method.id().0, method.name()),
            None => format!("  {} — {}", method.id().0, method.name()),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Write one `session_auth_required` line into the event log.
///
/// **The turn ends here, so without this the log ends without saying why.**
/// `model::log_agent_identity` has already written a `session_agent` line by
/// the time this fires, and nothing else on this path writes anything until the
/// prompt goes out — which it never does. A reader would find a session that
/// named its agent and then stopped, which is the same reading problem
/// `mode::log` exists to solve one step later.
///
/// The method ids go on the line and their names and descriptions do not: the
/// ids are what a later reader would grep for, and the prose is already in the
/// sentence the person saw.
pub(crate) fn log(
    conversation_id: &str,
    agent: &str,
    cwd: &str,
    said: &str,
    methods: &[AuthMethod],
) {
    let offered = if methods.is_empty() {
        "the agent named no way to sign in".to_owned()
    } else {
        methods
            .iter()
            .map(|method| format!("`{}`", method.id().0))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let summary = format!("refused with “{said}”; offered {offered}");
    crate::event_log::record(crate::event_log::Entry {
        v: None,
        agent,
        event: "session_auth_required",
        source: SOURCE,
        session_id: Some(conversation_id),
        // No session was opened, so the agent has no id for this turn and there
        // is nothing to join to. An absent field here is the honest answer, not
        // a gap: the agent's own record has no entry either.
        linked_session_id: None,
        call_id: None,
        parent_call_id: None,
        cwd: Some(cwd),
        project: crate::event_log::project_name(cwd),
        tool_name: None,
        tool_input_preview: None,
        summary: Some(&summary),
        error_type: Some("auth_required"),
        plugin_version: None,
        decision: None,
        answered_by: None,
        via: None,
        can_approve: None,
        applied: true,
    });
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
