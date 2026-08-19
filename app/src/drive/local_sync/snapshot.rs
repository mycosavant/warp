//! The bridge between the live store and the on-disk form.
//!
//! Everything in `format` and `tree` is deliberately ignorant of the app. This
//! is the one file that knows about both, and it is small on purpose.
//!
//! # Reading a payload out of a `dyn CloudObject`
//!
//! There is no accessor for it. `CloudObject` is object-safe and non-generic —
//! it has to be, since `CloudModel` stores its objects as trait objects — so the
//! model itself is only reachable by downcasting to the concrete
//! `GenericCloudObject<K, M>`, which means thirteen downcasts and a list this
//! file would have to keep in step by hand.
//!
//! [`CloudObject::update_object_queue_item`] is the way through. It is
//! object-safe, it is a pure constructor that delegates to the model, and every
//! object type has exactly one `Update*` variant carrying its typed model. So
//! one `match` covers all thirteen, and adding a type upstream fails to compile
//! here rather than silently exporting nothing. Nothing is enqueued: the item is
//! constructed, read and dropped.

use std::collections::HashMap;

use anyhow::{Result, bail};
use warpui::{AppContext, SingletonEntity};

use super::format::{Alias, Payload, PortableObject};
use super::tree::PlacedObject;
use crate::cloud_object::model::persistence::CloudModel;
use crate::cloud_object::{CloudObject, CloudObjectMetadata, CloudObjectPermissions, Space};
use crate::server::sync_queue::QueueItem;
use crate::workflows::aliases::WorkflowAliases;

/// What a snapshot took, and what it left behind.
#[derive(Debug, Default, PartialEq)]
pub struct SnapshotSummary {
    /// Objects belonging to a team drive, or shared with this user by someone
    /// else. Left out because a personal git repository is not where somebody
    /// else's objects belong, and counted so that "where did my shared folder
    /// go" has an answer.
    pub not_personal: usize,
    /// Objects whose payload could not be read. Should be zero; a non-zero
    /// value means an object type grew a shape this file does not know.
    pub unreadable: Vec<String>,
    /// Aliases whose workflow is not in the mirror — trashed on another
    /// machine, or somebody else's. They are left in settings untouched and
    /// counted here, because the alternative is a user whose alias did not
    /// travel and no sentence anywhere explaining why.
    pub aliases_not_mirrored: usize,
}

/// Reads the personal drive out of the live store.
///
/// Membership is decided by [`Space::Personal`] rather than by comparing owners
/// directly, because that is the seam that already answers "is this mine" for
/// both a signed-in user and an account-free one — and getting the two sides of
/// that question to disagree is exactly the bug T4.6 caught.
///
/// Trashed objects are included, with their `trashed` timestamp intact. Leaving
/// them out would make an export quietly destructive: emptying the trash is the
/// user's decision, and an export that pre-empted it would take the undo away.
pub fn snapshot(app: &AppContext) -> (Vec<PlacedObject>, SnapshotSummary) {
    let mut summary = SnapshotSummary::default();
    let mut objects = Vec::new();
    // T4.4g. The join that makes an alias travel with its workflow: read once
    // here rather than looked up per object, and drained as objects claim their
    // entries so that whatever is left over is exactly the set with no workflow
    // in the mirror.
    let mut aliases = aliases_by_workflow(app);

    for object in CloudModel::as_ref(app).cloud_objects() {
        if object.space(app) != Space::Personal {
            summary.not_personal += 1;
            continue;
        }

        match to_portable(object.as_ref()) {
            Ok(mut portable) => {
                portable.aliases = aliases.remove(&portable.id.to_string()).unwrap_or_default();
                objects.push(PlacedObject {
                    object: portable,
                    parent: object.metadata().folder_id,
                });
            }
            Err(err) => summary
                .unreadable
                .push(format!("{}: {err:#}", object.display_name())),
        }
    }

    summary.aliases_not_mirrored = aliases.values().map(Vec::len).sum();

    // Sorted by identity so two exports of the same store produce the same
    // order. Nothing downstream depends on order, but a summary and a log line
    // that shuffle between runs are much harder to read than ones that do not.
    objects.sort_by_key(|placed| placed.object.id.to_string());
    (objects, summary)
}

/// Every alias in settings, grouped by the workflow it points at.
fn aliases_by_workflow(app: &AppContext) -> HashMap<String, Vec<Alias>> {
    let mut by_workflow: HashMap<String, Vec<Alias>> = HashMap::new();

    for alias in WorkflowAliases::as_ref(app).get_all_aliases() {
        by_workflow
            .entry(alias.workflow_id.to_string())
            .or_default()
            .push(Alias {
                alias: alias.alias.clone(),
                env_vars: alias.env_vars.map(|id| id.to_string()),
                // Ordered on the way in, so the file's bytes do not depend on
                // a `HashMap`'s per-process iteration order.
                arguments: alias
                    .arguments
                    .as_ref()
                    .map(|arguments| arguments.clone().into_iter().collect()),
            });
    }

    by_workflow
}

fn to_portable(object: &dyn CloudObject) -> Result<PortableObject> {
    let metadata: &CloudObjectMetadata = object.metadata();
    let permissions: &CloudObjectPermissions = object.permissions();

    Ok(PortableObject {
        id: object.sync_id(),
        object_type: object.object_type(),
        name: object.display_name(),
        owner: permissions.owner,
        revision_ts: metadata
            .revision
            .map(|revision| revision.timestamp_micros()),
        metadata_last_updated_ts: metadata
            .metadata_last_updated_ts
            .map(|ts| ts.timestamp_micros()),
        trashed_ts: metadata.trashed_ts.map(|ts| ts.timestamp_micros()),
        creator_uid: metadata.creator_uid.clone(),
        last_editor_uid: metadata.last_editor_uid.clone(),
        is_welcome_object: metadata.is_welcome_object,
        // Filled in by `snapshot`, which is the only caller with the settings
        // group in reach.
        aliases: Vec::new(),
        payload: payload_of(object)?,
    })
}

/// The thirteen-way match. Each `Update*` variant carries its model already
/// typed, so this is a line apiece.
fn payload_of(object: &dyn CloudObject) -> Result<Payload> {
    // `CloudModelType::serialized` is what SQLite stores in the payload column
    // and what the server is sent, so re-reading it as JSON is a parse of
    // `serde_json`'s own output rather than a re-serialization that could
    // differ.
    use crate::cloud_object::CloudModelType as _;

    let json = |serialized: crate::server::sync_queue::SerializedModel| -> Result<Payload> {
        Ok(Payload::Json(serde_json::from_str(
            serialized.model_as_str(),
        )?))
    };

    match object.update_object_queue_item(None) {
        // Prose, kept as prose. `conversation_id` is not carried, and neither
        // does SQLite carry it — the `notebooks` table has no column for it,
        // because it names a conversation on Warp's server.
        QueueItem::UpdateNotebook { model, .. } => Ok(Payload::Notebook {
            markdown: model.data.clone(),
            ai_document_id: model.ai_document_id.as_ref().map(|id| id.to_string()),
        }),
        // `is_open` is deliberately dropped; see the `format` module docs.
        QueueItem::UpdateFolder { model, .. } => Ok(Payload::Folder {
            is_warp_pack: model.is_warp_pack,
        }),
        QueueItem::UpdateWorkflow { model, .. } => json(model.serialized()),
        QueueItem::UpdateCloudPreferences { model, .. } => json(model.serialized()),
        QueueItem::UpdateEnvVarCollection { model, .. } => json(model.serialized()),
        QueueItem::UpdateWorkflowEnum { model, .. } => json(model.serialized()),
        QueueItem::UpdateAIFact { model, .. } => json(model.serialized()),
        QueueItem::UpdateMCPServer { model, .. } => json(model.serialized()),
        QueueItem::UpdateAIExecutionProfile { model, .. } => json(model.serialized()),
        QueueItem::UpdateTemplatableMCPServer { model, .. } => json(model.serialized()),
        QueueItem::UpdateCloudEnvironment { model, .. } => json(model.serialized()),
        QueueItem::UpdateScheduledAmbientAgent { model, .. } => json(model.serialized()),
        QueueItem::UpdateCloudAgentConfig { model, .. } => json(model.serialized()),
        // `update_object_queue_item` only ever builds an `Update*` variant, so
        // reaching here means an object type gained one this file has not been
        // taught about — which is worth an error naming it rather than a
        // silently missing file.
        other => bail!("no file representation for {other:?}"),
    }
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
