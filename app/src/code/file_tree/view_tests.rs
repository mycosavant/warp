/// How many places in the project explorer turn a tree path into a `PathBuf`
/// this process can hand to `std::fs`, and why each one is allowed to.
///
/// **T16 phase 3.** `StandardizedPath::to_local_path_lossy` will happily turn
/// `/home/you/repo/src` into a path `std::fs` accepts, and this view's
/// operations include `std::fs::remove_dir_all`, `std::fs::rename` and
/// `std::fs::File::create_new`. Once phase 1 made a WSL repository's tree
/// remote, the only thing between those and the wrong machine was seven
/// separate `if !self.is_remote_item(id)` guards at the dispatch site --
/// correct today, and one forgotten `if` on a new action away from deleting a
/// directory on whichever machine Warp happens to be running on.
///
/// The four in `view.rs`:
///   1. `local_path_for` -- the guarded helper every filesystem operation now
///      takes its path from; returns `None` for a remote item.
///   2. `set_root_directories`, on a root built with `remote_host_id: None`.
///   3. `select_and_execute_item_at_id`, in the `else` of `if is_remote`.
///   4. `context_menu_items`, in the `else` of `if is_remote`.
///
/// The three in `view/editing.rs` are the create path and the two halves of
/// the rename, all after `commit_pending_edit`'s early return for a remote
/// item.
///
/// Counted rather than pinned by line, because a guard that fails on unrelated
/// edits is a guard that gets deleted. Adding a conversion fails this test;
/// moving one does not. The number is in the test's own name, so it goes stale
/// on exactly the schedule that makes it visible -- read it off the assertion,
/// never off this paragraph.
#[test]
fn the_project_explorer_converts_a_path_for_std_fs_in_exactly_seven_places() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/code/file_tree");
    let mut found = Vec::new();
    for rel in ["view.rs", "view/editing.rs"] {
        let text = std::fs::read_to_string(dir.join(rel)).expect("file tree source is readable");
        for (idx, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                continue;
            }
            if line.contains("to_local_path_lossy") {
                found.push(format!("{rel}:{}", idx + 1));
            }
        }
    }

    assert_eq!(
        found.len(),
        7,
        "the number of local-path conversions in the project explorer changed: {found:?}.\n\
         This view deletes, renames and creates files, and a tree path may belong to a \
         remote host that the conversion knows nothing about. Take the path from \
         `local_path_for`, which returns `None` for a remote item. If a new site is \
         genuinely inside an `is_remote` branch already, update this count and say which \
         branch in the doc above.",
    );
}
