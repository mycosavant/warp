use local_control::protocol::MainPaneResult;

/// The three actions all answer with post-call state, so a caller never has to
/// follow `set` with `get`. That only works if the shape is identical across
/// them — a `set` that reported a different field set would force callers to
/// special-case it.
#[test]
fn the_result_serializes_one_shape_for_all_three_actions() {
    let designated = serde_json::to_value(MainPaneResult {
        main_pane_id: Some("Pane Pane Terminal (12307)".to_owned()),
        main_pane_index: Some(1),
        anchors_working_directory: true,
    })
    .expect("serializes");

    assert_eq!(
        designated,
        serde_json::json!({
            "main_pane_id": "Pane Pane Terminal (12307)",
            "main_pane_index": 1,
            "anchors_working_directory": true,
        })
    );

    let cleared = serde_json::to_value(MainPaneResult {
        main_pane_id: None,
        main_pane_index: None,
        anchors_working_directory: false,
    })
    .expect("serializes");

    assert_eq!(
        cleared.as_object().expect("object").keys().count(),
        designated.as_object().expect("object").keys().count(),
        "cleared and designated results must carry the same fields",
    );
    assert_eq!(cleared["main_pane_id"], serde_json::Value::Null);
}

/// A main pane that is not a terminal is a legal state, not an error, and it is
/// distinguishable from having no main pane at all. The difference matters:
/// with no main pane the working directory follows the active pane, and with a
/// non-terminal one it follows nothing — which is the point, since following
/// nothing is what stops the file tree thrashing.
#[test]
fn a_non_terminal_main_pane_is_distinguishable_from_no_main_pane() {
    let editor_is_main = serde_json::to_value(MainPaneResult {
        main_pane_id: Some("Pane Pane Editor (900)".to_owned()),
        main_pane_index: Some(0),
        anchors_working_directory: false,
    })
    .expect("serializes");

    let nothing_is_main = serde_json::to_value(MainPaneResult {
        main_pane_id: None,
        main_pane_index: None,
        anchors_working_directory: false,
    })
    .expect("serializes");

    assert_eq!(
        editor_is_main["anchors_working_directory"], nothing_is_main["anchors_working_directory"],
        "both anchor nothing",
    );
    assert_ne!(
        editor_is_main, nothing_is_main,
        "but a caller must still be able to tell them apart",
    );
}

/// `main_pane_index` is only meaningful alongside an id, and the two are
/// produced from the same lookup. Emitting an index with a null id would invite
/// a caller to index into `pane list` with it.
#[test]
fn an_index_never_appears_without_an_id() {
    let value = serde_json::to_value(MainPaneResult {
        main_pane_id: None,
        main_pane_index: None,
        anchors_working_directory: false,
    })
    .expect("serializes");

    assert!(value["main_pane_id"].is_null());
    assert!(value["main_pane_index"].is_null());
}

// The inverse — an id with no index — is the shape a real bug produced, and it
// cannot be pinned here: it needs a live `PaneGroup` with a closed pane in it.
// It is covered by
// `pane_group::tests::test_main_pane_designation_does_not_survive_closing_that_pane`,
// which is where the two notions of "this pane still exists" actually meet.
