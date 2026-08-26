use axum::body::to_bytes;

use super::*;

async fn body_of(response: Response) -> String {
    let bytes = to_bytes(response.into_body(), 1 << 20)
        .await
        .expect("console bodies are small and finite");
    String::from_utf8(bytes.to_vec()).expect("both documents are UTF-8 source files")
}

fn header(response: &Response, name: HeaderName) -> String {
    response
        .headers()
        .get(&name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned()
}

/// Every line of `document` that mentions `needle` outside a `//` comment.
///
/// Scanning the whole file would fail on its own prose — these two documents
/// explain in comments exactly which constructs they avoid, and a test that
/// cannot tell an explanation from a use would force the explanation out. A
/// comment cannot execute, so excluding whole comment lines is sound; nothing
/// finer is attempted, because a half-correct JavaScript parser here would be a
/// worse liability than the property it checks.
fn executable_lines_mentioning<'a>(document: &'a str, needle: &str) -> Vec<&'a str> {
    document
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .filter(|line| line.contains(needle))
        .collect()
}

/// The claim the module docs rest on, checked rather than asserted in prose: the
/// page is safe to serve unauthenticated because it *is* a constant, and a
/// constant cannot contain a secret unless someone typed one into it.
///
/// Deliberately a change-detector. Any future edit that pastes a token in while
/// debugging has to come through here.
#[test]
fn the_console_is_a_constant_and_names_no_secret() {
    for document in [CONSOLE_HTML, CONSOLE_SCRIPT] {
        for forbidden in ["device_token\":", "bearer_token\":"] {
            assert!(
                executable_lines_mentioning(document, forbidden).is_empty(),
                "a served document must not carry a literal {forbidden:?}"
            );
        }
    }
    assert!(
        executable_lines_mentioning(CONSOLE_HTML, "Bearer").is_empty(),
        "the page itself never authenticates anything"
    );
    // The script composes the header at runtime from a value it fetched. One
    // site, and it joins a constant prefix to a variable — a second occurrence
    // would mean a token had been written down somewhere.
    assert_eq!(
        executable_lines_mentioning(CONSOLE_SCRIPT, "Bearer"),
        vec!["    return { authorization: 'Bearer ' + token };"]
    );
}

/// `script-src 'self'` is worth nothing if the page also ships an inline
/// `<script>`, because the policy would have to be relaxed to `'unsafe-inline'`
/// to make it run — at which point the whole reason for the second route is
/// gone. Pinned so the two cannot drift apart.
#[test]
fn the_page_runs_no_script_of_its_own() {
    assert!(POLICY.contains("script-src 'self'"));
    assert!(!POLICY.contains("script-src 'unsafe-inline'"));

    let mut scripts = CONSOLE_HTML.match_indices("<script");
    let (at, _) = scripts.next().expect("the page loads exactly one script");
    assert!(scripts.next().is_none(), "one script tag, and it has a src");
    let tag = &CONSOLE_HTML[at..];
    let tag = &tag[..tag.find('>').expect("a well-formed tag")];
    assert!(
        tag.contains(&format!("src=\"{CONSOLE_SCRIPT_PATH}\"")),
        "the only script tag must load {CONSOLE_SCRIPT_PATH}, not inline code"
    );
}

/// The rule that makes rendering agent-authored text safe. `textContent` is the
/// only escaping in this file that cannot be got wrong by accident, so the
/// absence of its alternatives is the property worth pinning — a reviewer
/// reading a diff will not reliably notice one `innerHTML` among four hundred
/// lines.
#[test]
fn the_script_never_assigns_markup() {
    for forbidden in [
        "innerHTML",
        "outerHTML",
        "insertAdjacentHTML",
        "document.write",
        "eval(",
    ] {
        assert!(
            executable_lines_mentioning(CONSOLE_SCRIPT, forbidden).is_empty(),
            "the console renders untrusted text and must not use {forbidden}"
        );
    }
}

/// A control plane inside a frame is a control plane a page you did not write
/// can click on. Both headers, because `frame-ancestors` is the one that is
/// actually consulted and `X-Frame-Options` is the one an older browser has.
#[tokio::test]
async fn the_console_refuses_to_be_framed() {
    let response = handle_console_request().await;
    assert!(header(&response, CONTENT_SECURITY_POLICY).contains("frame-ancestors 'none'"));
    assert_eq!(header(&response, X_FRAME_OPTIONS), "DENY");
}

/// What each route is, and that it says so. `nosniff` matters more than usual
/// here: a browser that decided the script was HTML would be parsing a document
/// served from the control plane's own origin.
#[tokio::test]
async fn each_document_is_served_as_what_it_is() {
    let page = handle_console_request().await;
    assert_eq!(page.status(), StatusCode::OK);
    assert_eq!(header(&page, CONTENT_TYPE), "text/html; charset=utf-8");
    assert_eq!(header(&page, X_CONTENT_TYPE_OPTIONS), "nosniff");
    assert_eq!(header(&page, CACHE_CONTROL), "no-store");
    assert_eq!(header(&page, REFERRER_POLICY), "no-referrer");
    assert!(body_of(page).await.starts_with("<!doctype html>"));

    let script = handle_console_script_request().await;
    assert_eq!(script.status(), StatusCode::OK);
    assert_eq!(
        header(&script, CONTENT_TYPE),
        "text/javascript; charset=utf-8"
    );
    assert_eq!(header(&script, X_CONTENT_TYPE_OPTIONS), "nosniff");
    assert!(body_of(script).await.contains("'use strict'"));
}

/// `default-src 'none'` has to come first and `connect-src` has to be `'self'`,
/// or a page that ran hostile code could reach off the machine with what it
/// read — which, on this origin, is a live agent transcript.
#[tokio::test]
async fn the_policy_denies_by_default_and_talks_only_to_itself() {
    let policy = header(&handle_console_request().await, CONTENT_SECURITY_POLICY);
    assert!(policy.starts_with("default-src 'none'"));
    assert!(policy.contains("connect-src 'self'"));
    assert!(policy.contains("base-uri 'none'"));
    assert!(policy.contains("form-action 'none'"));
    assert!(
        !policy.contains('*'),
        "no wildcard belongs in this policy: {policy}"
    );
}
