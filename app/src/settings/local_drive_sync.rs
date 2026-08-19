//! Fork-local settings for the git-backed Warp Drive mirror.
//!
//! Deliberately a separate group rather than fields on `WarpDriveSettings`: a
//! new file cannot conflict on an upstream merge, which is the same reason
//! [`LocalAiSettings`](super::LocalAiSettings) is its own file.
//!
//! There is exactly one setting, and the fact that it is a *setting* rather than
//! a parameter is the point. An export prunes files in the directory it writes
//! to. If the destination came in with the request, anything that could reach
//! local control could point a pruning exporter at a directory of its choosing.
//! Coming from settings, the destination is somewhere the user chose once,
//! deliberately, and `drive.sync.export` has no say in it.
//!
//! See `drive::local_sync` for the consumer and `.fork/TASKS.md` T4.4.

use settings::macros::define_settings_group;
use settings::{SupportedPlatforms, SyncToCloud};

define_settings_group!(LocalDriveSyncSettings, settings: [
    local_drive_sync_path: LocalDriveSyncPath {
        type: String,
        default: String::new(),
        supported_platforms: SupportedPlatforms::DESKTOP,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::ALL,
        private: false,
        toml_path: "warp_drive.local_sync.path",
        description: "Absolute path of the directory Warp Drive is mirrored into, for you to keep under git. Leave empty to disable. Warp writes this directory and never runs git itself.",
    },
]);
