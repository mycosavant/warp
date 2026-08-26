//! The console — the client T11 built a backend for (T12.1).
//!
//! # Why this exists at all
//!
//! T11 shipped an event log, a state snapshot, an SSE stream, a LAN listener,
//! QR pairing and remote approve/deny, and every one of them was reachable only
//! by `curl`. The named failure this fork was started over is *features
//! implemented but never wired*, and five tickets sitting behind no client is
//! that failure with the fork's own name on it. This module is one route, one
//! page, and the security policy that page makes necessary.
//!
//! # Why the page is unauthenticated, and why that is not a hole
//!
//! A browser following a QR cannot send an `Authorization` header, so the
//! document itself has to be free to fetch. That is safe because the document
//! is a **constant**: two `include_str!`s, no interpolation, no template, no
//! secret. Everything that requires authority happens in `fetch` calls the
//! script makes afterwards, each carrying a scoped credential the page obtained
//! by redeeming a pairing code.
//!
//! The pairing code arrives in the URL **fragment**, which browsers never send
//! to a server. `pairing::pair_url` has built its QR that way since T11.4, with
//! a note saying the guarantee was a convention until a page existed to hold it.
//! This is that page, so the note is now discharged — and the QR is retargeted
//! here, because a fragment convention that lands a browser on a `POST`-only
//! route was a client in name only.
//!
//! What is disclosed by serving it: anyone who can reach the listener learns
//! that this machine runs the fork. On loopback that is nothing. On the wide
//! listener `WARP_FORK_CONTROL_BIND` opens, it is a fingerprint — but so is a
//! port that answers `403` to everything, which is what was there before.
//!
//! # Why the script is a second route rather than an inline `<script>`
//!
//! So the policy can say `script-src 'self'` instead of `'unsafe-inline'`. The
//! page renders text three parties author — agents, tools, and whatever an agent
//! read off disk — and it is attacker-influenced by construction. The script
//! never assigns `innerHTML`; `script-src 'self'` is what makes that discipline
//! survive somebody forgetting it once. Styles stay inline, because
//! `default-src 'none'` leaves an injected stylesheet nowhere to send anything.

use axum::http::header::{
    CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, HeaderName, REFERRER_POLICY,
    X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS,
};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};

/// Where the console is served, and — since T12.1 — where a pairing QR points.
pub(crate) const CONSOLE_PATH: &str = "/";

/// The script, as its own route. See the module docs for why it is not inline.
pub(crate) const CONSOLE_SCRIPT_PATH: &str = "/console.js";

const CONSOLE_HTML: &str = include_str!("console.html");
const CONSOLE_SCRIPT: &str = include_str!("console.js");

/// The policy the page is served under.
///
/// `default-src 'none'` first, so every fetch directive that is not named below
/// is denied rather than defaulted — including `img-src` and `font-src`, which
/// are the usual way an injected stylesheet talks to the outside world.
///
/// `connect-src 'self'` is what lets the script reach `/v1/*` and nothing else:
/// a page that somehow ran hostile code still could not post what it read
/// anywhere, which matters more here than on an ordinary site because what it
/// can read is a live agent transcript.
const POLICY: &str = "default-src 'none'; \
     script-src 'self'; \
     style-src 'unsafe-inline'; \
     connect-src 'self'; \
     base-uri 'none'; \
     form-action 'none'; \
     frame-ancestors 'none'";

/// Answers `GET /` — the console document (T12.1).
pub(super) async fn handle_console_request() -> Response {
    served(CONSOLE_HTML, "text/html; charset=utf-8")
}

/// Answers `GET /console.js` — the console's script (T12.1).
pub(super) async fn handle_console_script_request() -> Response {
    served(CONSOLE_SCRIPT, "text/javascript; charset=utf-8")
}

/// The headers both documents carry.
///
/// `no-store` rather than a cache lifetime: a phone that keeps the console in a
/// back/forward cache is a phone showing an agent's state from an hour ago, and
/// a stale view of "is anything waiting on me" is worse than no view.
fn served(body: &'static str, content_type: &'static str) -> Response {
    let headers: [(HeaderName, &'static str); 6] = [
        (CONTENT_TYPE, content_type),
        (CONTENT_SECURITY_POLICY, POLICY),
        (X_CONTENT_TYPE_OPTIONS, "nosniff"),
        // Belt and braces with `frame-ancestors`, for anything that honours
        // only the older header.
        (X_FRAME_OPTIONS, "DENY"),
        // The fragment is never sent anyway, but a `Referer` would leak the
        // instance's address to anything the page ever linked to.
        (REFERRER_POLICY, "no-referrer"),
        (CACHE_CONTROL, "no-store"),
    ];
    let mut response = (StatusCode::OK, body).into_response();
    for (name, value) in headers {
        response
            .headers_mut()
            .insert(name, HeaderValue::from_static(value));
    }
    response
}

#[cfg(test)]
#[path = "console_tests.rs"]
mod tests;
