//! Fork policy tests for the third egress enforcement point.
//!
//! Two of these assert behaviour and two assert *shape*, and the shape ones are
//! the point. A guard that covers the call sites in front of it today is a fact
//! about today; what makes it a fact about the code is something that counts
//! the ways around it. Same reasoning as
//! `the_symbol_map_leaves_by_exactly_one_call_site_and_it_is_guarded` in the
//! app crate, and as the `eventsource` bypass that `http_client::egress` found
//! in itself.

use super::*;

/// The names this crate uses to dial a socket. A crate that has one of these as
/// a dependency can open a WebSocket without passing the check.
const DIALLERS: &[&str] = &["async-tungstenite", "tokio-tungstenite", "ws_stream_wasm"];

fn crate_src() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// A host on either deny-list is refused before a socket is opened.
///
/// End to end through the public door rather than against `refuse_if_blocked`,
/// because the thing worth proving is the *wiring*: the guard function could be
/// perfect and never called, and that is exactly what `eventsource` was for a
/// year one crate over.
///
/// Skipped rather than failed when the operator has lifted the switch, since
/// the switches are process-global and this suite has no business insisting on
/// an environment. `each_deny_list_has_its_own_switch` in `egress_policy` holds
/// the switch rules purely, where no environment can reach them.
#[test]
fn a_listed_host_is_refused_before_the_socket() {
    let host = "app.warp.dev";
    if !egress_policy::blocks_host(host) {
        eprintln!("skipped: {host} is not blocked in this process; a switch is lifted");
        return;
    }

    let result = futures::executor::block_on(WebSocket::connect(
        "wss://app.warp.dev/graphql",
        std::iter::empty::<&str>(),
    ));

    let error = result
        .err()
        .expect("a WebSocket to a first-party host must be refused")
        .to_string();
    assert!(
        error.contains(host),
        "the refusal must name the host it refused: {error}"
    );
    assert!(
        error.contains(egress_policy::ALLOW_FIRST_PARTY_ENV_VAR),
        "and name the switch that lifts it, or someone spends an hour on a \
         connection error: {error}"
    );
}

/// The check has to let ordinary traffic through, and this is the half that can
/// actually fail.
///
/// Asserted against the guard rather than through `connect`, because the only
/// way to prove the allowed path end to end is to dial a real host, and a unit
/// test that opens a socket to the internet is worse than the gap it closes.
/// `a_listed_host_is_refused_before_the_socket` is what proves `connect` calls
/// this at all.
#[test]
fn an_unlisted_host_is_left_alone() {
    for host in ["api.anthropic.com", "localhost", "127.0.0.1", "github.com"] {
        assert!(
            refuse_if_blocked(Some(host)).is_ok(),
            "{host} is not on either deny-list and must not be refused"
        );
    }

    assert!(
        refuse_if_blocked(None).is_ok(),
        "a request with no host has nothing for a deny-list to judge"
    );
}

/// The socket is opened by exactly one function, and that function checks
/// first.
///
/// `mod imp` is private, so `imp::connect` is unreachable from outside this
/// crate and `WebSocket::connect` is the only door. Both facts are load-bearing
/// and neither is enforced by anything else: making `imp` public, or calling
/// `imp::connect` from a third place, compiles fine and silently opens a way
/// past the guard.
///
/// Reads the source text, which is crude and is the only thing that can see
/// this. A type system cannot express "and it was checked first".
#[test]
fn the_socket_is_opened_by_exactly_one_function() {
    let lib = std::fs::read_to_string(crate_src().join("lib.rs")).expect("lib.rs");

    assert!(
        lib.contains("\nmod imp;"),
        "`imp` must stay private: a `pub mod imp` hands every consumer a \
         dialler with no check in front of it"
    );

    // One arm per target family, each inside `WebSocket::connect`.
    let dials = lib.match_indices("imp::connect(").count();
    assert_eq!(
        dials, 2,
        "expected exactly two `imp::connect` call sites (native and wasm); a \
         third is a way to the network that skips `refuse_if_blocked`"
    );

    // Each dial is preceded by a *call* to the check, and nothing between them.
    //
    // The exclusion of `fn refuse_if_blocked(` is load-bearing and was found by
    // calibration: without it, the function's own definition sits above both
    // dials and satisfies every one of them, so deleting the guard from
    // `connect` left this test green. A structural test that cannot fail is
    // worse than no structural test, because it is cited.
    let checks: Vec<_> = lib
        .match_indices("refuse_if_blocked(")
        .map(|(i, _)| i)
        .filter(|i| !lib[..*i].ends_with("fn "))
        .collect();
    assert!(
        !checks.is_empty(),
        "no call to `refuse_if_blocked` left in lib.rs, only its definition"
    );
    let dial_positions: Vec<_> = lib.match_indices("imp::connect(").map(|(i, _)| i).collect();
    for dial in dial_positions {
        assert!(
            checks
                .iter()
                .any(|check| *check < dial && !lib[*check..dial].contains("imp::connect(")),
            "every `imp::connect` must be guarded by a `refuse_if_blocked` \
             above it in the same function"
        );
    }

    // And no other file in the crate reaches for it. Test files are skipped
    // because this one quotes `imp::connect` in its own assertion messages --
    // the same exclusion `the_symbol_map_leaves_by_exactly_one_call_site...`
    // makes, and for the same reason.
    for entry in std::fs::read_dir(crate_src()).expect("src/").flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name == "lib.rs" || !name.ends_with(".rs") || name.ends_with("_tests.rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            !text.contains("imp::connect("),
            "{name} dials without going through `WebSocket::connect`"
        );
    }
}

/// No other crate in the workspace can dial a WebSocket.
///
/// The guard above only covers sockets opened *through this crate*. A crate
/// that took `async-tungstenite` as a dependency of its own would reach the
/// network with nothing in front of it, would compile, and would pass every
/// other test here — the same shape as a second caller of `get_relevant_files`,
/// which is why that one is pinned by count too.
///
/// **`crates/graphql` is a listed exception, and it is the one this test found
/// on its first run.** It declares `ws_stream_wasm = "0.7"` directly, under
/// `cfg(target_family = "wasm")` (`crates/graphql/Cargo.toml:34`). Three things
/// make it an exception rather than a hole, and the third is what this test
/// keeps watching: it is wasm-only and the fork ships native binaries; its real
/// subscription path is `websocket::WebSocket::connect_with_headers`, which is
/// guarded; and **its own source never names the dialler**, which the assertion
/// below re-checks every run. Left in place rather than deleted because
/// removing a dependency from a target family nothing here builds is a change
/// that cannot be verified from this side.
///
/// **What this does not cover, stated so nobody credits it with more than it
/// does:** it pins the tungstenite family by name. Hand-rolling a WebSocket
/// upgrade over `hyper` and a TLS connector would pass it, and so would any
/// dialler nobody has thought of. The deny-list underneath has the same
/// property, and for the same reason: this is retroactive protection against
/// the doors that exist.
#[test]
fn a_websocket_is_dialled_from_this_crate_alone() {
    let root = workspace_root();
    let mut manifests = vec![root.join("app").join("Cargo.toml")];
    for entry in std::fs::read_dir(root.join("crates"))
        .expect("crates/")
        .flatten()
    {
        let manifest = entry.path().join("Cargo.toml");
        if manifest.is_file() {
            manifests.push(manifest);
        }
    }

    let mut dependents = Vec::new();
    for manifest in manifests {
        let Ok(text) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        // A dependency is a key at the start of a line. Matching anywhere in
        // the file would flag `crates/graphql`, which enables a `tungstenite`
        // *feature* of `graphql-ws-client` and never sees the crate itself.
        let declares = text.lines().any(|line| {
            let key = line.trim_start();
            DIALLERS.iter().any(|dialler| {
                key.starts_with(&format!("{dialler} ")) || key.starts_with(&format!("{dialler}="))
            })
        });
        if declares {
            let name = manifest
                .parent()
                .and_then(|dir| dir.file_name())
                .and_then(|name| name.to_str())
                .unwrap_or("?")
                .to_owned();
            dependents.push(name);
        }
    }
    dependents.sort();

    assert_eq!(
        dependents,
        vec!["graphql".to_owned(), "websocket".to_owned()],
        "a crate other than `websocket` can now dial a WebSocket directly, \
         with no egress check in front of it. Route it through \
         `websocket::WebSocket::connect`, or give it its own check and add it \
         here with the reason. `graphql` is the standing exception -- see this \
         test's docs."
    );

    // What makes `graphql` an exception rather than a hole: it declares the
    // dialler and never touches it. If that stops being true, this is a second
    // door and the exception above has to be argued again rather than
    // inherited.
    let mut stack = vec![root.join("crates").join("graphql").join("src")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            for dialler in DIALLERS {
                let in_source = dialler.replace('-', "_");
                assert!(
                    !text.contains(&in_source),
                    "{} names {in_source}: `graphql` is only an exception \
                     while it declares a dialler it never uses",
                    path.display()
                );
            }
        }
    }
}
