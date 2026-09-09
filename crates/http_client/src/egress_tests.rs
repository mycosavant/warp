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
