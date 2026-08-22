use serde_json::json;
use settings::{Setting as _, SettingsManager};
use warpui::App;

use super::*;
use crate::auth::AuthStateProvider;
use crate::cloud_object::model::persistence::CloudModel;
use crate::cloud_object::{CloudObject, CloudObjectMetadata, CloudObjectPermissions};
use crate::network::NetworkStatus;
use crate::server::cloud_objects::update_manager::UpdateManager;
use crate::server::sync_queue::SyncQueue;
use crate::settings::{LocalDriveSyncPath, LocalDriveSyncSettings};
use crate::workflows::aliases::WorkflowAliases;
use crate::workflows::workflow::Workflow;
use crate::workflows::{CloudWorkflow, CloudWorkflowModel};
use crate::workspaces::team_tester::TeamTesterStatus;

/// The gap T1.12 was opened for, closed: the store is legible from outside the
/// GUI.
#[test]
fn list_reports_the_objects_in_the_personal_drive() {
    App::test((), |mut app| async move {
        let bridge = drive_app(&mut app, vec![workflow("deploy"), workflow("build")]);

        let result = bridge.update(&mut app, |_, ctx| {
            list(&json!({}), ctx).expect("drive.object.list succeeds")
        });

        let objects = result["objects"].as_array().unwrap();
        assert_eq!(objects.len(), 2);
        let mut names: Vec<_> = objects
            .iter()
            .map(|object| object["name"].as_str().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, ["build", "deploy"]);
        assert_eq!(objects[0]["object_type"], "workflow");
        assert_eq!(result["not_personal"], 0);
    });
}

/// The round trip that makes `create` usable at all: what `get` prints is a
/// worked example of what `create` accepts, so nothing has to be documented
/// twice.
#[test]
fn create_then_get_returns_the_body_that_was_written() {
    App::test((), |mut app| async move {
        let bridge = drive_app(&mut app, Vec::new());

        let body = serde_json::to_string(&Workflow::new("ship", "echo ship")).unwrap();
        let created = bridge.update(&mut app, |_, ctx| {
            create(
                &json!({ "object_type": "workflow", "name": "ship", "body": body }),
                ctx,
            )
            .expect("drive.object.create succeeds")
        });

        assert_eq!(created["name"], "ship");
        assert_eq!(created["object_type"], "workflow");
        let id = created["id"].as_str().unwrap().to_owned();

        let fetched = bridge.update(&mut app, |_, ctx| {
            get(&json!({ "id": id }), ctx).expect("drive.object.get succeeds")
        });

        let contents = fetched["contents"].as_str().unwrap();
        assert!(contents.contains("\"type\": \"WORKFLOW\""), "{contents}");
        assert!(contents.contains("echo ship"), "{contents}");
        // The id is the store's, not the caller's, and the file says so.
        assert!(contents.contains(&id), "{contents}");
    });
}

/// Identity is minted here, so two creates with the same name are two objects
/// rather than one overwritten one. This is the reason `create` does not accept
/// a file: a caller-supplied `uid` would make this test fail silently in
/// production instead.
#[test]
fn creating_the_same_name_twice_makes_two_objects() {
    App::test((), |mut app| async move {
        let bridge = drive_app(&mut app, Vec::new());
        let body = serde_json::to_string(&Workflow::new("ship", "echo ship")).unwrap();
        let params = json!({ "object_type": "workflow", "name": "ship", "body": body });

        let first = bridge.update(&mut app, |_, ctx| create(&params, ctx).unwrap());
        let second = bridge.update(&mut app, |_, ctx| create(&params, ctx).unwrap());

        assert_ne!(first["id"], second["id"]);
        let result = bridge.update(&mut app, |_, ctx| list(&json!({}), ctx).unwrap());
        assert_eq!(result["objects"].as_array().unwrap().len(), 2);
    });
}

/// Placement is reported by name, because that is what the panel shows.
#[test]
fn a_created_object_reports_the_folder_it_landed_in() {
    App::test((), |mut app| async move {
        let bridge = drive_app(&mut app, Vec::new());

        let folder = bridge.update(&mut app, |_, ctx| {
            create(&json!({ "object_type": "folder", "name": "Deploys" }), ctx)
                .expect("a folder is creatable")
        });
        let folder_id = folder["id"].as_str().unwrap().to_owned();

        let body = serde_json::to_string(&Workflow::new("ship", "echo ship")).unwrap();
        let created = bridge.update(&mut app, |_, ctx| {
            create(
                &json!({
                    "object_type": "workflow",
                    "name": "ship",
                    "body": body,
                    "folder": folder_id,
                }),
                ctx,
            )
            .expect("drive.object.create succeeds")
        });

        assert_eq!(created["path"], json!(["Deploys"]));
    });
}

/// Refused rather than quietly reparented to the top level. An object that
/// lands somewhere other than where it was asked to go is worse than one that
/// does not land, because nobody is told.
#[test]
fn creating_into_something_that_is_not_a_folder_is_refused() {
    App::test((), |mut app| async move {
        let bridge = drive_app(&mut app, vec![workflow("deploy")]);
        let listed = bridge.update(&mut app, |_, ctx| list(&json!({}), ctx).unwrap());
        let workflow_id = listed["objects"][0]["id"].as_str().unwrap().to_owned();

        let body = serde_json::to_string(&Workflow::new("ship", "echo ship")).unwrap();
        let error = bridge
            .update(&mut app, |_, ctx| {
                create(
                    &json!({
                        "object_type": "workflow",
                        "name": "ship",
                        "body": body,
                        "folder": workflow_id,
                    }),
                    ctx,
                )
            })
            .expect_err("a workflow is not a folder");

        assert_eq!(error.code, ErrorCode::InvalidParams);
    });
}

/// Trashed, not deleted — and hidden from `list` unless asked for, so a caller
/// cannot act on something the user believes is gone.
#[test]
fn trashing_hides_an_object_from_list_but_keeps_it() {
    App::test((), |mut app| async move {
        let bridge = drive_app(&mut app, vec![workflow("deploy")]);
        let listed = bridge.update(&mut app, |_, ctx| list(&json!({}), ctx).unwrap());
        let id = listed["objects"][0]["id"].as_str().unwrap().to_owned();

        let trashed = bridge.update(&mut app, |_, ctx| {
            trash(&json!({ "id": id }), ctx).expect("drive.object.trash succeeds")
        });
        assert_eq!(trashed["trashed"], true);
        assert_eq!(trashed["name"], "deploy");

        let after = bridge.update(&mut app, |_, ctx| list(&json!({}), ctx).unwrap());
        assert_eq!(after["objects"].as_array().unwrap().len(), 0);
        assert_eq!(after["trashed_hidden"], 1);

        let including = bridge.update(&mut app, |_, ctx| {
            list(&json!({ "include_trashed": true }), ctx).unwrap()
        });
        let objects = including["objects"].as_array().unwrap();
        assert_eq!(objects.len(), 1, "the object is still there");
        assert_eq!(objects[0]["trashed"], true);
    });
}

/// Not an error: the object is in the state that was asked for, and reporting
/// a failure would make a retry look like a bug.
#[test]
fn trashing_an_already_trashed_object_reports_false_rather_than_failing() {
    App::test((), |mut app| async move {
        let bridge = drive_app(&mut app, vec![workflow("deploy")]);
        let listed = bridge.update(&mut app, |_, ctx| list(&json!({}), ctx).unwrap());
        let id = listed["objects"][0]["id"].as_str().unwrap().to_owned();

        bridge.update(&mut app, |_, ctx| {
            trash(&json!({ "id": &id }), ctx).unwrap()
        });
        let again = bridge.update(&mut app, |_, ctx| {
            trash(&json!({ "id": id }), ctx).expect("a second trash is not an error")
        });

        assert_eq!(again["trashed"], false);
    });
}

#[test]
fn an_unknown_id_is_a_missing_target() {
    App::test((), |mut app| async move {
        let bridge = drive_app(&mut app, Vec::new());

        let error = bridge
            .update(&mut app, |_, ctx| get(&json!({ "id": "Client-nope" }), ctx))
            .expect_err("there is no such object");

        assert_eq!(error.code, ErrorCode::MissingTarget);
    });
}

/// The body is the one part of a create that can be wrong in a way the caller
/// can fix, so the refusal says where to look rather than only that it failed.
#[test]
fn a_workflow_body_that_is_not_json_is_refused_with_somewhere_to_look() {
    App::test((), |mut app| async move {
        let bridge = drive_app(&mut app, Vec::new());

        let error = bridge
            .update(&mut app, |_, ctx| {
                create(
                    &json!({ "object_type": "workflow", "name": "ship", "body": "echo ship" }),
                    ctx,
                )
            })
            .expect_err("a workflow body must be JSON");

        assert_eq!(error.code, ErrorCode::InvalidParams);
        assert!(
            error
                .details
                .as_deref()
                .is_some_and(|details| details.contains("drive object get")),
            "{:?}",
            error.details
        );
    });
}

#[test]
fn a_folder_may_not_carry_a_body() {
    App::test((), |mut app| async move {
        let bridge = drive_app(&mut app, Vec::new());

        let error = bridge
            .update(&mut app, |_, ctx| {
                create(
                    &json!({ "object_type": "folder", "name": "Deploys", "body": "{}" }),
                    ctx,
                )
            })
            .expect_err("a folder has no body");

        assert_eq!(error.code, ErrorCode::InvalidParams);
    });
}

#[test]
fn the_type_filter_uses_the_same_words_list_prints() {
    App::test((), |mut app| async move {
        let bridge = drive_app(&mut app, vec![workflow("deploy")]);
        bridge.update(&mut app, |_, ctx| {
            create(&json!({ "object_type": "folder", "name": "Deploys" }), ctx).unwrap()
        });

        let workflows = bridge.update(&mut app, |_, ctx| {
            list(&json!({ "object_type": "workflow" }), ctx).unwrap()
        });
        let objects = workflows["objects"].as_array().unwrap();
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0]["object_type"], "workflow");

        let folders = bridge.update(&mut app, |_, ctx| {
            list(&json!({ "object_type": "folder" }), ctx).unwrap()
        });
        assert_eq!(folders["objects"].as_array().unwrap().len(), 1);
    });
}

/// A `folder_id` is a plain string column with no referential integrity behind
/// it, so a cycle is representable. `tree` carries the same guard for the same
/// reason: without one this recurses until the stack runs out.
#[test]
fn a_folder_cycle_yields_a_short_path_rather_than_a_hang() {
    App::test((), |mut app| async move {
        let bridge = drive_app(&mut app, Vec::new());

        let outer = bridge.update(&mut app, |_, ctx| {
            create(&json!({ "object_type": "folder", "name": "outer" }), ctx).unwrap()
        });
        let outer_id = outer["id"].as_str().unwrap().to_owned();
        let inner = bridge.update(&mut app, |_, ctx| {
            create(
                &json!({ "object_type": "folder", "name": "inner", "folder": &outer_id }),
                ctx,
            )
            .unwrap()
        });
        let inner_id = inner["id"].as_str().unwrap().to_owned();

        // Close the loop by hand: nothing in the action surface can do this,
        // which is exactly why the guard has to be tested from underneath.
        bridge.update(&mut app, |_, ctx| {
            let (objects, _) = snapshot(ctx);
            let outer = objects
                .iter()
                .find(|placed| placed.object.id.to_string() == outer_id)
                .expect("the outer folder exists");
            let cycled = PlacedObject {
                object: outer.object.clone(),
                parent: Some(
                    objects
                        .iter()
                        .find(|placed| placed.object.id.to_string() == inner_id)
                        .expect("the inner folder exists")
                        .object
                        .id,
                ),
            };
            apply::put(&cycled, ctx).expect("the store accepts a cycle; the reader must not hang");
        });

        let listed = bridge.update(&mut app, |_, ctx| list(&json!({}), ctx).unwrap());
        for object in listed["objects"].as_array().unwrap() {
            let path = object["path"].as_array().map(Vec::len).unwrap_or(0);
            assert!(path <= 2, "{object}");
        }
    });
}

fn drive_app(
    app: &mut App,
    objects: Vec<Box<dyn CloudObject>>,
) -> warpui::ModelHandle<LocalControlBridge> {
    app.add_singleton_model(|_| NetworkStatus::new());
    app.add_singleton_model(TeamTesterStatus::mock);
    app.add_singleton_model(|_| AuthStateProvider::new_logged_out_for_test());
    app.add_singleton_model(UserWorkspaces::default_mock);
    app.add_singleton_model(SyncQueue::mock);
    app.add_singleton_model(|_| CloudModel::new(None, objects, None));
    app.add_singleton_model(UpdateManager::mock);
    app.add_singleton_model(|_| SettingsManager::default());
    app.update(crate::settings::init_and_register_user_preferences);
    app.add_singleton_model(WorkflowAliases::new_with_defaults);
    // Unused by these actions — the object store is not the mirror — but
    // `LocalControlBridge` reads the group at construction.
    app.add_singleton_model(|_| LocalDriveSyncSettings {
        local_drive_sync_path: LocalDriveSyncPath::new(Some(String::new())),
    });
    app.add_singleton_model(LocalControlBridge::new)
}

fn workflow(name: &str) -> Box<dyn CloudObject> {
    Box::new(CloudWorkflow::new(
        SyncId::ClientId(ClientId::new()),
        CloudWorkflowModel::new(Workflow::new(name, "echo hi")),
        CloudObjectMetadata::mock(),
        CloudObjectPermissions {
            owner: crate::fork::local_drive_owner()
                .expect("fork policy provides a local drive owner"),
            permissions_last_updated_ts: None,
            anyone_with_link: None,
            guests: Vec::new(),
        },
    ))
}
