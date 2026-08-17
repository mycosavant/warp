use super::*;

#[test]
fn loopback_endpoints_are_recognized() {
    // The forms a local collector is actually reachable at.
    assert!(endpoint_is_loopback("http://localhost:4318"));
    assert!(endpoint_is_loopback("http://127.0.0.1:4318"));
    assert!(endpoint_is_loopback("http://[::1]:4318"));
    assert!(endpoint_is_loopback("https://localhost:4318"));
    assert!(endpoint_is_loopback("http://LOCALHOST:4318"));
}

#[test]
fn remote_endpoints_are_not_loopback() {
    // These must keep requiring a dispatch credential.
    assert!(!endpoint_is_loopback("https://otlp.example.com"));
    assert!(!endpoint_is_loopback("https://api.warp.dev"));
    // Deceptive hostnames that merely contain a loopback-looking substring.
    assert!(!endpoint_is_loopback("https://localhost.evil.com"));
    assert!(!endpoint_is_loopback("https://127.0.0.1.evil.com"));
}

#[test]
fn malformed_endpoints_are_not_loopback() {
    // Must defer to the authenticated path rather than silently dropping auth.
    assert!(!endpoint_is_loopback(""));
    assert!(!endpoint_is_loopback("not a url"));
    assert!(!endpoint_is_loopback("://missing-scheme"));
}

#[test]
fn traces_endpoint_appends_the_signal_path() {
    assert_eq!(
        traces_endpoint("http://localhost:4318").unwrap(),
        "http://localhost:4318/v1/traces"
    );
    // A trailing slash must not produce a doubled path segment.
    assert_eq!(
        traces_endpoint("http://localhost:4318/").unwrap(),
        "http://localhost:4318/v1/traces"
    );
}

#[test]
fn plain_http_is_rejected_for_non_loopback_hosts() {
    // The guard that stops the local-export affordance from leaking traces
    // unencrypted to a remote collector.
    assert!(traces_endpoint("http://otlp.example.com").is_err());
    assert!(traces_endpoint("https://otlp.example.com").is_ok());
}
