//! Fork-local settings for the four small AI features.
//!
//! "Next Command", "Prompt Suggestions", "Shared Block Title Generation" and
//! "Commit & PR Generation" are each a single-shot POST to an `/ai/*` route on
//! `api.warp.dev`. No streaming, no tool use, no session state — Warp's server
//! is acting purely as an authenticated proxy in front of a model.
//!
//! These settings point that proxy at a model the user reaches directly.
//! Deliberately a separate group rather than fields on `AISettings`: a new file
//! cannot conflict on an upstream merge.
//!
//! **No secrets live here.** `settings.toml` is plaintext. The endpoint URL and
//! its API key come from `ai::api_keys::ApiKeyManager`, which already stores
//! them in the OS keychain and already has a settings UI ("Custom Inference" on
//! the Warp Agent page). These settings only *select* among what is stored
//! there, and override the model per feature.
//!
//! See `ai::local_completion` for the consumer and
//! `fork::local_ai_completions_enabled` for the enablement policy.

use settings::macros::define_settings_group;
use settings::{SupportedPlatforms, SyncToCloud};

/// One of the four features routed by `ai::local_completion`.
///
/// Not a setting itself — it selects which per-feature model override applies.
/// Kept here beside the overrides so adding a feature is a single-file change.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum LocalAiFeature {
    /// "Next Command" — `POST /ai/generate_input_suggestions`.
    NextCommand,
    /// "Prompt Suggestions" — `POST /ai/generate_am_query_suggestions`.
    PromptSuggestions,
    /// "Shared Block Title Generation" — `POST /ai/generate_block_title`.
    BlockTitle,
    /// "Commit & PR Generation" — `POST /ai/generate_code_review_content`.
    CodeReview,
}

impl LocalAiFeature {
    pub const ALL: [Self; 4] = [
        Self::NextCommand,
        Self::PromptSuggestions,
        Self::BlockTitle,
        Self::CodeReview,
    ];

    /// The name used in log lines and error messages.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NextCommand => "next_command",
            Self::PromptSuggestions => "prompt_suggestions",
            Self::BlockTitle => "block_title",
            Self::CodeReview => "code_review",
        }
    }
}

define_settings_group!(LocalAiSettings, settings: [
    local_ai_endpoint_name: LocalAiEndpointName {
        type: String,
        default: String::new(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::ALL,
        private: false,
        toml_path: "agents.local_ai.endpoint",
        description: "Name of the Custom Inference endpoint to use for the small AI features. Leave empty to use the first configured endpoint, or a pasted provider API key if there is none.",
    },
    local_ai_model: LocalAiModel {
        type: String,
        default: String::new(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::ALL,
        private: false,
        toml_path: "agents.local_ai.model",
        description: "Model used for the small AI features. Leave empty to use the endpoint's first model, or the provider's default.",
    },
    local_ai_model_next_command: LocalAiModelNextCommand {
        type: String,
        default: String::new(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::ALL,
        private: false,
        toml_path: "agents.local_ai.models.next_command",
        description: "Overrides `agents.local_ai.model` for Next Command. This one runs on every prompt, so a small fast model is usually the right choice.",
    },
    local_ai_model_prompt_suggestions: LocalAiModelPromptSuggestions {
        type: String,
        default: String::new(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::ALL,
        private: false,
        toml_path: "agents.local_ai.models.prompt_suggestions",
        description: "Overrides `agents.local_ai.model` for Prompt Suggestions.",
    },
    local_ai_model_block_title: LocalAiModelBlockTitle {
        type: String,
        default: String::new(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::ALL,
        private: false,
        toml_path: "agents.local_ai.models.block_title",
        description: "Overrides `agents.local_ai.model` for Shared Block Title Generation.",
    },
    local_ai_model_code_review: LocalAiModelCodeReview {
        type: String,
        default: String::new(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::ALL,
        private: false,
        toml_path: "agents.local_ai.models.code_review",
        description: "Overrides `agents.local_ai.model` for Commit & PR Generation. This one reads a whole diff, so a larger model usually pays off.",
    },
]);

impl LocalAiSettings {
    /// Name of the Custom Inference endpoint to use, or `""` for "pick one".
    pub fn endpoint_name(&self) -> &str {
        self.local_ai_endpoint_name.trim()
    }

    /// The model for `feature`, falling back to the shared model and then to
    /// `""`, which means "let the endpoint or provider decide".
    pub fn model_for(&self, feature: LocalAiFeature) -> &str {
        let specific = match feature {
            LocalAiFeature::NextCommand => self.local_ai_model_next_command.trim(),
            LocalAiFeature::PromptSuggestions => self.local_ai_model_prompt_suggestions.trim(),
            LocalAiFeature::BlockTitle => self.local_ai_model_block_title.trim(),
            LocalAiFeature::CodeReview => self.local_ai_model_code_review.trim(),
        };

        if specific.is_empty() {
            self.local_ai_model.trim()
        } else {
            specific
        }
    }
}

#[cfg(test)]
#[path = "local_ai_tests.rs"]
mod tests;
