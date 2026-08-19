//! T4.4b — the working tree: a whole Warp Drive as a directory of files.
//!
//! Folders become directories, so the hierarchy is the hierarchy and moving an
//! object is a rename that git reports as one. Everything here works on
//! [`PortableObject`] values and the filesystem; nothing in this file knows
//! about `AppContext`, SQLite or the in-memory model, which is what makes the
//! round trip in `tree_tests.rs` a real test rather than a mock.
//!
//! # Writing into a directory the user owns
//!
//! The export target is a git repository the user also keeps other things in —
//! a README, their own notes, `.git` itself. So the pruning rule is deliberately
//! timid: a file is removed only if it *parses as an object this exporter
//! wrote*, and a directory is removed only once it is empty. Anything
//! unrecognised is left exactly where it is and reported, never deleted.
//!
//! Files whose contents are already correct are not rewritten at all. That
//! keeps mtimes stable, which matters because Warp's own config watcher and
//! every build tool in the repository are watching them.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use super::format::{self, Conflict, FOLDER_FILE_NAME, Payload, PortableObject};
use crate::server::ids::SyncId;

/// An object together with the folder it sits in.
///
/// Placement is kept beside the object rather than inside it because on disk it
/// *is* the path — see the module docs on `format`.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedObject {
    pub object: PortableObject,
    /// The containing folder, or `None` at the top level of the drive.
    pub parent: Option<SyncId>,
}

/// What an export actually did. Reported rather than logged so a caller can
/// show it and a test can assert on it.
#[derive(Debug, Default, PartialEq)]
pub struct ExportSummary {
    pub written: usize,
    /// Files whose contents were already correct. In a healthy repository this
    /// is every file, every time, and `git status` is clean.
    pub unchanged: usize,
    pub removed_files: usize,
    pub removed_directories: usize,
    /// Objects whose parent folder was not in the set. Placed at the top level
    /// rather than dropped, and named here so the reparenting is visible.
    pub orphaned: Vec<SyncId>,
}

/// What an import found, including what it refused.
#[derive(Debug, Default)]
pub struct ImportSummary {
    pub objects: Vec<PlacedObject>,
    /// Paths that are not Warp Drive files — the repository's README, a stray
    /// note, a file from a newer format version — with the reason.
    pub ignored: Vec<(PathBuf, String)>,
    /// The same object id found in more than one file, which a copy-paste or a
    /// bungled merge produces. The first by path wins; the rest are listed.
    pub duplicates: Vec<(PathBuf, SyncId)>,
    /// Warp Drive files with an unresolved merge in them.
    ///
    /// Kept apart from `ignored` because the two mean opposite things to a
    /// caller. An ignored file is somebody else's and always will be; a
    /// conflicted one is *ours*, and the object it describes is missing from
    /// `objects` only because the merge has not been finished yet. Treating the
    /// second as the first is how a half-finished merge turns into a deletion.
    pub conflicted: Vec<Conflicted>,
}

/// One of our files, mid-merge.
#[derive(Debug, Clone, PartialEq)]
pub struct Conflicted {
    pub path: PathBuf,
    /// 1-based line of the opening `<<<<<<<`.
    pub line: usize,
    /// The object's name, taken from whichever side still parses.
    ///
    /// Worth carrying: "resolve `deploy-a1b2c3d4.json`" is a chore handed to
    /// someone who has to work out what it is first, and "your `deploy`
    /// workflow has an unresolved conflict" is a sentence they can act on.
    pub name: String,
}

/// The one thing that stops an export outright.
///
/// A type rather than a message, so a caller can tell "your working tree is
/// mid-merge" — which is the caller's to fix and takes ten seconds — apart from
/// "the disk is full", which is neither.
#[derive(Debug)]
pub struct ConflictsInTheWay(pub Vec<Conflicted>);

impl std::fmt::Display for ConflictsInTheWay {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "refusing to overwrite {} file(s) with unresolved merge conflicts. \
             Finish the merge in git, then export again",
            self.0.len()
        )
    }
}

impl std::error::Error for ConflictsInTheWay {}

/// Writes the whole drive into `root`, creating it if needed.
pub fn export(root: &Path, objects: &[PlacedObject]) -> Result<ExportSummary> {
    std::fs::create_dir_all(root)
        .with_context(|| format!("creating the drive directory {}", root.display()))?;

    let mut summary = ExportSummary::default();
    let layout = Layout::build(root, objects, &mut summary);
    let mut written = HashSet::new();

    // Every file this export would overwrite, read before a single one is
    // written. The read is not extra work — the loop below needs it anyway to
    // decide whether the file is already correct — but doing it up front is
    // what makes the refusal below all-or-nothing rather than half a drive.
    let mut planned = Vec::with_capacity(objects.len());
    let mut conflicted = Vec::new();
    for placed in objects {
        let path = layout.path_of(placed);
        let existing = std::fs::read_to_string(&path).ok();
        if let Some(conflict) = existing.as_deref().and_then(format::conflict) {
            conflicted.push(Conflicted {
                path: path.clone(),
                line: conflict.line,
                // Taken from the object rather than parsed back out of the
                // file: we are the ones about to write it, so we know.
                name: placed.object.name.clone(),
            });
        }
        planned.push((path, existing, placed));
    }

    // Only the paths this export owns are checked, never the whole tree: the
    // user's own conflicted README shares the repository and is none of our
    // business. Within those paths the refusal is absolute, because writing
    // over a half-merged file destroys the one copy of the merge that is in
    // front of the user, and they would have no reason to expect it.
    if !conflicted.is_empty() {
        return Err(anyhow::Error::new(ConflictsInTheWay(conflicted)));
    }

    for (path, existing, placed) in planned {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }

        let contents = placed
            .object
            .to_file_contents()
            .with_context(|| format!("serializing {}", placed.object.name))?;

        if existing.is_some_and(|existing| existing == contents) {
            summary.unchanged += 1;
        } else {
            std::fs::write(&path, &contents)
                .with_context(|| format!("writing {}", path.display()))?;
            summary.written += 1;
        }

        written.insert(path);
    }

    prune(root, &written, &mut summary)?;
    Ok(summary)
}

/// Reads a drive back out of `root`.
pub fn import(root: &Path) -> Result<ImportSummary> {
    let mut summary = ImportSummary::default();
    if !root.exists() {
        bail!("{} does not exist", root.display());
    }

    let mut seen = HashSet::new();
    read_directory(root, None, &mut summary, &mut seen)?;
    Ok(summary)
}

fn read_directory(
    directory: &Path,
    parent: Option<SyncId>,
    summary: &mut ImportSummary,
    seen: &mut HashSet<String>,
) -> Result<()> {
    for entry in sorted_entries(directory)? {
        if entry.is_dir() {
            // `.git` above all, but every dot-directory is machinery belonging
            // to something else.
            if file_name(&entry).starts_with('.') {
                continue;
            }

            let marker = entry.join(FOLDER_FILE_NAME);
            let folder_id = match read_object(&marker, summary, seen)? {
                Some(folder) => {
                    let id = folder.id;
                    summary.objects.push(PlacedObject {
                        object: folder,
                        parent,
                    });
                    Some(id)
                }
                // A directory the user made themselves. Descend anyway — the
                // files inside are still theirs — but do not invent a folder.
                None => parent,
            };

            read_directory(&entry, folder_id, summary, seen)?;
        } else if file_name(&entry) != FOLDER_FILE_NAME
            && let Some(object) = read_object(&entry, summary, seen)?
        {
            summary.objects.push(PlacedObject { object, parent });
        }
    }

    Ok(())
}

/// Parses one file, or explains in the summary why it was left alone.
fn read_object(
    path: &Path,
    summary: &mut ImportSummary,
    seen: &mut HashSet<String>,
) -> Result<Option<PortableObject>> {
    if !path.is_file() {
        return Ok(None);
    }

    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        // Binary, or something we have no business reading.
        Err(err) => {
            summary.ignored.push((path.to_owned(), err.to_string()));
            return Ok(None);
        }
    };

    if let Some(conflict) = format::conflict(&contents) {
        match conflicted_name(&conflict) {
            Some(name) => summary.conflicted.push(Conflicted {
                path: path.to_owned(),
                line: conflict.line,
                name,
            }),
            // Neither side is ours, so this is the user's own file in the
            // middle of their own merge. Not our business, and not our problem
            // to make them solve before they can use their drive.
            None => summary.ignored.push((
                path.to_owned(),
                format!(
                    "unresolved merge conflict at line {}, and neither side is a Warp Drive file",
                    conflict.line
                ),
            )),
        }
        return Ok(None);
    }

    match PortableObject::from_file_contents(&contents) {
        Ok(object) => {
            if !seen.insert(object.id.to_string()) {
                summary.duplicates.push((path.to_owned(), object.id));
                return Ok(None);
            }
            Ok(Some(object))
        }
        Err(err) => {
            summary.ignored.push((path.to_owned(), format!("{err:#}")));
            Ok(None)
        }
    }
}

/// Removes objects that are no longer in the drive, and nothing else.
///
/// "Nothing else" is the whole point: a file is deleted only after it has been
/// read and recognised as one of ours, and a directory only once it is empty.
fn prune(directory: &Path, written: &HashSet<PathBuf>, summary: &mut ExportSummary) -> Result<()> {
    for entry in sorted_entries(directory)? {
        if entry.is_dir() {
            if file_name(&entry).starts_with('.') {
                continue;
            }

            prune(&entry, written, summary)?;

            if std::fs::read_dir(&entry)?.next().is_none() && std::fs::remove_dir(&entry).is_ok() {
                summary.removed_directories += 1;
            }
        } else if !written.contains(&entry) && is_one_of_ours(&entry) {
            std::fs::remove_file(&entry)
                .with_context(|| format!("removing {}", entry.display()))?;
            summary.removed_files += 1;
        }
    }

    Ok(())
}

/// Decides whether a conflicted file is one of ours, by asking whether either
/// side of it parses — and takes the name while it is there.
///
/// Both sides are tried because a merge that touched the header leaves one side
/// unreadable, and one readable side is enough to know whose file this is.
fn conflicted_name(conflict: &Conflict) -> Option<String> {
    PortableObject::from_file_contents(&conflict.ours)
        .or_else(|_| PortableObject::from_file_contents(&conflict.theirs))
        .map(|object| object.name)
        .ok()
}

fn is_one_of_ours(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .is_ok_and(|contents| PortableObject::from_file_contents(&contents).is_ok())
}

/// Resolves every object to a path, folder chains included.
struct Layout {
    root: PathBuf,
    directories: HashMap<String, PathBuf>,
}

impl Layout {
    fn build(root: &Path, objects: &[PlacedObject], summary: &mut ExportSummary) -> Self {
        let folders: HashMap<String, &PlacedObject> = objects
            .iter()
            .filter(|placed| matches!(placed.object.payload, Payload::Folder { .. }))
            .map(|placed| (placed.object.id.to_string(), placed))
            .collect();

        let mut layout = Self {
            root: root.to_owned(),
            directories: HashMap::new(),
        };

        // Resolved in id order rather than hash order. It only matters when the
        // folder graph has a cycle, but then it decides which folder gets
        // called the orphan — and a summary that varies between two runs over
        // the same store is a summary nobody can act on.
        let mut ids: Vec<&String> = folders.keys().collect();
        ids.sort();
        for id in ids {
            let placed = folders[id];
            layout.resolve_folder(placed, &folders, &mut Vec::new(), summary);
        }

        layout
    }

    /// Walks a folder's ancestors, memoising as it goes.
    ///
    /// `chain` is not defensive clutter: `folder_id` is a plain string column
    /// with no referential integrity behind it, and upstream's own
    /// `is_trashed_internal` carries the same cycle guard for the same reason.
    /// A cycle here would recurse until the stack ran out.
    fn resolve_folder(
        &mut self,
        placed: &PlacedObject,
        folders: &HashMap<String, &PlacedObject>,
        chain: &mut Vec<String>,
        summary: &mut ExportSummary,
    ) -> PathBuf {
        let id = placed.object.id.to_string();
        if let Some(path) = self.directories.get(&id) {
            return path.clone();
        }

        let name = placed
            .object
            .folder_directory_name()
            .expect("only folders are resolved here");

        let parent_path = match placed.parent.map(|parent| parent.to_string()) {
            Some(parent_id) if chain.contains(&parent_id) => {
                summary.orphaned.push(placed.object.id);
                self.root.clone()
            }
            Some(parent_id) => match folders.get(&parent_id) {
                Some(parent) => {
                    chain.push(id.clone());
                    let path = self.resolve_folder(parent, folders, chain, summary);
                    chain.pop();
                    path
                }
                None => {
                    summary.orphaned.push(placed.object.id);
                    self.root.clone()
                }
            },
            None => self.root.clone(),
        };

        let path = parent_path.join(name);
        self.directories.insert(id, path.clone());
        path
    }

    fn path_of(&self, placed: &PlacedObject) -> PathBuf {
        let directory = match &placed.object.payload {
            Payload::Folder { .. } => self
                .directories
                .get(&placed.object.id.to_string())
                .cloned()
                .unwrap_or_else(|| self.root.clone()),
            _ => placed
                .parent
                .and_then(|parent| self.directories.get(&parent.to_string()).cloned())
                .unwrap_or_else(|| self.root.clone()),
        };

        directory.join(placed.object.file_name())
    }
}

fn sorted_entries(directory: &Path) -> Result<Vec<PathBuf>> {
    let mut entries = std::fs::read_dir(directory)
        .with_context(|| format!("reading {}", directory.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()
        .with_context(|| format!("reading {}", directory.display()))?;
    // Sorted so a walk is reproducible: which of two duplicate files wins
    // should not depend on the filesystem's iteration order.
    entries.sort();
    Ok(entries)
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "tree_tests.rs"]
mod tests;
