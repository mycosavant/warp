//! WSL implementation of [`RemoteTransport`] (`.fork/IDEAS.md`, I16).
//!
//! # Why this is small
//!
//! `RemoteTransport` shipped with one implementation. Reading
//! [`super::ssh_transport::SshTransport::connect`] is what shows how little the
//! second one has to do: it spawns `ssh` as a child with piped
//! stdin/stdout/stderr and hands the three pipes to
//! `RemoteServerClient::from_child_streams`. **SSH is a pipe.** Nothing below
//! the transport can tell what spawned it, so this is the same protocol with
//! `wsl.exe` in that position — which is independently how Zed reaches a
//! distro, and why its WSL support is the same code path as its SSH support.
//!
//! Everything else here is a one-shot command whose exit code is interpreted,
//! and the interpretations are deliberately identical to the SSH transport's:
//! `--version` exits 0/126/127, `test -d` exits 0/1. Those are properties of
//! the commands, not of the channel they travelled over.
//!
//! # What WSL does *not* need
//!
//! The SSH transport carries a `socket_path` and a `warp_owns_control_master`
//! flag, and its teardown has to decide whether to run `ssh -O exit` against a
//! master the user might own. None of that exists here. `wsl.exe` needs no
//! ControlMaster, no socket, no re-authentication and no credential: it runs as
//! the same Windows user and lands in the distro as that user's default Linux
//! account. `ControlPath::None` is already in the enum for this case.
//!
//! So the fields reduce to "which distribution", and reconnect is always worth
//! attempting because there is no connection state to have gone stale.
//!
//! # What is not wired yet
//!
//! Nothing constructs this. The SSH transport is reached because warpify
//! detects an `ssh` command being submitted and drives
//! `RemoteServerController` from the resulting `InitSubshell` hook; WSL has no
//! equivalent trigger, and Zed's is an explicit "Add WSL Distro" entry under
//! Open Remote. Choosing that entry point is the next piece of work, and
//! [`remote_server::wsl::list_distros`] exists to populate it.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use remote_server::auth::RemoteServerAuthContext;
use remote_server::client::RemoteServerClient;
use remote_server::manager::RemoteServerExitStatus;
use remote_server::setup::{PreinstallCheckResult, RemotePlatform, parse_uname_output};
use remote_server::transport::{
    Connection, ControlPath, Error, InstallOutcome, InstallSource, RemoteTransport,
};
use remote_server::wsl::{WSL_COMMAND, run_wsl_command, run_wsl_script, wsl_args};
use warpui::r#async::executor;

/// WSL transport: runs the remote server inside a WSL distribution.
#[derive(Clone)]
pub struct WslTransport {
    /// The distribution name as `wsl.exe -l -q` reports it, e.g. `Ubuntu`.
    distro: String,
    auth_context: Arc<RemoteServerAuthContext>,
}

impl fmt::Debug for WslTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WslTransport")
            .field("distro", &self.distro)
            .finish_non_exhaustive()
    }
}

impl WslTransport {
    pub fn new(distro: impl Into<String>, auth_context: Arc<RemoteServerAuthContext>) -> Self {
        Self {
            distro: distro.into(),
            auth_context,
        }
    }

    pub fn distro(&self) -> &str {
        &self.distro
    }

    /// The command run inside the distribution to become the protocol peer.
    ///
    /// Identical to the SSH transport's, because it is a property of the
    /// server binary rather than of how we reached it.
    fn remote_proxy_command(&self) -> String {
        let binary = remote_server::setup::remote_server_binary();
        let identity_key = self.auth_context.remote_server_identity_key();
        let quoted_identity_key = shell_words::quote(&identity_key);
        format!("{binary} remote-server-proxy --identity-key {quoted_identity_key}")
    }
}

impl RemoteTransport for WslTransport {
    fn detect_platform(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<RemotePlatform, Error>> + Send>> {
        let distro = self.distro.clone();
        Box::pin(async move {
            let output =
                run_wsl_command(&distro, "uname -sm", remote_server::setup::CHECK_TIMEOUT).await?;
            if output.status.success() {
                // Passthrough of the Linux process's stdout, so this is UTF-8
                // and needs no decoding — unlike `wsl.exe -l -q`, which is
                // UTF-16LE. See `remote_server::wsl`.
                parse_uname_output(&String::from_utf8_lossy(&output.stdout))
            } else {
                let code = output.status.code().unwrap_or(-1);
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(Error::Other(anyhow::anyhow!(
                    "uname -sm exited with code {code}: {stderr}"
                )))
            }
        })
    }

    fn run_preinstall_check(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<PreinstallCheckResult, Error>> + Send>> {
        let distro = self.distro.clone();
        Box::pin(async move {
            match run_wsl_script(
                &distro,
                remote_server::setup::PREINSTALL_CHECK_SCRIPT,
                remote_server::setup::CHECK_TIMEOUT,
            )
            .await
            {
                Ok(output) if output.status.success() => Ok(PreinstallCheckResult::parse(
                    &String::from_utf8_lossy(&output.stdout),
                )),
                Ok(output) => Err(Error::ScriptFailed {
                    exit_code: output.status.code().unwrap_or(-1),
                    stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
                }),
                Err(e) => Err(e.into()),
            }
        })
    }

    fn check_binary(&self) -> Pin<Box<dyn Future<Output = Result<bool, Error>> + Send>> {
        let distro = self.distro.clone();
        Box::pin(async move {
            let cmd = remote_server::setup::binary_check_command();
            log::info!("Running binary check in WSL distro {distro}: {cmd}");
            let output =
                run_wsl_command(&distro, &cmd, remote_server::setup::CHECK_TIMEOUT).await?;
            // Same contract as the SSH transport: `<binary> --version` exits 0
            // when present and functional, 127 when not found, 126 when found
            // but not executable. Anything else is a transport-level failure.
            let code = output.status.code();
            log::info!(
                "Binary check result: exit={code:?} stdout={}",
                String::from_utf8_lossy(&output.stdout)
            );
            match code {
                Some(0) => Ok(true),
                Some(126) | Some(127) => Ok(false),
                Some(code) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Err(Error::Other(anyhow::anyhow!(
                        "binary check exited with code {code}: {stderr}"
                    )))
                }
                None => Err(Error::Other(anyhow::anyhow!(
                    "binary check terminated by signal"
                ))),
            }
        })
    }

    fn check_has_old_binary(&self) -> Pin<Box<dyn Future<Output = anyhow::Result<bool>> + Send>> {
        let distro = self.distro.clone();
        Box::pin(async move {
            // As in the SSH transport: the install directory existing is
            // evidence of a prior install, which turns a would-be first-time
            // prompt into a silent auto-update.
            let cmd = format!("test -d {}", remote_server::setup::remote_server_dir());
            let output =
                run_wsl_command(&distro, &cmd, remote_server::setup::CHECK_TIMEOUT).await?;
            match output.status.code() {
                Some(0) => Ok(true),
                Some(1) => Ok(false),
                Some(code) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Err(anyhow::anyhow!(
                        "remote-server dir check exited with code {code}: {stderr}"
                    ))
                }
                None => Err(anyhow::anyhow!(
                    "remote-server dir check terminated by signal"
                )),
            }
        })
    }

    /// Runs the install script inside the distribution, then verifies.
    ///
    /// No SCP fallback, unlike the SSH transport: there is no network hop to
    /// fall back from. A distribution's filesystem is reachable directly —
    /// from Windows as `\\wsl.localhost\<distro>\…`, and from inside the distro
    /// as an ordinary path — so a failed download is better fixed by placing
    /// the file than by inventing a second transfer mechanism.
    ///
    /// On the OSS channel there is nothing to download in the first place:
    /// `remote_server_binary()` documents `Channel::Oss` as having "no
    /// release-pinned CDN artifact" and being "expected to be deployed/managed
    /// locally", so the binary is staged at the bare path by hand and
    /// [`Self::check_binary`] short-circuits this method entirely.
    fn install_binary(&self) -> Pin<Box<dyn Future<Output = InstallOutcome> + Send>> {
        let distro = self.distro.clone();
        Box::pin(async move {
            let binary_path = remote_server::setup::remote_server_binary();
            log::info!("Installing remote server binary to {binary_path} in WSL distro {distro}");

            let script = remote_server::setup::install_script(None);
            let result =
                match run_wsl_script(&distro, &script, remote_server::setup::INSTALL_TIMEOUT).await
                {
                    Ok(output) if output.status.success() => Ok(()),
                    Ok(output) => Err(Error::ScriptFailed {
                        exit_code: output.status.code().unwrap_or(-1),
                        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    }),
                    Err(e) => Err(e.into()),
                };

            let mut outcome = InstallOutcome {
                source: Some(InstallSource::Server),
                result,
            };

            // Post-install verification, for the same reason the SSH transport
            // does it: a silent install failure otherwise surfaces later as an
            // unreadable handshake error.
            if outcome.result.is_ok() {
                let check_cmd = remote_server::setup::binary_check_command();
                match run_wsl_command(&distro, &check_cmd, remote_server::setup::CHECK_TIMEOUT)
                    .await
                {
                    Ok(output) if output.status.success() => {}
                    Ok(output) => {
                        let code = output.status.code().unwrap_or(-1);
                        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                        outcome.result = Err(Error::Other(anyhow::anyhow!(
                            "Post-install verification failed: binary not found or not \
                             executable at {binary_path} (exit {code}): {stderr}"
                        )));
                    }
                    Err(e) => {
                        outcome.result = Err(Error::Other(anyhow::anyhow!(
                            "Post-install verification failed: {e}"
                        )));
                    }
                }
            }

            outcome
        })
    }

    fn connect(
        &self,
        executor: Arc<executor::Background>,
    ) -> Pin<Box<dyn Future<Output = Result<Connection>> + Send>> {
        let distro = self.distro.clone();
        let remote_proxy_command = self.remote_proxy_command();
        Box::pin(async move {
            // `kill_on_drop(true)` pairs with the `Child` returned in the
            // `Connection`: the manager holds it on per-session state, and
            // dropping that state kills this process.
            let mut child = command::r#async::Command::new(WSL_COMMAND)
                .args(wsl_args(&distro))
                .arg("sh")
                .arg("-c")
                .arg(&remote_proxy_command)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .kill_on_drop(true)
                .spawn()?;

            let stdin = child
                .stdin
                .take()
                .ok_or_else(|| anyhow::anyhow!("Failed to capture child stdin"))?;
            let stdout = child
                .stdout
                .take()
                .ok_or_else(|| anyhow::anyhow!("Failed to capture child stdout"))?;
            let stderr = child
                .stderr
                .take()
                .ok_or_else(|| anyhow::anyhow!("Failed to capture child stderr"))?;

            let (client, event_rx, failure_rx, host_response_rx, stderr_tail) =
                RemoteServerClient::from_child_streams(stdin, stdout, stderr, &executor);
            Ok(Connection {
                client,
                event_rx,
                failure_rx,
                host_response_rx,
                child,
                // No ControlMaster to tear down, so teardown has nothing to
                // decide. The enum documents this variant as "no ControlMaster
                // socket".
                control_path: ControlPath::None,
                stderr_tail,
            })
        })
    }

    fn remove_remote_server_binary(
        &self,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>> {
        let distro = self.distro.clone();
        Box::pin(async move {
            let cmd = remote_server::setup::remote_server_removal_command();
            log::info!("Removing stale remote server binary in WSL distro {distro}: {cmd}");
            let output =
                run_wsl_command(&distro, &cmd, remote_server::setup::CHECK_TIMEOUT).await?;
            if output.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(anyhow::anyhow!("Failed to remove binary: {stderr}"))
            }
        })
    }

    /// Always worth reconnecting.
    ///
    /// The SSH transport refuses after exit 255 because that means the
    /// ControlMaster's TCP connection is dead and every future command through
    /// it would fail the same way. WSL has no such shared state: each
    /// invocation is an independent local process against a running VM, so a
    /// previous failure says nothing about the next attempt. A distribution
    /// that has actually stopped will fail again quickly and cheaply.
    fn is_reconnectable(&self, _exit_status: Option<&RemoteServerExitStatus>) -> bool {
        true
    }
}

#[cfg(test)]
#[path = "wsl_transport_tests.rs"]
mod tests;

/// Starts a remote server for `session_id` inside `distro`.
///
/// Shared by `warpctrl remote wsl connect` and the command-palette action, so
/// the two cannot drift. Both arrive here having already resolved a session and
/// a distribution; everything after that — the auth context, the transport, the
/// manager call — is the same work, and duplicating it is how the two entry
/// points would end up connecting to subtly different daemons.
///
/// Generic over the calling model because every dependency it reaches for is a
/// singleton: the local-control bridge and `TerminalView` both satisfy it.
///
/// Returns once the setup pipeline has *started*.
/// [`RemoteServerManager::connect_session`] spawns the binary check, install
/// and handshake onto the background executor, so success is not knowable here
/// and is reported later as `RemoteServerManagerEvent`s.
pub fn start_wsl_remote_server<C>(
    session_id: warp_core::session_id::SessionId,
    distro: String,
    ctx: &mut C,
) where
    C: warpui::UpdateModel + warpui::ReadModel + warpui::GetSingletonModelHandle,
{
    use warpui::SingletonEntity as _;

    use crate::remote_server::auth_context::server_api_auth_context;

    let auth_state = crate::auth::AuthStateProvider::handle(ctx)
        .as_ref(ctx)
        .get()
        .clone();
    let auth_client = crate::server::server_api::ServerApiProvider::handle(ctx)
        .as_ref(ctx)
        .get_auth_client();
    let crash_reporting_enabled = crate::settings::PrivacySettings::handle(ctx)
        .as_ref(ctx)
        .is_crash_reporting_enabled;

    // Tolerates being logged out by design: `server_api_auth_context` yields no
    // bearer token when there is no account, and falls back to the anonymous id
    // for the identity key that partitions the daemon's socket. Nothing on this
    // path needs an account — verified by completing a credential-free
    // `Initialize` handshake against a daemon this binary spawned.
    let auth_context = Arc::new(server_api_auth_context(
        auth_state,
        auth_client,
        crash_reporting_enabled,
    ));

    let transport = WslTransport::new(distro.clone(), auth_context.clone());
    let label = format!("WSL: {distro}");
    log::info!("Connecting remote server to WSL distro {distro} for session {session_id:?}");
    remote_server::manager::RemoteServerManager::handle(ctx).update(ctx, |manager, ctx| {
        manager.connect_session(session_id, transport, auth_context, Some(label), ctx);
    });
}
