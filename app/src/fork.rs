//! Fork-local policy: no telemetry, no account requirement, local agent
//! harnesses driven by the user's own CLIs, subscriptions and API keys.
//!
//! Design constraint: this module is the *only* place fork policy is
//! expressed, and it is additive. Upstream call sites are never edited to
//! remove behaviour — instead the narrowest existing seams are steered:
//!
//! * feature flags, via [`FeatureFlag::set_user_preference`], which outranks
//!   server-pushed flag state and is never reset (see `warp_features`
//!   `USER_PREFERENCE_MAP`);
//! * harness availability, via [`forced_local_harnesses`], consumed by
//!   `ai::harness_availability::default_harnesses`.
//!
//! Keeping the policy here means an upstream merge can only conflict in the
//! two one-line call sites, never in the policy itself.

use warp_cli::agent::Harness;

use crate::features::FeatureFlag;

/// Set to `0`, `off` or `false` to run the fork with stock upstream behaviour.
/// Useful for A/B-ing a suspected fork-caused regression against upstream
/// without rebuilding.
const POLICY_ENV_VAR: &str = "WARP_FORK_POLICY";

/// Whether fork policy is active for this process.
pub fn is_active() -> bool {
    !matches!(
        std::env::var(POLICY_ENV_VAR).as_deref(),
        Ok("0") | Ok("off") | Ok("false")
    )
}

/// Telemetry, analytics and crash-reporting flags forced off.
///
/// Note this is defence in depth, not the primary removal: `crash_reporting`
/// and `cocoa_sentry` are Cargo features that gate `dep:sentry` itself, so a
/// build without them contains no Sentry code at all. These entries only
/// matter if such a build is ever produced.
const FORCE_DISABLED: &[FeatureFlag] = &[
    FeatureFlag::CrashReporting,
    FeatureFlag::CocoaSentry,
    FeatureFlag::LogExpensiveFramesInSentry,
    FeatureFlag::RecordAppActiveEvents,
    FeatureFlag::RecordPtyThroughput,
    FeatureFlag::WithSandboxTelemetry,
    FeatureFlag::GlobalAIAnalyticsCollection,
    FeatureFlag::GlobalAIAnalyticsBanner,
    FeatureFlag::SendTelemetryToFile,
    FeatureFlag::AgentModeAnalytics,
];

/// Flags forced on so local, account-free agent operation is reachable.
///
/// `LocalClaudeCodexChildHarnesses` is upstream-gated to the WarpLocal
/// developer build (`LOCAL_FLAGS`); it re-enables the local Codex child
/// harness. The local *Claude* harness is already ungated upstream — see
/// `ai::local_harness_setup::local_harness_product_disabled_message`, which
/// returns `None` for `Harness::Claude`.
const FORCE_ENABLED: &[FeatureFlag] = &[
    FeatureFlag::AgentHarness,
    FeatureFlag::APIKeyManagement,
    FeatureFlag::LocalClaudeCodexChildHarnesses,
    FeatureFlag::SoloUserByok,
    FeatureFlag::SkipFirebaseAnonymousUser,
];

/// Harnesses exposed locally without asking Warp's server for permission.
///
/// Upstream ships `default_harnesses()` containing only `Oz` (Warp's own
/// server-side agent) and then replaces the list from the server. That server
/// call is skipped entirely when logged out (`HarnessAvailabilityModel::
/// refresh` early-returns on `!is_logged_in()`), so without this the local
/// harness picker is empty for an account-free user.
///
/// Each still has to clear `local_harness_setup_state`, which requires the
/// corresponding CLI to actually be installed — a harness listed here but not
/// installed shows "Install <x> to use this local harness", not a broken entry.
///
/// `Gemini` is deliberately omitted: `orchestration::snapshots` filters it out
/// upstream because it hangs on "Spawning agents".
pub fn forced_local_harnesses() -> &'static [Harness] {
    &[Harness::Claude, Harness::Codex, Harness::OpenCode]
}

/// Whether to bypass Warp's "must have an account" gates.
///
/// Upstream disables the entire AI surface for anonymous/logged-out users in
/// exactly three places, each a single condition:
///
/// * `settings::ai::AISettings::is_any_ai_enabled` — the master switch. With
///   this false, no agent, no model picker, no harness selection.
/// * `workspaces::user_workspaces::UserWorkspaces::is_byo_api_key_enabled`
/// * `workspaces::user_workspaces::UserWorkspaces::is_custom_inference_enabled`
///
/// Bypassing them is coherent only because Warp's own settings UI states that
/// **"API keys added here are stored only on this device, not on Warp's
/// servers"** — BYO keys are used for direct client→provider calls, so a
/// logged-out user with their own key needs nothing from Warp's backend.
///
/// This does **not** grant access to anything server-side. Warp's own `Oz`
/// agent still requires a real account, because inference for it happens on
/// Warp's servers. The point is to reach the BYO-key and local-harness paths,
/// which do not.
pub fn account_gate_bypassed() -> bool {
    is_active()
}

/// Wraps an `is_anonymous_or_logged_out()` result for **UI gating only**.
///
/// Settings pages check auth directly rather than going through
/// `AISettings::is_any_ai_enabled`, so overriding the master switch alone
/// leaves pages rendering a "please create an account" banner in place of the
/// controls. Call sites that decide *what to draw* should route through here.
///
/// Deliberately not applied to call sites that decide whether to *talk to the
/// server*; those should keep seeing the real auth state so they fail fast
/// instead of issuing credential-less requests.
pub fn is_anonymous_for_ui(actual: bool) -> bool {
    if account_gate_bypassed() { false } else { actual }
}

/// Applies fork feature-flag policy.
///
/// Must run after upstream's channel flags are applied but before
/// `mark_initialized`, so nothing reads a flag mid-change.
pub fn apply_feature_preferences() {
    if !is_active() {
        return;
    }

    for flag in FORCE_DISABLED {
        flag.set_user_preference(false);
    }
    for flag in FORCE_ENABLED {
        flag.set_user_preference(true);
    }
}
