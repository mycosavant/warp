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
