use warp_core::HostId;

use super::*;

fn host() -> HostId {
    HostId::new("e35d6030-5e55-4940-a1ec-17a6cc6d064e".to_string())
}

#[test]
fn a_wsl_session_with_a_connected_server_is_reached_through_the_host() {
    // The whole reason this module exists. `SessionType::Local` is what
    // bootstrap decided and it stays that way; the files are still somewhere
    // else. Measured in T16: routing this arm took a repository's tree from a
    // 32-minute walk over 9p to a load served from ext4.
    assert_eq!(
        classify(SessionType::Local, true, Some(host())),
        SessionFilesystem::Host(host()),
    );
}

#[test]
fn a_wsl_session_without_a_server_keeps_the_local_path_it_has_always_had() {
    // Deliberately `Local`, not `Unreachable`: the UNC path works, it is
    // merely slow, and a WSL pane that has never run `remote wsl connect` must
    // not lose its file tree to a host that does not exist.
    assert_eq!(
        classify(SessionType::Local, true, None),
        SessionFilesystem::Local,
    );
}

#[test]
fn a_connected_server_does_not_capture_a_session_that_is_not_wsl() {
    // The manager is keyed by session id, so this should not arise — but the
    // `is_wsl` gate is what makes that a property of the code rather than of
    // the manager's bookkeeping. An ordinary local pane stays local even if a
    // host id is somehow associated with it.
    assert_eq!(
        classify(SessionType::Local, false, Some(host())),
        SessionFilesystem::Local,
    );
}

#[test]
fn an_ordinary_local_session_is_local() {
    assert_eq!(
        classify(SessionType::Local, false, None),
        SessionFilesystem::Local,
    );
}

#[test]
fn a_warpified_remote_session_routes_to_its_own_host() {
    assert_eq!(
        classify(
            SessionType::WarpifiedRemote {
                host_id: Some(host())
            },
            false,
            None,
        ),
        SessionFilesystem::Host(host()),
    );
}

#[test]
fn a_remote_session_with_no_host_is_unreachable_rather_than_local() {
    // The distinction that matters: a caller treating this as `Local` reads
    // *this* machine's filesystem for paths that belong to another one, which
    // succeeds often enough to be worse than failing.
    assert_eq!(
        classify(SessionType::WarpifiedRemote { host_id: None }, false, None),
        SessionFilesystem::Unreachable,
    );
    assert!(!classify(SessionType::WarpifiedRemote { host_id: None }, false, None).is_local());
}

#[test]
fn only_the_host_variant_offers_a_host_to_route_to() {
    assert_eq!(SessionFilesystem::Host(host()).host(), Some(&host()));
    assert_eq!(SessionFilesystem::Local.host(), None);
    assert_eq!(SessionFilesystem::Unreachable.host(), None);
}
