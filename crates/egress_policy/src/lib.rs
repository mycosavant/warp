//! Fork policy: the hosts this build must never talk to, and the switches that
//! lift each half.
//!
//! **Two lists, two switches, two different claims.**
//! [`BLOCKED_HOST_SUFFIXES`] is telemetry and analytics vendors, which must
//! never receive data. [`BLOCKED_FIRST_PARTY_HOST_SUFFIXES`] is Warp's own
//! services, which the product legitimately uses and this fork has replaced
//! one at a time. Conflating them would be convenient and wrong: the second
//! list has a legitimate reason to be lifted -- `WARP_FORK_POLICY=0` is the
//! documented way to A/B a suspected fork regression against stock upstream,
//! and it cannot reach this crate -- so it answers to
//! `WARP_FORK_ALLOW_WARP_EGRESS` rather than to the telemetry switch.
//!
//! # Why this is a crate of its own
//!
//! It was a private module of `http_client` until 2026-09-09, and the reason it
//! moved is the third enforcement point below. `crates/websocket` opens its own
//! sockets through `async-tungstenite` and has never had an `http_client`
//! dependency, so neither of the checks inside `http_client` could see a
//! WebSocket. The obvious repair -- have `websocket` call
//! `http_client::egress` -- **does not compile**: `http_client` depends on
//! `warp_core`, `warp_core` depends on `websocket`, and cargo refuses the
//! cycle. So the policy is a leaf with no dependencies of its own, and anything
//! that opens a socket can consult it.
//!
//! That is worth stating plainly, because the ticket that found the gap
//! (`.fork/tickets/T19-agent-footer-chips.md`) recorded the objection as one of
//! layering -- *"an `http_client` dependency would invert the layering"* -- and
//! a layering argument is one a later reader can weigh and overrule. A cycle is
//! not an opinion.
//!
//! # The enforcement points
//!
//! Three, in two crates, and the count has been wrong twice.
//!
//! | where | covers |
//! |---|---|
//! | `http_client::Client::execute_inner` | every verb builder and the oauth2 adapter |
//! | `http_client::RequestBuilder::redirect_if_blocked` | `eventsource`, which reaches `execute_inner` never |
//! | `websocket::WebSocket::connect` | every WebSocket this workspace opens |
//!
//! **The first count was one**, and `egress.rs` claimed a check there *"cannot
//! be bypassed by a call site that forgot"* while `eventsource` had been
//! bypassing it since the day the sentence was written. **The second count was
//! two**, and it was two for as long as `crates/websocket` existed beside it.
//! Neither miss was a live leak, and in both cases that is the argument for
//! closing it rather than noting it: "no call site does this today" is a fact
//! about today, and a backstop is supposed to be a fact about the code.
//!
//! **So the standing instruction for anyone adding a way to reach the
//! network:** a new path that sends bytes without passing one of the three
//! needs its own check, and this list needs a line. In `http_client`, grep for
//! `self.wrapped` in `lib.rs` -- that is the shape of a bypass. In `websocket`,
//! `imp::connect` is the only door and
//! `the_socket_is_opened_by_exactly_one_function` pins it. Across the
//! workspace, `tungstenite_is_dialled_from_this_crate_alone` pins that nobody
//! dials a WebSocket without coming through here.
//!
//! # This is the last line of defence, not the first
//!
//! 1. Don't compile the code in. Sentry is gated by the `ln` Cargo feature
//!    (`warp_logging`, `warp_errors`) and by `crash_reporting` in `app`.
//!    A build without both contains no Sentry at all.
//! 2. Force the telemetry feature flags off (`app/src/fork.rs`).
//! 3. This policy, which catches anything the first two missed.
//!
//! ## Known limitations
//!
//! **A client that consults nothing is not covered.** The Sentry SDK ships its
//! own HTTP transport and uses neither `http_client` nor `websocket`, so if a
//! build ever enables `ln` or `crash_reporting`, this backstop will *not* stop
//! it. Layer 1 is the only real defence there. Do not read "no blocked-egress
//! warnings in the log" as proof the process is telemetry-free -- verify
//! against a proxy instead.
//!
//! **And the lists below are deny-lists**: an unlisted host is an allowed host.
//! They stop the vendors named in them and nothing else, so a new upstream
//! telemetry endpoint on a domain nobody has added passes straight through.
//! That is a deliberate trade -- an allow-list would have to enumerate every
//! host Warp legitimately talks to, and getting that wrong breaks the product
//! silently rather than leaking silently -- but it means the list is the whole
//! of the protection, and it only ever protects retroactively.

/// Set to `1`/`true` to allow telemetry egress (e.g. to compare fork
/// behaviour against upstream). Absent or any other value keeps blocking.
pub const ALLOW_ENV_VAR: &str = "WARP_FORK_ALLOW_TELEMETRY_EGRESS";

/// Set to `1`/`true` to allow this build to talk to Warp's own services.
/// Absent or any other value keeps blocking.
///
/// Deliberately **not** the same switch as [`ALLOW_ENV_VAR`], and not
/// `WARP_FORK_POLICY`. The two lists below are blocked for different reasons
/// and one of them has a legitimate reason to be lifted: the fork's own docs
/// tell people to run `WARP_FORK_POLICY=0` to A/B a suspected fork regression
/// against stock upstream, and [`is_active`] reads neither that nor anything in
/// `app::fork` -- so without a switch of its own, first-party blocking would
/// silently break the one debugging workflow the fork documents. Telemetry has
/// no such case and keeps its own, narrower switch.
pub const ALLOW_FIRST_PARTY_ENV_VAR: &str = "WARP_FORK_ALLOW_WARP_EGRESS";

/// Hosts that must never receive data.
///
/// Matched as exact host or dot-suffix, so `sentry.io` also covers
/// `o12345.ingest.sentry.io`. Suffix matching is what makes this useful --
/// vendors rotate per-tenant subdomains constantly.
pub const BLOCKED_HOST_SUFFIXES: &[&str] = &[
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
/// this fork has replaced one at a time -- the agent transport,
/// `/ai/transcribe`, `/ai/relevant_files`, the embedding index, Warp Drive,
/// autoupdate -- and the entry here is what makes "replaced" mean "cannot
/// happen" rather than "does not happen on the paths anybody checked".
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
pub const BLOCKED_FIRST_PARTY_HOST_SUFFIXES: &[&str] = &[
    // `app.warp.dev` (GraphQL, `/ai/*`, `/client_version`), `rtc.app.warp.dev`
    // and `sessions.app.warp.dev` are all suffixes of this one.
    "warp.dev",
];

/// Whether the telemetry half of the backstop is active for this process.
pub fn is_active() -> bool {
    !matches!(
        std::env::var(ALLOW_ENV_VAR).as_deref(),
        Ok("1") | Ok("true")
    )
}

/// Whether the first-party half of the backstop is active for this process.
pub fn first_party_is_active() -> bool {
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
/// be blocked right now". The switches live in [`blocks_host`], so this stays
/// testable without touching the environment -- which matters more since there
/// are two switches and a test that read one of them would pass or fail
/// depending on how the suite was invoked.
pub fn is_listed(host: &str) -> bool {
    let host = normalize(host);
    matches_suffix(&host, BLOCKED_HOST_SUFFIXES)
        || matches_suffix(&host, BLOCKED_FIRST_PARTY_HOST_SUFFIXES)
}

/// The blocking rule, with both switches passed in.
///
/// Split out from [`blocks_host`] so the rule that actually matters -- that
/// lifting one switch never lifts the other -- can be tested without mutating
/// process-global environment variables. A test that sets and restores env has
/// to be serialised against every other test that reads the same names, and
/// neither this crate nor `http_client` carries `serial_test`; making the rule
/// pure was cheaper than adding a dependency, and is a better test besides.
pub fn blocked_by(host: &str, telemetry_active: bool, first_party_active: bool) -> bool {
    let host = normalize(host);
    (telemetry_active && matches_suffix(&host, BLOCKED_HOST_SUFFIXES))
        || (first_party_active && matches_suffix(&host, BLOCKED_FIRST_PARTY_HOST_SUFFIXES))
}

/// Returns true if a connection to `host` must be blocked right now.
///
/// Each half answers to its own switch, so lifting one never lifts the other.
/// Callers that hold a parsed URL should pass its host; a URL with no host
/// (`data:`, `file:`) is not something this policy has an opinion about, and
/// the caller decides what to do with `None` rather than this function
/// guessing.
pub fn blocks_host(host: &str) -> bool {
    blocked_by(host, is_active(), first_party_is_active())
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
