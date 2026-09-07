//! How the agent's model list reaches the panel's picker (T14.14, the second
//! half, 2026-09-07).
//!
//! The list is captured on the connection's own task, off the app thread, and
//! the picker reads `ModelsByFeature::agent_mode` through `UserWorkspaces` on
//! it. The hop is the one `local_control` already makes for every control
//! request: a [`ModelSpawner`] handed out by the model that owns the write,
//! here `LLMPreferences`, whose `update_feature_model_choices` is the same
//! path a server-fetched list takes, so the picker, the chip and the
//! "models unavailable" flag all learn of it the way they already know how.
//!
//! Installed once at startup from `lib.rs`, beside the model it spawns into.
//! Before that, or in a process that never registered `LLMPreferences`,
//! [`publish`] does nothing and says so; a turn does not fail because the
//! picker cannot be told.
//!
//! # The list is kept on disk, because upstream keeps it nowhere
//!
//! Measured 2026-09-07 on the first relaunch after the picker worked: the
//! chip read `auto (cost-efficient)` again, the first turn of a new
//! conversation ran on the agent's default, and the chip flipped to the
//! person's pick only once that turn's `session/new` had published the list
//! -- by which time the turn was already running on the wrong model. The
//! teamless list lives in `UserWorkspaces::workspaceless_models_by_feature`,
//! an in-memory `Option` that starts `None`, while the pick itself is in the
//! execution profile and did survive. So the list is written under
//! `fork::state_dir()` whenever it changes and read back at [`install`],
//! before any turn, and the pick resolves against it from the first turn.
//! Only the agent's advertised names and ids are in the file; nothing about
//! the person.

use std::sync::OnceLock;

use warpui::{ModelContext, ModelSpawner, SingletonEntity};

use crate::ai::llms::{AvailableLLMs, LLMPreferences};
use crate::workspaces::user_workspaces::UserWorkspaces;

static SPAWNER: OnceLock<ModelSpawner<LLMPreferences>> = OnceLock::new();

/// Where the last published list is kept between launches.
fn store_path() -> std::path::PathBuf {
    crate::fork::state_dir().join("acp-models.json")
}

/// Keep the way onto the app thread, and put the last launch's list back
/// before any turn runs. A second call is ignored: the first spawner is as
/// good as any.
pub(crate) fn install(
    spawner: ModelSpawner<LLMPreferences>,
    preferences: &mut LLMPreferences,
    ctx: &mut ModelContext<LLMPreferences>,
) {
    let _ = SPAWNER.set(spawner);
    if let Some(agent_mode) = read_store(&store_path()) {
        apply(agent_mode, preferences, ctx);
    }
}

/// The list as last written, or nothing: a missing, unreadable or malformed
/// file is a fresh launch, not an error anyone can act on.
fn read_store(path: &std::path::Path) -> Option<AvailableLLMs> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Best-effort and silent: the file is a convenience for the next launch,
/// and the list is already in force in this one.
fn write_store(path: &std::path::Path, agent_mode: &AvailableLLMs) {
    let Ok(text) = serde_json::to_string_pretty(agent_mode) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = crate::fork::create_private_dir(dir);
    }
    if let Ok(mut file) = crate::fork::create_private_file(path, false) {
        use std::io::Write as _;
        let _ = file.write_all(text.as_bytes());
    }
}

/// Replace the panel's agent-mode list, on the app thread, and say whether
/// it changed.
fn apply(
    agent_mode: AvailableLLMs,
    preferences: &mut LLMPreferences,
    ctx: &mut ModelContext<LLMPreferences>,
) -> bool {
    let mut models = UserWorkspaces::as_ref(ctx)
        .feature_model_choice_for_team_uid(None)
        .clone();
    if models.agent_mode == agent_mode {
        return false;
    }
    models.agent_mode = agent_mode;
    preferences.update_feature_model_choices(Ok(models), ctx);
    true
}

/// Replace the panel's agent-mode model list with the agent's own, on the app
/// thread, and say whether the list changed.
///
/// Unchanged lists are not written: `on_server_update` announces every write
/// as new models, and a per-turn write would announce the same list every
/// turn. The teamless scope, because the fork has no account and so no team;
/// what a team-scoped picker would read is upstream's concern.
pub(crate) async fn publish(agent_mode: AvailableLLMs) -> bool {
    let Some(spawner) = SPAWNER.get() else {
        return false;
    };
    spawner
        .spawn(move |preferences, ctx| {
            let changed = apply(agent_mode.clone(), preferences, ctx);
            if changed {
                write_store(&store_path(), &agent_mode);
            }
            changed
        })
        .await
        .unwrap_or(false)
}

#[cfg(test)]
#[path = "picker_tests.rs"]
mod tests;
