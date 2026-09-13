use ::local_control::ErrorCode;
use warpui::App;

use super::{ControlState, is_active, start, state_of, stop};

/// With no `LocalControlBridge` registered, every entry point answers "nothing
/// is paired" instead of panicking.
///
/// The bridge is absent under `WARP_FORK_POLICY=0`, which skips
/// `fork::FORCE_ENABLED` and so never turns on `WarpControlCli`, and in every
/// upstream test fixture. Until 2026-09-12 all four functions called
/// `LocalControlBridge::as_ref` unconditionally: `stop` ran on every
/// conversation removal and panicked 17 upstream tests
/// (`.fork/runs/lib-baseline-2026-09-12/`), and the footer's tick reaches
/// `state_of` after the chip is clicked with fork policy off.
///
/// No bridge means no listener was started, so no pairing exists: zero, idle,
/// inactive and a refusal are the exact answers, not a degradation.
///
/// Calibrate by making `pairing_context` skip its `has_singleton_model` check:
/// this reddens on the first call with "never registered".
#[test]
fn every_entry_point_answers_nothing_paired_when_no_bridge_is_registered() {
    App::test((), |app| async move {
        app.read(|ctx| {
            assert_eq!(stop("conversation", ctx), 0);
            assert!(matches!(state_of("conversation", ctx), ControlState::Idle));
            assert!(!is_active("conversation", ctx));
            let refusal = start("conversation", ctx)
                .expect_err("with no bridge there is no wide listener to pair through");
            assert_eq!(refusal.code, ErrorCode::LocalControlDisabled);
        });
    });
}
