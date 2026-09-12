//! Shared git "action" orchestration: the commit-chain, push, create-PR, and
//! view-PR workflows behind the code-review git buttons.
//!
//! These compose the single-command primitives in [`crate::util::git`] (plus
//! AI title/body generation) into the end-to-end actions a button triggers.
//! They are intentionally backend-agnostic: the local code-review dialog and
//! the remote-server daemon both call them, so local and remote behave
//! identically. Git ops are host-scoped and not tied to a diff-state model, so
//! this logic lives here rather than on a model.
//!
//! Callers own everything *around* the action: UI (toasts, telemetry, dialog
//! lifecycle), transport/model (applying the returned delta to a
//! `DiffStateModel`, building wire responses), and any execution-time guards
//! (e.g. the daemon's `git_operation_in_progress` backstop).

#[cfg(test)]
#[path = "git_actions_tests.rs"]
mod tests;

use std::path::Path;

use crate::ai::generate_code_review_content::api::{GenerateCodeReviewContentRequest, OutputType};
use crate::code_review::diff_state::CommitChainMode;
use crate::server::server_api::ai::AIClient;
use crate::util::git::{self, Commit, PrInfo, get_branch_commit_messages, get_diff_for_pr};

/// Runs the commit chain — always commits, then optionally pushes, then
/// optionally creates a PR per `mode` — and returns the post-chain delta
/// (refreshed unpushed commits + upstream ref) plus any created PR. The delta
/// is computed once after the whole chain settles.
///
/// When the chain creates a PR, `ai_client` (when `Some`) generates the
/// title/body with a `--fill` fallback; pass `None` to skip AI entirely.
pub async fn run_commit_chain(
    repo_path: &Path,
    mode: CommitChainMode,
    message: &str,
    include_unstaged: bool,
    branch: &str,
    ai_client: Option<&dyn AIClient>,
    path_env: Option<&str>,
) -> anyhow::Result<(Vec<Commit>, Option<String>, Option<PrInfo>)> {
    git::run_commit(repo_path, message, include_unstaged, path_env).await?;
    let pr_info = match mode {
        CommitChainMode::CommitOnly => None,
        CommitChainMode::CommitAndPush => {
            git::run_push(repo_path, branch, path_env).await?;
            None
        }
        CommitChainMode::CommitAndCreatePr => {
            git::run_push(repo_path, branch, path_env).await?;
            Some(create_pr(repo_path, branch, ai_client, path_env).await?)
        }
    };
    let (commits, upstream_ref) = git::compute_unpushed_state(repo_path).await;
    Ok((commits, upstream_ref, pr_info))
}

/// Pushes `branch` (setting upstream) and returns the refreshed
/// unpushed/upstream delta.
pub async fn run_push(
    repo_path: &Path,
    branch: &str,
    path_env: Option<&str>,
) -> anyhow::Result<(Vec<Commit>, Option<String>)> {
    git::run_push(repo_path, branch, path_env).await?;
    Ok(git::compute_unpushed_state(repo_path).await)
}

/// Creates a PR for `branch`. When `ai_client` is `Some`, generates the
/// title/body via AI with a `gh pr create --fill` fallback; otherwise creates
/// the PR with `--fill`.
pub async fn create_pr(
    repo_path: &Path,
    branch: &str,
    ai_client: Option<&dyn AIClient>,
    path_env: Option<&str>,
) -> anyhow::Result<PrInfo> {
    match ai_client {
        Some(ai) => create_pr_with_ai_content(repo_path, branch, ai, path_env).await,
        None => git::create_pr(repo_path, None, None, path_env).await,
    }
}

/// The working-tree diff a commit message would be generated from.
///
/// Split out of [`generate_commit_message`] so the two halves can run in
/// different processes: the remote-server daemon computes the diff beside the
/// files, and the client generates from it with its own model. Bails on an
/// empty diff, which is the same refusal [`generate_commit_message`] makes and
/// for the same reason — so the dialog says one thing whichever side ran.
pub async fn commit_message_diff(
    repo_path: &Path,
    include_unstaged: bool,
) -> anyhow::Result<String> {
    let diff = git::get_diff_for_commit_message(repo_path, include_unstaged).await?;
    if diff.trim().is_empty() {
        anyhow::bail!("no changes to generate a commit message from");
    }
    Ok(diff)
}

/// Generates an AI commit message from a diff already in hand.
/// Bails when the model returns an empty message.
pub async fn generate_commit_message_from_diff(
    diff: String,
    branch_name: &str,
    ai_client: &dyn AIClient,
) -> anyhow::Result<String> {
    let generated = ai_client
        .generate_code_review_content(GenerateCodeReviewContentRequest {
            output_type: OutputType::CommitMessage,
            diff,
            branch_name: branch_name.to_string(),
            commit_messages: Vec::new(),
        })
        .await?
        .content;
    let trimmed = generated.trim();
    if trimmed.is_empty() {
        anyhow::bail!("AI returned an empty commit message");
    }
    Ok(trimmed.to_string())
}

/// Generates an AI commit message for the working-tree changes.
/// Bails when there's nothing to summarize (empty diff) or the model returns an empty message.
pub async fn generate_commit_message(
    repo_path: &Path,
    branch_name: &str,
    include_unstaged: bool,
    ai_client: &dyn AIClient,
) -> anyhow::Result<String> {
    // Skip the AI round trip when there's nothing to summarize.
    let diff = commit_message_diff(repo_path, include_unstaged).await?;
    generate_commit_message_from_diff(diff, branch_name, ai_client).await
}

/// Generates PR title and body via AI (in parallel) and creates the PR.
/// Falls back to `gh pr create --fill` if AI generation fails or returns
/// empty content, so AI-assisted and manual PR creation produce PRs the same
/// way.
async fn create_pr_with_ai_content(
    repo_path: &Path,
    branch_name: &str,
    code_review_ai: &dyn AIClient,
    path_env: Option<&str>,
) -> anyhow::Result<PrInfo> {
    let diff = get_diff_for_pr(repo_path).await?;
    let commit_messages = get_branch_commit_messages(repo_path)
        .await
        .unwrap_or_default();

    let title_req = GenerateCodeReviewContentRequest {
        output_type: OutputType::PrTitle,
        diff: diff.clone(),
        branch_name: branch_name.to_string(),
        commit_messages: commit_messages.clone(),
    };
    let body_req = GenerateCodeReviewContentRequest {
        output_type: OutputType::PrDescription,
        diff,
        branch_name: branch_name.to_string(),
        commit_messages,
    };

    match futures::try_join!(
        code_review_ai.generate_code_review_content(title_req),
        code_review_ai.generate_code_review_content(body_req),
    ) {
        Ok((title_resp, body_resp))
            if !title_resp.content.trim().is_empty() && !body_resp.content.trim().is_empty() =>
        {
            git::create_pr(
                repo_path,
                Some(&title_resp.content),
                Some(&body_resp.content),
                path_env,
            )
            .await
        }
        Ok(_) => {
            // Empty title/body would make `gh pr create` fail; fall back to --fill.
            log::warn!(
                "AI PR content generation returned empty title/body, falling back to --fill"
            );
            git::create_pr(repo_path, None, None, path_env).await
        }
        Err(err) => {
            log::warn!("AI PR content generation failed, falling back to --fill: {err}");
            git::create_pr(repo_path, None, None, path_env).await
        }
    }
}
