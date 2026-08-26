//! `control.pair` — showing a code so a device can let itself in (T11.4).
//!
//! Like `events.subscribe`, this action does not *do* the thing it names. It
//! mints a pairing code and answers with the URL to encode and the QR already
//! rendered; the pairing itself happens when a device POSTs that code to
//! `/v1/pair`, which is a route rather than an action because the caller is by
//! definition not yet authorized to invoke actions.
//!
//! **Why the app renders the QR rather than the CLI.** The encoder already
//! exists — `drive::sharing::qr_code`, built for Warp Drive share links and
//! generic over a URL, which is the whole of what this needed. Rendering here
//! reuses it; rendering in `warp_cli` would have meant a second `qrcode`
//! dependency and a second implementation of the same thing.
use ::local_control::{ControlError, ErrorCode};

use crate::drive::sharing::qr_code::{QUIET_ZONE_MODULES, qr_matrix_for_url};
use crate::local_control::bridge::PairingContext;
use crate::local_control::console::CONSOLE_PATH;
use crate::local_control::pairing::{pair_url, pairable_actions};

/// Answers `control.pair`.
pub fn control_pair(pairing: Option<&PairingContext>) -> Result<serde_json::Value, ControlError> {
    let Some(pairing) = pairing else {
        // The common case by far, since the wide bind is off by default. Say
        // which variable turns it on rather than reporting a bare refusal: this
        // error is the entire discovery path for the feature.
        return Err(ControlError::new(
            ErrorCode::LocalControlDisabled,
            "this instance has no wide listener, so there is nothing to pair with; \
             set WARP_FORK_CONTROL_BIND to the address to listen on and restart",
        ));
    };
    let issued = {
        let mut pairings = pairing.pairings.lock().map_err(|_| {
            ControlError::new(ErrorCode::Internal, "local-control pairing is unavailable")
        })?;
        pairings.issue_code(chrono::Utc::now())
    };
    // **The QR points at the console, not at `/v1/pair` (T12.1).** It pointed at
    // the route until a page existed, and that URL was never scannable: `/v1/pair`
    // is `POST`-only, so a phone following it got `405` and a person got a dead
    // QR. The code still ends up POSTed there — by the page, from the fragment.
    let url = pair_url(&pairing.origin, CONSOLE_PATH, &issued.code);
    let result = ::local_control::PairingResult {
        qr: render_qr(&url)?,
        url,
        expires_at: issued.expires_at,
        actions: pairable_actions()
            .iter()
            .map(|action| action.as_str().to_owned())
            .collect(),
    };
    serde_json::to_value(result).map_err(|err| {
        ControlError::with_details(
            ErrorCode::Internal,
            "failed to serialize pairing result",
            err.to_string(),
        )
    })
}

/// Draws the matrix with half-block characters, two module rows per text row.
///
/// A QR module is square and a terminal cell is not, so one character per module
/// produces a code stretched to twice its height — which scanners tolerate
/// poorly and people read as broken. Half blocks put two rows in one cell and
/// come out roughly square.
///
/// Light modules are drawn, not skipped. A QR needs the quiet zone and the light
/// modules to be *light*, and on a terminal whose background is already dark,
/// "leave it blank" means "leave it dark" — the code would not scan at all. So
/// the light modules are painted with a character and the caller is expected to
/// show it as-is.
pub(super) fn render_qr(url: &str) -> Result<String, ControlError> {
    let matrix = qr_matrix_for_url(url).map_err(|err| {
        ControlError::with_details(
            ErrorCode::Internal,
            "failed to encode the pairing QR code",
            err.to_string(),
        )
    })?;
    let quiet = QUIET_ZONE_MODULES;
    let width = matrix.width();
    let side = width + quiet * 2;
    // Both bounds are checked here rather than left to `is_dark`, which cannot
    // do it: it indexes `y * width + x` into a flat `Vec`, so an `x` past the
    // right edge does not fall off the end, it *wraps into the next row*. A
    // quiet zone built on that would be a strip of the following row's modules
    // printed where the margin should be, and the code would not scan.
    let dark = |x: usize, y: usize| -> bool {
        let (Some(x), Some(y)) = (x.checked_sub(quiet), y.checked_sub(quiet)) else {
            return false;
        };
        x < width && y < width && matrix.is_dark(x, y)
    };
    let mut out = String::new();
    for row in (0..side).step_by(2) {
        for column in 0..side {
            // Inverted on purpose: a dark module is drawn as an *unlit* half, so
            // the rendering reads correctly against a light background. Terminals
            // are usually dark, which is why the light modules are painted.
            out.push(match (dark(column, row), dark(column, row + 1)) {
                (true, true) => ' ',
                (true, false) => '▄',
                (false, true) => '▀',
                (false, false) => '█',
            });
        }
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
#[path = "pairing_tests.rs"]
mod tests;
