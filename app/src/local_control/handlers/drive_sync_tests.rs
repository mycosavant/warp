use tempfile::TempDir;
use warpui::App;

use super::*;
use crate::auth::AuthStateProvider;
use crate::cloud_object::model::persistence::CloudModel;
use crate::cloud_object::{CloudObject, CloudObjectMetadata, CloudObjectPermissions};
use crate::server::ids::{ClientId, SyncId};
use crate::settings::LocalDriveSyncPath;
use crate::workflows::workflow::Workflow;
use crate::workflows::{CloudWorkflow, CloudWorkflowModel};
use crate::workspaces::user_workspaces::UserWorkspaces;

/// The trigger, end to end: a store, an action, files on a disk.
#[test]
fn an_export_writes_the_drive_into_the_configured_directory() {
    App::test((), |mut app| async move {
        let root = TempDir::new().unwrap();
        let bridge = drive_app(&mut app, Some(root.path()), vec![workflow("deploy")]);

        let result = bridge.update(&mut app, |_, ctx| {
            export(ctx).expect("drive.sync.export succeeds")
        });

        assert_eq!(result["written"], 1);
        assert_eq!(result["unchanged"], 0);
        assert_eq!(result["not_personal"], 0);
        assert_eq!(result["path"], root.path().display().to_string());

        let files: Vec<_> = std::fs::read_dir(root.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(files.len(), 1, "{files:?}");
        assert!(files[0].starts_with("deploy-"), "{files:?}");
    });
}

/// The property that makes this usable as a git repository, now reachable from
/// the outside: run the action twice and the second run touches nothing.
#[test]
fn a_second_export_writes_nothing() {
    App::test((), |mut app| async move {
        let root = TempDir::new().unwrap();
        let bridge = drive_app(&mut app, Some(root.path()), vec![workflow("deploy")]);

        bridge.update(&mut app, |_, ctx| export(ctx).unwrap());
        let second = bridge.update(&mut app, |_, ctx| export(ctx).unwrap());

        assert_eq!(second["written"], 0);
        assert_eq!(second["unchanged"], 1);
        assert_eq!(second["removed_files"], 0);
    });
}

/// `status` is the action you run to find out why an export will not run, so it
/// reports an unset path rather than refusing — and it must not create the
/// directory as a side effect of being asked about it.
#[test]
fn status_reports_the_destination_without_writing_anything() {
    App::test((), |mut app| async move {
        let root = TempDir::new().unwrap();
        let unwritten = root.path().join("not-yet");
        let bridge = drive_app(&mut app, Some(&unwritten), vec![workflow("deploy")]);

        let result = bridge.update(&mut app, |_, ctx| {
            status(ctx).expect("drive.sync.status succeeds")
        });

        assert_eq!(result["path"], unwritten.display().to_string());
        assert_eq!(result["path_exists"], false);
        assert_eq!(result["objects"], 1);
        assert!(!unwritten.exists(), "status created the directory");
    });
}

#[test]
fn status_with_no_path_configured_still_reports_the_store() {
    App::test((), |mut app| async move {
        let bridge = drive_app(&mut app, None, vec![workflow("deploy")]);

        let result = bridge.update(&mut app, |_, ctx| status(ctx).unwrap());

        assert!(result.get("path").is_none(), "{result}");
        assert_eq!(result["path_exists"], false);
        assert_eq!(result["objects"], 1);
    });
}

/// Every guard, and each one has to name itself: an export that fails with
/// "internal error" leaves the user with a setting and no idea which part of it
/// is wrong.
#[test]
fn an_export_refuses_a_destination_it_should_not_write_to() {
    let root_path = if cfg!(windows) { "C:\\" } else { "/" };
    let cases = [
        (None, "is not set"),
        (Some("relative/path"), "absolute"),
        (Some(root_path), "root"),
    ];

    for (path, expected) in cases {
        App::test((), move |mut app| async move {
            let bridge = drive_app(&mut app, path.map(std::path::Path::new), vec![]);

            let err = bridge.update(&mut app, |_, ctx| {
                export(ctx).expect_err("this destination must be refused")
            });

            assert_eq!(err.code, ErrorCode::InvalidRequest);
            assert!(
                format!("{} {}", err.message, err.details.unwrap_or_default()).contains(expected),
                "expected an error mentioning {expected:?} for {path:?}"
            );
        });
    }
}

/// A path pointing at a file rather than a directory. Caught before anything is
/// read or written, because the alternative is a confusing failure part-way
/// through a prune.
#[test]
fn an_export_refuses_a_path_that_is_not_a_directory() {
    App::test((), |mut app| async move {
        let root = TempDir::new().unwrap();
        let file = root.path().join("notes.txt");
        std::fs::write(&file, "not a directory\n").unwrap();
        let bridge = drive_app(&mut app, Some(&file), vec![]);

        let err = bridge.update(&mut app, |_, ctx| export(ctx).unwrap_err());

        assert_eq!(err.code, ErrorCode::InvalidRequest);
        assert!(err.message.contains("directory"), "{}", err.message);
    });
}

fn drive_app(
    app: &mut App,
    path: Option<&std::path::Path>,
    objects: Vec<Box<dyn CloudObject>>,
) -> warpui::ModelHandle<LocalControlBridge> {
    let configured = path
        .map(|path| path.display().to_string())
        .unwrap_or_default();

    app.add_singleton_model(|_| AuthStateProvider::new_logged_out_for_test());
    app.add_singleton_model(UserWorkspaces::default_mock);
    app.add_singleton_model(|_| CloudModel::new(None, objects, None));
    app.add_singleton_model(|_| LocalDriveSyncSettings {
        local_drive_sync_path: LocalDriveSyncPath::new(Some(configured)),
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

/// `WARP_FORK_POLICY=0` restores upstream behaviour without a rebuild, and this
/// action belongs entirely to fork policy. Asserted against the policy rather
/// than by setting the variable, for the reason recorded in `fork_tests`: it is
/// process-wide, so a test that sets and unsets it races every other test in
/// the binary. An earlier version of a test elsewhere did exactly that and
/// silently re-enabled fork policy mid-run for whatever ran alongside it.
#[test]
fn the_export_action_belongs_to_fork_policy() {
    App::test((), |mut app| async move {
        let root = TempDir::new().unwrap();
        let bridge = drive_app(&mut app, Some(root.path()), vec![workflow("deploy")]);

        let result = bridge.update(&mut app, |_, ctx| export(ctx));

        assert_eq!(
            result.is_ok(),
            crate::fork::is_active(),
            "the export must work exactly when fork policy is on"
        );
    });
}
