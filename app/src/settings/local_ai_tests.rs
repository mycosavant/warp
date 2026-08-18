use settings::{Setting, SyncToCloud};

use super::*;

/// Every one of these is empty out of the box: the fork does not guess at a
/// provider on the user's behalf. An unset group means `resolve` falls through
/// to whatever key is already in the keychain, which is the zero-configuration
/// path.
#[test]
fn everything_starts_unset() {
    assert!(LocalAiEndpointName::default_value().is_empty());
    assert!(LocalAiModel::default_value().is_empty());
    assert!(LocalAiModelNextCommand::default_value().is_empty());
    assert!(LocalAiModelPromptSuggestions::default_value().is_empty());
    assert!(LocalAiModelBlockTitle::default_value().is_empty());
    assert!(LocalAiModelCodeReview::default_value().is_empty());
}

/// These name an endpoint on one machine and a model choice that is a matter of
/// local cost. Syncing them would push that to a server the fork otherwise
/// never talks to.
#[test]
fn nothing_here_syncs_to_the_cloud() {
    assert_eq!(LocalAiEndpointName::sync_to_cloud(), SyncToCloud::Never);
    assert_eq!(LocalAiModel::sync_to_cloud(), SyncToCloud::Never);
    assert_eq!(LocalAiModelNextCommand::sync_to_cloud(), SyncToCloud::Never);
    assert_eq!(
        LocalAiModelPromptSuggestions::sync_to_cloud(),
        SyncToCloud::Never
    );
    assert_eq!(LocalAiModelBlockTitle::sync_to_cloud(), SyncToCloud::Never);
    assert_eq!(LocalAiModelCodeReview::sync_to_cloud(), SyncToCloud::Never);
}

#[test]
fn every_setting_lives_under_the_same_toml_table() {
    for path in [
        LocalAiEndpointName::toml_path(),
        LocalAiModel::toml_path(),
        LocalAiModelNextCommand::toml_path(),
        LocalAiModelPromptSuggestions::toml_path(),
        LocalAiModelBlockTitle::toml_path(),
        LocalAiModelCodeReview::toml_path(),
    ] {
        let path = path.expect("local AI settings are user-visible");
        assert!(path.starts_with("agents.local_ai."), "{path}");
    }
}

/// No API key or endpoint URL may be declared here — `settings.toml` is
/// plaintext on disk. Both come from `ApiKeyManager`, which uses the OS
/// keychain. This asserts the shape of the group, so adding a `..._key` or
/// `..._url` setting later fails loudly rather than quietly writing a secret to
/// a readable file.
#[test]
fn no_setting_here_could_hold_a_secret_or_a_url() {
    for path in [
        LocalAiEndpointName::toml_path(),
        LocalAiModel::toml_path(),
        LocalAiModelNextCommand::toml_path(),
        LocalAiModelPromptSuggestions::toml_path(),
        LocalAiModelBlockTitle::toml_path(),
        LocalAiModelCodeReview::toml_path(),
    ] {
        let path = path.expect("local AI settings are user-visible");
        for forbidden in ["key", "token", "secret", "password", "url"] {
            assert!(
                !path.contains(forbidden),
                "{path} looks like it holds a secret or an endpoint URL; those \
                 belong in ApiKeyManager, not settings.toml"
            );
        }
    }
}

fn settings(shared: &str, next_command: &str, code_review: &str) -> LocalAiSettings {
    LocalAiSettings {
        local_ai_endpoint_name: LocalAiEndpointName::new(Some("  my-server  ".into())),
        local_ai_model: LocalAiModel::new(Some(shared.into())),
        local_ai_model_next_command: LocalAiModelNextCommand::new(Some(next_command.into())),
        local_ai_model_prompt_suggestions: LocalAiModelPromptSuggestions::new(Some(String::new())),
        local_ai_model_block_title: LocalAiModelBlockTitle::new(Some(String::new())),
        local_ai_model_code_review: LocalAiModelCodeReview::new(Some(code_review.into())),
    }
}

#[test]
fn a_per_feature_model_overrides_the_shared_one() {
    let settings = settings("shared-model", "  tiny-model  ", "big-model");
    assert_eq!(
        settings.model_for(LocalAiFeature::NextCommand),
        "tiny-model"
    );
    assert_eq!(settings.model_for(LocalAiFeature::CodeReview), "big-model");
}

#[test]
fn an_unset_per_feature_model_falls_back_to_the_shared_one() {
    let settings = settings("shared-model", "tiny-model", "");
    assert_eq!(
        settings.model_for(LocalAiFeature::PromptSuggestions),
        "shared-model"
    );
    assert_eq!(
        settings.model_for(LocalAiFeature::BlockTitle),
        "shared-model"
    );
    assert_eq!(
        settings.model_for(LocalAiFeature::CodeReview),
        "shared-model"
    );
}

/// With nothing configured anywhere, `model_for` returns `""` rather than a
/// guess — `resolve` reads that as "use the endpoint's own model".
#[test]
fn nothing_configured_resolves_to_the_empty_string() {
    let settings = settings("", "", "");
    for feature in LocalAiFeature::ALL {
        assert_eq!(settings.model_for(feature), "", "{feature:?}");
    }
}

#[test]
fn accessors_trim_surrounding_whitespace() {
    let settings = settings("  shared  ", "", "");
    assert_eq!(settings.endpoint_name(), "my-server");
    assert_eq!(settings.model_for(LocalAiFeature::BlockTitle), "shared");
}
