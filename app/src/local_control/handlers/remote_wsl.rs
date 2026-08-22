//! `remote.wsl.list` — which WSL distributions this machine has
//! (`.fork/IDEAS.md`, I16).
//!
//! # Why this action exists before any UI
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

use futures::executor::block_on;
use local_control::protocol::{ControlError, ErrorCode, RemoteWslDistroListResult};
use serde::Serialize;
use warpui::ModelContext;

use crate::local_control::bridge::LocalControlBridge;

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
