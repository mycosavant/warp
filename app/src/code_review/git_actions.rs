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
#[allow(clippy::too_many_arguments)]
/// How far a commit chain got before it failed.
///
/// This is the part a user cannot work out for themselves, and the reason the
/// type exists: a chain that fails at the pull-request stage has already made
/// a commit and pushed it, and reporting that as *"Commit failed"* sends them
/// looking for work that is on disk and on the remote. Measured on a routed
/// pane 2026-09-12, where exactly that happened:
/// `.fork/runs/routedpr-2026-09-12/`.
///
/// Three states rather than a `committed` flag, because a **push** failure is
/// the case a flag gets wrong in the other direction: the commit exists
/// locally, so "the commit failed" would send the user to remake a commit that
/// is already there, and their retry would answer "nothing to commit".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitChainStage {
    /// Nothing was committed.
    NotCommitted,
    /// The commit was made and is in the local repository. Nothing was
    /// published.
    Committed,
    /// The commit was made and pushed. A later stage failed.
    Pushed,
}

/// A commit chain that failed, and how far it got.
#[derive(Debug)]
pub struct CommitChainError {
    pub stage: CommitChainStage,
    pub source: anyhow::Error,
}

impl CommitChainError {
    /// `.map_err(CommitChainError::at(CommitChainStage::Pushed))`
    fn at(stage: CommitChainStage) -> impl FnOnce(anyhow::Error) -> Self {
        move |source| Self { stage, source }
    }
}

impl std::fmt::Display for CommitChainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Deliberately just the cause: every consumer that wants the stage
        // reads `committed`, and the daemon turns this into a wire string
        // where an added prefix would be a second, unparsed claim.
        write!(f, "{}", self.source)
    }
}

// `std::error::Error` is what lets a caller that only wants the cause — the
// daemon, which has no way to put `committed` on the wire — convert with
// anyhow's blanket `From`. Implementing `From<Self> for anyhow::Error` by hand
// collides with that blanket impl.
impl std::error::Error for CommitChainError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.source()
    }
}

pub async fn run_commit_chain(
    repo_path: &Path,
    mode: CommitChainMode,
    message: &str,
    include_unstaged: bool,
    branch: &str,
    ai_client: Option<&dyn AIClient>,
    pr: PrStage,
    path_env: Option<&str>,
) -> Result<CommitChainOutcome, CommitChainError> {
    git::run_commit(repo_path, message, include_unstaged, path_env)
        .await
        .map_err(CommitChainError::at(CommitChainStage::NotCommitted))?;
    let mut pr_info = None;
    let mut pr_inputs = None;
    match mode {
        CommitChainMode::CommitOnly => {}
        CommitChainMode::CommitAndPush => {
            // Past the commit, so a push failure leaves one behind.
            git::run_push(repo_path, branch, path_env)
                .await
                .map_err(CommitChainError::at(CommitChainStage::Committed))?;
        }
        CommitChainMode::CommitAndCreatePr => {
            git::run_push(repo_path, branch, path_env)
                .await
                .map_err(CommitChainError::at(CommitChainStage::Committed))?;
            // After the push, and that ordering is load-bearing: the PR diff
            // is taken against `origin/<branch>`, so inputs computed before
            // this point would describe the branch without the commit just
            // made.
            match pr {
                PrStage::Create(content) => {
                    pr_info = Some(
                        create_pr(repo_path, branch, ai_client, content, path_env)
                            .await
                            .map_err(CommitChainError::at(CommitChainStage::Pushed))?,
                    );
                }
                PrStage::ReturnInputs => {
                    pr_inputs = Some(
                        pr_content_inputs(repo_path)
                            .await
                            .map_err(CommitChainError::at(CommitChainStage::Pushed))?,
                    );
                }
            }
        }
    }
    let (commits, upstream_ref) = git::compute_unpushed_state(repo_path).await;
    Ok(CommitChainOutcome {
        commits,
        upstream_ref,
        pr_info,
        pr_inputs,
    })
}

/// What the commit chain does at its PR stage, once the commit and push have
/// landed. Only consulted for [`CommitChainMode::CommitAndCreatePr`].
pub enum PrStage {
    /// Create the PR — from `content` when the caller generated it, otherwise
    /// from the chain's `ai_client`, otherwise `--fill`.
    Create(Option<PrContent>),
    /// Create nothing. Compute what a title and body would be generated from
    /// and hand them back, for the caller to generate and then call
    /// [`create_pr`] itself.
    ///
    /// This is the fork's path, and it cannot be replaced by generating
    /// before the chain: see the ordering note above.
    ReturnInputs,
}

/// What a commit chain produced. `pr_info` and `pr_inputs` are mutually
/// exclusive, and which one is set follows the [`PrStage`] the caller asked
/// for; both are `None` for the two modes that create no PR.
pub struct CommitChainOutcome {
    pub commits: Vec<Commit>,
    pub upstream_ref: Option<String>,
    pub pr_info: Option<PrInfo>,
    pub pr_inputs: Option<PrContentInputs>,
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

/// Creates a PR for `branch`.
///
/// Three ways in, in precedence order: `content` the caller already generated,
/// an `ai_client` to generate it here (with a `gh pr create --fill` fallback),
/// or neither, which is `--fill`.
///
/// `content` exists because the process holding the repository and the process
/// holding the model configuration are not always the same one — see
/// `fork::remote_pr_content_generated_locally`.
pub async fn create_pr(
    repo_path: &Path,
    branch: &str,
    ai_client: Option<&dyn AIClient>,
    content: Option<PrContent>,
    path_env: Option<&str>,
) -> anyhow::Result<PrInfo> {
    match (content, ai_client) {
        (Some(c), _) => git::create_pr(repo_path, Some(&c.title), Some(&c.body), path_env).await,
        (None, Some(ai)) => create_pr_with_ai_content(repo_path, branch, ai, path_env).await,
        (None, None) => git::create_pr(repo_path, None, None, path_env).await,
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

/// The two things a PR's title and body are generated from.
///
/// Split out so the halves can run in different processes, exactly as
/// [`commit_message_diff`] is: computing these needs the repository, and
/// generating from them needs a model. The remote-server daemon has the first
/// and, in this fork, not the second.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PrContentInputs {
    /// `merge_base(main, HEAD)..origin/<branch>` — so it is only complete
    /// after the branch has been pushed, which is why the commit chain
    /// computes it *after* its push rather than before the whole chain.
    pub diff: String,
    pub commit_messages: Vec<String>,
}

/// A PR's title and body, generated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrContent {
    pub title: String,
    pub body: String,
}

/// Reads what a PR's title and body would be generated from. Needs the
/// repository and no model.
pub async fn pr_content_inputs(repo_path: &Path) -> anyhow::Result<PrContentInputs> {
    let diff = get_diff_for_pr(repo_path).await?;
    // Deliberately not fatal, matching the behaviour this was split out of: a
    // branch with no readable commit subjects still gets a PR from its diff.
    let commit_messages = get_branch_commit_messages(repo_path)
        .await
        .unwrap_or_default();
    Ok(PrContentInputs {
        diff,
        commit_messages,
    })
}

/// Generates a PR title and body from inputs already in hand. Needs a model
/// and no repository.
///
/// Empty content is an `Err` rather than an empty [`PrContent`], because both
/// callers want the same thing when the model gives them nothing useful --
/// `gh pr create --fill` -- and an empty title makes `gh` fail outright.
pub async fn generate_pr_content(
    inputs: PrContentInputs,
    branch_name: &str,
    code_review_ai: &dyn AIClient,
) -> anyhow::Result<PrContent> {
    let PrContentInputs {
        diff,
        commit_messages,
    } = inputs;
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

    let (title_resp, body_resp) = futures::try_join!(
        code_review_ai.generate_code_review_content(title_req),
        code_review_ai.generate_code_review_content(body_req),
    )?;
    if title_resp.content.trim().is_empty() || body_resp.content.trim().is_empty() {
        anyhow::bail!("AI PR content generation returned an empty title or body");
    }
    Ok(PrContent {
        title: title_resp.content,
        body: body_resp.content,
    })
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
    let inputs = pr_content_inputs(repo_path).await?;
    match generate_pr_content(inputs, branch_name, code_review_ai).await {
        Ok(content) => {
            git::create_pr(
                repo_path,
                Some(&content.title),
                Some(&content.body),
                path_env,
            )
            .await
        }
        Err(err) => {
            log::warn!("AI PR content generation failed, falling back to --fill: {err}");
            git::create_pr(repo_path, None, None, path_env).await
        }
    }
}
