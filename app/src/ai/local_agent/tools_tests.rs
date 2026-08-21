use super::*;
use crate::ai::blocklist::child_agent_tool_policy::READ_ONLY_TOOLS;

fn arguments(allowed: &[ToolType]) -> String {
    permission_arguments(allowed).join(" ")
}

/// The read-only preset forbids the shell on the other side of the crossing.
///
/// This is the assertion the whole module exists for. Warp's list said
/// read-only; without the mapping, `claude` was never told, and the child had
/// a shell while the response said otherwise.
#[test]
fn a_read_only_child_cannot_reach_a_shell() {
    let arguments = arguments(READ_ONLY_TOOLS);
    let forbidden = arguments
        .split("--disallowedTools ")
        .nth(1)
        .expect("a read-only policy forbids something");

    for tool in ["Bash", "Write", "Edit", "NotebookEdit", "Task"] {
        assert!(forbidden.contains(tool), "{tool} should be forbidden");
    }
    assert!(arguments.contains("--allowedTools Read,Grep,Glob"));
}

/// Withholding the fan-out tools is what forbids fan-out.
///
/// `SUBAGENT` and `RUN_AGENTS` both map to `Task`, so a child that has neither
/// cannot spawn one — which is the stronger of the two controls in T6.6, since
/// it applies where the request is built rather than depending on a counter
/// being incremented.
#[test]
fn a_child_without_subagent_tools_cannot_spawn_one() {
    assert!(arguments(&[ToolType::ReadFiles]).contains("Task"));
    assert!(
        !arguments(&[ToolType::ReadFiles])
            .split("--disallowedTools")
            .next()
            .expect("the allowed half comes first")
            .contains("Task")
    );
    assert!(arguments(&[ToolType::Subagent]).contains("--allowedTools Task"));
}

/// An empty allowlist forbids everything this can name and grants nothing.
///
/// The strictest request a caller can make, and the one where an off-by-one
/// would be least visible: with no `--allowedTools` argument at all, a wrong
/// implementation looks exactly like "no restriction".
#[test]
fn no_tools_grants_nothing_and_forbids_everything_governed() {
    let arguments = permission_arguments(&[]);
    assert_eq!(
        arguments.first().map(String::as_str),
        Some("--disallowedTools")
    );
    let forbidden = arguments[1].split(',').collect::<Vec<_>>();
    assert_eq!(forbidden, GOVERNED);
}

/// A Warp tool with no Claude counterpart grants nothing.
///
/// The fail-closed direction. `SEARCH_CODEBASE` is Warp's semantic index and
/// has no equivalent; mapping it to `Grep` because both are "searching" would
/// hand out a tool the caller never named.
#[test]
fn an_unmappable_tool_grants_nothing() {
    let arguments = permission_arguments(&[ToolType::SearchCodebase]);
    assert_eq!(arguments, permission_arguments(&[]));
}
