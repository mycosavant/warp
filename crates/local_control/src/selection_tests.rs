use chrono::Utc;

use super::*;
use crate::discovery::{ControlEndpoint, CredentialBrokerReference};
use crate::protocol::{ActionKind, PROTOCOL_VERSION};

fn record(id: &str, pid: u32) -> InstanceRecord {
    InstanceRecord {
        protocol_version: PROTOCOL_VERSION,
        instance_id: InstanceId(id.to_owned()),
        pid,
        channel: "local".to_owned(),
        app_id: "dev.warp.WarpLocal".to_owned(),
        app_version: None,
        started_at: Utc::now(),
        executable_path: None,
        endpoint: Some(ControlEndpoint::localhost(4000)),
        credential_broker: Some(CredentialBrokerReference {
            socket_path: format!("{id}.broker.sock").into(),
        }),
        actions: vec![ActionKind::TabCreate.metadata()],
    }
}

#[test]
fn selects_instance_by_id() {
    let records = vec![record("one", 1), record("two", 2)];
    let selected = select_instance(
        &records,
        &InstanceSelector::Id(InstanceId("two".into())),
        &searched(),
    )
    .expect("selected");
    assert_eq!(selected.pid, 2);
}

#[test]
fn active_selector_rejects_ambiguity() {
    let records = vec![record("one", 1), record("two", 2)];
    let err =
        select_instance(&records, &InstanceSelector::Active, &searched()).expect_err("ambiguous");
    assert_eq!(err.code, ErrorCode::AmbiguousInstance);
}

#[test]
fn active_selector_rejects_no_instances() {
    let err =
        select_instance(&[], &InstanceSelector::Active, &searched()).expect_err("no instance");
    assert_eq!(err.code, ErrorCode::NoInstance);
}

/// …and it says **where** it looked, because the directory is the one fact that
/// separates "nothing is running" from "something is running in the other
/// registry".
///
/// `discovery_dir()` answers `$XDG_RUNTIME_DIR/warp/local-control` when that
/// variable is set and `$HOME/.warp/local-control` when it is not, so a GUI
/// launched from an environment without it is invisible to a `warpctrl` run
/// from one with it. Measured 2026-09-10: a record and broker socket with a
/// dead pid had sat in the second for thirteen days, and one scan aimed there
/// removed both. See `.fork/runs/discovery-2026-09-10/`.
#[test]
fn the_empty_answer_names_the_directory_it_looked_in() {
    let err =
        select_instance(&[], &InstanceSelector::Active, &searched()).expect_err("no instance");

    assert!(
        err.message.contains("/run/user/1000/warp/local-control"),
        "got: {}",
        err.message
    );
    assert!(
        err.message.contains("XDG_RUNTIME_DIR"),
        "the variable is the mechanism, and a person can check it, got: {}",
        err.message
    );
}

/// The directory a test pretends to have scanned. A literal rather than a call
/// to `discovery_dir()`, so this file never depends on the environment it runs
/// in — which is the failure it is about.
fn searched() -> std::path::PathBuf {
    std::path::PathBuf::from("/run/user/1000/warp/local-control")
}
