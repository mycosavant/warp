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
- [x] **T1.5b** Mutations verified on Windows 2026-08-17 (blocked on WSL, see
      below): `app focus`, `tab create` → created tab 2374, `tab list`
      confirms 3 tabs with the new one active.
- [x] **T1.8** `input.submit` — replaces the buffer and runs it. Verified
      end-to-end: submitting `Set-Content -Path C:\dev\warpctrl_proof.txt ...`
      produced the file with the expected contents. Newline and control-char
      rejection both confirmed to still fire, so one call runs exactly one
      command.
- [x] **T1.10** Windows named-pipe credential broker — **done and verified**.
- [ ] **T1.7** Document the verified command surface in `.fork/README.md`

Confirmed 2026-08-18: closing the window with `CloseMainWindow` removes the
discovery record, and `instance list` immediately reports none. So the stale
records that produced `ambiguous_instance` during T2 came specifically from
`Kill()`ing the process, not from ordinary shutdown — the cleanup path works,
it just never runs when the process is killed.

### RESOLVED 2026-08-17 — Windows is now the working platform

The port took **four** changes, not the one predicted. In discovery order:

1. `discovery_dir()` resolved through `XDG_RUNTIME_DIR`/`HOME`, neither of
   which Windows reliably has — it would have landed in the working
   directory. Now `LOCALAPPDATA`, with `USERPROFILE` as a `HOME` fallback.
2. `set_private_dir_permissions` / `set_private_permissions` hard-failed
   off-unix, so publication never ran. Now a protected DACL via
   `local_control::windows_security`.
3. The credential broker — the only gap originally identified. Now a named
   pipe carrying the same descriptor.
4. `local_control_publication_supported()` hardcoded
   `cfg!(not(target_os = "windows"))`. This was the one that kept the server
   silently dead after 1–3 were done: a fourth gate behind the feature flag,
   the Scripting setting and the broker. Now states the capability
   (`cfg!(any(unix, windows))`) rather than a platform list.

Verified on Windows against a live logged-out instance:

    discovery record  %LOCALAPPDATA%\warp\local-control\inst_<id>.json
    named pipe        \\.\pipe\warp-local-control\inst_<id>.broker.sock
    instance list     inst_826a... (pid 31152, channel warp-oss, protocol 1)
    app ping          reachable (protocol version 1)
    window list       has_workspace: true
    tab create        Created tab 2374 in window 0 (tab count 3)
    input submit      ok -> command actually executed, proof file written

`instance list` only returns instances that pass `probe_instance`, which runs
the whole broker→HTTP flow, so a bare listing is already end-to-end evidence.

ACL verified empirically rather than assumed — `icacls` on both the registry
directory and the record reports exactly one ACE:

    C:\Users\<user>\AppData\Local\warp\local-control <domain>\<user>:(F)

No SYSTEM, no Administrators, no inherited entries. That is stricter than the
Windows default, and is what `D:P(...)` buys.

### Remaining blocker: no workspace on Linux

Mutating actions fail on the WSL build with
`missing_target: tab.create requires a workspace in the target window`.
`window list` shows one window with `has_workspace: false, is_active: false`
— the window object exists but never gets a workspace because it never
composites under WSLg. Not a local-control defect; the same WSLg RAIL
forwarding failure that pushed the build to Windows.

Windows is now the working platform for local control, so this is no longer
on the critical path — but the Linux build is still unusable as a GUI.

- [ ] **T1.11** Fix WSLg rendering so the Linux build is usable.
      Also serves T6, but has resisted several attempts already.

Deferred, dependent on T1 landing:

- [ ] **T1.8** `input submit` action — upstream deliberately ships only
      `insert`/`replace`, so a seeded command is never auto-executed. Adding
      submit is a local patch. Decide whether we want it; it is the difference
      between "assist" and "autonomous".
- [x] **T1.9** MCP server — `warpctrl mcp`, done and verified. 85 tools
      generated from the catalog. No new dependencies: MCP over stdio is
      newline-delimited JSON-RPC 2.0 and the local-control client is blocking,
      so it is a synchronous stdin loop over `serde_json`.

      Verified on Windows driving a live instance end-to-end:
      `initialize` → `tools/list` (85) → `app.focus` → `tab.create`
      → `input.submit`, with the submitted command confirmed by the file it
      wrote. Errors surface as `isError` results carrying the `ControlError`
      code, so a model can read `missing_target` and focus a window.

      **Bug found by this testing, now fixed:** `input.submit` was reporting
      `isError` for commands that had in fact run. `has_pending_command` means
      *queued*, not *refused* — `can_execute_command` returns
      `No(NotBootstrapped)` while a freshly created tab's shell starts, and the
      pane runs the command once ready. Since `tab.create` immediately followed
      by `input.submit` is the obvious orchestration sequence, that path is
      common. The acknowledgement now carries `executed` and `queued`, both
      verified:

          bootstrapped pane -> executed: true,  queued: false  (file present at once)
          fresh tab         -> executed: false, queued: true   (absent, then present)

## T2 — Local voice transcription (replace Wispr Flow)

Cleanest seam in the codebase: `Transcriber` is a one-method trait and
`VoiceTranscriber::new` is the injection point. Upstream docstring states it
is built this way "to avoid the editor having a direct dependency on any
server api."

Privacy note: this is a fix, not a preference. `Provider::OpenAI` is **not** a
local path — `ServerVoiceTranscriber` POSTs base64 audio to `api.warp.dev`
regardless of provider. Voice currently leaves the machine either way.

- [x] **T2.1** `LocalTranscriber` implementing `voice::transcriber::Transcriber`
      — `app/src/voice/local_transcriber.rs`. Fail-closed: when it is installed
      it is the *only* transcriber, and a misconfiguration is an error rather
      than a fallback to the server. A fallback would paper over exactly the
      failure that matters — the audio went somewhere else.
- [x] **T2.2** Two backends, settings under `agents.voice.local_transcription`.

      `http` posts `multipart/form-data`. whisper.cpp's `whisper-server` and
      the OpenAI-compatible servers (speaches, faster-whisper-server, LocalAI)
      agree on the request shape and on `{"text": ...}` for the reply, and
      differ only in route — which is why the endpoint setting is a whole URL.
      Measured against a live server rather than assumed:

          POST /inference                 -> {"text":" List the files in this directory.\n"}
          POST /v1/audio/transcriptions   -> 404 File Not Found
          extra `model` / `language` form fields  -> tolerated
          empty body                      -> 400 "Invalid request" (plain text)

      So the default endpoint is whisper.cpp's `/inference`, the response
      parser trims the leading space and trailing newline, and a non-2xx body
      is surfaced verbatim (tail-truncated).

      `command` writes the recording to a 0600 temp file and runs a binary,
      reading the transcript from stdout. Arguments are split *before*
      placeholder substitution so a model path under `C:\Program Files` stays
      one argument.
- [x] **T2.3** Registration swapped at `lib.rs` via
      `fork::local_voice_transcription_enabled`. Settings are mirrored into a
      snapshot because `transcribe` runs off the main thread with no context; a
      test drives the real subscription and asserts an edited endpoint arrives
      without a restart. Settings groups emit a `ChangedEvent` rather than
      notifying observers — `ctx.observe` here would have silently never fired.
- [x] **T2.4** `WISPR_FLOW_URL` retained for the non-fork branch; under fork
      policy the description says audio is transcribed on this machine and
      links to whisper.cpp. The old text ("powered by Wispr Flow") would now be
      false, so the whole sentence changes, not just the URL.
- [~] **T2.5** No audio egress — *argued and unit-tested, not proxy-verified.*
      Under fork policy `ServerVoiceTranscriber` is never constructed
      (`fork_policy_installs_a_local_transcriber`), and `LocalTranscriber`
      contacts only the configured endpoint or spawns the configured binary.
      The default endpoint is asserted to be loopback. What is **not** done is
      a proxy capture during a real recording: that needs a microphone and a
      human to speak into it. Worth doing once by hand.

**Verified**

    linux    31 unit tests; live HTTP against whisper-server -> "List the files in this directory."
    windows  29 unit tests (2 unix-only skipped); same live transcription
    windows  app builds, launches, runs with the settings.toml block in place

**Bug caught by clippy, would have shipped:** `std::process::Command` flashes a
console window on Windows on every invocation. `command::blocking::Command`
fixes that but defaults to `CREATE_BREAKAWAY_FROM_JOB`, which `CreateProcess`
refuses inside a job that disallows it — `Access is denied` when running any
binary. Clearing the flags keeps `CREATE_NO_WINDOW` and drops breakaway, which
is right anyway: breakaway exists so shells outlive Warp, and a transcriber
Warp is synchronously waiting on should not.

**Not done, deliberately:** the new settings are not reachable from `warpctrl`.
`setting.get`/`set` operate on `ALLOWLISTED_SETTING_KEYS`, a curated list of
ten appearance and input keys each with hand-written accessors. Adding voice
keys is plumbing, and belongs with T1, not here.

**Open, unexplained:** the dev build never creates
`%LOCALAPPDATA%\warp\WarpOss\data\logs\warp-oss.log` — it writes to
`warp-oss.log.recovery` instead, and the `.old.N` slots are frozen at 169
bytes. `setup_log_files_for_current_execution` routes to the `.recovery` path
when `is_from_crash_recovery_process`, and Warp launches a recovery child that
takes over when the parent dies (`crash_recovery::wait_for_parent_crash`,
"Parent has crashed; continuing execution"), so repeatedly `Kill()`ing the
process during testing leaves the recovery child in charge. But a launch with
zero prior `warp-oss` processes reproduced it, so that is not the whole story.

Further evidence 2026-08-18: two `warp-oss` launches an hour apart, one wrote
`warp-oss.log` normally and the next wrote `.recovery`, with no deletion in
between. So it is per-run state, not a one-way door — consistent with the
recovery-child theory and inconsistent with "the file is broken".

I deleted `warp-oss.log` during this testing before understanding any of the
above, so I cannot fully separate cause from coincidence — but recreating the
file did not fix it and the recovery-path evidence points elsewhere. The
installed stable Warp logs normally and was not touched. Worth a look before
relying on the OSS build's file logs for anything.

## T3 — Re-plumb the four small AI features locally  ← DONE

These are **not** on the agent or any harness — each is an independent
single-shot call to `api.warp.dev`. No streaming, no tool use, no session
state. Individually shippable.

**The mapping recorded here previously was wrong for three of the four.** It
named GraphQL mutations; all four are in fact plain REST `POST`s to `/ai/*`
with a JSON body and a JSON reply. Corrected and verified by tracing each
settings toggle to the call it actually issues:

| Toggle | Setting | Route | Method |
|---|---|---|---|
| Next Command | `IntelligentAutosuggestionsEnabled` | `/ai/generate_input_suggestions` | `ServerApi::generate_ai_input_suggestions` |
| Prompt Suggestions | `AgentModeQuerySuggestionsEnabled` | `/ai/generate_am_query_suggestions` | `ServerApi::generate_am_query_suggestions` |
| Shared Block Title Generation | `SharedBlockTitleGenerationEnabled` | `/ai/generate_block_title` | `BlockClient::generate_shared_block_title` |
| Commit & PR Generation | `git_operations_autogen_enabled` | `/ai/generate_code_review_content` | `AIClient::generate_code_review_content` |

The three mutations named before are real, but back different features:
`generate_metadata_for_command` is the workflow-metadata assistant,
`generate_commands_from_natural_language` is `#`-prefixed AI command search,
and `generate_dialogue_answer` is the legacy AI assistant panel.

- [x] **T3.1** Local completion client (own key, provider-agnostic) —
      `ai::local_completion`. Three wire protocols, chosen by the existing
      `CustomEndpointSchema`: OpenAI Chat Completions, OpenAI Responses,
      Anthropic Messages. Zero new dependencies.
- [x] **T3.2** Route `/ai/generate_block_title`
- [x] **T3.3** Route `/ai/generate_input_suggestions`
- [x] **T3.4** Route `/ai/generate_am_query_suggestions`
- [x] **T3.5** Route `/ai/generate_code_review_content`
- [x] **T3.6** Per-feature model selection, independent of the agent model —
      `agents.local_ai.models.*`, each falling back to `agents.local_ai.model`

### Seam

Four one-line branches, one per method, not a decorator on
`ServerApiProvider::get_ai_client`. That seam looked narrower — one line, and
every consumer of `Arc<dyn AIClient>` goes through it — but it cannot work:
only one of the four methods is on `AIClient`. One is on `BlockClient` and two
are inherent methods on `ServerApi`. A decorator would also have meant ~500
lines delegating the other 68 `AIClient` methods, breaking on every upstream
trait change.

### Configuration reuses what already exists

No new secret storage and no new UI. `ai::api_keys::ApiKeyManager` already
holds Custom Inference endpoints (URL + key + protocol + models) in the OS
keychain, with an editor on the Warp Agent settings page. Upstream forwards
those to `api.warp.dev` so the *server* can call the provider; the fork uses
the identical configuration to call it directly.

`settings::LocalAiSettings` therefore holds no URL and no key — only which
endpoint to use and which model per feature. `settings.toml` is plaintext, and
a test asserts no key in the group can contain `key`/`token`/`secret`/
`password`/`url`, so adding a secret-shaped setting later fails loudly.

Resolution order, most explicit first: named Custom Inference endpoint → first
Custom Inference endpoint → pasted Anthropic key → OpenAI key → OpenRouter
key. A *named* endpoint that does not exist is an error, never a fall-through
to a different provider — that would send the payload somewhere the user did
not choose. Google is absent deliberately: the Gemini API is not OpenAI-shaped
at its documented endpoint, so a Google key needs an explicit endpoint entry
rather than a guessed compatibility route.

### Fail-closed, and why it matters more here than it looks

Under fork policy these four never reach `api.warp.dev`, even unconfigured.
The payloads are the reason: terminal output plus the command that produced
it, the working directory and recent shell history, and an entire working-tree
diff. `fork::account_gate_bypassed` (T1) makes the toggles reachable without
an account, so without this a fork user could switch one on and quietly resume
shipping exactly that upstream. An unconfigured endpoint surfaces as an error
naming the setting to fill in.

### Verified

- 72 new tests, all passing. Full suite 6500 passed / 19 failed against a
  measured same-session baseline of 6428 / 19 on the stashed tree — no T3
  regressions. The 19 are pre-existing (`gh`-dependent git tests, flaky
  secret-redaction globals, terminal view); the two secret-redaction entries
  differ run to run, which is what makes them flaky rather than broken.
- Request shape asserted on the wire against a `mockito` stub through the real
  `http_client`, with full-body equality for all three protocols. This is the
  failure mode that does not announce itself: a provider ignores a field name
  it does not recognise, so a wrong `max_tokens` would surface months later as
  answers that are mysteriously short, never as an error.
- Each of the four features round-tripped end to end through that stub,
  including a fenced-JSON reply (what a small local model actually returns).
- The runtime wiring driven through a real `App`: a key added to
  `ApiKeyManager` and a per-feature model edited in settings both reach
  `config::current` without a restart. Worth its own test because a missed
  subscription fails silently — it looks like a feature that works but needs a
  relaunch, which nobody reports as a bug. Settings groups *emit* their changed
  event without calling `notify`, so `observe` would never have fired.
- Windows: 64/64 module tests pass, `warp-oss` builds and launches, and
  `warpctrl instance list` reaches it — that command runs the whole
  broker → HTTP flow, so a bare listing is end-to-end evidence that startup
  completed. `config::install` runs before the first window, so a wrong
  registration order would have been an immediate panic rather than a subtle
  fault.

**Not verified: a real provider.** Doing so needs a key, and there is no local
LLM server on this machine to substitute one. Every field name and route here
was written from the protocol, and is asserted against a stub — but a stub
agrees with whatever it is told. One real request through a configured
endpoint is worth more than all of the above, and takes a minute:

1. Settings → Warp Agent → add a Custom Inference endpoint (or paste an
   Anthropic/OpenAI key).
2. In a repo with uncommitted changes, use Commit & PR generation.

`agents.local_ai.model` overrides the model if the default is wrong for the
endpoint; a bad model name comes back as the provider's own 404, naming it.

### Fields left empty on purpose

Two response fields cannot be filled honestly from the client and are empty
rather than fabricated. `AgentModeSuggestionV2.context_block_ids` needs block
IDs the request never sends — the server resolves those from its own copy of
the session — so Next Command offers command suggestions but no agent queries.
`Suggestion::Coding` needs file locations from a server-side codebase index;
without one it would carry no files and be discarded by
`is_valid_code_delegation` anyway, so prompt suggestions are always `Simple`.

### Known: a T1 consequence, surfacing as a test failure

`ai::request_usage_model::tests::test_byo_api_key_disabled_for_anonymous_firebase_user`
fails under fork policy and passes with `WARP_FORK_POLICY=0`. It asserts
upstream behaviour that `fork::account_gate_bypassed` deliberately inverts —
BYO keys stay disabled for anonymous users. Not T3-caused; it is in the
baseline. Recorded here because it was previously counted anonymously among
"the same failure families" and deserves a name.

## T4 — Local-first Warp Drive

Better shape than expected: a full local SQLite store already exists
(`crates/cloud_object_persistence`, diesel + bundled sqlite3). The server is a
**sync layer on top**, with `UpdateSource::{Server, Local}` already
distinguishing origins, plus a working offline mode and `ExportManager`.

So this is "keep the store, neutralize the sync" — not a rewrite.

- [x] **T4.1** Map the server-sync entry points — done, but the premise below
      was wrong. See "The map" and "Three blockers".
- [x] **T4.2** Local-only mode: full read/write, no account, no sync.
      Four seams, one per blocker below plus the guarantee. Not yet exercised
      in a running GUI — see "Not verified".
- [x] **T4.3** Offline read-only banner — answered by T4.1, no work needed.
      It is gated on `NetworkStatus::is_online()` (`drive/index.rs:2439`), not
      on auth, so a logged-out-but-online user never sees it. It is genuinely
      about the network, not the account. Under local-first it becomes a lie
      when the network *does* drop — nothing is read-only then either — so it
      wants suppressing, but that is one condition in T4.2, not its own item.
- [~] **T4.4** Git-backed sync — the store now has a portable on-disk form and
      a working tree, and the live store bridges to both. What is missing is a
      trigger and the write-back. See "T4.4 as built" below.
  - [x] **T4.4a** Lossless object↔file format — `drive/local_sync/format.rs`
  - [x] **T4.4b** Working-tree materializer — `drive/local_sync/tree.rs`
  - [x] **T4.4c** Round trip, replacing T4.5 — three levels of it, below
  - [ ] **T4.4d** Git operations, and something that invokes an export at all
  - [ ] **T4.4e** Conflict policy
  - [ ] **T4.4f** Apply an imported tree back into the store — new, and the
        remaining hard half; see "What is left, and why it is the hard half"
- [x] **T4.5** Round-trip via the existing import/export paths — **premise is
      wrong, same as T4.1's.** There is no round trip today: export and import
      do not even cover the same set of types, and neither carries identity.
      Replaced by T4.4c, which is done.

Explicitly **not** doing: Proton Drive. No general-purpose public API,
E2E-encrypted with client-side key management; integration means
rclone-shaped reverse engineering, trading a working local store for a
fragile sync target. Revisit only after T4.4 works.

### The map — T4.1 corrected

**There are no server-sync entry points in `cloud_object/model/persistence.rs`.**
That file is 1,838 lines of pure in-memory model: a `HashMap<ObjectUid, Box<dyn
CloudObject>>` plus accessors, with a `SyncSender<ModelEvent>` for SQLite
writes. It never holds a client and never issues a request. `UpdateSource::
{Server, Local}` lives there, but only as a tag on emitted events — nothing
branches on it to decide whether to talk to the network.

The sync layer is `server/cloud_objects/update_manager.rs` (4,833 lines) and
`server/sync_queue.rs` (1,988). Every local write follows the same three steps,
in this order, with no online/offline branch anywhere in them:

    1. update the in-memory CloudModel
    2. save_to_db(...)          -> SQLite, unconditional
    3. SyncQueue::enqueue(...)  -> server, eventually

Step 3 is the only server contact, and it is already decoupled: `enqueue` only
appends. Whether anything is *sent* is one bool, `SyncQueue::should_dequeue`
(`sync_queue.rs:348`), which starts `false` (`:384`).

So the sync is not something the fork has to sever. It is already severed when
logged out, and by exactly one line.

### It already doesn't sync. That is the problem.

`should_dequeue` is set true in exactly one place — `update_manager.rs:1071`,
at the end of `on_changed_objects_fetched`, i.e. only after a server fetch has
*succeeded*. That function is reachable only via `poll_for_updated_objects`,
which early-returns when logged out (`:688`), and polling itself only starts
when `TeamTesterStatus::initiate_data_pollers` fires — emitted from
`auth_manager.rs:449`, on user-fetched.

No account, therefore: no poll, no fetch, no dequeue. Reads and local writes
work; the SQLite store is loaded at startup (`lib.rs:2174`) with no auth check
at all. Nothing leaks.

But the same successful fetch is also the only thing that sets
`UpdateManager::has_initial_load` (`:1073`), and **24 call sites across 15
files `await` that condition** before doing their work. Logged out, they wait
forever. Confirmed consumers include:

    drive/index.rs:961          Warp Drive spinner never stops; sections
                                never initialize (has_initialized_sections)
    ai/agent_sdk/mcp.rs:31       `warp mcp list`
    ai/agent_sdk/profiles.rs:34  `warp profiles list`
    ai/agent_sdk/environment.rs  5 sites
    settings/cloud_preferences_syncer.rs:496, notebooks, env var collections,
    workflow_view, pane_group, workspace/view, docker_sandbox, privacy

**Correction, 2026-08-18, from running the binary.** The two CLI entries above
were previously written up as "never returns *because of this*". That
attribution is wrong. `warp mcp list` does hang forever, but it never reaches
the await: `command_requires_auth` returns `true` for `MCPCommand::List`
(`ai/agent_sdk/mod.rs:1575`), so `launch_command` errors out with "You are not
logged in" first — and then the process hangs anyway, because that error path
never terminates the app. Two separate faults, neither of them blocker 2. The
await is real and the other 22 sites are genuinely blocked by it; these two are
behind an earlier gate. `warp mcp list` reads local Drive objects and arguably
should not need an account at all, but that match arm also covers agent, run,
environment and memory commands which really do talk to Warp's server, so
opening it is its own decision and not part of T4.2.

The drive spinner is gated `show_warp_drive_loading_icon && is_online`
(`index.rs:2515`), so the visible symptom is precisely "logged out but online"
— which is the fork's normal state. Warp Drive looks perpetually loading while
the store underneath it is fully populated and writable.

This inverts the task. "Neutralize the sync" is done. The work is to stop the
app *waiting* for a sync that is never coming.

### Three blockers

1. **No owner.** `UserWorkspaces::personal_drive` (`user_workspaces.rs:979`)
   maps `AuthStateProvider::user_id()` to `Owner::User`, and returns `None`
   when unauthenticated. Every create path needs an `Owner` and every call site
   bails on `None`. So logged out you can read and edit, but cannot create
   anything. One function, ~20 call sites downstream of it — the narrowest
   seam in T4.

2. **`has_initial_load` never fires.** Above. The condition is
   interior-mutable (`reset_initial_load` takes `&self`), so it can be set from
   anywhere. Open question is *when*: at `UpdateManager::new` the SQLite load
   has already happened (`lib.rs:2174` precedes `:2289`), so the state it
   asserts is true — but auth restoration is async, and `auth_manager` only
   calls `reset_initial_load` for `!from_refresh`, so a restored session may
   not re-arm it. Settle this in T4.2 rather than assuming.

3. **Logout deletes the database.** `auth::log_out` (`auth/mod.rs:281`) calls
   `persistence::remove` — "so sessions and cloud objects don't persist between
   accounts" — then `CloudModel::reset()`. Upstream that is safe: the local
   store is a cache of server-owned objects. Once it is authoritative it is
   data loss, and it is reachable from a menu item. This one is not a feature
   gap, it is a hazard, and it did not exist before local-first made the store
   the original rather than the copy.

### T4.2 as built

The local identity is a **fixed sentinel**, `UserUid::new("local")`, not a
per-install UUID. `owner_to_space` files an object under `Space::Personal` only
when its owner equals the *current* user and under `Space::Shared` otherwise, so
a per-machine identity would put a store that moved machines into "Shared with
me" — exactly what T4.4 exists to do. It cannot collide with an account: real
Warp user ids are Firebase uids.

Four seams, all in the established additive style — no upstream behaviour is
deleted, and `WARP_FORK_POLICY=0` restores every one of them:

    fork::local_drive_owner          -> UserWorkspaces::personal_drive
                                        blocker 1: the drive becomes writable
    fork::local_drive_is_authoritative -> UpdateManager::new
                                        blocker 2: the SQLite store *is* the
                                        initial load, so nothing waits forever
                                     -> SyncQueue::enqueue
                                        the guarantee: refused at the door
    fork::local_drive_enabled        -> auth::log_out
                                        blocker 3: the store is not deleted
                                     -> drive::index render_all_sections
                                        T4.3: no false read-only banner

`local_drive_is_authoritative` is the auth-dependent half — fork policy *and*
no account. A fork user who does sign in gets upstream behaviour back, because
their objects then exist somewhere other than this machine.

Two things fell out for free rather than needing work. Objects with pending
changes already render as a laptop icon reading **"Saved locally"** rather than
a spinner: upstream's condition is `has_in_flight_requests &&
!sync_queue_is_dequeueing`, and under local-first the queue never dequeues, so
the correct indicator was already the one that shows. And the Warp Drive
spinner needed no separate fix — it is gated on the same initial-load condition
as everything else, so blocker 2 turned it off.

The enqueue refusal is what turns "does not sync" from an ordering accident
into a property. Upstream already never *sends* while logged out, but the item
survives in the queue, and `lib.rs` reseeds the queue at startup from every
object with pending changes — so the first time an account was added, locally
owned objects would have been pushed under a uid the server has never heard of.
Both paths are now closed, the startup one by owner rather than by auth, since
by then a real account may legitimately be present.

### Verified

- 9 new tests. Each seam is asserted in both directions — logged out *and*
  signed in — because a guard that never turns off would silently break a fork
  user who does log in, and that failure would look like a Warp bug.
- 14 new tests. Full suite **6512 passed / 21 failed** on the final run,
  against a same-session baseline of **6500 / 19** measured by stashing this
  work. Total count rises by exactly 14, matching the new tests. The failure
  delta is the two inversions below plus the flaky set, which varies run to
  run — consecutive runs gave 22, 20 and 21.
- Newly observed in that flaky set:
  `server::cloud_objects::update_manager::tests::
  test_pending_metadata_update_with_polling`. It is in a module this work
  touches, so it was checked rather than assumed: passes 3/3 in isolation and
  under `WARP_FORK_POLICY=0`, and only fails under parallel load. A polling
  timeout, not a regression. Recorded because "it's probably flaky" is exactly
  the reasoning that hides a real fault.
- `cargo clippy -p warp --lib --all-targets` clean; `cargo fmt --check` clean
  for every file touched here.

**Caught by the test suite, and worth recording because it was mine:** the
first version of the `WARP_FORK_POLICY=0` test set and unset the variable
around its assertion. `std::env` is process-wide and the suite runs in
parallel, so it re-enabled fork policy mid-run for whatever happened to be
executing alongside it — and made a `WARP_FORK_POLICY=0` baseline run report
6510/18 instead of the truth. It presented as unrelated tests failing, which is
the expensive kind of wrong. The policy-off path is covered by running the
whole suite with the variable set, which is the real check anyway.

### Verified on Windows, 2026-08-18

Driven from WSL over `powershell.exe`; see `.fork/README.md` "Driving the
Windows build from WSL" for the mechanics.

    surface warp-drive open   ok: true
    screenshot                Warp Drive renders: PERSONAL space with a `+`,
                              MCP Servers, Rules, TRASH. No spinner.

**Warp Drive renders its contents instead of a perpetual spinner with no
account.** That is the claim T4.2 existed to make, and it holds. The instance
was genuinely account-free — the binary said so itself on another path ("You
are not logged in").

The store path is now known rather than guessed:

    %LOCALAPPDATA%\warp\WarpOss\data\warp.sqlite

- [x] **T4.6** A created object survives a restart — **verified 2026-08-18**,
      and it found a bug. A workflow `simple-workflow-test` was created by hand
      in the GUI with no account (the `+` cannot be reached by scripting; see
      T1.12). In SQLite it is exactly what the design predicts:

          object_type  WORKFLOW
          server_id    <empty>        never synced
          client_id    Client-56cd792e-...
          is_pending   1              -> renders "Saved locally", not a spinner
          subject_uid  local          the sentinel, written by personal_drive

      After a full close-and-relaunch it is still there, editable, with its
      `wf-test` alias intact. The alias lives outside `workflows.data`, which
      is why the payload column does not mention it.

- [ ] **T1.12** Add Warp Drive object actions to the local-control catalog.
      Surfaced by T4.2 verification: the catalog can drive every part of the
      app *except* its object store, which makes exactly the fork's own
      headline feature the one thing an agent cannot exercise. 85 actions
      across app, window, tab, pane, session, input, surface, setting, theme,
      appearance, keybinding and file, and nothing that creates a workflow,
      rule or folder. `input.*` writes to the terminal's input editor rather
      than to whatever UI has focus, so the `+` button is unreachable. Same
      shape as the `setting.get/set` allowlist gap recorded under T2.

### The bug T4.6 caught, and why nothing else could have

The restored workflow came back filed under **"Shared with me"** — an object
this client had created itself one restart earlier.

T4.2 taught `personal_drive` to *write* the local sentinel as owner, but left
`owner_to_space` *reading* `AuthStateProvider::user_id()` directly. Signed in,
those two agree. Account-free they do not: `user_id()` is `None` while the
sentinel is not, so `Some(uid) == None` was false for every locally-created
object and all of them read as somebody else's.

Both sides now resolve through `personal_drive`, which is already the seam that
answers "who am I". For a signed-in user the two forms are identical by
construction, since `personal_drive` is `Owner::User { user_uid: <current> }`.

**Every unit test passed with this bug in place**, and they were not bad tests —
they covered the writing side and the reading side, separately and correctly.
The defect lived in the agreement between them, which is not a place a unit
test naturally looks. It took creating a real object and restarting a real
window. Worth remembering the next time a change looks fully covered: a seam
that is correct at both ends can still be wrong in the middle.

### Known: two T4.2 consequences, surfacing as test failures

`ai::execution_profiles::profiles::tests::
auth_completion_waits_for_cloud_initial_load_before_migrating` fails under fork
policy and passes with `WARP_FORK_POLICY=0` — A/B'd, not assumed. Same category
as the T1 entry above it, and the second such inversion in the fork.

It asserts that legacy execution profiles do not migrate until cloud objects
arrive. Under local-first they migrate at startup instead, because the local
store *is* the load. For a fork user that is the only behaviour that works at
all — waiting for a fetch that never comes means legacy profiles never migrate.
For someone who launches logged out and then signs in, local profiles migrate
first and the server's merge in afterwards via `CloudModelEvent::
InitialLoadCompleted`, which `profiles.rs` already subscribes to. Both sets
survive; the test is asserting the intermediate state, and that state is
genuinely different now.

`workspace::view::tests::
test_tools_panel_preferences_activate_after_signup_and_ai_enablement` fails the
same way and for the same kind of reason, added by the drive-availability fix
below. It asserts the left panel reports `RequiresAccount` for Warp Drive
before signup; under fork policy it reports `Available`, because it is. A/B'd
against `WARP_FORK_POLICY=0`, which passes.

Three inversions in the fork now, all of the same shape: a test pinning
upstream's "this needs an account" premise, which is the premise the fork
exists to remove. Worth watching as a count — if it keeps climbing, the fork is
diverging faster than the seam design intends.

## T4.4 scope — git-backed sync

Scoped 2026-08-18. Not started.

### Most of the machinery is already here

- **`git2` 0.20.4 is already an `app` dependency**, `vendored-libgit2`, so it
  builds without a system libgit2. Already used in three places, including
  `Repository::discover` in `workflows/local_workflows.rs:183`. Like T3 and
  T1.9, this can be done with **zero new dependencies**.
- **Warp already reads workflows out of a git repository.** `WorkflowSource`
  has eight variants, two of which are file-based and have nothing to do with
  Warp Drive: `Project` loads `.warp/workflows/*.yaml` from the discovered git
  worktree, and `Local` loads the user's home workflow directory. So "workflows
  in a git repo" is an upstream feature, not something the fork invents.
- **Live reload already works.** `WarpManagedPathsWatcher` watches the config
  directories and `user_config::native` reloads themes, workflows, launch
  configs and model routers on change. `git pull` is picked up without a
  restart, for free, on whatever paths are wired into it.
- **A complete, type-generic serializer already exists.** `CloudObject::
  serialized() -> SerializedModel` (`cloud_object/mod.rs:497`) is implemented
  for every object type — it is what gets sent to the server and stored. JSON
  types go through `serde_json::to_string` (`json_model.rs:24`).

Useful detail: `load_project_workflows` uses the `WARP_CONFIG_DIR` **constant**
(`.warp`), not `base_warp_config_dir_name()`. So repo workflows live in `.warp`
on every channel, while the *home* config dir is channel-suffixed — `.warp-oss`
for this fork's build. Repo-relative paths are therefore already portable
between the fork and stock Warp; home-relative ones are not.

### What is actually missing

Not git. **A lossless file representation of a cloud object.** The existing
export/import paths are a sharing feature and cannot be reused as a
serialization layer, which is also why T4.5's premise is wrong:

    export  Workflow -> .yaml   Notebook -> .md   EnvVarCollection -> .env
            everything else: `anyhow::bail!("exporting {other:?} not yet supported")`

    import  .md -> Notebook     .yaml/.yml -> Workflow (+ enums)   dirs -> Folders
            nothing else

The two sets are not even the same: export writes env var collections that
import cannot read, and import creates folders that export only expresses as
directory names. Neither carries the object's identity, its folder placement,
its trash state or its timestamps — export serializes `model().data` and
nothing else. So a round trip through them loses the object graph and mints new
ids on the way back in. Ten or so types have no representation at all: AI
facts, MCP servers, templatable MCP servers, execution profiles, workflow
enums, cloud preferences, ambient agent environments, scheduled agents, cloud
agent configs.

The store itself is far more tractable than that suggests. There are only
**four payload shapes** plus one shared spine:

    object_metadata          the spine — type, server_id/client_id, folder_id,
                             trashed_ts, timestamps, creator/editor uids
    object_permissions       owner (subject_uid/subject_type), guests, links
    workflows                data: Text
    notebooks                title, data, ai_document_id
    folders                  name, is_open, is_warp_pack
    generic_string_objects   data: Text   <- all ten JSON types land here

So one file format plus a metadata sidecar covers everything, and the ten
"unsupported" types are collectively a single case.

### Decomposition

- **T4.4a** Lossless object↔file format. The real work. One file per object,
  carrying identity, folder path, and payload. Must survive a round trip
  byte-for-byte on unchanged objects, or every `git status` is dirty.
- **T4.4b** Working-tree materializer: write the whole store to a directory,
  read it back. Folder hierarchy as directories, so the tree is browsable and
  diffs are legible.
- **T4.4c** Round trip, replacing T4.5: materialize → mutate on disk → reload →
  assert the object graph is identical, ids included.
- **T4.4d** Git operations. Thin, given git2 is present.
- **T4.4e** Conflict policy. Cheap or expensive depending on decision 1 below.

### Decisions to settle before starting

1. **Who drives git — Warp, or you?** If Warp auto-commits and pulls, it needs
   a merge and conflict story for a graph of objects with ids, which is a sync
   engine and is where this task's risk actually lives. If Warp only reads and
   writes a directory and *you* run git, conflicts are text conflicts in your
   own repo, T4.4d and T4.4e nearly vanish, and the existing file watcher
   already handles the pull side.
2. **Is the working tree authoritative, or a mirror of SQLite?** T4.2 just made
   SQLite the source of truth. Two sources of truth needs reconciliation; a
   mirror needs a rule for which side wins on divergence.
3. **Does this extend Warp Drive, or the existing `WorkflowSource::Project`
   path?** The second is a much smaller change and already git-native, but it
   is a parallel store — workflows would live in two places, which is the
   confusion upstream already has and the fork would be doubling down on.

Recommendation: user-driven git (1), working tree as a materialized mirror with
SQLite authoritative (2), extending Warp Drive rather than the project path
(3). That keeps the sync engine out of the fork entirely — git is the sync —
and leaves T4.4a as the only substantial piece of work.

**All three settled as recommended, 2026-08-18.**

## T4.4 as built

Three files under `app/src/drive/local_sync/`, 31 tests, zero new
dependencies. The layering is deliberate: `format` and `tree` know nothing
about the app, so their tests are real rather than mock-shaped, and `snapshot`
is the only file that knows about both sides.

    format.rs     one object <-> one file
    tree.rs       one drive <-> one directory
    snapshot.rs   CloudModel  -> the above

### What the format carries, and what it refuses to

A file carries identity, content and content-level metadata. It deliberately
drops:

| dropped | why |
| --- | --- |
| `id`, `shareable_object_id`, `author_id` | per-machine integers, meaningless in another checkout |
| `is_pending`, `retry_count`, `current_editor` | state of a server conversation this fork does not have |
| `folders.is_open` | sidebar view state — expanding a folder would dirty the repo |
| the parent folder id | placement *is* the path, so a move is a rename git can follow |
| `notebooks.conversation_id` | names a conversation on Warp's server; SQLite has no column for it either |

The `is_open` and `folder_id` exclusions are the two that matter. Both are
about the property the whole thing rests on: **an object that has not changed
must produce the bytes it produced last time**, or `git status` is permanently
dirty and the repository is useless as a sync target. `is_open` would break
that on every sidebar click. `folder_id` would not break it, but it would be a
second representation of placement, and two representations of one fact are
two things that can disagree — which is precisely the shape of the bug T4.6
caught.

### Two envelopes, one header

    notebook          <slug>-<hash>.md      YAML front matter + markdown body
    everything else   <slug>-<hash>.json    one JSON object, payload under "data"
    folder            <dir>/.warp-folder.json

Notebooks are prose and belong in a file a diff can read. Everything else is
JSON `serde_json` already produced, and is **re-emitted rather than converted
to YAML**: prettier diffs are not worth a format in which a workflow argument
named `on` or `no` comes back as a boolean. `serde_json`'s maps are `BTreeMap`,
so keys sort and the bytes are stable — pinned by a test, because a workspace
that ever enabled `preserve_order` would silently make byte-stability depend on
hash iteration order.

The filename hash is not decoration. Without it two objects named "deploy"
collide, and disambiguating against siblings would make one object's filename
depend on another's existence — so creating a second "deploy" would rename the
first and churn the repo. The hash makes the name a pure function of the
object.

Ten object types that upstream's export cannot represent at all — AI facts,
MCP servers, execution profiles, cloud preferences, scheduled agents and the
rest — collapse into a single case, because they share one payload column.

### Reading a payload out of a `dyn CloudObject`

There is no accessor for it, and there cannot be a simple one: `CloudObject` is
object-safe and non-generic because `CloudModel` stores its objects as trait
objects, so the model is only reachable by downcasting to the concrete
`GenericCloudObject<K, M>` — thirteen downcasts and a list to maintain by hand.

`update_object_queue_item` is the way through. It is object-safe, it is a pure
constructor that delegates to the model, and every object type has exactly one
`Update*` variant carrying its typed model. One `match` covers all thirteen,
and a new type upstream fails to compile here rather than silently exporting
nothing. Nothing is enqueued — the item is constructed, read and dropped.

### Writing into a directory the user owns

The export target is a repository the user keeps their own things in. An
exporter that treats it as its own is a data-loss bug, not a sync feature, so
the pruning rule is timid: a file is deleted only after it has been read and
**recognised as one this exporter wrote**, a directory only once it is empty,
and dot-directories are never entered. The test for this exports a drive into
a directory holding a README, a `.git`, and the user's own notes, then exports
an *empty* drive over it — the most destructive thing a caller can ask for —
and asserts every one of the user's files is still there.

Trashed objects are exported, with their timestamp. Dropping them would make
an export quietly destructive: emptying the trash is the user's decision, and
an export that pre-empted it would take the undo away.

### Three levels of round trip

Deliberately three, because two correct halves that disagree in the middle is
exactly how T4.6's bug survived a green suite:

1. `format` — one object through one file's bytes and back
2. `tree` — a drive with nested folders through a directory and back
3. `snapshot` — the **live store** through the bridge, onto a disk, and back

Only the third spans the seam between the other two.

### What is left, and why it is the hard half

- **T4.4d** — nothing invokes an export yet. Needs a repository path setting
  and a trigger. The natural trigger is a local-control action, which folds in
  T1.12 and makes the whole feature drivable and verifiable without clicking;
  it costs a catalog entry, a handler and permission wiring.
- **T4.4f** — applying an imported tree *back into* the store. `snapshot` only
  reads. Writing means creating and updating objects through `CloudModel`'s
  typed paths, which is thirteen constructors rather than thirteen accessors,
  and it has to reconcile against what is already there rather than replacing
  it. This is where the remaining risk lives.
- **T4.4e** — conflict policy, which is nearly free given decision 1: a
  conflict is a text conflict in the user's own repo. What is *not* free is
  what Warp does when it reads a file with conflict markers in it; right now
  that is "ignored, with a reason", which is defensible but should be a
  deliberate choice rather than a side effect.

Two things the tests caught that reading would not have: serde_yaml 0.8 opens
its output with a document-start marker, which is the same three characters as
the front-matter fence and becomes the *closing* fence on read; and
`user:local` contains a colon, so YAML quotes it.

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
