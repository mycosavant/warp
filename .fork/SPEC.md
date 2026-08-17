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

### Critical open question (blocks Phase 3 design)

`LLMProvider` is **metadata only** — it supplies icons, display names, and key
slugs. It is *not* a transport trait; there is no `trait LlmProvider { fn
complete(...) }`. Model choice arrives from the server via
`warp_graphql::queries::get_feature_model_choices`.

**Unresolved:** whether a pasted API key is used for a direct client→provider
call, or is merely forwarded so Warp's backend makes the call. This determines
whether Phase 3 is "swap a transport" or "build a transport." Resolve by
tracing the outbound request path before writing Phase 3 code.

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

## Phase 2 — Claude via Agent SDK subprocess (start here)

Rationale: a Pro/Max subscription authenticates **only** through Claude Code's
own auth flow. The raw Anthropic API bills separate API credits and cannot use
a subscription. So driving the Agent SDK as a subprocess is the *only* route
that spends the subscription rather than credits.

Inherited for free: subscription auth, MCP servers, my skills, hooks, and
existing tool definitions.

Sketch: new crate `warp_fork_agent` that spawns the Agent SDK, speaks its
streaming JSON protocol over stdio, and adapts that stream onto the event type
Warp's existing agent UI already consumes — so the terminal UI is reused rather
than rebuilt.

Deliberately deferred: making this the *default* agent path. First make it work
behind a flag alongside the stock path, so upstream's agent keeps working while
this is unstable.

## Phase 3 — pluggable provider layer (design now, build after Phase 2)

Target: one trait, several backends — Agent SDK (subscription), direct
Anthropic/OpenAI/Google API keys, and local models (Ollama).

```rust
trait AgentBackend {
    async fn stream(&self, req: AgentRequest) -> Result<BoxStream<AgentEvent>>;
    fn capabilities(&self) -> Capabilities;  // tools, vision, context window
}
```

`LLMProvider` is extended rather than replaced (add `Ollama`, `Local`), keeping
the existing settings UI, icons, and key-storage code working.

Sequencing depends entirely on the open question above. Answer it first.

### Risks

- **UI coupling.** Warp's agent UI may assume server-shaped responses. If so,
  the adapter in Phase 2 is the real work, not the transport.
- **GraphQL coupling.** Model lists arrive from the server. Offline operation
  needs a local model registry to stand in.
- **Merge drift.** Every upstream merge can move these seams. Keep the fork
  diff small enough to re-audit in one sitting.
