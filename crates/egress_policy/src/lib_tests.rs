use super::*;

#[test]
fn blocks_exact_hosts() {
    assert!(is_listed("sentry.io"));
    assert!(is_listed("segment.io"));
    assert!(is_listed("rudderstack.com"));
}

#[test]
fn blocks_subdomains() {
    // The case that motivates suffix matching: per-tenant Sentry ingest hosts.
    assert!(is_listed("o12345.ingest.sentry.io"));
    assert!(is_listed("api.segment.io"));
    assert!(is_listed("a.b.c.datadoghq.com"));
}

#[test]
fn is_case_and_trailing_dot_insensitive() {
    assert!(is_listed("O12345.Ingest.Sentry.IO"));
    assert!(is_listed("sentry.io."));
}

#[test]
fn does_not_block_lookalike_suffixes() {
    // Must not match on bare substring: these are different registrable
    // domains that merely end in the same characters.
    assert!(!is_listed("notsentry.io"));
    assert!(!is_listed("mysegment.com"));
    assert!(!is_listed("evilsentry.io"));
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
    assert!(!is_listed("api.anthropic.com"));
    assert!(!is_listed("api.openai.com"));
    assert!(!is_listed("github.com"));
    assert!(!is_listed("localhost"));
}

/// Warp's own hosts are on the list, including the subdomains that carry the
/// interesting traffic.
///
/// `app.warp.dev` is the GraphQL endpoint, the `/ai/*` routes and
/// `/client_version`; `rtc.` and `sessions.` are the realtime and
/// session-sharing sockets. All three are suffixes of `warp.dev`, so one entry
/// covers them -- asserted individually anyway, because "it is a suffix" is the
/// kind of reasoning that stops being true when somebody narrows the entry.
#[test]
fn warps_own_services_are_blocked() {
    assert!(is_listed("warp.dev"));
    assert!(is_listed("app.warp.dev"));
    assert!(is_listed("rtc.app.warp.dev"));
    assert!(is_listed("sessions.app.warp.dev"));
    assert!(is_listed("oz.warp.dev"));
    assert!(is_listed("staging.warp.dev"));

    // The suffix must not spill onto a lookalike, the same way `notsentry.io`
    // must not match `sentry.io`.
    assert!(!is_listed("notwarp.dev"));
    assert!(!is_listed("warp.dev.example.com"));
}

/// The two halves answer to different switches, and neither lifts the other.
///
/// This is the whole reason the first-party hosts are a separate list. The
/// telemetry switch exists so fork behaviour can be compared against upstream's
/// telemetry; it must not double as permission to send a prompt to Warp. And
/// `WARP_FORK_POLICY=0` -- which this repo documents as the way to A/B a
/// suspected fork regression -- cannot reach this crate at all, so without a
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

/// Every host on either list is a bare registrable name, not a URL or a
/// pattern.
///
/// The matcher is exact-or-dot-suffix, so an entry with a scheme, a port, a
/// path or a leading dot silently matches nothing at all -- it compiles, every
/// other test here passes, and the host it was meant to stop goes through. That
/// is the deny-list's characteristic failure: a wrong entry looks exactly like
/// a right one.
#[test]
fn every_entry_is_a_bare_host() {
    for entry in BLOCKED_HOST_SUFFIXES
        .iter()
        .chain(BLOCKED_FIRST_PARTY_HOST_SUFFIXES)
    {
        assert_eq!(
            *entry,
            entry.to_ascii_lowercase(),
            "{entry} must be lowercase: the matcher lowercases the host it is \
             given and compares literally"
        );
        assert!(
            !entry.starts_with('.'),
            "{entry} must not lead with a dot: `.foo.com` matches neither \
             `foo.com` nor `a.foo.com`"
        );
        for forbidden in ['/', ':', '*', ' '] {
            assert!(
                !entry.contains(forbidden),
                "{entry} must be a bare host, with no {forbidden:?}"
            );
        }
        assert!(
            entry.contains('.'),
            "{entry} has no dot, so it is not a registrable host and would only \
             ever match itself"
        );
    }
}
