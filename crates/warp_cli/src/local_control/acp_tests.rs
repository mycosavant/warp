//! The half of an ACP probe that can be decided without an agent.
//!
//! Everything here is about the *shape* of the exchange — where the session
//! runs, and what a transcript line looks like. The exchange itself is one
//! `initialize`/`session/new`/`session/prompt` against a real process, and is
//! verified by running it (`.fork/TASKS.md`, "T14.1 — as built").

use local_control::protocol::ErrorCode;

use super::*;

fn probe(command: &str) -> AcpProbeArgs {
    AcpProbeArgs {
        command: command.to_owned(),
        prompt: "hello".to_owned(),
        cwd: None,
        approve: false,
        mode: None,
    }
}

#[test]
fn an_empty_command_is_refused_before_anything_is_spawned() {
    let error = run_probe(probe("   ")).expect_err("an empty command should be refused");
    assert_eq!(error.code, ErrorCode::InvalidParams);
}

#[test]
fn a_directory_that_is_not_one_is_refused() {
    let file = std::env::temp_dir().join("warpctrl-acp-not-a-directory");
    std::fs::write(&file, b"").expect("the fixture should be writable");

    let error = session_directory(Some(&file)).expect_err("a file is not a directory");

    assert_eq!(error.code, ErrorCode::InvalidParams);
    assert!(
        error.message.contains("not a directory"),
        "the message should say what was wrong, got: {}",
        error.message
    );
    let _ = std::fs::remove_file(&file);
}

#[test]
fn a_directory_that_does_not_exist_is_refused() {
    let missing = std::env::temp_dir().join("warpctrl-acp-no-such-directory-here");
    let _ = std::fs::remove_dir_all(&missing);

    let error =
        session_directory(Some(&missing)).expect_err("a missing directory should be refused");

    assert_eq!(error.code, ErrorCode::InvalidParams);
}

/// The T13.3 failure, prevented rather than described: a relative path that
/// resolves silently is how a run reads the wrong tree and still looks like a
/// success.
#[test]
fn the_session_directory_is_made_absolute() {
    let resolved = session_directory(Some(std::path::Path::new(".")))
        .expect("the current directory should resolve");

    assert!(
        resolved.is_absolute(),
        "a session directory must be absolute, got: {}",
        resolved.display()
    );
}

/// On a POSIX host a `/`-rooted path is real and absolute, so
/// `is_foreign_filesystem_path` must say no and the strict local check above
/// it must still refuse a path that is genuinely missing — `cfg(unix)` rather
/// than assumed, so this is honestly green on the platform it runs on instead
/// of describing the other one.
#[cfg(unix)]
#[test]
fn a_posix_rooted_path_is_not_treated_as_foreign_on_a_posix_host() {
    assert!(!is_foreign_filesystem_path(std::path::Path::new(
        "/definitely/not/a/real/directory"
    )));
}

#[cfg(unix)]
#[test]
fn a_missing_posix_rooted_directory_is_still_refused_on_a_posix_host() {
    let error = session_directory(Some(std::path::Path::new(
        "/warpctrl-acp-no-such-directory-here",
    )))
    .expect_err("a missing directory should be refused, not passed through");

    assert_eq!(error.code, ErrorCode::InvalidParams);
}

/// The T18 case, run for real rather than argued: on Windows, `Path::
/// is_absolute()` for a `/`-rooted path is `false` (no drive prefix), which is
/// exactly what makes a WSL cwd unresolvable from this process — and exactly
/// what `is_foreign_filesystem_path` is watching for.
#[cfg(windows)]
#[test]
fn a_posix_rooted_path_this_process_cannot_see_is_treated_as_foreign_on_windows() {
    assert!(is_foreign_filesystem_path(std::path::Path::new(
        "/home/effatha/git/warp"
    )));
}

#[cfg(windows)]
#[test]
fn an_unresolvable_posix_rooted_cwd_is_passed_through_on_windows() {
    let resolved = session_directory(Some(std::path::Path::new("/home/effatha/git/warp")))
        .expect("a WSL-style cwd should be passed through, not refused");

    assert_eq!(resolved, std::path::Path::new("/home/effatha/git/warp"));
}

/// **The ordering defect, pinned by the one case that can tell.** The first cut
/// asked `is_foreign_filesystem_path` *after* `is_dir()`, as a fallback for a
/// path that failed the check — which reads as belt-and-braces and is not. On
/// Windows a POSIX-rooted path resolves against the current drive, so if the
/// matching `C:\…` tree exists then `is_dir()` is **true**, the fallback never
/// runs, and `canonicalize` returns a directory on the wrong machine.
///
/// That tree is not hypothetical: T20.1 is the ticket about Warp creating
/// `C:\home\effatha\git\warp` and writing a run's conversation into it. This
/// test builds the same shape on purpose and asserts the probe is not fooled by
/// it. Calibrated by moving the check back below `is_dir()`, which reddens this
/// and nothing else.
#[cfg(windows)]
#[test]
fn a_posix_cwd_is_not_silently_resolved_to_a_matching_windows_tree() {
    let name = "warpctrl-t20-5-drive-collision";
    let windows_tree = std::path::PathBuf::from(format!("C:\\{name}"));
    // Cleaned up first in case a previous failing run left it behind.
    let _ = std::fs::remove_dir_all(&windows_tree);
    std::fs::create_dir_all(&windows_tree).expect("the collision directory should be creatable");

    let posix = format!("/{name}");
    let resolved = session_directory(Some(std::path::Path::new(&posix)));

    let _ = std::fs::remove_dir_all(&windows_tree);

    assert_eq!(
        resolved.expect("a POSIX-rooted cwd is passed through"),
        std::path::Path::new(&posix),
        "the probe resolved a WSL cwd to the Windows tree it collides with",
    );
}

/// A Windows path really is missing when it is missing — the pass-through is
/// only for the POSIX-rooted shape a WSL cwd takes, not a blanket "anything
/// unresolvable is fine".
#[cfg(windows)]
#[test]
fn a_missing_windows_rooted_directory_is_still_refused_on_windows() {
    let error = session_directory(Some(std::path::Path::new(
        "C:\\warpctrl-acp-no-such-directory-here",
    )))
    .expect_err("a missing Windows directory should be refused, not passed through");

    assert_eq!(error.code, ErrorCode::InvalidParams);
}

#[test]
fn the_default_session_directory_is_the_current_one() {
    let resolved = session_directory(None).expect("the current directory should resolve");
    let expected = std::env::current_dir()
        .expect("the current directory should be readable")
        .canonicalize()
        .expect("the current directory should resolve");

    assert_eq!(resolved, expected);
}

/// One object per line, and both keys present — this is the contract anything
/// reading the transcript depends on, including `jq` and the mapping work the
/// probe exists to feed.
#[test]
fn a_transcript_line_is_one_json_object_with_a_kind_and_a_payload() {
    let line = record(
        "update",
        &serde_json::json!({ "sessionUpdate": "agent_message_chunk" }),
    )
    .expect("a plain value should render");

    assert!(
        !line.contains('\n'),
        "a record must be one line, got: {line}"
    );

    let parsed: serde_json::Value = serde_json::from_str(&line).expect("the line should be JSON");
    assert_eq!(parsed["kind"], "update");
    assert_eq!(parsed["payload"]["sessionUpdate"], "agent_message_chunk");
}

/// The probe prints what arrived, so it must not lose a message it cannot name.
/// An unknown variant is exactly the case the mapping table is being built to
/// discover, and dropping it would hide the thing worth seeing.
#[test]
fn a_record_keeps_a_payload_it_does_not_understand() {
    let line = record(
        "update",
        &serde_json::json!({ "somethingNewUpstream": [1, 2, 3] }),
    )
    .expect("an unfamiliar value should still render");

    let parsed: serde_json::Value = serde_json::from_str(&line).expect("the line should be JSON");
    assert_eq!(
        parsed["payload"]["somethingNewUpstream"],
        serde_json::json!([1, 2, 3])
    );
}
