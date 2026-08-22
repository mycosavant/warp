//! Running commands inside a WSL distribution, for [`WslTransport`].
//!
//! # Why this exists alongside `ssh.rs`
//!
//! [`crate::transport::RemoteTransport`] shipped with exactly one
//! implementation, `SshTransport`, and the trait's own doc comment says it is
//! boxed "so implementations can be stored as `Arc<dyn RemoteTransport>`". This
//! is the second implementation's command layer.
//!
//! Reading `SshTransport::connect` is what makes the shape obvious: it spawns
//! `ssh` as a child with piped stdin/stdout/stderr and hands the three pipes to
//! `RemoteServerClient::from_child_streams`. **SSH is a pipe.** Nothing below
//! the transport knows or cares what spawned it, so a WSL transport is the same
//! protocol over `wsl.exe` instead — which is also, independently, how Zed
//! reaches a distro.
//!
//! WSL is the *easier* case. There is no ControlMaster to create, share or tear
//! down, no socket lifecycle, no re-authentication, and no credential at all:
//! `wsl.exe` runs as the same Windows user and lands in the distro as that
//! user's default Linux account. `ControlPath::None` already exists for exactly
//! this — the enum documents it as "No ControlMaster socket".
//!
//! # The two encodings, measured
//!
//! `wsl.exe` speaks two different encodings depending on which side is talking,
//! and mixing them up produces strings full of NUL bytes that compare equal to
//! nothing:
//!
//! | invocation | who writes stdout | encoding |
//! |---|---|---|
//! | `wsl.exe -l -q` | `wsl.exe` itself (a Windows program) | **UTF-16LE, CRLF** |
//! | `wsl.exe -d X -- cmd` | the Linux process | **passthrough, usually UTF-8/LF** |
//!
//! Verified 2026-08-22 by hexdump: the distro list came back
//! `55 00 62 00 75 00 6e 00 74 00 75 00 0d 00 0a 00` — `U\0b\0u\0n\0t\0u\0\r\0\n\0` —
//! while `uname -sm` in the same distro came back `4c 69 6e 75 78 …`, plain
//! `Linux x86_64\n`. So [`parse_distro_list`] decodes UTF-16LE and the command
//! runners do not decode anything.

use std::process::Output;
use std::time::Duration;

use command::r#async::Command;
use warpui_core::r#async::FutureExt as _;

/// The Windows binary that fronts every distribution.
///
/// Spelled with the `.exe` suffix on purpose. On Windows it resolves through
/// `PATH` normally; from *inside* a distro it resolves through WSL interop,
/// which is what makes this transport testable from a Linux checkout.
pub const WSL_COMMAND: &str = "wsl.exe";

/// Transport-level error from [`run_wsl_command`] or [`run_wsl_script`].
///
/// Mirrors [`crate::ssh::SshCommandError`] so both transports promote timeouts
/// to [`crate::transport::Error::TimedOut`] and everything else to `Other`.
#[derive(Debug, thiserror::Error)]
pub enum WslCommandError {
    /// The command or script did not complete within the timeout.
    #[error("Timed out after {timeout:?}")]
    TimedOut { timeout: Duration },
    /// The `wsl.exe` process could not be spawned. On a machine without WSL
    /// this is what a caller sees.
    #[error("Failed to spawn {WSL_COMMAND}: {0}")]
    SpawnFailed(std::io::Error),
    /// Writing the script to stdin failed.
    #[error("Failed to write to {WSL_COMMAND} stdin: {0}")]
    StdinWriteFailed(std::io::Error),
    /// The process was spawned but `output()` returned an I/O error.
    #[error("WSL I/O error: {0}")]
    IoError(std::io::Error),
}

impl From<WslCommandError> for crate::transport::Error {
    fn from(err: WslCommandError) -> Self {
        match err {
            WslCommandError::TimedOut { .. } => Self::TimedOut,
            other => Self::Other(other.into()),
        }
    }
}

/// Builds the argument list that targets `distro` and ends the flag section.
///
/// The trailing `--` is load-bearing: without it `wsl.exe` parses the remote
/// command's own flags as its own. `sh -c` is then the shell that runs it,
/// which matters because every command this transport sends contains a `~` that
/// something has to expand — `remote_server_dir()` returns a literal
/// `~/.warp-dev/remote-server`.
///
/// `sh -c` rather than `sh -lc`: SSH runs commands through the user's login
/// shell, but a login shell here would source a profile on every one-shot check
/// for no benefit, and Warp's own shell bootstrap is in those profiles. Tilde
/// expansion does not need it.
pub fn wsl_args(distro: &str) -> Vec<String> {
    vec!["-d".to_owned(), distro.to_owned(), "--".to_owned()]
}

/// Runs one command inside `distro` and returns its output, where:
/// - `Err` is a transport-level failure (`wsl.exe` missing, spawn failed, timeout).
/// - `Ok(output)` means the command ran; callers inspect `output.status` to tell
///   a successful run from a non-zero exit inside the distro.
pub async fn run_wsl_command(
    distro: &str,
    remote_command: &str,
    timeout: Duration,
) -> Result<Output, WslCommandError> {
    async {
        Command::new(WSL_COMMAND)
            .args(wsl_args(distro))
            .arg("sh")
            .arg("-c")
            .arg(remote_command)
            .kill_on_drop(true)
            .output()
            .await
    }
    .with_timeout(timeout)
    .await
    .map_err(|_| WslCommandError::TimedOut { timeout })?
    .map_err(WslCommandError::IoError)
}

/// Pipes a script into `bash -s` inside `distro`.
///
/// Same reasoning as [`crate::ssh::run_ssh_script`]: the preinstall and install
/// scripts are multi-line and full of shell constructs, so passing them as an
/// argument would need fragile escaping and would run into argument length
/// limits. stdin has neither problem.
pub async fn run_wsl_script(
    distro: &str,
    script: &str,
    timeout: Duration,
) -> Result<Output, WslCommandError> {
    use std::process::Stdio;

    let mut child = Command::new(WSL_COMMAND)
        .args(wsl_args(distro))
        .arg("bash")
        .arg("-s")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(WslCommandError::SpawnFailed)?;

    if let Some(mut stdin) = child.stdin.take() {
        use futures_lite::io::AsyncWriteExt;
        stdin
            .write_all(script.as_bytes())
            .await
            .map_err(WslCommandError::StdinWriteFailed)?;
        // Close stdin so the remote bash exits after reading the script.
        drop(stdin);
    }

    child
        .output()
        .with_timeout(timeout)
        .await
        .map_err(|_| WslCommandError::TimedOut { timeout })?
        .map_err(WslCommandError::IoError)
}

/// Decodes the UTF-16LE output of `wsl.exe -l -q` into distribution names.
///
/// Separate from [`list_distros`] so the decoding — the part with a trap in it —
/// is testable without a Windows machine or a WSL install.
///
/// Odd trailing bytes are ignored rather than erroring: a truncated read is
/// better reported as a short list than as a hard failure in a picker.
pub fn parse_distro_list(bytes: &[u8]) -> Vec<String> {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();

    String::from_utf16_lossy(&units)
        .lines()
        .map(|line| line.trim().trim_matches('\u{feff}').to_owned())
        .filter(|line| !line.is_empty())
        .collect()
}

/// Lists the installed WSL distributions, most-recently-default first.
///
/// Returns an empty list rather than an error when `wsl.exe` is absent, since
/// "no distros" and "no WSL" mean the same thing to a caller deciding whether
/// to offer this transport at all.
pub async fn list_distros(timeout: Duration) -> Vec<String> {
    let output = async {
        Command::new(WSL_COMMAND)
            .arg("-l")
            .arg("-q")
            .kill_on_drop(true)
            .output()
            .await
    }
    .with_timeout(timeout)
    .await;

    match output {
        Ok(Ok(output)) if output.status.success() => parse_distro_list(&output.stdout),
        _ => Vec::new(),
    }
}

#[cfg(test)]
#[path = "wsl_tests.rs"]
mod tests;
