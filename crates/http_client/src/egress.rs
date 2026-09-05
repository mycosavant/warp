//! Fork policy: a last-resort egress backstop.
//!
//! **Two lists, two switches, two different claims.**
//! [`BLOCKED_HOST_SUFFIXES`] is telemetry and analytics vendors, which must
//! never receive data. [`BLOCKED_FIRST_PARTY_HOST_SUFFIXES`] is Warp's own
//! services, which the product legitimately uses and this fork has replaced
//! one at a time. Conflating them would be convenient and wrong: the second
//! list has a legitimate reason to be lifted — `WARP_FORK_POLICY=0` is the
//! documented way to A/B a suspected fork regression against stock upstream,
//! and it cannot reach this module — so it answers to
//! `WARP_FORK_ALLOW_WARP_EGRESS` rather than to the telemetry switch.
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

/// Set to `1`/`true` to allow this build to talk to Warp's own services.
/// Absent or any other value keeps blocking.
///
/// Deliberately **not** the same switch as [`ALLOW_ENV_VAR`], and not
/// `WARP_FORK_POLICY`. The two lists below are blocked for different reasons
/// and one of them has a legitimate reason to be lifted: this file already
/// tells people to run `WARP_FORK_POLICY=0` to A/B a suspected fork
/// regression against stock upstream, and `is_active` reads neither that nor
/// anything in `app::fork` — so without a switch of its own, first-party
/// blocking would silently break the one debugging workflow the fork
/// documents. Telemetry has no such case and keeps its own, narrower switch.
const ALLOW_FIRST_PARTY_ENV_VAR: &str = "WARP_FORK_ALLOW_WARP_EGRESS";

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

/// Warp's own services, blocked because this fork does not use them.
///
/// **This is a different claim from the list above and is worth keeping
/// separate.** Those hosts must never receive data under any reading of the
/// fork's thesis. These are hosts the *product* legitimately talks to, that
/// this fork has replaced one at a time — the agent transport, `/ai/transcribe`,
/// `/ai/relevant_files`, the embedding index, Warp Drive, autoupdate — and the
/// entry here is what makes "replaced" mean "cannot happen" rather than "does
/// not happen on the paths anybody checked".
///
/// The specific hole it closes: `ai::agent::api::generate_multi_agent_output`
/// intercepts for the ACP and local agents **only when one is configured**.
/// With neither `WARP_FORK_ACP_COMMAND` nor `WARP_FORK_LOCAL_AGENT` set it
/// falls through to `warp_multi_agent_client` and sends the user's prompt and
/// context to `app.warp.dev`, with no warning and nothing in the log. That is
/// upstream's default behaving exactly as upstream intends, and it was the one
/// live first-party call left when this list was written (2026-09-04).
///
/// `ai::agent::api::r#impl` refuses that turn first, with a message naming the
/// two variables, because a blocked request surfaces as an opaque connection
/// failure rather than an explanation. This is the backstop under that refusal,
/// for the call site that has not been written yet.
///
/// Note `firebase_auth_api_key` and Firebase's own hosts are **not** here.
/// Sign-in is gated in `app::fork::account_gate_bypassed` rather than at the
/// socket, and blocking Google's identity endpoints by suffix would reach
/// further than this fork's argument does.
const BLOCKED_FIRST_PARTY_HOST_SUFFIXES: &[&str] = &[
    // `app.warp.dev` (GraphQL, `/ai/*`, `/client_version`), `rtc.app.warp.dev`
    // and `sessions.app.warp.dev` are all suffixes of this one.
    "warp.dev",
];

/// Where blocked requests are redirected.
///
/// Port 0 can never be connected to, so the request fails immediately at the
/// socket layer and no payload is transmitted anywhere — including to
/// localhost. Rewriting beats returning an error only because
/// `reqwest::Error` has no public constructor; the observable result is the
/// same (the caller sees a connection failure).
const BLACKHOLE_URL: &str = "http://0.0.0.0:0/";

/// Whether the telemetry half of the backstop is active for this process.
pub(crate) fn is_active() -> bool {
    !matches!(
        std::env::var(ALLOW_ENV_VAR).as_deref(),
        Ok("1") | Ok("true")
    )
}

/// Whether the first-party half of the backstop is active for this process.
pub(crate) fn first_party_is_active() -> bool {
    !matches!(
        std::env::var(ALLOW_FIRST_PARTY_ENV_VAR).as_deref(),
        Ok("1") | Ok("true")
    )
}

fn normalize(host: &str) -> String {
    host.trim_end_matches('.').to_ascii_lowercase()
}

fn matches_suffix(host: &str, suffixes: &[&str]) -> bool {
    suffixes
        .iter()
        .any(|blocked| host == *blocked || host.ends_with(&format!(".{blocked}")))
}

/// Returns true if `host` is, or is a subdomain of, a host on either list.
///
/// Deliberately pure: it answers "is this host listed", not "would this request
/// be blocked right now". The switches live in [`is_blocked`], so this stays
/// testable without touching the environment — which matters more since there
/// are now two switches and a test that read one of them would pass or fail
/// depending on how the suite was invoked.
pub(crate) fn is_blocked_host(host: &str) -> bool {
    let host = normalize(host);
    matches_suffix(&host, BLOCKED_HOST_SUFFIXES)
        || matches_suffix(&host, BLOCKED_FIRST_PARTY_HOST_SUFFIXES)
}

/// The blocking rule, with both switches passed in.
///
/// Split out from [`is_blocked`] so the rule that actually matters -- that
/// lifting one switch never lifts the other -- can be tested without mutating
/// process-global environment variables. A test that sets and restores env has
/// to be serialised against every other test that reads the same names, and
/// `http_client` carries no `serial_test`; making the rule pure was cheaper
/// than adding a dependency, and is a better test besides.
fn blocked_by(host: &str, telemetry_active: bool, first_party_active: bool) -> bool {
    let host = normalize(host);
    (telemetry_active && matches_suffix(&host, BLOCKED_HOST_SUFFIXES))
        || (first_party_active && matches_suffix(&host, BLOCKED_FIRST_PARTY_HOST_SUFFIXES))
}

/// Returns true if the request to `url` must be blocked.
///
/// Each half answers to its own switch, so lifting one never lifts the other.
pub(crate) fn is_blocked(url: &reqwest::Url) -> bool {
    url.host_str()
        .is_some_and(|host| blocked_by(host, is_active(), first_party_is_active()))
}

/// The URL blocked requests are rewritten to.
/// Points a built request at the blackhole and takes its payload with it.
///
/// **Both halves matter, and until review 2026-08-31 only the first was
/// done.** Rewriting the URL alone left the headers and the body assembled and
/// attached, so what stopped the request leaving was that `0.0.0.0:0` cannot be
/// connected to. That is a property of a port number, not of this code, and it
/// is strictly weaker than what [`crate::RequestBuilder::redirect_if_blocked`]
/// claims about itself one module over -- on the check that covers essentially
/// all traffic rather than a single method. This module's own docs described
/// the two as enforcement points of equal standing; they now are.
///
/// It also had a reachable edge. `Client::new` sets no `no_proxy`, so system
/// and environment proxies are honoured, and the rewrite drops the scheme to
/// `http` -- which hyper writes to a proxy in absolute-URI form, headers and
/// body first and the unreachable destination only afterwards. The path that
/// matters was immune because upstream's Rudderstack client sets
/// `https_only(true)`, for a reason unrelated to any of this and held by no
/// test here.
pub(crate) fn blackhole(request: &mut reqwest::Request) {
    *request.url_mut() = blackhole_url();
    request.headers_mut().clear();
    *request.body_mut() = None;
}

pub(crate) fn blackhole_url() -> reqwest::Url {
    // Parsed from a const literal that is covered by a test, so this cannot
    // fail in practice.
    reqwest::Url::parse(BLACKHOLE_URL).expect("BLACKHOLE_URL is a valid URL")
}

#[cfg(test)]
#[path = "egress_tests.rs"]
mod tests;
