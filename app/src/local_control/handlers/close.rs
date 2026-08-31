//! Close handlers for local-control window, tab, and pane actions.
use ::local_control::protocol::{TabCloseMode, TabCloseParams, TabTarget};
use ::local_control::{Action, ActionKind, ControlError, ErrorCode, InstanceId, RequestEnvelope};
use warpui::ModelContext;
use warpui::platform::TerminationMode;

use crate::local_control::LocalControlBridge;
use crate::local_control::handlers::ack;
use crate::local_control::resolver::{
    reject_target_families, tab_index_from_target, target_pane_group, target_pane_id,
    target_window_id_for_target, target_workspace,
};
use crate::workspace::view::OpenDialogSource;

fn tab_close_mode(action: &Action) -> Result<TabCloseMode, ControlError> {
    Ok(action.params_as::<TabCloseParams>()?.mode)
}

fn validate_empty_params(action: &Action) -> Result<(), ControlError> {
    if action
        .params
        .as_object()
        .is_some_and(serde_json::Map::is_empty)
    {
        return Ok(());
    }
    Err(ControlError::new(
        ErrorCode::InvalidParams,
        format!("{} does not accept parameters", action.kind.as_str()),
    ))
}

/// What `window.close` answers, and why it is not a bare `ok`.
///
/// **`ok: true` here has always meant "the request was dispatched", never "the
/// window closed", and nothing in the payload said so.** The close is sent with
/// [`TerminationMode::Cancellable`] — the mode whose own doc says *"the
/// termination can be interrupted"* — and this handler returns the instant it
/// has asked, without observing the outcome. So a caller that reads `ok: true`
/// as "Warp exited" is reading a claim this process never made and cannot make.
///
/// **This is the mistake `approvals.rs` already refuses to make**, one action
/// over: it reports the keystroke it sent rather than `approved: true`, on the
/// stated grounds that *"a result claiming `approved: true` would assert an
/// effect this process cannot observe"*. Same situation, same answer.
///
/// **It has a measured cost, which is why this is worth a field.** With a CLI
/// agent alive in a pane, `window close` answered `ok: true` and the process
/// stayed up; three instances accumulated in one session that way, and stale
/// instances make every later `warpctrl` call answer `ambiguous_instance`. A
/// check that greps only for `"ok"` sails straight past it. A wedged ACP turn
/// does the same, reproduced twice, once after waiting 43 seconds.
///
/// **What is deliberately not claimed here: why.** The mechanism that
/// interrupts a cancellable termination has not been established by running it,
/// and one candidate was ruled out by reading — `CloseSessionConfirmationDialog`
/// covers pane and tab closes (`OpenDialogSource` has no window arm) and so is
/// not it. Naming a cause on this evidence would be inventing one, which is
/// exactly what `unconfined_reason` was corrected for in T14.8. The field says
/// the close may be refused and that the caller must look; it does not guess
/// what would refuse.
fn close_requested(instance_id: &Option<InstanceId>) -> serde_json::Value {
    let mut response = ack(instance_id, ActionKind::WindowClose);
    if let Some(object) = response.as_object_mut() {
        // Additive: `ok` keeps its existing meaning for every caller that
        // already reads it, and the qualifier sits beside it rather than
        // changing it out from under them.
        object.insert("close".to_owned(), serde_json::json!("requested"));
        object.insert("cancellable".to_owned(), serde_json::json!(true));
        object.insert(
            "verify".to_owned(),
            serde_json::json!(
                "a cancellable close can be refused and this result does not observe the \
                 outcome; poll `instance list` until this instance is gone. If it stays, end \
                 any CLI agent running in a pane and cancel any in-flight agent turn, then \
                 close again."
            ),
        );
    }
    response
}

pub(crate) fn window_close(
    instance_id: &Option<InstanceId>,
    request: &RequestEnvelope,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    validate_empty_params(&request.action)?;
    reject_target_families(
        ActionKind::WindowClose,
        request.target.tab.is_some()
            || request.target.pane.is_some()
            || request.target.session.is_some(),
        "tab, pane, or session selectors",
    )?;
    let window_id = target_window_id_for_target(ctx, &request.target, ActionKind::WindowClose)?;
    ctx.windows()
        .close_window(window_id, TerminationMode::Cancellable);
    Ok(close_requested(instance_id))
}

pub(crate) fn tab_close(
    instance_id: &Option<InstanceId>,
    request: &RequestEnvelope,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    reject_target_families(
        ActionKind::TabClose,
        request.target.pane.is_some() || request.target.session.is_some(),
        "pane or session selectors",
    )?;
    let mode = tab_close_mode(&request.action)?;
    let workspace = target_workspace(ActionKind::TabClose, &request.target, ctx)?;
    let closed = workspace.update(ctx, |workspace, ctx| {
        let selected_index = tab_index_from_target(&request.target, workspace, ctx)?;
        let tab_count = workspace.tab_count();
        let tab_indices: Vec<usize> = match mode {
            TabCloseMode::Target => vec![selected_index],
            TabCloseMode::Active => {
                if !matches!(request.target.tab.as_ref(), None | Some(TabTarget::Active)) {
                    return Err(ControlError::new(
                        ErrorCode::InvalidSelector,
                        "tab.close active does not accept a concrete tab selector",
                    ));
                }
                vec![workspace.active_tab_index()]
            }
            TabCloseMode::Others => (0..tab_count)
                .filter(|index| *index != selected_index)
                .collect(),
            TabCloseMode::RightOf => ((selected_index + 1)..tab_count).collect(),
        };
        if tab_indices.is_empty() {
            return Ok(true);
        }
        let closed = workspace.close_tabs(
            tab_indices.into_iter(),
            OpenDialogSource::CloseTab {
                tab_index: selected_index,
            },
            false,
            true,
            ctx,
        );
        Ok(closed)
    })?;
    if closed {
        return Ok(ack(instance_id, ActionKind::TabClose));
    }
    Err(ControlError::new(
        ErrorCode::TargetStateConflict,
        "tab close was cancelled by an existing app warning",
    ))
}

pub(crate) fn pane_close(
    instance_id: &Option<InstanceId>,
    request: &RequestEnvelope,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    validate_empty_params(&request.action)?;
    reject_target_families(
        ActionKind::PaneClose,
        request.target.session.is_some(),
        "session selectors",
    )?;
    let pane_group = target_pane_group(ActionKind::PaneClose, &request.target, ctx)?;
    let pane_id = target_pane_id(ActionKind::PaneClose, &request.target, &pane_group, ctx)?;
    pane_group.update(ctx, |pane_group, ctx| pane_group.close_pane(pane_id, ctx));
    Ok(ack(instance_id, ActionKind::PaneClose))
}

#[cfg(test)]
#[path = "close_tests.rs"]
mod tests;
