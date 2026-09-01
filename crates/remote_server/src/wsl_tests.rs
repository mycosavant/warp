use super::*;

/// The trailing `--` is the whole reason this helper exists rather than being
/// inlined: without it, `wsl.exe` swallows the remote command's flags as its
/// own, and a command like `uname -sm` fails in a way that looks like the
/// distro is broken.
#[test]
fn wsl_args_target_the_distro_and_end_the_flag_section() {
    assert_eq!(
        wsl_args("Ubuntu"),
        vec!["-d".to_owned(), "Ubuntu".to_owned(), "--".to_owned()]
    );
}

/// `wsl.exe -l -q` writes UTF-16LE, because the thing writing it is a Windows
/// program rather than a Linux process. Bytes taken verbatim from a hexdump of
/// the real command on 2026-08-22.
#[test]
fn distro_list_is_decoded_from_utf16le() {
    let real_output = b"\x55\x00\x62\x00\x75\x00\x6e\x00\x74\x00\x75\x00\x0d\x00\x0a\x00\
                        \x64\x00\x6f\x00\x63\x00\x6b\x00\x65\x00\x72\x00\x2d\x00\x64\x00\
                        \x65\x00\x73\x00\x6b\x00\x74\x00\x6f\x00\x70\x00\x0d\x00\x0a\x00";

    assert_eq!(
        parse_distro_list(real_output),
        vec!["Ubuntu".to_owned(), "docker-desktop".to_owned()]
    );
}

/// Reading the same bytes as UTF-8 is the failure this test pins against: every
/// name comes back interleaved with NULs and matches nothing.
#[test]
fn treating_the_distro_list_as_utf8_would_have_produced_garbage() {
    let real_output = b"\x55\x00\x62\x00\x75\x00\x6e\x00\x74\x00\x75\x00\x0d\x00\x0a\x00";

    let wrong = String::from_utf8_lossy(real_output);
    assert!(wrong.contains('\0'), "{wrong:?}");
    assert_ne!(wrong.trim(), "Ubuntu");

    assert_eq!(parse_distro_list(real_output), vec!["Ubuntu".to_owned()]);
}

/// A BOM is legal at the start of UTF-16 output and must not become part of the
/// first distribution's name, which would make it fail to match on the command
/// line.
#[test]
fn a_byte_order_mark_is_not_part_of_the_first_name() {
    let with_bom = b"\xff\xfe\x55\x00\x62\x00\x75\x00\x6e\x00\x74\x00\x75\x00";
    assert_eq!(parse_distro_list(with_bom), vec!["Ubuntu".to_owned()]);
}

/// A short read should shorten the list, not fail the picker.
#[test]
fn an_odd_trailing_byte_is_ignored_rather_than_failing() {
    let truncated = b"\x55\x00\x62\x00\x75\x00\x6e\x00\x74\x00\x75\x00\x0d";
    assert_eq!(parse_distro_list(truncated), vec!["Ubuntu".to_owned()]);
}

#[test]
fn an_empty_list_is_empty_rather_than_one_blank_name() {
    assert!(parse_distro_list(b"").is_empty());
    assert!(parse_distro_list(b"\x0d\x00\x0a\x00").is_empty());
}

/// Timeouts have to reach [`crate::transport::Error::TimedOut`] so the setup
/// banner can say "the operation timed out" instead of showing a raw I/O
/// error, which is what `SshCommandError` does and what the manager expects.
#[test]
fn timeouts_map_to_the_transport_timeout_variant() {
    let err: crate::transport::Error = WslCommandError::TimedOut {
        timeout: Duration::from_secs(3),
    }
    .into();
    assert!(matches!(err, crate::transport::Error::TimedOut));

    let err: crate::transport::Error =
        WslCommandError::SpawnFailed(std::io::Error::other("no wsl here")).into();
    assert!(matches!(err, crate::transport::Error::Other(_)));
}

/// **A machine with no `wsl.exe` gets the error written for it.**
///
/// `SpawnFailed`'s doc says it is what a caller sees on a machine without WSL.
/// That was true only for `run_wsl_script`, which spawns the child itself.
/// `run_wsl_command` goes through `output()`, which folds spawning and running
/// into one `io::Error`, and mapped everything to `IoError` — so the first thing
/// anyone without WSL hits (`detect_platform`, a `run_wsl_command` caller) never
/// produced the purpose-built error.
///
/// Both halves asserted, because a classifier that returned `SpawnFailed` for
/// everything would satisfy the first alone and lose the distinction the enum
/// exists to draw.
///
/// **What this does not pin, so nobody credits it with more: the call site.**
/// The bug was that `run_wsl_command` used the wrong mapper, and reverting
/// `wsl.rs:121` to `.map_err(WslCommandError::IoError)` leaves all 107 tests in
/// this crate green -- verified by running. What catches it is the compiler's
/// `function classify_spawn_failure is never used`, which is a real guard but
/// not this one. A hermetic test of the call site would need `WSL_COMMAND`
/// injectable, and on a machine with WSL interop `wsl.exe` is present, so a test
/// that spawned it would pass or fail on the environment rather than the code.
///
/// The other spawn site uses `.spawn()` and maps everything to `SpawnFailed`
/// unconditionally, which is correct rather than inconsistent: this classifier
/// exists because `.output()` folds spawning and running into one error, and
/// `.spawn()` does not fold them.
#[test]
fn a_missing_wsl_binary_is_a_spawn_failure_and_nothing_else_is() {
    let missing = classify_spawn_failure(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "program not found",
    ));
    assert!(
        matches!(missing, WslCommandError::SpawnFailed(_)),
        "a missing wsl.exe must say so: {missing}"
    );

    let broken_pipe = classify_spawn_failure(std::io::Error::new(
        std::io::ErrorKind::BrokenPipe,
        "pipe closed mid-read",
    ));
    assert!(
        matches!(broken_pipe, WslCommandError::IoError(_)),
        "a failure after the process started is not a spawn failure: {broken_pipe}"
    );
}
