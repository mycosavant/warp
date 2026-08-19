//! T4.4a — the portable, on-disk form of one Warp Drive object.
//!
//! # What "lossless" means here
//!
//! A file carries an object's **identity, content and content-level metadata**,
//! and nothing else. It deliberately does *not* carry:
//!
//! * local SQLite row ids (`id`, `shareable_object_id`, `author_id`) — these are
//!   per-machine integers, meaningless in another checkout, and re-minted on
//!   import;
//! * sync bookkeeping (`is_pending`, `retry_count`, `current_editor`) — state of
//!   a server conversation this fork does not have;
//! * `folders.is_open` — sidebar view state. It is the single largest source of
//!   spurious churn available: expanding a folder would dirty the repo.
//! * the parent folder id — placement is expressed by *where the file sits*, so
//!   that moving an object is a file rename and git reports it as one. Two
//!   representations of placement would be two things to disagree.
//!
//! Everything else round-trips exactly. The [`PortableObject`] type is the
//! neutral middle: [`to_file_contents`](PortableObject::to_file_contents) and
//! [`from_file_contents`](PortableObject::from_file_contents) are inverses.
//!
//! # Two envelopes, one header
//!
//! Notebooks are prose, and prose belongs in a file a diff can read. Everything
//! else is structured data that `serde_json` already produced. So there are two
//! envelopes sharing one [`Header`]:
//!
//! ```text
//! notebook          <slug>-<hash>.md      YAML front matter, markdown body
//! everything else   <slug>-<hash>.json    one JSON object, payload under "data"
//! folder            <dir>/.warp-folder.json
//! ```
//!
//! JSON payloads are re-emitted rather than converted to YAML on purpose.
//! Converting would make prettier diffs and would also introduce a class of bug
//! this format cannot afford — YAML's plain scalars turn a workflow argument
//! named `on` or `no` into a boolean. `serde_json` round-trips its own output
//! exactly, and its maps are `BTreeMap`, so keys are sorted and the bytes are
//! stable.

use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::auth::UserUid;
use crate::cloud_object::{
    GENERIC_STRING_OBJECT_PREFIX, GenericStringObjectFormat, JSON_OBJECT_PREFIX, JsonObjectType,
    ObjectType, Owner,
};
use crate::server::ids::{ClientId, HashableId, ServerId, SyncId};

/// Written into every file and checked on read. A file from a future version is
/// refused rather than half-understood.
pub const FORMAT_VERSION: u32 = 1;

/// The metadata file inside a folder's directory. Dot-prefixed so it sorts to
/// the top and reads as machinery rather than content.
pub const FOLDER_FILE_NAME: &str = ".warp-folder.json";

const FRONT_MATTER_FENCE: &str = "---";
const SLUG_HASH_LEN: usize = 8;
const MAX_SLUG_LEN: usize = 48;
const UNNAMED_SLUG: &str = "untitled";

/// One Warp Drive object, in the form that goes to disk.
///
/// Built from the SQLite rows for an object rather than from the in-memory
/// `dyn CloudObject`, because the store is what T4.2 made authoritative.
#[derive(Debug, Clone, PartialEq)]
pub struct PortableObject {
    /// The object's identity: a client id for an object this machine created,
    /// or a server id for one that predates going local-first.
    pub id: SyncId,
    pub object_type: ObjectType,
    /// The object's display name. Also the source of the filename slug, which
    /// is why it is carried for types whose name lives outside the payload.
    pub name: String,
    pub owner: Owner,
    pub revision_ts: Option<i64>,
    pub metadata_last_updated_ts: Option<i64>,
    pub trashed_ts: Option<i64>,
    pub creator_uid: Option<String>,
    pub last_editor_uid: Option<String>,
    pub is_welcome_object: bool,
    pub payload: Payload,
}

/// The four payload shapes the store actually has.
///
/// `workflows.data` and `generic_string_objects.data` are both `serde_json`
/// output, which collapses ten object types — AI facts, MCP servers, execution
/// profiles, cloud preferences and the rest — into a single case.
#[derive(Debug, Clone, PartialEq)]
pub enum Payload {
    Notebook {
        markdown: String,
        ai_document_id: Option<String>,
    },
    Folder {
        is_warp_pack: bool,
    },
    Json(serde_json::Value),
}

/// The shared header. Field order here is the field order in the file, for both
/// envelopes, and `serde` preserves it — so the bytes are stable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Header {
    pub warp_drive: u32,
    #[serde(rename = "type")]
    pub object_type: String,
    pub uid: String,
    pub name: String,
    pub owner: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trashed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_editor: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub welcome: bool,
    /// Notebooks only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_document: Option<String>,
    /// Folders only.
    #[serde(default, skip_serializing_if = "is_false")]
    pub warp_pack: bool,
}

/// The JSON envelope. `data` comes last so the payload is the tail of the file
/// and the header stays a readable preamble.
#[derive(Debug, Serialize, Deserialize)]
struct JsonEnvelope {
    #[serde(flatten)]
    header: Header,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

impl PortableObject {
    /// The file's name within its containing directory.
    ///
    /// The hash suffix is not decoration: without it two objects with the same
    /// name collide, and disambiguating by sibling would make one object's
    /// filename depend on another's existence — so creating a second "deploy"
    /// would rename the first and churn the repo. The suffix makes the name a
    /// pure function of the object.
    pub fn file_name(&self) -> String {
        match &self.payload {
            Payload::Folder { .. } => FOLDER_FILE_NAME.to_owned(),
            Payload::Notebook { .. } => format!("{}.md", self.file_stem()),
            Payload::Json(_) => format!("{}.json", self.file_stem()),
        }
    }

    /// The directory a folder object owns, or `None` for anything else.
    pub fn folder_directory_name(&self) -> Option<String> {
        matches!(self.payload, Payload::Folder { .. }).then(|| self.file_stem())
    }

    fn file_stem(&self) -> String {
        format!("{}-{}", slug(&self.name), uid_hash(&self.id))
    }

    /// Serializes this object to the exact bytes of its file.
    pub fn to_file_contents(&self) -> Result<String> {
        let header = self.header()?;
        match &self.payload {
            Payload::Notebook { markdown, .. } => {
                Ok(format!("{}\n{}", front_matter(&header)?, markdown))
            }
            Payload::Folder { .. } => json_envelope(header, None),
            Payload::Json(data) => json_envelope(header, Some(data.clone())),
        }
    }

    /// Parses a file back into an object. Inverse of
    /// [`to_file_contents`](Self::to_file_contents).
    pub fn from_file_contents(contents: &str) -> Result<Self> {
        match split_front_matter(contents) {
            Some((header, body)) => {
                let header: Header = serde_yaml::from_str(header)
                    .context("front matter is not a valid Warp Drive header")?;
                Self::from_parts(header, Some(body.to_owned()), None)
            }
            None => {
                let envelope: JsonEnvelope = serde_json::from_str(contents)
                    .context("file is neither front matter nor a Warp Drive JSON object")?;
                Self::from_parts(envelope.header, None, envelope.data)
            }
        }
    }

    fn header(&self) -> Result<Header> {
        let (ai_document, warp_pack) = match &self.payload {
            Payload::Notebook { ai_document_id, .. } => (ai_document_id.clone(), false),
            Payload::Folder { is_warp_pack } => (None, *is_warp_pack),
            Payload::Json(_) => (None, false),
        };

        Ok(Header {
            warp_drive: FORMAT_VERSION,
            object_type: object_type_to_str(self.object_type),
            uid: self.id.to_string(),
            name: self.name.clone(),
            owner: owner_to_str(&self.owner),
            revision: self.revision_ts.map(micros_to_rfc3339).transpose()?,
            updated: self
                .metadata_last_updated_ts
                .map(micros_to_rfc3339)
                .transpose()?,
            trashed: self.trashed_ts.map(micros_to_rfc3339).transpose()?,
            creator: self.creator_uid.clone(),
            last_editor: self.last_editor_uid.clone(),
            welcome: self.is_welcome_object,
            ai_document,
            warp_pack,
        })
    }

    fn from_parts(
        header: Header,
        body: Option<String>,
        data: Option<serde_json::Value>,
    ) -> Result<Self> {
        if header.warp_drive > FORMAT_VERSION {
            bail!(
                "file was written by a newer Warp Drive format (v{}, this build understands v{FORMAT_VERSION})",
                header.warp_drive
            );
        }

        let object_type = object_type_from_str(&header.object_type)?;
        let payload = match object_type {
            ObjectType::Notebook => Payload::Notebook {
                markdown: body.ok_or_else(|| {
                    anyhow!("a notebook file must carry its markdown after the front matter")
                })?,
                ai_document_id: header.ai_document,
            },
            ObjectType::Folder => Payload::Folder {
                is_warp_pack: header.warp_pack,
            },
            ObjectType::Workflow | ObjectType::GenericStringObject(_) => Payload::Json(
                data.ok_or_else(|| anyhow!("a {object_type:?} file must carry a \"data\" key"))?,
            ),
        };

        Ok(Self {
            id: sync_id_from_str(&header.uid)?,
            object_type,
            name: header.name,
            owner: owner_from_str(&header.owner)?,
            revision_ts: header
                .revision
                .as_deref()
                .map(rfc3339_to_micros)
                .transpose()?,
            metadata_last_updated_ts: header
                .updated
                .as_deref()
                .map(rfc3339_to_micros)
                .transpose()?,
            trashed_ts: header
                .trashed
                .as_deref()
                .map(rfc3339_to_micros)
                .transpose()?,
            creator_uid: header.creator,
            last_editor_uid: header.last_editor,
            is_welcome_object: header.welcome,
            payload,
        })
    }
}

/// A file git has left half-merged.
///
/// Warp never resolves one of these, and T4.4e is the decision to say so out
/// loud. Picking a side would be exactly the "work out which version wins"
/// behaviour that decision 1 rejected — and it would do it silently, on the one
/// occasion the user is demonstrably already looking at the file. Both sides are
/// reconstructed here for one purpose only: telling a conflicted *Warp Drive*
/// file apart from a conflicted README that happens to share the repository.
#[derive(Debug, Clone, PartialEq)]
pub struct Conflict {
    /// 1-based line of the opening `<<<<<<<`, so a message can point at it.
    pub line: usize,
    /// The file as it would read with our side of every region taken.
    pub ours: String,
    /// The same, with their side taken.
    pub theirs: String,
}

/// Git writes seven marker characters by default, and the `conflict-marker-size`
/// attribute only ever raises that — so the length is a minimum, not a match.
const CONFLICT_MARKER_LEN: usize = 7;

/// Detects an unresolved merge, or `None` for an ordinary file.
///
/// Two deliberate narrowings, both there to avoid crying conflict over a
/// perfectly good file:
///
/// * a region must be **closed**. An opening marker on its own is a line
///   somebody wrote, not a merge.
/// * `=======` counts as a separator only *between* markers. A bare row of
///   equals signs is a markdown setext heading underline far more often than it
///   is anything else, and notebooks are markdown — so a notebook whose
///   headings are underlined that way has to pass through untouched.
pub fn conflict(contents: &str) -> Option<Conflict> {
    enum Side {
        Outside,
        Ours,
        /// The `|||||||` common ancestor of `merge.conflictStyle = diff3`.
        /// Discarded: it is neither side, and no one merges by hand from it.
        Base,
        Theirs,
    }

    let mut ours = String::new();
    let mut theirs = String::new();
    let mut side = Side::Outside;
    let mut opened_at = None;
    let mut closed = false;

    for (index, line) in contents.split_inclusive('\n').enumerate() {
        match side {
            Side::Outside if is_marker(line, '<') => {
                opened_at.get_or_insert(index + 1);
                side = Side::Ours;
            }
            Side::Outside => {
                ours.push_str(line);
                theirs.push_str(line);
            }
            Side::Ours if is_marker(line, '|') => side = Side::Base,
            Side::Ours if is_marker(line, '=') => side = Side::Theirs,
            // A region closed without a separator is malformed, but it is still
            // unmistakably a merge, and reading the rest of the file as content
            // would be worse than tolerating it.
            Side::Ours if is_marker(line, '>') => {
                closed = true;
                side = Side::Outside;
            }
            Side::Ours => ours.push_str(line),
            Side::Base if is_marker(line, '=') => side = Side::Theirs,
            Side::Base => {}
            Side::Theirs if is_marker(line, '>') => {
                closed = true;
                side = Side::Outside;
            }
            Side::Theirs => theirs.push_str(line),
        }
    }

    closed.then(|| Conflict {
        line: opened_at.unwrap_or(1),
        ours,
        theirs,
    })
}

/// A marker line: at least [`CONFLICT_MARKER_LEN`] of the character, then either
/// nothing or the label git appends (`<<<<<<< HEAD`).
fn is_marker(line: &str, marker: char) -> bool {
    let line = line.trim_end();
    // Every marker character is ASCII, so the count is also the byte offset.
    let run = line.chars().take_while(|ch| *ch == marker).count();
    let rest = &line[run..];
    run >= CONFLICT_MARKER_LEN && (rest.is_empty() || rest.starts_with(char::is_whitespace))
}

/// Renders the header as a front-matter block, fences included.
fn front_matter(header: &Header) -> Result<String> {
    let mut yaml = serde_yaml::to_string(header).context("serializing the object header")?;

    // serde_yaml 0.8 opens its output with a document-start marker, which is
    // the same three characters as the front-matter fence. Left in place it
    // becomes the *closing* fence on read, and every header parses as empty.
    if let Some(rest) = yaml.strip_prefix(FRONT_MATTER_FENCE) {
        yaml = rest.trim_start_matches(['\r', '\n']).to_owned();
    }
    if !yaml.ends_with('\n') {
        yaml.push('\n');
    }

    // Defence against an emitter that chooses block style for a string
    // containing a newline: a notebook titled with a line that is exactly `---`
    // would otherwise put a fence inside its own header and truncate it. JSON
    // is valid YAML and escapes newlines, so a single-line JSON header cannot
    // contain a bare fence at all. serde_yaml 0.8 double-quotes such strings
    // and so does not reach this, but the emitter's choice is not ours to
    // depend on across a version bump.
    if yaml
        .lines()
        .any(|line| line.trim_end() == FRONT_MATTER_FENCE)
    {
        yaml = format!("{}\n", serde_json::to_string(header)?);
    }

    Ok(format!("{FRONT_MATTER_FENCE}\n{yaml}{FRONT_MATTER_FENCE}"))
}

/// Splits `---\n<header>\n---\n<body>`, or `None` if there is no front matter.
///
/// The closing fence is the *first* one after the opening, never the last, so a
/// body containing its own `---` lines — a horizontal rule, or nested front
/// matter someone pasted in — survives untouched.
fn split_front_matter(contents: &str) -> Option<(&str, &str)> {
    let rest = contents
        .strip_prefix(&format!("{FRONT_MATTER_FENCE}\n"))
        .or_else(|| contents.strip_prefix(&format!("{FRONT_MATTER_FENCE}\r\n")))?;

    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == FRONT_MATTER_FENCE {
            return Some((&rest[..offset], &rest[offset + line.len()..]));
        }
        offset += line.len();
    }
    None
}

fn json_envelope(header: Header, data: Option<serde_json::Value>) -> Result<String> {
    let envelope = JsonEnvelope { header, data };
    let mut contents =
        serde_json::to_string_pretty(&envelope).context("serializing the object envelope")?;
    contents.push('\n');
    Ok(contents)
}

/// The sqlite type string is reused verbatim rather than given a prettier
/// fork-local spelling. A second naming system would be a second thing to keep
/// in step with upstream, and this one already covers all thirteen types.
fn object_type_to_str(object_type: ObjectType) -> String {
    object_type.sqlite_object_type_as_str().into_owned()
}

fn object_type_from_str(value: &str) -> Result<ObjectType> {
    match value {
        "NOTEBOOK" => Ok(ObjectType::Notebook),
        "WORKFLOW" => Ok(ObjectType::Workflow),
        "FOLDER" => Ok(ObjectType::Folder),
        _ => {
            let json_type = value
                .strip_prefix(GENERIC_STRING_OBJECT_PREFIX)
                .and_then(|rest| rest.strip_prefix(JSON_OBJECT_PREFIX))
                .ok_or_else(|| anyhow!("unrecognised object type {value:?}"))?;
            Ok(ObjectType::GenericStringObject(
                GenericStringObjectFormat::Json(JsonObjectType::try_from(json_type)?),
            ))
        }
    }
}

fn owner_to_str(owner: &Owner) -> String {
    match owner {
        Owner::User { user_uid } => format!("user:{}", user_uid.as_str()),
        Owner::Team { team_uid } => format!("team:{team_uid}"),
    }
}

fn owner_from_str(value: &str) -> Result<Owner> {
    match value.split_once(':') {
        Some(("user", uid)) => Ok(Owner::User {
            user_uid: UserUid::new(uid),
        }),
        // Parsed rather than `from_string_lossy`, which panics in debug builds
        // on anything that is not exactly 22 characters. A malformed file
        // should be an error, not a crash.
        Some(("team", uid)) => Ok(Owner::Team {
            team_uid: ServerId::try_from(uid)
                .map_err(|err| anyhow!("unusable team id in {value:?}: {err}"))?,
        }),
        _ => bail!("unrecognised owner {value:?}"),
    }
}

/// `Client-<uuid>` is 43 characters and a [`ServerId`] is exactly 22, so the two
/// forms cannot be confused for each other.
fn sync_id_from_str(value: &str) -> Result<SyncId> {
    if let Some(client_id) = ClientId::from_hash(value) {
        return Ok(SyncId::ClientId(client_id));
    }
    ServerId::try_from(value)
        .map(SyncId::ServerId)
        .map_err(|err| anyhow!("unusable object id {value:?}: {err}"))
}

/// RFC 3339 at microsecond precision — the precision SQLite stores, so this is
/// exact rather than merely close, and still legible in a diff.
fn micros_to_rfc3339(micros: i64) -> Result<String> {
    DateTime::<Utc>::from_timestamp_micros(micros)
        .map(|ts| ts.to_rfc3339_opts(SecondsFormat::Micros, true))
        .ok_or_else(|| anyhow!("timestamp {micros} is outside the representable range"))
}

fn rfc3339_to_micros(value: &str) -> Result<i64> {
    Ok(DateTime::parse_from_rfc3339(value)
        .with_context(|| format!("unparseable timestamp {value:?}"))?
        .timestamp_micros())
}

/// Lowercase ASCII, hyphen-separated, no path separators and no reserved
/// characters — safe on NTFS as well as on a case-sensitive filesystem. The
/// hash suffix appended by [`PortableObject::file_stem`] also means a slug of
/// `con` or `nul` can never become a reserved Windows device name on its own.
fn slug(name: &str) -> String {
    let mut slug = String::with_capacity(name.len().min(MAX_SLUG_LEN));
    let mut pending_separator = false;

    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_separator && !slug.is_empty() {
                slug.push('-');
            }
            pending_separator = false;
            slug.push(ch.to_ascii_lowercase());
            if slug.len() >= MAX_SLUG_LEN {
                break;
            }
        } else {
            pending_separator = true;
        }
    }

    if slug.is_empty() {
        UNNAMED_SLUG.to_owned()
    } else {
        slug
    }
}

/// A short digest of the full id rather than a prefix of it. Client ids are
/// UUIDs and server ids are opaque 22-character strings that may contain
/// characters a filesystem would object to; hashing gives one filename-safe
/// rule for both.
fn uid_hash(id: &SyncId) -> String {
    let digest = Sha256::digest(id.to_string().as_bytes());
    digest
        .iter()
        .take(SLUG_HASH_LEN / 2)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(test)]
#[path = "format_tests.rs"]
mod tests;
