//! One request, three wire protocols.
//!
//! The four features this module serves all want the same thing — a system
//! prompt, a user prompt, and a string back. What differs is only the shape the
//! provider expects, and `ai::api_keys::CustomEndpointSchema` already
//! enumerates the three Warp supports. This translates between them.
//!
//! "Local" here means *issued from this machine*, not *inferred on it*: the
//! same code path serves a llama.cpp server on loopback and `api.anthropic.com`
//! with the user's own key. Either way `api.warp.dev` is not in the path, which
//! is the property being bought.

use std::sync::OnceLock;
use std::time::Duration;

use ai::api_keys::CustomEndpointSchema;
use anyhow::{Context as _, anyhow, bail};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::config::LocalCompletionConfig;
use crate::settings::LocalAiFeature;

/// These are interactive: a suggestion that arrives a minute late is worse than
/// none. Long enough for a large diff through a CPU-only local model, short
/// enough that a wedged endpoint does not pin the caller forever.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// Only the tail of an error body is ever surfaced; providers return HTML error
/// pages and local servers return unbounded diagnostics.
const MAX_ERROR_DETAIL_BYTES: usize = 1024;

/// Shared so the connection pool survives between requests. Next Command fires
/// on nearly every prompt, and a fresh TLS handshake each time would dominate
/// its latency.
pub fn shared() -> &'static http_client::Client {
    static HTTP: OnceLock<http_client::Client> = OnceLock::new();
    HTTP.get_or_init(http_client::Client::new)
}

/// A single-turn completion request, before it is shaped for a provider.
pub struct Completion {
    pub system: String,
    pub user: String,
    pub max_tokens: u32,
    /// Low across the board: every one of these features wants a predictable
    /// answer in a fixed format, not a creative one.
    pub temperature: f32,
}

/// Issues `completion` against the configured endpoint and returns the text.
///
/// The client is a parameter rather than [`shared`] so the request this
/// actually puts on the wire can be asserted against a stub server. Field names
/// are the whole risk here — a provider that does not recognise one ignores it
/// silently, so a wrong `max_tokens` shows up as a mysteriously truncated
/// answer months later, not as an error.
pub async fn complete(
    http: &http_client::Client,
    config: &LocalCompletionConfig,
    feature: LocalAiFeature,
    completion: Completion,
) -> anyhow::Result<String> {
    let model = config.model_for(feature);
    let body = match config.schema {
        CustomEndpointSchema::OpenaiChatCompletions => json!({
            "model": model,
            "messages": [
                {"role": "system", "content": completion.system},
                {"role": "user", "content": completion.user},
            ],
            // `max_tokens` rather than `max_completion_tokens`: llama.cpp,
            // Ollama, LM Studio and vLLM all accept the former and most do not
            // recognise the latter, and self-hosted servers are the common case
            // for this schema.
            "max_tokens": completion.max_tokens,
            "temperature": completion.temperature,
            "stream": false,
        }),
        CustomEndpointSchema::OpenaiResponses => json!({
            "model": model,
            "instructions": completion.system,
            "input": completion.user,
            "max_output_tokens": completion.max_tokens,
            "temperature": completion.temperature,
            "stream": false,
        }),
        CustomEndpointSchema::AnthropicMessages => json!({
            "model": model,
            "system": completion.system,
            "messages": [{"role": "user", "content": completion.user}],
            "max_tokens": completion.max_tokens,
            "temperature": completion.temperature,
            "stream": false,
        }),
    };

    let mut request = http.post(config.endpoint.as_str()).timeout(REQUEST_TIMEOUT);
    if !config.api_key.is_empty() {
        request = match config.schema {
            CustomEndpointSchema::AnthropicMessages => {
                request.header("x-api-key", config.api_key.as_str())
            }
            _ => request.bearer_auth(config.api_key.as_str()),
        };
    }
    if matches!(config.schema, CustomEndpointSchema::AnthropicMessages) {
        request = request.header("anthropic-version", "2023-06-01");
    }

    let response = request.json(&body).send().await.with_context(|| {
        format!(
            "Could not reach the AI endpoint at {}. Is it running and reachable?",
            config.endpoint
        )
    })?;

    let status = response.status();
    let text = response
        .text()
        .await
        .with_context(|| format!("Could not read the response from {}", config.endpoint))?;

    if !status.is_success() {
        bail!(
            "{} returned {status}: {}",
            config.endpoint,
            truncate_detail(&text)
        );
    }

    let parsed: Value = serde_json::from_str(&text).with_context(|| {
        format!(
            "{} returned a non-JSON body: {}",
            config.endpoint,
            truncate_detail(&text)
        )
    })?;

    let content = extract_text(config.schema, &parsed).ok_or_else(|| {
        anyhow!(
            "{} returned no completion text: {}",
            config.endpoint,
            truncate_detail(&text)
        )
    })?;

    Ok(content)
}

/// Issues `completion` and parses the reply as JSON.
///
/// Models routinely wrap JSON in a Markdown fence even when told not to, and
/// small local models add a sentence of preamble, so this recovers the object
/// rather than failing on either. Anything it cannot recover is an error with
/// the raw text attached, which is the only way to debug a local model that has
/// gone off-format.
pub async fn complete_json<T: DeserializeOwned>(
    http: &http_client::Client,
    config: &LocalCompletionConfig,
    feature: LocalAiFeature,
    completion: Completion,
) -> anyhow::Result<T> {
    let raw = complete(http, config, feature, completion).await?;
    let json = extract_json_object(&raw)
        .ok_or_else(|| anyhow!("Expected JSON, got: {}", truncate_detail(&raw)))?;
    serde_json::from_str(json).with_context(|| {
        format!(
            "Could not parse the model's JSON: {}",
            truncate_detail(json)
        )
    })
}

/// Pulls the assistant text out of whichever response shape came back.
fn extract_text(schema: CustomEndpointSchema, body: &Value) -> Option<String> {
    let text = match schema {
        CustomEndpointSchema::OpenaiChatCompletions => body
            .get("choices")?
            .get(0)?
            .get("message")?
            .get("content")?
            .as_str()?
            .to_string(),
        CustomEndpointSchema::AnthropicMessages => body
            .get("content")?
            .as_array()?
            .iter()
            .filter_map(|block| block.get("text")?.as_str())
            .collect::<Vec<_>>()
            .join(""),
        // The Responses API nests text two levels deep and interleaves
        // non-message items (reasoning summaries, tool calls) in the same
        // array, so this walks rather than indexes.
        CustomEndpointSchema::OpenaiResponses => {
            if let Some(text) = body.get("output_text").and_then(Value::as_str) {
                text.to_string()
            } else {
                body.get("output")?
                    .as_array()?
                    .iter()
                    .filter_map(|item| item.get("content")?.as_array())
                    .flatten()
                    .filter_map(|block| block.get("text")?.as_str())
                    .collect::<Vec<_>>()
                    .join("")
            }
        }
    };

    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// Finds the outermost JSON object in `raw`, ignoring fences and prose.
///
/// Scans for balanced braces while tracking string state, so a `}` inside a
/// quoted value — a shell command in a suggestion, say — does not end the
/// object early.
fn extract_json_object(raw: &str) -> Option<&str> {
    let bytes = raw.as_bytes();
    let start = raw.find('{')?;

    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for index in start..bytes.len() {
        let byte = bytes[index];
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&raw[start..=index]);
                }
            }
            _ => {}
        }
    }

    None
}

/// Strips a Markdown fence and surrounding prose from a plain-text reply.
///
/// Used by the features that want prose rather than JSON — a commit message
/// should not arrive wrapped in triple backticks just because the model likes
/// formatting code.
pub fn strip_code_fence(raw: &str) -> &str {
    let trimmed = raw.trim();
    let Some(rest) = trimmed.strip_prefix("```") else {
        return trimmed;
    };
    // The opening fence may carry a language tag; drop the remainder of that line.
    let rest = rest.split_once('\n').map_or("", |(_, body)| body);
    rest.trim_end().strip_suffix("```").unwrap_or(rest).trim()
}

fn truncate_detail(detail: &str) -> String {
    let detail = detail.trim();
    if detail.len() <= MAX_ERROR_DETAIL_BYTES {
        return detail.to_string();
    }
    let mut cut = detail.len() - MAX_ERROR_DETAIL_BYTES;
    while cut < detail.len() && !detail.is_char_boundary(cut) {
        cut += 1;
    }
    format!("...{}", &detail[cut..])
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
