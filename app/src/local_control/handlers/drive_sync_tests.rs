use tempfile::TempDir;
use warpui::App;

use super::*;
use crate::auth::AuthStateProvider;
use crate::cloud_object::model::persistence::CloudModel;
use crate::cloud_object::{CloudObject, CloudObjectMetadata, CloudObjectPermissions};
use crate::drive::local_sync::format::{Alias, PortableObject};
use crate::network::NetworkStatus;
use crate::server::cloud_objects::update_manager::UpdateManager;
use crate::server::ids::{ClientId, SyncId};
use crate::server::sync_queue::SyncQueue;
use crate::settings::LocalDriveSyncPath;
use crate::workflows::aliases::WorkflowAliases;
use crate::workflows::workflow::Workflow;
use crate::workflows::{CloudWorkflow, CloudWorkflowModel};
use crate::workspaces::team_tester::TeamTesterStatus;
use crate::workspaces::user_workspaces::UserWorkspaces;
use settings::SettingsManager;

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

/// T4.4e, and the reason it is a refusal rather than a skip.
///
/// A conflicted file does not parse, so the object it describes is absent from
/// the tree — and absence is precisely how this import is told an object was
/// deleted. Skipping the file would trash the one object the user is in the
/// middle of merging. So the import stops, and the store is untouched.
#[test]
fn an_import_refuses_a_tree_with_an_unresolved_conflict() {
    App::test((), |mut app| async move {
        let root = TempDir::new().unwrap();
        let bridge = drive_app(&mut app, Some(root.path()), vec![workflow("deploy")]);
        bridge.update(&mut app, |_, ctx| export(ctx).unwrap());
        conflict_the_only_file(root.path());

        let err = bridge.update(&mut app, |_, ctx| {
            import(ctx).expect_err("a half-merged tree must be refused")
        });

        assert_eq!(err.code, ErrorCode::InvalidRequest);
        assert!(err.message.contains("merge conflicts"), "{}", err.message);
        assert!(
            err.details.unwrap_or_default().contains("(deploy)"),
            "the refusal must name the object, not just the file"
        );
        app.read(|ctx| {
            let (objects, _) = snapshot(ctx);
            assert_eq!(objects.len(), 1, "the object is still there");
            assert!(
                objects[0].object.trashed_ts.is_none(),
                "a conflicted file was read as a deletion"
            );
        });
    });
}

/// The same condition in the other direction, and it must not read as an
/// internal error: nothing is broken, the user is simply mid-merge.
#[test]
fn an_export_refuses_to_overwrite_a_half_merged_file() {
    App::test((), |mut app| async move {
        let root = TempDir::new().unwrap();
        let bridge = drive_app(&mut app, Some(root.path()), vec![workflow("deploy")]);
        bridge.update(&mut app, |_, ctx| export(ctx).unwrap());
        conflict_the_only_file(root.path());

        let err = bridge.update(&mut app, |_, ctx| {
            export(ctx).expect_err("a half-merged file must not be overwritten")
        });

        assert_eq!(err.code, ErrorCode::InvalidRequest);
        assert!(err.message.contains("merge conflicts"), "{}", err.message);
    });
}

/// An unresolved merge is the only condition that stops both directions, so the
/// action whose job is answering "why will this not run" has to see it.
#[test]
fn status_names_the_files_with_unresolved_conflicts() {
    App::test((), |mut app| async move {
        let root = TempDir::new().unwrap();
        let bridge = drive_app(&mut app, Some(root.path()), vec![workflow("deploy")]);
        bridge.update(&mut app, |_, ctx| export(ctx).unwrap());

        let clean = bridge.update(&mut app, |_, ctx| status(ctx).unwrap());
        conflict_the_only_file(root.path());
        let conflicted = bridge.update(&mut app, |_, ctx| status(ctx).unwrap());

        assert!(clean.get("conflicted").is_none(), "{clean}");
        assert_eq!(conflicted["conflicted"].as_array().unwrap().len(), 1);
        assert!(
            conflicted["conflicted"][0]
                .as_str()
                .unwrap()
                .contains("(deploy)"),
            "{conflicted}"
        );
    });
}

/// T4.4g through the action surface: an alias in a file becomes an alias in
/// settings, and the reply says how many.
#[test]
fn an_alias_in_a_file_reaches_settings_through_the_actions() {
    App::test((), |mut app| async move {
        let root = TempDir::new().unwrap();
        let bridge = drive_app(&mut app, Some(root.path()), vec![workflow("deploy")]);
        bridge.update(&mut app, |_, ctx| export(ctx).unwrap());

        let path = std::fs::read_dir(root.path())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let mut portable =
            PortableObject::from_file_contents(&std::fs::read_to_string(&path).unwrap()).unwrap();
        portable.aliases.push(Alias {
            alias: "dep".to_owned(),
            env_vars: None,
            arguments: None,
        });
        std::fs::write(&path, portable.to_file_contents().unwrap()).unwrap();

        let result = bridge.update(&mut app, |_, ctx| import(ctx).unwrap());

        assert_eq!(result["aliases_set"], 1);
        app.read(|ctx| {
            let aliases = WorkflowAliases::as_ref(ctx).get_all_aliases();
            assert_eq!(aliases.len(), 1);
            assert_eq!(aliases[0].alias, "dep");
        });
    });
}

/// Leaves the exported file half-merged, the way `git pull` would.
fn conflict_the_only_file(root: &std::path::Path) {
    let path = std::fs::read_dir(root)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let ours = std::fs::read_to_string(&path).unwrap();
    let theirs = ours.replace("echo hi", "echo elsewhere");
    std::fs::write(
        &path,
        format!("<<<<<<< HEAD\n{ours}=======\n{theirs}>>>>>>> theirs\n"),
    )
    .unwrap();
}

fn drive_app(
    app: &mut App,
    path: Option<&std::path::Path>,
    objects: Vec<Box<dyn CloudObject>>,
) -> warpui::ModelHandle<LocalControlBridge> {
    let configured = path
        .map(|path| path.display().to_string())
        .unwrap_or_default();

    app.add_singleton_model(|_| NetworkStatus::new());
    app.add_singleton_model(TeamTesterStatus::mock);
    app.add_singleton_model(|_| AuthStateProvider::new_logged_out_for_test());
    app.add_singleton_model(UserWorkspaces::default_mock);
    app.add_singleton_model(SyncQueue::mock);
    app.add_singleton_model(|_| CloudModel::new(None, objects, None));
    // `apply` persists through the update manager and routes deletions through
    // its trash path, so an import needs it registered where an export does not.
    app.add_singleton_model(UpdateManager::mock);
    // `WorkflowAliases` is a private setting, so writing one goes through the
    // preferences store as well as the manager.
    app.add_singleton_model(|_| SettingsManager::default());
    app.update(crate::settings::init_and_register_user_preferences);
    app.add_singleton_model(WorkflowAliases::new_with_defaults);
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

/// The export destination must stay something only the user can change.
///
/// `setting.set` is gated by an allowlist, and this key is deliberately not on
/// it: an agent can ask for an export but cannot decide where a pruning
/// exporter points. That is the entire argument for the destination being a
/// setting rather than a parameter, and adding one line to that allowlist would
/// undo it without touching anything in this file.
#[test]
fn the_mirror_path_cannot_be_repointed_through_local_control() {
    let key = LocalDriveSyncPath::toml_path().expect("the mirror path is user-visible");

    assert!(
        !crate::local_control::handlers::settings_surfaces::ALLOWLISTED_SETTING_KEYS.contains(&key),
        "{key} became settable through local control, which lets a caller \
         choose the directory drive.sync.export prunes"
    );
}

/// The whole loop through the action surface: export, edit the file the way a
/// `git pull` would have, import, and see the store follow.
#[test]
fn an_edit_on_disk_reaches_the_store_through_the_actions() {
    App::test((), |mut app| async move {
        let root = TempDir::new().unwrap();
        let bridge = drive_app(&mut app, Some(root.path()), vec![workflow("deploy")]);

        bridge.update(&mut app, |_, ctx| export(ctx).unwrap());

        let file = std::fs::read_dir(root.path())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let edited = std::fs::read_to_string(&file)
            .unwrap()
            .replace("echo hi", "echo edited");
        std::fs::write(&file, edited).unwrap();

        let result = bridge.update(&mut app, |_, ctx| {
            import(ctx).expect("drive.sync.import succeeds")
        });

        assert_eq!(result["updated"], 1);
        assert_eq!(result["created"], 0);
        assert_eq!(result["trashed"], 0);
    });
}

/// An import that would trash the drive because it was pointed somewhere with
/// nothing in it. Reported as a bad request rather than an internal error,
/// because it is the caller's path that is wrong.
#[test]
fn an_import_from_an_empty_directory_is_refused() {
    App::test((), |mut app| async move {
        let root = TempDir::new().unwrap();
        let bridge = drive_app(&mut app, Some(root.path()), vec![workflow("deploy")]);

        let err = bridge.update(&mut app, |_, ctx| {
            import(ctx).expect_err("an empty tree must be refused")
        });

        assert_eq!(err.code, ErrorCode::InvalidRequest);
    });
}

/// A directory that is not there at all, told apart from one that is empty:
/// the first is a mistyped setting, the second is a drive someone emptied.
#[test]
fn an_import_from_a_missing_directory_says_so() {
    App::test((), |mut app| async move {
        let root = TempDir::new().unwrap();
        let missing = root.path().join("never-created");
        let bridge = drive_app(&mut app, Some(&missing), vec![workflow("deploy")]);

        let err = bridge.update(&mut app, |_, ctx| import(ctx).unwrap_err());

        assert_eq!(err.code, ErrorCode::InvalidRequest);
        assert!(err.message.contains("nothing at"), "{}", err.message);
    });
}
