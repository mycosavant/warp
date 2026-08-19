//! `drive.sync.status` and `drive.sync.export` — the trigger for the git-backed
//! Warp Drive mirror.
//!
//! A local-control action rather than a button, for three reasons. It makes the
//! feature drivable by an agent, which is the fork's whole orchestration story;
//! it makes it verifiable from outside the GUI, which is how the account-free
//! drive got its bug found; and it is the surface T1.12 was already asking for,
//! since none of the other 85 actions touch the object store at all.
//!
//! # Why the destination is a setting and not a parameter
//!
//! An export prunes. It removes files it recognises as ones it wrote, and it
//! removes directories once they are empty. If the destination arrived with the
//! request, anything that could reach local control could aim that at a
//! directory of its choosing. Coming from settings, it is somewhere the user
//! chose once and can see, and the action has no say in it.
//!
//! The guards below exist for the same reason. A path that is empty, relative,
//! or a filesystem root is refused before anything is read or written — a
//! mistyped `/` would otherwise walk the entire filesystem reading every file
//! to decide whether it was one of ours.

use std::path::{Path, PathBuf};

use ::local_control::protocol::{DriveSyncExportResult, DriveSyncStatusResult};
use ::local_control::{ControlError, ErrorCode};
use serde::Serialize;
use settings::Setting as _;
use warpui::{ModelContext, SingletonEntity};

use crate::drive::local_sync::snapshot::{SnapshotSummary, snapshot};
use crate::drive::local_sync::tree;
use crate::local_control::LocalControlBridge;
use crate::settings::LocalDriveSyncSettings;

pub(crate) fn status(
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    // Deliberately reports rather than refuses when the path is unset: this is
    // the action you run to find out *why* an export will not run.
    let path = configured_path(ctx);
    let (objects, summary) = snapshot(ctx);

    to_control_data(DriveSyncStatusResult {
        path: path.as_ref().map(|path| path.display().to_string()),
        path_exists: path.as_deref().is_some_and(Path::exists),
        objects: objects.len(),
        not_personal: summary.not_personal,
        unreadable: summary.unreadable,
    })
}

pub(crate) fn export(
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    let path = usable_path(ctx)?;
    let (
        objects,
        SnapshotSummary {
            not_personal,
            unreadable,
        },
    ) = snapshot(ctx);

    // Blocking I/O on the main thread. A drive is hundreds of small files at
    // most and unchanged ones are not rewritten, so this is milliseconds; it is
    // also synchronous on purpose, because the caller is asking "is my drive on
    // disk now" and a result that arrives before the work does is a lie.
    let summary = tree::export(&path, &objects).map_err(|err| {
        ControlError::with_details(
            ErrorCode::Internal,
            format!("drive.sync.export failed writing {}", path.display()),
            format!("{err:#}"),
        )
    })?;

    log::info!(
        "Warp Drive mirror: {} written, {} unchanged, {} removed in {}",
        summary.written,
        summary.unchanged,
        summary.removed_files,
        path.display()
    );

    to_control_data(DriveSyncExportResult {
        path: path.display().to_string(),
        written: summary.written,
        unchanged: summary.unchanged,
        removed_files: summary.removed_files,
        removed_directories: summary.removed_directories,
        orphaned: summary.orphaned.iter().map(|id| id.to_string()).collect(),
        not_personal,
        unreadable,
    })
}

fn configured_path(ctx: &mut ModelContext<LocalControlBridge>) -> Option<PathBuf> {
    let path = LocalDriveSyncSettings::as_ref(ctx)
        .local_drive_sync_path
        .value()
        .trim()
        .to_owned();

    (!path.is_empty()).then(|| PathBuf::from(path))
}

/// The configured path, or an error explaining exactly which guard it failed.
fn usable_path(ctx: &mut ModelContext<LocalControlBridge>) -> Result<PathBuf, ControlError> {
    // `WARP_FORK_POLICY=0` is documented as restoring stock upstream behaviour
    // without a rebuild, and it is what gets reached for when something is
    // suspected of being a fork-caused regression. The catalog is a
    // compile-time list so the action cannot vanish from it, but an action that
    // deletes files in a directory is exactly the kind that should stop working
    // when the policy it belongs to is switched off.
    if !crate::fork::local_drive_enabled() {
        return Err(ControlError::new(
            ErrorCode::UnsupportedAction,
            "drive.sync.export needs fork policy, which WARP_FORK_POLICY has disabled",
        ));
    }

    let Some(path) = configured_path(ctx) else {
        return Err(ControlError::new(
            ErrorCode::InvalidRequest,
            "warp_drive.local_sync.path is not set, so there is nowhere to mirror the drive to",
        ));
    };

    if !path.is_absolute() {
        return Err(ControlError::with_details(
            ErrorCode::InvalidRequest,
            "warp_drive.local_sync.path must be an absolute path",
            // A relative path resolves against Warp's working directory, which
            // is not somewhere the user reasoned about when they set this.
            format!("{} is relative", path.display()),
        ));
    }

    if path.parent().is_none() {
        return Err(ControlError::with_details(
            ErrorCode::InvalidRequest,
            "warp_drive.local_sync.path must not be a filesystem root",
            format!("{} has no parent directory", path.display()),
        ));
    }

    if path.exists() && !path.is_dir() {
        return Err(ControlError::with_details(
            ErrorCode::InvalidRequest,
            "warp_drive.local_sync.path must be a directory",
            format!("{} is not a directory", path.display()),
        ));
    }

    Ok(path)
}

fn to_control_data<T: Serialize>(value: T) -> Result<serde_json::Value, ControlError> {
    serde_json::to_value(value).map_err(|err| {
        ControlError::with_details(
            ErrorCode::Internal,
            "failed to serialize local-control response",
            err.to_string(),
        )
    })
}

#[cfg(test)]
#[path = "drive_sync_tests.rs"]
mod tests;
