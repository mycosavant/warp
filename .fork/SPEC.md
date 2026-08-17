# Fork spec — de-telemetry, de-account, own-agent Warp

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

An **`overrides` mechanism already exists and outranks server-pushed state.**
That is a single, upstream-maintained, highest-priority hook through which the
entire fork can force flags on/off without editing a single call site. This is
the ideal kill switch — near-zero merge surface.

Relevant flags: `AgentHarness`, `APIKeyManagement`, `McpOauth` (enable);
`CrashReporting`, `CocoaSentry`, `WithSandboxTelemetry`,
`RecordAppActiveEvents`, `LogExpensiveFramesInSentry`, `RecordPtyThroughput`
(disable); `CloudEnvironments`, `CloudAgentRunners`, `CloudRunners`,
`CloudConversations`, `OzIdentityFederation`, `OzPlatformSkills`, `OzHandoff`
(disable — these are Warp's paid cloud infra).

## Phase 0 — repo hygiene and branches ✅ done

See `README.md`. Working tree normalized 5,894 → 7 files; branch topology
established.

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

Work items, smallest first:

1. Force `AgentHarness` + `APIKeyManagement` on via the `overrides` hook.
2. Trace how `Harness::ClaudeCode` runs are dispatched, and add a **local**
   dispatch path that does not route through `oz agent run`.
3. Adapt the harness output stream (`driver/output.rs`,
   `harness/claude_transcript.rs`) onto whatever event type the local agent UI
   consumes. **This is the bulk of the work** — see risk below.

Deliberately deferred: making this the default. Keep it behind a fork flag
alongside the stock path so upstream's agent keeps working while this is
unstable.

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

### Risks

- **UI coupling.** Warp's agent UI may assume server-shaped responses. If so,
  the adapter in Phase 2 is the real work, not the transport.
- **GraphQL coupling.** Model lists arrive from the server. Offline operation
  needs a local model registry to stand in.
- **Merge drift.** Every upstream merge can move these seams. Keep the fork
  diff small enough to re-audit in one sitting.
