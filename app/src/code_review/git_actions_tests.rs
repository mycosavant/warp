use std::path::Path;

use command::Stdio;
use command::r#async::Command;
use tempfile::TempDir;

use super::commit_message_diff;
use crate::util::git::MAX_DIFF_CHARS_FOR_AI;

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
    // The developer's global config reaches a temp repo. `tag.gpgsign = true`
    // turns a lightweight `git tag v1.0` into an annotated one, which fails
    // non-interactively with "no tag message?" -- so the tag is never created,
    // the checkout that follows never detaches, and a test asserting detached
    // behaviour sees the branch it started on. Measured 2026-09-12 against the
    // maintainer's own config; `commit.gpgsign` is off here for the same
    // reason, though ssh signing happens to succeed unattended.
    git(&path, &["config", "commit.gpgsign", "false"]).await;
    git(&path, &["config", "tag.gpgsign", "false"]).await;
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

/// Overwrites a tracked file with `line_count` lines, each long enough that
/// the diff comfortably clears `MAX_DIFF_CHARS_FOR_AI`. Verified rather than
/// assumed: 1,000 lines of similar filler produced a 160,891-byte `git diff`
/// on this machine, ten times the 16,000-byte budget.
fn write_oversized_file(path: &Path, line_count: usize) {
    let mut content = String::new();
    for i in 0..line_count {
        content.push_str(&format!(
            "line {i} filler text to pad the diff well past the budget\n"
        ));
    }
    std::fs::write(path, content).expect("write");
}

/// The commit-message path never sends an unbounded diff to a model — this is
/// the budget from `.fork/HANDOFF-GHTESTS.md` Task 4 that had "never been
/// tried, on either the commit-message or the PR path" until this test.
#[cfg(feature = "local_fs")]
#[tokio::test]
async fn a_huge_commit_message_diff_is_truncated_within_budget() {
    let (_dir, path) = init_repo().await;
    let file = path.join("big.txt");
    write_oversized_file(&file, 1_000);
    git(&path, &["add", "big.txt"]).await;
    git(&path, &["commit", "-m", "add big.txt"]).await;
    // Edit every line rather than rewriting identical content -- an
    // unchanged file produces an empty diff and would make this test measure
    // nothing, the same trap `an_empty_diff_is_refused_before_any_model_call`
    // exists to catch on the other side.
    let edited: String = std::fs::read_to_string(&file)
        .expect("read")
        .lines()
        .map(|line| format!("{line} EDITED\n"))
        .collect();
    std::fs::write(&file, edited).expect("write");
    git(&path, &["add", "big.txt"]).await;

    let diff = commit_message_diff(&path, true).await.expect("a diff");
    assert!(
        diff.len() <= MAX_DIFF_CHARS_FOR_AI + "\n... (diff truncated)".len(),
        "the model must never see more than the budget plus its own marker: {} bytes",
        diff.len()
    );
    assert!(
        diff.ends_with("... (diff truncated)"),
        "a truncated diff must say so, or the model has no signal it's incomplete: {}",
        &diff[diff.len().saturating_sub(80)..]
    );
}

/// The PR path has the same budget, applied separately -- `get_diff_for_pr`
/// truncates on its own, so a fix to one path is not a fix to the other.
#[cfg(feature = "local_fs")]
#[tokio::test]
async fn a_huge_pr_diff_is_truncated_within_budget() {
    let (_dir, path) = init_repo().await;
    git(&path, &["checkout", "-b", "feature"]).await;
    let file = path.join("big.txt");
    write_oversized_file(&file, 1_000);
    git(&path, &["add", "big.txt"]).await;
    git(&path, &["commit", "-m", "add big.txt"]).await;

    let inputs = super::pr_content_inputs(&path).await.expect("inputs");
    assert!(
        inputs.diff.len() <= MAX_DIFF_CHARS_FOR_AI + "\n... (diff truncated)".len(),
        "the model must never see more than the budget plus its own marker: {} bytes",
        inputs.diff.len()
    );
    assert!(
        inputs.diff.ends_with("... (diff truncated)"),
        "a truncated diff must say so: {}",
        &inputs.diff[inputs.diff.len().saturating_sub(80)..]
    );
}

/// Writes an executable fake `gh` into its own directory and returns
/// `(dir, path_env)` — the directory handle, and a `PATH` string with it first.
///
/// Until 2026-09-12 a fake like this was never executed: `run_gh_command`
/// resolved the program through the parent's `PATH`, and on Linux that wins
/// whenever the name is installed. It resolves `path_env` now, so this works.
/// `.fork/runs/ghpath-2026-09-12/`.
#[cfg(all(feature = "local_fs", unix))]
fn fake_gh(script: &str) -> (TempDir, String) {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("failed to create fake bin dir");
    let gh = dir.path().join("gh");
    fs::write(&gh, script).expect("failed to write fake gh");
    let mut perms = fs::metadata(&gh).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&gh, perms).unwrap();
    let path_env = format!(
        "{}:{}",
        dir.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    (dir, path_env)
}

/// A chain that fails while creating the PR reports that the commit landed.
///
/// The defect this pins, measured on a routed pane 2026-09-12: the dialog
/// labelled every chain failure *"Commit failed"*, including one where the
/// commit had been made and pushed and only `gh` refused. See
/// `.fork/runs/routedpr-2026-09-12/`.
#[cfg(all(feature = "local_fs", unix))]
#[tokio::test]
async fn a_chain_that_fails_at_the_pr_says_the_commit_landed() {
    let (_dir, _remote, repo) = init_repo_with_origin().await;
    git(&repo, &["checkout", "-b", "feature"]).await;
    std::fs::write(repo.join("f.txt"), "content\n").unwrap();
    let (_fake_dir, path_env) = fake_gh("#!/bin/sh\nprintf 'refused\\n' >&2\nexit 1\n");

    let err = super::run_commit_chain(
        &repo,
        super::CommitChainMode::CommitAndCreatePr,
        "a message",
        true,
        "feature",
        None,
        super::PrStage::Create(None),
        Some(&path_env),
    )
    .await
    .err()
    .expect("the fake gh refuses, so the chain must fail");

    assert_eq!(
        err.stage,
        super::CommitChainStage::Pushed,
        "the PR stage runs after the commit and push, so both landed: {err}"
    );
    // And they really did, not just by the flag's say-so.
    assert_eq!(
        git(&repo, &["log", "--oneline", "-1", "--format=%s"]).await,
        "a message"
    );
    assert_eq!(
        git(&repo, &["rev-list", "--count", "origin/feature..HEAD"]).await,
        "0",
        "the chain pushed before it tried the PR"
    );
}

/// A chain that fails before anything is published says so.
#[cfg(all(feature = "local_fs", unix))]
#[tokio::test]
async fn a_chain_that_fails_at_the_commit_does_not_claim_it_landed() {
    let (_dir, _remote, repo) = init_repo_with_origin().await;
    git(&repo, &["checkout", "-b", "feature"]).await;
    // Nothing to commit, so the commit stage fails.
    let (_fake_dir, path_env) = fake_gh("#!/bin/sh\nexit 0\n");

    let err = super::run_commit_chain(
        &repo,
        super::CommitChainMode::CommitAndCreatePr,
        "a message",
        true,
        "feature",
        None,
        super::PrStage::Create(None),
        Some(&path_env),
    )
    .await
    .err()
    .expect("an empty tree has nothing to commit");

    assert_eq!(
        err.stage,
        super::CommitChainStage::NotCommitted,
        "nothing was committed: {err}"
    );
}

/// A chain whose push fails reports the commit as made but unpublished.
///
/// Added because a calibration found nothing guarding it: breaking the push
/// arm's stage reddened no test at all. This is the case a `committed` flag
/// gets wrong in the other direction — the commit is in the repository, and
/// "Commit failed" would have the user remake it.
#[cfg(all(feature = "local_fs", unix))]
#[tokio::test]
async fn a_chain_whose_push_fails_says_the_commit_was_made() {
    let (_dir, repo) = init_repo().await;
    git(&repo, &["checkout", "-b", "feature"]).await;
    // An origin that is not a repository, so the push cannot succeed and no
    // network is involved either way.
    let not_a_repo = tempfile::tempdir().expect("failed to create non-repo dir");
    git(
        &repo,
        &[
            "remote",
            "add",
            "origin",
            &not_a_repo.path().display().to_string(),
        ],
    )
    .await;
    std::fs::write(repo.join("f.txt"), "content\n").unwrap();

    let err = super::run_commit_chain(
        &repo,
        super::CommitChainMode::CommitAndPush,
        "a message",
        true,
        "feature",
        None,
        super::PrStage::Create(None),
        None,
    )
    .await
    .err()
    .expect("pushing to a non-repository must fail");

    assert_eq!(err.stage, super::CommitChainStage::Committed, "{err}");
    // And the commit really is there, which is the whole claim.
    assert_eq!(
        git(&repo, &["log", "--oneline", "-1", "--format=%s"]).await,
        "a message"
    );
}
