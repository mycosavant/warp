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

use std::time::Duration;

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
///
/// `SshRemoteServer` is gated differently from the rest, and the difference is
/// the whole reason it is listed here. It lives in `RELEASE_FLAGS`, which
/// `features::enabled_features` only extends when
/// `ChannelState::is_release_bundle()` — and that is `cfg!(feature =
/// "release_bundle")`, a Cargo feature absent from `app/Cargo.toml`'s default
/// list. So the entire remote-development stack is compiled in and switched
/// off in every build you make yourself, including `--release`. Measured
/// 2026-08-22: submitting `ssh localhost` into a pane fired the
/// `PreInteractiveSSHSession` warpify hook and then stopped, because this flag
/// was false.
///
/// Nothing about it needs an account. The daemon's `Initialize` handler stores
/// the bearer token and replies without validating it, the only credential
/// check in the daemon is scoped to remote codebase indexing, and the protocol
/// documents `user_id` as "Empty when not logged in". Verified by completing a
/// credential-free handshake against a daemon this binary spawned.
const FORCE_ENABLED: &[FeatureFlag] = &[
    FeatureFlag::AgentHarness,
    FeatureFlag::APIKeyManagement,
    FeatureFlag::LocalClaudeCodexChildHarnesses,
    FeatureFlag::SoloUserByok,
    FeatureFlag::SkipFirebaseAnonymousUser,
    FeatureFlag::WarpControlCli,
    FeatureFlag::SshRemoteServer,
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

/// Set to `1`, `on` or `true` to answer agent conversations from the local
/// `claude` CLI instead of `api.warp.dev`.
const LOCAL_AGENT_ENV_VAR: &str = "WARP_FORK_LOCAL_AGENT";

/// Whether the agent conversation is answered on this machine (T5).
///
/// **Default off, unlike every other predicate in this module.** The others
/// enlarge what works — an account-free user gets a Drive that can be written
/// to, a microphone that stays on the machine, a harness list that is not
/// empty. This one *substitutes* for something that works, and the substitute
/// is a spike: Claude runs its own tools, so Warp's diff review and command
/// approval do not participate, and only a plain user query is handled at all.
/// Switching it on by fork policy would take working behaviour away from anyone
/// signed in.
///
/// Consumed by `ai::agent::api::generate_multi_agent_output`, which is the
/// entire seam: one async fn, `RequestParams` in and a stream of
/// `ResponseEvent` out. Everything the agent surface does hangs off it, and
/// nothing above it knows whether the events came off an SSE socket or a pipe.
/// See `ai::local_agent` for why that is true and `.fork/TASKS.md` T5 for how
/// it was established.
pub fn local_agent_enabled() -> bool {
    is_active()
        && matches!(
            std::env::var(LOCAL_AGENT_ENV_VAR).as_deref(),
            Ok("1") | Ok("on") | Ok("true")
        )
}

/// Set to a number to change how deep `warpctrl agent spawn` may nest.
const SPAWN_DEPTH_ENV_VAR: &str = "WARP_FORK_AGENT_SPAWN_DEPTH";

/// The default cap on how deep spawned child agents may nest.
///
/// Two, so the shape the fork was asked for fits and one more does not: a lead
/// agent scopes work and delegates it (depth 1), and a delegated agent may
/// hand its result to a reviewer (depth 2). A conversation a person started is
/// depth 0.
const DEFAULT_SPAWN_DEPTH: u32 = 2;

/// How deep spawned child agents may nest (T6.6).
///
/// **The weaker of the two guardrails, and it exists because there are two
/// spawn paths.** A tool allowlist governs what the *model* may reach for, and
/// withholding `SUBAGENT` and `RUN_AGENTS` forbids fan-out at the point the
/// request is built — a harder guarantee than any counter. But `warpctrl` is a
/// second path: a lead agent that can run `agent spawn` can run it in a loop
/// whatever its own tool list says, because it is not using a tool to do it.
/// This is the backstop for the path the allowlist cannot see.
///
/// It bounds depth and not breadth. Ten siblings at depth 1 are within it;
/// that is the tool list's job, and the honest reading of this one is "a
/// runaway cannot recurse", not "a runaway cannot happen".
pub fn agent_spawn_depth_limit() -> u32 {
    spawn_depth_limit_from(std::env::var(SPAWN_DEPTH_ENV_VAR).ok().as_deref())
}

/// Split from the environment so the decision can be asserted without setting
/// a process-global variable from a test that runs beside others.
fn spawn_depth_limit_from(value: Option<&str>) -> u32 {
    // Unparseable falls back to the default rather than to zero: `0` is a
    // meaningful setting — it forbids spawning outright — and reaching it by
    // typo is the kind of failure that gets diagnosed as "spawn is broken".
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_SPAWN_DEPTH)
}

/// Whether the `oz-harness-support` Claude Code plugin may be installed (I17).
///
/// **This is the one plugin in `warpdotdev/claude-code-warp` that does not
/// belong here.** Reviewed 2026-08-23: the sibling `warp` plugin is seven bash
/// hooks with no network calls at all, emitting an OSC 777 sequence to the
/// TTY, and is welcome. `oz-harness-support` is the cloud harness integration
/// — `DEFAULT_SERVER_ROOT = "https://app.warp.dev"`, an `oz-parent-listener`,
/// a mailbox drain, and skills that upload files and report PRs.
///
/// It is refused at the *manager*, not at the caller. Today the only path that
/// asks for it is `ensure_local_claude_child_plugins`, reached when an in-app
/// agent spawns a child running a third-party CLI harness in a terminal — and
/// the fork's own `agent spawn` does not go that way, because it uses
/// `Harness::Oz` through the transport rather than a terminal harness. So the
/// hole is currently closed *by accident of architecture*, which is exactly
/// the kind of thing this file exists to convert into a guarantee. Guarding
/// the manager means a future call site cannot reopen it by not knowing.
///
/// The refusal is silent and returns `Ok(())` rather than an error: nothing
/// downstream depends on the platform plugin existing, and reporting a failure
/// would surface a warning about a thing the user did not ask for.
pub fn cloud_harness_plugin_allowed() -> bool {
    !is_active()
}

/// Whether to pin what an MCP server's tools claim to be, and say so when a
/// definition changes under a name that was already approved (T8.4).
///
/// The defence is against the tool rug-pull: a tool's `description` is prompt,
/// written by a third party, that the user reviews once at install and the
/// model re-reads on every turn. A server is free to rewrite it afterwards, and
/// nothing in the protocol or the client notices. Hashing what was advertised
/// and comparing on the next connect turns that into something a person can be
/// told about.
///
/// On under fork policy and off under `WARP_FORK_POLICY=0`, with no variable of
/// its own: there is no reading of the fork's thesis under which watching your
/// own tool definitions is optional, and a switch would only ever be found by
/// somebody trying to silence a warning they should read.
pub fn mcp_tool_pinning_enabled() -> bool {
    is_active()
}

/// The directory for fork-local state that has no upstream home.
///
/// Deliberately a subdirectory rather than loose files in `state_dir()`: what
/// the fork writes should be identifiable, inspectable and deletable as a unit
/// without a list of filenames.
pub fn state_dir() -> std::path::PathBuf {
    warp_core::paths::secure_state_dir()
        .unwrap_or_else(warp_core::paths::state_dir)
        .join("fork")
}

const FRAME_LOG_ENV_VAR: &str = "WARP_FORK_FRAME_LOG";

/// Two frames' worth of budget at 60Hz. One frame's would report the ordinary
/// jitter of a busy machine, which is noise; this is roughly where a stutter
/// stops being a number and starts being something a person notices.
const DEFAULT_SLOW_FRAME_THRESHOLD: Duration = Duration::from_millis(33);

/// How slow a frame has to be before it is worth a line in the local log, or
/// `None` to leave frames untimed (T8.2).
///
/// **This exists because of a gate this file closed.** Upstream's only
/// frame-cost instrumentation is [`FeatureFlag::LogExpensiveFramesInSentry`],
/// which [`FORCE_DISABLED`] switches off with the rest of the telemetry flags.
/// Correct — it reports to Sentry — but the consequence was that the fork could
/// not put a number on its own rendering, which stayed invisible until somebody
/// reported a drag as laggy and there was nothing to measure it with. The
/// replacement follows the same pattern as `LocalTranscriber` and the local
/// OTLP export: keep the capability, drop the network path.
///
/// Off by default, because an always-on measurement of a thing that is usually
/// fine is a cost with no reader. `WARP_FORK_FRAME_LOG=on` takes the default
/// threshold, a bare number is a threshold in milliseconds, and `0` / `off` is
/// the same answer as unset. Consumed by `warpui::frame_log`, which owns the
/// accounting and holds no policy.
pub fn slow_frame_threshold() -> Option<Duration> {
    if !is_active() {
        return None;
    }
    slow_frame_threshold_from(std::env::var(FRAME_LOG_ENV_VAR).ok().as_deref())
}

/// Split from the environment so the decision can be asserted without setting a
/// process-global variable from a test that runs beside others.
fn slow_frame_threshold_from(value: Option<&str>) -> Option<Duration> {
    match value.map(str::trim) {
        None | Some("") | Some("0") | Some("off") | Some("false") => None,
        Some("on") | Some("true") => Some(DEFAULT_SLOW_FRAME_THRESHOLD),
        // An unparseable value takes the default rather than switching off: the
        // variable being present at all is a request to measure, and answering
        // a typo with silence looks exactly like "the feature does not work".
        Some(value) => Some(
            value
                .parse()
                .map(Duration::from_millis)
                .unwrap_or(DEFAULT_SLOW_FRAME_THRESHOLD),
        ),
    }
}

/// Whether a tab can be dragged into the pane area to split a pane (T8.2).
///
/// **The gate was an axis.** Upstream wraps each tab in a `Draggable` pinned to
/// `DragAxis::HorizontalOnly` unless [`FeatureFlag::DragTabsToWindows`] is on —
/// and that flag lives in `RELEASE_FLAGS` under
/// `cfg!(any(target_os = "macos", target_os = "windows"))`, so on Linux it is
/// off at *compile* time and a tab physically cannot leave the tab bar. Every
/// other half of the feature was already built: the quadrant maths, the tree
/// surgery, and the pane drop targets the drag would land on.
///
/// This deliberately relaxes only the axis, and not the flag. The same flag
/// gates cross-window tab detach at four other sites, which spawns a ghost
/// window and has never been exercised on this fork's platform — opening the
/// axis is the whole of what a tab-to-pane drag needs, and opening the flag
/// would take a working behaviour and replace it with an untested one.
///
/// **Amended 2026-08-22, after dragging one.** "The whole of what a
/// tab-to-pane drag needs" was true and still is. What it missed is that the
/// axis it relaxes is also the axis you pull along to *tear a tab out into a
/// new window*, and that detach reads the same flag from a different site
/// (`workspace/view.rs`, `is_drag_outside_tab_bar`) — so in the horizontal tab
/// bar a tab can now leave the strip and land nowhere.
///
/// Scope, because a first draft of this note overstated it: this applies to
/// **`tab.rs` only**. `workspace/view/vertical_tabs.rs` is a separate
/// implementation with its own axis lock, is untouched by the fork, and is the
/// one most users here actually see. `DragTabsToWindows` is measured off in
/// every build made here (`RELEASE_FLAGS` needs `cfg!(feature =
/// "release_bundle")`), so tab-out-to-new-window has never worked in either
/// layout — a gap against stock, not something this predicate broke.
///
/// The fix under consideration is the opposite of the paragraph above: force
/// the flag on via [`FORCE_ENABLED`] (which outranks both `cfg`s, per I16),
/// which opens both axis locks and the detach together, and delete this relax
/// as redundant. See `.fork/TASKS.md` T8.2 "REVISIT SOON".
///
/// Consumed by `tab::Tab::render` and `workspace::view`'s drop handling.
pub fn tab_pane_drag_enabled() -> bool {
    is_active()
}

/// Set to `0`, `off` or `false` to make the hotkey window open a plain
/// terminal, the way upstream does.
const QUAKE_VISOR_ENV_VAR: &str = "WARP_FORK_QUAKE_VISOR";

/// Whether the dedicated hotkey window opens in agent view (T8.1).
///
/// Upstream's "quake mode" window is a finished feature — global shortcut,
/// `WindowStyle::Pin`, per-edge geometry, hide-on-blur — pointed at a shell.
/// The only thing the fork changes is what is in it: a drop-down prompt is far
/// more useful as an agent you can ask something than as a fifth terminal.
///
/// **Default on, but it never overrides an explicit choice.** The window is
/// built by `configure_empty_workspace`, which already enters agent view when
/// the global *default session mode* is `Agent`; forcing it a second time
/// would start a second conversation in the same pane. So this only decides
/// the case the setting leaves as a terminal, and a user who wants the stock
/// behaviour turns it off here without giving up the hotkey window.
///
/// It is deliberately *not* gated on [`local_agent_enabled`]. The visor is a
/// surface, not a transport — it is equally the right window whether the reply
/// comes from the `claude` CLI or from a signed-in account.
///
/// Consumed by `root_view::toggle_quake_mode_window`.
pub fn quake_visor_opens_agent() -> bool {
    is_active() && quake_visor_from(std::env::var(QUAKE_VISOR_ENV_VAR).ok().as_deref())
}

/// Split from the environment so the decision can be asserted without setting
/// a process-global variable from a test that runs beside others.
fn quake_visor_from(value: Option<&str>) -> bool {
    !matches!(
        value.map(str::trim),
        Some("0") | Some("off") | Some("false")
    )
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

/// Whether a Warp Drive object's whole trash lifecycle happens without the
/// server: trash, restore, delete forever, empty trash.
///
/// One predicate for all four because they are one question — does removing an
/// object need permission from somewhere else? — and answering it differently
/// per verb is how a trash you can fill but not empty comes about, which is
/// exactly the state this fork was in between T4.4f and T4.7.
///
/// Upstream's `UpdateManager::trash_object` opens with
/// `let Some(server_id) = id.server_id() else { return; }` — an object the
/// server has never heard of cannot be trashed, because trashing *is* a server
/// mutation. Account-free, no object ever has a server id, so the Drive
/// panel's Trash menu item, `WorkflowAction::Trash` and the workflow modal's
/// delete all silently do nothing.
///
/// Worse if it got past that gate: the request is made optimistically and the
/// local `trashed_ts` is *reverted* when it fails, and without credentials it
/// always fails. So there is no ordering in which upstream's path works here.
///
/// Under this policy the trash is what it already claims to be locally — a
/// timestamp on the object plus a row update — and the server round trip is
/// skipped rather than attempted and rolled back.
///
/// Found while designing T4.4f, whose "an object missing from the tree is
/// trashed rather than deleted" rule depends on trashing working at all.
///
/// T4.7 finished the lifecycle. `empty_trash` and `delete_object_with_
/// initiated_by` are bare server calls that only touch anything locally once
/// the response arrives, so account-free a trashed object could not be got rid
/// of at all. And the Drive panel gates "Restore" and "Delete forever" on
/// `has_server_id`, so neither was ever drawn: the fix had to reach the view
/// or the fixed code would have had no way to be called.
pub fn drive_deletes_are_local(app: &AppContext) -> bool {
    local_drive_is_authoritative(app)
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
