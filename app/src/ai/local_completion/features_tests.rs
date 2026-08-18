use super::*;

fn request_with(prefix: Option<&str>, rejected: &[&str]) -> GenerateAIInputSuggestionsRequest {
    GenerateAIInputSuggestionsRequest {
        prefix: prefix.map(str::to_string),
        rejected_suggestions: rejected.iter().map(|s| (*s).to_string()).collect(),
        ..Default::default()
    }
}

fn reply(commands: &[&str]) -> NextCommandReply {
    NextCommandReply {
        commands: commands.iter().map(|s| (*s).to_string()).collect(),
    }
}

// ------------------------------------------------------------- next command

/// `next_command_model` checks `most_likely_action.starts_with(prefix)` and
/// discards the whole response when it does not hold. Filtering here means one
/// stray candidate costs a suggestion, not the entire prediction.
#[test]
fn suggestions_that_do_not_extend_the_prefix_are_dropped() {
    let response = finalize_next_commands(
        reply(&["git status", "cargo build", "git push"]),
        &request_with(Some("git "), &[]),
    );
    assert_eq!(response.commands, vec!["git status", "git push"]);
    assert_eq!(response.most_likely_action, "git status");
}

#[test]
fn rejected_suggestions_are_not_offered_again() {
    let response = finalize_next_commands(
        reply(&["cargo build", "cargo test"]),
        &request_with(None, &["cargo build"]),
    );
    assert_eq!(response.commands, vec!["cargo test"]);
    assert_eq!(response.most_likely_action, "cargo test");
}

/// Small models repeat themselves. Two identical rows in the suggestion list
/// reads as a bug.
#[test]
fn duplicate_suggestions_are_collapsed() {
    let response = finalize_next_commands(
        reply(&["cargo test", "cargo test", "cargo build"]),
        &request_with(None, &[]),
    );
    assert_eq!(response.commands, vec!["cargo test", "cargo build"]);
}

#[test]
fn blank_suggestions_are_dropped_and_whitespace_is_trimmed() {
    let response = finalize_next_commands(
        reply(&["  cargo test  ", "   ", ""]),
        &request_with(None, &[]),
    );
    assert_eq!(response.commands, vec!["cargo test"]);
}

/// Everything filtered out has to leave `most_likely_action` empty rather than
/// falling back to an unfiltered candidate — the consumer treats an empty
/// string as "no prediction", which is the honest answer here.
#[test]
fn filtering_everything_out_yields_no_prediction() {
    let response =
        finalize_next_commands(reply(&["cargo build"]), &request_with(Some("git "), &[]));
    assert!(response.commands.is_empty());
    assert!(response.most_likely_action.is_empty());
}

/// `AgentModeSuggestionV2` carries block IDs the request never sends, so these
/// stay empty by design rather than by omission.
#[test]
fn no_agent_queries_are_fabricated() {
    let response = finalize_next_commands(reply(&["ls"]), &request_with(None, &[]));
    assert!(response.ai_queries.is_empty());
}

#[test]
fn the_prompt_states_the_prefix_the_suggestions_must_extend() {
    let mut request = request_with(Some("git com"), &["git commit --amend"]);
    request.history_context = "git add -A".into();
    request.context_messages = vec!["$ git add -A".into()];
    request.system_context = Some("macOS, zsh".into());

    let prompt = next_command_prompt(&request);
    assert!(prompt.contains("git com"), "{prompt}");
    assert!(prompt.contains("git commit --amend"), "{prompt}");
    assert!(prompt.contains("git add -A"), "{prompt}");
    assert!(prompt.contains("macOS, zsh"), "{prompt}");
}

/// With an empty input the model must be told so explicitly; left unsaid, it
/// tends to invent a prefix to satisfy the instruction.
#[test]
fn the_prompt_says_so_when_nothing_has_been_typed() {
    let prompt = next_command_prompt(&request_with(None, &[]));
    assert!(prompt.contains("has not typed anything"), "{prompt}");

    let prompt = next_command_prompt(&request_with(Some("   "), &[]));
    assert!(prompt.contains("has not typed anything"), "{prompt}");
}

// --------------------------------------------------------------- block title

#[test]
fn a_quoted_or_multiline_title_is_reduced_to_one_clean_line() {
    assert_eq!(
        finalize_block_title("\"Install project dependencies\"").unwrap(),
        "Install project dependencies"
    );
    assert_eq!(
        finalize_block_title("Install project dependencies\n\nThis ran npm install.").unwrap(),
        "Install project dependencies"
    );
    assert_eq!(
        finalize_block_title("```\nBuild the release binary\n```").unwrap(),
        "Build the release binary"
    );
}

#[test]
fn an_empty_title_is_an_error() {
    assert!(finalize_block_title("   \n  ").is_err());
}

// --------------------------------------------------------------- code review

/// The PR title field is one line. A model that appends a description would
/// otherwise put the whole thing in the title.
#[test]
fn a_pr_title_is_reduced_to_one_line() {
    let content = finalize_code_review_content(
        "Add retry to the uploader\n\nBecause it flakes on slow links.",
        OutputType::PrTitle,
    )
    .unwrap();
    assert_eq!(content, "Add retry to the uploader");
}

/// A commit message body is meaningful; collapsing it the way a title is
/// collapsed would throw away the "why".
#[test]
fn a_commit_message_keeps_its_body() {
    let content = finalize_code_review_content(
        "Add retry to the uploader\n\nBecause it flakes on slow links.",
        OutputType::CommitMessage,
    )
    .unwrap();
    assert_eq!(
        content,
        "Add retry to the uploader\n\nBecause it flakes on slow links."
    );
}

#[test]
fn a_fenced_description_is_unwrapped() {
    let content = finalize_code_review_content(
        "```markdown\nAdds a retry.\n\n- bounded backoff\n```",
        OutputType::PrDescription,
    )
    .unwrap();
    assert_eq!(content, "Adds a retry.\n\n- bounded backoff");
}

/// `git_actions` falls back to `gh pr create --fill` on an error but would
/// commit an empty message on an empty success, so this has to be an error.
#[test]
fn empty_code_review_content_is_an_error() {
    for output_type in [
        OutputType::CommitMessage,
        OutputType::PrTitle,
        OutputType::PrDescription,
    ] {
        assert!(finalize_code_review_content("  \n ", output_type).is_err());
    }
}

// ---------------------------------------------------------- prompt suggestion

#[test]
fn a_suggestion_becomes_a_simple_query() {
    let response = finalize_prompt_suggestion(PromptSuggestionReply {
        query: "  Why did the build fail?  ".into(),
        should_plan_task: true,
    });
    match response.suggestion {
        Some(Suggestion::Simple(query)) => {
            assert_eq!(query.query, "Why did the build fail?");
            assert!(query.should_plan_task);
        }
        other => panic!("expected a simple query, got {other:?}"),
    }
}

/// The prompt tells the model to return an empty query when nothing is worth
/// asking. That has to become "no suggestion", not a blank one the UI renders.
#[test]
fn an_empty_query_produces_no_suggestion() {
    let response = finalize_prompt_suggestion(PromptSuggestionReply {
        query: "   ".into(),
        should_plan_task: false,
    });
    assert!(response.suggestion.is_none());
}

/// Upstream returns a server-side request ID used to identify the suggestion.
/// Two suggestions sharing one would collide wherever that identity is used.
#[test]
fn every_suggestion_gets_a_distinct_id() {
    let first = finalize_prompt_suggestion(PromptSuggestionReply {
        query: "a".into(),
        should_plan_task: false,
    });
    let second = finalize_prompt_suggestion(PromptSuggestionReply {
        query: "a".into(),
        should_plan_task: false,
    });
    assert!(!first.id.is_empty());
    assert_ne!(first.id, second.id);
}

/// `is_valid_code_delegation` discards a coding suggestion with no files, and
/// the client has no codebase index to fill them from.
#[test]
fn no_coding_suggestion_is_fabricated() {
    let response = finalize_prompt_suggestion(PromptSuggestionReply {
        query: "refactor the parser".into(),
        should_plan_task: true,
    });
    assert!(!response.is_valid_code_delegation());
}

// -------------------------------------------------------------------- helpers

/// Terminal output is unbounded and errors land at the end, so the tail is what
/// gets kept.
#[test]
fn truncation_keeps_the_tail_of_output_and_the_head_of_a_diff() {
    let long = "abcdefghij".repeat(10);
    assert_eq!(tail(&long, 5), "fghij");
    assert_eq!(head(&long, 5), "abcde");
    assert_eq!(tail("short", 100), "short");
    assert_eq!(head("short", 100), "short");
}

/// Command output is arbitrary UTF-8. Slicing by byte offset would panic on a
/// multibyte character straddling the cut.
#[test]
fn truncation_does_not_split_a_multibyte_character() {
    let value = "→←↑↓".repeat(10);
    assert_eq!(tail(&value, 2).chars().count(), 2);
    assert_eq!(head(&value, 2).chars().count(), 2);
}
