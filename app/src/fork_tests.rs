use warpui::{App, ModelHandle, SingletonEntity};

use super::*;
use crate::auth::AuthStateProvider;
use crate::cloud_object::model::actions::ObjectActions;
use crate::cloud_object::model::persistence::CloudModel;
use crate::cloud_object::{
    CloudObject, CloudObjectEventEntrypoint, CloudObjectMetadata, CloudObjectPermissions,
    ObjectType, Space,
};
use crate::drive::settings::WarpDriveSettings;
use crate::network::NetworkStatus;
use crate::server::cloud_objects::update_manager::{InitiatedBy, UpdateManager};
use crate::server::ids::{ClientId, SyncId};
use crate::server::sync_queue::{QueueItem, SyncQueue};
use crate::settings::AISettings;
use crate::settings::ai::FocusedTerminalInfo;
use crate::workflows::workflow::Workflow;
use crate::workflows::{CloudWorkflow, CloudWorkflowModel};
use crate::workspace::view::left_panel::{ToolPanelAvailability, ToolPanelView};
use crate::workspaces::team_tester::TeamTesterStatus;
use crate::workspaces::user_workspaces::UserWorkspaces;

/// The local owner is a fixed constant rather than a per-install identity, and
/// this is the reason: `UserWorkspaces::owner_to_space` puts an object in
/// [`Space::Personal`] only when its owner matches the *current* user, and in
/// `Space::Shared` otherwise. A per-machine uid would therefore file a store
/// that moved machines under "Shared with me" — which is precisely what T4.4's
/// git-backed sync exists to do.
#[test]
fn the_local_drive_owner_is_stable_across_processes() {
    let first = local_drive_owner().expect("fork policy provides a local drive owner");
    let second = local_drive_owner().expect("fork policy provides a local drive owner");

    assert_eq!(
        first, second,
        "a local drive owner that varies would move objects into the Shared space"
    );
    assert!(is_local_drive_owner(&first));
}

/// The sentinel must not be mistaken for a real account. Warp user ids are
/// Firebase uids, so this is about the recogniser, not about collision odds.
#[test]
fn a_real_account_is_not_a_local_drive_owner() {
    let real = Owner::User {
        user_uid: UserUid::new("kK3sBqL9vXdF2mN7pR1tY4wZ8cJ6"),
    };

    assert!(!is_local_drive_owner(&real));
}

/// The one function that makes Warp Drive writable without an account. Every
/// create path resolves an [`Owner`] through here and bails on `None`, so
/// without this the drive is readable and editable but cannot be added to.
#[test]
fn personal_drive_falls_back_to_the_local_owner_when_logged_out() {
    App::test((), |app| async move {
        app.add_singleton_model(|_| AuthStateProvider::new_logged_out_for_test());
        app.add_singleton_model(UserWorkspaces::default_mock);

        app.read(|ctx| {
            let owner = UserWorkspaces::as_ref(ctx)
                .personal_drive(ctx)
                .expect("fork policy keeps Warp Drive writable with no account");

            assert!(
                is_local_drive_owner(&owner),
                "a logged-out personal drive must be owned by the local sentinel"
            );
        });
    });
}

/// A real account still wins. Fork policy adds a fallback; it does not
/// substitute for an identity the user actually has.
#[test]
fn personal_drive_prefers_a_real_account_over_the_local_owner() {
    App::test((), |app| async move {
        app.add_singleton_model(|_| AuthStateProvider::new_for_test());
        app.add_singleton_model(UserWorkspaces::default_mock);

        app.read(|ctx| {
            let owner = UserWorkspaces::as_ref(ctx)
                .personal_drive(ctx)
                .expect("a logged-in user has a personal drive");

            assert!(
                !is_local_drive_owner(&owner),
                "fork policy must not shadow the signed-in user's own drive"
            );
        });
    });
}

/// Upstream sets `has_initial_load` only at the end of a *successful server
/// fetch*, which needs an account. 24 call sites across 15 files await it — the
/// Warp Drive spinner, `warp mcp list`, execution profiles, environments — so
/// without this they wait forever over a store that is already fully loaded.
#[test]
fn the_local_store_counts_as_the_initial_load_when_logged_out() {
    App::test((), |app| async move {
        app.add_singleton_model(|_| NetworkStatus::new());
        app.add_singleton_model(TeamTesterStatus::mock);
        app.add_singleton_model(|_| AuthStateProvider::new_logged_out_for_test());
        app.add_singleton_model(SyncQueue::mock);
        let update_manager = app.add_singleton_model(UpdateManager::mock);

        app.read(|ctx| {
            assert!(
                update_manager.as_ref(ctx).has_completed_initial_load(),
                "everything that waits on the initial load would hang without an account"
            );
        });
    });
}

/// With an account the condition must stay upstream's: it is only satisfied by
/// a real fetch, so cloud objects are never read before they have arrived.
#[test]
fn a_logged_in_user_still_waits_for_a_real_fetch() {
    App::test((), |app| async move {
        app.add_singleton_model(|_| NetworkStatus::new());
        app.add_singleton_model(TeamTesterStatus::mock);
        app.add_singleton_model(|_| AuthStateProvider::new_for_test());
        app.add_singleton_model(SyncQueue::mock);
        let update_manager = app.add_singleton_model(UpdateManager::mock);

        app.read(|ctx| {
            assert!(
                !update_manager.as_ref(ctx).has_completed_initial_load(),
                "resolving this early would let stale local data stand in for the server's"
            );
        });
    });
}

/// The hard guarantee behind local-first. Upstream already never *sends* these
/// while logged out, because `should_dequeue` is only set after a successful
/// fetch — but the item survives in the queue, so adding an account later would
/// start dequeueing and push objects owned by a uid the server has never seen.
/// Dropping it at the door is what makes "no leak" a property rather than a
/// coincidence of ordering.
#[test]
fn nothing_is_queued_for_the_server_while_logged_out() {
    App::test((), |mut app| async move {
        app.add_singleton_model(|_| AuthStateProvider::new_logged_out_for_test());
        let sync_queue = app.add_singleton_model(SyncQueue::mock);

        app.update(|ctx| {
            SyncQueue::handle(ctx).update(ctx, |queue, ctx| {
                queue.enqueue(local_object_creation(), ctx);
            });
        });

        app.read(|ctx| {
            assert!(
                sync_queue.as_ref(ctx).queue().is_empty(),
                "a locally owned object must never reach the sync queue"
            );
        });
    });
}

/// The same call with an account behind it is ordinary upstream behaviour, and
/// has to stay that way — this guard is about the account-free case only.
#[test]
fn a_logged_in_user_still_queues_normally() {
    App::test((), |mut app| async move {
        app.add_singleton_model(|_| AuthStateProvider::new_for_test());
        let sync_queue = app.add_singleton_model(SyncQueue::mock);

        app.update(|ctx| {
            SyncQueue::handle(ctx).update(ctx, |queue, ctx| {
                queue.enqueue(local_object_creation(), ctx);
            });
        });

        app.read(|ctx| {
            assert_eq!(
                sync_queue.as_ref(ctx).queue().len(),
                1,
                "fork policy must not stop a signed-in user syncing"
            );
        });
    });
}

/// The fork disabled Warp Drive with its own T1 change, and only a live run
/// caught it. `is_warp_drive_available` is
/// `!SkipFirebaseAnonymousUser.is_enabled() || !is_anonymous_or_logged_out()`,
/// and `SkipFirebaseAnonymousUser` is in [`FORCE_ENABLED`] — so under fork
/// policy the first clause is false, being account-free makes the second false,
/// and the drive switches itself off. Everything T4.2 fixed underneath it was
/// unreachable: `surface.warp_drive.open` answered "Warp Drive is disabled".
#[test]
fn warp_drive_is_available_without_an_account() {
    let _guard = FeatureFlag::SkipFirebaseAnonymousUser.override_enabled(true);

    App::test((), |app| async move {
        app.add_singleton_model(|_| AuthStateProvider::new_logged_out_for_test());

        app.read(|ctx| {
            assert!(
                WarpDriveSettings::is_warp_drive_available(ctx),
                "fork policy must not hide a drive that works fine without an account"
            );
        });
    });
}

/// The flag is what makes the check bite, so pin that too — if
/// `SkipFirebaseAnonymousUser` ever leaves `FORCE_ENABLED` this test should
/// start passing for a different reason, and the one above would go quiet.
#[test]
fn the_drive_availability_gate_depends_on_the_anonymous_user_flag() {
    assert!(
        FORCE_ENABLED.contains(&FeatureFlag::SkipFirebaseAnonymousUser),
        "the availability gate above is only reachable while this flag is forced on"
    );
}

/// The same shape as the drive gate, one panel over, and it outlived its own
/// justification. "Create an account and enable AI to access your conversation
/// history" was true while the only agent was Warp's, because the history was
/// the server's. T5 made conversations local: they are written to the local
/// database and read back at startup, and
/// `AgentConversationsModel::unfiltered_entries` ends with a loop over
/// `get_local_conversations_metadata` that touches no server. The panel was
/// refusing to show a history that exists, complete, on this disk.
#[test]
fn agent_conversations_are_available_without_an_account() {
    let _guard = FeatureFlag::SkipFirebaseAnonymousUser.override_enabled(true);

    App::test((), |app| async move {
        app.add_singleton_model(|_| AuthStateProvider::new_logged_out_for_test());
        app.add_singleton_model(UserWorkspaces::default_mock);
        app.add_singleton_model(FocusedTerminalInfo::new);
        app.add_singleton_model(AISettings::new_with_defaults);

        app.read(|ctx| {
            assert_eq!(
                ToolPanelView::ConversationListView.availability(ctx),
                ToolPanelAvailability::Available,
                "the conversation list is local, so an account-free user has one to read"
            );
        });
    });
}

/// The writing side and the reading side have to agree on who "I" am.
///
/// `personal_drive` stamps new objects with the local sentinel; `owner_to_space`
/// decides which space an object belongs to. Account-free, `user_id()` is `None`
/// while the sentinel is not, so comparing against `user_id()` made every
/// locally-created object fail the match and file itself under "Shared with me".
///
/// Every unit test passed with that bug in place, because they covered the two
/// sides separately. It took creating a workflow in the running Windows build
/// and restarting to see it. This test is the pair of them together.
#[test]
fn a_locally_created_object_belongs_to_the_personal_space() {
    let _guard = FeatureFlag::SharedWithMe.override_enabled(true);

    App::test((), |app| async move {
        app.add_singleton_model(|_| AuthStateProvider::new_logged_out_for_test());
        app.add_singleton_model(UserWorkspaces::default_mock);

        app.read(|ctx| {
            let workspaces = UserWorkspaces::as_ref(ctx);
            let owner = workspaces
                .personal_drive(ctx)
                .expect("fork policy provides a local drive owner");

            assert_eq!(
                workspaces.owner_to_space(owner, ctx),
                Space::Personal,
                "an object this client just created must not come back as someone else's"
            );
        });
    });
}

/// The same round trip with an account, which is upstream's own behaviour and
/// has to keep working — the change routes both sides through one seam rather
/// than special-casing the account-free path.
#[test]
fn a_signed_in_users_own_object_still_belongs_to_the_personal_space() {
    let _guard = FeatureFlag::SharedWithMe.override_enabled(true);

    App::test((), |app| async move {
        app.add_singleton_model(|_| AuthStateProvider::new_for_test());
        app.add_singleton_model(UserWorkspaces::default_mock);

        app.read(|ctx| {
            let workspaces = UserWorkspaces::as_ref(ctx);
            let owner = workspaces
                .personal_drive(ctx)
                .expect("a signed-in user has a personal drive");

            assert_eq!(workspaces.owner_to_space(owner, ctx), Space::Personal);
        });
    });
}

/// Someone else's object must still read as shared, or the fix would have
/// quietly turned "Shared with me" into a second personal space.
#[test]
fn another_users_object_is_still_shared() {
    let _guard = FeatureFlag::SharedWithMe.override_enabled(true);

    App::test((), |app| async move {
        app.add_singleton_model(|_| AuthStateProvider::new_logged_out_for_test());
        app.add_singleton_model(UserWorkspaces::default_mock);

        app.read(|ctx| {
            let someone_else = Owner::User {
                user_uid: UserUid::new("kK3sBqL9vXdF2mN7pR1tY4wZ8cJ6"),
            };

            assert_eq!(
                UserWorkspaces::as_ref(ctx).owner_to_space(someone_else, ctx),
                Space::Shared
            );
        });
    });
}

/// Deleting a workflow you made yourself. It did nothing at all before this.
///
/// `UpdateManager::trash_object` opens by requiring a server id, and
/// account-free no object has one, so the Drive panel's Trash item and
/// `WorkflowAction::Trash` both returned immediately. Found by reading the
/// trash path while designing T4.4f, whose deletion rule depends on it.
#[test]
fn an_object_created_without_an_account_can_be_trashed() {
    App::test((), |mut app| async move {
        app.add_singleton_model(|_| NetworkStatus::new());
        app.add_singleton_model(TeamTesterStatus::mock);
        app.add_singleton_model(|_| AuthStateProvider::new_logged_out_for_test());
        app.add_singleton_model(SyncQueue::mock);

        let workflow = local_workflow();
        let type_and_id = workflow.cloud_object_type_and_id();
        let uid = workflow.sync_id().uid();
        app.add_singleton_model(|_| CloudModel::new(None, vec![workflow], None));
        let update_manager = app.add_singleton_model(UpdateManager::mock);

        update_manager.update(&mut app, |update_manager, ctx| {
            update_manager.trash_object(type_and_id, ctx);
        });

        app.read(|ctx| {
            let cloud_model = CloudModel::as_ref(ctx);
            let object = cloud_model
                .get_by_uid(&uid)
                .expect("the object is still there");

            assert!(
                object.metadata().trashed_ts.is_some(),
                "a locally-created object could not be deleted"
            );
            assert!(
                !object
                    .metadata()
                    .pending_changes_statuses
                    .has_pending_metadata_change,
                "nothing is pending: there is no server to wait for"
            );
        });
    });
}

/// The same call with an account still goes to the server, and upstream's
/// no-server-id guard still applies, because a signed-in user's objects really
/// do live somewhere else too.
#[test]
fn a_signed_in_user_still_trashes_through_the_server() {
    App::test((), |mut app| async move {
        app.add_singleton_model(|_| NetworkStatus::new());
        app.add_singleton_model(TeamTesterStatus::mock);
        app.add_singleton_model(|_| AuthStateProvider::new_for_test());
        app.add_singleton_model(SyncQueue::mock);

        let workflow = local_workflow();
        let type_and_id = workflow.cloud_object_type_and_id();
        let uid = workflow.sync_id().uid();
        app.add_singleton_model(|_| CloudModel::new(None, vec![workflow], None));
        let update_manager = app.add_singleton_model(UpdateManager::mock);

        update_manager.update(&mut app, |update_manager, ctx| {
            update_manager.trash_object(type_and_id, ctx);
        });

        app.read(|ctx| {
            assert!(
                CloudModel::as_ref(ctx)
                    .get_by_uid(&uid)
                    .expect("the object is still there")
                    .metadata()
                    .trashed_ts
                    .is_none(),
                "fork policy must not change what a signed-in user's delete does"
            );
        });
    });
}

/// T4.7's headline: something in the trash can be got rid of.
///
/// `empty_trash` asks the server and only removes anything locally once the
/// answer comes back, so account-free the trash filled up and stayed full —
/// with no other way to empty it, since "Delete forever" is not drawn either.
#[test]
fn emptying_the_trash_without_an_account_actually_empties_it() {
    App::test((), |mut app| async move {
        let mut workflow = local_workflow();
        workflow.metadata_mut().trashed_ts = Some(chrono::Utc::now().into());
        let uid = workflow.sync_id().uid();
        let update_manager = drive_app(&mut app, logged_out(), vec![workflow]);

        update_manager.update(&mut app, |update_manager, ctx| {
            update_manager.empty_trash(Space::Personal, ctx);
        });

        app.read(|ctx| {
            assert!(
                CloudModel::as_ref(ctx).get_by_uid(&uid).is_none(),
                "a trashed object survived emptying the trash"
            );
        });
    });
}

/// Trashing a folder marks only the folder, so its contents carry no
/// `trashed_ts` of their own. The server's answer includes them; read the
/// trashed set alone and they are left behind, in memory and in SQLite,
/// pointing at a parent that no longer exists.
#[test]
fn emptying_the_trash_takes_what_is_inside_a_trashed_folder() {
    App::test((), |mut app| async move {
        let mut folder = local_folder();
        folder.metadata_mut().trashed_ts = Some(chrono::Utc::now().into());
        let folder_id = folder.sync_id();

        let mut inside = local_workflow();
        inside.metadata_mut().folder_id = Some(folder_id);
        let inside_uid = inside.sync_id().uid();

        let update_manager = drive_app(&mut app, logged_out(), vec![folder, inside]);

        update_manager.update(&mut app, |update_manager, ctx| {
            update_manager.empty_trash(Space::Personal, ctx);
        });

        app.read(|ctx| {
            assert!(
                CloudModel::as_ref(ctx).get_by_uid(&inside_uid).is_none(),
                "the workflow inside the deleted folder was orphaned rather than deleted"
            );
        });
    });
}

/// The other half of the same call, and the one worth being sure about: this is
/// the most destructive thing the fork can do to a drive.
#[test]
fn emptying_the_trash_leaves_everything_that_is_not_in_it() {
    App::test((), |mut app| async move {
        let mut trashed = local_workflow();
        trashed.metadata_mut().trashed_ts = Some(chrono::Utc::now().into());
        let kept = local_workflow();
        let kept_uid = kept.sync_id().uid();
        let update_manager = drive_app(&mut app, logged_out(), vec![trashed, kept]);

        update_manager.update(&mut app, |update_manager, ctx| {
            update_manager.empty_trash(Space::Personal, ctx);
        });

        app.read(|ctx| {
            assert!(
                CloudModel::as_ref(ctx).get_by_uid(&kept_uid).is_some(),
                "emptying the trash deleted an object that was not in it"
            );
        });
    });
}

/// "Delete forever" on a single object — the same gap, and the path every
/// feature that deletes its own objects goes through: environments, MCP
/// servers, AI rules.
#[test]
fn an_object_can_be_deleted_for_good_one_at_a_time() {
    App::test((), |mut app| async move {
        let workflow = local_workflow();
        let type_and_id = workflow.cloud_object_type_and_id();
        let uid = workflow.sync_id().uid();
        let update_manager = drive_app(&mut app, logged_out(), vec![workflow]);

        update_manager.update(&mut app, |update_manager, ctx| {
            update_manager.delete_object_by_user(type_and_id, ctx);
        });

        app.read(|ctx| {
            assert!(CloudModel::as_ref(ctx).get_by_uid(&uid).is_none());
        });
    });
}

/// A trash you cannot restore from is a delete, and T4.4f's whole safety
/// argument — "an object missing from the tree is trashed, which is
/// recoverable" — rests on this working.
#[test]
fn an_object_in_the_trash_can_be_restored() {
    App::test((), |mut app| async move {
        let mut workflow = local_workflow();
        workflow.metadata_mut().trashed_ts = Some(chrono::Utc::now().into());
        let type_and_id = workflow.cloud_object_type_and_id();
        let uid = workflow.sync_id().uid();
        let update_manager = drive_app(&mut app, logged_out(), vec![workflow]);

        update_manager.update(&mut app, |update_manager, ctx| {
            update_manager.untrash_object(type_and_id, ctx);
        });

        app.read(|ctx| {
            let object = CloudModel::as_ref(ctx).get_by_uid(&uid).unwrap();
            assert!(
                object.metadata().trashed_ts.is_none(),
                "the object could not be got out of the trash"
            );
            assert!(
                !object.metadata().pending_changes_statuses.pending_untrash,
                "nothing is pending: there is no server to wait for"
            );
        });
    });
}

/// Restoring into a folder that is itself in the trash restores the object
/// *into* the trash, where the user cannot see it and has no way to find out
/// where it went. The server moves it to the root instead; with no server,
/// this does.
#[test]
fn restoring_an_object_whose_folder_is_in_the_trash_puts_it_at_the_root() {
    App::test((), |mut app| async move {
        let mut folder = local_folder();
        folder.metadata_mut().trashed_ts = Some(chrono::Utc::now().into());
        let folder_id = folder.sync_id();

        let mut inside = local_workflow();
        inside.metadata_mut().trashed_ts = Some(chrono::Utc::now().into());
        inside.metadata_mut().folder_id = Some(folder_id);
        let type_and_id = inside.cloud_object_type_and_id();
        let uid = inside.sync_id().uid();

        let update_manager = drive_app(&mut app, logged_out(), vec![folder, inside]);

        update_manager.update(&mut app, |update_manager, ctx| {
            update_manager.untrash_object(type_and_id, ctx);
        });

        app.read(|ctx| {
            assert_eq!(
                CloudModel::as_ref(ctx)
                    .get_by_uid(&uid)
                    .unwrap()
                    .metadata()
                    .folder_id,
                None,
                "the object was restored into a folder that is still in the trash"
            );
        });
    });
}

/// The same call with an account still goes to the server. A signed-in user's
/// trash lives somewhere else too, and deleting only the local copy would show
/// an empty trash that refills at the next fetch.
#[test]
fn a_signed_in_user_still_empties_the_trash_through_the_server() {
    App::test((), |mut app| async move {
        let mut workflow = local_workflow();
        workflow.metadata_mut().trashed_ts = Some(chrono::Utc::now().into());
        let uid = workflow.sync_id().uid();
        let update_manager = drive_app(&mut app, logged_in(), vec![workflow]);

        update_manager.update(&mut app, |update_manager, ctx| {
            update_manager.empty_trash(Space::Personal, ctx);
        });

        app.read(|ctx| {
            assert!(
                CloudModel::as_ref(ctx).get_by_uid(&uid).is_some(),
                "fork policy must not delete a signed-in user's object before the server agrees"
            );
        });
    });
}

/// The singletons the trash lifecycle touches. `ObjectActions` is here because
/// a permanent delete takes an object's actions with it, and `UserWorkspaces`
/// because emptying the trash is scoped to a space.
fn drive_app(
    app: &mut App,
    auth: AuthStateProvider,
    objects: Vec<Box<dyn CloudObject>>,
) -> ModelHandle<UpdateManager> {
    app.add_singleton_model(|_| NetworkStatus::new());
    app.add_singleton_model(TeamTesterStatus::mock);
    app.add_singleton_model(move |_| auth);
    app.add_singleton_model(UserWorkspaces::default_mock);
    app.add_singleton_model(SyncQueue::mock);
    app.add_singleton_model(|_| ObjectActions::new(Vec::new()));
    app.add_singleton_model(|_| CloudModel::new(None, objects, None));
    app.add_singleton_model(UpdateManager::mock)
}

fn logged_out() -> AuthStateProvider {
    AuthStateProvider::new_logged_out_for_test()
}

fn logged_in() -> AuthStateProvider {
    AuthStateProvider::new_for_test()
}

fn local_folder() -> Box<dyn CloudObject> {
    Box::new(crate::drive::folders::CloudFolder::new(
        SyncId::ClientId(ClientId::new()),
        crate::drive::folders::CloudFolderModel {
            name: "Scripts".to_owned(),
            is_open: false,
            is_warp_pack: false,
        },
        CloudObjectMetadata::mock(),
        CloudObjectPermissions {
            owner: local_drive_owner().expect("fork policy provides a local drive owner"),
            permissions_last_updated_ts: None,
            anyone_with_link: None,
            guests: Vec::new(),
        },
    ))
}

fn local_workflow() -> Box<dyn CloudObject> {
    Box::new(CloudWorkflow::new(
        SyncId::ClientId(ClientId::new()),
        CloudWorkflowModel::new(Workflow::new("deploy", "echo hi")),
        CloudObjectMetadata::mock(),
        CloudObjectPermissions {
            owner: local_drive_owner().expect("fork policy provides a local drive owner"),
            permissions_last_updated_ts: None,
            anyone_with_link: None,
            guests: Vec::new(),
        },
    ))
}

fn local_object_creation() -> QueueItem {
    QueueItem::CreateObject {
        object_type: ObjectType::Workflow,
        owner: local_drive_owner().expect("fork policy provides a local drive owner"),
        id: ClientId::new(),
        title: None,
        serialized_model: None,
        initial_folder_id: None,
        entrypoint: CloudObjectEventEntrypoint::default(),
        initiated_by: InitiatedBy::User,
    }
}

/// `WARP_FORK_POLICY=0` has to restore upstream behaviour exactly — that is the
/// whole point of it, A/B-ing a suspected regression without a rebuild — and
/// upstream returns `None` here when unauthenticated.
///
/// Asserted against the policy rather than by setting the variable: `is_active`
/// reads process-wide state, and a test that sets and unsets it races every
/// other test in the binary. An earlier version of this test did exactly that
/// and silently re-enabled fork policy mid-run for whatever happened to be
/// executing alongside it. The `WARP_FORK_POLICY=0` path is covered properly by
/// running the whole suite with it set, which is the real check anyway.
#[test]
fn the_local_owner_exists_only_under_fork_policy() {
    assert_eq!(
        local_drive_owner().is_some(),
        is_active(),
        "the local drive owner must appear and disappear with fork policy"
    );
}

/// The spawn depth limit reads what it is given, and falls back rather than
/// forbidding.
///
/// The fallback direction is the point. `0` is a meaningful setting — it stops
/// `agent spawn` outright — so parsing a typo as zero would turn a fat-fingered
/// variable into "spawning is broken", diagnosed anywhere but here.
///
/// Asserted against the parser rather than by setting the variable, for the
/// same reason as [`the_local_owner_exists_only_under_fork_policy`]: env vars
/// are process-wide and a test that sets one races every test beside it.
#[test]
fn the_spawn_depth_limit_falls_back_to_the_default() {
    assert_eq!(spawn_depth_limit_from(None), DEFAULT_SPAWN_DEPTH);
    assert_eq!(spawn_depth_limit_from(Some("")), DEFAULT_SPAWN_DEPTH);
    assert_eq!(spawn_depth_limit_from(Some("   ")), DEFAULT_SPAWN_DEPTH);
    assert_eq!(spawn_depth_limit_from(Some("deep")), DEFAULT_SPAWN_DEPTH);
    assert_eq!(spawn_depth_limit_from(Some("-1")), DEFAULT_SPAWN_DEPTH);

    assert_eq!(spawn_depth_limit_from(Some("0")), 0);
    assert_eq!(spawn_depth_limit_from(Some(" 5 ")), 5);
}

/// The visor's default is the opposite of every other env-gated predicate in
/// this module: on unless switched off. That asymmetry is deliberate — the
/// hotkey window is a fork surface, not a substitution for something that
/// works — and it means the *absence* of the variable is the case most likely
/// to regress, since nothing in a normal run ever exercises the parse.
///
/// Asserted against the parser rather than by setting the variable, for the
/// same reason as [`the_spawn_depth_limit_falls_back_to_the_default`]: env
/// vars are process-wide and a test that sets one races every test beside it.
#[test]
fn the_visor_opens_an_agent_unless_it_is_switched_off() {
    assert!(quake_visor_from(None));
    assert!(quake_visor_from(Some("")));
    assert!(quake_visor_from(Some("1")));
    assert!(quake_visor_from(Some("on")));

    assert!(!quake_visor_from(Some("0")));
    assert!(!quake_visor_from(Some("off")));
    assert!(!quake_visor_from(Some("false")));
    assert!(!quake_visor_from(Some("  off  ")));

    // Not a recognised negative. Anything unrecognised keeps the default
    // rather than guessing, which is the same call `WARP_FORK_POLICY` makes.
    assert!(quake_visor_from(Some("no")));
}

/// The frame log is off unless asked for, and asking badly still measures.
///
/// Asserted against the parser rather than by setting the variable, for the
/// same reason as [`the_visor_opens_an_agent_unless_it_is_switched_off`]: env
/// vars are process-wide and a test that sets one races every test beside it.
#[test]
fn the_frame_log_is_off_until_it_is_asked_for() {
    // Absent is the ordinary case, and it must cost nothing.
    assert_eq!(slow_frame_threshold_from(None), None);
    assert_eq!(slow_frame_threshold_from(Some("")), None);
    assert_eq!(slow_frame_threshold_from(Some("0")), None);
    assert_eq!(slow_frame_threshold_from(Some("off")), None);
    assert_eq!(slow_frame_threshold_from(Some("false")), None);

    assert_eq!(
        slow_frame_threshold_from(Some("on")),
        Some(DEFAULT_SLOW_FRAME_THRESHOLD)
    );
    assert_eq!(
        slow_frame_threshold_from(Some("true")),
        Some(DEFAULT_SLOW_FRAME_THRESHOLD)
    );

    // A bare number is a threshold in milliseconds.
    assert_eq!(
        slow_frame_threshold_from(Some("100")),
        Some(Duration::from_millis(100))
    );
    assert_eq!(
        slow_frame_threshold_from(Some("  8  ")),
        Some(Duration::from_millis(8))
    );

    // Unparseable takes the default rather than switching off. The variable
    // being set at all is a request to measure, and answering a typo with
    // silence is indistinguishable from the feature not working — the opposite
    // call from `WARP_FORK_QUAKE_VISOR`, because here the default is *off* and
    // the failure being avoided is a measurement that silently never happens.
    assert_eq!(
        slow_frame_threshold_from(Some("later")),
        Some(DEFAULT_SLOW_FRAME_THRESHOLD)
    );
}

/// The cloud harness plugin is refused exactly when fork policy is active.
///
/// Asserted against `is_active()` rather than by setting `WARP_FORK_POLICY`,
/// for the reason given on [`the_local_owner_exists_only_under_fork_policy`].
/// Tracking the policy rather than asserting a constant is the point: this
/// catches both an inverted condition and a well-meaning "just hardcode it",
/// and it keeps `WARP_FORK_POLICY=0` honest — A/B-ing a regression has to give
/// back upstream behaviour here too, or the switch does not mean what the
/// README says it means.
#[test]
fn the_cloud_harness_plugin_is_refused_under_fork_policy() {
    assert_eq!(
        cloud_harness_plugin_allowed(),
        !is_active(),
        "oz-harness-support must be installable only when fork policy is off"
    );
}

/// Tab-out-to-new-window is stock behaviour that this fork's builds could not
/// reach, and the reason is worth pinning: `DragTabsToWindows` is gated twice
/// by `cfg`, and a user preference is the only thing that outranks a `cfg`.
///
/// This also guards a deletion. `tab.rs` used to relax its own drag axis when
/// `fork::tab_pane_drag_enabled()`; that line is gone, and the tab-to-pane
/// drag now depends on this flag being forced on. If it ever leaves
/// `FORCE_ENABLED`, a tab stops being draggable off the strip at all and
/// nothing else would say so.
#[test]
fn dragging_a_tab_out_of_the_strip_depends_on_a_forced_flag() {
    assert!(
        FORCE_ENABLED.contains(&FeatureFlag::DragTabsToWindows),
        "both tab strips lock their drag axis unless this flag is on"
    );
}

/// The wide bind is off unless an address is named, and a value that cannot be
/// honoured is refused rather than guessed at.
///
/// Asserted against the parser rather than by setting the variable, for the
/// same reason as [`the_frame_log_is_off_until_it_is_asked_for`]: env vars are
/// process-wide and a test that sets one races every test beside it.
#[test]
fn a_bind_wider_than_loopback_has_to_be_named_exactly() {
    // Absent is the ordinary case and must cost nothing.
    for absent in [None, Some(""), Some("0"), Some("off"), Some("false")] {
        assert_eq!(control_bind_from(absent), ControlBind::LoopbackOnly);
    }

    // A bare address still means "any port", which is what every value meant
    // before T12.3.
    assert_eq!(
        control_bind_from(Some("192.168.1.5")),
        ControlBind::Additional("192.168.1.5".parse().expect("a v4 address"), 0)
    );
    assert_eq!(
        control_bind_from(Some("  fd00::1  ")),
        ControlBind::Additional("fd00::1".parse().expect("a v6 address"), 0)
    );

    // **T12.3 accepts a port, and this reverses a case this test used to pin as
    // refused.** `192.168.1.5:8080` sat in the ambiguous list below, which was
    // right while the only thing that could be ambiguous was an address — and
    // wrong once the console became installable, because a home-screen icon is a
    // saved URL and an ephemeral port makes it dead on the next launch. A port
    // is not a wildcard: it is one number, typed on purpose, and the `Host`
    // check compares it like any other part of the authority.
    assert_eq!(
        control_bind_from(Some("192.168.1.5:8080")),
        ControlBind::Additional("192.168.1.5".parse().expect("a v4 address"), 8080)
    );
    assert_eq!(
        control_bind_from(Some("[fd00::1]:8080")),
        ControlBind::Additional("fd00::1".parse().expect("a v6 address"), 8080)
    );
    // Explicit zero says what the default does, so it is honoured rather than
    // treated as a typo — there is nothing to be ambiguous about.
    assert_eq!(
        control_bind_from(Some("192.168.1.5:0")),
        ControlBind::Additional("192.168.1.5".parse().expect("a v4 address"), 0)
    );
    // **The one case a port cannot be disambiguated from, asserted so nobody
    // "fixes" it into a refusal it cannot correctly make.** `fd00::1:8080` is a
    // perfectly valid IPv6 address — `fd00:0:0:0:0:0:1:8080` — so a person who
    // meant "fd00::1, port 8080" and omitted the brackets has typed something
    // else that is real, and no parser can tell. Brackets are the disambiguation
    // and that is what they are for. It still fails closed rather than binding
    // somewhere surprising: this machine does not hold that address, so the bind
    // fails and is logged, and loopback keeps serving.
    assert_eq!(
        control_bind_from(Some("fd00::1:8080")),
        ControlBind::Additional("fd00::1:8080".parse().expect("a v6 address"), 0)
    );

    // A port does not buy a wildcard a hearing, and does not make loopback wide.
    assert!(matches!(
        control_bind_from(Some("0.0.0.0:8080")),
        ControlBind::Refused(_)
    ));
    assert_eq!(
        control_bind_from(Some("127.0.0.1:8080")),
        ControlBind::LoopbackOnly
    );

    // Asking for loopback is asking for what is already true, which is not an
    // error and is not a *wide* bind — the caller gets no second listener.
    assert_eq!(
        control_bind_from(Some("127.0.0.1")),
        ControlBind::LoopbackOnly
    );
    assert_eq!(control_bind_from(Some("::1")), ControlBind::LoopbackOnly);

    // The named anti-pattern. `HOST=0.0.0.0` is refused because it is
    // unanswerable, not merely because it is broad: nothing can say which
    // networks it joined, and no `Host` can be pinned for clients to present.
    assert!(matches!(
        control_bind_from(Some("0.0.0.0")),
        ControlBind::Refused(_)
    ));
    assert!(matches!(
        control_bind_from(Some("::")),
        ControlBind::Refused(_)
    ));

    // Fail closed on an ambiguous config, and a hostname is ambiguous: what it
    // resolves to is decided elsewhere and can change between check and bind.
    for ambiguous in [
        "lan",
        "my-laptop.local",
        "on",
        // A port that is not a port. Accepting the address and shrugging at the
        // rest would bind somewhere the person did not ask for.
        "192.168.1.5:notaport",
        "192.168.1.5:99999",
    ] {
        assert!(
            matches!(control_bind_from(Some(ambiguous)), ControlBind::Refused(_)),
            "{ambiguous:?} should be refused rather than interpreted"
        );
    }
}

/// Letting a phone say *yes* to an agent is opt-in, and everything that is not
/// an explicit yes is a no (T11.5).
///
/// The contrast with [`a_bind_wider_than_loopback_has_to_be_named_exactly`] is
/// deliberate and is why this parser is a different shape. There, an
/// unparseable value has to be *refused loudly*, because a typo would otherwise
/// silently mean something. Here there is nothing to mis-read: a value that is
/// not one of the affirmative words is simply not consent, and the safe side and
/// the default side are the same side.
///
/// Asserted against the parser rather than by setting the variable, because env
/// vars are process-wide and a test that sets one races every test beside it.
#[test]
fn a_paired_device_says_yes_only_when_the_owner_says_so() {
    for affirmative in ["1", "on", "true", "yes", "  on  "] {
        assert!(
            remote_approve_from(Some(affirmative)),
            "{affirmative:?} should turn it on"
        );
    }

    for otherwise in [None, Some(""), Some("0"), Some("off"), Some("false")] {
        assert!(!remote_approve_from(otherwise));
    }

    // A typo is not consent. Notably `"enabled"` and `"allow"` read like a yes
    // and are not one, which is the right way round: the cost of a missed opt-in
    // is that a person walks to their desk.
    for typo in ["enabled", "allow", "y", "ON!", "2"] {
        assert!(
            !remote_approve_from(Some(typo)),
            "{typo:?} should not be read as consent"
        );
    }
}

/// **The transcript and the event log were world-readable, and this is the pin.**
///
/// Measured on a live session 2026-08-31: `.warp/transcripts/*.md` and the event
/// log's `*.jsonl` were `0644` inside `0755` directories, holding the user's
/// prompts verbatim and the `tool_input` preview of every command an agent ran.
/// Nothing had gone wrong — no mode was ever set, so both inherited a `022`
/// umask. `discovery.rs` had the right instinct from the start; these two never
/// got it.
///
/// Asserted on the *mode bits*, not on "it is not 0644", because a test that
/// only excludes today's wrong answer passes for tomorrow's.
#[cfg(unix)]
#[test]
fn a_private_file_is_owner_only_from_the_moment_it_exists() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = std::env::temp_dir().join(format!("fork-private-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    create_private_dir(&dir).expect("the directory is creatable");

    let mode = std::fs::metadata(&dir)
        .expect("the directory exists")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o700, "the directory is readable by someone else");

    let path = dir.join("transcript.md");
    let file = create_private_file(&path, false).expect("the file is creatable");
    drop(file);

    let mode = std::fs::metadata(&path)
        .expect("the file exists")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600, "the file is readable by someone else");

    let _ = std::fs::remove_dir_all(&dir);
}

/// The upgrade path, which is the half that is easy to skip.
///
/// `OpenOptions::mode` is ignored for a file that already exists, so without
/// `tighten_existing` the fix would protect only conversations started after it
/// landed — and every log written by an earlier build would stay world-readable
/// for exactly as long as it remained useful.
#[cfg(unix)]
#[test]
fn a_file_an_earlier_build_left_open_is_narrowed() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = std::env::temp_dir().join(format!("fork-tighten-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the directory is creatable");

    let path = dir.join("events.jsonl");
    std::fs::write(&path, b"{}\n").expect("the file is writable");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
        .expect("the fixture is settable");

    // What the running code does: open for append, then narrow.
    let file = create_private_file(&path, true).expect("the file reopens");
    drop(file);
    tighten_existing(&path);

    let mode = std::fs::metadata(&path)
        .expect("the file exists")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(
        mode, 0o600,
        "an existing file was left as the umask made it"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// The transcript directory refuses to be committed.
///
/// **This fix shipped with no test at all**, found by reviewing the tests rather
/// than the code — and it is the one finding in that review whose consequence
/// leaves the machine, so it is the last one that should have gone unpinned. The
/// directory follows the pane, so it lands inside whatever repository the person
/// is working in; this repo's own `.gitignore` line is root-anchored and
/// protects this repo alone.
///
/// Calibrated on the assertion that must fail: asserting the directory exists
/// proves nothing, because `create_private_dir` alone would satisfy it.
#[test]
fn a_transcript_directory_ignores_itself() {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = root.path().join(".warp").join("transcripts");

    super::keep_dir_out_of_git(&dir);

    let ignore = dir.join(".gitignore");
    assert!(
        ignore.is_file(),
        "a directory the fork chose must carry its own ignore"
    );
    assert_eq!(
        std::fs::read_to_string(&ignore).expect("readable"),
        "*\n",
        "everything in here, including the ignore itself"
    );
}

/// An ignore the fork did not write is left alone.
///
/// A caller who named a directory owns it. Overwriting a `.gitignore` that was
/// already there could ignore files someone else put beside it, which is a worse
/// failure than the disclosure being fixed.
#[test]
fn an_existing_ignore_is_not_overwritten() {
    let root = tempfile::tempdir().expect("tempdir");
    let dir = root.path().join("somewhere-the-user-named");
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(dir.join(".gitignore"), "*.log\n").expect("seed");

    super::keep_dir_out_of_git(&dir);

    assert_eq!(
        std::fs::read_to_string(dir.join(".gitignore")).expect("readable"),
        "*.log\n",
        "somebody else's ignore is not ours to replace"
    );
}

/// A gate in this codebase has two halves, and reading one of them gives a
/// confident wrong answer in either direction.
///
/// T16 recorded three surfaces as lost when a WSL session is routed to a
/// server, and asked whether any of them cost anything in this fork. Answering
/// it by resolving `FeatureFlag` list membership says that `AIContextMenuCode`
/// and `FileBasedMcp` are in no list at all — not `DOGFOOD_FLAGS`, not
/// `PREVIEW_FLAGS`, not `RELEASE_FLAGS` — and so the surfaces are dead and the
/// gap is free. That was about to be written down. It is wrong: both flags are
/// also entries in `enabled_features()` behind a cargo feature, and both cargo
/// features are in `app/Cargo.toml`'s `default` list, so both are **on** in
/// every build made here and both losses are real.
///
/// Asserted on `cfg!` rather than on `is_enabled()` deliberately. `is_enabled`
/// resolves override → user preference → channel state, and the user preference
/// is a fact about the machine the test ran on; the cargo feature is the half
/// that a reader of the flag lists cannot see, so it is the half worth pinning.
#[test]
fn the_surfaces_routing_costs_are_switched_on_in_this_build() {
    assert!(
        cfg!(feature = "ai_context_menu_code"),
        "the @ menu's Code category is live, so outlines never built for a \
         routed repository are a real loss and not a moot one"
    );
    assert!(
        cfg!(feature = "file_based_mcp"),
        "FileBasedMCPManager subscribes to FileMCPWatcher, so a project-scoped \
         MCP config in a routed repository is genuinely never discovered"
    );

    for flag in [FeatureFlag::AIContextMenuCode, FeatureFlag::FileBasedMcp] {
        assert!(
            !crate::features::DOGFOOD_FLAGS.contains(&flag)
                && !crate::features::PREVIEW_FLAGS.contains(&flag)
                && !crate::features::RELEASE_FLAGS.contains(&flag),
            "{flag:?} is enabled by its cargo feature and by nothing else — if \
             it gains a list entry, the two halves have stopped agreeing and \
             the reasoning above needs re-reading rather than trusting"
        );
    }
}

/// The index that uploads source is forced off, and the entry that does it
/// looks redundant to anyone who checks the cargo features.
///
/// This is the assertion most likely to be deleted by a careful reader.
/// `full_source_code_embedding` is not in `app/Cargo.toml`'s `default` list, so
/// checking there says the feature is off and the `FORCE_DISABLED` entry is
/// dead weight. It is on: `remote_codebase_indexing` *is* in `default` and is
/// declared `= ["full_source_code_embedding"]`. The `cfg!` here is the evidence
/// for that, and it is why the runtime force is the primary removal rather than
/// belt-and-braces — the opposite of the Sentry entries beside it.
///
/// The upload it removes is `generate_embeddings` sending
/// `Fragment { content: String, .. }` to `ServerApi`, which `egress.rs` does not
/// cover. `auto_indexing_enabled` is still asserted, because if this entry ever
/// does go, upstream's default is what is left holding the line.
#[test]
fn the_index_that_uploads_source_is_forced_off_not_merely_absent() {
    assert!(
        cfg!(feature = "full_source_code_embedding"),
        "if this ever goes false the FORCE_DISABLED entry becomes belt-and-braces \
         rather than the removal — check `remote_codebase_indexing`'s deps before \
         concluding the entry can go"
    );
    assert!(
        FORCE_DISABLED.contains(&FeatureFlag::FullSourceCodeEmbedding),
        "the cargo feature is on, so nothing else in this build stops the upload"
    );
    assert!(
        {
            use ::settings::Setting as _;
            !crate::settings::AutoIndexingEnabled::default_value()
        },
        "upstream's speedbump is the second layer, and the only one left if the \
         force above is ever removed"
    );
}

/// Both halves of the outline gate default the same way, because wiring one of
/// them is a silent no-op.
///
/// `should_build_outlines` is `indexing_enabled && (codebase_context_enabled ||
/// outline_codebase_symbols_for_at_context_menu)`. It is a **disjunction**, so a
/// change that flips one default to off leaves the walk running through the
/// other, produces no error, and looks exactly like a change that worked. That
/// is the whole reason this test exists rather than a comment.
///
/// Asserted against `is_active()` rather than against `false`, for the same
/// reason as [`the_cloud_harness_plugin_is_refused_under_fork_policy`]:
/// `WARP_FORK_POLICY` is process-wide, and a test that reads a literal would
/// fail for anyone A/B-ing a suspected fork regression with it set.
#[test]
fn both_halves_of_the_outline_gate_default_the_same_way() {
    use ::settings::Setting as _;

    assert_eq!(
        crate::settings::CodebaseContextEnabled::default_value(),
        !is_active(),
        "codebase context is half of what starts an outline walk"
    );
    assert_eq!(
        crate::settings::OutlineCodebaseSymbolsForAtContextMenu::default_value(),
        !is_active(),
        "the @ menu setting is the other half, and either one alone starts it"
    );
}

/// The repository's symbol map leaves by exactly one call site, and that call
/// site is guarded.
///
/// `ServerApi::get_relevant_files` `POST`s `/ai/relevant_files` with every
/// candidate file's path, its symbol names and the comments written above each
/// symbol. `fork::rank_relevant_files_locally` diverts it into
/// `ai::get_relevant_files::local_rank`, and that diversion protects exactly
/// the call sites it sits in front of.
///
/// So the thing worth pinning is not the guard — it is the *count*. A second
/// caller added anywhere in `app/src` would send the same payload with nothing
/// in front of it, would compile, and would pass every other test here. This is
/// the same shape as `egress.rs`'s bypass: a backstop that covers today's call
/// sites is a fact about today unless something counts them.
///
/// Comments are skipped, because this file and `fork.rs` both name the function
/// in prose in order to explain that it is diverted.
#[test]
fn the_symbol_map_leaves_by_exactly_one_call_site_and_it_is_guarded() {
    const GUARDED: &str = "ai/get_relevant_files/controller.rs";

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut callers = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !name.ends_with(".rs") || name.ends_with("_tests.rs") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let calls_it = text.lines().any(|line| {
                let trimmed = line.trim_start();
                !trimmed.starts_with("//") && trimmed.contains(".get_relevant_files(")
            });
            if calls_it {
                callers.push(
                    path.strip_prefix(&root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
    callers.sort();

    assert_eq!(
        callers,
        vec![GUARDED.to_owned()],
        "a new caller of get_relevant_files would upload the symbol map with no \
         guard in front of it; divert it through local_rank, then add it here"
    );

    let guarded = std::fs::read_to_string(root.join(GUARDED)).expect("the guarded file");
    assert!(
        guarded.contains("fork::rank_relevant_files_locally()"),
        "the only call site must still be behind the fork predicate"
    );
}
