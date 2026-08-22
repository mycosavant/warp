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
