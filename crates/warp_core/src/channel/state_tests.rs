use std::path::PathBuf;

use super::{derive_http_origin_from_ws_url, read_version_sidecar, version_sidecar_path};

#[test]
fn wss_becomes_https_and_strips_path() {
    let got = derive_http_origin_from_ws_url("wss://rtc.app.warp.dev/graphql/v2");
    assert_eq!(got.as_deref(), Some("https://rtc.app.warp.dev"));
}

#[test]
fn ws_becomes_http_and_preserves_port() {
    let got = derive_http_origin_from_ws_url("ws://localhost:8080/graphql/v2");
    assert_eq!(got.as_deref(), Some("http://localhost:8080"));
}

#[test]
fn unparseable_input_returns_none() {
    assert!(derive_http_origin_from_ws_url("not a url").is_none());
    assert!(derive_http_origin_from_ws_url("https://app.warp.dev").is_none());
}

/// A scratch directory unique to one test, so tests beside each other cannot
/// read one another's sidecar.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("warp-core-sidecar-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn the_sidecar_sits_beside_the_binary_under_the_same_name_on_both_platforms() {
    assert_eq!(
        version_sidecar_path(std::path::Path::new("/t/release/warp-oss")),
        PathBuf::from("/t/release/warp-oss.version")
    );
    assert_eq!(
        version_sidecar_path(std::path::Path::new("C:/t/release/warp-oss.exe")),
        PathBuf::from("C:/t/release/warp-oss.version")
    );
}

#[test]
fn a_sidecar_names_the_build_and_only_its_first_line_counts() {
    let dir = scratch("first-line");
    let exe = dir.join("warp-oss");
    std::fs::write(
        version_sidecar_path(&exe),
        "v0.fork.ea61116e1\nsecond line\n",
    )
    .unwrap();
    assert_eq!(
        read_version_sidecar(&exe).as_deref(),
        Some("v0.fork.ea61116e1")
    );
    // Trailing whitespace and a Windows line ending are not part of a version.
    std::fs::write(version_sidecar_path(&exe), "  v0.fork.1a42ecdb8-dirty \r\n").unwrap();
    assert_eq!(
        read_version_sidecar(&exe).as_deref(),
        Some("v0.fork.1a42ecdb8-dirty")
    );
}

#[test]
fn no_sidecar_or_an_empty_one_means_no_version() {
    let dir = scratch("absent");
    let exe = dir.join("warp-oss");
    assert_eq!(read_version_sidecar(&exe), None);
    std::fs::write(version_sidecar_path(&exe), "\n\n").unwrap();
    assert_eq!(read_version_sidecar(&exe), None);
    std::fs::write(version_sidecar_path(&exe), "   \n").unwrap();
    assert_eq!(read_version_sidecar(&exe), None);
    // A line with whitespace inside it is not a version either.
    std::fs::write(version_sidecar_path(&exe), "v0.fork not a version\n").unwrap();
    assert_eq!(read_version_sidecar(&exe), None);
}
