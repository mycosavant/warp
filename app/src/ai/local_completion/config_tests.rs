use ai::api_keys::{CustomEndpoint, CustomEndpointModel};
use settings::Setting as _;

use super::*;
use crate::settings::{
    LocalAiEndpointName, LocalAiModel, LocalAiModelBlockTitle, LocalAiModelCodeReview,
    LocalAiModelNextCommand, LocalAiModelPromptSuggestions,
};

fn no_settings() -> LocalAiSettings {
    settings_with("", "")
}

fn settings_with(endpoint_name: &str, model: &str) -> LocalAiSettings {
    LocalAiSettings {
        local_ai_endpoint_name: LocalAiEndpointName::new(Some(endpoint_name.into())),
        local_ai_model: LocalAiModel::new(Some(model.into())),
        local_ai_model_next_command: LocalAiModelNextCommand::new(Some(String::new())),
        local_ai_model_prompt_suggestions: LocalAiModelPromptSuggestions::new(Some(String::new())),
        local_ai_model_block_title: LocalAiModelBlockTitle::new(Some(String::new())),
        local_ai_model_code_review: LocalAiModelCodeReview::new(Some(String::new())),
    }
}

fn endpoint(name: &str, url: &str, models: &[&str]) -> CustomEndpoint {
    CustomEndpoint {
        name: name.to_string(),
        url: url.to_string(),
        api_key: format!("{name}-key"),
        models: models
            .iter()
            .map(|model| CustomEndpointModel {
                name: (*model).to_string(),
                alias: None,
                config_key: format!("{model}-config"),
            })
            .collect(),
        schema: CustomEndpointSchema::OpenaiChatCompletions,
    }
}

/// The whole feature has to be inert until the user configures something. A
/// default that silently picked a provider would be a surprise egress.
#[test]
fn nothing_configured_is_an_error_not_a_default() {
    let error = resolve(&ApiKeys::default(), &no_settings()).unwrap_err();
    assert_eq!(error, Unconfigured::NothingConfigured);
    assert!(
        error.to_string().contains("Settings > Warp Agent"),
        "{error}"
    );
}

#[test]
fn a_single_custom_endpoint_is_used_without_naming_it() {
    let keys = ApiKeys {
        custom_endpoints: vec![endpoint(
            "local",
            "http://127.0.0.1:8080/v1/chat/completions",
            &["qwen"],
        )],
        ..Default::default()
    };

    let config = resolve(&keys, &no_settings()).unwrap();
    assert_eq!(config.endpoint, "http://127.0.0.1:8080/v1/chat/completions");
    assert_eq!(config.api_key, "local-key");
    assert_eq!(config.schema, CustomEndpointSchema::OpenaiChatCompletions);
    for feature in LocalAiFeature::ALL {
        assert_eq!(config.model_for(feature), "qwen", "{feature:?}");
    }
}

#[test]
fn a_named_endpoint_wins_over_the_first_one() {
    let keys = ApiKeys {
        custom_endpoints: vec![
            endpoint("first", "http://127.0.0.1:1/v1/chat/completions", &["a"]),
            endpoint("second", "http://127.0.0.1:2/v1/chat/completions", &["b"]),
        ],
        ..Default::default()
    };

    let config = resolve(&keys, &settings_with("second", "")).unwrap();
    assert_eq!(config.endpoint, "http://127.0.0.1:2/v1/chat/completions");
    assert_eq!(config.model_for(LocalAiFeature::BlockTitle), "b");
}

/// Naming an endpoint is an explicit choice. Falling through to a different
/// provider because of a typo would send the payload somewhere the user did not
/// pick — the one outcome this module exists to prevent.
#[test]
fn a_named_endpoint_that_does_not_exist_is_an_error_not_a_fallback() {
    let keys = ApiKeys {
        anthropic: Some("sk-ant-xxx".into()),
        custom_endpoints: vec![endpoint("first", "http://127.0.0.1:1/v1", &["a"])],
        ..Default::default()
    };

    let error = resolve(&keys, &settings_with("typo", "")).unwrap_err();
    assert_eq!(
        error,
        Unconfigured::NoSuchEndpoint {
            name: "typo".into(),
            available: vec!["first".into()],
        }
    );
    // The message has to list what *is* configured, or the user cannot tell a
    // typo from a missing endpoint.
    assert!(error.to_string().contains("first"), "{error}");
}

#[test]
fn a_pasted_anthropic_key_needs_no_other_configuration() {
    let keys = ApiKeys {
        anthropic: Some("sk-ant-xxx".into()),
        ..Default::default()
    };

    let config = resolve(&keys, &no_settings()).unwrap();
    assert_eq!(config.endpoint, ANTHROPIC_ENDPOINT);
    assert_eq!(config.api_key, "sk-ant-xxx");
    assert_eq!(config.schema, CustomEndpointSchema::AnthropicMessages);
    assert_eq!(
        config.model_for(LocalAiFeature::NextCommand),
        DEFAULT_ANTHROPIC_MODEL
    );
}

#[test]
fn a_pasted_openai_key_needs_no_other_configuration() {
    let keys = ApiKeys {
        openai: Some("sk-xxx".into()),
        ..Default::default()
    };

    let config = resolve(&keys, &no_settings()).unwrap();
    assert_eq!(config.endpoint, OPENAI_ENDPOINT);
    assert_eq!(config.schema, CustomEndpointSchema::OpenaiChatCompletions);
    assert_eq!(
        config.model_for(LocalAiFeature::NextCommand),
        DEFAULT_OPENAI_MODEL
    );
}

#[test]
fn a_custom_endpoint_outranks_a_pasted_key() {
    let keys = ApiKeys {
        anthropic: Some("sk-ant-xxx".into()),
        openai: Some("sk-xxx".into()),
        custom_endpoints: vec![endpoint("local", "http://127.0.0.1:8080/v1", &["qwen"])],
        ..Default::default()
    };

    assert_eq!(
        resolve(&keys, &no_settings()).unwrap().endpoint,
        "http://127.0.0.1:8080/v1"
    );
}

/// OpenRouter serves hundreds of models behind one URL, so there is no honest
/// default. It has to ask rather than pick.
#[test]
fn openrouter_without_a_model_asks_for_one() {
    let keys = ApiKeys {
        open_router: Some("sk-or-xxx".into()),
        ..Default::default()
    };

    let error = resolve(&keys, &no_settings()).unwrap_err();
    assert_eq!(
        error,
        Unconfigured::NoModel {
            endpoint: OPENROUTER_ENDPOINT.into()
        }
    );
    assert!(
        error.to_string().contains("agents.local_ai.model"),
        "{error}"
    );

    let config = resolve(&keys, &settings_with("", "anthropic/claude-haiku-4.5")).unwrap();
    assert_eq!(config.endpoint, OPENROUTER_ENDPOINT);
    assert_eq!(
        config.model_for(LocalAiFeature::CodeReview),
        "anthropic/claude-haiku-4.5"
    );
}

/// A Custom Inference endpoint with no models listed is a real state — the UI
/// lets you save one — and it must ask rather than send `"model": ""`.
#[test]
fn an_endpoint_with_no_models_asks_for_one() {
    let keys = ApiKeys {
        custom_endpoints: vec![endpoint("local", "http://127.0.0.1:8080/v1", &[])],
        ..Default::default()
    };

    assert_eq!(
        resolve(&keys, &no_settings()).unwrap_err(),
        Unconfigured::NoModel {
            endpoint: "http://127.0.0.1:8080/v1".into()
        }
    );
}

#[test]
fn the_settings_model_overrides_the_endpoints_own() {
    let keys = ApiKeys {
        custom_endpoints: vec![endpoint("local", "http://127.0.0.1:8080/v1", &["qwen"])],
        ..Default::default()
    };

    let mut settings = settings_with("", "");
    settings.local_ai_model_code_review = LocalAiModelCodeReview::new(Some("bigger".into()));

    let config = resolve(&keys, &settings).unwrap();
    assert_eq!(config.model_for(LocalAiFeature::CodeReview), "bigger");
    assert_eq!(config.model_for(LocalAiFeature::BlockTitle), "qwen");
}

/// Google is deliberately not in the fallback chain: the Gemini API is not
/// OpenAI-shaped at its documented endpoint, and guessing a compatibility route
/// here would fail at request time instead of at configuration time.
#[test]
fn a_google_key_alone_is_not_enough() {
    let keys = ApiKeys {
        google: Some("AIzaSyxxx".into()),
        ..Default::default()
    };

    assert_eq!(
        resolve(&keys, &no_settings()).unwrap_err(),
        Unconfigured::NothingConfigured
    );
}

#[test]
fn whitespace_only_keys_are_not_keys() {
    let keys = ApiKeys {
        anthropic: Some("   ".into()),
        openai: Some("\n".into()),
        ..Default::default()
    };

    assert_eq!(
        resolve(&keys, &no_settings()).unwrap_err(),
        Unconfigured::NothingConfigured
    );
}

/// An endpoint saved with a name but no URL should not shadow a working key.
#[test]
fn an_endpoint_with_no_url_is_skipped() {
    let keys = ApiKeys {
        anthropic: Some("sk-ant-xxx".into()),
        custom_endpoints: vec![endpoint("half-finished", "  ", &["a"])],
        ..Default::default()
    };

    assert_eq!(
        resolve(&keys, &no_settings()).unwrap().endpoint,
        ANTHROPIC_ENDPOINT
    );
}
