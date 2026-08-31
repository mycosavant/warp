//! Fork policy: a last-resort egress backstop for telemetry and analytics.
//!
//! Requests built through [`crate::Client`] are checked in two places, and the
//! second exists because the first sentence here used to claim there was only
//! one. `Client::execute_inner` covers every verb builder and the oauth2
//! adapter; `RequestBuilder::eventsource` hands its request to
//! `reqwest_eventsource` **directly** and reaches `execute_inner` never, so it
//! carries the same check itself (`RequestBuilder::redirect_if_blocked`).
//!
//! **The claim that stood here — *"a check there cannot be bypassed by a call
//! site that forgot"* — was false from the day it was written**, and it was
//! false in the file the fork's strongest claim rests on. It was not a live leak:
//! every SSE call site targets Warp's own service, which is not on the list
//! below. That is the reason to close it rather than note it, since "no call
//! site does this today" is a fact about today and a backstop is supposed to be
//! a fact about the code. Found 2026-08-31 by an agent in Warp's own panel,
//! asked to walk the request path and name any way to reach the network without
//! passing the check.
//!
//! **So the standing instruction for anyone adding a way out of `Client`:** a
//! new method that sends bytes without going through `execute_inner` needs its
//! own `redirect_if_blocked` call, and this list needs a line. Grep for
//! `self.wrapped` in `lib.rs` — that is the shape of a bypass.
//!
//! This is deliberately the *last* line of defence, not the first:
//!
//! 1. Don't compile the code in. Sentry is gated by the `ln` Cargo feature
//!    (`warp_logging`, `warp_errors`) and by `crash_reporting` in `app`.
//!    A build without both contains no Sentry at all.
//! 2. Force the telemetry feature flags off (`app/src/fork.rs`).
//! 3. This module, which catches anything the first two missed.
//!
//! ## Known limitation
//!
//! This only covers traffic that goes through [`crate::Client`]. **The Sentry
//! SDK ships its own HTTP transport and does not use this client**, so if a
//! build ever enables `ln` or `crash_reporting`, this backstop will *not* stop
//! it. Layer 1 is the only real defence there. Do not read "no blocked-egress
//! warnings in the log" as proof the process is telemetry-free — verify
//! against a proxy instead.
//!
//! And the list below is a **deny-list**: an unlisted host is an allowed host.
//! It stops the vendors named in it and nothing else, so a new upstream
//! telemetry endpoint on a domain nobody has added passes straight through. That
//! is a deliberate trade — an allow-list would have to enumerate every host Warp
//! legitimately talks to, and getting that wrong breaks the product silently
//! rather than leaking silently — but it means the list is the whole of the
//! protection, and it only ever protects retroactively.

/// Set to `1`/`true` to allow telemetry egress (e.g. to compare fork
/// behaviour against upstream). Absent or any other value keeps blocking.
const ALLOW_ENV_VAR: &str = "WARP_FORK_ALLOW_TELEMETRY_EGRESS";

/// Hosts that must never receive data.
///
/// Matched as exact host or dot-suffix, so `sentry.io` also covers
/// `o12345.ingest.sentry.io`. Suffix matching is what makes this useful —
/// vendors rotate per-tenant subdomains constantly.
const BLOCKED_HOST_SUFFIXES: &[&str] = &[
    // Crash and error reporting.
    "sentry.io",
    "bugsnag.com",
    // Product analytics / CDP. `segment` and `rudderstack` are the two the
    // survey found actually referenced in this codebase (148 and 16 files).
    "segment.io",
    "segment.com",
    "rudderstack.com",
    "rudderlabs.com",
    "amplitude.com",
    "mixpanel.com",
    "posthog.com",
    "heap.io",
    "fullstory.com",
    // Metrics / APM.
    "datadoghq.com",
    "datadoghq.eu",
    "newrelic.com",
    // Google analytics surfaces.
    "google-analytics.com",
    "analytics.google.com",
    "googletagmanager.com",
    // Feature-flag / experiment services, which double as behavioural
    // telemetry sinks.
    "statsig.com",
    "launchdarkly.com",
];

/// Where blocked requests are redirected.
///
/// Port 0 can never be connected to, so the request fails immediately at the
/// socket layer and no payload is transmitted anywhere — including to
/// localhost. Rewriting beats returning an error only because
/// `reqwest::Error` has no public constructor; the observable result is the
/// same (the caller sees a connection failure).
const BLACKHOLE_URL: &str = "http://0.0.0.0:0/";

/// Whether the egress backstop is active for this process.
pub(crate) fn is_active() -> bool {
    !matches!(
        std::env::var(ALLOW_ENV_VAR).as_deref(),
        Ok("1") | Ok("true")
    )
}

/// Returns true if `host` is, or is a subdomain of, a blocked host.
pub(crate) fn is_blocked_host(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    BLOCKED_HOST_SUFFIXES
        .iter()
        .any(|blocked| host == *blocked || host.ends_with(&format!(".{blocked}")))
}

/// Returns true if the request to `url` must be blocked.
pub(crate) fn is_blocked(url: &reqwest::Url) -> bool {
    if !is_active() {
        return false;
    }
    url.host_str().is_some_and(is_blocked_host)
}

/// The URL blocked requests are rewritten to.
pub(crate) fn blackhole_url() -> reqwest::Url {
    // Parsed from a const literal that is covered by a test, so this cannot
    // fail in practice.
    reqwest::Url::parse(BLACKHOLE_URL).expect("BLACKHOLE_URL is a valid URL")
}

#[cfg(test)]
#[path = "egress_tests.rs"]
mod tests;
