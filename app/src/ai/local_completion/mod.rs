//! The four small AI features, issued from this machine.
//!
//! "Next Command", "Prompt Suggestions", "Shared Block Title Generation" and
//! "Commit & PR Generation" each amount to one `POST` to an `/ai/*` route on
//! `api.warp.dev` with a JSON body and a JSON reply. No streaming, no tool use,
//! no session state — Warp's server is a bearer-authenticated proxy in front of
//! a model. That makes them separable from the agent in a way nothing else in
//! this codebase is, and it is why they come before the agent work rather than
//! after it.
//!
//! Each function here has the signature of the `ServerApi` method it replaces,
//! so the four call sites in `server::server_api` swap one `await` for another.
//!
//! ## Why this is fail-closed
//!
//! The payloads are the point. Between them these four carry terminal output
//! and the command that produced it, the working directory and recent shell
//! history, and an entire working-tree diff. `fork::account_gate_bypassed`
//! makes the toggles reachable without an account, so a fork user could switch
//! one on and quietly resume shipping that upstream. Under fork policy these
//! calls therefore never reach `api.warp.dev`: an unconfigured endpoint
//! surfaces as an error naming the setting to fill in.
//!
//! ## "Local"
//!
//! Issued from this machine, not necessarily inferred on it. The same path
//! serves a llama.cpp server on loopback and `api.anthropic.com` with the
//! user's own key — what both have in common, and what is actually being
//! bought, is that Warp is not in the middle.

pub mod client;
pub mod config;
mod features;

#[cfg(test)]
#[path = "install_tests.rs"]
mod install_tests;

#[cfg(test)]
#[path = "wire_tests.rs"]
mod wire_tests;

pub use config::install;

use crate::ai::generate_block_title::api::{GenerateBlockTitleRequest, GenerateBlockTitleResponse};
use crate::ai::generate_code_review_content::api::{
    GenerateCodeReviewContentRequest, GenerateCodeReviewContentResponse,
};
use crate::ai::predict::generate_ai_input_suggestions::{
    GenerateAIInputSuggestionsRequest, GenerateAIInputSuggestionsResponseV2,
};
use crate::ai::predict::generate_am_query_suggestions::{
    GenerateAMQuerySuggestionsRequest, GenerateAMQuerySuggestionsResponse,
};

/// Replaces `POST {server}/ai/generate_block_title`.
pub async fn generate_block_title(
    request: GenerateBlockTitleRequest,
) -> anyhow::Result<GenerateBlockTitleResponse> {
    features::generate_block_title(client::shared(), &config::current()?, request).await
}

/// Replaces `POST {server}/ai/generate_code_review_content`.
pub async fn generate_code_review_content(
    request: GenerateCodeReviewContentRequest,
) -> anyhow::Result<GenerateCodeReviewContentResponse> {
    features::generate_code_review_content(client::shared(), &config::current()?, request).await
}

/// Replaces `POST {server}/ai/generate_input_suggestions`.
pub async fn generate_ai_input_suggestions(
    request: &GenerateAIInputSuggestionsRequest,
) -> anyhow::Result<GenerateAIInputSuggestionsResponseV2> {
    features::generate_ai_input_suggestions(client::shared(), &config::current()?, request).await
}

/// Replaces `POST {server}/ai/generate_am_query_suggestions`.
pub async fn generate_am_query_suggestions(
    request: &GenerateAMQuerySuggestionsRequest,
) -> anyhow::Result<GenerateAMQuerySuggestionsResponse> {
    features::generate_am_query_suggestions(client::shared(), &config::current()?, request).await
}
