use ::local_control::protocol::{SessionSelector, SessionTarget, TabTarget, TargetSelector};
use warp_core::HostId;

use super::{
    SessionFilesystem, SurfaceDestination, filesystem_value, session_inspect_target,
    surface_unavailable_reason,
};
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

// The regression this whole group pins: `warpctrl session inspect` with no
// argument used to answer `ambiguous_target` on any profile with more than
// one tab open, because `is_active` is one-per-tab (a structural invariant of
// `PaneGroup`, not a restore artifact) and the old defaulting helper
// (`active_session_target`) was a no-op for the argument-less case, leaving
// pane selection unscoped across every tab before the session-level filter
// ever ran. Reproduced live 2026-09-12 against two real tabs before this fix
// (`ambiguous_target`) and after it (resolves to one).

#[test]
fn no_target_at_all_narrows_to_the_active_tab_and_session() {
    let (target, session) = session_inspect_target(&TargetSelector::default());
    assert_eq!(
        target.tab,
        Some(TabTarget::Active),
        "would else see every tab"
    );
    assert_eq!(
        target.window, None,
        "select_tab_entries's own cascade widens this"
    );
    assert_eq!(
        target.pane, None,
        "unconstrained: session, not UI focus, disambiguates"
    );
    assert_eq!(session, Some(SessionTarget::Active));
}

#[test]
fn an_explicit_active_session_target_also_narrows_to_the_active_tab() {
    let given = TargetSelector {
        session: Some(SessionTarget::Active),
        ..TargetSelector::default()
    };
    let (target, session) = session_inspect_target(&given);
    assert_eq!(target.tab, Some(TabTarget::Active));
    assert_eq!(session, Some(SessionTarget::Active));
}

#[test]
fn an_explicit_session_id_is_not_narrowed_to_the_active_tab() {
    // The trap this function's first draft fell into: widening `tab` to
    // `Active` unconditionally would make a lookup for a specific session id
    // sitting in a background tab miss it, by scoping the search to the
    // active tab before the id filter ever runs.
    let given = TargetSelector {
        session: Some(SessionTarget::Id {
            id: SessionSelector("some-other-tab-session".to_string()),
        }),
        ..TargetSelector::default()
    };
    let (target, session) = session_inspect_target(&given);
    assert_eq!(
        target.tab, None,
        "an id can live in any tab, not just the active one"
    );
    assert_eq!(
        session,
        Some(SessionTarget::Id {
            id: SessionSelector("some-other-tab-session".to_string())
        })
    );
}

#[test]
fn an_explicit_tab_is_not_overridden_by_the_active_default() {
    let given = TargetSelector {
        tab: Some(TabTarget::Index { index: 2 }),
        ..TargetSelector::default()
    };
    let (target, _session) = session_inspect_target(&given);
    assert_eq!(target.tab, Some(TabTarget::Index { index: 2 }));
}
