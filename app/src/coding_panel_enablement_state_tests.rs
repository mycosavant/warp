use super::CodingPanelEnablementState;

#[test]
fn a_routed_wsl_pane_is_a_remote_session_with_a_server() {
    assert_eq!(
        CodingPanelEnablementState::from_session_env_with_wsl_routing(
            true, false, true, false, true
        ),
        CodingPanelEnablementState::RemoteSession {
            has_remote_server: true
        }
    );
}

#[test]
fn an_unrouted_wsl_pane_is_still_unsupported() {
    assert_eq!(
        CodingPanelEnablementState::from_session_env_with_wsl_routing(
            true, false, true, false, false
        ),
        CodingPanelEnablementState::UnsupportedSession
    );
}

#[test]
fn routing_is_a_fact_about_wsl_panes_only() {
    // A non-WSL session cannot be "routed" by this arm; the flag is ignored
    // and upstream's rule answers.
    assert_eq!(
        CodingPanelEnablementState::from_session_env_with_wsl_routing(
            true, false, false, false, true
        ),
        CodingPanelEnablementState::Enabled
    );
    assert_eq!(
        CodingPanelEnablementState::from_session_env_with_wsl_routing(
            true, true, false, false, true
        ),
        CodingPanelEnablementState::RemoteSession {
            has_remote_server: false
        }
    );
}

#[test]
fn the_upstream_rule_is_unchanged_when_nothing_is_routed() {
    for is_enabled in [true, false] {
        for is_remote in [true, false] {
            for is_wsl in [true, false] {
                for has_remote_server in [true, false] {
                    assert_eq!(
                        CodingPanelEnablementState::from_session_env_with_wsl_routing(
                            is_enabled,
                            is_remote,
                            is_wsl,
                            has_remote_server,
                            false,
                        ),
                        CodingPanelEnablementState::from_session_env(
                            is_enabled,
                            is_remote,
                            is_wsl,
                            has_remote_server
                        ),
                    );
                }
            }
        }
    }
}
