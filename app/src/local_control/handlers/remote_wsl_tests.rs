use local_control::protocol::RemoteWslDistroListResult;

/// `available` and a non-empty `distros` are not the same claim, and the
/// serialized shape has to keep them apart. A caller deciding whether to offer
/// a WSL option needs "no WSL on this machine" and "WSL with nothing
/// installed" to look different; an empty list alone cannot say which.
#[test]
fn availability_is_reported_separately_from_the_list() {
    let no_wsl = RemoteWslDistroListResult {
        available: false,
        distros: vec![],
    };
    let wsl_but_empty = RemoteWslDistroListResult {
        available: true,
        distros: vec![],
    };

    assert_ne!(
        serde_json::to_value(&no_wsl).unwrap(),
        serde_json::to_value(&wsl_but_empty).unwrap(),
    );
}

/// The result is what a picker renders, so the field names are a contract.
#[test]
fn the_result_serializes_the_shape_a_picker_reads() {
    let value = serde_json::to_value(RemoteWslDistroListResult {
        available: true,
        distros: vec!["Ubuntu".to_owned(), "docker-desktop".to_owned()],
    })
    .expect("serializes");

    assert_eq!(
        value,
        serde_json::json!({
            "available": true,
            "distros": ["Ubuntu", "docker-desktop"],
        })
    );
}

/// Order is `wsl.exe -l -q`'s order, which puts the default distribution
/// first. A picker that pre-selects the first entry depends on this, so it must
/// not be sorted on the way through.
#[test]
fn distro_order_is_preserved() {
    let value = serde_json::to_value(RemoteWslDistroListResult {
        available: true,
        distros: vec!["zulu".to_owned(), "Ubuntu".to_owned(), "alpha".to_owned()],
    })
    .expect("serializes");

    assert_eq!(
        value["distros"],
        serde_json::json!(["zulu", "Ubuntu", "alpha"])
    );
}
