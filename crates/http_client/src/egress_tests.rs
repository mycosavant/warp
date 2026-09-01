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
fn blocks_exact_hosts() {
    assert!(is_blocked_host("sentry.io"));
    assert!(is_blocked_host("segment.io"));
    assert!(is_blocked_host("rudderstack.com"));
}

#[test]
fn blocks_subdomains() {
    // The case that motivates suffix matching: per-tenant Sentry ingest hosts.
    assert!(is_blocked_host("o12345.ingest.sentry.io"));
    assert!(is_blocked_host("api.segment.io"));
    assert!(is_blocked_host("a.b.c.datadoghq.com"));
}

#[test]
fn is_case_and_trailing_dot_insensitive() {
    assert!(is_blocked_host("O12345.Ingest.Sentry.IO"));
    assert!(is_blocked_host("sentry.io."));
}

#[test]
fn does_not_block_lookalike_suffixes() {
    // Must not match on bare substring: these are different registrable
    // domains that merely end in the same characters.
    assert!(!is_blocked_host("notsentry.io"));
    assert!(!is_blocked_host("mysegment.com"));
    assert!(!is_blocked_host("evilsentry.io"));
}

#[test]
fn does_not_block_warp_or_provider_hosts() {
    // Warp's own API must keep working; so must the AI providers the fork
    // exists to talk to.
    assert!(!is_blocked_host("api.warp.dev"));
    assert!(!is_blocked_host("app.warp.dev"));
    assert!(!is_blocked_host("api.anthropic.com"));
    assert!(!is_blocked_host("api.openai.com"));
    assert!(!is_blocked_host("github.com"));
}

#[test]
fn urls_without_a_host_are_not_blocked() {
    let url = reqwest::Url::parse("file:///tmp/x").unwrap();
    assert!(!is_blocked(&url));
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
