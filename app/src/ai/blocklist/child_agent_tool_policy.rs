//! What a child agent is allowed to do (`.fork/TASKS.md`, T6.6).
//!
//! A lead agent that hands work to a child should be able to say what the
//! child may reach for: a self-contained prompt delegated to a reviewer needs
//! to read the tree and nothing else. Upstream has the seam for it —
//! `RequestInput::with_supported_tools` sets `supported_tools_override`, and
//! `generate_multi_agent_output` uses that list *instead of*
//! `get_supported_tools` — but the seam is per-request, and a policy has to
//! outlive the turn that set it.
//!
//! So this is the per-surface half: a child agent gets a dedicated terminal
//! surface, and the policy is keyed by it, the same way
//! `apply_child_agent_model_override` keys a child's model by surface through
//! `LLMPreferences`. `RequestInput::new_with_common_fields` reads it, so every
//! turn of that child carries the same list without anything having to
//! remember to re-apply it.
//!
//! **This is a guardrail, not a sandbox.** It stops the model *calling* a
//! tool. It does not stop a long-running shell command a tool already started,
//! and it is not a boundary against a determined prompt injection — the child
//! is still a process on this machine with the user's credentials.

use std::collections::HashMap;

use warp_multi_agent_api::ToolType;
use warpui::{Entity, EntityId, SingletonEntity};

/// Tool allowlists for child agent surfaces.
pub struct ChildAgentToolPolicy {
    allowed_by_surface: HashMap<EntityId, Vec<ToolType>>,
}

impl ChildAgentToolPolicy {
    pub fn new() -> Self {
        Self {
            allowed_by_surface: HashMap::new(),
        }
    }

    /// Restricts a surface's agent to `tools`.
    ///
    /// An empty list is meaningful and is kept: it means "no tools", which is
    /// the strictest thing a caller can ask for and would be silently turned
    /// into "no policy" if this treated it as absent.
    pub fn restrict(&mut self, surface_id: EntityId, tools: Vec<ToolType>) {
        self.allowed_by_surface.insert(surface_id, tools);
    }

    pub fn allowed_tools(&self, surface_id: EntityId) -> Option<&[ToolType]> {
        self.allowed_by_surface.get(&surface_id).map(Vec::as_slice)
    }

    /// Forgets a surface's policy, for when its pane is discarded.
    pub fn release(&mut self, surface_id: EntityId) {
        self.allowed_by_surface.remove(&surface_id);
    }
}

impl Default for ChildAgentToolPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl Entity for ChildAgentToolPolicy {
    type Event = ();
}

impl SingletonEntity for ChildAgentToolPolicy {}

/// A named tool set, so a caller can say `read-only` instead of listing ten
/// proto enum names correctly.
///
/// Presets rather than only free-form names because the free-form list is the
/// part that is easy to get subtly wrong — leaving `RUN_SHELL_COMMAND` in a
/// "read-only" set makes the whole restriction decorative, and a shell is
/// exactly what someone assembling a read-only list forgets to take out.
pub const READ_ONLY_PRESET: &str = "read-only";

/// Tools that cannot change anything.
///
/// `RUN_SHELL_COMMAND` is the obvious exclusion and `CALL_MCP_TOOL` is the
/// non-obvious one: an MCP tool can do anything the server behind it does, so
/// admitting it would make the list a statement about Warp rather than about
/// the machine. `READ_SHELL_COMMAND_OUTPUT` is admitted — without the tool
/// that starts a command there is nothing for it to read but a block the user
/// ran themselves.
pub const READ_ONLY_TOOLS: &[ToolType] = &[
    ToolType::ReadFiles,
    ToolType::ReadDocuments,
    ToolType::ReadMcpResource,
    ToolType::ReadShellCommandOutput,
    ToolType::ReadSkill,
    ToolType::Grep,
    ToolType::FileGlob,
    ToolType::FileGlobV2,
    ToolType::SearchCodebase,
    ToolType::FetchConversation,
];

/// Resolves one `--allow-tools` token: a preset name, or a `ToolType` name.
///
/// Proto names (`RUN_SHELL_COMMAND`) rather than a fork-invented vocabulary,
/// so that what a caller writes is the same string `ToolType` uses and there
/// is no second list to keep in step.
pub fn resolve_tool_token(token: &str) -> Option<Vec<ToolType>> {
    let token = token.trim();
    if token.eq_ignore_ascii_case(READ_ONLY_PRESET) {
        return Some(READ_ONLY_TOOLS.to_vec());
    }
    ToolType::from_str_name(&token.to_ascii_uppercase().replace('-', "_")).map(|tool| vec![tool])
}

#[cfg(test)]
#[path = "child_agent_tool_policy_tests.rs"]
mod tests;
