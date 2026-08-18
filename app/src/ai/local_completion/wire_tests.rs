//! What actually goes on the wire, asserted against a stub server.
//!
//! The unit tests elsewhere in this module cover parsing, which fails loudly.
//! These cover the *request*, which does not: a provider that receives a field
//! it does not recognise ignores it silently, so a wrong `max_tokens` key
//! surfaces months later as answers that are mysteriously short. Every
//! assertion here is a field name a real provider would have to see.

use ai::api_keys::CustomEndpointSchema;
use futures::executor::block_on;
use mockito::{Matcher, Server};
use serde_json::json;

use super::client::{self, Completion};
use super::config::LocalCompletionConfig;
use super::features;
use crate::ai::generate_block_title::api::GenerateBlockTitleRequest;
use crate::ai::generate_code_review_content::api::{GenerateCodeReviewContentRequest, OutputType};
use crate::ai::predict::generate_ai_input_suggestions::GenerateAIInputSuggestionsRequest;
use crate::ai::predict::generate_am_query_suggestions::{
    GenerateAMQuerySuggestionsRequest, Suggestion,
};
use crate::settings::LocalAiFeature;

fn completion() -> Completion {
    Completion {
        system: "be terse".into(),
        user: "say hello".into(),
        max_tokens: 42,
        temperature: 0.25,
    }
}

/// llama.cpp, Ollama, LM Studio and vLLM all read `messages`, `max_tokens` and
/// `temperature` under these exact names. A rename in any of them is a silent
/// downgrade, not an error.
#[test]
fn the_openai_chat_completions_request_uses_the_field_names_providers_read() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/v1/chat/completions")
        .match_header("authorization", "Bearer test-key")
        // Full-body equality, not a subset: it has to prove both that the
        // fields providers read are present under these names and that nothing
        // else is sent.
        .match_body(Matcher::Json(json!({
            "model": "qwen2.5-coder",
            "messages": [
                {"role": "system", "content": "be terse"},
                {"role": "user", "content": "say hello"},
            ],
            "max_tokens": 42,
            "temperature": 0.25,
            "stream": false,
        })))
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":"hello"}}]}"#)
        .with_header("content-type", "application/json")
        .create();

    let config = LocalCompletionConfig::for_test(
        &format!("{}/v1/chat/completions", server.url()),
        CustomEndpointSchema::OpenaiChatCompletions,
        "qwen2.5-coder",
    );

    let text = block_on(client::complete(
        &http_client::Client::new_for_test(),
        &config,
        LocalAiFeature::BlockTitle,
        completion(),
    ))
    .unwrap();

    assert_eq!(text, "hello");
    mock.assert();
}

/// Anthropic authenticates with `x-api-key`, not a bearer token, and rejects a
/// request with no `anthropic-version`. Both are easy to get wrong and both
/// fail at request time with a 401 that says nothing about which one.
#[test]
fn the_anthropic_request_carries_the_api_key_header_and_a_version() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/v1/messages")
        .match_header("x-api-key", "test-key")
        .match_header("anthropic-version", "2023-06-01")
        // Anthropic takes the system prompt as a top-level field, not as a
        // message with `role: "system"` — sent as a message it is accepted and
        // ignored, so the equality here is the assertion that matters.
        .match_body(Matcher::Json(json!({
            "model": "claude-haiku-4-5-20251001",
            "system": "be terse",
            "messages": [{"role": "user", "content": "say hello"}],
            "max_tokens": 42,
            "temperature": 0.25,
            "stream": false,
        })))
        .with_status(200)
        .with_body(r#"{"content":[{"type":"text","text":"hello"}]}"#)
        .create();

    let config = LocalCompletionConfig::for_test(
        &format!("{}/v1/messages", server.url()),
        CustomEndpointSchema::AnthropicMessages,
        "claude-haiku-4-5-20251001",
    );

    let text = block_on(client::complete(
        &http_client::Client::new_for_test(),
        &config,
        LocalAiFeature::BlockTitle,
        completion(),
    ))
    .unwrap();

    assert_eq!(text, "hello");
    mock.assert();
}

/// The Responses API renames both of the fields that matter: `instructions`
/// for the system prompt and `max_output_tokens` for the limit.
#[test]
fn the_responses_request_uses_instructions_and_max_output_tokens() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/v1/responses")
        .match_header("authorization", "Bearer test-key")
        .match_body(Matcher::Json(json!({
            "model": "gpt-4o-mini",
            "instructions": "be terse",
            "input": "say hello",
            "max_output_tokens": 42,
            "temperature": 0.25,
            "stream": false,
        })))
        .with_status(200)
        .with_body(r#"{"output_text":"hello"}"#)
        .create();

    let config = LocalCompletionConfig::for_test(
        &format!("{}/v1/responses", server.url()),
        CustomEndpointSchema::OpenaiResponses,
        "gpt-4o-mini",
    );

    let text = block_on(client::complete(
        &http_client::Client::new_for_test(),
        &config,
        LocalAiFeature::BlockTitle,
        completion(),
    ))
    .unwrap();

    assert_eq!(text, "hello");
    mock.assert();
}

/// A provider error is the common failure — a wrong model name, an expired key,
/// a server that is up but has no model loaded. The provider's own message is
/// the only thing that says which, so it has to reach the user.
#[test]
fn a_provider_error_body_reaches_the_caller() {
    let mut server = Server::new();
    let _mock = server
        .mock("POST", "/v1/chat/completions")
        .with_status(404)
        .with_body(r#"{"error":{"message":"model 'qwen2.5-coder' not found"}}"#)
        .create();

    let config = LocalCompletionConfig::for_test(
        &format!("{}/v1/chat/completions", server.url()),
        CustomEndpointSchema::OpenaiChatCompletions,
        "qwen2.5-coder",
    );

    let error = block_on(client::complete(
        &http_client::Client::new_for_test(),
        &config,
        LocalAiFeature::BlockTitle,
        completion(),
    ))
    .unwrap_err()
    .to_string();

    assert!(error.contains("404"), "{error}");
    assert!(error.contains("model 'qwen2.5-coder' not found"), "{error}");
}

/// An endpoint that is not listening is the other common failure, and the
/// message has to say which endpoint so a typo is findable.
#[test]
fn an_unreachable_endpoint_names_itself() {
    let config = LocalCompletionConfig::for_test(
        "http://127.0.0.1:1/v1/chat/completions",
        CustomEndpointSchema::OpenaiChatCompletions,
        "qwen",
    );

    let error = format!(
        "{:#}",
        block_on(client::complete(
            &http_client::Client::new_for_test(),
            &config,
            LocalAiFeature::BlockTitle,
            completion(),
        ))
        .unwrap_err()
    );

    assert!(error.contains("127.0.0.1:1"), "{error}");
}

// --------------------------------------------------- end to end, per feature

#[test]
fn a_block_title_round_trips_from_command_and_output_to_a_title() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/v1/chat/completions")
        .match_body(Matcher::AllOf(vec![
            Matcher::Regex("npm install".into()),
            Matcher::Regex("added 214 packages".into()),
        ]))
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":"\"Install project dependencies\""}}]}"#)
        .create();

    let config = LocalCompletionConfig::for_test(
        &format!("{}/v1/chat/completions", server.url()),
        CustomEndpointSchema::OpenaiChatCompletions,
        "qwen",
    );

    let response = block_on(features::generate_block_title(
        &http_client::Client::new_for_test(),
        &config,
        GenerateBlockTitleRequest {
            command: "npm install".into(),
            output: "added 214 packages in 3s".into(),
        },
    ))
    .unwrap();

    mock.assert();
    assert_eq!(response.title, "Install project dependencies");
}

/// The diff, the branch and the commits all have to reach the model, or the
/// message describes the wrong thing.
#[test]
fn a_commit_message_round_trips_with_the_diff_in_the_prompt() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/v1/chat/completions")
        .match_body(Matcher::AllOf(vec![
            Matcher::Regex("fix-upload".into()),
            Matcher::Regex("retry_count".into()),
        ]))
        .with_status(200)
        .with_body(
            r#"{"choices":[{"message":{"content":"Add a retry to the uploader\n\nSlow links flake."}}]}"#,
        )
        .create();

    let config = LocalCompletionConfig::for_test(
        &format!("{}/v1/chat/completions", server.url()),
        CustomEndpointSchema::OpenaiChatCompletions,
        "qwen",
    );

    let response = block_on(features::generate_code_review_content(
        &http_client::Client::new_for_test(),
        &config,
        GenerateCodeReviewContentRequest {
            output_type: OutputType::CommitMessage,
            diff: "+ let retry_count = 3;".into(),
            branch_name: "fix-upload".into(),
            commit_messages: vec!["wip".into()],
        },
    ))
    .unwrap();

    mock.assert();
    assert_eq!(
        response.content,
        "Add a retry to the uploader\n\nSlow links flake."
    );
}

/// The realistic case for a small local model: correct content, wrapped in a
/// fence it was told not to use. The feature has to survive that end to end.
#[test]
fn next_command_survives_a_fenced_json_reply() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/v1/chat/completions")
        .with_status(200)
        .with_body(
            r#"{"choices":[{"message":{"content":"```json\n{\"commands\": [\"git commit -m 'wip'\", \"npm test\"]}\n```"}}]}"#,
        )
        .create();

    let config = LocalCompletionConfig::for_test(
        &format!("{}/v1/chat/completions", server.url()),
        CustomEndpointSchema::OpenaiChatCompletions,
        "qwen",
    );

    let response = block_on(features::generate_ai_input_suggestions(
        &http_client::Client::new_for_test(),
        &config,
        &GenerateAIInputSuggestionsRequest {
            prefix: Some("git ".into()),
            ..Default::default()
        },
    ))
    .unwrap();

    mock.assert();
    // `npm test` is dropped because it does not extend the typed prefix.
    assert_eq!(response.commands, vec!["git commit -m 'wip'"]);
    assert_eq!(response.most_likely_action, "git commit -m 'wip'");
}

#[test]
fn a_prompt_suggestion_round_trips_with_the_exit_code_in_the_prompt() {
    let mut server = Server::new();
    let mock = server
        .mock("POST", "/v1/chat/completions")
        .match_body(Matcher::Regex("exited with code 127".into()))
        .with_status(200)
        .with_body(
            r#"{"choices":[{"message":{"content":"{\"query\": \"Why is cargo not on PATH?\", \"should_plan_task\": false}"}}]}"#,
        )
        .create();

    let config = LocalCompletionConfig::for_test(
        &format!("{}/v1/chat/completions", server.url()),
        CustomEndpointSchema::OpenaiChatCompletions,
        "qwen",
    );

    let response = block_on(features::generate_am_query_suggestions(
        &http_client::Client::new_for_test(),
        &config,
        &GenerateAMQuerySuggestionsRequest {
            context_messages: vec!["$ cargo build\ncargo: command not found".into()],
            system_context: None,
            exit_code: 127,
        },
    ))
    .unwrap();

    mock.assert();
    match response.suggestion {
        Some(Suggestion::Simple(query)) => {
            assert_eq!(query.query, "Why is cargo not on PATH?");
            assert!(!query.should_plan_task);
        }
        other => panic!("expected a simple query, got {other:?}"),
    }
}

/// A model that answers in prose instead of JSON is a real failure mode for
/// small local models. It has to be an error naming what came back, not a
/// silent empty suggestion.
#[test]
fn a_non_json_reply_to_a_json_feature_is_a_named_error() {
    let mut server = Server::new();
    let _mock = server
        .mock("POST", "/v1/chat/completions")
        .with_status(200)
        .with_body(r#"{"choices":[{"message":{"content":"I am not sure what to suggest."}}]}"#)
        .create();

    let config = LocalCompletionConfig::for_test(
        &format!("{}/v1/chat/completions", server.url()),
        CustomEndpointSchema::OpenaiChatCompletions,
        "qwen",
    );

    let error = format!(
        "{:#}",
        block_on(features::generate_ai_input_suggestions(
            &http_client::Client::new_for_test(),
            &config,
            &GenerateAIInputSuggestionsRequest::default(),
        ))
        .unwrap_err()
    );

    assert!(error.contains("I am not sure what to suggest."), "{error}");
}
