//! What is left here is the `reqwest` half. The host-matching rules and the
//! two switches moved to `egress_policy` with the lists they act on, and their
//! tests moved with them -- a test asserting `sentry.io` is listed belongs
//! beside the list, not beside the code that rewrites a URL.

use super::*;

#[test]
fn blackhole_url_is_parseable() {
    let url = blackhole_url();
    assert_eq!(
        url.port(),
        Some(0),
        "port 0 is what makes this unconnectable"
    );
}

#[test]
fn urls_without_a_host_are_not_blocked() {
    let url = reqwest::Url::parse("file:///tmp/x").unwrap();
    assert!(!is_blocked(&url));
}

/// The `reqwest` layer really does consult the shared policy.
///
/// Cheap, and it is the seam that the move of 2026-09-09 could have broken
/// silently: `is_blocked` returning `false` for everything would leave every
/// test in `egress_policy` passing and this crate's backstop switched off.
#[test]
fn a_listed_host_is_blocked_through_the_url_layer() {
    let blocked = reqwest::Url::parse("https://api.segment.io/v1/track").unwrap();
    let allowed = reqwest::Url::parse("https://api.anthropic.com/v1/messages").unwrap();

    assert!(is_blocked(&blocked));
    assert!(!is_blocked(&allowed));
}

/// Every `reqwest` client built outside this crate that is not handed to
/// `Client::from_client_builder`, per file, and why each is allowed.
///
/// These reach the network through none of the three enforcement points, so the
/// deny-lists never see them. Whether each should is `HANDOFF-SECURITY.md` task
/// 6; this list only makes a new one stop at the gate instead of shipping.
const UNCHECKED_CLIENTS: &[(&str, usize, &str)] = &[
    (
        "app/src/ai/agent_sdk/test_support.rs",
        1,
        "test support; handed to `from_client_builder` through a variable, \
         which the scan cannot follow",
    ),
    (
        "app/src/integration_testing/agent_mode/llm_judge/mod.rs",
        1,
        "integration-test LLM judge, built from `use reqwest::blocking::Client`",
    ),
    ("app/src/tracing/cloud_agent_auth.rs", 1, "tracing auth"),
    (
        "app/src/tracing/local_export.rs",
        1,
        "OTLP export, gated to loopback",
    ),
    (
        "crates/local_control/src/client.rs",
        2,
        "warpctrl's loopback client, `reqwest::blocking`",
    ),
    ("crates/mcp/src/runtime.rs", 1, "MCP HTTP transport"),
    (
        "crates/mcp/src/sse_transport/reqwest_impl.rs",
        1,
        "MCP SSE transport",
    ),
];

const CLIENT_CONSTRUCTORS: &[&str] = &[
    "Client::new(",
    "Client::builder(",
    "Client::default(",
    "ClientBuilder::new(",
    "ClientBuilder::default(",
];

/// Counts constructions in a file that mentions `reqwest`. A constructor
/// qualified by some other path (`http_client::Client::new`) or glued to a
/// longer name (`HttpClient::new`) is not reqwest's; a bare one is counted,
/// because `use reqwest::blocking::Client` is how `llm_judge` builds its client.
/// One inside an unfinished `from_client_builder(` statement is wrapped.
fn unchecked_constructions(text: &str) -> usize {
    let mut count = 0;
    for constructor in CLIENT_CONSTRUCTORS {
        for (at, _) in text.match_indices(constructor) {
            let before = &text[..at];
            if before.ends_with("::") {
                if !(before.ends_with("reqwest::") || before.ends_with("reqwest::blocking::")) {
                    continue;
                }
            } else if before.ends_with(|c: char| c.is_alphanumeric() || c == '_') {
                continue;
            }
            if let Some(wrap) = before.rfind("from_client_builder(") {
                if !before[wrap..].contains(';') {
                    continue;
                }
            }
            count += 1;
        }
    }
    count
}

/// No `reqwest` client is built outside this crate except the listed ones.
///
/// The `reqwest` counterpart of `a_websocket_is_dialled_from_this_crate_alone`.
/// A merge that adds an analytics client the way upstream's MCP code builds
/// one would compile, pass every egress test, and send.
///
/// **What this does not cover:** test files (`*_tests.rs`), which quote the
/// constructors in assertions; a client built through a helper in another
/// crate; and `hyper` or any other HTTP stack. It pins the doors that exist.
#[test]
fn every_reqwest_client_outside_this_crate_is_listed() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let mut roots = vec![root.join("app").join("src")];
    for entry in std::fs::read_dir(root.join("crates"))
        .expect("crates/")
        .flatten()
    {
        if entry.file_name() != "http_client" {
            roots.push(entry.path().join("src"));
        }
    }

    let mut found = Vec::new();
    let mut stack = roots;
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !name.ends_with(".rs") || name.ends_with("_tests.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            if !text.contains("reqwest") {
                continue;
            }
            let count = unchecked_constructions(&text);
            if count > 0 {
                let relative = path
                    .strip_prefix(&root)
                    .expect("under the workspace root")
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                found.push((relative, count));
            }
        }
    }
    found.sort();

    let expected: Vec<_> = UNCHECKED_CLIENTS
        .iter()
        .map(|(path, count, _)| ((*path).to_owned(), *count))
        .collect();
    assert_eq!(
        found, expected,
        "the set of `reqwest` clients that bypass the egress checks changed. \
         Hand a new one to `http_client::Client::from_client_builder`, or add \
         it to UNCHECKED_CLIENTS with the reason it may bypass them"
    );
}

/// The blackhole takes the payload, not just the destination.
///
/// **Calibrated by what must disappear, not by what must arrive.** Asserting
/// only that the URL changed cannot fail if the headers and body are still
/// attached, and that was exactly the state this fixed: the request was fully
/// assembled and the sole thing preventing transmission was that `0.0.0.0:0`
/// cannot be connected to.
#[test]
fn a_blackholed_request_keeps_no_headers_and_no_body() {
    let client = reqwest::Client::new();
    let mut request = client
        .post("https://api.segment.io/v1/track")
        .header("authorization", "Bearer a-real-token")
        .body("{\"userId\":\"someone\"}")
        .build()
        .expect("request builds");

    assert!(
        request.headers().contains_key("authorization"),
        "the test is worthless unless the header was there to begin with"
    );
    assert!(request.body().is_some(), "and the body too");

    blackhole(&mut request);

    assert_eq!(*request.url(), blackhole_url());
    assert!(
        request.headers().is_empty(),
        "a blocked request must carry no headers: {:?}",
        request.headers()
    );
    assert!(
        request.body().is_none(),
        "and no body -- the payload is the thing that must not leave"
    );
}
