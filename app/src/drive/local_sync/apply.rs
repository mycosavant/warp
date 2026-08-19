//! T4.4f — putting an imported tree back into the store.
//!
//! The reverse of `snapshot`, and the harder direction. Reading an object needs
//! one dyn-safe accessor; writing one needs its concrete `M`, so this is
//! thirteen constructors where `snapshot` was thirteen accessors.
//!
//! # What happens to an object that is not in the tree
//!
//! It is **trashed, not deleted**. That is the rule the rest of the design
//! hangs off, and it is worth spelling out because the alternatives are both
//! wrong:
//!
//! * *Ignore it* and deletions never propagate. Delete a workflow on machine A,
//!   pull on B, and B's next export puts the file straight back — the two
//!   machines resurrect each other's deletions forever.
//! * *Delete it* and a single import against the wrong directory destroys the
//!   drive with no undo.
//!
//! Trashing composes with the format instead. A trashed object still exports,
//! carrying its `trashed` timestamp, so "I deleted this" travels as *content*
//! rather than as absence. Absence therefore means something stronger — the
//! trash was emptied — and echoing that as a local trash is the conservative
//! reading of it, recoverable from the UI either way.
//!
//! It also has a prerequisite that turned out not to hold: upstream's
//! `trash_object` opens by requiring a server id, so account-free it did
//! nothing at all. See `fork::drive_deletes_are_local`.
//!
//! # The tree wins
//!
//! No merge, no revision comparison, no reconciliation. Import overwrites from
//! the files, because the moment this starts deciding which side is newer it is
//! a sync engine, and the whole point of decision 1 in T4.4 was that git is
//! already a better one than anything this fork should write.

use std::collections::{HashMap, HashSet};

use anyhow::{Result, anyhow, bail};
use warpui::{GetSingletonModelHandle, ModelContext, ReadModel, SingletonEntity, UpdateModel};

use super::format::{Alias, Payload, PortableObject};
use super::snapshot::snapshot;
use super::tree::PlacedObject;
use crate::ai::ambient_agents::scheduled::CloudScheduledAmbientAgentModel;
use crate::ai::cloud_agent_config::CloudAgentConfigModel;
use crate::ai::cloud_environments::CloudAmbientAgentEnvironmentModel;
use crate::ai::execution_profiles::CloudAIExecutionProfileModel;
use crate::ai::facts::CloudAIFactModel;
use crate::ai::mcp::CloudMCPServerModel;
use crate::ai::mcp::templatable::CloudTemplatableMCPServerModel;
use crate::cloud_object::model::persistence::CloudModel;
use crate::cloud_object::{
    CloudModelType, CloudObjectMetadata, CloudObjectPermissions, CloudObjectStatuses,
    CloudObjectSyncStatus, GenericCloudObject, GenericStringObjectFormat, JsonObjectType,
    NumInFlightRequests, ObjectType, Revision,
};
use crate::drive::folders::{CloudFolderModel, FolderId};
use crate::env_vars::CloudEnvVarCollectionModel;
use crate::notebooks::{CloudNotebookModel, NotebookId};
use crate::server::cloud_objects::update_manager::UpdateManager;
use crate::server::ids::{GenericStringObjectId, HashableId, SyncId, ToServerId};
use crate::settings::cloud_preferences::CloudPreferenceModel;
use crate::workflows::aliases::{WorkflowAlias, WorkflowAliases};
use crate::workflows::workflow::Workflow;
use crate::workflows::workflow_enum::CloudWorkflowEnumModel;
use crate::workflows::{CloudWorkflowModel, WorkflowId};
use ai::document::AIDocumentId;

/// What an apply did.
#[derive(Debug, Default, PartialEq)]
pub struct ApplySummary {
    pub created: usize,
    pub updated: usize,
    /// Objects the tree already agreed with. In the steady state this is all of
    /// them, and an import is a no-op — the mirror of the export side's
    /// `unchanged`.
    pub unchanged: usize,
    /// Objects in the store but not in the tree, echoed as a local trash.
    pub trashed: usize,
    /// Files this build cannot turn back into an object, with the reason. An
    /// object type that grew a payload shape the reader does not know would
    /// land here rather than being silently dropped.
    pub unreadable: Vec<String>,
    /// Alias entries added or rewritten from the tree.
    pub aliases_set: usize,
    /// Alias entries dropped because the tree's workflow no longer lists them.
    pub aliases_removed: usize,
    /// Aliases taken from a workflow the tree does not describe.
    ///
    /// An alias is keyed by its text and nothing else, so a tree that claims
    /// `dep` takes it from whatever it pointed at here — including a team
    /// workflow the mirror never sees. That is the right outcome, since the
    /// alternative is two `dep`s, but it is a change to something outside the
    /// mirror and so it gets named rather than counted.
    pub aliases_reassigned: Vec<String>,
}

/// Writes an imported tree into the live store.
pub fn apply<A>(objects: &[PlacedObject], app: &mut A) -> Result<ApplySummary>
where
    A: UpdateModel + ReadModel + GetSingletonModelHandle,
{
    // The guard against the most destructive mistake available: an import
    // pointed at an empty or wrong directory would read as "every object was
    // deleted" and trash the entire drive in one call. A genuinely empty drive
    // is not distinguishable from a wrong path, so refuse and let the user say
    // what they meant.
    if objects.is_empty() {
        bail!("refusing to import from a tree with no Warp Drive objects in it");
    }

    let existing: HashMap<String, PlacedObject> = CloudModel::handle(app)
        .read(app, |_, ctx| snapshot(ctx))
        .0
        .into_iter()
        .map(|placed| (placed.object.id.to_string(), placed))
        .collect();

    let mut summary = ApplySummary::default();
    let mut touched = Vec::new();

    CloudModel::handle(app).update(app, |cloud_model, ctx| {
        for placed in objects {
            let key = placed.object.id.to_string();
            if existing.get(&key) == Some(placed) {
                summary.unchanged += 1;
                continue;
            }

            match write_object(placed, cloud_model, ctx) {
                Ok(true) => summary.created += 1,
                Ok(false) => summary.updated += 1,
                Err(err) => {
                    summary
                        .unreadable
                        .push(format!("{}: {err:#}", placed.object.name));
                    continue;
                }
            }
            touched.push(key);
        }
    });

    persist(&touched, app);
    summary.trashed = trash_absent(objects, &existing, app);
    apply_aliases(objects, &mut summary, app)?;

    Ok(summary)
}

/// T4.4g — reconciles workflow aliases against the tree.
///
/// # Only for the workflows the tree describes
///
/// The rule that keeps this safe. An alias pointing at a workflow the tree does
/// not contain is left completely alone: it belongs to a team drive, or to an
/// object outside the mirror, and an import has nothing to say about either.
/// Replacing the whole alias list with the tree's would wipe those, and they
/// have no file anywhere to come back from.
///
/// So the shape is deliberately unlike the object side. Objects use absence to
/// mean deletion because a deleted object still exports as a trashed one, which
/// makes absence meaningful. Aliases have no such tombstone — an alias that is
/// gone is just gone — so absence only counts *within* the workflows the tree
/// speaks for.
///
/// Runs after the objects have landed, because an alias for a workflow that
/// does not exist yet would point at nothing. That ordering also means
/// `trash_absent` has already run, and `WorkflowAliases::connect` drops the
/// aliases of any workflow trashed by it — which is the same thing that happens
/// when a workflow is trashed from the panel, and is consistent here since a
/// workflow with no file has no aliases in the tree either.
fn apply_aliases<A>(objects: &[PlacedObject], summary: &mut ApplySummary, app: &mut A) -> Result<()>
where
    A: UpdateModel + ReadModel + GetSingletonModelHandle,
{
    let described: Vec<(SyncId, &[Alias])> = objects
        .iter()
        .filter(|placed| placed.object.object_type == ObjectType::Workflow)
        .map(|placed| (placed.object.id, placed.object.aliases.as_slice()))
        .collect();
    let described_ids: HashSet<SyncId> = described.iter().map(|(id, _)| *id).collect();

    let wanted: Vec<WorkflowAlias> = described
        .iter()
        .flat_map(|(workflow_id, aliases)| {
            aliases.iter().map(move |alias| WorkflowAlias {
                alias: alias.alias.clone(),
                workflow_id: *workflow_id,
                arguments: alias
                    .arguments
                    .as_ref()
                    .map(|arguments| arguments.clone().into_iter().collect()),
                // A hand-mangled id drops the environment rather than the whole
                // alias: the shortcut is the part worth keeping.
                env_vars: alias
                    .env_vars
                    .as_deref()
                    .and_then(|id| super::format::sync_id_from_str(id).ok()),
            })
        })
        .collect();
    let wanted_names: HashSet<&str> = wanted.iter().map(|alias| alias.alias.as_str()).collect();

    WorkflowAliases::handle(app).update(app, |aliases, ctx| {
        let before = aliases.get_all_aliases().to_vec();

        let stale: Vec<String> = before
            .iter()
            .filter(|alias| {
                described_ids.contains(&alias.workflow_id)
                    && !wanted_names.contains(alias.alias.as_str())
            })
            .map(|alias| alias.alias.clone())
            .collect();

        summary.aliases_reassigned = before
            .iter()
            .filter(|alias| {
                !described_ids.contains(&alias.workflow_id)
                    && wanted_names.contains(alias.alias.as_str())
            })
            .map(|alias| alias.alias.clone())
            .collect();

        // Only the entries that actually differ, so importing the same tree
        // twice writes settings once — the alias half of the idempotence the
        // object half already has.
        let changed: Vec<WorkflowAlias> = wanted
            .iter()
            .filter(|alias| !before.contains(alias))
            .cloned()
            .collect();

        summary.aliases_set = changed.len();
        summary.aliases_removed = stale.len();

        if !stale.is_empty() {
            aliases.remove_aliases(stale, ctx)?;
        }
        if !changed.is_empty() {
            aliases.set_aliases(changed, ctx)?;
        }
        // Loud rather than counted: the objects are already in the store by
        // now, so a silent failure here leaves the drive imported and the
        // shortcuts to it in a state nobody has been told about.
        Ok(())
    })
}

/// Writes one object, returning whether it was created rather than updated.
///
/// The thirteen-way dispatch. Only three shapes underneath it — the ten JSON
/// types share one body, because they share one payload column.
fn write_object(
    placed: &PlacedObject,
    cloud_model: &mut CloudModel,
    ctx: &mut ModelContext<CloudModel>,
) -> Result<bool> {
    let object = &placed.object;

    match object.object_type {
        ObjectType::Workflow => {
            let workflow: Workflow = json_payload(object)?;
            write_typed::<WorkflowId, CloudWorkflowModel>(
                placed,
                CloudWorkflowModel::new(workflow),
                cloud_model,
                ctx,
            )
        }
        ObjectType::Notebook => {
            let Payload::Notebook {
                markdown,
                ai_document_id,
            } = &object.payload
            else {
                bail!("a notebook must carry markdown");
            };
            write_typed::<NotebookId, CloudNotebookModel>(
                placed,
                CloudNotebookModel {
                    title: object.name.clone(),
                    data: markdown.clone(),
                    // A malformed id in a hand-edited file drops the link
                    // rather than failing the whole object: the notebook's
                    // prose is the part worth saving.
                    ai_document_id: ai_document_id
                        .clone()
                        .and_then(|id| AIDocumentId::try_from(id).ok()),
                    // Names a conversation on Warp's server, which is why the
                    // format does not carry it and neither does SQLite.
                    conversation_id: None,
                },
                cloud_model,
                ctx,
            )
        }
        ObjectType::Folder => {
            let Payload::Folder { is_warp_pack } = &object.payload else {
                bail!("a folder must carry its folder metadata");
            };
            // `is_open` is sidebar state and deliberately absent from the file,
            // so an import must not decide it. Keep whatever this machine
            // already had, and default closed for a folder arriving for the
            // first time.
            let is_open = cloud_model
                .get_folder(&object.id)
                .is_some_and(|folder| folder.model().is_open);
            write_typed::<FolderId, CloudFolderModel>(
                placed,
                CloudFolderModel {
                    name: object.name.clone(),
                    is_open,
                    is_warp_pack: *is_warp_pack,
                },
                cloud_model,
                ctx,
            )
        }
        ObjectType::GenericStringObject(GenericStringObjectFormat::Json(json_type)) => {
            write_json_object(placed, json_type, cloud_model, ctx)
        }
    }
}

/// The ten JSON types. Each is `GenericCloudObject<GenericStringObjectId, M>`
/// over a different `M`, so the arms differ only in that one parameter.
fn write_json_object(
    placed: &PlacedObject,
    json_type: JsonObjectType,
    cloud_model: &mut CloudModel,
    ctx: &mut ModelContext<CloudModel>,
) -> Result<bool> {
    macro_rules! write_as {
        ($model:ty) => {{
            let serialized = serialized_payload(&placed.object)?;
            let model = <$model>::deserialize_owned(&serialized)?;
            write_typed::<GenericStringObjectId, $model>(placed, model, cloud_model, ctx)
        }};
    }

    match json_type {
        JsonObjectType::Preference => write_as!(CloudPreferenceModel),
        JsonObjectType::EnvVarCollection => write_as!(CloudEnvVarCollectionModel),
        JsonObjectType::WorkflowEnum => write_as!(CloudWorkflowEnumModel),
        JsonObjectType::AIFact => write_as!(CloudAIFactModel),
        JsonObjectType::MCPServer => write_as!(CloudMCPServerModel),
        JsonObjectType::AIExecutionProfile => write_as!(CloudAIExecutionProfileModel),
        JsonObjectType::TemplatableMCPServer => write_as!(CloudTemplatableMCPServerModel),
        JsonObjectType::CloudEnvironment => write_as!(CloudAmbientAgentEnvironmentModel),
        JsonObjectType::ScheduledAmbientAgent => write_as!(CloudScheduledAmbientAgentModel),
        JsonObjectType::CloudAgentConfig => write_as!(CloudAgentConfigModel),
    }
}

/// Creates or replaces one object. Returns whether it was created.
fn write_typed<K, M>(
    placed: &PlacedObject,
    model: M,
    cloud_model: &mut CloudModel,
    ctx: &mut ModelContext<CloudModel>,
) -> Result<bool>
where
    K: HashableId
        + ToServerId
        + std::fmt::Debug
        + Into<String>
        + Clone
        + Copy
        + Send
        + Sync
        + 'static,
    M: CloudModelType<IdType = K, CloudObjectType = GenericCloudObject<K, M>> + 'static,
{
    let id = placed.object.id;
    let uid = id.uid();
    let metadata = metadata_for(placed)?;
    let permissions = CloudObjectPermissions {
        owner: placed.object.owner,
        permissions_last_updated_ts: None,
        anyone_with_link: None,
        guests: Vec::new(),
    };

    if cloud_model.get_by_uid(&uid).is_some() {
        // Emits `ObjectUpdated`, which is what the panel and every open view
        // are listening for; replacing the object wholesale would emit
        // `ObjectCreated` for something the user already has open.
        cloud_model.update_object_from_edit::<K, M>(model, id, ctx);
        if let Some(object) = cloud_model.get_mut_by_uid(&uid) {
            *object.metadata_mut() = metadata;
            *object.permissions_mut() = permissions;
        }
        Ok(false)
    } else {
        cloud_model.create_object(
            id,
            GenericCloudObject::<K, M>::new(id, model, metadata, permissions),
            ctx,
        );
        Ok(true)
    }
}

fn metadata_for(placed: &PlacedObject) -> Result<CloudObjectMetadata> {
    let object = &placed.object;

    Ok(CloudObjectMetadata {
        revision: object
            .revision_ts
            .map(Revision::from_unix_timestamp_micros)
            .transpose()?,
        metadata_last_updated_ts: object
            .metadata_last_updated_ts
            .map(server_timestamp)
            .transpose()?,
        trashed_ts: object.trashed_ts.map(server_timestamp).transpose()?,
        folder_id: placed.parent,
        is_welcome_object: object.is_welcome_object,
        creator_uid: object.creator_uid.clone(),
        last_editor_uid: object.last_editor_uid.clone(),
        // Runtime state belonging to the "grab the baton" flow, and to a server
        // conversation this fork does not have.
        current_editor_uid: None,
        last_task_run_ts: None,
        // An imported object has never been sent anywhere, which is exactly
        // what the store already says about every object here — and what draws
        // the "Saved locally" indicator rather than a sync spinner.
        pending_changes_statuses: CloudObjectStatuses {
            content_sync_status: CloudObjectSyncStatus::InFlight(NumInFlightRequests(1)),
            has_pending_metadata_change: false,
            has_pending_permissions_change: false,
            pending_untrash: false,
            pending_delete: false,
        },
    })
}

/// Trashes objects the store has and the tree does not.
fn trash_absent<A>(
    objects: &[PlacedObject],
    existing: &HashMap<String, PlacedObject>,
    app: &mut A,
) -> usize
where
    A: UpdateModel + ReadModel + GetSingletonModelHandle,
{
    let incoming: std::collections::HashSet<String> = objects
        .iter()
        .map(|placed| placed.object.id.to_string())
        .collect();

    let absent: Vec<SyncId> = existing
        .values()
        // Already trashed objects are left alone: they are absent from the
        // tree for the same reason they are trashed, and trashing them again
        // would rewrite the timestamp on every import.
        .filter(|placed| placed.object.trashed_ts.is_none())
        .filter(|placed| !incoming.contains(&placed.object.id.to_string()))
        .map(|placed| placed.object.id)
        .collect();

    if absent.is_empty() {
        return 0;
    }

    let type_and_ids = CloudModel::handle(app).read(app, |cloud_model, _| {
        absent
            .iter()
            .filter_map(|id| cloud_model.get_by_uid(&id.uid()))
            .map(|object| object.cloud_object_type_and_id())
            .collect::<Vec<_>>()
    });

    // Routed through `UpdateManager::trash_object` rather than reimplemented,
    // so an object vanishing from the tree lands in exactly the state pressing
    // Trash in the panel would leave it — including the SQLite write.
    let trashed = type_and_ids.len();
    UpdateManager::handle(app).update(app, |update_manager, ctx| {
        for type_and_id in type_and_ids {
            update_manager.trash_object(type_and_id, ctx);
        }
    });

    trashed
}

/// Writes the objects this apply touched to SQLite.
///
/// Without this the whole import lives in memory and is gone at the next
/// restart — which is the failure a round-trip test in one process cannot see.
fn persist<A>(uids: &[String], app: &mut A)
where
    A: UpdateModel + ReadModel + GetSingletonModelHandle,
{
    if uids.is_empty() {
        return;
    }

    let events = CloudModel::handle(app).read(app, |cloud_model, _| {
        uids.iter()
            .filter_map(|uid| cloud_model.get_by_uid(uid))
            .map(|object| object.upsert_event())
            .collect::<Vec<_>>()
    });

    UpdateManager::handle(app).update(app, |update_manager, _| {
        update_manager.save_to_db(events);
    });
}

fn json_payload<T: serde::de::DeserializeOwned>(object: &PortableObject) -> Result<T> {
    let Payload::Json(value) = &object.payload else {
        bail!("{:?} must carry a json payload", object.object_type);
    };
    Ok(serde_json::from_value(value.clone())?)
}

/// The payload as the string SQLite would hold, which is what every
/// `GenericStringModel` deserializer takes.
fn serialized_payload(object: &PortableObject) -> Result<String> {
    let Payload::Json(value) = &object.payload else {
        bail!("{:?} must carry a json payload", object.object_type);
    };
    Ok(serde_json::to_string(value)?)
}

fn server_timestamp(micros: i64) -> Result<warp_graphql::scalars::time::ServerTimestamp> {
    warp_graphql::scalars::time::ServerTimestamp::from_unix_timestamp_micros(micros)
        .map_err(|err| anyhow!("unusable timestamp {micros}: {err}"))
}

#[cfg(test)]
#[path = "apply_tests.rs"]
mod tests;
