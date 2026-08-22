//! `remote.wsl.list` and `remote.wsl.connect` — reaching a WSL distribution
//! with Warp's remote-development server (`.fork/IDEAS.md`, I16).
//!
//! # Why these actions exist before any UI
//!
//! Warp's remote-development stack shipped with one transport, SSH, and it is
//! only ever reached ambiently: warpify notices a submitted `ssh` command and
//! drives `RemoteServerController` from the resulting `InitSubshell` hook. A
//! WSL connection has no equivalent trigger and cannot have one — there is no
//! command a user types that means "attach a remote server to my distro". Zed
//! solves this with an explicit "Add WSL Distro" entry under Open Remote.
//!
//! Whatever that entry eventually looks like here, it needs a list to show,
//! and something has to answer "is this machine even a candidate" before the
//! entry is worth rendering. That is this action. Exposing it through local
//! control rather than only through a picker also keeps it drivable by an
//! agent, which is the fork's orchestration story, and testable from outside
//! the GUI, which is how most of this fork's findings arrived.
//!
//! # Blocking, deliberately
//!
//! [`remote_server::wsl::list_distros`] spawns `wsl.exe` and awaits it, and
//! this handler runs on the main thread. It blocks, with a short timeout, for
//! the same reason `drive_sync::export` does its file I/O inline: the local
//! control bridge answers one request at a time and has no mechanism for
//! deferring a reply. `wsl.exe -l -q` is a local process listing local state
//! and returns in tens of milliseconds; [`LIST_TIMEOUT`] bounds the case where
//! WSL is installed but wedged.

use std::time::Duration;

use ::local_control::ActionKind;
use ::local_control::protocol::{
    ControlError, ErrorCode, RemoteWslConnectParams, RemoteWslConnectStartedResult,
    RemoteWslDistroListResult, TargetSelector,
};
use futures::executor::block_on;
use serde::Serialize;
use warpui::ModelContext;

use crate::local_control::bridge::LocalControlBridge;
use crate::local_control::resolver::{input_target_pane_id, target_pane_group};
use crate::remote_server::wsl_transport::start_wsl_remote_server;

/// Upper bound on how long the main thread will wait for `wsl.exe -l -q`.
///
/// Generous next to the tens of milliseconds the command actually takes,
/// because the cost of being wrong in the other direction is reporting "no WSL"
/// on a machine that has it. A wedged WSL service is the case this bounds.
const LIST_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) fn list(
    _ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    let distros = block_on(remote_server::wsl::list_distros(LIST_TIMEOUT));

    // `list_distros` collapses "no wsl.exe", "wsl.exe failed" and "timed out"
    // into an empty list, because to a caller deciding whether to offer WSL at
    // all they mean the same thing. Reporting `available` separately keeps the
    // *other* distinction — WSL present with nothing installed — visible, which
    // is a state a picker should explain rather than hide.
    to_control_data(RemoteWslDistroListResult {
        available: !distros.is_empty(),
        distros,
    })
}

/// Attaches a remote server to the targeted pane's terminal session, running
/// inside a WSL distribution.
///
/// # Why a session and not a window
///
/// The SSH transport attaches to the pane running `ssh`, because that pane's
/// shell *is* the remote one. WSL is the same relationship: the useful thing is
/// a remote server serving the distro your shell is already in, so file
/// browsing, code review and command generation happen on the Linux side of the
/// 9p boundary rather than across it. So this is session-scoped, and the
/// default distribution is the pane's own.
///
/// # Why this returns "started"
///
/// [`RemoteServerManager::connect_session`] advances the setup pipeline and
/// spawns the rest onto the background executor. It cannot report success,
/// because success has not happened yet — binary check, install and handshake
/// all follow. A reply here means the pipeline began; the outcome arrives as
/// `RemoteServerManagerEvent`s, and the observable proof is a
/// `remote-server-daemon` process and a socket under `remote_server_dir()`.
pub(crate) fn connect(
    params: &serde_json::Value,
    target: &TargetSelector,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    let action = ActionKind::RemoteWslConnect;
    let params: RemoteWslConnectParams = serde_json::from_value(params.clone()).map_err(|err| {
        ControlError::with_details(
            ErrorCode::InvalidParams,
            "invalid parameters",
            err.to_string(),
        )
    })?;

    let pane_group = target_pane_group(action, target, ctx)?;
    let pane_id = input_target_pane_id(action, target, &pane_group, ctx)?;
    let terminal_view = pane_group
        .read(ctx, |pane_group, ctx| {
            pane_group.terminal_view_from_pane_id(pane_id, ctx)
        })
        .ok_or_else(|| {
            ControlError::new(
                ErrorCode::MissingTarget,
                format!("{} requires a terminal session target", action.as_str()),
            )
        })?;

    // `active_block_session_id` is the session the pane's most recent block
    // belongs to, which is the same session a command typed there would join.
    // `None` means the pane has not bootstrapped a shell yet.
    let (session_id, pane_distro) = terminal_view.read(ctx, |view, ctx| {
        (
            view.active_block_session_id(),
            view.active_session_wsl_distro(ctx),
        )
    });
    let session_id = session_id.ok_or_else(|| {
        ControlError::new(
            ErrorCode::StaleTarget,
            format!(
                "{} found no bootstrapped terminal session in the target pane",
                action.as_str()
            ),
        )
    })?;

    let distro_from_pane = params.distro.is_none();
    let distro = params.distro.or(pane_distro).ok_or_else(|| {
        ControlError::with_details(
            ErrorCode::InvalidParams,
            "no WSL distribution to connect to",
            "the target pane is not running a WSL shell, so --distro is required; \
             `remote wsl list` reports what is installed"
                .to_owned(),
        )
    })?;

    // Shared with the command-palette action so the two entry points cannot
    // connect to subtly different daemons.
    start_wsl_remote_server(session_id, distro.clone(), ctx);

    to_control_data(RemoteWslConnectStartedResult {
        session_id: session_id.as_u64(),
        distro,
        distro_from_pane,
    })
}

fn to_control_data<T: Serialize>(value: T) -> Result<serde_json::Value, ControlError> {
    serde_json::to_value(value).map_err(|err| {
        ControlError::with_details(
            ErrorCode::Internal,
            "failed to serialize local-control response",
            err.to_string(),
        )
    })
}

#[cfg(test)]
#[path = "remote_wsl_tests.rs"]
mod tests;
