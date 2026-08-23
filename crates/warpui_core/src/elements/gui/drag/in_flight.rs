//! Whether a drag is happening anywhere, and a way to stop it.
//!
//! A [`crate::elements::DraggableState`] is owned by the view that renders the
//! element, which is right for everything a drag normally does and useless for
//! the one thing it cannot: answering *"is the user dragging something right
//! now?"* from somewhere else in the app. Without that answer there is no
//! cancel key, because the code that sees the keystroke is nowhere near the
//! code that owns the drag.
//!
//! Keystrokes are dispatched by the keymap along the responder chain **before**
//! the element tree is offered the event, and the chain is walked innermost
//! first — so a `Draggable` never sees a key press, and a binding on an
//! ancestor view loses to the focused one. That is why this is a process-global
//! register and not a context, a subscription, or an element arm; the check has
//! to be answerable from inside whichever view happens to own the focus.
//!
//! What a cancel here means: **the drag stops where it is and does not commit.**
//! Nothing is replayed backwards. For a gesture that previews and then commits
//! on drop — a pane header — that is a true cancel, because nothing had
//! happened yet. For one that mutates as it moves — the tab strip reorders live
//! — the movement so far stands. Callers that hold extra state built up during
//! the drag are responsible for clearing it; this only stops the drag.

use parking_lot::Mutex;

use super::DraggableState;

/// Every drag that has started and not yet finished.
///
/// A `Vec` rather than a set because `DraggableState` has no identity to hash,
/// and because the expected length is zero or one — a second entry means two
/// windows are being dragged in at once, which the platform does not do.
///
/// Entries are removed lazily, by [`prune`], rather than at the exact moment a
/// drag ends. Precise deregistration would mean finding every path that leaves
/// `Dragging` and adding a call, and missing one would leak an entry that makes
/// [`any_in_flight`] answer yes forever. Asking each entry whether it is still
/// dragging cannot go stale, because the entry itself is the answer.
static IN_FLIGHT: Mutex<Vec<DraggableState>> = Mutex::new(Vec::new());

/// Records that a drag has started. Called by `Draggable` on the transition
/// into `Dragging`, and nowhere else.
pub(super) fn register(state: DraggableState) {
    let mut in_flight = IN_FLIGHT.lock();
    in_flight.retain(DraggableState::is_dragging);
    in_flight.push(state);
}

/// Whether anything in this process is mid-drag.
pub fn any_in_flight() -> bool {
    let mut in_flight = IN_FLIGHT.lock();
    prune(&mut in_flight);
    !in_flight.is_empty()
}

/// Stops every drag in flight, and says whether there was one.
///
/// The return value is the point: a caller swallowing a keystroke needs to know
/// whether it was consumed, and asking first and cancelling second would be two
/// lock acquisitions with a gap in between.
pub fn cancel_all() -> bool {
    let mut in_flight = IN_FLIGHT.lock();
    prune(&mut in_flight);
    let cancelled = !in_flight.is_empty();
    for state in in_flight.drain(..) {
        state.cancel_drag();
    }
    cancelled
}

fn prune(in_flight: &mut Vec<DraggableState>) {
    in_flight.retain(DraggableState::is_dragging);
}

#[cfg(test)]
#[path = "in_flight_tests.rs"]
mod tests;
