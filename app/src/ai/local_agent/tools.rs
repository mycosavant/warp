//! Warp's tool vocabulary in Claude Code's terms (`.fork/TASKS.md`, T6.6).
//!
//! # Why this has to exist
//!
//! `generate_multi_agent_output` reads `supported_tools_override` *after* the
//! local-agent intercept:
//!
//! ```ignore
//! if local_agent_enabled() && local_agent::handles(&params) {
//!     return local_agent::generate(...)      // <- returns here
//! }
//! let supported_tools = params.supported_tools_override.take()...
//! ```
//!
//! So in this fork a tool allowlist set on the Warp side governs every request
//! Warp's own agent answers and none of the requests the local agent answers —
//! which, with the local agent on, is every plain user query. A child told it
//! was read-only would have had a shell. That is worse than no allowlist,
//! because it reads as a guarantee.
//!
//! `claude` takes `--allowedTools` and `--disallowedTools`, so the restriction
//! survives the crossing once the vocabulary is mapped. This is that mapping.
//!
//! # The mapping is deliberately partial, and fails closed
//!
//! A guardrail only has to be exact about the things it *forbids*. So the
//! table below names every Claude tool this can govern, and anything a
//! `ToolType` does not explicitly grant ends up in `--disallowedTools`. A Warp
//! tool with no Claude counterpart — `SEARCH_CODEBASE`, `UPLOAD_FILE_ARTIFACT`
//! — therefore grants nothing rather than being quietly waved through, and a
//! Claude tool that no `ToolType` names — `WebFetch`, `WebSearch` — can never
//! be granted at all, only forbidden.

use warp_multi_agent_api::ToolType;

/// Every Claude Code tool this mapping can speak about.
///
/// The set `--disallowedTools` is drawn from. Anything outside it is out of
/// this module's reach, and saying so here is the honest version of a list
/// that cannot be complete: Claude's tool set is not ours to enumerate.
const GOVERNED: &[&str] = &[
    "Bash",
    "Read",
    "Write",
    "Edit",
    "NotebookEdit",
    "Grep",
    "Glob",
    "Task",
    "WebFetch",
    "WebSearch",
];

/// The Claude tools a Warp tool grants.
///
/// Only the safety-relevant correspondences, and each one errs toward granting
/// less: `EDIT_DOCUMENTS` grants `NotebookEdit` as well as `Edit` because a
/// notebook is a document, while `SEARCH_CODEBASE` grants nothing because
/// Warp's semantic index has no Claude equivalent and mapping it to `Grep`
/// would hand out a tool the caller never asked for.
fn claude_tools_for(tool: ToolType) -> &'static [&'static str] {
    match tool {
        ToolType::RunShellCommand | ToolType::WriteToLongRunningShellCommand => &["Bash"],
        ToolType::ReadFiles | ToolType::ReadDocuments | ToolType::ReadSkill => &["Read"],
        ToolType::ApplyFileDiffs => &["Edit", "Write"],
        ToolType::CreateDocuments => &["Write"],
        ToolType::EditDocuments => &["Edit", "NotebookEdit"],
        ToolType::Grep => &["Grep"],
        ToolType::FileGlob | ToolType::FileGlobV2 => &["Glob"],
        ToolType::Subagent | ToolType::RunAgents => &["Task"],
        _ => &[],
    }
}

/// The `claude` arguments that enforce a Warp tool allowlist.
///
/// Both halves are emitted, and the second is the one that does the work:
/// `--allowedTools` says what may run without a prompt, while
/// `--disallowedTools` is what actually forbids. Emitting only the first would
/// leave everything else merely *prompting* — and in `--print` mode a prompt
/// is not something anyone can answer, so the difference would show up as a
/// child that hangs rather than one that refuses.
pub(super) fn permission_arguments(allowed: &[ToolType]) -> Vec<String> {
    let mut granted = Vec::new();
    for tool in allowed {
        for name in claude_tools_for(*tool) {
            if !granted.contains(name) {
                granted.push(*name);
            }
        }
    }
    let forbidden = GOVERNED
        .iter()
        .filter(|name| !granted.contains(*name))
        .copied()
        .collect::<Vec<_>>();

    let mut arguments = Vec::new();
    if !granted.is_empty() {
        arguments.push("--allowedTools".to_owned());
        arguments.push(granted.join(","));
    }
    if !forbidden.is_empty() {
        arguments.push("--disallowedTools".to_owned());
        arguments.push(forbidden.join(","));
    }
    arguments
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
