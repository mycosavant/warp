use std::ffi::OsStr;

use super::wsl_argv;

#[test]
fn a_distro_command_is_a_login_shell_with_the_flags_closed() {
    let argv = wsl_argv("Ubuntu", OsStr::new("rust-analyzer"));
    assert_eq!(
        argv,
        [
            "-d",
            "Ubuntu",
            "--shell-type",
            "login",
            "--",
            "rust-analyzer"
        ]
    );
}

#[test]
fn the_program_is_the_last_argument_so_server_flags_follow_it() {
    // `pyright-langserver --stdio`: the caller's `.args()` land after this.
    let argv = wsl_argv("Debian", OsStr::new("pyright-langserver"));
    assert_eq!(argv.last().unwrap(), "pyright-langserver");
}
