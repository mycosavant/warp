//! `/remote-control`, the fork's way (2026-09-05).
//!
//! Upstream's chip of that name relays the pane and the conversation through
//! Warp's sharing server, over a closed protocol, behind a login; T19 read it
//! and found nothing to point elsewhere. This is the fork's: the same words,
//! meaning what Claude Code's `/remote-control` means. A person at the machine
//! points at one conversation, a QR appears, a phone scans it, and that phone
//! can watch the conversation's record, answer its permission requests, stop
//! it, and prompt it -- that conversation and no other. Stop sharing cuts the
//! phone off.
//!
//! Everything here is a thin call into the pairing map the control server
//! already keeps (`pairing.rs`, `Scope::Control`) through the bridge's
//! handle, so the chip and `warpctrl pair show --conversation` mint the same
//! code and the console pairs the same way whichever produced it. Nothing new
//! is served; the wide listener has to be open (`WARP_FORK_CONTROL_BIND`), and
//! the refusal when it is not names the variable, because that refusal is the
//! whole discovery path for the feature.
use ::local_control::ControlError;
use ::local_control::protocol::PairingResult;
use warpui::{AppContext, SingletonEntity};

use crate::local_control::LocalControlBridge;
use crate::local_control::handlers::pairing::mint;
use crate::local_control::pairing::Scope;

/// Hands a conversation to whatever scans the code this returns.
///
/// The caller has the conversation on screen, so it exists; no lookup here.
pub(crate) fn start(
    conversation_id: &str,
    app: &AppContext,
) -> Result<PairingResult, ControlError> {
    let pairing = LocalControlBridge::as_ref(app).pairing().cloned();
    mint(
        pairing.as_ref(),
        Scope::Control {
            conversation_id: conversation_id.to_owned(),
        },
    )
}

/// Takes a conversation back: every device paired for it and every unscanned
/// code for it stop working. Returns how many devices were cut off, so the
/// panel can say whether a phone was connected or only a code had been shown.
pub(crate) fn stop(conversation_id: &str, app: &AppContext) -> usize {
    let Some(pairing) = LocalControlBridge::as_ref(app).pairing().cloned() else {
        return 0;
    };
    // The credentials first, because they are what a request carries: a
    // device that is gone from the pairing map but still holds a five-minute
    // credential is a device that is not gone. Measured, before this line.
    if let Ok(mut credentials) = pairing.credentials.lock() {
        crate::local_control::confine::purge_confined(&mut credentials, conversation_id);
    }
    let Ok(mut pairings) = pairing.pairings.lock() else {
        return 0;
    };
    pairings.revoke_conversation(conversation_id)
}

/// Whether a conversation is handed to a device right now: a live device
/// paired for it, or a code for it still waiting to be scanned. What the chip
/// reads to decide between *start* and *stop*.
pub(crate) fn is_active(conversation_id: &str, app: &AppContext) -> bool {
    let Some(pairing) = LocalControlBridge::as_ref(app).pairing().cloned() else {
        return false;
    };
    let Ok(pairings) = pairing.pairings.lock() else {
        return false;
    };
    pairings.is_controlling(conversation_id, chrono::Utc::now())
}
