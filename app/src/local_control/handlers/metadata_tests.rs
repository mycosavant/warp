use warp_core::HostId;

use super::{SessionFilesystem, SurfaceDestination, filesystem_value, surface_unavailable_reason};
use crate::features::FeatureFlag;

#[test]
fn agent_management_surface_reports_feature_flag_unavailable() {
    let flag_guard = FeatureFlag::AgentManagementView.override_enabled(false);
    warpui::App::test((), |mut app| async move {
        assert_eq!(
            app.update(|ctx| {
                surface_unavailable_reason(SurfaceDestination::AgentManagement, ctx)
            }),
            Some("agent management is unavailable or disabled")
        );
    });
    drop(flag_guard);
}

#[test]
fn a_session_inspect_reports_the_host_its_files_are_on() {
    // The instrument T16 phase 2 exists to provide. A WSL session with a
    // connected server keeps `SessionType::Local` on purpose, so nothing in
    // this payload could previously distinguish it from a session whose files
    // really are on this machine.
    let host = HostId::new("e35d6030-5e55-4940-a1ec-17a6cc6d064e".to_string());
    let value = filesystem_value(Some(&SessionFilesystem::Host(host)));
    assert_eq!(value["where"], "host");
    assert_eq!(value["host_id"], "e35d6030-5e55-4940-a1ec-17a6cc6d064e");
}

#[test]
fn an_unreachable_session_is_not_reported_as_local() {
    // The distinction a script must be able to branch on: "no server attached
    // yet" is not "the files are here". Reporting `local` would send a caller
    // to this machine's filesystem for another machine's paths.
    let value = filesystem_value(Some(&SessionFilesystem::Unreachable));
    assert_eq!(value["where"], "unreachable");
    assert!(value.get("host_id").is_none());

    let local = filesystem_value(Some(&SessionFilesystem::Local));
    assert_eq!(local["where"], "local");
    assert!(local.get("host_id").is_none());
}

#[test]
fn a_pane_with_no_session_yet_says_unknown_rather_than_local() {
    let value = filesystem_value(None);
    assert_eq!(value["where"], "unknown");
}
