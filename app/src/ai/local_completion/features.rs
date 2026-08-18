//! The four features, each mapped onto one [`Completion`].
//!
//! Every function here takes and returns the exact upstream request and
//! response type, so the call sites in `server::server_api` swap one `await`
//! for another and nothing downstream can tell the difference.
//!
//! The prompts are the interesting part. Warp's server-side prompts are not
//! visible from the client, so these were written against the *response* types,
//! which are: what fields exist, and how does the consuming code read them.
//! Where a field cannot be filled honestly from the client — `ai_queries`
//! needs block IDs the request does not carry, `Suggestion::Coding` needs a
//! codebase index — it is left empty rather than fabricated.

use anyhow::Context as _;
use serde::Deserialize;

use super::client::{self, Completion};
use super::config::LocalCompletionConfig;
use crate::ai::generate_block_title::api::{GenerateBlockTitleRequest, GenerateBlockTitleResponse};
use crate::ai::generate_code_review_content::api::{
    GenerateCodeReviewContentRequest, GenerateCodeReviewContentResponse, OutputType,
};
use crate::ai::predict::generate_ai_input_suggestions::{
    AgentModeSuggestionV2, GenerateAIInputSuggestionsRequest, GenerateAIInputSuggestionsResponseV2,
};
use crate::ai::predict::generate_am_query_suggestions::{
    GenerateAMQuerySuggestionsRequest, GenerateAMQuerySuggestionsResponse, SimpleQuery, Suggestion,
};
use crate::settings::LocalAiFeature;

/// Terminal output is unbounded; a `cat` of a large file would otherwise become
/// the whole prompt. The tail is kept because that is where errors land.
const MAX_OUTPUT_CHARS: usize = 8_000;

/// A diff large enough to exceed this is past the point where a one-shot commit
/// message is useful anyway, and the head of a diff is the informative part.
const MAX_DIFF_CHARS: usize = 24_000;

/// Serialized `BlockContext` is verbose and only the most recent block matters.
const MAX_BLOCK_CONTEXT_CHARS: usize = 4_000;

// ---------------------------------------------------------------- block title

const BLOCK_TITLE_SYSTEM: &str = "\
You name terminal blocks. Given a shell command and its output, reply with a \
short title describing what the block did.

Rules:
- At most 6 words. No trailing period.
- Describe the action and its subject, not the literal command text.
- If the command failed, say so.
- Reply with the title alone. No quotes, no preamble, no formatting.";

pub async fn generate_block_title(
    http: &http_client::Client,
    config: &LocalCompletionConfig,
    request: GenerateBlockTitleRequest,
) -> anyhow::Result<GenerateBlockTitleResponse> {
    let user = format!(
        "Command:\n{}\n\nOutput:\n{}",
        request.command.trim(),
        tail(request.output.trim(), MAX_OUTPUT_CHARS)
    );

    let raw = client::complete(
        http,
        config,
        LocalAiFeature::BlockTitle,
        Completion {
            system: BLOCK_TITLE_SYSTEM.to_string(),
            user,
            max_tokens: 64,
            temperature: 0.2,
        },
    )
    .await?;

    Ok(GenerateBlockTitleResponse {
        title: finalize_block_title(&raw)?,
    })
}

fn finalize_block_title(raw: &str) -> anyhow::Result<String> {
    // A model that ignores "no quotes" or answers in two lines is common enough
    // to be worth handling, rather than putting a paragraph in a title field.
    let title = first_line(client::strip_code_fence(raw));
    anyhow::ensure!(!title.is_empty(), "The model returned an empty block title");
    Ok(title)
}

// ---------------------------------------------------------------- code review

const COMMIT_MESSAGE_SYSTEM: &str = "\
You write git commit messages. Given a diff, reply with the message alone.

Rules:
- First line: imperative mood, under 72 characters, no trailing period.
- Then a blank line, then a short body explaining why, only if the diff is not \
self-explanatory.
- Describe the change, not the files touched.
- No Markdown, no code fences, no preamble.";

const PR_TITLE_SYSTEM: &str = "\
You write pull request titles. Given a diff and the branch's commits, reply \
with the title alone.

Rules:
- One line, imperative mood, under 72 characters, no trailing period.
- Summarize the whole branch, not the last commit.
- No Markdown, no code fences, no preamble.";

const PR_DESCRIPTION_SYSTEM: &str = "\
You write pull request descriptions. Given a diff and the branch's commits, \
reply with the description alone.

Rules:
- Open with one paragraph on what changes and why.
- Follow with a short bullet list only if the change has genuinely separable \
parts.
- Do not restate the diff line by line, and do not invent testing that is not \
evidenced.
- Markdown is fine. No code fence around the whole reply, no preamble.";

pub async fn generate_code_review_content(
    http: &http_client::Client,
    config: &LocalCompletionConfig,
    request: GenerateCodeReviewContentRequest,
) -> anyhow::Result<GenerateCodeReviewContentResponse> {
    let system = match request.output_type {
        OutputType::CommitMessage => COMMIT_MESSAGE_SYSTEM,
        OutputType::PrTitle => PR_TITLE_SYSTEM,
        OutputType::PrDescription => PR_DESCRIPTION_SYSTEM,
    };

    let mut user = String::new();
    if !request.branch_name.trim().is_empty() {
        user.push_str(&format!("Branch: {}\n\n", request.branch_name.trim()));
    }
    if !request.commit_messages.is_empty() {
        user.push_str("Commits on this branch:\n");
        for message in &request.commit_messages {
            user.push_str(&format!("- {}\n", message.trim().replace('\n', " ")));
        }
        user.push('\n');
    }
    user.push_str("Diff:\n");
    user.push_str(head(request.diff.trim(), MAX_DIFF_CHARS));

    let max_tokens = match request.output_type {
        OutputType::PrTitle => 64,
        OutputType::CommitMessage => 512,
        OutputType::PrDescription => 1024,
    };

    let raw = client::complete(
        http,
        config,
        LocalAiFeature::CodeReview,
        Completion {
            system: system.to_string(),
            user,
            max_tokens,
            temperature: 0.2,
        },
    )
    .await?;

    Ok(GenerateCodeReviewContentResponse {
        content: finalize_code_review_content(&raw, request.output_type)?,
    })
}

fn finalize_code_review_content(raw: &str, output_type: OutputType) -> anyhow::Result<String> {
    let content = client::strip_code_fence(raw);
    let content = match output_type {
        // A title is one line by contract; a model that adds a body would
        // otherwise put a paragraph in the PR title field.
        OutputType::PrTitle => first_line(content),
        _ => content.trim().to_string(),
    };

    anyhow::ensure!(
        !content.is_empty(),
        "The model returned empty code review content"
    );
    Ok(content)
}

// --------------------------------------------------------------- next command

const NEXT_COMMAND_SYSTEM: &str = "\
You predict the next shell command a developer will run, from their recent \
terminal session.

Reply with JSON and nothing else:
{\"commands\": [\"...\", \"...\"]}

Rules:
- One to three candidates, most likely first.
- Each must be a complete, runnable command line for the shell shown in the \
context.
- If a prefix is given, every command must start with it exactly.
- Prefer a command that follows naturally from what just happened: fixing an \
error, the next step of a build, or a repeat with corrected arguments.
- Never repeat a command listed as already rejected.
- No explanation, no Markdown, no code fences.";

#[derive(Debug, Default, Deserialize)]
struct NextCommandReply {
    #[serde(default)]
    commands: Vec<String>,
}

pub async fn generate_ai_input_suggestions(
    http: &http_client::Client,
    config: &LocalCompletionConfig,
    request: &GenerateAIInputSuggestionsRequest,
) -> anyhow::Result<GenerateAIInputSuggestionsResponseV2> {
    let user = next_command_prompt(request);

    let reply: NextCommandReply = client::complete_json(
        http,
        config,
        LocalAiFeature::NextCommand,
        Completion {
            system: NEXT_COMMAND_SYSTEM.to_string(),
            user,
            max_tokens: 256,
            temperature: 0.1,
        },
    )
    .await
    .context("Could not generate a next-command suggestion")?;

    Ok(finalize_next_commands(reply, request))
}

/// Enforces the two invariants the consumer relies on but the model cannot be
/// trusted to hold: `next_command_model` asserts `most_likely_action` extends
/// the typed prefix, and a rejected suggestion reappearing is precisely what
/// the user asked not to see.
fn finalize_next_commands(
    reply: NextCommandReply,
    request: &GenerateAIInputSuggestionsRequest,
) -> GenerateAIInputSuggestionsResponseV2 {
    let prefix = request.prefix.as_deref().unwrap_or_default();
    let mut seen = Vec::new();
    let commands: Vec<String> = reply
        .commands
        .into_iter()
        .map(|command| command.trim().to_string())
        .filter(|command| !command.is_empty() && command.starts_with(prefix))
        .filter(|command| !request.rejected_suggestions.contains(command))
        .filter(|command| {
            let fresh = !seen.contains(command);
            if fresh {
                seen.push(command.clone());
            }
            fresh
        })
        .collect();

    GenerateAIInputSuggestionsResponseV2 {
        most_likely_action: commands.first().cloned().unwrap_or_default(),
        commands,
        // `AgentModeSuggestionV2` carries `context_block_ids`, and the request
        // does not include block IDs for the context it sends — the server
        // resolves those from its own copy of the session. Fabricating them
        // would produce agent queries wired to nothing, so this path offers
        // command suggestions only.
        ai_queries: Vec::<AgentModeSuggestionV2>::new(),
    }
}

fn next_command_prompt(request: &GenerateAIInputSuggestionsRequest) -> String {
    let mut user = String::new();

    if let Some(system_context) = request
        .system_context
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        user.push_str(&format!("System:\n{system_context}\n\n"));
    }

    if !request.history_context.trim().is_empty() {
        user.push_str(&format!(
            "Relevant command history:\n{}\n\n",
            request.history_context.trim()
        ));
    }

    if !request.context_messages.is_empty() {
        user.push_str("Recent blocks:\n");
        for message in &request.context_messages {
            user.push_str(&format!("{}\n", tail(message.trim(), MAX_OUTPUT_CHARS)));
        }
        user.push('\n');
    }

    if let Some(Ok(rendered)) = request.block_context.as_deref().map(serde_json::to_string) {
        user.push_str(&format!(
            "Just-completed block:\n{}\n\n",
            head(&rendered, MAX_BLOCK_CONTEXT_CHARS)
        ));
    }

    if !request.rejected_suggestions.is_empty() {
        user.push_str("Already rejected, do not repeat:\n");
        for rejected in &request.rejected_suggestions {
            user.push_str(&format!("- {rejected}\n"));
        }
        user.push('\n');
    }

    match request
        .prefix
        .as_deref()
        .map(str::trim_end)
        .filter(|prefix| !prefix.is_empty())
    {
        Some(prefix) => user.push_str(&format!(
            "The user has typed this prefix; every suggestion must start with \
             it exactly:\n{prefix}"
        )),
        None => user.push_str("The user has not typed anything yet."),
    }

    user
}

// ---------------------------------------------------------- prompt suggestions

const PROMPT_SUGGESTIONS_SYSTEM: &str = "\
You suggest what a developer might ask an AI agent next, from their recent \
terminal session.

Reply with JSON and nothing else:
{\"query\": \"...\", \"should_plan_task\": false}

Rules:
- `query` is what the developer would ask, in their voice, addressed to the \
agent. One sentence.
- Make it specific to what just happened — name the failing command, the file, \
the error. A generic suggestion is worse than none.
- Set `should_plan_task` to true only when the work plainly spans several \
steps or files.
- If nothing in the session warrants asking anything, reply {\"query\": \"\"}.
- No explanation, no Markdown, no code fences.";

#[derive(Debug, Default, Deserialize)]
struct PromptSuggestionReply {
    #[serde(default)]
    query: String,
    #[serde(default)]
    should_plan_task: bool,
}

pub async fn generate_am_query_suggestions(
    http: &http_client::Client,
    config: &LocalCompletionConfig,
    request: &GenerateAMQuerySuggestionsRequest,
) -> anyhow::Result<GenerateAMQuerySuggestionsResponse> {
    let mut user = String::new();
    if let Some(system_context) = request
        .system_context
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        user.push_str(&format!("System:\n{system_context}\n\n"));
    }
    user.push_str(&format!(
        "The last command exited with code {}.\n\n",
        request.exit_code
    ));
    user.push_str("Recent blocks:\n");
    for message in &request.context_messages {
        user.push_str(&format!("{}\n", tail(message.trim(), MAX_OUTPUT_CHARS)));
    }

    let reply: PromptSuggestionReply = client::complete_json(
        http,
        config,
        LocalAiFeature::PromptSuggestions,
        Completion {
            system: PROMPT_SUGGESTIONS_SYSTEM.to_string(),
            user,
            max_tokens: 256,
            temperature: 0.3,
        },
    )
    .await
    .context("Could not generate a prompt suggestion")?;

    Ok(finalize_prompt_suggestion(reply))
}

fn finalize_prompt_suggestion(reply: PromptSuggestionReply) -> GenerateAMQuerySuggestionsResponse {
    let query = reply.query.trim().to_string();

    GenerateAMQuerySuggestionsResponse {
        // Upstream returns a server-side request ID, used to correlate feedback
        // on the suggestion. Nothing is correlated locally, but the field is
        // read as an identity for the suggestion, so it still has to be unique.
        id: uuid::Uuid::new_v4().to_string(),
        // `Suggestion::Coding` additionally requires file locations, which
        // upstream draws from a server-side codebase index. Without one, a
        // coding suggestion would carry no files and be discarded by
        // `is_valid_code_delegation` anyway.
        suggestion: (!query.is_empty()).then_some(Suggestion::Simple(SimpleQuery {
            query,
            should_plan_task: reply.should_plan_task,
        })),
    }
}

// -------------------------------------------------------------------- helpers

/// First non-empty line, unquoted. Models add quotes around titles even when
/// told not to, and answer in two lines when told to answer in one.
fn first_line(value: &str) -> String {
    value
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .trim_matches(|c| c == '"' || c == '\'')
        .trim()
        .to_string()
}

/// Keeps the last `limit` characters, on a character boundary.
fn tail(value: &str, limit: usize) -> &str {
    if value.chars().count() <= limit {
        return value;
    }
    let start = value
        .char_indices()
        .nth(value.chars().count() - limit)
        .map_or(0, |(index, _)| index);
    &value[start..]
}

/// Keeps the first `limit` characters, on a character boundary.
fn head(value: &str, limit: usize) -> &str {
    match value.char_indices().nth(limit) {
        Some((index, _)) => &value[..index],
        None => value,
    }
}

#[cfg(test)]
#[path = "features_tests.rs"]
mod tests;
