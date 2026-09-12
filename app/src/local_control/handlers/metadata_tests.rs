use ::local_control::InstanceId;
use ::local_control::protocol::{SessionSelector, SessionTarget, TabTarget, TargetSelector};
use warp_core::HostId;
use warpui::App;

use super::{
    SessionFilesystem, SurfaceDestination, filesystem_value, session_inspect,
    session_inspect_target, surface_unavailable_reason,
};
use crate::features::FeatureFlag;
use crate::local_control::LocalControlBridge;
use crate::local_control::handlers::layout::create_tab;
use crate::workspace::view::tests::{initialize_app, mock_workspace};

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

/// The regression `session_inspect_target`'s own unit tests can name but not
/// witness: `select_tab_entries's own cascade widens this` is a comment on a
/// field the pure test asserts is `None`, which proves the *shape* of the fix
/// but not that a real multi-tab workspace actually resolves through it. This
/// drives the real handler against a real second tab, the way `warpctrl
/// session inspect` is actually called.
///
/// Reproduced live 2026-09-12 against two real tabs before this fix
/// (`ambiguous_target`) and after (resolves to one) --
/// `.fork/runs/` from that session has the transcript; this is the
/// same shape, pinned as an automated test using the pattern
/// `layout_tests.rs` already established for this handler family.
#[test]
fn session_inspect_resolves_the_active_tab_when_a_profile_has_several_tabs() {
    App::test((), |mut app| async move {
        initialize_app(&mut app);
        let _workspace = mock_workspace(&mut app);
        let bridge = app.add_singleton_model(LocalControlBridge::new);
        let instance_id = InstanceId("inst_test".to_owned());

        let (created, response) = bridge.update(&mut app, |bridge, ctx| {
            bridge.set_instance_id(instance_id.clone());
            let created = create_tab(
                &Some(instance_id.clone()),
                &serde_json::json!({}),
                &TargetSelector::default(),
                ctx,
            )
            .expect("tab.create handler succeeds");

            let response = session_inspect(&TargetSelector::default(), ctx).expect(
                "must resolve to the active tab's session, not ambiguous_target, \
                 now that two tabs each have their own is_active pane",
            );
            (created, response)
        });

        assert_eq!(response["action"], "session.inspect");
        assert!(
            response["session"]["session_id"].is_string(),
            "must resolve to exactly one session: {response}"
        );
        // Asserting `is_active` here would prove nothing, and did until
        // 2026-09-12: `select_session_entries`' `Active` arm filters on exactly
        // that field, so every entry reaching this response carries it by
        // construction. It also cannot say *which* tab, since `is_active` is
        // one pane per `PaneGroup` -- the background tab's session reports
        // `true` too, so a regression that resolved the wrong tab passed. Pin
        // the tab the resolved session actually lives in instead.
        assert_eq!(
            response["session"]["tab_id"], created["tab"]["id"],
            "must resolve to the newly created, active tab's session: {response}"
        );
    });
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
