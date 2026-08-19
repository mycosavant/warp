use tempfile::TempDir;
use warpui::App;

use super::*;
use crate::auth::AuthStateProvider;
use crate::cloud_object::CloudObject;
use crate::drive::local_sync::tree;
use crate::network::NetworkStatus;
use crate::server::ids::ClientId;
use crate::server::sync_queue::SyncQueue;
use crate::workspaces::team_tester::TeamTesterStatus;
use crate::workspaces::user_workspaces::UserWorkspaces;

/// The round trip T4.4c could only half-assert, now closed: a real store, out
/// to a directory, **edited on disk**, and back into the store.
///
/// Everything before this could show that files describe the drive. This is the
/// first test that shows the drive follows the files, which is the only reason
/// to have written any of it.
#[test]
fn an_edit_made_on_disk_reaches_the_store() {
    App::test((), |mut app| async move {
        let root = TempDir::new().unwrap();
        let workflow = local_workflow("deploy", "cargo build");
        let uid = workflow.sync_id().uid();
        drive_app(&mut app, vec![workflow]);

        let (objects, _) = app.read(snapshot);
        tree::export(root.path(), &objects).unwrap();

        // Edit the file the way a user would after a `git pull`.
        let path = root.path().join(objects[0].object.file_name());
        let edited = std::fs::read_to_string(&path)
            .unwrap()
            .replace("cargo build", "cargo build --release");
        std::fs::write(&path, edited).unwrap();

        let imported = tree::import(root.path()).unwrap();
        let summary = apply(&imported.objects, &mut app).unwrap();

        assert_eq!(summary.updated, 1);
        assert_eq!(summary.created, 0);
        assert_eq!(summary.trashed, 0);

        app.read(|ctx| {
            let (objects, _) = snapshot(ctx);
            let Payload::Json(data) = &objects[0].object.payload else {
                panic!("a workflow carries json");
            };
            assert_eq!(data["command"], "cargo build --release");
            assert_eq!(
                objects[0].object.id.uid(),
                uid,
                "the edit minted a new object instead of updating the old one"
            );
        });
    });
}

/// A file that arrives from another machine becomes an object, keeping the
/// identity the file carries — which is what stops the next export from
/// producing a second copy of it.
#[test]
fn a_new_file_becomes_an_object_with_the_identity_in_it() {
    App::test((), |mut app| async move {
        drive_app(&mut app, vec![local_workflow("mine", "echo mine")]);

        let theirs = PlacedObject {
            object: PortableObject {
                id: SyncId::ClientId(ClientId::new()),
                object_type: ObjectType::Workflow,
                name: "from-elsewhere".to_owned(),
                owner: crate::fork::local_drive_owner().unwrap(),
                revision_ts: None,
                metadata_last_updated_ts: None,
                trashed_ts: None,
                creator_uid: None,
                last_editor_uid: None,
                is_welcome_object: false,
                payload: Payload::Json(serde_json::json!({
                    "name": "from-elsewhere",
                    "command": "echo hi",
                    "arguments": [],
                })),
            },
            parent: None,
        };
        let their_uid = theirs.object.id.uid();

        // Both objects: the tree from the other machine has been merged with
        // this one's, which is what a `git pull` leaves behind.
        let (mine, _) = app.read(snapshot);
        let mut incoming = mine;
        incoming.push(theirs);

        let summary = apply(&incoming, &mut app).unwrap();

        assert_eq!(summary.created, 1);
        assert_eq!(summary.unchanged, 1);
        assert_eq!(summary.trashed, 0);
        app.read(|ctx| {
            assert!(
                CloudModel::as_ref(ctx).get_by_uid(&their_uid).is_some(),
                "the imported object did not arrive under its own id"
            );
        });
    });
}

/// The rule the deletion story rests on. Absence means the other machine
/// emptied its trash, and the conservative echo of that is a local trash —
/// recoverable — rather than a delete, which is not.
#[test]
fn an_object_missing_from_the_tree_is_trashed_rather_than_deleted() {
    App::test((), |mut app| async move {
        let root = TempDir::new().unwrap();
        let kept = local_workflow("kept", "echo kept");
        let removed = local_workflow("removed", "echo removed");
        let removed_uid = removed.sync_id().uid();
        drive_app(&mut app, vec![kept, removed]);

        let (objects, _) = app.read(snapshot);
        tree::export(root.path(), &objects).unwrap();

        // The other machine deleted it and emptied its trash.
        let removed_path = objects
            .iter()
            .find(|placed| placed.object.name == "removed")
            .map(|placed| root.path().join(placed.object.file_name()))
            .unwrap();
        std::fs::remove_file(removed_path).unwrap();

        let imported = tree::import(root.path()).unwrap();
        let summary = apply(&imported.objects, &mut app).unwrap();

        assert_eq!(summary.trashed, 1);
        app.read(|ctx| {
            let object = CloudModel::as_ref(ctx)
                .get_by_uid(&removed_uid)
                .expect("trashed, not deleted — the user can still get it back");
            assert!(object.metadata().trashed_ts.is_some());
        });
    });
}

/// Import has to be idempotent or a repeated pull churns the store, rewrites
/// every SQLite row and reshuffles the panel for nothing.
#[test]
fn applying_the_same_tree_twice_changes_nothing_the_second_time() {
    App::test((), |mut app| async move {
        let root = TempDir::new().unwrap();
        drive_app(&mut app, vec![local_workflow("deploy", "cargo build")]);

        let (objects, _) = app.read(snapshot);
        tree::export(root.path(), &objects).unwrap();
        let imported = tree::import(root.path()).unwrap();

        apply(&imported.objects, &mut app).unwrap();
        let second = apply(&imported.objects, &mut app).unwrap();

        assert_eq!(
            second,
            ApplySummary {
                created: 0,
                updated: 0,
                unchanged: 1,
                trashed: 0,
                unreadable: Vec::new(),
            }
        );
    });
}

/// An already-trashed object is absent from nothing — it exports with its
/// timestamp. Re-trashing it would rewrite that timestamp on every import and
/// make the drive look like it had just been emptied.
#[test]
fn an_already_trashed_object_is_left_alone() {
    App::test((), |mut app| async move {
        let root = TempDir::new().unwrap();
        let mut trashed = local_workflow("old", "echo bye");
        trashed.metadata_mut().trashed_ts = Some(chrono::Utc::now().into());
        let uid = trashed.sync_id().uid();
        let was = trashed.metadata().trashed_ts;
        drive_app(&mut app, vec![trashed, local_workflow("kept", "echo kept")]);

        let (objects, _) = app.read(snapshot);
        tree::export(root.path(), &objects).unwrap();
        let imported = tree::import(root.path()).unwrap();

        let summary = apply(&imported.objects, &mut app).unwrap();

        assert_eq!(summary.trashed, 0);
        app.read(|ctx| {
            assert_eq!(
                CloudModel::as_ref(ctx)
                    .get_by_uid(&uid)
                    .unwrap()
                    .metadata()
                    .trashed_ts,
                was,
                "the trash timestamp was rewritten by an import that changed nothing"
            );
        });
    });
}

/// The most destructive call available, refused. An import pointed at an empty
/// or wrong directory reads as "everything was deleted", and would trash the
/// whole drive in one go.
#[test]
fn an_empty_tree_is_refused() {
    App::test((), |mut app| async move {
        drive_app(&mut app, vec![local_workflow("deploy", "cargo build")]);

        let err = apply(&[], &mut app).unwrap_err();

        assert!(
            format!("{err:#}").contains("no Warp Drive objects"),
            "{err:#}"
        );
        app.read(|ctx| {
            assert_eq!(snapshot(ctx).0.len(), 1, "the drive was emptied anyway");
        });
    });
}

/// `is_open` is sidebar state, deliberately absent from the format. An import
/// must not decide it, or pulling would collapse folders someone had open.
#[test]
fn an_import_does_not_decide_whether_a_folder_is_open() {
    App::test((), |mut app| async move {
        let root = TempDir::new().unwrap();
        let folder = local_folder("Scripts", true);
        let folder_uid = folder.sync_id().uid();
        drive_app(&mut app, vec![folder]);

        let (objects, _) = app.read(snapshot);
        tree::export(root.path(), &objects).unwrap();
        let imported = tree::import(root.path()).unwrap();

        apply(&imported.objects, &mut app).unwrap();

        app.read(|ctx| {
            let folder = CloudModel::as_ref(ctx)
                .get_folder_by_uid(&folder_uid)
                .expect("the folder is still there");
            assert!(
                folder.model().is_open,
                "an import closed a folder the user had open"
            );
        });
    });
}

/// Placement is the path, so moving a file between directories has to move the
/// object between folders.
#[test]
fn moving_a_file_moves_the_object_between_folders() {
    App::test((), |mut app| async move {
        let root = TempDir::new().unwrap();
        let folder = local_folder("Scripts", false);
        let folder_id = folder.sync_id();
        let mut workflow = local_workflow("deploy", "cargo build");
        workflow.metadata_mut().folder_id = Some(folder_id);
        let workflow_uid = workflow.sync_id().uid();
        drive_app(&mut app, vec![folder, workflow]);

        let (objects, _) = app.read(snapshot);
        tree::export(root.path(), &objects).unwrap();

        // Move the workflow out of the folder, as a `git mv` would.
        let placed = objects
            .iter()
            .find(|placed| placed.object.name == "deploy")
            .unwrap();
        let folder_dir = objects
            .iter()
            .find_map(|placed| placed.object.folder_directory_name())
            .unwrap();
        std::fs::rename(
            root.path()
                .join(&folder_dir)
                .join(placed.object.file_name()),
            root.path().join(placed.object.file_name()),
        )
        .unwrap();

        let imported = tree::import(root.path()).unwrap();
        apply(&imported.objects, &mut app).unwrap();

        app.read(|ctx| {
            assert_eq!(
                CloudModel::as_ref(ctx)
                    .get_by_uid(&workflow_uid)
                    .unwrap()
                    .metadata()
                    .folder_id,
                None,
                "the object did not follow its file out of the folder"
            );
        });
    });
}

fn drive_app(app: &mut App, objects: Vec<Box<dyn CloudObject>>) {
    app.add_singleton_model(|_| NetworkStatus::new());
    app.add_singleton_model(TeamTesterStatus::mock);
    app.add_singleton_model(|_| AuthStateProvider::new_logged_out_for_test());
    app.add_singleton_model(UserWorkspaces::default_mock);
    app.add_singleton_model(SyncQueue::mock);
    app.add_singleton_model(|_| CloudModel::new(None, objects, None));
    app.add_singleton_model(UpdateManager::mock);
}

fn local_permissions() -> CloudObjectPermissions {
    CloudObjectPermissions {
        owner: crate::fork::local_drive_owner().expect("fork policy provides a local drive owner"),
        permissions_last_updated_ts: None,
        anyone_with_link: None,
        guests: Vec::new(),
    }
}

fn local_workflow(name: &str, command: &str) -> Box<dyn CloudObject> {
    Box::new(crate::workflows::CloudWorkflow::new(
        SyncId::ClientId(ClientId::new()),
        CloudWorkflowModel::new(Workflow::new(name, command)),
        CloudObjectMetadata::mock(),
        local_permissions(),
    ))
}

fn local_folder(name: &str, is_open: bool) -> Box<dyn CloudObject> {
    Box::new(crate::drive::folders::CloudFolder::new(
        SyncId::ClientId(ClientId::new()),
        CloudFolderModel {
            name: name.to_owned(),
            is_open,
            is_warp_pack: false,
        },
        CloudObjectMetadata::mock(),
        local_permissions(),
    ))
}
