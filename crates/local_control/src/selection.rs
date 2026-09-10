//! Instance selection helpers for local-control clients.
use std::path::Path;

use crate::discovery::{InstanceId, InstanceRecord};
use crate::protocol::{ControlError, ErrorCode};

/// CLI-level selector for choosing one discovered Warp instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceSelector {
    Active,
    Id(InstanceId),
    Pid(u32),
}

/// Choose one discovered instance, or say why none was chosen.
///
/// `searched` is the directory `records` came out of, and it exists for exactly
/// one of the messages below. Passed in rather than read from the environment
/// here, so this stays a pure function with tests that run anywhere — the same
/// reason `session::filesystem::classify` takes its three facts.
pub fn select_instance(
    records: &[InstanceRecord],
    selector: &InstanceSelector,
    searched: &Path,
) -> Result<InstanceRecord, ControlError> {
    match selector {
        InstanceSelector::Active => select_active(records, searched),
        InstanceSelector::Id(instance_id) => records
            .iter()
            .find(|record| &record.instance_id == instance_id)
            .cloned()
            .ok_or_else(|| {
                ControlError::new(
                    ErrorCode::NoInstance,
                    format!("no Warp instance with id {}", instance_id.0),
                )
            }),
        InstanceSelector::Pid(pid) => records
            .iter()
            .find(|record| record.pid == *pid)
            .cloned()
            .ok_or_else(|| {
                ControlError::new(
                    ErrorCode::NoInstance,
                    format!("no Warp instance with pid {pid}"),
                )
            }),
    }
}

/// **The empty case names the directory it looked in, and that is the whole
/// point of this change** (2026-09-10, T15).
///
/// [`crate::discovery::discovery_dir`] resolves to `$XDG_RUNTIME_DIR/warp/…`
/// when that variable is set and to `$HOME/.warp/…` when it is not, so one user
/// on one machine has two registries and which one a process uses is decided by
/// the environment it was launched from. A GUI Warp started without the
/// variable registers in the second; a `warpctrl` run from a shell that has it
/// reads the first, finds nothing, and said so — *"no local Warp control
/// instances were discovered"* — while a Warp was running.
///
/// Measured the same day: a record and broker socket from 2026-08-28 were still
/// in `$HOME/.warp/local-control` with a dead pid, thirteen days later, and a
/// single scan pointed at that directory pruned both immediately. **The pruner
/// was never broken. It was never pointed there.** `.fork/runs/discovery-2026-09-10/`.
///
/// So this message is not decoration: the directory is the one fact that
/// separates "nothing is running" from "something is running somewhere you did
/// not look", and a person cannot tell those apart without it.
fn select_active(
    records: &[InstanceRecord],
    searched: &Path,
) -> Result<InstanceRecord, ControlError> {
    match records {
        [] => Err(ControlError::new(
            ErrorCode::NoInstance,
            format!(
                "no local Warp control instances were discovered in {}. A Warp launched \
                 without XDG_RUNTIME_DIR set registers under $HOME/.warp/local-control \
                 instead, and is invisible from a shell that has it — check there before \
                 concluding nothing is running.",
                searched.display()
            ),
        )),
        [record] => Ok(record.clone()),
        _ => Err(ControlError::new(
            ErrorCode::AmbiguousInstance,
            "multiple local Warp control instances were discovered; pass --instance",
        )),
    }
}

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;
