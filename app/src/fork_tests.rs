use warpui::{App, SingletonEntity};

use super::*;
use crate::auth::AuthStateProvider;
use crate::cloud_object::{CloudObjectEventEntrypoint, ObjectType, Space};
use crate::drive::settings::WarpDriveSettings;
use crate::network::NetworkStatus;
use crate::server::cloud_objects::update_manager::{InitiatedBy, UpdateManager};
use crate::server::ids::ClientId;
use crate::server::sync_queue::{QueueItem, SyncQueue};
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
