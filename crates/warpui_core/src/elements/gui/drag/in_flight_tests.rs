use pathfinder_geometry::vector::vec2f;

use super::*;

/// The register is process-global, so tests that touch it cannot interleave.
static SERIAL: Mutex<()> = Mutex::new(());

fn dragging() -> DraggableState {
    let state = DraggableState::default();
    state.set_dragging(vec2f(10., 10.), vec2f(0., 0.));
    state
}

#[test]
fn nothing_is_in_flight_by_default() {
    let _guard = SERIAL.lock();
    cancel_all();

    assert!(!any_in_flight());
}

#[test]
fn a_registered_drag_is_in_flight_until_it_is_cancelled() {
    let _guard = SERIAL.lock();
    cancel_all();

    let state = dragging();
    register(state.clone());
    assert!(any_in_flight());

    assert!(cancel_all());
    assert!(!any_in_flight());
    assert!(!state.is_dragging(), "the owner's state must be cancelled");
}

#[test]
fn cancelling_nothing_says_so() {
    let _guard = SERIAL.lock();
    cancel_all();

    // The return value is what a caller uses to decide whether to swallow a
    // keystroke. Answering "yes, cancelled" when there was no drag would eat
    // an Escape that something else wanted.
    assert!(!cancel_all());
}

#[test]
fn a_drag_that_ended_on_its_own_does_not_linger() {
    let _guard = SERIAL.lock();
    cancel_all();

    let state = dragging();
    register(state.clone());
    // A normal drop: the element stores `None` without telling this register,
    // which is the case lazy pruning exists for.
    state.cancel_drag();

    assert!(!any_in_flight());
    assert!(!cancel_all(), "and it must not be reported as cancelled");
}

#[test]
fn a_second_drag_does_not_displace_the_first() {
    let _guard = SERIAL.lock();
    cancel_all();

    let first = dragging();
    let second = dragging();
    register(first.clone());
    register(second.clone());

    assert!(cancel_all());
    assert!(!first.is_dragging());
    assert!(!second.is_dragging());
}
