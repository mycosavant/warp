//! `pane.main.get`, `pane.main.set` and `pane.main.clear` — designating the
//! pane a tab's ambient surfaces follow (`.fork/TASKS.md` T8.5, `IDEAS.md`
//! I13 + I6).
//!
//! # What "main" means today
//!
//! Exactly one thing: the working directory that the file tree and code review
//! resolve to. Warp already picks a repository per tab, and it picks it from
//! whichever pane is active — which means glancing at a split moves the file
//! tree out from under you. A designated pane is stable by construction.
//!
//! The designation is deliberately more general than its one consumer, because
//! the same answer is wanted by two other questions already on the board: which
//! pane a layout should anchor on, and which agent is the lead. Those read the
//! same `Option<PaneId>` when they arrive; nothing about this needs revisiting
//! for them.
//!
//! # Why three actions rather than a toggle
//!
//! `PaneGroupAction::ToggleMainPane` — what the command palette entry
//! dispatches — is the right shape for a keystroke and the wrong shape for a
//! script: a caller that wants "make this the main pane" cannot express it
//! without first reading the current state, and loses a race if it changes in
//! between. `set` and `clear` are idempotent; `get` exists so the effect is
//! observable without a screenshot.
//!
//! All three answer with the state *after* the call, so a mutation never needs
//! a follow-up read.

use ::local_control::ActionKind;
use ::local_control::protocol::{ControlError, ErrorCode, MainPaneResult, TargetSelector};
use serde::Serialize;
use warpui::{ModelContext, ViewHandle};

use crate::local_control::bridge::LocalControlBridge;
use crate::local_control::resolver::{active_target_pane_group, target_pane_id};
use crate::pane_group::PaneGroup;

/// Reads the targeted tab's main pane without changing it.
pub(crate) fn get(
    target: &TargetSelector,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    let pane_group = active_target_pane_group(ActionKind::PaneMainGet, target, ctx)?;
    to_control_data(describe(&pane_group, ctx))
}

/// Designates the targeted pane as its tab's main pane.
///
/// With no `--pane` selector this uses the tab's *focused* pane, which is the
/// same default `pane.rename` and friends use, and the one a person means by
/// "this pane".
pub(crate) fn set(
    target: &TargetSelector,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    let action = ActionKind::PaneMainSet;
    let pane_group = active_target_pane_group(action, target, ctx)?;

    let pane_id = if target.pane.is_some() {
        target_pane_id(action, target, &pane_group, ctx)?
    } else {
        // Not `input_target_pane_id`: that one resolves to the active
        // *terminal session*, and a main pane does not have to be a terminal.
        // An editor or code-review pane is a legal designation — it simply
        // stops the ambient surfaces moving rather than anchoring them
        // somewhere new.
        pane_group.read(ctx, |pane_group, ctx| pane_group.focused_pane_id(ctx))
    };

    pane_group.update(ctx, |pane_group, ctx| {
        pane_group.set_main_pane(Some(pane_id), ctx);
    });

    to_control_data(describe(&pane_group, ctx))
}

/// Clears the targeted tab's main pane, restoring follow-the-active-pane.
///
/// Succeeds when there was nothing designated. Clearing is the kind of call a
/// script makes to get back to a known state, and erroring on "already clear"
/// would make that need a guard for no benefit.
pub(crate) fn clear(
    target: &TargetSelector,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    let pane_group = active_target_pane_group(ActionKind::PaneMainClear, target, ctx)?;
    pane_group.update(ctx, |pane_group, ctx| {
        pane_group.set_main_pane(None, ctx);
    });
    to_control_data(describe(&pane_group, ctx))
}

/// Snapshots a group's main-pane state, including its index so a caller can
/// line the answer up with `pane list`.
fn describe(
    pane_group: &ViewHandle<PaneGroup>,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> MainPaneResult {
    pane_group.read(ctx, |pane_group, ctx| {
        let Some(main_pane_id) = pane_group.main_pane() else {
            return MainPaneResult {
                main_pane_id: None,
                main_pane_index: None,
                anchors_working_directory: false,
            };
        };
        MainPaneResult {
            main_pane_id: Some(main_pane_id.to_string()),
            // Same ordering `pane.list` enumerates, so the index means the
            // same thing in both answers.
            main_pane_index: pane_group
                .visible_pane_ids()
                .into_iter()
                .position(|pane_id| pane_id == main_pane_id),
            anchors_working_directory: pane_group
                .terminal_view_from_pane_id(main_pane_id, ctx)
                .is_some(),
        }
    })
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
#[path = "main_pane_tests.rs"]
mod tests;
