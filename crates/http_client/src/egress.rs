//! Fork policy: a last-resort egress backstop, in `reqwest` terms.
//!
//! **The lists, the switches and the argument for both live in the
//! [`egress_policy`] crate**, which has no dependencies and is consulted by
//! `crates/websocket` as well. Read that crate's docs first; this module is the
//! part that only makes sense once there is a [`reqwest::Request`] in hand.
//!
//! It was the whole of the policy until 2026-09-09, and it moved because
//! `crates/websocket` dials its own sockets and could not reach it. Having
//! `websocket` depend on `http_client` is not merely bad layering, it is a
//! cycle -- `http_client` depends on `warp_core`, which depends on `websocket`
//! -- so the policy became a leaf that both can sit on top of.
//!
//! Requests built through [`crate::Client`] are checked in two places here, and
//! the second exists because this file's own docs used to claim there was only
//! one. `Client::execute_inner` covers every verb builder and the oauth2
//! adapter; `RequestBuilder::eventsource` hands its request to
//! `reqwest_eventsource` **directly** and reaches `execute_inner` never, so it
//! carries the same check itself (`RequestBuilder::redirect_if_blocked`).
//! `websocket::WebSocket::connect` is the third, in the other crate.
//!
//! **So the standing instruction for anyone adding a way out of `Client`:** a
//! new method that sends bytes without going through `execute_inner` needs its
//! own `redirect_if_blocked` call. Grep for `self.wrapped` in `lib.rs` -- that
//! is the shape of a bypass.

/// Where blocked requests are redirected.
///
/// Port 0 can never be connected to, so the request fails immediately at the
/// socket layer and no payload is transmitted anywhere — including to
/// localhost. Rewriting beats returning an error only because
/// `reqwest::Error` has no public constructor; the observable result is the
/// same (the caller sees a connection failure).
///
/// `websocket` does *not* do this, and the difference is the error type rather
/// than a disagreement about policy: its connect path returns
/// `anyhow::Result`, so a refusal there can say what refused it and why. This
/// one cannot, which is why the blackhole exists at all.
const BLACKHOLE_URL: &str = "http://0.0.0.0:0/";

/// Returns true if the request to `url` must be blocked.
///
/// Each half of the policy answers to its own switch, so lifting one never
/// lifts the other; see [`egress_policy`]. A URL with no host (`file:`,
/// `data:`) is not blocked -- there is no host for the deny-list to have an
/// opinion about, and nothing leaves the machine.
pub(crate) fn is_blocked(url: &reqwest::Url) -> bool {
    url.host_str().is_some_and(egress_policy::blocks_host)
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
