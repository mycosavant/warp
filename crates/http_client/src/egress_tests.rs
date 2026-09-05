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

/// The providers the fork exists to talk to are untouched.
///
/// **This test used to assert `api.warp.dev` and `app.warp.dev` were allowed,
/// under the heading "Warp's own API must keep working".** That was the policy
/// until 2026-09-04 and it is now the opposite -- see
/// `warps_own_services_are_blocked`. Renamed rather than edited in place,
/// because a test called `does_not_block_warp_or_provider_hosts` that no longer
/// says anything about Warp is a stale doc waiting to be read as a guarantee.
///
/// What survives is the half that always mattered more: the fork routes agents
/// to the user's own Anthropic/OpenAI keys and local models, so a deny-list
/// that caught those would break the thing this fork is for. `github.com`
/// stands in for ordinary product traffic.
#[test]
fn does_not_block_provider_hosts() {
    assert!(!is_blocked_host("api.anthropic.com"));
    assert!(!is_blocked_host("api.openai.com"));
    assert!(!is_blocked_host("github.com"));
    assert!(!is_blocked_host("localhost"));
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

/// Warp's own hosts are on the list, including the subdomains that carry the
/// interesting traffic.
///
/// `app.warp.dev` is the GraphQL endpoint, the `/ai/*` routes and
/// `/client_version`; `rtc.` and `sessions.` are the realtime and
/// session-sharing sockets. All three are suffixes of `warp.dev`, so one entry
/// covers them — asserted individually anyway, because "it is a suffix" is the
/// kind of reasoning that stops being true when somebody narrows the entry.
#[test]
fn warps_own_services_are_blocked() {
    assert!(is_blocked_host("warp.dev"));
    assert!(is_blocked_host("app.warp.dev"));
    assert!(is_blocked_host("rtc.app.warp.dev"));
    assert!(is_blocked_host("sessions.app.warp.dev"));
    assert!(is_blocked_host("oz.warp.dev"));
    assert!(is_blocked_host("staging.warp.dev"));

    // The suffix must not spill onto a lookalike, the same way `notsentry.io`
    // must not match `sentry.io`.
    assert!(!is_blocked_host("notwarp.dev"));
    assert!(!is_blocked_host("warp.dev.example.com"));
}

/// The two halves answer to different switches, and neither lifts the other.
///
/// This is the whole reason the first-party hosts are a separate list. The
/// telemetry switch exists so fork behaviour can be compared against upstream's
/// telemetry; it must not double as permission to send a prompt to Warp. And
/// `WARP_FORK_POLICY=0` -- which this repo documents as the way to A/B a
/// suspected fork regression -- cannot reach this module at all, so without a
/// switch of its own the first-party block would silently break that workflow.
#[test]
fn each_deny_list_has_its_own_switch() {
    let vendor = "sentry.io";
    let first_party = "app.warp.dev";

    // Both switches at their defaults: everything blocked.
    assert!(blocked_by(vendor, true, true));
    assert!(blocked_by(first_party, true, true));

    // Telemetry lifted. The first-party block must survive it.
    assert!(!blocked_by(vendor, false, true));
    assert!(
        blocked_by(first_party, false, true),
        "comparing telemetry against upstream is not consent to send a prompt \
         to Warp"
    );

    // First-party lifted, e.g. for a WARP_FORK_POLICY=0 A/B. Telemetry must
    // survive that.
    assert!(!blocked_by(first_party, true, false));
    assert!(
        blocked_by(vendor, true, false),
        "there is no reading of this fork under which a vendor endpoint \
         becomes acceptable"
    );
}
