# Fork spec — de-telemetry, de-account, own-agent Warp

> **Read this for the reasoning, not for the status.** Every phase below has
> shipped. The plan was superseded by `TASKS.md` from Phase 5 on, and the
> per-phase status markers here were never maintained — the board is the source
> of truth for what is done.
>
> What is still worth reading, and is not written down anywhere else: the
> **survey findings** (what the request path actually looked like before any of
> this), the **kill-switch seam** analysis, and the **status correction** — the
> moment an earlier draft's "Tier 1 already works, zero code" turned out to be
> wrong when someone finally ran the built app. That correction is why this fork
> verifies by running.
>
> | phase here | shipped as | note |
> |---|---|---|
> | 0 — repo hygiene, branches | `P0` | |
> | 1 — kill switch | `P1a`–`P1e` | Behavioural, not textual. No deletions. |
> | 2 — reroute the `claude_code` harness | `P2a`, then **T5** | The real answer was smaller than this plan: one function, not a harness reroute. See T5.3. |
> | 3 — extend the provider layer | **T3** | The open question below was answered — BYO keys are direct client→provider, so there is no transport to rewrite. |
> | 4 — local OpenTelemetry | `P4a` | |

Phased plan. Each phase is independently shippable and independently
revertible. Ordering is chosen so the highest-value thing (Claude as my agent)
lands before the largest-surface thing (full provider abstraction).

## Survey findings (measured 2026-08-17, upstream @ `19dc50535`)

Counts are *files containing the term*, across `crates/` and `app/`:

| Term        | Files | Note                                                     |
|-------------|-------|----------------------------------------------------------|
| `telemetry` | 435   | **342 of them in `app/src` alone** — heavily concentrated |
| `segment`   | 148   | Segment analytics                                        |
| `sentry`    | 63    | Crash/error reporting                                    |
| `quota`     | 38    | Request limiting                                         |
| `request_limit` | 34 | Request limiting                                         |
| `anonymous_user` | 42 | Logged-out handling                                     |
| `rudderstack` | 16  | Analytics pipeline                                       |
| `analytics` | 31    |                                                          |
| `subscription` | 184 | **Mostly Rust stream subscriptions, not billing.** Must be separated by hand — do not bulk-edit this term. |

Load-bearing modules:

- `crates/warp_features/src/lib.rs` (1316 lines) — the `FeatureFlag` enum.
  **Already contains** `CrashReporting`, `CocoaSentry`, `WithSandboxTelemetry`,
  `RecordAppActiveEvents`, `LogExpensiveFramesInSentry`, `RecordPtyThroughput`.
  Several kill switches therefore already exist upstream and only need to be
  forced off.
- `crates/ai/src/llm_provider.rs` (137 lines) — `LLMProvider` enum:
  `OpenAI | Anthropic | Google | Xai | Unknown`.
- `crates/ai/src/api_keys.rs` (836 lines) — BYO API key storage. Pasted keys
  already supported for OpenAI, Anthropic, Google (not Xai).
- `crates/ai/src/telemetry.rs` (224 lines) — `AITelemetryEvent`.
- `crates/mcp/` — MCP runtime **with OAuth already implemented**.
- `crates/warp_multi_agent_client/`, `crates/warp_server_auth/`,
  `crates/graphql/src/api/{billing,api_keys}.rs`.

### Request-path trace — RESOLVED 2026-08-17

`LLMProvider` is **metadata only** — icons, display names, key slugs. It is
*not* a transport trait. But tracing the outbound path found **two entirely
separate request paths**, which is the single most important fact in this spec:

**Path A — Warp's native agent (proxied, account-bound).** No provider
hostname (`api.anthropic.com`, `api.openai.com`, …) appears anywhere in the
client's Rust source; the only hits are bundled skill *documentation*. Warp's
own agent talks to `api.warp.dev`. This path cannot be freed client-side — the
inference happens on Warp's servers. **Abandon it.**

**Path B — the agent-harness layer (direct, already local).**
`app/src/ai/agent_sdk/` is a full pluggable harness system that spawns
*external agent CLIs* which talk directly to providers, with no Warp proxy:

- `driver/harness/claude_code.rs` (+ `parent_bridge.rs`, `wake_driver.rs`,
  `claude_transcript.rs`) — drives the real `claude` CLI
- `driver/harness/codex.rs`, `driver/harness/gemini.rs`
- `provider.rs`, `oauth_flow.rs`, `api_key.rs`, `mcp.rs`, `mcp_config.rs`,
  `profiles.rs`, `model.rs`, `runner.rs`

It sets `ANTHROPIC_API_KEY`, `ANTHROPIC_MODEL`, `CLAUDE_CODE_USE_BEDROCK`, and
crucially `CLAUDE_CONFIG_DIR` — which **already defaults to `~/.claude`**
(`claude_transcript.rs:86`, `claude_code.rs:665`). So the harness inherits the
real Claude Code config: **subscription auth, MCP servers, skills, hooks.**

**Conclusion: Phase 3 is not "build a provider layer" — it already exists.**
The fork's job is to reroute it, not write it.

### The catch, and therefore the actual work

`FeatureFlag::AgentHarness` is documented as: *"Enables the `--harness` flag for
`oz agent run`, allowing external agent CLIs (e.g. `claude`) to execute prompts
instead of Warp's agent harness."*

The harness is wired to **`oz agent run`** — Warp's *cloud* runner — not to the
local terminal agent surface. `mod.rs:279` and `:323` both read
`args.harness != Harness::Oz && !FeatureFlag::AgentHarness.is_enabled()`.

So the real engineering task is **bridging the existing claude_code harness to
the local agent UI**, bypassing Oz. Everything else (spawning, transcript
parsing, MCP config, API keys, retry) is reusable as-is.

### The kill-switch seam — better than expected

`FeatureFlag::is_enabled()` (`warp_features/src/lib.rs:1075`) resolves in
priority order:

```rust
overrides::get_override(*self)              // ← highest priority, local
    .or(USER_PREFERENCE_MAP[*self as usize].get())
    .or(Some(FLAG_STATES[*self as usize].load(Ordering::Relaxed)))  // ← server-pushed
    .unwrap_or(false)
```

**Correction to an earlier draft of this document:** `overrides` is *not*
usable. It is `#[cfg(feature = "test-util")]`; in production
(`lib.rs:1171-1177`) `get_override` is a hardcoded `None` stub, and the
test-util version is thread-local — useless for a GUI app's many threads.

The usable seam is the **second** line: `set_user_preference`
(`lib.rs:1106`) is production API, and `USER_PREFERENCE_MAP` is referenced in
exactly three places — definition, read, write. It is **never cleared or
reset**, so a preference set once at startup permanently outranks both channel
defaults and server-pushed state. Only one caller exists upstream (a test), so
there is no contention.

That makes the kill switch a *purely additive* call at startup — no upstream
list edited, no call site touched.

Relevant flags: `AgentHarness`, `APIKeyManagement`, `McpOauth` (enable);
`CrashReporting`, `CocoaSentry`, `WithSandboxTelemetry`,
`RecordAppActiveEvents`, `LogExpensiveFramesInSentry`, `RecordPtyThroughput`
(disable); `CloudEnvironments`, `CloudAgentRunners`, `CloudRunners`,
`CloudConversations`, `OzIdentityFederation`, `OzPlatformSkills`, `OzHandoff`
(disable — these are Warp's paid cloud infra).

### Cargo features do the heavy lifting

Flags are enabled at **compile time** via Cargo features
(`app/src/features.rs`, `#[cfg(feature = "…")]`). Measured against
`app/Cargo.toml`'s 198-entry `default` list:

- `crash_reporting` / `cocoa_sentry` are **not** in `default`, and
  `crash_reporting = ["dep:sentry", "dep:minidumper", "dep:crash-handler", …]`.
  **A stock build already contains no Sentry code at all.** This is a real
  removal, not a no-op — worth knowing before "removing telemetry" by hand.
- `agent_harness`, `api_key_management`, `solo_user_byok`,
  `skip_firebase_anonymous_user`, `mcp_oauth`, `mcp_server` **are** in
  `default` — already on.
- `global_ai_analytics_collection` **is** in `default` — a genuine target.

Cargo features are additive-only, so *disabling* cannot be done by adding a
feature. Hence the split: **enable** via Cargo features, **disable** via
`set_user_preference` at runtime. This avoids ever editing the 198-line
`default` array, which would conflict on nearly every upstream merge.

Relevant flags: `AgentHarness`, `APIKeyManagement`, `McpOauth` (enable);
`CrashReporting`, `CocoaSentry`, `WithSandboxTelemetry`,
`RecordAppActiveEvents`, `LogExpensiveFramesInSentry`, `RecordPtyThroughput`
(disable); `CloudEnvironments`, `CloudAgentRunners`, `CloudRunners`,
`CloudConversations`, `OzIdentityFederation`, `OzPlatformSkills`, `OzHandoff`
(disable — these are Warp's paid cloud infra).

## Phase 0 — repo hygiene and branches ✅ done

See `README.md`. Working tree normalized 5,894 → 7 files; branch topology
established.

## Status correction (2026-08-17, after first real run)

An earlier draft claimed Tier 1 "already works, zero code". **That was wrong.**
Running the built app showed the entire AI surface greyed out, with
*"Without an account, you won't have access to Warp's AI features"* and
*"Create an account to use your own API keys"*. The account gate — specced in
Phase 1 but never implemented — was the actual blocker, not telemetry.

Two things were then fixed:

**Account gate.** Three single conditions, not 31 call sites:
`settings::ai::AISettings::is_any_ai_enabled` (master switch),
`UserWorkspaces::is_byo_api_key_enabled`, and
`UserWorkspaces::is_custom_inference_enabled`. Deliberately *not* overriding
`is_anonymous_or_logged_out` itself — that opens all 31 gates including ones
that then call the server with no credential.

**Telemetry collection.** The flag work stopped scheduled *sending*, but a live
run logged `Successfully wrote telemetry events to disk`
(`server/telemetry/collector.rs:88`) — events were still being recorded and
persisted as a backlog "for sending on the next app startup". The egress
deny-list was doing all the real work. Now the five
`PrivacySettingsSnapshot` accessors are overridden, including
`is_telemetry_force_enabled` (an override that re-enables telemetry regardless
of preference) and `should_collect_ai_ugc_telemetry` (prompts and responses).

**The open Phase 3 question is also answered**, by Warp's own settings copy:
*"API keys added here are stored only on this device, not on Warp's servers."*
BYO keys are direct client→provider calls. There is no proxy to replace —
Phase 3 is a gate removal plus a harness, not a transport rewrite.

## Phase 1 — kill switch (chosen strategy: behavioral, not textual)

**Principle: no deletions.** Add one fork-owned module that forces the relevant
switches off at the narrowest seam, rather than ripping out call sites. A
several-hundred-file deletion patch would conflict on essentially every
upstream merge; a seam override conflicts almost never.

New crate `warp_fork_config` exposing a single source of truth:

```rust
pub fn telemetry_enabled() -> bool { false }
pub fn crash_reporting_enabled() -> bool { false }
pub fn account_required() -> bool { false }
```

Seams to override, narrowest first:

1. **Feature flags** — force the existing telemetry flags to evaluate `false`
   in `warp_features`. Cheapest possible change; covers everything already
   gated upstream.
2. **Emit sites** — no-op the Segment/Rudderstack dispatch function and the
   Sentry init, at the one place each is constructed. Events still get
   *created*; they are simply never *sent*. Costs a negligible amount of dead
   work and keeps every call site compiling untouched.
3. **Network egress** — deny-list analytics hosts in `crates/http_client` as a
   backstop, so anything missed at layers 1–2 still cannot phone home.

Layer 3 is what makes this trustworthy: it converts "I think I found every
call site" into "nothing can leave regardless."

**Verification:** the fork is only as good as its proof. Run the built client
against a local proxy and assert zero requests to Segment/Rudderstack/Sentry
hosts. Do not claim telemetry is gone on the basis of grep counts alone.

Account/paywall gating gets the same treatment — force entitlement checks to
report full local entitlement rather than deleting the checks. Note this
removes *client-side* gating only; anything genuinely computed on Warp's
servers cannot be unlocked client-side and must instead be replaced by a local
implementation (which is the point of Phases 2–3).

## Phase 2 — reroute the claude_code harness to the local agent

Rationale for going through the `claude` CLI rather than the raw API: a Pro/Max
subscription authenticates **only** through Claude Code's own auth flow. The
raw Anthropic API bills separate credits and cannot use a subscription.
Path B already spawns that CLI with `CLAUDE_CONFIG_DIR` defaulting to
`~/.claude`, so subscription auth is inherited with no new auth code.

### "Claude as primary agent" has three tiers, not one

Tracing `Harness::Oz` turned up a third system, separate from both Path A and
the orchestration harness. Ranked by cost:

**Tier 1 — Claude as a CLI agent in a pane. Already works, zero code.**
`app/src/terminal/cli_agent.rs` defines a `CLIAgent` enum with first-class
support for Claude, Codex, Gemini, OpenCode, Copilot, Cursor, Goose, Amp,
Droid, Auggie, Pi, Antigravity. Warp gives these rich input, a Warpify footer,
notifications, and per-agent icons. Every backing Cargo feature
(`cli_agent_rich_input`, `warpify_footer`, `agent_cli_launch_modal`,
`pluggable_notifications`) is in `default`, and **neither `cli_agent.rs` nor
`local_harness_launch.rs` contains any auth check**. Just run `claude` in a
Warp pane.

This is the pragmatic answer to "Claude instead of Oz" and it is available
today, account-free.

**Tier 2 — Claude as a local orchestration/child harness.** Delivered by
`app/src/fork.rs` (`forced_local_harnesses`). Lets Warp *spawn* Claude as a
child agent rather than only hosting it in a pane.

**Tier 3 — replace Agent Mode's backend so Warp's own agent surface is driven
by Claude.** This is the expensive one, and the cost is concrete:

- `AIClient` (`app/src/server/server_api/ai.rs`) is a **70-method** trait
  behind `Arc<dyn AIClient>`. The trait object is a real seam, but a local
  implementation must satisfy conversations, agents, tasks, memory stores,
  skills, artifacts, credits and request limits.
- Agent-run events arrive over a **separate SSE stream**
  (`ai/agent_events/driver.rs`, `AgentEventFilter`, reconnect/backoff logic)
  keyed on server-issued run IDs.
- Conversation persistence, `credit_availability`, and `request_usage_model`
  all assume server-side state.

Recommendation: take Tiers 1 and 2, and treat Tier 3 as a research spike
rather than committed work. The incremental benefit over Tier 1 is mostly
cosmetic — Claude already runs with Warp's rich UI — while the cost is
re-implementing 70 trait methods plus an event-stream protocol, all of which
upstream will keep changing under the fork.

## Phase 3 — extend the existing provider layer

Revised: do **not** build a new `AgentBackend` trait. Warp's `Harness` enum
already is that abstraction, with working `claude_code`, `codex`, and `gemini`
implementations. Extending it beats replacing it — an added enum variant is a
far smaller merge surface than a parallel architecture.

- Add a local-model harness (Ollama / OpenAI-compatible endpoint) alongside the
  existing three.
- Extend `LLMProvider` with `Ollama`/`Local` so the existing settings UI, icons,
  and key storage keep working unmodified.
- Replace the server-provided model list
  (`get_feature_model_choices`) with a local registry so the client works fully
  offline.

Sequencing: Phase 2 must land first — it proves the local dispatch path that
every other harness will reuse.

## Phase 4 — local telemetry (OpenTelemetry, not Sentry)

Use **OpenTelemetry**. This is not a close call, and it needs almost no new
code, because upstream already ships it:

- `opentelemetry` 0.32, `opentelemetry-otlp`, `opentelemetry_sdk` and
  `tracing-opentelemetry` are already workspace dependencies.
- `app/src/tracing/native.rs` already builds an OTLP exporter over
  `Protocol::HttpBinary` — standard OTLP/HTTP, so any collector works
  (otel-collector, Jaeger, Grafana Tempo, …).
- Export is **opt-in and off by default**: with the `CLOUD_AGENT_OTLP_ENDPOINT`
  env var absent or empty, `init` returns early and nothing is exported
  (`native.rs:107`).
- It **deliberately permits plain `http` for loopback hosts** and requires
  HTTPS otherwise (`native.rs:217`). That affordance exists precisely for a
  local collector.
- `OTEL_SERVICE_NAME` is honoured for resource naming.

Sentry is the wrong tool for this regardless of the fork's goals: it is
crash/error reporting, not tracing or metrics. It answers "what broke", not
"what did the agent do, in what order, and how long did each step take".
Keep it removed.

### It already gives harness/agent observability

`app/src/ai/agent_sdk/setup_observability.rs` (313 lines) already emits named
`tracing` spans for agent lifecycle stages — `setup_environment_resolution`,
`setup_environment_repo_clone`, `setup_environment_setup_commands`,
`setup_environment_codebase_indexing`, `setup_environment_skill_loading` —
alongside `driver/harness/telemetry.rs`. Because `tracing-opentelemetry`
bridges `tracing` spans into OTel, these become OTLP spans for free.

**One change is needed.** `filter_cloud_agent_span` (`native.rs:526`) drops
every span not tagged `tags.cloud_agent`, so today only cloud-agent runs are
exported. Local harness spans are created but filtered out. Extending that
filter to include local harness spans is the whole job.

Sequencing: this is additive and independent of Phases 1–3, so it can be done
whenever. Bring it up by pointing `CLOUD_AGENT_OTLP_ENDPOINT` at
`http://localhost:4318` with a collector running, then widen the filter.

Note the egress backstop deliberately does not block loopback, so a local
collector is unaffected by Phase 1.

### Risks

- **UI coupling.** Warp's agent UI may assume server-shaped responses. If so,
  the adapter in Phase 2 is the real work, not the transport.
- **GraphQL coupling.** Model lists arrive from the server. Offline operation
  needs a local model registry to stand in.
- **Merge drift.** Every upstream merge can move these seams. Keep the fork
  diff small enough to re-audit in one sitting.
