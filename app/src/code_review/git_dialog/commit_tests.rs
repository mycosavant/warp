use super::{chain_failure_toast, user_facing_git_error};
use crate::code_review::diff_state::CommitChainFailure;
use crate::code_review::git_actions::CommitChainStage;

/// The message a routed `Commit and create PR` produced on 2026-09-12, when
/// the commit and push had landed and only `gh` refused.
const GH_NO_GITHUB_REMOTE: &str = "gh command failed: none of the git remotes configured for \
     this repository point to a known GitHub host. To tell gh about a new GitHub host, please \
     use `gh auth login`";

/// The whole point of carrying the stage: say the commit survived.
#[test]
fn a_pr_failure_after_a_commit_says_the_commit_survived() {
    let toast = chain_failure_toast(&CommitChainFailure::at(
        CommitChainStage::Pushed,
        GH_NO_GITHUB_REMOTE,
    ));
    assert!(
        toast.starts_with("Committed and pushed"),
        "the commit is on the remote and the toast must say so: {toast}"
    );
}

/// And never say it when it is not true.
#[test]
fn a_failure_before_the_commit_claims_nothing_about_it() {
    let toast = chain_failure_toast(&CommitChainFailure::at(
        CommitChainStage::NotCommitted,
        "nothing to commit, working tree clean",
    ));
    assert!(
        !toast.contains("Committed"),
        "nothing was committed: {toast}"
    );
    assert_eq!(toast, "No changes to commit.");
}

/// `gh`'s own message for *no GitHub remote* ends by suggesting `gh auth
/// login`, so a substring match on that phrase told the user to authenticate.
/// It is not an authentication problem and authenticating will not fix it.
#[test]
fn a_repo_with_no_github_remote_is_not_reported_as_an_auth_failure() {
    let advice = user_facing_git_error(GH_NO_GITHUB_REMOTE);
    assert_eq!(advice, "No GitHub remote for this repository.");
}

/// The arm that one displaced still works on a real auth failure.
#[test]
fn a_real_gh_auth_failure_still_asks_for_a_login() {
    let advice = user_facing_git_error(
        "gh command failed: You are not logged in to any GitHub hosts. Run `gh auth login` to authenticate.",
    );
    assert_eq!(advice, "GitHub CLI not authenticated. Run `gh auth login`.");
}

/// The case a `committed: bool` gets wrong in the other direction, and the
/// reason the stage is an enum: the commit is in the repository, so telling
/// the user it failed would have them remake it and be answered "nothing to
/// commit".
#[test]
fn a_push_failure_says_the_commit_was_made() {
    let toast = chain_failure_toast(&CommitChainFailure::at(
        CommitChainStage::Committed,
        "failed to push some refs",
    ));
    assert!(
        toast.starts_with("Committed, but the push failed."),
        "the commit is on disk and the toast must say so: {toast}"
    );
}

/// A failure the daemon reported with no stage claims nothing either way.
#[test]
fn a_stageless_failure_claims_nothing_about_the_commit() {
    let toast = chain_failure_toast(&CommitChainFailure::unknown_stage(
        "another git operation is in progress",
    ));
    assert_eq!(
        toast,
        "Another git operation is in progress. Finish or abort it first."
    );
}
