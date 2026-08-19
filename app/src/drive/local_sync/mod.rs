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
//! See `.fork/TASKS.md` T4.4 for the full scope.

pub mod format;
pub mod snapshot;
pub mod tree;
