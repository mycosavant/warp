#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CodingPanelEnablementState {
    Enabled,
    /// An SSH command has been detected at preexec time but the remote
    /// session has not finished bootstrapping yet. The file tree should
    /// show a loading state immediately to avoid flickering the stale
    /// local tree.
    PendingRemoteSession,
    /// The active session is on a remote host.
    ///
    /// `has_remote_server` is `true` when the session is registered with
    /// `RemoteServerManager` (i.e. Auto SSH Warpification / mode 1). When
    /// `true`, remote repo metadata may arrive and the file tree should show
    /// a loading state. When `false` (tmux or subshell SSH), no data will
    /// arrive and the file tree should show a disabled message.
    RemoteSession {
        has_remote_server: bool,
    },
    UnsupportedSession,
    Disabled,
}

impl CodingPanelEnablementState {
    pub(crate) fn from_session_env(
        is_enabled: bool,
        is_remote: bool,
        is_unsupported_session: bool,
        has_remote_server: bool,
    ) -> Self {
        if is_remote {
            Self::RemoteSession { has_remote_server }
        } else if is_unsupported_session {
            Self::UnsupportedSession
        } else if is_enabled {
            Self::Enabled
        } else {
            Self::Disabled
        }
    }
}

impl CodingPanelEnablementState {
    /// `from_session_env` with the one fact the fork's WSL routing adds.
    ///
    /// A WSL pane is `SessionType::Local` (`session/filesystem.rs` says why)
    /// and upstream reads that as *unsupported*: the tree, search and the
    /// diff panel all take the `is_wsl` bool and draw a "doesn't work in WSL"
    /// fallback. Since T16 a WSL pane can have Warp's remote-development
    /// server attached inside its distribution, and then it is exactly what
    /// upstream calls a remote session with a server: repo metadata arrives
    /// from the daemon, and the panels should wait for it rather than
    /// declare the pane unsupported. `wsl_routed` is
    /// `session_filesystem(..).host().is_some()` for a WSL session.
    ///
    /// An unrouted WSL pane keeps the `UnsupportedSession` arm on purpose.
    /// Its files are still read from Windows over 9p, which is slow rather
    /// than broken; what changed for it is only the fallback text, which now
    /// names that.
    pub(crate) fn from_session_env_with_wsl_routing(
        is_enabled: bool,
        is_remote: bool,
        is_wsl: bool,
        has_remote_server: bool,
        wsl_routed: bool,
    ) -> Self {
        let wsl_routed = is_wsl && wsl_routed;
        Self::from_session_env(
            is_enabled,
            is_remote || wsl_routed,
            is_wsl && !wsl_routed,
            has_remote_server || wsl_routed,
        )
    }
}

#[cfg(test)]
#[path = "coding_panel_enablement_state_tests.rs"]
mod tests;
