//! Resolving where the four small AI features send their requests.
//!
//! Two inputs, both of which already exist upstream:
//!
//! * `ai::api_keys::ApiKeyManager` holds the endpoint URL, its API key and its
//!   protocol. Upstream calls these "Custom Inference" endpoints and forwards
//!   them to `api.warp.dev` so the *server* can call the provider on the user's
//!   behalf; the fork uses the identical configuration to call it directly.
//!   Keys live in the OS keychain, never in `settings.toml`.
//! * `settings::LocalAiSettings` picks which endpoint, and which model per
//!   feature.
//!
//! [`resolve`] is a pure function of those two so the precedence rules are
//! testable without an app. [`install`] mirrors its result into a process
//! global, because the consumers — four `async fn`s on `ServerApi` — run
//! without a `Context` and cannot read a singleton.

use ai::api_keys::{ApiKeyManager, ApiKeyManagerEvent, ApiKeys, CustomEndpointSchema};
use parking_lot::RwLock;
use warpui::{AppContext, SingletonEntity as _};

use crate::settings::{LocalAiFeature, LocalAiSettings};

/// Anthropic's Messages API. Used when the user has pasted an Anthropic key
/// and configured no Custom Inference endpoint.
const ANTHROPIC_ENDPOINT: &str = "https://api.anthropic.com/v1/messages";

/// OpenAI's Chat Completions API.
const OPENAI_ENDPOINT: &str = "https://api.openai.com/v1/chat/completions";

/// OpenRouter's OpenAI-compatible Chat Completions API.
const OPENROUTER_ENDPOINT: &str = "https://openrouter.ai/api/v1/chat/completions";

/// Starting points, not recommendations — every one is overridable with
/// `agents.local_ai.model` or a per-feature override. These four features are
/// small, frequent and latency-sensitive, so the defaults are the cheap tier of
/// each provider. A model that no longer exists surfaces as the provider's own
/// 404, which names the model, so a stale default is self-diagnosing.
const DEFAULT_ANTHROPIC_MODEL: &str = "claude-haiku-4-5-20251001";
const DEFAULT_OPENAI_MODEL: &str = "gpt-4o-mini";

/// Everything needed to issue one request, snapshotted so it can be read off
/// the main thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalCompletionConfig {
    /// Full URL, including the route. Providers disagree on the path
    /// (`/v1/messages` vs `/v1/chat/completions` vs `/v1/responses`), and
    /// self-hosted servers mount them anywhere, so this is never assembled
    /// from a host.
    pub endpoint: String,
    pub api_key: String,
    pub schema: CustomEndpointSchema,
    /// Model for each feature, already resolved through the per-feature
    /// override, the shared setting, the endpoint's first model, and the
    /// provider default — in that order.
    models: FeatureModels,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct FeatureModels {
    next_command: String,
    prompt_suggestions: String,
    block_title: String,
    code_review: String,
}

impl LocalCompletionConfig {
    /// One endpoint, one model for every feature.
    #[cfg(test)]
    pub(crate) fn for_test(endpoint: &str, schema: CustomEndpointSchema, model: &str) -> Self {
        Self {
            endpoint: endpoint.to_string(),
            api_key: "test-key".to_string(),
            schema,
            models: FeatureModels {
                next_command: model.to_string(),
                prompt_suggestions: model.to_string(),
                block_title: model.to_string(),
                code_review: model.to_string(),
            },
        }
    }

    pub fn model_for(&self, feature: LocalAiFeature) -> &str {
        match feature {
            LocalAiFeature::NextCommand => &self.models.next_command,
            LocalAiFeature::PromptSuggestions => &self.models.prompt_suggestions,
            LocalAiFeature::BlockTitle => &self.models.block_title,
            LocalAiFeature::CodeReview => &self.models.code_review,
        }
    }
}

/// Why no request can be issued.
///
/// Separate from a bare `None` so each dead end names the specific thing to
/// fix. These are shown to the user in place of a suggestion or a commit
/// message, so they have to be actionable on their own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unconfigured {
    /// No API key and no Custom Inference endpoint at all.
    NothingConfigured,
    /// `agents.local_ai.endpoint` names an endpoint that does not exist.
    NoSuchEndpoint {
        name: String,
        available: Vec<String>,
    },
    /// An endpoint or key was found but no model could be determined.
    NoModel { endpoint: String },
}

impl std::fmt::Display for Unconfigured {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NothingConfigured => write!(
                f,
                "No AI endpoint is configured. Add an API key or a Custom Inference \
                 endpoint under Settings > Warp Agent, or point \
                 `agents.local_ai.endpoint` at a local server."
            ),
            Self::NoSuchEndpoint { name, available } if available.is_empty() => write!(
                f,
                "`agents.local_ai.endpoint` is set to \"{name}\", but no Custom \
                 Inference endpoints are configured."
            ),
            Self::NoSuchEndpoint { name, available } => write!(
                f,
                "`agents.local_ai.endpoint` is set to \"{name}\", which does not \
                 match any configured Custom Inference endpoint. Configured: {}.",
                available.join(", ")
            ),
            Self::NoModel { endpoint } => write!(
                f,
                "No model is configured for {endpoint}. Set `agents.local_ai.model` \
                 in settings.toml, or add a model to the endpoint under \
                 Settings > Warp Agent."
            ),
        }
    }
}

impl std::error::Error for Unconfigured {}

/// Picks an endpoint and resolves a model for each feature.
///
/// Precedence, most explicit first:
///
/// 1. the Custom Inference endpoint named by `agents.local_ai.endpoint` — an
///    explicit choice, so a name that matches nothing is an error rather than a
///    silent fall-through to a different provider;
/// 2. the first configured Custom Inference endpoint;
/// 3. a pasted Anthropic, OpenAI or OpenRouter key, in that order.
///
/// Google is deliberately absent: the Gemini API is not OpenAI-shaped at its
/// documented endpoint, so a Google key needs a Custom Inference entry naming
/// the compatibility route explicitly rather than a guess made here.
pub fn resolve(
    keys: &ApiKeys,
    settings: &LocalAiSettings,
) -> Result<LocalCompletionConfig, Unconfigured> {
    let requested = settings.endpoint_name();

    let (endpoint, api_key, schema, endpoint_model) = if !requested.is_empty() {
        let found = keys
            .custom_endpoints
            .iter()
            .find(|candidate| candidate.name.trim() == requested)
            .ok_or_else(|| Unconfigured::NoSuchEndpoint {
                name: requested.to_string(),
                available: keys
                    .custom_endpoints
                    .iter()
                    .map(|candidate| candidate.name.trim().to_string())
                    .filter(|name| !name.is_empty())
                    .collect(),
            })?;
        (
            found.url.trim().to_string(),
            found.api_key.trim().to_string(),
            found.schema,
            first_model(found),
        )
    } else if let Some(found) = keys
        .custom_endpoints
        .iter()
        .find(|candidate| !candidate.url.trim().is_empty())
    {
        (
            found.url.trim().to_string(),
            found.api_key.trim().to_string(),
            found.schema,
            first_model(found),
        )
    } else if let Some(key) = non_empty(keys.anthropic.as_deref()) {
        (
            ANTHROPIC_ENDPOINT.to_string(),
            key.to_string(),
            CustomEndpointSchema::AnthropicMessages,
            DEFAULT_ANTHROPIC_MODEL.to_string(),
        )
    } else if let Some(key) = non_empty(keys.openai.as_deref()) {
        (
            OPENAI_ENDPOINT.to_string(),
            key.to_string(),
            CustomEndpointSchema::OpenaiChatCompletions,
            DEFAULT_OPENAI_MODEL.to_string(),
        )
    } else if let Some(key) = non_empty(keys.open_router.as_deref()) {
        // OpenRouter serves hundreds of models behind one URL and has no
        // sensible default, so this deliberately leaves the model empty and
        // lets the `NoModel` branch below name the setting to fill in.
        (
            OPENROUTER_ENDPOINT.to_string(),
            key.to_string(),
            CustomEndpointSchema::OpenaiChatCompletions,
            String::new(),
        )
    } else {
        return Err(Unconfigured::NothingConfigured);
    };

    if endpoint.is_empty() {
        return Err(Unconfigured::NothingConfigured);
    }

    let model_for = |feature| {
        let configured = settings.model_for(feature);
        if configured.is_empty() {
            endpoint_model.clone()
        } else {
            configured.to_string()
        }
    };

    let models = FeatureModels {
        next_command: model_for(LocalAiFeature::NextCommand),
        prompt_suggestions: model_for(LocalAiFeature::PromptSuggestions),
        block_title: model_for(LocalAiFeature::BlockTitle),
        code_review: model_for(LocalAiFeature::CodeReview),
    };

    if LocalAiFeature::ALL.iter().any(|feature| {
        match feature {
            LocalAiFeature::NextCommand => &models.next_command,
            LocalAiFeature::PromptSuggestions => &models.prompt_suggestions,
            LocalAiFeature::BlockTitle => &models.block_title,
            LocalAiFeature::CodeReview => &models.code_review,
        }
        .is_empty()
    }) {
        return Err(Unconfigured::NoModel { endpoint });
    }

    Ok(LocalCompletionConfig {
        endpoint,
        api_key,
        schema,
        models,
    })
}

fn first_model(endpoint: &ai::api_keys::CustomEndpoint) -> String {
    endpoint
        .models
        .iter()
        .map(|model| model.name.trim())
        .find(|name| !name.is_empty())
        .unwrap_or_default()
        .to_string()
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

/// The last resolution result, refreshed whenever keys or settings change.
///
/// A process global rather than a field on some model because the consumers are
/// `async fn`s on `ServerApi`, which is constructed without a `Context` and
/// borrows none. `Err` is cached as well as `Ok` so a misconfiguration produces
/// the same actionable message every time instead of a generic one.
static CONFIG: RwLock<Option<Result<LocalCompletionConfig, Unconfigured>>> = RwLock::new(None);

/// The current configuration, or the reason there isn't one.
pub fn current() -> Result<LocalCompletionConfig, Unconfigured> {
    CONFIG
        .read()
        .clone()
        .unwrap_or(Err(Unconfigured::NothingConfigured))
}

/// Seeds [`current`] and keeps it in step with keys and settings.
///
/// Both inputs change at runtime — a key pasted into Settings emits
/// `ApiKeyManagerEvent::KeysUpdated`, and a settings edit emits
/// `LocalAiSettingsChangedEvent` — so this subscribes to both rather than
/// snapshotting once at startup. Settings groups *emit* their changed event
/// without calling `notify`, so `observe` would never fire here; the
/// subscription is required.
pub fn install(ctx: &mut AppContext) {
    refresh(ctx);

    let keys = ApiKeyManager::handle(ctx);
    ctx.subscribe_to_model(&keys, |_, event, ctx| {
        let ApiKeyManagerEvent::KeysUpdated = event;
        refresh(ctx);
    });

    let settings = LocalAiSettings::handle(ctx);
    ctx.subscribe_to_model(&settings, |_, _, ctx| refresh(ctx));
}

fn refresh(ctx: &AppContext) {
    let resolved = resolve(
        ApiKeyManager::as_ref(ctx).keys(),
        LocalAiSettings::as_ref(ctx),
    );
    *CONFIG.write() = Some(resolved);
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
