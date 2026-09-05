use std::path::{Path, PathBuf};

use warp_util::host_id::HostId;

use super::{WslHosts, distro_of_root, wsl_unc_path};

#[test]
fn a_remote_buffer_is_spelled_the_way_the_fork_keys_wsl_directories() {
    let spelled = wsl_unc_path("Ubuntu", "/home/effatha/git/warp/src/main.rs");
    assert_eq!(
        spelled,
        PathBuf::from(r"\\wsl$\ubuntu\home\effatha\git\warp\src\main.rs")
    );
    // Pinned to the normal form every other map uses, so the path the buffer
    // model files a routed buffer under is one the manager and
    // `PersistedWorkspace` can already have as a key.
    assert_eq!(
        Some(spelled),
        warp_util::path::canonicalize_wsl_unc_path(Path::new(
            r"\\wsl.localhost\Ubuntu\home\effatha\git\warp\src\main.rs"
        ))
    );
}

#[test]
fn the_distribution_root_has_no_trailing_separator() {
    assert_eq!(wsl_unc_path("Ubuntu", "/"), PathBuf::from(r"\\wsl$\ubuntu"));
}

#[test]
fn only_a_wsl_unc_root_names_a_distribution() {
    assert_eq!(
        distro_of_root(Path::new(r"\\wsl$\Ubuntu\home\e\repo")),
        Some("Ubuntu".to_string())
    );
    assert_eq!(
        distro_of_root(Path::new(r"\\wsl.localhost\Debian\srv")),
        Some("Debian".to_string())
    );
    assert_eq!(distro_of_root(Path::new(r"C:\Users\e\repo")), None);
    assert_eq!(distro_of_root(Path::new("/home/e/repo")), None);
    assert_eq!(
        distro_of_root(Path::new(r"\\fileserver\share\repo")),
        None,
        "a UNC path to another machine is not a distribution"
    );
}

#[test]
fn hosts_are_looked_up_both_ways_and_distributions_compare_case_insensitively() {
    let mut hosts = WslHosts::default();
    let host = HostId::new("abc123".to_string());
    hosts.record(host.clone(), "Ubuntu".to_string());
    assert_eq!(hosts.distro_for(&host), Some("Ubuntu"));
    assert_eq!(hosts.host_for_distro("ubuntu"), Some(&host));
    assert_eq!(hosts.host_for_distro("UBUNTU"), Some(&host));
    assert_eq!(hosts.host_for_distro("Debian"), None);
    assert_eq!(hosts.distro_for(&HostId::new("other".to_string())), None);
}
