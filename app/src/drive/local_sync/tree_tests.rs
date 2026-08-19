use serde_json::json;
use tempfile::TempDir;

use super::*;
use crate::auth::UserUid;
use crate::cloud_object::{ObjectType, Owner};
use crate::server::ids::ClientId;

/// T4.4c. The whole point of the two previous pieces: a drive written to a
/// directory and read back is the same drive, identities and hierarchy
/// included.
///
/// Upstream's export/import cannot do this — it serializes `model().data` and
/// nothing else, so a round trip through it mints new ids and loses the object
/// graph. That is why T4.5's premise was wrong and why this replaced it.
#[test]
fn a_whole_drive_survives_a_round_trip() {
    let root = TempDir::new().unwrap();
    let drive = a_drive_with_nested_folders();

    export(root.path(), &drive).unwrap();
    let imported = import(root.path()).unwrap();

    assert!(imported.ignored.is_empty(), "{:?}", imported.ignored);
    assert!(imported.duplicates.is_empty(), "{:?}", imported.duplicates);
    assert_eq!(sorted(imported.objects), sorted(drive));
}

/// The property the format was designed around, now at tree level: exporting a
/// drive that has not changed must touch nothing at all. If this fails, every
/// `git status` is dirty and the repository is unusable as a sync target.
#[test]
fn re_exporting_an_unchanged_drive_writes_nothing() {
    let root = TempDir::new().unwrap();
    let drive = a_drive_with_nested_folders();

    let first = export(root.path(), &drive).unwrap();
    assert_eq!(first.written, drive.len());

    let second = export(root.path(), &drive).unwrap();

    assert_eq!(
        second,
        ExportSummary {
            written: 0,
            unchanged: drive.len(),
            removed_files: 0,
            removed_directories: 0,
            orphaned: Vec::new(),
        }
    );
}

/// Folders are directories rather than a `folder:` field, so that the tree is
/// browsable without Warp and a move is a rename git can follow.
#[test]
fn folders_are_directories_on_disk() {
    let root = TempDir::new().unwrap();
    let drive = a_drive_with_nested_folders();

    export(root.path(), &drive).unwrap();

    let scripts = root
        .path()
        .join("scripts-".to_owned() + &hash_of(&drive[0]));
    let nested = scripts.join("nested-".to_owned() + &hash_of(&drive[1]));

    assert!(scripts.join(FOLDER_FILE_NAME).is_file());
    assert!(nested.join(FOLDER_FILE_NAME).is_file());
    assert!(
        nested.join(drive[3].object.file_name()).is_file(),
        "the nested workflow is not inside its folder"
    );
}

/// Moving an object between folders is a file move, and the old location is
/// left clean.
#[test]
fn moving_an_object_moves_its_file() {
    let root = TempDir::new().unwrap();
    let mut drive = a_drive_with_nested_folders();

    export(root.path(), &drive).unwrap();
    let was_at = root
        .path()
        .join("scripts-".to_owned() + &hash_of(&drive[0]))
        .join("nested-".to_owned() + &hash_of(&drive[1]))
        .join(drive[3].object.file_name());
    assert!(was_at.is_file());

    // Move the nested workflow up to the top level of the drive.
    drive[3].parent = None;
    let summary = export(root.path(), &drive).unwrap();

    assert!(!was_at.exists(), "the file was left behind at its old path");
    assert!(root.path().join(drive[3].object.file_name()).is_file());
    assert_eq!(summary.removed_files, 1);
}

/// Deleting an object deletes its file, and empties the folder that held it.
#[test]
fn a_deleted_object_and_its_empty_folder_are_removed() {
    let root = TempDir::new().unwrap();
    let drive = a_drive_with_nested_folders();
    export(root.path(), &drive).unwrap();

    let remaining: Vec<_> = drive.iter().take(1).cloned().collect();
    let summary = export(root.path(), &remaining).unwrap();

    // The nested folder marker, both workflows and the notebook.
    assert_eq!(summary.removed_files, 4);
    assert_eq!(summary.removed_directories, 1);
    assert!(
        root.path()
            .join("scripts-".to_owned() + &hash_of(&drive[0]))
            .is_dir(),
        "the surviving folder was removed with the rest"
    );
}

/// The one that matters most. The export target is a repository the user keeps
/// their own things in, and an exporter that treats it as its own is a data
/// loss bug rather than a sync feature. Nothing is deleted unless it parses as
/// a file this exporter wrote.
#[test]
fn an_export_never_touches_files_it_did_not_write() {
    let root = TempDir::new().unwrap();
    std::fs::write(root.path().join("README.md"), "# My drive\n").unwrap();
    std::fs::write(root.path().join("notes.json"), "{\"mine\": true}\n").unwrap();
    std::fs::create_dir(root.path().join(".git")).unwrap();
    std::fs::write(root.path().join(".git").join("HEAD"), "ref: main\n").unwrap();
    std::fs::create_dir(root.path().join("my-notes")).unwrap();
    std::fs::write(root.path().join("my-notes").join("todo.md"), "- [ ] x\n").unwrap();

    // Export, then export an empty drive — the most destructive thing a caller
    // can ask for.
    export(root.path(), &a_drive_with_nested_folders()).unwrap();
    let summary = export(root.path(), &[]).unwrap();

    assert!(root.path().join("README.md").is_file());
    assert!(root.path().join("notes.json").is_file());
    assert!(root.path().join(".git").join("HEAD").is_file());
    assert!(root.path().join("my-notes").join("todo.md").is_file());
    // Every one of the five exported objects, and neither of the user's two
    // files nor anything under `.git`.
    assert_eq!(summary.removed_files, 5);
    assert_eq!(summary.removed_directories, 2);
}

/// A directory the user made is descended into — their files inside it are
/// still theirs — but it does not become a Warp Drive folder, because nothing
/// declared it one.
#[test]
fn a_plain_directory_does_not_become_a_folder() {
    let root = TempDir::new().unwrap();
    let workflow = placed(workflow("deploy"), None);
    std::fs::create_dir(root.path().join("by-hand")).unwrap();
    export(
        &root.path().join("by-hand"),
        std::slice::from_ref(&workflow),
    )
    .unwrap();

    let imported = import(root.path()).unwrap();

    assert_eq!(imported.objects.len(), 1);
    assert_eq!(
        imported.objects[0].parent, None,
        "an undeclared directory invented a parent folder"
    );
}

/// A `folder_id` pointing at a folder that is not there is not hypothetical:
/// the column has no referential integrity behind it, and a partial checkout
/// produces exactly this. Reparenting to the top level keeps the object
/// reachable; naming it in the summary keeps that from being silent.
#[test]
fn an_object_with_a_missing_parent_lands_at_the_top_level() {
    let root = TempDir::new().unwrap();
    let missing = SyncId::ClientId(ClientId::new());
    let orphan = placed(folder("lost"), Some(missing));

    let summary = export(root.path(), std::slice::from_ref(&orphan)).unwrap();

    assert_eq!(summary.orphaned, vec![orphan.object.id]);
    assert!(
        root.path()
            .join("lost-".to_owned() + &hash_of(&orphan))
            .join(FOLDER_FILE_NAME)
            .is_file()
    );
}

/// Two folders each claiming the other as parent. Without the chain guard this
/// recurses until the stack runs out; upstream carries the same guard in
/// `is_trashed_internal`, for the same reason.
#[test]
fn a_cycle_in_the_folder_graph_terminates() {
    let root = TempDir::new().unwrap();
    let mut first = placed(folder("first"), None);
    let mut second = placed(folder("second"), None);
    first.parent = Some(second.object.id);
    second.parent = Some(first.object.id);

    let summary = export(root.path(), &[first, second]).unwrap();

    assert_eq!(summary.written, 2);
    assert_eq!(
        summary.orphaned.len(),
        1,
        "the cycle was not broken exactly once"
    );
}

/// A copy-pasted file, or a merge that resolved by keeping both sides. Two
/// files claiming one identity cannot both be imported, so the first by path
/// wins and the other is reported rather than silently overwriting it.
#[test]
fn a_duplicated_identity_is_reported_rather_than_merged() {
    let root = TempDir::new().unwrap();
    let workflow = placed(workflow("deploy"), None);
    export(root.path(), std::slice::from_ref(&workflow)).unwrap();

    let original = root.path().join(workflow.object.file_name());
    std::fs::copy(&original, root.path().join("a-copy.json")).unwrap();

    let imported = import(root.path()).unwrap();

    assert_eq!(imported.objects.len(), 1);
    assert_eq!(imported.duplicates.len(), 1);
    assert_eq!(imported.duplicates[0].1, workflow.object.id);
}

/// Files that are not ours are reported with a reason rather than dropped on
/// the floor, so "why is my workflow missing" has an answer.
#[test]
fn unreadable_files_are_reported_with_a_reason() {
    let root = TempDir::new().unwrap();
    std::fs::write(root.path().join("README.md"), "# not a notebook\n").unwrap();
    export(root.path(), &[placed(workflow("deploy"), None)]).unwrap();

    let imported = import(root.path()).unwrap();

    assert_eq!(imported.objects.len(), 1);
    assert_eq!(imported.ignored.len(), 1);
    assert!(imported.ignored[0].0.ends_with("README.md"));
}

/// T4.4e. A half-merged file has to be told apart from a missing one, because
/// this layer reports absence and the layer above reads absence as deletion.
///
/// Left in `ignored` — where it landed before, as an unparseable file — the
/// object is simply not in the tree, and the next import trashes it. That is the
/// worst possible response to "the user is in the middle of merging this".
#[test]
fn a_half_merged_file_is_reported_as_conflicted_rather_than_missing() {
    let root = TempDir::new().unwrap();
    let drive = a_drive_with_nested_folders();
    export(root.path(), &drive).unwrap();

    conflict_the_file(&root.path().join(drive[2].object.file_name()));

    let imported = import(root.path()).unwrap();

    assert_eq!(imported.conflicted.len(), 1, "{:?}", imported.conflicted);
    assert_eq!(imported.conflicted[0].name, "deploy");
    assert_eq!(imported.conflicted[0].line, 1);
    assert!(imported.ignored.is_empty(), "{:?}", imported.ignored);
    assert_eq!(
        imported.objects.len(),
        drive.len() - 1,
        "the conflicted object must not be half-imported"
    );
}

/// The repository belongs to the user, and so does their merge. A conflict in a
/// file that was never ours must not stop their drive from working — which is
/// why the test for ours-ness is "does either side parse", and not "is there a
/// marker in it".
#[test]
fn a_conflict_in_a_file_that_is_not_ours_stops_nothing() {
    let root = TempDir::new().unwrap();
    let drive = a_drive_with_nested_folders();
    export(root.path(), &drive).unwrap();
    std::fs::write(
        root.path().join("README.md"),
        "<<<<<<< HEAD\nmine\n=======\ntheirs\n>>>>>>> theirs\n",
    )
    .unwrap();

    let imported = import(root.path()).unwrap();

    assert!(imported.conflicted.is_empty(), "{:?}", imported.conflicted);
    assert_eq!(imported.objects.len(), drive.len());
    assert!(
        imported.ignored.iter().any(|(path, reason)| {
            path.ends_with("README.md") && reason.contains("neither side")
        }),
        "{:?}",
        imported.ignored
    );
    export(root.path(), &drive).expect("someone else's merge is not our business");
}

/// The refusal is all-or-nothing, and it has to be: a merge that stopped an
/// export half way through would leave the tree in a state neither the store
/// nor git could explain.
#[test]
fn an_export_writes_nothing_at_all_when_a_file_it_owns_is_half_merged() {
    let root = TempDir::new().unwrap();
    let mut drive = a_drive_with_nested_folders();
    export(root.path(), &drive).unwrap();

    let conflicted = root.path().join(drive[2].object.file_name());
    conflict_the_file(&conflicted);

    // An unrelated edit the export would otherwise write out. Changing the
    // revision rather than the name keeps the filename — and so the path being
    // asserted on — the same.
    drive[3].object.revision_ts = Some(1_755_544_456_999_999);
    let unrelated = root
        .path()
        .join("scripts-".to_owned() + &hash_of(&drive[0]))
        .join("nested-".to_owned() + &hash_of(&drive[1]))
        .join(drive[3].object.file_name());
    let before = std::fs::read_to_string(&unrelated).unwrap();

    let err = export(root.path(), &drive).unwrap_err();

    assert!(
        err.downcast_ref::<ConflictsInTheWay>().is_some(),
        "a merge in the way must be its own error, not a write failure: {err:#}"
    );
    assert_eq!(
        std::fs::read_to_string(&unrelated).unwrap(),
        before,
        "a file was written before the refusal"
    );
    assert!(
        std::fs::read_to_string(&conflicted)
            .unwrap()
            .contains("<<<<<<<"),
        "the export overwrote the merge the user was in the middle of"
    );
}

/// A folder's marker file can conflict too — two machines renaming the same
/// folder — and it is reported under the folder's name rather than the
/// directory's, because the directory name is derived from the old one.
#[test]
fn a_conflicted_folder_marker_is_reported_under_the_folders_name() {
    let root = TempDir::new().unwrap();
    let drive = a_drive_with_nested_folders();
    export(root.path(), &drive).unwrap();

    conflict_the_file(
        &root
            .path()
            .join("scripts-".to_owned() + &hash_of(&drive[0]))
            .join(FOLDER_FILE_NAME),
    );

    let imported = import(root.path()).unwrap();

    assert_eq!(imported.conflicted.len(), 1, "{:?}", imported.conflicted);
    assert_eq!(imported.conflicted[0].name, "Scripts");
}

/// Replaces a file with what git leaves in the working tree when two edits to
/// it collide. Both sides are the real file, so both sides still parse.
fn conflict_the_file(path: &Path) {
    let ours = std::fs::read_to_string(path).unwrap();
    let theirs = ours.replace("2025-08-18T19:14:16", "2025-08-19T09:30:00");
    assert_ne!(ours, theirs, "the fixture's two sides are identical");
    std::fs::write(
        path,
        format!("<<<<<<< HEAD\n{ours}=======\n{theirs}>>>>>>> theirs\n"),
    )
    .unwrap();
}

fn a_drive_with_nested_folders() -> Vec<PlacedObject> {
    let scripts = placed(folder("Scripts"), None);
    let nested = placed(folder("Nested"), Some(scripts.object.id));
    let top_level = placed(workflow("deploy"), None);
    let inner = placed(workflow("build"), Some(nested.object.id));
    let notes = placed(
        notebook("Field notes", "# Notes\n\n---\n\nbody\n"),
        Some(scripts.object.id),
    );

    vec![scripts, nested, top_level, inner, notes]
}

fn placed(object: PortableObject, parent: Option<SyncId>) -> PlacedObject {
    PlacedObject { object, parent }
}

fn base(name: &str) -> PortableObject {
    PortableObject {
        id: SyncId::ClientId(ClientId::new()),
        object_type: ObjectType::Workflow,
        name: name.to_owned(),
        owner: Owner::User {
            user_uid: UserUid::new("local"),
        },
        revision_ts: Some(1_755_544_456_123_456),
        metadata_last_updated_ts: None,
        trashed_ts: None,
        creator_uid: None,
        last_editor_uid: None,
        is_welcome_object: false,
        payload: Payload::Json(json!({ "name": name })),
    }
}

fn workflow(name: &str) -> PortableObject {
    base(name)
}

fn folder(name: &str) -> PortableObject {
    PortableObject {
        object_type: ObjectType::Folder,
        payload: Payload::Folder {
            is_warp_pack: false,
        },
        ..base(name)
    }
}

fn notebook(name: &str, markdown: &str) -> PortableObject {
    PortableObject {
        object_type: ObjectType::Notebook,
        payload: Payload::Notebook {
            markdown: markdown.to_owned(),
            ai_document_id: None,
        },
        ..base(name)
    }
}

/// The filename hash for an object, so path assertions do not have to hardcode
/// a digest of a randomly generated id.
fn hash_of(placed: &PlacedObject) -> String {
    let stem = placed
        .object
        .folder_directory_name()
        .unwrap_or_else(|| placed.object.file_name());
    stem.rsplit('-')
        .next()
        .expect("the stem ends in a hash")
        .trim_end_matches(".json")
        .trim_end_matches(".md")
        .to_owned()
}

fn sorted(mut objects: Vec<PlacedObject>) -> Vec<PlacedObject> {
    objects.sort_by_key(|placed| placed.object.id.to_string());
    objects
}
