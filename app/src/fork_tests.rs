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
