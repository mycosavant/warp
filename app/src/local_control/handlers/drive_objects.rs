//! `drive.object.list`, `drive.object.get`, `drive.object.create` and
//! `drive.object.trash` — the Warp Drive object store, one object at a time.
//!
//! T1.12. `drive.sync.*` moves the whole store to and from a directory, which
//! is the right shape for a git mirror and the wrong one for "make me a
//! workflow that does X". Until these existed the catalog could drive every
//! part of the app *except* its object store, which made the fork's own
//! headline feature the single thing an agent could not exercise. `input.*`
//! writes to the terminal's input editor rather than to whatever has focus, so
//! the panel's `+` button was not reachable either.
//!
//! # Why the read side and the write side disagree about format
//!
//! [`get`] returns the object's file exactly as an export would write it, and
//! [`create`] does not accept one. That asymmetry is deliberate.
//!
//! The file's header opens with a `uid` and an `owner`. Neither is a caller's
//! to choose — an identity supplied from outside is precisely how one object
//! silently overwrites another — so a `create` that took a file would have to
//! ignore the first two lines of everything it was handed. Asking instead for
//! the three things that *are* the caller's (what kind, what it is called,
//! what is in it) is a contract that means what it says. The action that
//! writes a supplied identity on purpose is `drive.sync.import`, where the
//! identity comes from a file the user has in git and can see.
//!
//! The file is still the documentation. `drive object get` on any existing
//! object prints a worked example of that type's body, which is the intended
//! way to learn a shape this module deliberately does not restate.
//!
//! # Why creating goes through `apply::put` and not `apply`
//!
//! `apply` is reconciliation: absence means deletion, so handing it a single
//! object would trash the rest of the drive. `put` is the same thirteen
//! constructors with no opinion about what is not in front of it.

use std::collections::HashMap;

use ::local_control::protocol::{
    DriveObjectCreateParams, DriveObjectGetParams, DriveObjectListParams, DriveObjectListResult,
    DriveObjectResult, DriveObjectSummary, DriveObjectTrashParams, DriveObjectTrashedResult,
    DriveObjectWrittenResult,
};
use ::local_control::{ControlError, ErrorCode};
use serde::Serialize;
use warpui::{ModelContext, SingletonEntity};

use crate::cloud_object::ObjectType;
use crate::drive::local_sync::apply;
use crate::drive::local_sync::format::{Payload, PortableObject};
use crate::drive::local_sync::snapshot::snapshot;
use crate::drive::local_sync::tree::PlacedObject;
use crate::local_control::LocalControlBridge;
use crate::server::ids::{ClientId, SyncId};
use crate::workspaces::user_workspaces::UserWorkspaces;

pub(crate) fn list(
    params: &serde_json::Value,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    let params: DriveObjectListParams = parse(params)?;
    let wanted = params
        .object_type
        .as_deref()
        .map(parse_object_type)
        .transpose()?;

    let (objects, summary) = snapshot(ctx);
    let paths = Paths::of(&objects);

    let mut trashed_hidden = 0;
    let mut listed = Vec::new();
    for placed in &objects {
        if wanted.is_some_and(|wanted| wanted != placed.object.object_type) {
            continue;
        }
        if placed.object.trashed_ts.is_some() && !params.include_trashed {
            trashed_hidden += 1;
            continue;
        }
        listed.push(summarize(placed, &paths));
    }

    to_control_data(DriveObjectListResult {
        objects: listed,
        not_personal: summary.not_personal,
        trashed_hidden,
        unreadable: summary.unreadable,
    })
}

pub(crate) fn get(
    params: &serde_json::Value,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    let params: DriveObjectGetParams = parse(params)?;

    let (objects, _) = snapshot(ctx);
    let placed = find(&objects, &params.id)?;
    let paths = Paths::of(&objects);

    // The same bytes an export would write. Trashed objects are returned
    // rather than refused: `list --include-trashed` names them, and being told
    // "no such object" about one you can see in the trash would be a lie.
    let contents = placed.object.to_file_contents().map_err(|err| {
        ControlError::with_details(
            ErrorCode::Internal,
            format!("drive.object.get could not render {}", params.id),
            format!("{err:#}"),
        )
    })?;

    to_control_data(DriveObjectResult {
        summary: summarize(placed, &paths),
        contents,
    })
}

pub(crate) fn create(
    params: &serde_json::Value,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    let params: DriveObjectCreateParams = parse(params)?;
    if params.name.trim().is_empty() {
        return Err(ControlError::new(
            ErrorCode::InvalidParams,
            "drive.object.create requires a non-empty name",
        ));
    }
    let object_type = parse_object_type(&params.object_type)?;
    let payload = payload_for(object_type, params.body.as_deref())?;

    // Resolved before anything is written, and refused rather than silently
    // reparented to the top level. An object that lands somewhere other than
    // where it was asked to go is worse than one that does not land: the
    // caller is told nothing and the user finds it later, elsewhere.
    let parent = params
        .folder
        .as_deref()
        .map(|folder| {
            let (objects, _) = snapshot(ctx);
            let placed = find(&objects, folder)?;
            if !matches!(placed.object.payload, Payload::Folder { .. }) {
                return Err(ControlError::with_details(
                    ErrorCode::InvalidParams,
                    format!("{folder} is not a folder"),
                    format!("it is a {}", type_name(placed.object.object_type)),
                ));
            }
            Ok(placed.object.id)
        })
        .transpose()?;

    let owner = UserWorkspaces::as_ref(ctx)
        .personal_drive(ctx)
        .ok_or_else(|| {
            ControlError::new(
                ErrorCode::TargetStateConflict,
                "there is no personal drive to create this in yet",
            )
        })?;

    let object = PortableObject {
        // A fresh client id, which is what every object this machine creates
        // gets. Nothing here has ever been near a server, and `ClientId` is
        // the store's own word for that.
        id: SyncId::ClientId(ClientId::new()),
        object_type,
        name: params.name.clone(),
        owner,
        // Left unset rather than stamped with "now". `revision` is the
        // server's word for a version this object does not have, and
        // `metadata_last_updated_ts` records a *change* to metadata, which
        // creation is not. `apply` writes both as `None` for the same reason.
        revision_ts: None,
        metadata_last_updated_ts: None,
        trashed_ts: None,
        creator_uid: None,
        last_editor_uid: None,
        is_welcome_object: false,
        // Aliases are a settings group keyed by workflow id, so one cannot
        // exist before the workflow does. `drive.sync.import` is where they
        // arrive, carried in the workflow's own file.
        aliases: Vec::new(),
        payload,
    };
    let placed = PlacedObject { object, parent };

    apply::put(&placed, ctx).map_err(|err| {
        ControlError::with_details(
            ErrorCode::InvalidParams,
            format!(
                "drive.object.create could not write a {}",
                params.object_type
            ),
            format!("{err:#}"),
        )
    })?;

    let (objects, _) = snapshot(ctx);
    let paths = Paths::of(&objects);

    log::info!(
        "Warp Drive: created {} {:?} ({})",
        type_name(object_type),
        placed.object.name,
        placed.object.id
    );

    to_control_data(DriveObjectWrittenResult {
        id: placed.object.id.to_string(),
        object_type: type_name(object_type),
        name: placed.object.name.clone(),
        path: paths.of_parent(placed.parent),
    })
}

pub(crate) fn trash(
    params: &serde_json::Value,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    let params: DriveObjectTrashParams = parse(params)?;

    let (objects, _) = snapshot(ctx);
    let placed = find(&objects, &params.id)?;
    let id = placed.object.id;
    let name = placed.object.name.clone();

    // Trashed, not deleted — the rule the whole local-sync design hangs off,
    // and the reason this action is safe to expose at all. A caller that got
    // the id wrong has cost the user a restore from the panel rather than
    // their work. Already-trashed is `false`, not an error: the object is in
    // the state that was asked for.
    let trashed = apply::trash(id, ctx);
    if trashed {
        log::info!("Warp Drive: trashed {name:?} ({id})");
    }

    to_control_data(DriveObjectTrashedResult {
        id: id.to_string(),
        name,
        trashed,
    })
}

/// Every folder's display name and parent, for turning a `folder_id` chain into
/// something a person can read.
///
/// Names rather than the mirror's slugged directory names: this answers "where
/// is it in the panel", and the panel shows names.
struct Paths {
    folders: HashMap<String, (String, Option<SyncId>)>,
}

impl Paths {
    fn of(objects: &[PlacedObject]) -> Self {
        Self {
            folders: objects
                .iter()
                .filter(|placed| matches!(placed.object.payload, Payload::Folder { .. }))
                .map(|placed| {
                    (
                        placed.object.id.to_string(),
                        (placed.object.name.clone(), placed.parent),
                    )
                })
                .collect(),
        }
    }

    fn of_parent(&self, parent: Option<SyncId>) -> Vec<String> {
        let mut path = Vec::new();
        let mut seen = Vec::new();
        let mut next = parent;

        // `folder_id` is a plain string column with no referential integrity
        // behind it, so a cycle is representable and would loop forever here.
        // The same guard exists in `tree`'s exporter and in upstream's own
        // `is_trashed_internal`, for the same reason.
        while let Some(id) = next {
            let key = id.to_string();
            if seen.contains(&key) {
                break;
            }
            let Some((name, parent)) = self.folders.get(&key) else {
                // A parent outside the personal drive, or trashed away. The
                // export reparents these to the top level and says so; here
                // the shorter path is the honest answer.
                break;
            };
            path.push(name.clone());
            seen.push(key);
            next = *parent;
        }

        path.reverse();
        path
    }
}

fn summarize(placed: &PlacedObject, paths: &Paths) -> DriveObjectSummary {
    DriveObjectSummary {
        id: placed.object.id.to_string(),
        object_type: type_name(placed.object.object_type),
        name: placed.object.name.clone(),
        path: paths.of_parent(placed.parent),
        trashed: placed.object.trashed_ts.is_some(),
        aliases: placed
            .object
            .aliases
            .iter()
            .map(|alias| alias.alias.clone())
            .collect(),
    }
}

fn find<'a>(objects: &'a [PlacedObject], id: &str) -> Result<&'a PlacedObject, ControlError> {
    objects
        .iter()
        .find(|placed| placed.object.id.to_string() == id)
        .ok_or_else(|| {
            ControlError::with_details(
                ErrorCode::MissingTarget,
                format!("no Warp Drive object with id {id}"),
                "`drive object list` reports the ids this instance holds. Objects in a team \
                 drive or shared by someone else are not among them."
                    .to_owned(),
            )
        })
}

fn parse_object_type(value: &str) -> Result<ObjectType, ControlError> {
    value.parse().map_err(|_| {
        ControlError::with_details(
            ErrorCode::InvalidParams,
            format!("{value} is not a Warp Drive object type"),
            "Use `workflow`, `notebook`, `folder`, `prompt` or `env-vars`.".to_owned(),
        )
    })
}

fn payload_for(object_type: ObjectType, body: Option<&str>) -> Result<Payload, ControlError> {
    match object_type {
        ObjectType::Folder => {
            if body.is_some() {
                return Err(ControlError::new(
                    ErrorCode::InvalidParams,
                    "a folder has no body; put objects in it with --folder instead",
                ));
            }
            // A Warp Pack is a folder published as a shareable bundle, which is
            // a sharing decision made in the panel rather than a property of a
            // folder being created.
            Ok(Payload::Folder {
                is_warp_pack: false,
            })
        }
        ObjectType::Notebook => Ok(Payload::Notebook {
            markdown: body.unwrap_or_default().to_owned(),
            // Set when a notebook is generated from an agent conversation.
            // Nothing here has one, and inventing a link to a document that
            // does not exist would be worse than the absent link.
            ai_document_id: None,
        }),
        ObjectType::Workflow | ObjectType::GenericStringObject(_) => {
            let body = body.ok_or_else(|| {
                ControlError::with_details(
                    ErrorCode::InvalidParams,
                    format!("a {} needs a JSON body", type_name(object_type)),
                    "`drive object get <id>` on an existing one prints the shape.".to_owned(),
                )
            })?;
            let value = serde_json::from_str(body).map_err(|err| {
                ControlError::with_details(
                    ErrorCode::InvalidParams,
                    format!("the body of a {} must be JSON", type_name(object_type)),
                    format!("{err}. `drive object get <id>` on an existing one prints the shape."),
                )
            })?;
            Ok(Payload::Json(value))
        }
    }
}

/// The protocol spelling of an object type.
///
/// The three concrete kinds get the word `ObjectType::from_str` accepts, so
/// what `list` prints is what `create` takes. The JSON-backed types keep the
/// file format's spelling instead: there are ten of them and upstream's parser
/// has a friendly word for exactly one (`env-vars`), so inventing nine more
/// would be a second naming system to keep in step — which is the thing
/// `format::object_type_to_str` deliberately refuses to do.
///
/// The consequence is real and worth knowing: `list` can name a type `create`
/// cannot make. That is upstream's gap, reported rather than papered over.
fn type_name(object_type: ObjectType) -> String {
    match object_type {
        ObjectType::Workflow => "workflow".to_owned(),
        ObjectType::Notebook => "notebook".to_owned(),
        ObjectType::Folder => "folder".to_owned(),
        ObjectType::GenericStringObject(_) => object_type.sqlite_object_type_as_str().into_owned(),
    }
}

fn parse<T: serde::de::DeserializeOwned>(params: &serde_json::Value) -> Result<T, ControlError> {
    serde_json::from_value(params.clone())
        .map_err(|err| ControlError::new(ErrorCode::InvalidParams, err.to_string()))
}

fn to_control_data<T: Serialize>(value: T) -> Result<serde_json::Value, ControlError> {
    serde_json::to_value(value)
        .map_err(|err| ControlError::new(ErrorCode::Internal, err.to_string()))
}

#[cfg(test)]
#[path = "drive_objects_tests.rs"]
mod tests;
