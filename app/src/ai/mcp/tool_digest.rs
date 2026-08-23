//! Fork: pin what an MCP tool claims to be (T8.4).
//!
//! An MCP server tells the client the name, description and JSON schema of
//! every tool it offers. The description is the part that matters: it is
//! *prompt*, authored by a third party, that the model re-reads on every turn
//! and that the user reviewed exactly once — when they installed the server.
//! Nothing in the protocol stops the server from returning a different one
//! tomorrow, and nothing in the client was watching. That is the tool rug-pull,
//! and it is the reason this file exists.
//!
//! The mechanism is trust-on-first-use. The first time a server connects, what
//! it advertises is recorded and nothing is said — there is no prior approval
//! to compare against, so a warning would be noise. On every later connect the
//! digests are compared, and a definition that changed under a name that was
//! already there is reported. Then the record is updated, so the same change is
//! reported once rather than at every launch.
//!
//! Deliberately **not** a block. A false positive that silently disables
//! somebody's tooling is worse than a warning they read, and the noise level of
//! real servers is not yet known — most description changes will be ordinary
//! upgrades. Blocking is a decision to make once there is evidence.
//!
//! This module holds no policy and does no reporting: it hashes, stores and
//! diffs. Whether to run at all is [`crate::fork::mcp_tool_pinning_enabled`],
//! and saying so is the caller's job.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rmcp::model::Tool;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

/// Bumped when the set of hashed fields or the framing changes. A store written
/// by a different version is discarded rather than migrated: the whole value of
/// the file is that a digest means one specific thing, and re-establishing the
/// baseline costs a single silent connect.
const STORE_VERSION: u32 = 1;

/// Domain separation, so a digest from this file can never be confused with a
/// same-length hash computed anywhere else in the app.
const DOMAIN: &[u8] = b"warp-fork/mcp-tool-digest/v1";

const STORE_FILE_NAME: &str = "mcp_tool_digests.json";

/// The parts of a tool definition that are hashed, each one separately so a
/// change can be attributed rather than merely detected.
///
/// `name` is not here because it is the key: a tool whose name changed is a
/// tool that was removed and another that was added, which is what the diff
/// already says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ToolField {
    Title,
    Description,
    InputSchema,
    OutputSchema,
    Annotations,
}

impl ToolField {
    pub fn label(self) -> &'static str {
        match self {
            ToolField::Title => "title",
            ToolField::Description => "description",
            ToolField::InputSchema => "input schema",
            ToolField::OutputSchema => "output schema",
            ToolField::Annotations => "annotations",
        }
    }
}

/// One tool's definition, reduced to hashes.
///
/// Hashes rather than the text itself, and that is a real trade-off: the store
/// cannot show what a description *used* to say. It is the right way round
/// anyway — the fork would otherwise keep a growing local copy of third-party
/// prompt text it does not own, and what a person needs in order to act is the
/// definition that is live *now*, which the caller has in hand.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDigest {
    /// Over every field below, in a fixed order. Present so an unchanged tool
    /// is one comparison, and so the file has something to eyeball.
    pub digest: String,
    pub title: String,
    pub description: String,
    pub input_schema: String,
    pub output_schema: String,
    pub annotations: String,
}

impl ToolDigest {
    /// Which fields differ between what was approved and what arrived.
    pub fn changed_fields(&self, current: &ToolDigest) -> Vec<ToolField> {
        [
            (ToolField::Title, &self.title, &current.title),
            (
                ToolField::Description,
                &self.description,
                &current.description,
            ),
            (
                ToolField::InputSchema,
                &self.input_schema,
                &current.input_schema,
            ),
            (
                ToolField::OutputSchema,
                &self.output_schema,
                &current.output_schema,
            ),
            (
                ToolField::Annotations,
                &self.annotations,
                &current.annotations,
            ),
        ]
        .into_iter()
        .filter(|(_, previous, current)| previous != current)
        .map(|(field, _, _)| field)
        .collect()
    }
}

/// What a server advertised, the last time it was believed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerRecord {
    pub tools: BTreeMap<String, ToolDigest>,
}

/// Every server's record.
///
/// Keyed by server **name**, not by installation id, and that is not the
/// obvious choice — the id looks more like an identity. It is not one:
/// `parsing.rs` mints a fresh `Uuid::new_v4()` for every file-based server on
/// every parse, so a `.mcp.json` server has a different installation id at each
/// launch and keying on it would make every connect look like a first connect,
/// which is exactly the case that says nothing. The name is the key in the
/// config file, it is stable, and it is what the user would be told about.
///
/// The cost is that two servers with the same name from different providers
/// share a record and would report changes against each other. Worth knowing;
/// it fails loud rather than silent, which is the right direction for this.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolDigestStore {
    pub version: u32,
    pub servers: BTreeMap<String, ServerRecord>,
}

impl Default for ToolDigestStore {
    fn default() -> Self {
        Self {
            version: STORE_VERSION,
            servers: BTreeMap::new(),
        }
    }
}

/// What changed under a server between two connects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolChange {
    /// A name that was not there before.
    Added { tool: String },
    /// A name that is no longer offered.
    Removed { tool: String },
    /// **The one that matters.** A name that was already approved, now
    /// describing itself differently.
    Redefined {
        tool: String,
        fields: Vec<ToolField>,
    },
}

impl ToolChange {
    pub fn tool(&self) -> &str {
        match self {
            ToolChange::Added { tool }
            | ToolChange::Removed { tool }
            | ToolChange::Redefined { tool, .. } => tool,
        }
    }

    /// Whether this change is worth interrupting somebody over.
    ///
    /// Only a redefinition is. A new tool is a thing the server gained and the
    /// user will see when they use it; a removed one cannot do anything. A
    /// redefinition is the only one where the answer to "did you approve this?"
    /// is no while the client behaves as though it were yes.
    pub fn is_alarming(&self) -> bool {
        matches!(self, ToolChange::Redefined { .. })
    }
}

/// Reduce a tool to its digests.
pub fn digest_tool(tool: &Tool) -> ToolDigest {
    let title = field_digest("title", tool.title.as_deref().map(str::as_bytes));
    let description = field_digest(
        "description",
        tool.description.as_deref().map(str::as_bytes),
    );
    let input_schema = json_field_digest(
        "input_schema",
        Some(&Value::Object((*tool.input_schema).clone())),
    );
    let output_schema = json_field_digest(
        "output_schema",
        tool.output_schema
            .as_ref()
            .map(|schema| Value::Object((**schema).clone()))
            .as_ref(),
    );
    let annotations = json_field_digest(
        "annotations",
        tool.annotations
            .as_ref()
            .and_then(|annotations| serde_json::to_value(annotations).ok())
            .as_ref(),
    );

    let mut hasher = Sha256::new();
    hasher.update(DOMAIN);
    for part in [
        tool.name.as_ref(),
        title.as_str(),
        description.as_str(),
        input_schema.as_str(),
        output_schema.as_str(),
        annotations.as_str(),
    ] {
        hasher.update((part.len() as u64).to_le_bytes());
        hasher.update(part.as_bytes());
    }

    ToolDigest {
        digest: hex::encode(hasher.finalize()),
        title,
        description,
        input_schema,
        output_schema,
        annotations,
    }
}

/// Reduce a server's whole advertisement to digests, keyed by tool name.
///
/// A duplicate name is not expected and is not an error here: the last one
/// wins, matching `has_tool`/`tool_input_schema`, which take the first — the
/// disagreement is only reachable from a server that is already misbehaving.
pub fn digest_tools(tools: &[Tool]) -> BTreeMap<String, ToolDigest> {
    tools
        .iter()
        .map(|tool| (tool.name.to_string(), digest_tool(tool)))
        .collect()
}

/// Diff what a server advertised now against what it advertised before.
///
/// Ordering is deterministic — redefinitions first, then additions, then
/// removals, each alphabetically — so a log line and a test read the same way
/// twice.
pub fn compare(previous: &ServerRecord, current: &BTreeMap<String, ToolDigest>) -> Vec<ToolChange> {
    let mut redefined = Vec::new();
    let mut added = Vec::new();

    for (name, digest) in current {
        match previous.tools.get(name) {
            Some(approved) if approved.digest == digest.digest => {}
            Some(approved) => redefined.push(ToolChange::Redefined {
                tool: name.clone(),
                fields: approved.changed_fields(digest),
            }),
            None => added.push(ToolChange::Added { tool: name.clone() }),
        }
    }

    let removed = previous
        .tools
        .keys()
        .filter(|name| !current.contains_key(*name))
        .map(|name| ToolChange::Removed { tool: name.clone() });

    redefined.extend(added);
    redefined.extend(removed);
    redefined
}

impl ToolDigestStore {
    /// Read the store, or start a new one.
    ///
    /// Every failure — missing, unreadable, malformed, wrong version — answers
    /// with an empty store rather than an error. The consequence of getting
    /// this wrong in the strict direction is that a corrupt file blocks MCP
    /// startup, which trades a security nicety for the user's tools; the
    /// consequence in this direction is one silent connect that re-establishes
    /// the baseline.
    pub fn load_from(path: &Path) -> Self {
        let Ok(contents) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        match serde_json::from_str::<Self>(&contents) {
            Ok(store) if store.version == STORE_VERSION => store,
            Ok(store) => {
                log::info!(
                    "Discarding MCP tool digest store written by version {} (this build writes \
                     {STORE_VERSION})",
                    store.version
                );
                Self::default()
            }
            Err(err) => {
                log::warn!("Could not read the MCP tool digest store at {path:?}: {err}");
                Self::default()
            }
        }
    }

    /// Write the store, atomically, creating the directory if needed.
    pub fn save_to(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let serialized = serde_json::to_string_pretty(self)?;
        // Write-then-rename: a store truncated by a crash mid-write would be
        // discarded on the next read, silently forgetting every approval.
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, serialized)?;
        std::fs::rename(&temporary, path)?;
        Ok(())
    }
}

/// Where the store lives.
pub fn store_path() -> PathBuf {
    crate::fork::state_dir().join(STORE_FILE_NAME)
}

/// Record what `server_name` just advertised, and report what changed since the
/// last time it connected.
///
/// The first connect of a server returns nothing: there is no approval to
/// compare against, so the advertisement *is* the approval.
pub fn record_in(store_path: &Path, server_name: &str, tools: &[Tool]) -> Vec<ToolChange> {
    let mut store = ToolDigestStore::load_from(store_path);
    let current = digest_tools(tools);

    let changes = match store.servers.get(server_name) {
        Some(previous) => compare(previous, &current),
        None => Vec::new(),
    };

    // Update even when nothing changed, so a server that vanishes from the
    // config and comes back is still measured against what it last said.
    store
        .servers
        .insert(server_name.to_owned(), ServerRecord { tools: current });
    if let Err(err) = store.save_to(store_path) {
        log::warn!("Could not record MCP tool digests for '{server_name}': {err:#}");
    }

    changes
}

/// [`record_in`] against the real store.
pub fn record(server_name: &str, tools: &[Tool]) -> Vec<ToolChange> {
    record_in(&store_path(), server_name, tools)
}

/// A one-line description of a change, for a toast or a log line.
pub fn describe(server_name: &str, change: &ToolChange) -> String {
    match change {
        ToolChange::Added { tool } => {
            format!("MCP server '{server_name}' is offering a new tool, '{tool}'.")
        }
        ToolChange::Removed { tool } => {
            format!("MCP server '{server_name}' no longer offers the tool '{tool}'.")
        }
        ToolChange::Redefined { tool, fields } => {
            let fields = fields
                .iter()
                .map(|field| field.label())
                .collect::<Vec<_>>()
                .join(", ");
            let fields = if fields.is_empty() {
                "its definition".to_owned()
            } else {
                fields
            };
            format!(
                "MCP server '{server_name}' changed the {fields} of tool '{tool}' since you \
                 approved it. Review the tool before letting an agent use it."
            )
        }
    }
}

/// What a tool says about itself right now, pretty-printed.
///
/// This is the "show the diff" half of the warning, and it shows only one side
/// of it: the store keeps hashes, so the previous text is gone. That is the
/// side that matters — the question a person has to answer is not *what did it
/// used to say* but *am I willing to run this*, and the answer is in front of
/// them. Written to the server's own MCP log rather than a toast, because a
/// JSON schema is not a notification.
pub fn current_definition(tools: &[Tool], tool_name: &str) -> Option<String> {
    let tool = tools.iter().find(|tool| tool.name == tool_name)?;
    serde_json::to_string_pretty(tool).ok()
}

/// Hash one optional field, distinguishing absent from empty.
fn field_digest(label: &str, bytes: Option<&[u8]>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(DOMAIN);
    hasher.update((label.len() as u64).to_le_bytes());
    hasher.update(label.as_bytes());
    match bytes {
        // A tool that stops having a description is not the same as one whose
        // description became the empty string, and both are worth noticing.
        None => hasher.update([0u8]),
        Some(bytes) => {
            hasher.update([1u8]);
            hasher.update((bytes.len() as u64).to_le_bytes());
            hasher.update(bytes);
        }
    }
    hex::encode(hasher.finalize())
}

/// Hash one optional JSON field, canonically.
fn json_field_digest(label: &str, value: Option<&Value>) -> String {
    let canonical = value.map(|value| canonical_json(value));
    field_digest(label, canonical.as_deref().map(str::as_bytes))
}

/// Serialize with object keys in sorted order, at every depth.
///
/// Insurance rather than a fix: this workspace builds `serde_json` without
/// `preserve_order` (checked 2026-08-23 — nothing pulls `indexmap` in for it),
/// so `Map` is a `BTreeMap` and already sorts. But that is a *transitive
/// feature of somebody else's dependency tree*, and if any crate in the graph
/// ever turns it on, every stored digest silently stops matching and every
/// server reads as a rug-pull. Sorting here costs nothing and makes the digest
/// depend only on what the tool claims to be.
fn canonical_json(value: &Value) -> String {
    fn canonicalize(value: &Value) -> Value {
        match value {
            Value::Object(map) => {
                let sorted: BTreeMap<&String, &Value> = map.iter().collect();
                Value::Object(
                    sorted
                        .into_iter()
                        .map(|(key, value)| (key.clone(), canonicalize(value)))
                        .collect(),
                )
            }
            Value::Array(items) => Value::Array(items.iter().map(canonicalize).collect()),
            other => other.clone(),
        }
    }

    serde_json::to_string(&canonicalize(value)).unwrap_or_default()
}

#[cfg(test)]
#[path = "tool_digest_tests.rs"]
mod tests;
