use super::*;

#[test]
fn enabled_support_uses_remote_server_for_ssh_wrapper_when_feature_is_enabled() {
    assert!(SshRemoteServerSupport::Enabled.should_use_remote_server(true, true,));
}

#[test]
fn disabled_support_skips_remote_server_for_ssh_wrapper() {
    assert!(!SshRemoteServerSupport::Disabled.should_use_remote_server(true, true,));
}

#[test]
fn enabled_support_skips_remote_server_when_feature_is_disabled() {
    assert!(!SshRemoteServerSupport::Enabled.should_use_remote_server(false, true,));
}

#[test]
fn enabled_support_skips_remote_server_for_non_ssh_session() {
    assert!(!SshRemoteServerSupport::Enabled.should_use_remote_server(true, false,));
}

/// A WSL pane attaches a server to its own distribution, and only its own.
#[test]
fn a_wsl_session_connects_to_the_distribution_it_was_launched_into() {
    assert_eq!(
        wsl_auto_connect_target(
            SshRemoteServerSupport::Enabled,
            true,
            true,
            Some("Ubuntu"),
            false,
        ),
        Some("Ubuntu".to_owned())
    );
}

/// Every gate the SSH arm has, the WSL arm has too, plus the fork's own.
#[test]
fn a_wsl_session_stays_unconnected_when_any_gate_is_shut() {
    let open = |support, feature, fork| {
        wsl_auto_connect_target(support, feature, fork, Some("Ubuntu"), false)
    };
    assert_eq!(open(SshRemoteServerSupport::Disabled, true, true), None);
    assert_eq!(open(SshRemoteServerSupport::Enabled, false, true), None);
    assert_eq!(open(SshRemoteServerSupport::Enabled, true, false), None);
}

/// A session with no distribution is not a WSL session, whatever else it is.
#[test]
fn a_session_without_a_distribution_is_left_alone() {
    assert_eq!(
        wsl_auto_connect_target(SshRemoteServerSupport::Enabled, true, true, None, false),
        None
    );
    assert_eq!(
        wsl_auto_connect_target(SshRemoteServerSupport::Enabled, true, true, Some(""), false),
        None
    );
}

/// `ssh` typed into a WSL pane is the SSH arm's session, not this one's.
#[test]
fn an_ssh_wrapper_session_belongs_to_the_ssh_arm_even_inside_wsl() {
    assert_eq!(
        wsl_auto_connect_target(
            SshRemoteServerSupport::Enabled,
            true,
            true,
            Some("Ubuntu"),
            true,
        ),
        None
    );
}
