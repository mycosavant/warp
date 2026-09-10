//! What a person is told when the agent will not start without a credential.
//!
//! The fixtures are the bytes two agents actually sent, copied out of
//! `.fork/runs/auth-2026-09-10/` rather than typed from memory. Deserialising
//! them is part of the test: an invented `AuthMethod` would keep passing if the
//! wire shape moved underneath it.
//!
//! # One of these was decoration and the calibration is what found it
//!
//! Every test here was run against a deliberately broken module before being
//! kept — `.fork/runs/auth-2026-09-10/calibration.txt`, three breaks. The
//! second break replaced [`super::sign_in_somehow`] with a call to
//! [`super::sign_in_with_one_of`] carrying an empty slice, which is exactly the
//! collapse a "the two arms are distinguishable" test exists to catch.
//!
//! There *was* such a test, asserting the two arms produce different strings,
//! and **it passed the break.** It could not do otherwise: the two calls differ
//! in their method list, so their output differs whether or not the second arm
//! is real. It has been removed rather than repaired, because the property it
//! was reaching for is already held by
//! [`an_agent_that_names_no_way_to_sign_in_says_that_instead_of_listing_nothing`],
//! which reddened on that break — the heading with nothing under it is the
//! thing that actually separates the arms, and one test asserting it is one
//! more than two tests where only one works.

use super::*;

/// `@zed-industries/codex-acp@0.16.0`, verbatim from `codex.jsonl`.
const CODEX_METHODS: &str = r#"[
  {"id":"chatgpt","name":"Login with ChatGPT","description":"Use your ChatGPT login with Codex CLI (requires a paid ChatGPT subscription)"},
  {"id":"codex-api-key","name":"Use CODEX_API_KEY","description":"Requires setting the `CODEX_API_KEY` environment variable."},
  {"id":"openai-api-key","name":"Use OPENAI_API_KEY","description":"Requires setting the `OPENAI_API_KEY` environment variable."}
]"#;

/// `@google/gemini-cli@0.58.0 --acp`, verbatim from `gemini.jsonl`. Kept
/// because one of its entries carries `_meta` and one carries no `description`
/// in other versions — the renderer must survive both.
const GEMINI_METHODS: &str = r#"[
  {"id":"oauth-personal","name":"Log in with Google","description":"Log in with your Google account"},
  {"id":"gemini-api-key","name":"Gemini API key","description":"Use an API key with Gemini Developer API","_meta":{"api-key":{"provider":"google"}}},
  {"id":"vertex-ai","name":"Vertex AI","description":"Use an API key with Vertex AI GenAI API"},
  {"id":"gateway","name":"AI API Gateway","description":"Use a custom AI API Gateway","_meta":{"gateway":{"protocol":"google","restartRequired":"false"}}}
]"#;

fn methods(json: &str) -> Vec<AuthMethod> {
    serde_json::from_str(json).expect("these are the bytes an agent sent")
}

/// The whole point: the agent's refusal is explained in the agent's own words,
/// with the agent's own list, and Warp picks nothing out of it.
#[test]
fn a_credential_refusal_quotes_the_agent_and_lists_what_it_named() {
    let message = refused_for_a_credential(
        "@zed-industries/codex-acp",
        ErrorCode::AuthRequired,
        "Authentication required",
        &methods(CODEX_METHODS),
    )
    .expect("-32000 is the agent saying it wants a credential");

    assert!(
        message.contains("@zed-industries/codex-acp"),
        "got: {message}"
    );
    assert!(
        message.contains("Authentication required"),
        "the agent's own words are more useful than anything Warp would write, got: {message}"
    );
    for id in ["chatgpt", "codex-api-key", "openai-api-key"] {
        assert!(
            message.contains(id),
            "{id} came off the wire and must reach the person, got: {message}"
        );
    }
    assert!(
        message.contains("Requires setting the `CODEX_API_KEY` environment variable."),
        "the description is where the actionable half lives, got: {message}"
    );
    assert!(
        message.contains("WARP_FORK_ACP_AUTH"),
        "the person is owed the name of the thing that does not exist yet, got: {message}"
    );
}

/// …and Gemini's own prose survives the same path, which is the case that
/// separates this from a table of known agents.
///
/// Its message is nothing like Codex's and its ids are nothing like Codex's, so
/// a rule that recognised either would fail here.
#[test]
fn an_agent_that_words_it_its_own_way_is_quoted_not_normalised() {
    let message = refused_for_a_credential(
        "@google/gemini-cli",
        ErrorCode::AuthRequired,
        "Gemini API key is missing or not configured.",
        &methods(GEMINI_METHODS),
    )
    .expect("-32000 is the agent saying it wants a credential");

    assert!(
        message.contains("Gemini API key is missing or not configured."),
        "got: {message}"
    );
    for id in ["oauth-personal", "gemini-api-key", "vertex-ai", "gateway"] {
        assert!(message.contains(id), "got: {message}");
    }
}

/// **The arm that earns the design.** A refusal that is not about a credential
/// gets no credential advice — *even from an agent advertising every way in the
/// world to sign in*.
///
/// This is the case the obvious rule gets wrong. `authMethods` is answered at
/// `initialize`, before the agent has seen the request, so Codex names its three
/// methods whether or not a key is set. The moment one is set on this machine,
/// the next `session/new` failure is the WSL cwd one `CLAUDE.md` has recorded
/// since 2026-09-02 — and "advertised methods and refused" would answer it with
/// *"sign in"*, sending the person to a login flow that is already complete.
#[test]
fn a_refusal_that_is_not_about_a_credential_gets_no_credential_advice() {
    assert!(
        refused_for_a_credential(
            "@zed-industries/codex-acp",
            ErrorCode::InvalidParams,
            "Invalid params: `cwd` does not exist on the machine running the agent: \
             /home/effatha/git/warp",
            &methods(CODEX_METHODS),
        )
        .is_none(),
        "a signed-in agent with a bad directory must not be told to sign in"
    );
}

/// The control from the same run, kept because it is the one measured
/// non-credential refusal: `claude-agent-acp` 0.73.0, signed in, `-32602`, and
/// an `authMethods` of `[]`.
#[test]
fn the_measured_control_is_left_to_the_explainer_that_owns_it() {
    assert!(
        refused_for_a_credential(
            "@agentclientprotocol/claude-agent-acp",
            ErrorCode::InvalidParams,
            "Invalid params: `cwd` does not exist on the machine running the agent: \
             /home/effatha/definitely-not-a-real-directory-xyz",
            &[],
        )
        .is_none(),
        "this failure has a better sentence already, and it names WSL"
    );
}

/// An agent that wants a credential and names no way to supply one says so,
/// rather than printing a heading over nothing.
#[test]
fn an_agent_that_names_no_way_to_sign_in_says_that_instead_of_listing_nothing() {
    let message = refused_for_a_credential("someagent", ErrorCode::AuthRequired, "no soup", &[])
        .expect("-32000 with no list is still -32000");

    assert!(message.contains("no soup"), "got: {message}");
    assert!(
        message.contains("named no way to sign in"),
        "the absence is the fact, got: {message}"
    );
    assert!(
        !message.contains("Ways it named"),
        "a heading with nothing under it reads as a bug, got: {message}"
    );
}

/// The refusal is the only thing a person sees, so it must not name a mechanism
/// they cannot act on.
///
/// The variables the person actually sets are deducted before the scan, for the
/// reason `the_continuation_refusal_explains_itself_without_protocol_jargon`
/// gives: `WARP_FORK_ACP_AUTH` contains the letters `ACP`, and a knob someone
/// turns themselves is not jargon.
#[test]
fn the_credential_refusal_explains_itself_without_protocol_jargon() {
    let message = refused_for_a_credential(
        "@google/gemini-cli",
        ErrorCode::AuthRequired,
        "Gemini API key is missing or not configured.",
        &methods(GEMINI_METHODS),
    )
    .expect("listed");
    let prose = message
        .replace("WARP_FORK_ACP_AUTH", "")
        .replace("WARP_FORK_ACP_COMMAND", "");

    for jargon in [
        "session/new",
        "authMethods",
        "authenticate",
        "JSON-RPC",
        "-32000",
        "ACP",
    ] {
        assert!(
            !prose.contains(jargon),
            "{jargon} means nothing to a person reading a conversation, got: {message}"
        );
    }
}

/// A method with no description renders without a dangling separator.
///
/// The protocol makes `description` optional and both measured agents happen to
/// send one for every method, so nothing on this machine exercises the other
/// branch. That is exactly why it is pinned here.
#[test]
fn a_method_with_no_description_is_rendered_without_a_trailing_colon() {
    let bare = methods(r#"[{"id":"only-id","name":"Only a name"}]"#);
    let rendered = offered_list(&bare);

    assert_eq!(rendered, "  only-id — Only a name");
}
