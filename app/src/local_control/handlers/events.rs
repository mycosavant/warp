//! The read surface's discovery half (T11.2).
//!
//! `events.subscribe` is the only catalog action that does not *do* the thing it
//! names. It answers where the stream is and how long the caller's credential
//! will open it for; the stream itself is `GET /v1/events`, because
//! server-sent events are not a request/response shape and [`RequestEnvelope`]
//! is.
//!
//! Splitting it this way keeps one credential flow instead of two: a client asks
//! the broker for an `events.subscribe` grant, spends it here to learn the URL,
//! and presents the same bearer to the stream.
//!
//! [`RequestEnvelope`]: ::local_control::RequestEnvelope
use ::local_control::{ControlError, CredentialGrant, ErrorCode};

use crate::local_control::EVENT_STREAM_PATH;

/// Answers `events.subscribe`.
///
/// The bearer token is not echoed back. The caller already has it — presenting
/// it is what got them here — and a secret in a result is a secret in a shell
/// scrollback.
pub fn events_subscribe(
    control_origin: Option<&str>,
    grant: &CredentialGrant,
) -> Result<serde_json::Value, ControlError> {
    let Some(origin) = control_origin else {
        // Reachable only if the bridge was asked before `start()` told it where
        // the server bound. Refuse rather than guess a port: a URL that points
        // at nothing is worse than an error, because a client will retry it.
        return Err(ControlError::new(
            ErrorCode::BridgeUnavailable,
            "local-control bridge does not know its own control endpoint yet",
        ));
    };
    let result = ::local_control::EventStreamResult {
        url: format!("http://{origin}{EVENT_STREAM_PATH}"),
        expires_at: grant.expires_at,
    };
    serde_json::to_value(result).map_err(|err| {
        ControlError::with_details(
            ErrorCode::Internal,
            "failed to serialize event stream result",
            err.to_string(),
        )
    })
}

#[cfg(test)]
#[path = "events_tests.rs"]
mod tests;
