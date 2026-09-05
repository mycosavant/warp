//! Language servers for routed WSL buffers (`.fork/docs/wsl.md`, item 3).
//!
//! A routed WSL pane's buffers are `LocalOrRemotePath::Remote`, and every
//! entry point of the LSP stack -- the manager's keys, the editor's attach,
//! the buffer model's document lifecycle, `PersistedWorkspace`'s enablement
//! -- takes a `Path`. Upstream resolves that with `to_local_path()`, which is
//! `None` for a remote buffer, so a routed buffer gets syntax highlighting
//! from its extension and nothing else.
//!
//! This module gives such a buffer a path: the `\\wsl$\<distro>\...` spelling
//! the rest of the fork already keys WSL directories on
//! (`warp_util::path::canonicalize_wsl_unc_path`). Nothing downstream needs
//! to know the buffer is remote. The workspace root is the same UNC path the
//! unrouted pane next door uses, so enabling `rust-analyzer` from either pane
//! enables it for both and they share one server; the server itself runs
//! inside the distribution, spawned through `wsl.exe` by
//! `lsp::LspServerConfig::with_wsl_distro`, and `lsp::UriMapper::WslDistro`
//! spells the path as `file:///...` on the wire and back.
//!
//! What this needs that nothing held before is *which distribution a host
//! is*. A remote buffer carries a `HostId`; the daemon's handshake mints it,
//! and the manager keeps a label (`WSL: Ubuntu`) rather than the name. So
//! [`WslHosts`] records the pairing when a WSL session connects, from the
//! session that connected it, which is the one place both facts are in hand.
//!
//! Every answer here is gated on `fork::wsl_lsp_in_distro_enabled`, so with
//! it off a routed buffer has no LSP path and the fork behaves as upstream.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use remote_server::manager::RemoteServerManager;
use typed_path::PathType;
use warp_util::host_id::HostId;
use warp_util::local_or_remote_path::LocalOrRemotePath;
use warp_util::path::parse_wsl_unc_path;
use warp_util::remote_path::RemotePath;
use warp_util::standardized_path::StandardizedPath;
use warpui::{AppContext, Entity, SingletonEntity};

use crate::fork;

/// Which WSL distribution each connected host is.
#[derive(Default)]
pub struct WslHosts {
    distros: HashMap<HostId, String>,
}

impl Entity for WslHosts {
    type Event = ();
}

impl SingletonEntity for WslHosts {}

pub fn init(app: &mut AppContext) {
    app.add_singleton_model(|_| WslHosts::default());
}

impl WslHosts {
    /// Records that `host_id` is a server inside `distro`. Called from the
    /// `Sessions` subscription on `SessionConnected`, for a session whose
    /// `wsl_distro_name()` is set. Never removed: a host id names one
    /// daemon identity per machine and user, and a stale entry can only be
    /// consulted through a remote buffer, which implies a live connection.
    pub fn record(&mut self, host_id: HostId, distro: String) {
        self.distros.insert(host_id, distro);
    }

    pub fn distro_for(&self, host_id: &HostId) -> Option<&str> {
        self.distros.get(host_id).map(String::as_str)
    }

    /// The host inside `distro`, compared the way the redirector does:
    /// distribution names are case-insensitive.
    pub fn host_for_distro(&self, distro: &str) -> Option<&HostId> {
        self.distros
            .iter()
            .find(|(_, d)| d.eq_ignore_ascii_case(distro))
            .map(|(host, _)| host)
    }
}

/// `\\wsl$\<lower-case distro>\<linux path>`, the fork's canonical spelling of
/// a file inside a distribution, built without touching the filesystem.
pub fn wsl_unc_path(distro: &str, linux_path: &str) -> PathBuf {
    let mut spelled = format!(r"\\wsl$\{}", distro.to_ascii_lowercase());
    if linux_path != "/" {
        spelled.push_str(&linux_path.replace('/', r"\"));
    }
    PathBuf::from(spelled)
}

/// The distribution `root` is inside, without asking the policy. Pure, so it
/// can be asserted; callers want [`distro_for_lsp_root`].
pub fn distro_of_root(root: &Path) -> Option<String> {
    parse_wsl_unc_path(root).map(|parsed| parsed.distro)
}

/// The distribution a language server for `root` should run inside, or
/// `None` to run it on this machine as upstream does. `None` for every root
/// that is not a WSL UNC path, and for every root while the fork's switch is
/// off.
pub fn distro_for_lsp_root(root: &Path) -> Option<String> {
    if !fork::wsl_lsp_in_distro_enabled() {
        return None;
    }
    distro_of_root(root)
}

/// The path the LSP stack knows a buffer by. A local buffer's own path; for a
/// remote buffer on a host [`WslHosts`] knows, the `\\wsl$` spelling of its
/// Linux path; `None` for any other remote buffer, which is upstream's answer.
pub fn lsp_path_for(location: &LocalOrRemotePath, ctx: &AppContext) -> Option<PathBuf> {
    match location {
        LocalOrRemotePath::Local(path) => Some(path.clone()),
        LocalOrRemotePath::Remote(remote) => {
            if !fork::wsl_lsp_in_distro_enabled() || !ctx.has_singleton_model::<WslHosts>() {
                return None;
            }
            let distro = WslHosts::as_ref(ctx)
                .distro_for(&remote.host_id)?
                .to_owned();
            Some(wsl_unc_path(&distro, remote.path.as_str()))
        }
    }
}

/// Where a path the LSP stack handed back actually lives. A server inside a
/// distribution answers with paths inside it, mapped to `\\wsl$\...` by
/// `lsp::UriMapper`; when a host for that distribution is connected, the file
/// is that host's remote buffer and opens through the daemon rather than over
/// the redirector. Anything else is the local path it was.
pub fn location_for_lsp_path(path: &Path, ctx: &AppContext) -> LocalOrRemotePath {
    if fork::wsl_lsp_in_distro_enabled()
        && ctx.has_singleton_model::<WslHosts>()
        && ctx.has_singleton_model::<RemoteServerManager>()
        && let Some(parsed) = parse_wsl_unc_path(path)
        && let Some(host) = WslHosts::as_ref(ctx).host_for_distro(&parsed.distro)
        && RemoteServerManager::as_ref(ctx)
            .client_for_host(host)
            .is_some()
        && let Ok(standardized) =
            StandardizedPath::try_with_encoding(&parsed.linux_path, PathType::Unix)
    {
        return LocalOrRemotePath::Remote(RemotePath::new(host.clone(), standardized));
    }
    LocalOrRemotePath::Local(path.to_path_buf())
}

/// The workspace root to enable a server for, given a buffer's LSP path.
///
/// Upstream's order, kept: a workspace `PersistedWorkspace` already holds,
/// then the repository `DetectedRepositories` found for the *local* path.
/// The fork's addition is the third step: for a routed buffer the
/// repository was registered by the daemon as a remote one, so the local
/// lookup misses and the parent directory would have been enabled instead
/// of the repository. The remote root is asked for and spelled back the same
/// way the buffer was.
#[cfg(feature = "local_fs")]
pub fn repo_root_for_lsp_path(path: &Path, ctx: &AppContext) -> Option<PathBuf> {
    use repo_metadata::repositories::DetectedRepositories;

    use crate::ai::persisted_workspace::PersistedWorkspace;

    if let Some(root) = PersistedWorkspace::as_ref(ctx).root_for_workspace(path) {
        return Some(root.to_path_buf());
    }
    let repositories = DetectedRepositories::as_ref(ctx);
    if let Some(root) = repositories
        .get_root_for_path(&LocalOrRemotePath::Local(path.to_path_buf()))
        .and_then(|root| root.to_local_path().map(Path::to_path_buf))
    {
        return Some(root);
    }
    let location = location_for_lsp_path(path, ctx);
    if location.is_remote()
        && let Some(root) = repositories.get_root_for_path(&location)
        && let Some(root) = lsp_path_for(&root, ctx)
    {
        return Some(root);
    }
    path.parent().map(Path::to_path_buf)
}

#[cfg(test)]
#[path = "routed_lsp_tests.rs"]
mod tests;
