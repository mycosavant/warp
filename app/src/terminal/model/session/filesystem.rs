//! Where a session's files actually live.
//!
//! **This exists because the question was being asked in several places and
//! they did not agree.** T16 measured the consequence: a WSL session is
//! `SessionType::Local` — `determine_session_type` decides by hostname
//! equality and WSL2 inherits the Windows machine name — while its files are
//! inside the distribution. Every call site that asked `session_type()` got
//! `Local` and reached across the 9p redirector for them, at roughly 20 ms per
//! directory entry, while a remote-development server sat idle inside the
//! distribution that could have answered from ext4.
//!
//! Phase 1 fixed two of those call sites with a private predicate on
//! `TerminalView`. Routing only one of the two produced
//! `Repository not found` from a server that had never been told the
//! repository existed — so the failure mode of an incomplete answer is already
//! on the record. This module is that predicate promoted to the one place
//! everything can reach, so a third call site cannot quietly disagree with the
//! first two.
//!
//! It answers *reachability*, not classification. `SessionType` stays what
//! bootstrap decided it was, deliberately: it also drives path conversion,
//! agent execution context, command corrections and chips, and reclassifying a
//! WSL session breaks all of them. See T16's "Do not reclassify the session".

use warp_core::SessionId;
use warpui::AppContext;

use super::{Session, SessionType};

/// Where the files a session talks about can actually be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionFilesystem {
    /// This process can open the session's paths directly.
    Local,
    /// The paths belong to `host_id` and must be reached through the
    /// remote-development server.
    Host(warp_core::HostId),
    /// The session's files are on another host and no server is attached, so
    /// they cannot be reached at all. Distinct from `Local` on purpose: a
    /// caller that treats this as local will read the *wrong machine's*
    /// filesystem or silently find nothing.
    Unreachable,
}

impl SessionFilesystem {
    /// The host to route requests to, if any.
    pub fn host(&self) -> Option<&warp_core::HostId> {
        match self {
            Self::Host(host_id) => Some(host_id),
            Self::Local | Self::Unreachable => None,
        }
    }

    /// Whether this process may open the session's paths with `std::fs`.
    pub fn is_local(&self) -> bool {
        matches!(self, Self::Local)
    }
}

/// The decision itself, with its three inputs named.
///
/// Split out from `session_filesystem` so it can be tested without an
/// `AppContext`: the lookups around it are a `session_type()` read, an
/// `is_wsl()` read and a map lookup on the remote-server manager, and this is
/// the part with a rule in it. T16 recorded that the routing seam has no unit
/// test because a `RemoteTransport` double does not exist — that is still true
/// of the *transport*, and it was never true of the rule.
pub(crate) fn classify(
    session_type: SessionType,
    is_wsl: bool,
    connected_host: Option<warp_core::HostId>,
) -> SessionFilesystem {
    match session_type {
        SessionType::WarpifiedRemote {
            host_id: Some(host_id),
        } => SessionFilesystem::Host(host_id),
        SessionType::WarpifiedRemote { host_id: None } => SessionFilesystem::Unreachable,
        // A WSL session says `Local` and is not. Only a *connected* server
        // makes its files reachable; without one the caller keeps the local
        // UNC path it has always had, because a host that cannot answer is
        // worse than a slow path that can.
        SessionType::Local => match (is_wsl, connected_host) {
            (true, Some(host_id)) => SessionFilesystem::Host(host_id),
            (true, None) | (false, _) => SessionFilesystem::Local,
        },
    }
}

/// Resolves where `session`'s files live.
#[cfg(not(target_family = "wasm"))]
pub fn session_filesystem(
    session: &Session,
    session_id: SessionId,
    ctx: &AppContext,
) -> SessionFilesystem {
    use warpui::SingletonEntity as _;

    use crate::remote_server::manager::RemoteServerManager;

    classify(
        session.session_type(),
        session.is_wsl(),
        RemoteServerManager::as_ref(ctx)
            .host_for_connected_session(session_id)
            .cloned(),
    )
}

#[cfg(target_family = "wasm")]
pub fn session_filesystem(
    session: &Session,
    _session_id: SessionId,
    _ctx: &AppContext,
) -> SessionFilesystem {
    classify(session.session_type(), session.is_wsl(), None)
}

#[cfg(test)]
#[path = "filesystem_tests.rs"]
mod tests;
