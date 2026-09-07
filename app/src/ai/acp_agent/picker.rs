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

use std::sync::OnceLock;

use warpui::{ModelSpawner, SingletonEntity};

use crate::ai::llms::{AvailableLLMs, LLMPreferences};
use crate::workspaces::user_workspaces::UserWorkspaces;

static SPAWNER: OnceLock<ModelSpawner<LLMPreferences>> = OnceLock::new();

/// Keep the way onto the app thread. A second call is ignored: the first
/// spawner is as good as any.
pub(crate) fn install(spawner: ModelSpawner<LLMPreferences>) {
    let _ = SPAWNER.set(spawner);
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
            let mut models = UserWorkspaces::as_ref(ctx)
                .feature_model_choice_for_team_uid(None)
                .clone();
            if models.agent_mode == agent_mode {
                return false;
            }
            models.agent_mode = agent_mode;
            preferences.update_feature_model_choices(Ok(models), ctx);
            true
        })
        .await
        .unwrap_or(false)
}
