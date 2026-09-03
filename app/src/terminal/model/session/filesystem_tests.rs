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

// ── `native_path`: the session's spelling into this process's (T20.1) ────

/// Builds a session with the given launch data and bootstrap type.
fn session_for(
    launch: Option<crate::terminal::ShellLaunchData>,
    bootstrap: Option<crate::terminal::model::session::BootstrapSessionType>,
) -> Session {
    use std::sync::Arc;

    use crate::terminal::model::session::SessionInfo;
    use crate::terminal::model::session::command_executor::testing::TestCommandExecutor;
    use crate::terminal::shell::ShellType;

    let mut info = SessionInfo::new_for_test().with_shell_type(ShellType::Bash);
    if let Some(bootstrap) = bootstrap {
        info = info.with_session_type(bootstrap);
    }
    let session = Session::new(info, Arc::new(TestCommandExecutor::default()));
    match launch {
        Some(launch) => session.with_shell_launch_data(launch),
        None => session,
    }
}

/// **The T20.1 case, and it is `#[cfg(windows)]` because it can only be true
/// there.** Measured on the Windows build: a WSL pane reports
/// `/home/effatha/git/warp`, the transcript joined `.warp/transcripts` onto it
/// literally, and Windows resolved the result to
/// `C:\home\effatha\git\warp\.warp\transcripts` — which it then *created*,
/// wrote 43,014 bytes of the user's prompts into, and never logged a word
/// about, because a POSIX-rooted path is a valid relative-to-the-current-drive
/// path on Windows.
///
/// **Not run on the machine this fork is developed on**, which is stated rather
/// than left for someone to discover: `PathBuf::try_from` refuses a
/// Windows-encoded path on a Unix host, so the conversion cannot produce a
/// `PathBuf` here at all. That is not a gap in the guard — see the sibling
/// below, which runs everywhere and pins the half that would let the bug back
/// in.
#[cfg(windows)]
#[test]
fn a_wsl_sessions_unix_cwd_comes_back_in_the_spelling_this_process_can_open() {
    let session = session_for(
        Some(crate::terminal::ShellLaunchData::WSL {
            distro: "Ubuntu".to_owned(),
        }),
        None,
    );

    let native = native_path(&session, "/home/effatha/git/warp")
        .expect("a WSL session's files are reachable, through the UNC path");

    // The distribution is lower-cased by `convert_wsl_to_windows_host_path`,
    // which is the spelling `canonicalize_wsl_unc_path` folds to -- asserted in
    // that form rather than case-insensitively so a producer that disagreed
    // with the normal form is caught here rather than as a duplicate map key
    // somewhere downstream.
    assert_eq!(
        native,
        std::path::Path::new(r"\\wsl$\ubuntu\home\effatha\git\warp")
    );

    // A `/mnt/c/...` cwd is a Windows path the pane spells the Linux way, and
    // it converts back to the drive rather than to a UNC path -- the case a
    // reader assumes is the same one and is not.
    assert_eq!(
        native_path(&session, "/mnt/c/dev/warp").expect("the drive is reachable"),
        std::path::Path::new(r"C:\dev\warp"),
    );
}

/// **The half that runs everywhere, and it is the one that pins the defect.**
/// The bug was never "the conversion produced the wrong string" — it was that
/// *no conversion happened* and the session's own spelling went straight into a
/// `join`. So the invariant worth holding on every platform is that a WSL
/// session never gets its POSIX-rooted path handed back: on Windows it becomes
/// the UNC spelling, and on a Unix host it is `None`, because `PathBuf` there
/// cannot hold a Windows-encoded path and a WSL session cannot exist anyway.
///
/// Calibrated by making it fail: dropping the conversion and returning the
/// input verbatim reddens this and nothing else in the file.
#[test]
fn a_wsl_session_never_gets_its_own_spelling_back() {
    let session = session_for(
        Some(crate::terminal::ShellLaunchData::WSL {
            distro: "Ubuntu".to_owned(),
        }),
        None,
    );

    let native = native_path(&session, "/home/effatha/git/warp");
    assert_ne!(
        native.as_deref(),
        Some(std::path::Path::new("/home/effatha/git/warp")),
        "a WSL cwd handed back unchanged is the T20.1 bug",
    );
    #[cfg(not(windows))]
    assert_eq!(native, None);
}

/// **`None` is a stop, not a fallback.** A warpified-remote session's files are
/// on another machine and no spelling of the path reaches them, so the caller
/// must write nothing rather than create the remote machine's directory tree on
/// this one. Before T20.1 the transcript did exactly that -- `resolve` joined
/// the remote cwd locally and `create_dir_all` succeeded.
#[test]
fn a_warpified_remote_sessions_paths_are_refused_rather_than_taken_literally() {
    use crate::terminal::model::session::BootstrapSessionType;

    let session = session_for(None, Some(BootstrapSessionType::WarpifiedRemote));
    assert_eq!(native_path(&session, "/home/someone/project"), None);
}

/// An ordinary local session is the identity, and this is the calibration for
/// the two above: if the conversion were applied unconditionally every local
/// transcript would move.
#[test]
fn a_plain_local_session_gets_its_path_back_unchanged() {
    let session = session_for(None, None);
    assert_eq!(
        native_path(&session, "/home/effatha/git/warp").as_deref(),
        Some(std::path::Path::new("/home/effatha/git/warp")),
    );
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
