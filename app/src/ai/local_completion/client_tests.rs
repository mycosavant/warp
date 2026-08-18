use serde_json::json;

use super::*;

#[test]
fn openai_chat_completions_text_is_read_from_the_first_choice() {
    let body = json!({
        "choices": [
            {"message": {"role": "assistant", "content": "  cargo test  "}},
            {"message": {"role": "assistant", "content": "ignored"}},
        ]
    });
    assert_eq!(
        extract_text(CustomEndpointSchema::OpenaiChatCompletions, &body).as_deref(),
        Some("cargo test")
    );
}

/// Anthropic returns content as a list of blocks and may split one reply across
/// several, so reading only the first would silently truncate.
#[test]
fn anthropic_text_blocks_are_concatenated() {
    let body = json!({
        "content": [
            {"type": "text", "text": "cargo "},
            {"type": "text", "text": "test"},
        ]
    });
    assert_eq!(
        extract_text(CustomEndpointSchema::AnthropicMessages, &body).as_deref(),
        Some("cargo test")
    );
}

/// A thinking block carries no `text`, and must not become part of the answer
/// or abort the walk.
#[test]
fn anthropic_non_text_blocks_are_skipped() {
    let body = json!({
        "content": [
            {"type": "thinking", "thinking": "hmm"},
            {"type": "text", "text": "cargo test"},
        ]
    });
    assert_eq!(
        extract_text(CustomEndpointSchema::AnthropicMessages, &body).as_deref(),
        Some("cargo test")
    );
}

#[test]
fn the_responses_api_shorthand_field_is_preferred() {
    let body = json!({"output_text": "cargo test", "output": []});
    assert_eq!(
        extract_text(CustomEndpointSchema::OpenaiResponses, &body).as_deref(),
        Some("cargo test")
    );
}

/// The Responses API interleaves reasoning items with message items in one
/// array, so indexing `output[0]` would read the wrong thing.
#[test]
fn the_responses_api_output_array_is_walked_not_indexed() {
    let body = json!({
        "output": [
            {"type": "reasoning", "summary": []},
            {"type": "message", "content": [{"type": "output_text", "text": "cargo test"}]},
        ]
    });
    assert_eq!(
        extract_text(CustomEndpointSchema::OpenaiResponses, &body).as_deref(),
        Some("cargo test")
    );
}

/// An empty completion is a failure, not an answer — a blank commit message or
/// a blank block title is worse than an error that says what went wrong.
#[test]
fn a_blank_completion_is_not_text() {
    let body = json!({"choices": [{"message": {"content": "   \n  "}}]});
    assert!(extract_text(CustomEndpointSchema::OpenaiChatCompletions, &body).is_none());
}

#[test]
fn a_response_missing_the_expected_shape_is_not_text() {
    let body = json!({"error": {"message": "model not found"}});
    for schema in [
        CustomEndpointSchema::OpenaiChatCompletions,
        CustomEndpointSchema::OpenaiResponses,
        CustomEndpointSchema::AnthropicMessages,
    ] {
        assert!(extract_text(schema, &body).is_none(), "{schema:?}");
    }
}

#[test]
fn a_bare_json_object_is_returned_as_is() {
    assert_eq!(
        extract_json_object(r#"{"commands": ["ls"]}"#),
        Some(r#"{"commands": ["ls"]}"#)
    );
}

/// Models fence JSON even when told not to, and small local models add a
/// sentence of preamble. Both are recoverable, and failing on them would make
/// the feature unusable against exactly the models it is meant to serve.
#[test]
fn a_fenced_or_prefaced_json_object_is_recovered() {
    for raw in [
        "```json\n{\"query\": \"fix it\"}\n```",
        "Sure! Here is the JSON:\n{\"query\": \"fix it\"}",
        "```\n{\"query\": \"fix it\"}\n```\nHope that helps.",
    ] {
        assert_eq!(
            extract_json_object(raw),
            Some(r#"{"query": "fix it"}"#),
            "{raw}"
        );
    }
}

/// A shell command in a suggestion routinely contains braces. Counting them
/// without tracking string state would truncate the object mid-value.
#[test]
fn braces_inside_strings_do_not_end_the_object() {
    let raw = r#"{"commands": ["awk '{print $1}' log", "echo }"]}"#;
    assert_eq!(extract_json_object(raw), Some(raw));
}

#[test]
fn an_escaped_quote_does_not_end_a_string() {
    let raw = r#"{"query": "say \"}\" out loud"}"#;
    assert_eq!(extract_json_object(raw), Some(raw));
}

#[test]
fn a_nested_object_is_kept_whole() {
    let raw = r#"{"suggestion": {"query": "fix", "meta": {"n": 1}}}"#;
    assert_eq!(extract_json_object(raw), Some(raw));
}

#[test]
fn prose_with_no_object_is_not_json() {
    assert_eq!(
        extract_json_object("I could not determine a command."),
        None
    );
}

/// An unterminated object means the reply was cut off — usually `max_tokens`.
/// Returning the truncated prefix would hand the parser a guess.
#[test]
fn an_unterminated_object_is_not_json() {
    assert_eq!(extract_json_object(r#"{"commands": ["ls""#), None);
}

#[test]
fn a_fenced_reply_is_unwrapped() {
    assert_eq!(
        strip_code_fence("```\nAdd a retry to the uploader\n```"),
        "Add a retry to the uploader"
    );
    assert_eq!(
        strip_code_fence("```markdown\nAdd a retry\n\nBecause it flakes.\n```"),
        "Add a retry\n\nBecause it flakes."
    );
}

#[test]
fn an_unfenced_reply_is_left_alone() {
    assert_eq!(
        strip_code_fence("  Add a retry to the uploader  "),
        "Add a retry to the uploader"
    );
}

/// A commit message legitimately contains a fenced block. Stripping the whole
/// reply because it *starts* with one would be wrong, but here the fence opens
/// mid-message, so nothing is stripped.
#[test]
fn an_inner_fence_is_preserved() {
    let message = "Add a retry\n\n```\ncargo test\n```";
    assert_eq!(strip_code_fence(message), message);
}

#[test]
fn a_long_error_body_is_truncated_to_its_tail() {
    let detail = "x".repeat(MAX_ERROR_DETAIL_BYTES * 2);
    let truncated = truncate_detail(&detail);
    assert!(truncated.starts_with("..."));
    assert_eq!(truncated.len(), MAX_ERROR_DETAIL_BYTES + 3);
}

/// Truncation is byte-based but the input is arbitrary UTF-8, so the cut has to
/// land on a character boundary or this panics on a non-ASCII error page.
#[test]
fn truncation_does_not_split_a_multibyte_character() {
    let detail = "é".repeat(MAX_ERROR_DETAIL_BYTES);
    let truncated = truncate_detail(&detail);
    assert!(truncated.starts_with("..."));
    assert!(truncated.chars().all(|c| c == '.' || c == 'é'));
}

#[test]
fn a_short_error_body_is_left_intact() {
    assert_eq!(truncate_detail("  model not found  "), "model not found");
}
