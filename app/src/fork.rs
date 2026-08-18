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
use warpui::{AppContext, SingletonEntity};

use crate::auth::{AuthStateProvider, UserUid};
use crate::cloud_object::Owner;
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
    FeatureFlag::WarpControlCli,
];

/// Whether local control (`warpctrl`) should default to enabled.
///
/// Local control is the fork's orchestration surface: it lets an external
/// agent drive windows, tabs, panes, sessions and the input buffer of a
/// running Warp instance. Upstream ships it complete but gated twice — by
/// [`FeatureFlag::WarpControlCli`] (a `DOGFOOD_FLAGS` entry, so off in every
/// public channel) and by `LocalControlSettings`, whose default comes from
/// `default_mode_for_channel` and is `Disabled` off-dogfood.
///
/// [`FORCE_ENABLED`] handles the first gate. This handles the second, and is
/// consulted from `settings::local_control::LocalControlModeSetting::
/// default_value` — it changes only the *default*, so an explicit user choice
/// stored in secure storage still wins. Upstream's `default_mode_for_channel`
/// is deliberately left pure so its per-channel test keeps passing.
///
/// Note this is a local privilege surface, not a remote one: the credential
/// broker authenticates the OS account via a 0600 Unix socket and
/// kernel-reported peer UID. Enabling it grants nothing to anything that
/// isn't already running as this user.
pub fn local_control_default_enabled() -> bool {
    is_active()
}

/// Whether voice input is transcribed on this machine.
///
/// Upstream has one transcription path and it is not local: `server::
/// voice_transcriber::ServerVoiceTranscriber` base64-encodes the recording and
/// POSTs it to `api.warp.dev`. The `ai::voice::transcribe::Provider` enum
/// (`Wispr` | `OpenAI`) selects *Warp's upstream vendor*, not where inference
/// runs, so neither value keeps audio on the machine.
///
/// This is unconditional under fork policy rather than opt-in, for two reasons.
/// It is a privacy fix, not a preference — no toggle should be able to put
/// microphone audio back on the wire. And the server path cannot work here
/// anyway: transcription is an authenticated call, and this fork runs without
/// an account, so upstream's transcriber can only fail. Substituting a local
/// one strictly enlarges what works.
///
/// Consumed by `voice::local_transcriber::fork_voice_transcriber`, which is
/// deliberately fail-closed: when it is installed it is the *only* transcriber,
/// and a misconfigured endpoint surfaces as an error rather than falling back
/// to the server.
pub fn local_voice_transcription_enabled() -> bool {
    is_active()
}

/// Whether the four small AI features call a model the user controls.
///
/// "Next Command", "Prompt Suggestions", "Shared Block Title Generation" and
/// "Commit & PR Generation" are each a single-shot `POST` to an `/ai/*` route
/// on `api.warp.dev` — no streaming, no tool use, no session state. Warp's
/// server is a bearer-authenticated proxy in front of a model, nothing more,
/// which is why these four can be re-pointed without touching the agent.
///
/// Fail-closed for the same reason voice transcription is: every one of these
/// requests carries user data off the machine — terminal output and the command
/// that produced it, the working directory and recent shell history, or an
/// entire working-tree diff. [`account_gate_bypassed`] makes the toggles
/// reachable without an account, so without this a fork user could switch one
/// on and quietly resume shipping exactly that payload upstream.
///
/// So under fork policy these four never reach `api.warp.dev`. An unconfigured
/// endpoint surfaces as an error naming the setting to fill in, rather than
/// falling back to the server.
///
/// Consumed by `ai::local_completion`, which resolves the endpoint and key from
/// `ai::api_keys::ApiKeyManager` and the model from `settings::LocalAiSettings`.
pub fn local_ai_completions_enabled() -> bool {
    is_active()
}

/// The owner written into Warp Drive objects created without an account.
///
/// Deliberately a fixed constant rather than a per-install UUID. `UserWorkspaces
/// ::owner_to_space` maps an object to [`Space::Personal`] only when its owner
/// equals the *current* user and otherwise falls through to [`Space::Shared`],
/// so a per-machine identity would make a store that moved machines show up
/// under "Shared with me" — the one place local-first objects must never land,
/// since the whole point of T4.4 is to carry this store between machines.
///
/// It cannot collide with a real account: Warp user ids are Firebase uids.
const LOCAL_DRIVE_UID: &str = "local";

/// Whether Warp Drive is authoritative locally rather than a cache of the server.
///
/// Nothing here severs the sync — logging out already does that, and by exactly
/// one line. `SyncQueue::should_dequeue` starts `false` and is set true in a
/// single place, at the end of a *successful* server fetch, which requires an
/// account. So without one, local writes reach the in-memory model and SQLite
/// and simply accumulate in the queue unsent.
///
/// What this enables is the other half: the same successful fetch is also the
/// only thing that sets `UpdateManager::has_initial_load`, and 24 call sites
/// across 15 files await that condition before doing their work. Logged out
/// they wait forever — Warp Drive spins indefinitely over a store that is fully
/// populated and writable, and `warp mcp list` never returns.
///
/// See `.fork/TASKS.md` T4.1 for the full map.
pub fn local_drive_enabled() -> bool {
    is_active()
}

/// The [`Owner`] for objects created with no account, or `None` under upstream
/// policy.
///
/// Consumed by `workspaces::user_workspaces::UserWorkspaces::personal_drive`,
/// which upstream returns `None` from when unauthenticated. Every create path
/// needs an `Owner` and every call site bails on `None`, so that one function
/// is the difference between a Warp Drive that can be read and edited without
/// an account and one that can also be added to.
pub fn local_drive_owner() -> Option<Owner> {
    local_drive_enabled().then(|| Owner::User {
        user_uid: UserUid::new(LOCAL_DRIVE_UID),
    })
}

/// Whether Warp Drive is running account-free, and so is authoritative locally.
///
/// The auth-dependent half of [`local_drive_enabled`]. Fork policy alone is not
/// enough for the seams that decide whether to talk to the server: a fork user
/// who does sign in should get upstream behaviour back, because their objects
/// now exist somewhere else too.
///
/// Returns `false` when [`AuthStateProvider`] is not registered. That is not a
/// state production reaches — `lib.rs` registers it long before any of this
/// runs — but plenty of unit tests build a narrow set of singletons without it,
/// and falling back to upstream behaviour there leaves those tests measuring
/// exactly what they measured before.
pub fn local_drive_is_authoritative(app: &AppContext) -> bool {
    local_drive_enabled()
        && app.has_singleton_model::<AuthStateProvider>()
        && !AuthStateProvider::as_ref(app).get().is_logged_in()
}

/// Whether an object belongs to the account-free local drive.
///
/// Used to keep locally-owned objects out of the sync queue if an account is
/// ever added later. Without this they would be pushed to the server under a
/// `user_uid` that does not exist there.
pub fn is_local_drive_owner(owner: &Owner) -> bool {
    matches!(owner, Owner::User { user_uid } if user_uid.as_str() == LOCAL_DRIVE_UID)
}

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
    if account_gate_bypassed() {
        false
    } else {
        actual
    }
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

#[cfg(test)]
#[path = "fork_tests.rs"]
mod tests;
