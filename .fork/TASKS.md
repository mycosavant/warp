# Fork task board

Tracks the full scope agreed 2026-08-17. Ordered by value-per-line-of-code, not
by conceptual grandeur — see `SPEC.md` for the reasoning behind each.

Status key: `[ ]` not started · `[~]` in progress · `[x]` done · `[!]` blocked
· `[-]` dropped (with reason)

Phases 0–4 in `SPEC.md` are the original de-telemetry/de-account track. This
board supersedes it from Phase 5 on, and renumbers nothing.

---

## Done (carried over from SPEC Phases 0–2)

- [x] **P0** Repo hygiene, branch topology, git-lfs, CRLF/symlink corruption
- [x] **P1a** Telemetry egress deny-list (`crates/http_client/src/egress.rs`)
- [x] **P1b** Telemetry collection shutdown (`settings/privacy.rs` accessors)
- [x] **P1c** Feature-flag kill switch (`app/src/fork.rs`)
- [x] **P1d** Account gates — master AI switch, BYO key, custom inference
- [x] **P1e** Account gate — settings UI banner (`is_anonymous_for_ui`)
- [x] **P4a** Local OpenTelemetry export, loopback-only auth bypass
- [x] **P2a** Local harnesses forced available when logged out
- [x] Native Windows build verified end-to-end (`C:\dev\warp`)

---

## T1 — `warpctrl` local control plane  ← ACTIVE

The highest value-per-line item in the fork. A complete local IPC control
plane for a running Warp instance already exists, fully written and tested,
disabled behind a dogfood flag. This is the orchestration surface for driving
Warp from Claude Code.

Reference: `crates/warp_cli/src/local_control/`, `app/src/local_control/`,
`crates/local_control/`.

- [x] **T1.1** Force `FeatureFlag::WarpControlCli` on in `fork::FORCE_ENABLED`
- [x] **T1.2** Default `LocalControlSettings` to `Enabled` under fork policy
      via `settings::local_control::effective_default_mode`. Upstream's
      `default_mode_for_channel` left pure so its per-channel test still holds.
- [x] **T1.3** `--warpctrl` entrypoint dispatches. Verified: feature-flag init
      runs before the dispatch in `lib.rs::run`, so `fork::
      apply_feature_preferences` lands in time.
- [x] **T1.4** Verified 2026-08-17 against a live instance, **logged out**:
      `instance list` → instance discovered; `app ping` → reachable;
      `app version`; `app active`; `window list`; `tab list`;
      `setting list` → real setting values. Full chain exercised: discovery
      record → Unix-socket credential broker → loopback HTTP + bearer →
      `LocalControlBridge` on the main thread.
- [x] **T1.6** No account gate anywhere on the local-control path — every
      command above ran with no Warp account. Confirmed by reading
      (`permissions.rs` checks only the feature flag + settings) and
      empirically.
- [x] **T1.5a** Catalog verified: **84 actions, all `implemented`** —
      `app` 4, `window` 5, `tab` 10, `pane` 11, `session` 6, `input` 2,
      `surface` 20, `setting` 4, `theme` 6, `appearance` 7, `keybinding` 2,
      `file` 1, plus `instance`/`action`/`capability` introspection.
- [!] **T1.5b** Mutations unverified — blocked, see below.
- [ ] **T1.7** Document the verified command surface in `.fork/README.md`

### Blocker: no workspace on Linux, no broker on Windows

Mutating actions fail on the WSL build with
`missing_target: tab.create requires a workspace in the target window`.
`window list` shows one window with `has_workspace: false, is_active: false`
— the window object exists but never gets a workspace because it never
composites under WSLg. Not a local-control defect; the same WSLg RAIL
forwarding failure that pushed the build to Windows.

And local control **does not work on Windows at all**, by upstream design:

    #[cfg(not(unix))]
    /// Fails closed on platforms without an owner-authenticated broker transport.
    fn request_credential_over_owner_ipc(..) -> Result<String, ControlError> {
        Err(ControlError::new(ErrorCode::LocalControlDisabled,
            "local control requires an owner-authenticated credential broker"))
    }

`request_credential` is on every request path, so every action fails closed.
Discovery *is* ported (`is_pid_alive` has a `#[cfg(windows)]` arm via
`tasklist`) and the loopback HTTP transport is `cfg(not(wasm))`, so the
credential broker is the sole missing piece. Unimplemented platform, not a bug.

Net: the build that renders can't be controlled; the build that can be
controlled won't render.

- [ ] **T1.10** Windows named-pipe credential broker — the recommended unblock.
      Bounded: a `#[cfg(windows)]` sibling for `bind_credential_broker` /
      `run_credential_broker` (app side) and `request_credential_over_owner_ipc`
      (client side). Named pipes preserve the security model: a restrictive
      DACL replaces the 0600 mode bits, and `GetNamedPipeClientProcessId` plus
      client impersonation replaces kernel-reported peer UID. Purely additive
      alongside the existing `#[cfg(unix)]` arms.
- [ ] **T1.11** Alternative: fix WSLg rendering so the Linux build is usable.
      Also serves T6, but has resisted several attempts already.

Deferred, dependent on T1 landing:

- [ ] **T1.8** `input submit` action — upstream deliberately ships only
      `insert`/`replace`, so a seeded command is never auto-executed. Adding
      submit is a local patch. Decide whether we want it; it is the difference
      between "assist" and "autonomous".
- [ ] **T1.9** MCP wrapper over the action catalog. `warpctrl action list` and
      `capability inspect` already emit machine-readable metadata, so tool
      definitions can be generated rather than hardcoded.

## T2 — Local voice transcription (replace Wispr Flow)

Cleanest seam in the codebase: `Transcriber` is a one-method trait and
`VoiceTranscriber::new` is the injection point. Upstream docstring states it
is built this way "to avoid the editor having a direct dependency on any
server api."

Privacy note: this is a fix, not a preference. `Provider::OpenAI` is **not** a
local path — `ServerVoiceTranscriber` POSTs base64 audio to `api.warp.dev`
regardless of provider. Voice currently leaves the machine either way.

- [ ] **T2.1** `LocalTranscriber` implementing `voice::transcriber::Transcriber`
- [ ] **T2.2** Backend: local whisper endpoint and/or `whisper.cpp` subprocess
- [ ] **T2.3** Swap the singleton registration under fork policy
- [ ] **T2.4** Repoint the `wisprflow.ai` settings link
      (`WISPR_FLOW_URL`, `warp_agent_page.rs:128`)
- [ ] **T2.5** Verify no audio egress under recording (proxy check)

## T3 — Re-plumb the four small AI features locally

These are **not** on the agent or any harness — each is an independent
single-shot GraphQL call to `api.warp.dev`. No streaming, no tool use, no
session state. Individually shippable.

| Toggle | Backing call |
|---|---|
| Next Command | `generate_commands_from_natural_language` |
| Prompt Suggestions | `generate_dialogue_answer` |
| Block Title Generation | `generate_metadata_for_command` |
| Commit & PR Generation | `generate_code_review_content` |

- [ ] **T3.1** Local completion client (own key, provider-agnostic)
- [ ] **T3.2** Route `generate_metadata_for_command` (smallest — do first)
- [ ] **T3.3** Route `generate_commands_from_natural_language`
- [ ] **T3.4** Route `generate_dialogue_answer`
- [ ] **T3.5** Route `generate_code_review_content`
- [ ] **T3.6** Model selection for these independent of the agent model

## T4 — Local-first Warp Drive

Better shape than expected: a full local SQLite store already exists
(`crates/cloud_object_persistence`, diesel + bundled sqlite3). The server is a
**sync layer on top**, with `UpdateSource::{Server, Local}` already
distinguishing origins, plus a working offline mode and `ExportManager`.

So this is "keep the store, neutralize the sync" — not a rewrite.

- [ ] **T4.1** Map every server-sync entry point in `cloud_object/model/persistence.rs`
- [ ] **T4.2** Local-only mode: full read/write, no account, no sync
- [ ] **T4.3** Confirm the offline read-only banner does not apply in local mode
- [ ] **T4.4** Git-backed sync — versioned, portable, self-hosted or not
- [ ] **T4.5** Round-trip via the existing import/export paths

Explicitly **not** doing: Proton Drive. No general-purpose public API,
E2E-encrypted with client-side key management; integration means
rclone-shaped reverse engineering, trading a working local store for a
fragile sync target. Revisit only after T4.4 works.

## T5 — Claude in Oz's seat (the spike)

Making Claude the Warp Agent proper, not a CLI harness in a pane. This is the
genuinely hard one: the 70-method `AIClient` trait plus the SSE agent-event
stream.

- [ ] **T5.1** Determine the true minimum viable `AIClient` subset
- [ ] **T5.2** Map the SSE agent-event protocol
- [ ] **T5.3** Decide: implement the trait, or shim at the transport layer
- [ ] **T5.4** Prototype behind a fork flag, default off

## T6 — WSL integration

User-stated high-priority feature-add, not yet scoped. File explorer and
remaining features seamless across Windows and WSL2.

- [ ] **T6.1** Scope what "seamless" means concretely; enumerate broken surfaces
- [ ] **T6.2** Path translation (`\\wsl.localhost\...` ↔ `/mnt/c/...`)
- [ ] **T6.3** File explorer across the boundary
- [ ] **T6.4** Decide the WSLg window-forwarding story or stay Windows-native

---

## Decisions on record

- **Claude subscription auth: do not reimplement Anthropic's OAuth.**
  `crates/ai/src/grok_subscription/oauth.rs` proves the pattern works and is
  fully client-side (loopback PKCE, no Warp server) — but it works by reusing
  Grok-CLI's allowlisted `client_id`, i.e. Warp impersonates Grok-CLI. The
  Claude equivalent would put the subscription itself at risk, and would yield
  a bare token without the Claude Code config, MCP servers, skills, plugin or
  memory that make it useful. Drive the real `claude` binary instead — which
  is what `Harness::Claude` already does.

  Consequence: subscription → CLI harness. API key → Custom Inference. Two
  separate doors; the subscription only fits the first.

- **Ordering is inverted from intuition.** The four small features (T3) are
  easy and independent; the "just make Claude the agent" ask (T5) is the hard
  one. Do the small ones first so the provider layer is proven before the
  spike.

## Open questions

- [ ] Log spam on window move (`workspace:save_app` per window event). Upstream,
      present since the earliest runs, not fork-introduced. Silence via
      `RUST_LOG` or debounce?
- [ ] Windows Developer Mode so `.claude/skills` resolves as a symlink on the
      Windows checkout.
- [ ] Proxy-based verification that nothing escapes under real activity — only
      idle runs observed so far.
