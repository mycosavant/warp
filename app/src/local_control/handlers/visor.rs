//! `window.visor.toggle` and `window.visor.status` — the dedicated hotkey
//! window, and whether it opens as an agent (`.fork/TASKS.md` T8.1,
//! `IDEAS.md` I8).
//!
//! # Why the control plane owns a hotkey
//!
//! Upstream reaches this window exactly one way: a global keyboard shortcut.
//! That makes it the one feature in the app a headless check cannot exercise,
//! and on this fork's primary Linux target it cannot be exercised at all —
//! synthetic keystrokes reach no X11 client under WSLg, XTEST and XSendEvent
//! both (`.fork/TASKS.md`, "it is not Warp, and X11 is exhausted"). So the
//! visor either gets a second entry point or it ships unverified.
//!
//! The second entry point is also the better one for an agent. A shortcut is
//! for a person's hands; a lead agent that wants a scratch window should ask
//! for one by name.
//!
//! # Toggle does not report state
//!
//! `pane.main.*` answers every call with post-call state, and this
//! deliberately does not. Toggling is a *global action*, and dispatching one
//! from a model context queues an effect that runs after the current update
//! completes — so anything read alongside the dispatch is the state from
//! before it. Reporting that would be worse than reporting nothing. `status`
//! is a separate call for exactly this reason.

use ::local_control::ActionKind;
use ::local_control::protocol::{
    ControlError, ErrorCode, TargetSelector, VisorState, VisorStatusResult,
};
use serde::Serialize;
use warpui::{ModelContext, SingletonEntity};

use crate::global_resource_handles::GlobalResourceHandlesProvider;
use crate::local_control::bridge::LocalControlBridge;
use crate::local_control::handlers::ack;
use crate::local_control::resolver::reject_target_families;
use crate::root_view::{
    WindowState, quake_mode_window_id, quake_mode_window_state, visor_opens_agent,
};
use crate::terminal::keys_settings::KeysSettings;

/// Shows the hotkey window, or hides it if it is already showing.
///
/// Takes no target: there is at most one hotkey window per process and it is
/// not addressable by index, so a `--window` selector could only ever be
/// wrong. Works whether or not the global shortcut is bound or enabled — this
/// dispatches the same action the shortcut does, not the shortcut.
pub(crate) fn toggle(
    instance_id: &Option<::local_control::InstanceId>,
    target: &TargetSelector,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    let action = ActionKind::WindowVisorToggle;
    reject_target_families(
        action,
        target.window.is_some()
            || target.tab.is_some()
            || target.pane.is_some()
            || target.session.is_some(),
        "target selectors",
    )?;

    let global_resource_handles = GlobalResourceHandlesProvider::as_ref(ctx).get().clone();
    ctx.dispatch_global_action(
        "root_view:toggle_quake_mode_window",
        global_resource_handles,
    );
    Ok(ack(instance_id, action))
}

/// Reports whether the hotkey window exists, whether it is showing, and how
/// this build would open it.
pub(crate) fn status(
    target: &TargetSelector,
    ctx: &mut ModelContext<LocalControlBridge>,
) -> Result<serde_json::Value, ControlError> {
    reject_target_families(
        ActionKind::WindowVisorStatus,
        target.window.is_some()
            || target.tab.is_some()
            || target.pane.is_some()
            || target.session.is_some(),
        "target selectors",
    )?;

    let keys_settings = KeysSettings::as_ref(ctx);
    let result = VisorStatusResult {
        state: match quake_mode_window_state() {
            None => VisorState::Absent,
            Some(WindowState::Open) => VisorState::Open,
            Some(WindowState::PendingOpen) => VisorState::PendingOpen,
            Some(WindowState::Hidden) => VisorState::Hidden,
        },
        // Same formatting `window list` uses, so the two answers join.
        window_id: quake_mode_window_id().map(|window_id| window_id.to_string()),
        opens_agent: visor_opens_agent(ctx),
        hotkey_enabled: *keys_settings.quake_mode_enabled,
        // `normalized`, not `displayed`: the caller of a control-plane action
        // wants the string that goes in `settings.toml`, not the one with the
        // platform's modifier glyphs in it.
        hotkey: keys_settings
            .quake_mode_settings
            .keybinding
            .as_ref()
            .map(|keystroke| keystroke.normalized()),
    };

    to_control_data(result)
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
#[path = "visor_tests.rs"]
mod tests;
