use settings::macros::define_settings_group;
use settings::{RespectUserSyncSetting, SupportedPlatforms, SyncToCloud};
use warp_core::features::FeatureFlag;

use super::DriveSortOrder;

pub const HAS_AUTO_OPENED_WELCOME_FOLDER: &str = "HasAutoOpenedWelcomeFolder";

define_settings_group!(WarpDriveSettings, settings: [
    sorting_choice: WarpDriveSortingChoice {
        type: DriveSortOrder,
        default: DriveSortOrder::ByObjectType,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "warp_drive.sorting_choice",
        description: "The sort order for items in Warp Drive.",
    },
    sharing_onboarding_block_shown: WarpDriveSharingOnboardingBlockShown {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        surface: settings::SettingSurfaces::GUI,
        private: true,
    },
    // Controls whether Warp Drive appears in the tools panel, command palette, and command search.
    enable_warp_drive: EnableWarpDrive {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Globally(RespectUserSyncSetting::Yes),
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "warp_drive.enabled",
        description: "Whether Warp Drive is enabled.",
    },
]);

impl WarpDriveSettings {
    /// Returns whether Warp Drive is available for the current auth state.
    ///
    /// This is intentionally separate from the stored `enable_warp_drive`
    /// preference. Logged-out and anonymous users can retain their onboarding
    /// preference so Warp Drive appears automatically after signup, while the
    /// feature remains unavailable until then.
    pub fn is_warp_drive_available(app: &warpui::AppContext) -> bool {
        use warpui::SingletonEntity as _;
        // The auth check routes through `fork::is_anonymous_for_ui` because this
        // decides what to *draw* — whether Warp Drive appears in the tools panel,
        // command palette and command search — rather than whether to talk to the
        // server. Under fork policy the drive is local-first and fully functional
        // with no account, so hiding it would hide a working feature.
        //
        // Without this, fork policy disabled Warp Drive by way of its own T1
        // change: `SkipFirebaseAnonymousUser` is in `fork::FORCE_ENABLED`, which
        // makes the first clause false, and being logged out makes the second
        // false. Found by running the Windows build, not by reading — every
        // static trace of the drive stopped at the model layer. See T4.2.
        !FeatureFlag::SkipFirebaseAnonymousUser.is_enabled()
            || !crate::fork::is_anonymous_for_ui(
                crate::auth::AuthStateProvider::as_ref(app)
                    .get()
                    .is_anonymous_or_logged_out(),
            )
    }
    /// Returns whether Warp Drive should be considered enabled.
    /// Returns `false` when the user is anonymous or fully logged out,
    /// regardless of the user setting.
    pub fn is_warp_drive_enabled(app: &warpui::AppContext) -> bool {
        use warpui::SingletonEntity as _;
        *Self::as_ref(app).enable_warp_drive && Self::is_warp_drive_available(app)
    }
}
