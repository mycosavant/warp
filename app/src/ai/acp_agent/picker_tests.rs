use super::*;
use crate::ai::llms::{LLMId, LLMInfo};

/// The list written at one launch is the list read at the next, and a file
/// that is not one is nothing rather than a panic.
#[test]
fn the_list_round_trips_through_the_store_and_garbage_is_nothing() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let path = dir.path().join("fork").join("acp-models.json");
    let list = AvailableLLMs::new(
        LLMId::from("sonnet".to_owned()),
        vec![
            LLMInfo::new_for_test("fable"),
            LLMInfo::new_for_test("sonnet"),
        ],
        None,
    )
    .expect("two choices");

    assert!(read_store(&path).is_none(), "no file is a fresh launch");
    write_store(&path, &list);
    assert_eq!(read_store(&path), Some(list), "the same list comes back");

    std::fs::write(&path, "not json").expect("overwrite");
    assert!(read_store(&path).is_none(), "garbage is a fresh launch too");
}
