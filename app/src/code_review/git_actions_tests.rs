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

/// A repo with an `origin` pointing at a local bare clone.
///
/// `create_pr` resolves its base through `detect_main_branch`, which asks git
/// for `refs/remotes/origin/HEAD` first and errors out with *"no git remotes
/// found"* when there is no remote at all. A bare repo on disk satisfies that
/// without any network, which is the point: these tests must never be able to
/// reach GitHub even if `gh` were real.
async fn init_repo_with_origin() -> (TempDir, TempDir, std::path::PathBuf) {
    let (dir, path) = init_repo().await;
    let remote = tempfile::tempdir().expect("failed to create bare remote dir");
    git(remote.path(), &["init", "--bare", "-b", "main"]).await;
    git(
        &path,
        &[
            "remote",
            "add",
            "origin",
            &remote.path().display().to_string(),
        ],
    )
    .await;
    git(&path, &["push", "-u", "origin", "main"]).await;
    git(
        &path,
        &[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/main",
        ],
    )
    .await;
    (dir, remote, path)
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

/// The PR inputs are the two things a title and body are generated from, and
/// they come from the repository rather than a model.
///
/// The diff is taken against `origin/<branch>` when that ref exists and
/// `HEAD` otherwise, which is the fact the commit chain's ordering depends on:
/// inputs read before a commit describe the branch without it.
#[cfg(feature = "local_fs")]
#[tokio::test]
async fn the_pr_inputs_carry_the_branch_diff_and_its_commit_subjects() {
    let (_dir, path) = init_repo().await;
    git(&path, &["checkout", "-b", "feature"]).await;
    std::fs::write(path.join("shipped.txt"), "one\n").expect("write");
    git(&path, &["add", "shipped.txt"]).await;
    git(&path, &["commit", "-m", "add the shipped file"]).await;

    let inputs = super::pr_content_inputs(&path).await.expect("inputs");
    assert!(
        inputs.diff.contains("shipped.txt"),
        "the committed change must reach the model: {}",
        inputs.diff
    );
    assert!(
        inputs
            .commit_messages
            .iter()
            .any(|m| m.contains("add the shipped file")),
        "the branch's own subjects are context the PR body is written from: {:?}",
        inputs.commit_messages
    );
}

/// A commit made but not yet committed is invisible to the PR inputs.
///
/// This is the ordering constraint stated as a test rather than as a comment.
/// It is why the commit chain computes its inputs *after* its own commit and
/// push, and why the client cannot generate before sending the chain.
#[cfg(feature = "local_fs")]
#[tokio::test]
async fn the_pr_inputs_do_not_see_an_uncommitted_change() {
    let (_dir, path) = init_repo().await;
    git(&path, &["checkout", "-b", "feature"]).await;
    std::fs::write(path.join("uncommitted.txt"), "not yet\n").expect("write");

    let inputs = super::pr_content_inputs(&path).await.expect("inputs");
    assert!(
        !inputs.diff.contains("uncommitted.txt"),
        "generating before the chain's commit would describe the wrong branch: {}",
        inputs.diff
    );
}
