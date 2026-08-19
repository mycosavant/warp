//! Local-first Warp Drive on disk: a materialized mirror of the SQLite store
//! that a user can keep in their own git repository.
//!
//! T4.2 made SQLite authoritative for Warp Drive and dropped the server sync.
//! This is the replacement for that sync, and the shape of it is deliberate:
//!
//! * **The user drives git.** Warp reads and writes a directory; it never
//!   commits, pulls or merges. A conflict is then a text conflict in a repo the
//!   user already understands, resolved with the tools they already have —
//!   rather than a three-way merge over a graph of objects with identities,
//!   which is a sync engine and is exactly what this fork exists to remove.
//! * **SQLite stays authoritative; the tree is a mirror.** Two sources of truth
//!   would need reconciliation. One source plus a projection needs only a rule
//!   for which side wins, and the rule is: the store wins on export, the tree
//!   wins on import, and nothing happens implicitly.
//! * **This extends Warp Drive rather than `WorkflowSource::Project`.** The
//!   project path is already git-native and would have been a much smaller
//!   change, but it is a *parallel* store — workflows would live in two places.
//!   That split is a confusion upstream already has, and the fork should not
//!   double down on it.
//!
//! # What happens when git leaves a conflict behind (T4.4e)
//!
//! The first decision above settles who resolves a conflict — the user, in
//! their repository, with `git mergetool`. It does not settle what Warp does
//! when it *meets* one, and until T4.4e that was a side effect rather than a
//! choice: a file with markers in it failed to parse, landed in
//! [`tree::ImportSummary::ignored`] alongside the user's README, and the object
//! it described was therefore absent from the tree. Absence is how [`apply`]
//! is told an object was deleted. So the objects in the middle of being merged
//! — the only ones the user was demonstrably working on — were the ones that
//! got trashed.
//!
//! The policy is now three rules, and the first is the one the other two serve:
//!
//! * **Warp never resolves a conflict, and never guesses.** Both sides are
//!   reconstructed, but only to answer "is this file one of mine?". Choosing a
//!   side is the merge behaviour decision 1 rejected, and it would happen
//!   silently.
//! * **Both directions refuse, whole.** An import stops rather than skipping the
//!   conflicted files, because skipping is what turns a merge into a deletion;
//!   an export stops rather than overwriting them, because the half-merged file
//!   is the only copy of the merge in front of the user.
//! * **Only our files count.** Ours-ness is decided by parsing each side, not by
//!   spotting a marker — the mirror shares a repository with the user's own
//!   work, and their conflicted README is not ours to have an opinion about.
//!
//! # Things that are not drive objects (T4.4g)
//!
//! A workflow alias is the first of these, and it needed a rule of its own.
//! `WorkflowAliases` is a settings group, not a cloud object, so it has no
//! identity, no folder and no file — and upstream syncs it through cloud
//! settings sync, which this fork removed. So a workflow arrived on another
//! machine having lost the one thing the user typed to reach it.
//!
//! It is carried **inside the workflow's own file** rather than in a list of
//! its own, so that it moves when the workflow moves and dies when it dies.
//! See [`format::Alias`] for why the side-car alternative loses.
//!
//! The import rule is deliberately unlike the object rule. Objects read absence
//! as deletion, which works because a deleted object still exports as a trashed
//! one — absence therefore means something. An alias has no such tombstone, so
//! absence is only read *within the workflows the tree describes*: an alias
//! pointing anywhere else is left completely alone.
//!
//! See `.fork/TASKS.md` T4.4 for the full scope.

pub mod apply;
pub mod format;
pub mod snapshot;
pub mod tree;
