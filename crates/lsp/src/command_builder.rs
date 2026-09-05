#[cfg(not(target_arch = "wasm32"))]
use command::r#async::Command;

/// A wrapper around `path_env_var` that produces correctly-configured commands.
///
/// This follows the same wrapping pattern as `command::r#async::Command`:
/// callers construct commands through the executor, which transparently sets
/// the PATH environment variable. On wasm, a dummy implementation is provided
/// so that consumer code doesn't need cfg gating.
#[derive(Clone)]
pub struct CommandBuilder {
    path_env_var: Option<String>,
    /// When set, every command runs inside this WSL distribution through
    /// `wsl.exe` instead of on this machine. See [`wsl_argv`].
    wsl_distro: Option<String>,
}

/// The `wsl.exe` arguments that run `program` inside `distro`.
///
/// `--shell-type login` is load-bearing: without it the child gets WSL's
/// system PATH and not the user's profile, so a server installed under
/// `~/.cargo/bin` or `~/.local/bin` is invisible. Measured 2026-09-05 on the
/// distribution this fork lives in: the default PATH had nothing under
/// `/home`, the login one had three entries there. `--` ends the flag section
/// so a program name starting with `-` cannot be read as one of `wsl.exe`'s.
pub fn wsl_argv(distro: &str, program: &std::ffi::OsStr) -> Vec<std::ffi::OsString> {
    vec![
        "-d".into(),
        distro.into(),
        "--shell-type".into(),
        "login".into(),
        "--".into(),
        program.to_os_string(),
    ]
}

impl CommandBuilder {
    /// Creates a new CommandBuilder with the given PATH environment variable.
    pub fn new(path_env_var: Option<String>) -> Self {
        Self {
            path_env_var,
            wsl_distro: None,
        }
    }

    /// Runs every command inside `distro` (`Some`) or on this machine (`None`).
    pub fn in_wsl_distro(mut self, distro: Option<String>) -> Self {
        self.wsl_distro = distro;
        self
    }

    /// The distribution commands run inside, if any.
    pub fn wsl_distro(&self) -> Option<&str> {
        self.wsl_distro.as_deref()
    }

    /// Returns the PATH environment variable, if set.
    pub fn path_env_var(&self) -> Option<&str> {
        self.path_env_var.as_deref()
    }

    /// Creates a new Command with PATH already set.
    ///
    /// Use this when you need to run a command. The returned Command has the
    /// same API as `command::r#async::Command`, so callers don't need to change
    /// how they construct commands.
    ///
    /// On Windows, the command is wrapped in `cmd.exe /c` so that `.cmd` and
    /// `.bat` scripts on PATH are resolved correctly (e.g. `npm.cmd`,
    /// `typescript-language-server.cmd`). Rust's `Command::new` uses
    /// `CreateProcessW` which only resolves `.exe` extensions.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn command(&self, program: impl AsRef<std::ffi::OsStr>) -> Command {
        if let Some(distro) = &self.wsl_distro {
            // No `cmd.exe` here: the program is a Linux binary resolved by the
            // distribution's login shell, and `.cmd` resolution is a Windows
            // concern. PATH is still set for `wsl.exe` itself.
            let mut cmd = Command::new("wsl.exe");
            cmd.args(wsl_argv(distro, program.as_ref()));
            if let Some(path) = &self.path_env_var {
                cmd.env("PATH", path);
            }
            return cmd;
        }
        #[cfg(windows)]
        let mut cmd = {
            let mut cmd = Command::new("cmd.exe");
            cmd.arg("/c").arg(program);
            cmd
        };
        #[cfg(not(windows))]
        let mut cmd = Command::new(program);
        if let Some(path) = &self.path_env_var {
            cmd.env("PATH", path);
        }
        cmd
    }
}

#[cfg(test)]
#[path = "command_builder_tests.rs"]
mod tests;
