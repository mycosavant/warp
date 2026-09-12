use ::local_control::InstanceId;
use ::local_control::protocol::TargetSelector;
use warpui::App;

use super::tab_reset_name;
use crate::local_control::LocalControlBridge;
use crate::workspace::view::tests::{initialize_app, mock_workspace};

/// A tab *write* must resolve the same way a read does when the platform
/// reports no focused window.
///
/// `ctx.windows().active_window()` is `None` whenever no Warp window holds OS
/// focus. On the winit backend that is
/// `windows.find(|w| w.has_focus() && w.is_visible())`
/// (`crates/warpui/src/windowing/winit/window.rs:193`), and it is `None`
/// unconditionally under the test platform
/// (`crates/warpui_core/src/platform/test/delegate.rs:100`) -- which is why
/// this test can witness the condition at all.
///
/// For a control plane invoked from a shell, no focus is the *normal* state:
/// whoever runs `warpctrl` is looking at a terminal, or another monitor.
/// Until 2026-09-12 `select_window_ids` demanded a reported active window, so
/// `tab.rename`, `tab.reset_name`, the two tab-colour actions and the two pane
/// ones answered `missing_target` at the same moment, in the same
/// single-window instance, that `session inspect` resolved fine. `619c345a6`
/// unified the read path onto `active_or_single_window_id` and left this one
/// behind.
///
/// Calibrate by reverting `select_window_ids`' first arm to
/// `require_active_window_id_for_action(ctx.windows().active_window(), action)`:
/// this reddens with `missing_target`, and the metadata read tests do not
/// move.
#[test]
fn a_tab_write_resolves_the_single_window_when_the_platform_reports_no_focus() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let _workspace = mock_workspace(&mut app);
        let bridge = app.add_singleton_model(LocalControlBridge::new);
        let instance_id = InstanceId("inst_test".to_owned());

        let result = bridge.update(&mut app, |bridge, ctx| {
            bridge.set_instance_id(instance_id.clone());
            tab_reset_name(&Some(instance_id.clone()), &TargetSelector::default(), ctx)
        });

        let value = result.expect(
            "a single open window is not an ambiguous target for a write, and no OS-reported \
             focus is the ordinary case for a shell-driven control plane",
        );
        assert_eq!(value["action"], "tab.reset_name");
        assert_eq!(value["ok"], true);
        assert!(
            value["window_id"].is_string(),
            "the resolved window must be named back: {value}"
        );
        assert!(
            value["tab_id"].is_string(),
            "the resolved tab must be named back: {value}"
        );
    });
}
