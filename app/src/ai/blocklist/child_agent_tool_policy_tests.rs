use super::*;

/// The read-only preset admits nothing that can change anything.
///
/// Asserted as a property rather than as a copy of the list, because a copy
/// would be edited in step with the mistake it exists to catch. The named
/// exclusions are the ones a person assembling this by hand gets wrong:
/// `RUN_SHELL_COMMAND` because a shell is how everything else is done, and
/// `CALL_MCP_TOOL` because an MCP tool can do whatever the server behind it
/// does, which no list here can know.
#[test]
fn the_read_only_preset_cannot_write() {
    for forbidden in [
        ToolType::RunShellCommand,
        ToolType::WriteToLongRunningShellCommand,
        ToolType::ApplyFileDiffs,
        ToolType::CreateDocuments,
        ToolType::EditDocuments,
        ToolType::CallMcpTool,
        ToolType::UseComputer,
        ToolType::UploadFileArtifact,
        ToolType::InsertReviewComments,
        ToolType::Subagent,
        ToolType::RunAgents,
        ToolType::SendMessageToAgent,
    ] {
        assert!(
            !READ_ONLY_TOOLS.contains(&forbidden),
            "{forbidden:?} is in the read-only preset"
        );
    }
    assert!(READ_ONLY_TOOLS.contains(&ToolType::ReadFiles));
    assert!(READ_ONLY_TOOLS.contains(&ToolType::Grep));
}

/// A token is a preset or a proto name, and anything else is refused.
///
/// Refused rather than ignored: a caller that misspells a tool name and gets
/// silence has been handed a policy it did not ask for, and the direction of
/// that mistake is always toward *fewer* tools than intended in an allowlist,
/// which looks like the child simply refusing to work.
#[test]
fn tool_tokens_resolve_or_fail() {
    assert_eq!(
        resolve_tool_token("read-only"),
        Some(READ_ONLY_TOOLS.to_vec())
    );
    assert_eq!(
        resolve_tool_token("  READ-ONLY  "),
        resolve_tool_token("read-only")
    );
    assert_eq!(
        resolve_tool_token("READ_FILES"),
        Some(vec![ToolType::ReadFiles])
    );
    // Spelled the way a person types it on a command line.
    assert_eq!(
        resolve_tool_token("read-files"),
        Some(vec![ToolType::ReadFiles])
    );
    assert_eq!(resolve_tool_token("Bash"), None);
    assert_eq!(resolve_tool_token(""), None);
}

/// An empty allowlist is a policy, not the absence of one.
///
/// "This child gets no tools" is the strictest thing a lead agent can ask for.
/// Storing it as `None` would turn the strictest request into no restriction
/// at all, which is the one direction a guardrail must never fail in.
#[test]
fn no_tools_is_different_from_no_policy() {
    let mut policy = ChildAgentToolPolicy::new();
    let unrestricted = EntityId::from_usize(1);
    let restricted = EntityId::from_usize(2);

    policy.restrict(restricted, Vec::new());

    assert_eq!(policy.allowed_tools(unrestricted), None);
    assert_eq!(policy.allowed_tools(restricted), Some([].as_slice()));

    policy.release(restricted);
    assert_eq!(policy.allowed_tools(restricted), None);
}
