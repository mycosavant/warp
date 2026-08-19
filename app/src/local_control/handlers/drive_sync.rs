//! `drive.sync.status`, `drive.sync.export` and `drive.sync.import` — the
//! trigger for the git-backed Warp Drive mirror.
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
//!
//! # The third refusal
//!
//! Both directions also stop dead while any mirrored file still has git
//! conflict markers in it, and `status` reports those files so this is
//! diagnosable rather than mysterious. The reasoning is in
//! `drive::local_sync`'s module docs; the short version is that a conflicted
//! file does not parse, an object whose file does not parse is absent, and
//! absence is how an import is told to trash something.

use std::path::{Path, PathBuf};

use ::local_control::protocol::{
    DriveSyncExportResult, DriveSyncImportResult, DriveSyncStatusResult,
};
use ::local_control::{ControlError, ErrorCode};
use serde::Serialize;
use settings::Setting as _;
use warpui::{ModelContext, SingletonEntity};

use crate::drive::local_sync::apply::apply;
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
        aliases_not_mirrored: summary.aliases_not_mirrored,
        // Reading the tree is the one thing this action does not have to do to
        // answer its literal question, and the reason it does it anyway is the
        // sentence above: an unresolved merge is the only condition that stops
        // *both* directions, so a status that could not see it would send the
        // user looking at the setting instead of at their working tree.
        conflicted: conflicts(path.as_deref()),
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
            aliases_not_mirrored,
        },
    ) = snapshot(ctx);

    // Blocking I/O on the main thread. A drive is hundreds of small files at
    // most and unchanged ones are not rewritten, so this is milliseconds; it is
    // also synchronous on purpose, because the caller is asking "is my drive on
    // disk now" and a result that arrives before the work does is a lie.
    let summary = tree::export(&path, &objects).map_err(|err| {
        // A half-merged file in the way is the caller's tree to fix, and saying
        // "internal error" about it would send them to the wrong place.
        conflict_refusal(&err, &path).unwrap_or_else(|| {
            ControlError::with_details(
                ErrorCode::Internal,
                format!("drive.sync.export failed writing {}", path.display()),
                format!("{err:#}"),
            )
        })
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
        aliases_not_mirrored,
    })
}

/// Reads the configured directory back into the store.
///
/// The direction that changes the user's data rather than a directory, so it
/// reports what it did in the same shape the export does. The files win, and an
/// object whose file is gone is trashed rather than deleted — see
/// `drive::local_sync::apply` for why absence means what it means.
pub(crate) fn import(
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    let path = usable_path(ctx)?;
    if !path.is_dir() {
        return Err(ControlError::with_details(
            ErrorCode::InvalidRequest,
            "there is nothing at warp_drive.local_sync.path to import",
            format!("{} does not exist", path.display()),
        ));
    }

    let found = tree::import(&path).map_err(|err| {
        ControlError::with_details(
            ErrorCode::Internal,
            format!("drive.sync.import failed reading {}", path.display()),
            format!("{err:#}"),
        )
    })?;

    // Refused whole, not file by file. Skipping the conflicted files would look
    // tidier and would be the most destructive thing this action can do: a file
    // that fails to parse is absent from the tree, and absence is how the store
    // is told an object was deleted. The objects mid-merge — the ones the user
    // is actively working on — would be exactly the ones trashed.
    //
    // Nothing is auto-resolved either. `--ours` and `--theirs` are git's words
    // and belong to the user; guessing here is what decision 1 ruled out.
    if !found.conflicted.is_empty() {
        return Err(unresolved(&found.conflicted, &path));
    }

    let summary = apply(&found.objects, ctx).map_err(|err| {
        // The empty-tree refusal lands here, and it is a request problem rather
        // than an internal one: the user pointed this somewhere with nothing in
        // it, and applying that would have trashed the whole drive.
        ControlError::with_details(
            ErrorCode::InvalidRequest,
            format!("drive.sync.import refused {}", path.display()),
            format!("{err:#}"),
        )
    })?;

    log::info!(
        "Warp Drive mirror: imported {} created, {} updated, {} unchanged, {} trashed from {}",
        summary.created,
        summary.updated,
        summary.unchanged,
        summary.trashed,
        path.display()
    );

    to_control_data(DriveSyncImportResult {
        path: path.display().to_string(),
        created: summary.created,
        updated: summary.updated,
        unchanged: summary.unchanged,
        trashed: summary.trashed,
        ignored: found
            .ignored
            .into_iter()
            .map(|(path, reason)| format!("{}: {reason}", path.display()))
            .collect(),
        duplicates: found
            .duplicates
            .into_iter()
            .map(|(path, id)| format!("{}: {id}", path.display()))
            .collect(),
        unreadable: summary.unreadable,
        aliases_set: summary.aliases_set,
        aliases_removed: summary.aliases_removed,
        aliases_reassigned: summary.aliases_reassigned,
    })
}

/// Conflicted files under `path`, or nothing when there is no directory to look
/// in. A tree that cannot be read is not reported as conflicted — that is a
/// different failure, and `export` will name it properly.
fn conflicts(path: Option<&Path>) -> Vec<String> {
    path.filter(|path| path.is_dir())
        .and_then(|path| tree::import(path).ok())
        .map(|found| describe(&found.conflicted))
        .unwrap_or_default()
}

fn describe(conflicted: &[tree::Conflicted]) -> Vec<String> {
    conflicted
        .iter()
        .map(|file| format!("{}:{} ({})", file.path.display(), file.line, file.name))
        .collect()
}

/// The same refusal in both directions, named the same way, so a caller that
/// hits it exporting recognises it when it happens importing.
fn unresolved(conflicted: &[tree::Conflicted], path: &Path) -> ControlError {
    ControlError::with_details(
        ErrorCode::InvalidRequest,
        format!(
            "{} file(s) under {} have unresolved merge conflicts",
            conflicted.len(),
            path.display()
        ),
        describe(conflicted).join("; "),
    )
}

/// Recognises an export stopped by a merge, rather than by a disk.
fn conflict_refusal(err: &anyhow::Error, path: &Path) -> Option<ControlError> {
    let conflicts = err.downcast_ref::<tree::ConflictsInTheWay>()?;
    Some(unresolved(&conflicts.0, path))
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
            "the drive mirror needs fork policy, which WARP_FORK_POLICY has disabled",
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
