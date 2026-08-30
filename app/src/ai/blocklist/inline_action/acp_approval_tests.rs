//! The rules that do not need a view context. The control itself is verified by
//! running it — the fork's standard for anything with a surface.

use super::*;

fn view() -> AcpApprovalView {
    AcpApprovalView::new("conv-1".to_owned())
}

/// A fresh control is not armed. The first tap on *Yes* can never answer.
#[test]
fn nothing_is_armed_to_begin_with() {
    assert!(!view().armed_for("turn-1:0"));
}

/// Arming is per request, and this is the point of it.
///
/// Three surfaces can answer one question — the panel, `warpctrl`, and the
/// console — and a turn can end under any of them. So between a person reading
/// this request and tapping twice, the request can be gone and a different one
/// can be parked in its place. A boolean `armed` would send the second tap to
/// the newcomer carrying a decision made about its predecessor, which is the
/// stale-answer hazard the control plane's digest exists to prevent.
#[test]
fn arming_one_request_does_not_arm_the_next_one() {
    let mut view = view();
    view.armed = Some("turn-1:0".to_owned());

    assert!(view.armed_for("turn-1:0"), "the armed request is armed");
    assert!(
        !view.armed_for("turn-1:1"),
        "a request that arrived after the first tap is not armed by it"
    );
}

/// A conversation with nothing parked has no question, so the control renders
/// nothing rather than an empty frame. `waiting_for` is process-global and this
/// id belongs to no test that parks anything.
#[test]
fn a_conversation_with_nothing_parked_has_no_question() {
    assert!(
        AcpApprovalView::new("conv-t1416-empty".to_owned())
            .current()
            .is_none()
    );
}
