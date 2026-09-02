use warp_core::HostId;

use super::*;

fn host() -> HostId {
    HostId::new("e35d6030-5e55-4940-a1ec-17a6cc6d064e".to_string())
}

#[test]
fn a_wsl_session_with_a_connected_server_is_reached_through_the_host() {
    // The whole reason this module exists. `SessionType::Local` is what
    // bootstrap decided and it stays that way; the files are still somewhere
    // else. Measured in T16: routing this arm took a repository's tree from a
    // 32-minute walk over 9p to a load served from ext4.
    assert_eq!(
        classify(SessionType::Local, true, Some(host())),
        SessionFilesystem::Host(host()),
    );
}

#[test]
fn a_wsl_session_without_a_server_keeps_the_local_path_it_has_always_had() {
    // Deliberately `Local`, not `Unreachable`: the UNC path works, it is
    // merely slow, and a WSL pane that has never run `remote wsl connect` must
    // not lose its file tree to a host that does not exist.
    assert_eq!(
        classify(SessionType::Local, true, None),
        SessionFilesystem::Local,
    );
}

#[test]
fn a_connected_server_does_not_capture_a_session_that_is_not_wsl() {
    // The manager is keyed by session id, so this should not arise — but the
    // `is_wsl` gate is what makes that a property of the code rather than of
    // the manager's bookkeeping. An ordinary local pane stays local even if a
    // host id is somehow associated with it.
    assert_eq!(
        classify(SessionType::Local, false, Some(host())),
        SessionFilesystem::Local,
    );
}

#[test]
fn an_ordinary_local_session_is_local() {
    assert_eq!(
        classify(SessionType::Local, false, None),
        SessionFilesystem::Local,
    );
}

#[test]
fn a_warpified_remote_session_routes_to_its_own_host() {
    assert_eq!(
        classify(
            SessionType::WarpifiedRemote {
                host_id: Some(host())
            },
            false,
            None,
        ),
        SessionFilesystem::Host(host()),
    );
}

#[test]
fn a_remote_session_with_no_host_is_unreachable_rather_than_local() {
    // The distinction that matters: a caller treating this as `Local` reads
    // *this* machine's filesystem for paths that belong to another one, which
    // succeeds often enough to be worse than failing.
    assert_eq!(
        classify(SessionType::WarpifiedRemote { host_id: None }, false, None),
        SessionFilesystem::Unreachable,
    );
    assert!(!classify(SessionType::WarpifiedRemote { host_id: None }, false, None).is_local());
}

#[test]
fn only_the_host_variant_offers_a_host_to_route_to() {
    assert_eq!(SessionFilesystem::Host(host()).host(), Some(&host()));
    assert_eq!(SessionFilesystem::Local.host(), None);
    assert_eq!(SessionFilesystem::Unreachable.host(), None);
}

/// Files that may read `Session::session_type()` in live code.
///
/// **This is the guard T16 asked for and could not get any other way.** The
/// routing seam has no unit test because `RemoteSessionState::Connected` holds
/// a live `async_process::Child` and an `Arc<RemoteServerClient>`, so a
/// connected session cannot be constructed without spawning one; and the
/// integration harness needs a real SSH host plus the gcloud SDK, neither of
/// which exists for WSL. So the wiring is verified by running.
///
/// What *can* be pinned is the failure mode, which has now happened twice:
/// **a new call site asks `session_type()` about a file and gets `Local` for a
/// WSL session whose files are in the distribution.** Phase 1 shipped with two
/// such sites and only one routed, and the server answered
/// `Repository not found`. Phase 2 found four more.
///
/// Every entry below was read and classified. A file that appears here without
/// being added deliberately is the bug coming back.
const SESSION_TYPE_READERS: &[(&str, &str)] = &[
    // The definition, and the one place the two are allowed to disagree.
    (
        "terminal/model/session.rs",
        "defines SessionType and determine_session_type",
    ),
    (
        "terminal/model/session/filesystem.rs",
        "folds session type and WSL host into one answer",
    ),
    (
        "terminal/model/session/active_session.rs",
        "session_type() accessor; location_for_path routes on filesystem()",
    ),
    (
        "ai/blocklist/controller.rs",
        "SessionContext holds both; its accessors route on filesystem()",
    ),
    // Where *commands* run, which is a different question from where files
    // are. A WSL session's shell is already native Linux.
    (
        "ai/agent/api.rs",
        "orchestration gate: subagents run shell commands in the session",
    ),
    (
        "completer/mod.rs",
        "directory listing, with its own WSL guest path (APP-3993)",
    ),
    // Display and affordance decisions, which follow the session's identity
    // rather than its filesystem.
    (
        "terminal/prompt/mod.rs",
        "renders a user@host prefix for remote sessions",
    ),
    (
        "ai/blocklist/agent_view/zero_state_block.rs",
        "renders a user@host prefix for remote sessions",
    ),
    ("context_chips/builtins.rs", "chip availability"),
    ("context_chips/display_chip.rs", "chip availability"),
    (
        "terminal/input.rs",
        "input affordances for warpified-remote sessions",
    ),
    (
        "terminal/universal_developer_input.rs",
        "@ menu gate: blocks SSH wrappers with no server",
    ),
    (
        "terminal/view.rs",
        "pwd_as_local_or_remote's remote arm, plus display and telemetry",
    ),
];

#[test]
fn every_file_that_reads_session_type_has_been_classified() {
    // Files whose only match is inside a comment or a `selected_session_type`
    // (a settings enum of the same name, unrelated to `Session`).
    const COMMENT_OR_UNRELATED: &[&str] = &[
        "ai/agent/api/impl.rs",
        "ai/blocklist/action_model/execute/read_files.rs",
        "ai/blocklist/action_model/execute/request_file_edits.rs",
        "terminal/model/terminal_model.rs",
        "tab_configs/session_config_modal.rs",
        "workspace/hoa_onboarding/hoa_onboarding_flow.rs",
    ];

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut unclassified = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !name.ends_with(".rs") || name.ends_with("_tests.rs") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            // Only code, never a comment: this guard exists to catch a new
            // *decision*, and the fork's own history has plenty of comments
            // that name `session_type()` precisely to say they do not use it.
            let reads_it = text.lines().any(|line| {
                let trimmed = line.trim_start();
                !trimmed.starts_with("//")
                    && !trimmed.starts_with("///")
                    && line.contains("session_type()")
                    && !line.contains("selected_session_type()")
                    && !line.contains("determine_session_type()")
            });
            if !reads_it {
                continue;
            }
            let rel = path
                .strip_prefix(&root)
                .expect("walked from root")
                .to_string_lossy()
                .replace('\\', "/");
            let known = SESSION_TYPE_READERS.iter().any(|(f, _)| *f == rel)
                || COMMENT_OR_UNRELATED.contains(&rel.as_str());
            if !known {
                unclassified.push(rel);
            }
        }
    }
    unclassified.sort();
    assert!(
        unclassified.is_empty(),
        "new `session_type()` reader(s) not classified in SESSION_TYPE_READERS: {unclassified:?}.\n\
         If the decision is about where a *file* is, use `session_filesystem` instead -- a WSL \
         session reports `Local` and its files are in the distribution. If it is about display, \
         affordances or where *commands* run, add the file to the list with the reason.",
    );
}
