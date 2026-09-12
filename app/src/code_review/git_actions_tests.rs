use std::path::Path;

use command::Stdio;
use command::r#async::Command;
use tempfile::TempDir;

use super::commit_message_diff;

/// Helper: run a git command inside the given repo directory.
async fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .expect("failed to run git");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

async fn init_repo() -> (TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = dir.path().to_path_buf();

    git(&path, &["init", "-b", "main"]).await;
    git(&path, &["config", "user.email", "test@test.com"]).await;
    git(&path, &["config", "user.name", "Test"]).await;
    git(&path, &["commit", "--allow-empty", "-m", "initial"]).await;

    (dir, path)
}

/// The refusal both sides make.
///
/// `commit_message_diff` is what the daemon runs when the client asked for a
/// diff; `generate_commit_message` is what it runs otherwise. A clean tree has
/// nothing to summarise either way, and the dialog must say the same thing —
/// otherwise which process happened to hold the model configuration would be
/// visible to the user as a different error.
#[cfg(feature = "local_fs")]
#[tokio::test]
async fn an_empty_diff_is_refused_before_any_model_call() {
    let (_dir, path) = init_repo().await;

    let err = commit_message_diff(&path, true)
        .await
        .expect_err("a clean tree has no changes to summarise");
    assert!(
        err.to_string().contains("no changes"),
        "the message reaches the dialog verbatim: {err}"
    );
}

/// What the diff-only path actually returns: the same text the model would
/// have been given, not a summary of it.
///
/// The point of the split is that the daemon does the half that needs the
/// files and the client does the half that needs the model. If this returned
/// anything but the diff, the client would generate a commit message for the
/// wrong thing and nothing would error.
#[cfg(feature = "local_fs")]
#[tokio::test]
async fn the_diff_only_path_returns_the_working_tree_diff() {
    let (_dir, path) = init_repo().await;
    std::fs::write(path.join("tracked.txt"), "one\n").expect("write");
    git(&path, &["add", "tracked.txt"]).await;
    git(&path, &["commit", "-m", "add tracked"]).await;
    std::fs::write(path.join("tracked.txt"), "one\ntwo\n").expect("write");

    let diff = commit_message_diff(&path, true).await.expect("a diff");
    assert!(
        diff.contains("tracked.txt") && diff.contains("+two"),
        "the added line must be in the text the client sends to the model: {diff}"
    );
}

/// Untracked files reach the client too.
///
/// `include_unstaged` makes `get_diff_for_commit_message` synthesise hunks for
/// untracked files, because a commit made entirely of new files is otherwise
/// invisible to `git diff HEAD`. That work happens beside the files, on the
/// daemon — which is the argument for returning a diff rather than having the
/// client walk the tree over the wire.
#[cfg(feature = "local_fs")]
#[tokio::test]
async fn an_untracked_file_is_in_the_returned_diff() {
    let (_dir, path) = init_repo().await;
    std::fs::write(path.join("fresh.txt"), "hello\n").expect("write");

    let diff = commit_message_diff(&path, true).await.expect("a diff");
    assert!(
        diff.contains("fresh.txt"),
        "a commit of only new files would otherwise summarise as nothing: {diff}"
    );
}
