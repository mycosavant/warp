use tempfile::TempDir;
use warpui::App;

use super::*;
use crate::auth::{AuthStateProvider, UserUid};
use crate::cloud_object::{CloudObjectMetadata, CloudObjectPermissions, ObjectType, Owner};
use crate::drive::folders::{CloudFolder, CloudFolderModel};
use crate::drive::local_sync::format::Payload;
use crate::drive::local_sync::tree;
use crate::features::FeatureFlag;
use crate::notebooks::{CloudNotebook, CloudNotebookModel};
use crate::server::ids::{ClientId, SyncId};
use crate::workflows::aliases::{Aliases, WorkflowAlias, WorkflowAliases};
use crate::workflows::workflow::Workflow;
use crate::workflows::{CloudWorkflow, CloudWorkflowModel};
use crate::workspaces::user_workspaces::UserWorkspaces;
use settings::Setting as _;

/// The end-to-end shape of T4.4: a real store, through the bridge, onto a
/// disk, and back — with the object graph intact.
///
/// The pieces below it are tested against each other in `format_tests` and
/// `tree_tests`, which is exactly the arrangement that let T4.6's bug through:
/// two correct halves that disagreed in the middle. This is the test that spans
/// the seam.
#[test]
fn the_live_store_round_trips_through_a_directory() {
    let folder_id = SyncId::ClientId(ClientId::new());

    with_drive(
        vec![
            local_folder(folder_id, "Scripts"),
            local_workflow(None, "deploy", "cargo build"),
            local_workflow(Some(folder_id), "test", "cargo test"),
            local_notebook(None, "Field notes", "# Notes\n\n---\n\nbody\n"),
        ],
        move |ctx| {
            let root = TempDir::new().unwrap();
            let (objects, summary) = snapshot(ctx);
            assert_eq!(summary, SnapshotSummary::default());
            assert_eq!(objects.len(), 4);

            tree::export(root.path(), &objects).unwrap();
            let imported = tree::import(root.path()).unwrap();

            assert!(imported.ignored.is_empty(), "{:?}", imported.ignored);
            let mut imported = imported.objects;
            imported.sort_by_key(|placed| placed.object.id.to_string());

            assert_eq!(imported, objects);
        },
    );
}

/// Placement comes from `folder_id` on the way out and from the directory on
/// the way back in, and the two have to agree.
#[test]
fn an_objects_folder_becomes_the_directory_it_sits_in() {
    let folder_id = SyncId::ClientId(ClientId::new());

    with_drive(
        vec![
            local_folder(folder_id, "Scripts"),
            local_workflow(Some(folder_id), "test", "cargo test"),
        ],
        move |ctx| {
            let root = TempDir::new().unwrap();
            let (objects, _) = snapshot(ctx);
            tree::export(root.path(), &objects).unwrap();

            let workflow = objects
                .iter()
                .find(|placed| placed.object.object_type == ObjectType::Workflow)
                .expect("the workflow was snapshotted");
            let folder = objects
                .iter()
                .find(|placed| placed.object.object_type == ObjectType::Folder)
                .expect("the folder was snapshotted");

            assert_eq!(workflow.parent, Some(folder.object.id));
            assert!(
                root.path()
                    .join(folder.object.folder_directory_name().unwrap())
                    .join(workflow.object.file_name())
                    .is_file()
            );
        },
    );
}

/// A workflow's payload is `serde_json`'s own output, re-read rather than
/// re-serialized, so what reaches the file is what SQLite holds.
#[test]
fn a_workflow_keeps_its_payload() {
    with_drive(vec![local_workflow(None, "deploy", "cargo build")], |ctx| {
        let (objects, _) = snapshot(ctx);

        let Payload::Json(data) = &objects[0].object.payload else {
            panic!("a workflow should carry a json payload");
        };
        assert_eq!(data["command"], "cargo build");
        assert_eq!(objects[0].object.name, "deploy");
    });
}

/// Someone else's object is not this user's to put in their git repository, and
/// dropping it silently would leave "where did my shared folder go" with no
/// answer.
#[test]
fn objects_from_another_drive_are_left_out_and_counted() {
    let _guard = FeatureFlag::SharedWithMe.override_enabled(true);
    let mut theirs = local_workflow(None, "not-mine", "rm -rf /");
    theirs.permissions_mut().owner = Owner::User {
        user_uid: UserUid::new("kK3sBqL9vXdF2mN7pR1tY4wZ8cJ6"),
    };

    with_drive(
        vec![local_workflow(None, "mine", "echo hi"), theirs],
        move |ctx| {
            let (objects, summary) = snapshot(ctx);

            assert_eq!(objects.len(), 1);
            assert_eq!(objects[0].object.name, "mine");
            assert_eq!(summary.not_personal, 1);
            assert!(summary.unreadable.is_empty());
        },
    );
}

/// Trash is still the user's data, and emptying it is their decision. An export
/// that dropped trashed objects would take the undo away the first time it ran.
#[test]
fn a_trashed_object_is_exported_with_its_timestamp() {
    let mut trashed = local_workflow(None, "old", "echo bye");
    trashed.metadata_mut().trashed_ts = Some(chrono::Utc::now().into());
    let expected = trashed
        .metadata()
        .trashed_ts
        .map(|ts| ts.timestamp_micros());

    with_drive(vec![trashed], move |ctx| {
        let (objects, _) = snapshot(ctx);

        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].object.trashed_ts, expected);
    });
}

/// T4.4g. An alias is not a drive object — it lives in a settings group — so
/// without this join a workflow arrives on another machine having lost the one
/// thing the user typed to reach it.
#[test]
fn an_alias_travels_in_its_workflows_file() {
    let workflow = local_workflow(None, "deploy", "cargo build");
    let workflow_id = workflow.sync_id();

    with_drive_and_aliases(
        vec![workflow],
        vec![alias("dep", workflow_id)],
        move |ctx| {
            let (objects, summary) = snapshot(ctx);

            assert_eq!(
                objects[0].object.aliases,
                vec![Alias {
                    alias: "dep".to_owned(),
                    env_vars: None,
                    arguments: None,
                }]
            );
            assert_eq!(summary.aliases_not_mirrored, 0);
        },
    );
}

/// An alias for a workflow the mirror does not contain — a team workflow, or one
/// this machine trashed. It cannot travel, since there is no file for it to
/// travel in, so it is counted rather than silently missing.
#[test]
fn an_alias_for_a_workflow_outside_the_mirror_is_counted() {
    let elsewhere = SyncId::ClientId(ClientId::new());

    with_drive_and_aliases(
        vec![local_workflow(None, "deploy", "cargo build")],
        vec![alias("team-thing", elsewhere)],
        move |ctx| {
            let (objects, summary) = snapshot(ctx);

            assert!(objects[0].object.aliases.is_empty());
            assert_eq!(summary.aliases_not_mirrored, 1);
        },
    );
}

/// Builds an app with `objects` in its store and runs `assertions` against it.
fn with_drive(objects: Vec<Box<dyn CloudObject>>, assertions: impl FnOnce(&AppContext) + 'static) {
    with_drive_and_aliases(objects, Vec::new(), assertions);
}

fn with_drive_and_aliases(
    objects: Vec<Box<dyn CloudObject>>,
    aliases: Vec<WorkflowAlias>,
    assertions: impl FnOnce(&AppContext) + 'static,
) {
    App::test((), |app| async move {
        app.add_singleton_model(|_| AuthStateProvider::new_logged_out_for_test());
        app.add_singleton_model(UserWorkspaces::default_mock);
        app.add_singleton_model(|_| CloudModel::new(None, objects, None));
        app.add_singleton_model(move |_| WorkflowAliases {
            aliases: Aliases::new(Some(aliases)),
        });

        app.read(assertions);
    });
}

fn alias(text: &str, workflow_id: SyncId) -> WorkflowAlias {
    WorkflowAlias {
        alias: text.to_owned(),
        workflow_id,
        arguments: None,
        env_vars: None,
    }
}

fn local_permissions() -> CloudObjectPermissions {
    CloudObjectPermissions {
        owner: crate::fork::local_drive_owner().expect("fork policy provides a local drive owner"),
        permissions_last_updated_ts: None,
        anyone_with_link: None,
        guests: Vec::new(),
    }
}

fn metadata(folder_id: Option<SyncId>) -> CloudObjectMetadata {
    CloudObjectMetadata {
        folder_id,
        ..CloudObjectMetadata::mock()
    }
}

fn local_workflow(folder_id: Option<SyncId>, name: &str, command: &str) -> Box<dyn CloudObject> {
    Box::new(CloudWorkflow::new(
        SyncId::ClientId(ClientId::new()),
        CloudWorkflowModel::new(Workflow::new(name, command)),
        metadata(folder_id),
        local_permissions(),
    ))
}

fn local_folder(id: SyncId, name: &str) -> Box<dyn CloudObject> {
    Box::new(CloudFolder::new(
        id,
        CloudFolderModel::new(name, false),
        metadata(None),
        local_permissions(),
    ))
}

fn local_notebook(folder_id: Option<SyncId>, title: &str, body: &str) -> Box<dyn CloudObject> {
    Box::new(CloudNotebook::new(
        SyncId::ClientId(ClientId::new()),
        CloudNotebookModel {
            title: title.to_owned(),
            data: body.to_owned(),
            ai_document_id: None,
            conversation_id: None,
        },
        metadata(folder_id),
        local_permissions(),
    ))
}
