use warpui::r#async::BoxFuture;

use super::*;

fn static_auth_context() -> Arc<RemoteServerAuthContext> {
    Arc::new(RemoteServerAuthContext::new(
        || -> BoxFuture<'static, Option<String>> { Box::pin(async { None }) },
        || "user id/with spaces".to_string(),
        String::new(),
        String::new(),
        true,
    ))
}

/// The identity key reaches the distribution through a shell, so an
/// unquoted one with a space in it would silently become two arguments and
/// the daemon would partition its socket under the wrong name. Mirrors
/// `ssh_transport_tests::remote_proxy_command_quotes_identity_key`, because
/// the hazard is the shell rather than the channel.
#[test]
fn remote_proxy_command_quotes_identity_key() {
    let transport = WslTransport::new("Ubuntu", static_auth_context());

    let command = transport.remote_proxy_command();

    assert!(command.contains("remote-server-proxy --identity-key"));
    assert!(command.contains("'user id/with spaces'"));
}

/// The proxy command names the server binary and the session's identity,
/// neither of which has anything to do with how the bytes travel. It must
/// therefore be byte-identical to the SSH transport's, or a session would
/// attach to a different daemon depending on how it was reached.
///
/// Asserted against the shared helper rather than against `SshTransport`
/// directly, because `SshTransport::remote_proxy_command` is private and
/// widening it for a test would be an upstream edit this fork does not need.
/// The twin lives at `ssh_transport.rs:89` and any change there should change
/// this.
#[test]
fn the_proxy_command_is_built_from_shared_inputs_only() {
    let transport = WslTransport::new("Ubuntu", static_auth_context());

    let expected = format!(
        "{} remote-server-proxy --identity-key {}",
        remote_server::setup::remote_server_binary(),
        shell_words::quote("user id/with spaces"),
    );

    assert_eq!(transport.remote_proxy_command(), expected);
    // Nothing distro-specific may leak into it.
    assert!(!transport.remote_proxy_command().contains("Ubuntu"));
}

/// Unlike SSH, there is no shared connection state that a previous failure
/// could have poisoned — each `wsl.exe` invocation is an independent local
/// process against a running VM. SSH refuses to retry after exit 255 because
/// its ControlMaster is dead; there is no equivalent here, and refusing would
/// strand a session that a single slow start had upset.
#[test]
fn reconnect_is_always_worth_attempting() {
    let transport = WslTransport::new("Ubuntu", static_auth_context());

    assert!(transport.is_reconnectable(None));
    assert!(transport.is_reconnectable(Some(&RemoteServerExitStatus {
        code: Some(255),
        signal_killed: false,
    })));
    assert!(transport.is_reconnectable(Some(&RemoteServerExitStatus {
        code: None,
        signal_killed: true,
    })));
}

/// `Debug` is written by hand on both transports so that an auth context
/// never reaches a log through a derived impl. The distribution name is
/// safe and useful; nothing else on the struct is.
#[test]
fn debug_shows_the_distro_and_not_the_auth_context() {
    let rendered = format!("{:?}", WslTransport::new("Ubuntu", static_auth_context()));

    assert!(rendered.contains("Ubuntu"), "{rendered}");
    assert!(!rendered.contains("user id"), "{rendered}");
}

/// Drives the real transport against a real distribution.
///
/// Everything above this is shape-checking, and this fork's standard is to
/// verify by running (`../CLAUDE.md`). `#[ignore]` because it needs `wsl.exe`
/// and at least one installed distribution:
///
/// ```text
/// cargo test -p warp --lib --features gui,warp_control_cli \
///     wsl_transport::tests::against_a_real_distro -- --ignored --nocapture
/// ```
///
/// Runnable from a Linux checkout as well as from Windows: WSL interop puts
/// `wsl.exe` on `PATH` inside a distribution, so a Linux build can drive a
/// distribution through the same code path a Windows client would.
#[test]
#[ignore = "requires wsl.exe and an installed distribution"]
fn against_a_real_distro() {
    use futures::executor::block_on;

    let distros = block_on(remote_server::wsl::list_distros(
        std::time::Duration::from_secs(30),
    ));
    assert!(
        !distros.is_empty(),
        "no WSL distributions found — is wsl.exe on PATH?"
    );
    println!("distributions: {distros:?}");

    let distro = distros
        .iter()
        .find(|d| *d != "docker-desktop")
        .unwrap_or(&distros[0]);
    println!("using: {distro}");

    let transport = WslTransport::new(distro.clone(), static_auth_context());

    // `uname -sm` inside the distribution, parsed by the same
    // `parse_uname_output` the SSH transport uses.
    let platform = block_on(transport.detect_platform()).expect("detect_platform");
    println!("platform: {platform:?}");

    // `<binary> --version`: `Ok(false)` when nothing is staged at the bare OSS
    // path, `Ok(true)` once it is. Either answer proves the command reached the
    // distribution and its exit code was understood; only `Err` would mean the
    // transport failed.
    let has_binary = block_on(transport.check_binary()).expect("check_binary");
    println!("remote server binary present: {has_binary}");

    // `test -d ~/.warp-*/remote-server`.
    let has_old = block_on(transport.check_has_old_binary()).expect("check_has_old_binary");
    println!("prior install directory present: {has_old}");
}
