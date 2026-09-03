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

    // `has_singleton_model` rather than a bare `as_ref`, because this is now
    // called from `SessionContext::from_session` -- which every skill, chip
    // and agent-tool decision goes through -- and the manager is genuinely
    // absent in test apps that register only what they exercise. Panicking
    // there would make an unrelated test's harness depend on a routing
    // decision it never asked about. Absent means no host, which is the same
    // answer as "no server attached".
    let connected_host = if ctx.has_singleton_model::<RemoteServerManager>() {
        RemoteServerManager::as_ref(ctx)
            .host_for_connected_session(session_id)
            .cloned()
    } else {
        None
    };
    classify(session.session_type(), session.is_wsl(), connected_host)
}

#[cfg(target_family = "wasm")]
pub fn session_filesystem(
    session: &Session,
    _session_id: SessionId,
    _ctx: &AppContext,
) -> SessionFilesystem {
    classify(session.session_type(), session.is_wsl(), None)
}

/// Which spelling of a session's path this process can open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Spelling {
    /// Already this process's own — open it as given.
    Verbatim,
    /// Behind an emulation layer this process can reach; ask the session to
    /// convert it (`\\wsl$\<distro>\…` from Windows, or the MSYS2 root).
    Convert,
    /// No spelling of this path reaches this process.
    Refuse,
}

/// The rule, with its four inputs named, and split out for the same reason
/// [`classify`] is: it is the part with a decision in it, and everything around
/// it is a lookup.
///
/// **The `!windows` row is here because the first cut got it wrong and shipped.**
/// T20.1 assumed a WSL session could only exist on a Windows host and made every
/// non-Windows answer for one `Refuse`. It is exactly backwards on the platform
/// this fork is developed on: `bash_body.sh:1423` sends `wsl_name` from
/// `$WSL_DISTRO_NAME` unconditionally, so a **Linux** Warp running inside WSL
/// reports `is_wsl()` for every pane it owns — and those panes' files are simply
/// its own. Found by review 2026-09-03; the transcript wrote nothing at all on
/// that build for as long as the mistake stood.
pub(crate) fn spelling(
    host_is_windows: bool,
    session_type: SessionType,
    session_distro: Option<&str>,
    this_process_distro: Option<&str>,
) -> Spelling {
    // Another machine. No spelling reaches it, and treating it as local reads
    // *this* filesystem for another host's paths -- the hazard `Unreachable`
    // exists for above.
    if matches!(session_type, SessionType::WarpifiedRemote { .. }) {
        return Spelling::Refuse;
    }
    match (host_is_windows, session_distro) {
        // Windows looking into a distribution: the UNC spelling. Windows with no
        // distribution: MSYS2 or plain, which the session's own converter
        // handles -- including refusing a Unix-encoded path that no `PathBuf`
        // here can hold.
        (true, _) => Spelling::Convert,
        // Same distribution: this process *is* in there, so the path is already
        // its own. Compared case-insensitively because a distribution name is
        // (`canonicalize_wsl_unc_path` folds it for the same reason).
        (false, Some(distro))
            if this_process_distro.is_some_and(|own| own.eq_ignore_ascii_case(distro)) =>
        {
            Spelling::Verbatim
        }
        // A different distribution, from outside Windows: there is no `\\wsl$`
        // to reach it through.
        (false, Some(_)) => Spelling::Refuse,
        (false, None) => Spelling::Verbatim,
    }
}

/// The distribution this process is running inside, if any.
fn this_process_distro() -> Option<String> {
    std::env::var("WSL_DISTRO_NAME")
        .ok()
        .filter(|v| !v.is_empty())
}

/// A path this session reported, spelled so **this process** can open it with
/// `std::fs` — or `None` when this process cannot open it at all.
///
/// **This is a sibling of [`session_filesystem`], not a caller of it, and the
/// difference is the whole reason it exists.** `SessionFilesystem` answers
/// *which route file operations should take*, and for a WSL session it answers
/// `Host` as soon as a remote-development server is attached. That is the right
/// answer for the file tree, which has a server to ask. It is the wrong answer
/// for a caller that only needs to open one small file, because a WSL session's
/// files are reachable from Windows either way — through `\\wsl$\<distro>\…` —
/// and routing on `Host` would make the caller's fate depend on whether anyone
/// had run `remote wsl connect`, which is exactly the "order two unrelated
/// commands were typed in" bug T16 phase 3 removed elsewhere.
///
/// The rule is [`spelling`]; read its table there rather than restating it here.
///
/// **`None` is a real stop, never "fall back to the path as given".** T20.1
/// measured what falling back costs: `WARP_FORK_TRANSCRIPT` joined a WSL pane's
/// `/home/effatha/git/warp` on the Windows side, Windows resolved it to
/// `C:\home\effatha\git\warp\…`, **creating it succeeded**, and 43,014 bytes of
/// the user's prompts landed in a tree at the root of `C:` that nothing on
/// either side of the boundary was ever going to read. Nothing errored and
/// nothing logged, because there was no error: a POSIX-rooted path is a
/// perfectly good relative-to-the-current-drive path on Windows.
pub fn native_path(session: &Session, path: &str) -> Option<std::path::PathBuf> {
    // The one `session_type()` read here, and it is about identity rather than
    // routing -- see `spelling`.
    match spelling(
        cfg!(windows),
        session.session_type(),
        session.wsl_distro_name(),
        this_process_distro().as_deref(),
    ) {
        Spelling::Verbatim => Some(std::path::PathBuf::from(path)),
        Spelling::Convert => {
            let typed = session.convert_directory_to_typed_path_buf(path.to_owned());
            session.maybe_convert_to_native_path(&typed.to_path()).ok()
        }
        Spelling::Refuse => None,
    }
}

#[cfg(test)]
#[path = "filesystem_tests.rs"]
mod tests;
