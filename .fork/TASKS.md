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
- [x] **T1.7** Document the verified command surface in `.fork/README.md` —
      done by running all 88 actions rather than by reading the catalog. See
      "T1.7 as built" below; it corrected the count, the namespace list and the
      focus rule, all three of which were wrong.

Confirmed 2026-08-18: closing the window with `CloseMainWindow` removes the
discovery record, and `instance list` immediately reports none. So the stale
records that produced `ambiguous_instance` during T2 came specifically from
`Kill()`ing the process, not from ordinary shutdown — the cleanup path works,
it just never runs when the process is killed.

### T1.7 as built — the surface was documented by running it

Verified 2026-08-19 by executing **all 88 actions** against the live Windows
build, one at a time, appending each result to a file before the next call so
that if the app died the last line would name the action that killed it.
Nothing killed it.

Three things in the existing documentation were wrong, and only running it
would have found any of them:

* **The count.** The README said 85 and this board said 84. It is 88, and the
  arithmetic reconciles exactly: T1.5a counted 84 on 2026-08-17, then T1.8
  added `input.submit` and T4.4 added `drive.sync.{status,export,import}`. So
  the fork's own additions were the drift, and nobody had come back to count.
  The namespace list was also missing `instance`, `capability` and `action`.
  (The "other 85 actions" phrasing elsewhere on this board and in
  `drive_sync.rs` is still right: 88 minus the three `drive` actions.)
* **The focus rule.** "Mutations need a focused window; `app focus` first" is
  backwards on both halves. `app focus` returns `ok: true` and cannot raise the
  window at all from WSL — Windows' foreground lock forbids a background
  process doing that — yet creating tabs, splitting panes, submitting input,
  settings, themes, appearance, surfaces and the whole `drive` namespace all
  work with `is_active: false`. What actually fails is any action left to
  resolve *the active* target, and the fix is a selector, not focus.
* **`--window <id>` is the fix for all of it.** Every `missing_target` in the
  sweep cleared by naming the window, because everything else resolves inside
  one — `tab inspect --tab-index 0` alone means nothing without a window to
  count within. With `--window` present, ids and indexes are interchangeable.

  Worth recording how nearly this went in wrong: the first pass had
  `pane focus --pane <id>` and `session activate --session <id>` down as
  broken, on two reproducible `stale_target` results, and a table saying to use
  indexes for those two. Re-testing before publishing showed both work — the
  earlier failures were a *closed pane and a Settings tab left active by the
  sweep itself*, i.e. the ids really were stale and the error was exactly
  right. A failure observed twice is still not a property of the surface.

Two state preconditions worth the words in the README: `input.*` needs the
active tab to be a terminal (opening the settings surface silently breaks
every subsequent `input` call until `tab activate` puts a terminal back), and
`surface.code_review.open` needs that terminal to be in a repository — it
answered `target_state_conflict` until `input submit 'cd C:\dev\warp'`, then
succeeded.

**What is deliberately not claimed.** The window could not be focused for these
runs, so no action was tested through the "active target" default path; every
verification used an explicit selector. Making the window foreground from WSL
means defeating the foreground lock, which is not something to do on a desktop
the user is sitting at — the permission classifier refused it, correctly.

One event that looked alarming and was not: mid-session the running build
exited, its log ending in `NativeModalAction::TriggerButtonCallback(0)` after
starting as a crash-recovery child. Explained by the user immediately
afterwards — they had closed the debug terminal window, which spawns the
recovery child, and then clicked "yes, exit Warp" on the confirmation dialog.
So the button callback in the log is exactly what it says: a person clicking a
button. Worth keeping only as a reminder that a `TriggerButtonCallback` in this
log is a human, not a fault.

Driven by `C:\dev\sweep.ps1`, which is worth keeping: re-running it after an
upstream merge is the cheapest way to find out what the merge broke.

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

### RESOLVED 2026-08-19 — there was no rendering bug (T1.11)

The record above said: "the window object exists but never gets a workspace
because it never composites under WSLg." Every word after "exists" was wrong.

It composites. The Linux build renders the whole UI correctly under WSLg with
the two documented environment tweaks, and always did. What it was showing was
the **onboarding slides**, and while those are up `RootView` sits in
`AuthOnboardingState::Onboarding` — `Workspace` is built by the *other* branch
(`root_view.rs:1925`). So `has_workspace: false` was not a symptom of a
graphics failure at all. It was the app truthfully reporting that a fresh
profile had not been through onboarding yet.

Completing it — three slides, then **Skip → "Skip for now"** on the account
slide — produced, in order:

    window list     has_workspace: true   (was false for weeks)
    tab create      Created tab 2144 in window 0 (tab count 2)
    input submit    the command ran; /tmp/t111-proof.txt written

Then rebuilt at HEAD and relaunched, because the binary that found this was
from 2026-08-17 and the point is whether *current* code is usable:

    window list     has_workspace: true straight from launch — onboarding is
                    persisted, so the workspace is what you get
    tab create      Created tab 2764 in window 0 (tab count 3)
    input submit    ok, executed: false, queued: true, and the file appeared
                    — the T1.9 fix behaving correctly here too; the old binary
                    reported `isError` for this same case
    drive status    answers (0 objects: the Linux profile has its own store)

Cost of software rendering, measured rather than assumed: **0% CPU at idle**,
peaking around 280% of one core while painting 50,000 lines of scrollback and
back to zero within two seconds. An earlier 25% reading was a cargo build on
the same machine, not llvmpipe.

**How it hid for so long.** There *is* a real WSLg rendering failure, and it is
already documented: with `WAYLAND_DISPLAY` set the window is created but never
paints. That is a grey rectangle, and it is genuinely broken. The X11 fallback
fixes it. But the `has_workspace: false` symptom looks identical either way, so
after the switch to X11 it kept being read as the same problem. Nobody
screenshotted the X11 window — which shows a perfectly rendered "Welcome to
Warp" — because the diagnosis was already written down.

The generalisable bit: **a symptom that survives the fix for its supposed cause
is evidence the cause was wrong**, not evidence the fix was incomplete.

- [x] **T1.11** The Linux build is usable. No code changed — the fix was
      finishing a flow, and the honest deliverable is that the blocker was a
      misdiagnosis. See above and `.fork/README.md`.

**The trap next door, not fixed.** Under account-first onboarding,
`mark_local_onboarding_completed` is called only from `complete_account_first`
(`root_view.rs:2690` skips it when `account_first`), so the flag is written
only if the user reaches the end of the account slide — including via Skip.
Quit while that slide is up and the entire sequence returns on the next launch,
forever, which reads as "the app never finishes starting". Not a wall, since
Skip works, but it is an account-shaped papercut in a fork whose premise is
that there is no account. One condition under fork policy would remove it;
worth doing only if it starts biting, and named here so it is a decision rather
than an oversight.

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
- [x] **T4.4** Git-backed sync — the mirror is two-way and drivable, it refuses
      to act on a half-merged tree, and workflow aliases travel with their
      workflows. See "T4.4 as built" below.
  - [x] **T4.4a** Lossless object↔file format — `drive/local_sync/format.rs`
  - [x] **T4.4b** Working-tree materializer — `drive/local_sync/tree.rs`
  - [x] **T4.4c** Round trip, replacing T4.5 — three levels of it, below
  - [x] **T4.4d** A trigger — `drive.sync.status` and `drive.sync.export`,
        verified on the Windows build against a real git repository
  - [x] **T4.4e** Conflict policy — both directions refuse a half-merged tree
        rather than reading it as a deletion; see "T4.4e as built" below
  - [x] **T4.4f** Apply an imported tree back into the store — done and
        verified live; see "T4.4f as built" below
  - [x] **T4.4g** Workflow aliases travel inside their workflow's file — found
        by the live run; see "The alias gap" and "T4.4g as built" below
- [x] **T4.7** Deleting a Warp Drive object without an account — the whole
      lifecycle: trash, restore, delete forever, empty trash. See "Two things
      that were never possible without an account" and "T4.7 as built" below.
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

### T4.4d as built — the trigger

Two actions, `drive.sync.status` and `drive.sync.export`, bringing the catalog
to 87. Namespaced `drive.sync.*` rather than `drive.*` because upstream retired
a whole `drive.*` group and pins the old names as unparseable in
`malformed_and_removed_action_names_are_not_deserialized`; these are not a
revival of those. This also closes T1.12 — none of the other 85 actions touch
the object store.

**The destination is a setting, not a parameter, and that is a security
property rather than a convenience.** An export prunes. If the destination
arrived with the request, anything that could reach local control could aim a
pruning exporter at a directory of its choosing. `warp_drive.local_sync.path`
is also deliberately **not** in `ALLOWLISTED_SETTING_KEYS`, so `setting.set`
cannot repoint it either: an agent can ask for an export but cannot decide
where it lands. Pinned by a test, because adding one line to that allowlist
would undo the whole argument without touching the drive code.

Guards, each naming itself in the error: unset, relative, filesystem root, and
not-a-directory are refused before anything is read or written. A mistyped `/`
would otherwise walk the entire filesystem reading every file to decide whether
it was one of ours. `WARP_FORK_POLICY=0` refuses too — the catalog is a
compile-time list so the action cannot vanish from it, but an action that
deletes files should stop working when its policy is off.

### Verified on Windows, 2026-08-18

Built at 21:09, run against the real store containing the `simple-workflow-test`
workflow created by hand in the previous session:

    drive status     objects 1, path_exists false
    drive export     written 1
    drive export     written 0, unchanged 1     <- the property, live

The file is `simple-workflow-test-8f89f76f.json`, carrying
`uid: Client-56cd792e-...` — the same client id recorded in the SQLite
inspection two sessions ago, so identity really does survive the store → file
boundary.

Then the destructive case, against an actual git repository rather than a
tempdir: `git init` in the mirror, add a README, a `notes.json`, and a
`my-notes/todo.md`, commit, and export twice more.

    removed_files 0        nothing of the user's was touched
    git status --porcelain (empty)      the repository is clean
    .git\HEAD present

That last line is the one worth having. The unit tests assert the same thing
against a tempdir, but "an export leaves a real git repository clean" is the
claim the whole format was designed around, and until now it had only ever been
checked against a directory this code also created.

### T4.4f as built — the mirror is two-way

`drive.sync.import` reads the configured directory into the live store, so a
`git pull` reaches Warp Drive. Thirteen constructors where `snapshot` was
thirteen accessors, but only three bodies: the ten JSON types share a payload
column and therefore share a deserializer.

**An object missing from the tree is trashed, not deleted.** Both alternatives
are wrong, and the reasoning is the load-bearing part of the design:

- *Ignore it* and deletions never propagate. Delete a workflow on machine A,
  pull on B, and B's next export puts the file straight back. The two machines
  resurrect each other's deletions forever.
- *Delete it* and one import against the wrong directory destroys the drive
  with no undo.

Trashing composes with the format instead. A trashed object still exports,
carrying its `trashed` timestamp, so "I deleted this" travels as **content**
rather than as absence. Absence therefore means something stronger — the trash
was emptied — and echoing that as a local trash is the recoverable reading of
it.

The tree wins: no revision comparison, no merge. The moment this starts
deciding which side is newer it is a sync engine, which decision 1 exists to
avoid.

`is_open` is preserved from whatever the machine already had rather than
decided by the import, since it is sidebar state the format deliberately omits.

An empty tree is refused. Pointed at the wrong directory it would read as
"everything was deleted" and trash the whole drive in one call, and a genuinely
empty drive is not distinguishable from a wrong path.

**Consequence worth knowing:** with a single-object drive, deleting that object
and emptying the trash produces an empty tree, which the guard refuses. So the
very last deletion cannot propagate. The safety trade is deliberate, but it is
a real edge and not a theoretical one.

#### Verified on Windows, 2026-08-19

Against the same real store, through the action surface:

    export                     unchanged 1
    (edit the file)
    import                     updated 1
    export                     unchanged 1     <- store and file now agree

    (hand-author a new file)
    import                     created 1
    (delete that file)
    import                     trashed 1
    export                     the object still exports, carrying
                               "trashed": "2026-08-19T03:54:17.595874Z"

    (move the drive files aside)
    import                     invalid_request: refusing to import from a tree
                               with no Warp Drive objects in it

The import also reported the user's own `README.md`, `notes.json` and
`my-notes/todo.md` as ignored, each with the reason — so files that are not
ours are visible rather than silently skipped.

One thing the first attempt at this got wrong, worth recording: copying a file
aside before deleting it produced `unchanged`, not `trashed`. That is correct —
identity is in the header, so a rename is not a delete — but it meant the test
proved nothing until it was redone without the copy.

### Two things that were never possible without an account — T4.7 (both fixed)

Found while designing T4.4f's deletion rule, by reading the path it depends on.

**Trashing — fixed.** `UpdateManager::trash_object` opens with
`let Some(server_id) = id.server_id() else { return; }`. Account-free no object
has a server id, so the Drive panel's Trash item, `WorkflowAction::Trash` and
the workflow modal's delete all silently did nothing. Worse if it had got past
that gate: the local `trashed_ts` is set optimistically and **reverted** when
the request fails, and without credentials it always fails — so there was no
ordering in which the upstream path worked here. Now routed through
`fork::drive_deletes_are_local`, and pinned by a test that was confirmed to
fail with the guard disabled.

**Permanent deletion — fixed, see below.** `UpdateManager::empty_trash` is a
bare server call: it asks `object_client.empty_trash(owner)` and only removes
anything locally on success. Account-free that request cannot succeed, so
emptying the trash did nothing — a trashed object could not be got rid of at
all.

### T4.7 as built — the trash is a place things can leave

Four verbs, not two. The recorded scope was `empty_trash`; reading the path
found the other three, and the last of them is the one that mattered most.

**`empty_trash` and `delete_object_with_initiated_by` are the same bug.** Both
ask the server and only touch anything locally once an answer arrives. The
local half already exists and is already correct — `on_object_delete_success`
does the model, the objects' actions and the SQLite rows — so what is missing
account-free is *only the list of ids the server would have replied with*, and
that list can simply be read: the objects in this space carrying a `trashed_ts`
are what the trash is.

**Descendants have to be walked, not listed.** Trashing a folder marks only the
folder; its contents carry no `trashed_ts` of their own. Delete the trashed set
alone and everything inside a deleted folder is left behind — in memory and in
SQLite — pointing at a parent that no longer exists. The server's reply
includes descendants, which is why upstream never has to think about this.

**The fix had to reach the view, or nothing could have called it.** The Drive
panel gates its trash context menu on `online_only_operation_allowed`, which
requires `has_server_id()`. Account-free that is never true, so "Restore" and
"Delete forever" were never *drawn* on a trashed object. Fixing the update
manager alone would have left both fixed and unreachable.

**Which made restore part of this task.** Exposing "Restore" without fixing
`untrash_object` — same server-id guard — would have been worse than leaving it
hidden. And it is load-bearing beyond the menu: T4.4f's safety argument is that
an object missing from the tree is *trashed rather than deleted, because
trashing is recoverable*, which was not true here. A trash you cannot restore
from is a delete with extra steps.

Restoring moves an object to the root when its folder is itself in the trash,
because restoring into a trashed folder restores it *into* the trash, where the
user cannot see it and has no way to find out where it went. Not an invention:
upstream's `test_metadata_after_untrash_item_and_move_to_root` asserts the
server answers exactly this way. With no server, the client decides it.

One predicate for all four verbs, `fork::drive_deletes_are_local`, because they
are one question — does removing an object need permission from somewhere else?
Answering it per-verb is how a trash you can fill but not empty comes about,
which is the state the fork was in between T4.4f and T4.7.

**Known limits, both narrow.** A local delete's completion event carries
`server_id: None`, because `ServerId::from_string_lossy` asserts 22 characters
and a client uid is 43 — it panics rather than lying, which the first version
of this found the hard way. So the two listeners keyed on `server_id` — the
environments page's success toast and `ambient_agents::scheduled`'s completion
channel — stay silent for a local object. Neither was reachable before, since
the delete they wait on never happened; `scheduled`'s waiter hangs either way,
which is its own upstream bug and not this one.

And T4.4f's empty-tree guard is now reachable in earnest: on a single-object
drive, deleting the object *and* emptying the trash produces an empty tree,
which the import refuses. The last deletion still cannot propagate.

#### Verified on Windows, 2026-08-19

Through the panel, on the running build, against the very object that could not
be got rid of — `from-another-machine`, trashed since the T4.4f session.

The first thing to check was the menu, since before this it had no entries at
all on a trashed object. Right-click now draws **Restore** and **Delete
forever**.

    Restore          -> trash empties, object back under PERSONAL,
                        "Empty trash" greys out
    drive export     -> written 1, unchanged 1
                        and the file no longer carries "trashed"

    Trash it again, then Delete forever
                     -> toast "1 object deleted forever"
    drive status     -> objects 2 -> 1
    drive export     -> removed_files 1

    Import a hand-authored throwaway (created 1), trash it,
    then the Empty trash button
                     -> confirmation dialog, then
                        "Trash emptied: 1 object deleted forever"
    drive export     -> removed_files 1

Then closed and reopened Warp: `objects 1`, export `unchanged 1`. Nothing came
back, which is the part only a restart can show — the in-memory model and the
panel would look identical either way, and the SQLite delete is what makes it
permanent.

The toasts are worth noting rather than skipping past: they are driven by the
completion event, so seeing them is what confirms the local path emits it. The
`drive export` numbers are the independent check — `removed_files 1` means the
object was gone from the *store*, not just from the panel.

Driven with a new `C:\dev\click.ps1` (see `.fork/README.md`), because the trash
menu has no `warpctrl` action and no keybinding: this is the first fork
behaviour that could only be reached through the GUI.

### The alias gap — T4.4g

The live run found something reading would not have. The exported workflow has
no alias, and `wf-test` was set on it.

Not a defect in the format: **workflow aliases are not drive objects at all.**
`WorkflowAliases` (`workflows/aliases.rs`) is a *settings group* — a
`Vec<WorkflowAlias>` under storage key `WorkflowAliases`, each entry holding an
`alias` string and the `workflow_id: SyncId` it points at. So the export is
lossless with respect to the drive; the alias was never in it.

It is still a real gap for what T4.4 is *for*. Carry the repository to another
machine, import, and the workflow returns without its alias — because aliases
live in settings, which in this fork sync nowhere.

Tractable, and the format already did the hard part: aliases reference
workflows by `SyncId`, and a `SyncId` is exactly what the files preserve. So
the link would survive if the aliases travelled. What needs deciding is whether
settings-shaped data belongs in a *drive* mirror at all, and what happens to an
alias pointing at a workflow that is not in the personal space. That is a
design call, not a line of code, which is why it is its own task.

### T4.4g as built — the alias travels in the workflow's file

**Where it goes was the whole question.** The obvious answer is a side-car:
mirror the settings group as a top-level `.warp-aliases.json`. Everything that
makes the rest of this format work argues against it. Placement is the path,
identity is in the file, deleting the file deletes the thing — a list of ids has
none of those properties. Delete a workflow's file and the side-car entry is
left pointing at nothing; and a list of ids is exactly the shape a diff cannot
review.

Carried in the workflow's own file, an alias moves when the workflow moves, dies
when it dies, and **cannot dangle, because there is nowhere for it to dangle
from**. The cost is that `PortableObject` is now a join of two sources rather
than a projection of one object's rows — done in `snapshot`, which is the layer
whose entire job is bridging the store to this form.

**The import rule is deliberately unlike the object rule**, and this is the part
worth reading twice. Objects read absence as deletion, which works *because* a
deleted object still exports as a trashed one — absence therefore means
something specific. An alias has no such tombstone: an alias that is gone is
just gone. So absence is only read **within the workflows the tree describes**.
An alias pointing anywhere else — a team workflow, an object outside the mirror
— is left completely alone, because reconciling the whole list against the tree
would wipe it with nothing anywhere to restore it from.

Three smaller decisions, each because the alternative breaks byte stability or
loses data:

- **`arguments` becomes a `BTreeMap`.** The setting holds a `HashMap`, and
  `serde_json` writes a map in iteration order — which for a `HashMap` is
  randomised per process. The same alias would have produced different bytes
  after every restart, and the repository would never have been clean twice
  running. The alias *list* is sorted for the same reason: reordering two
  aliases must not be a diff.
- **The format version goes to 2.** Not ceremony. A v1 build reading a v2 file
  ignores `aliases`, and its next export writes the file back without them — so
  a build that believed it was doing nothing would destroy them. Refusing the
  whole file is the only reading of a version number that protects against that.
  Old files still read; the bump only stops old *builds*.
- **`env_vars` is the one sideways reference the format keeps as an id.**
  Placement is a path because it points at a container; an env var collection is
  a sibling. It resolves after an import because the collection is itself a
  drive object travelling in the same tree.

Aliases whose workflow is not in the mirror are counted as `aliases_not_mirrored`
by `status` and `export`, so "why didn't my alias travel" has an answer. A tree
that claims an alias currently held by a workflow outside the mirror takes it —
two `dep`s is not a state — and that is reported by name under
`aliases_reassigned`, since it changes something the tree does not describe.

**Known edge, inherited rather than introduced:** `WorkflowAliases::connect`
drops a workflow's aliases when it is trashed, so an import that trashes a
workflow loses its aliases permanently — restoring it from the panel does not
bring them back. Identical to trashing a workflow from the GUI, so it is
upstream behaviour rather than a mirror bug, but it is worth knowing.

9 tests, 6 of which were confirmed to fail with the join stubbed out. The other
three are guard tests — "an alias outside the tree is left alone", the
idempotence check — which pass trivially when the feature does nothing, which is
what a guard test is for.

#### Verified on Windows, 2026-08-19

Against the very alias whose absence started this task — `wf-test`, on
`simple-workflow-test`:

    export     written 2      <- every file rewritten once, for the v2 bump
    the file now carries
        "aliases": [ { "alias": "wf-test", "arguments": {} } ]

    (rename it to "wft" in the file, as the other machine would have)
    import     aliases_removed 1, aliases_set 1, updated 1
    export     unchanged 2, written 0

    (rename it back)
    import     aliases_removed 1, aliases_set 1
    import     unchanged 2, updated 0, no alias counters at all

That `export → unchanged 2, written 0` immediately after the import is the
line that proves it. If the alias had not actually reached the settings store,
the export would have written the file straight back to `wf-test`.
### T4.4e as built — what happens when git leaves a conflict behind

Decision 1 settles *who* resolves a conflict: the user, in their own
repository, with the tools they already have. It says nothing about what Warp
does when it **meets** one, and that was a side effect rather than a decision.

It was also a bug, and a bad one. A file with `<<<<<<<` in it does not parse, so
it landed in `ignored` next to the user's README, so the object it describes was
absent from the tree — and absence is exactly how T4.4f's import is told an
object was deleted. **The objects in the middle of being merged were the ones
that got trashed.** Nothing about the old behaviour announced this; the import
reported success and a trash count.

The policy is three rules, and the first is the one the other two serve.

**Warp never resolves a conflict, and never guesses.** Both sides are
reconstructed, but only to answer "is this file one of mine?" — never to pick
one. Choosing a side is the merge behaviour decision 1 rejected, and it would
happen silently, on the one occasion the user is demonstrably already looking at
the file. `--ours` and `--theirs` are git's words and they belong to the user.

**Both directions refuse, whole.** Import stops rather than skipping the
conflicted files, for the reason above. Export stops rather than overwriting
them, because the half-merged file is the only copy of the merge in front of the
user and git will not put it back for them. All-or-nothing in both cases: the
export reads every file it would write *before* writing any of them, so a
refusal never leaves half a drive on disk. That pre-read is not extra work — the
"is this file already correct" check needed it anyway.

**Only our files count.** Ours-ness is decided by parsing each side, not by
spotting a marker. The mirror shares a repository with the user's own work, and
their conflicted README is not ours to have an opinion about — it stays in
`ignored`, and an export runs straight past it.

Two narrowings in the detector, both aimed at not crying conflict over a good
file. A region must be **closed**: an opening marker alone is somebody writing
*about* merges. And `=======` counts as a separator only between markers,
because a bare row of equals signs is a markdown setext `<h1>` — and notebooks
are markdown, so reading one as a conflict would make every notebook written in
that style unimportable. The diff3 `|||||||` ancestor is parsed and discarded;
it is neither side.

The refusal is a type (`tree::ConflictsInTheWay`) rather than a message, so
`drive.sync.export` can answer `invalid_request` — your tree is mid-merge, ten
seconds to fix — instead of `internal`, which would send the user to look at the
wrong thing entirely. Both directions produce the same sentence, naming
`path:line (object name)`. The name is the point: "resolve `deploy-a1b2c3d4.json`"
is a chore handed to someone who has to work out what it is first.

`drive.sync.status` now reads the tree as well as the store, and lists the
conflicted files. It already described itself as the action you run to find out
why an export will not run, and an unresolved merge is the only condition that
stops *both* directions — a status that could not see it would send the user to
inspect the setting.

13 tests, all confirmed to fail with the detector stubbed out.

#### Verified on Windows, 2026-08-19

Against the same real repository, by hand-writing a genuine merge conflict into
the exported `simple-workflow-test` file — both sides real, differing in the
command:

    status     conflicted: ["...\simple-workflow-test-8f89f76f.json:1
                            (simple-workflow-test)"]
    import     invalid_request: 1 file(s) under C:\dev\warp-drive-mirror have
               unresolved merge conflicts
    export     invalid_request: the same sentence

    (the file still has its markers — the export did not overwrite the merge)

    (resolve to our side)
    status     no conflicted key
    import     updated 1, unchanged 1, trashed 0   <- the workflow survived
    export     written 1, unchanged 1

`trashed: 0` is the line that matters. Before this, the refused import would
have been a successful import that trashed `simple-workflow-test`.

Then the other half, with a real conflict in the user's own `README.md`:

    status     no conflicted key
    export     unchanged 2, written 0
    import     unchanged 2, trashed 0, and README.md in ignored with
               "unresolved merge conflict at line 1, and neither side is a
                Warp Drive file"

Their merge, their file, their business — and it stops nothing.

Two things the tests caught that reading would not have: serde_yaml 0.8 opens
its output with a document-start marker, which is the same three characters as
the front-matter fence and becomes the *closing* fence on read; and
`user:local` contains a colon, so YAML quotes it.

## T5 — Claude in Oz's seat (the spike)

Making Claude the Warp Agent proper, not a CLI harness in a pane. This is the
genuinely hard one: the 70-method `AIClient` trait plus the SSE agent-event
stream.

- [x] **T5.1** Determine the true minimum viable `AIClient` subset
- [x] **T5.2** Map the SSE agent-event protocol
- [x] **T5.3** Decide: implement the trait, or shim at the transport layer
- [x] **T5.4** Prototype behind a fork flag, default off

### T5.1 — the premise was wrong, and that is the finding

The board says "the 70-method `AIClient` trait plus the SSE agent-event
stream", as though the trait were the obstacle. **`AIClient` is not on the
agent conversation path at all.** Nothing in the trait is required to hold a
conversation, so the minimum viable subset is *empty*.

The conversation goes through a different door entirely:

    app/src/ai/blocklist/controller/response_stream.rs  spawn_generate
      -> ai::agent::api::generate_multi_agent_output(server_api, params, cancel)
         -> warp_multi_agent_client::generate_multi_agent_output(BaseClient, Request)

`warp_multi_agent_client` takes a `BaseClient` — the HTTP/auth client — not an
`AIClient`. The trait's 70 methods are conversation *metadata* (list, rename,
fork, delete), agent-definition CRUD, memory stores, ambient/cloud tasks,
artifacts, and usage reporting. Every one is beside the conversation, not in
it.

Three things do call `AIClient` near the agent, and none blocks a turn:

| Method | Caller | What breaks without it |
| --- | --- | --- |
| `get_feature_model_choices` | `ai::llms` | the model picker falls back to `ModelsByFeature::default()` |
| `get_ai_credit_availability`, `get_request_limit_info` | `ai::request_usage_model` | the usage readout is blank |
| `list_ai_conversation_metadata` | `history_model`, after a stream | titles in the conversation list |

One method *is* load-bearing, and for the path you would least expect:
`create_agent_task`. `pane_group::pane::local_harness_launch` calls it to mint
a run id before launching a **local** Claude child pane. So upstream's "local"
harness still needs an account to start. That is the sharpest single fact in
T5: the existing local-harness feature is not actually account-free.

### T5.2 — the protocol is a mutation log, not a token stream

`POST {server_root}/ai/multi-agent`, protobuf body, `text/event-stream`
response, each `data:` line a **base64url-encoded protobuf `ResponseEvent`**
(`crates/warp_multi_agent_client/src/lib.rs`, 163 lines — the whole transport).

Three event types:

    StreamInit      conversation_id, request_id, run_id   (exactly once, first)
    ClientActions   repeated ClientAction
    StreamFinished  Done | QuotaLimit | ContextWindowExceeded | InternalError |
                    InvalidApiKey | ... + token usage    (exactly once, last)

The surprise is `ClientAction`. These are not chunks of text — they are remote
mutations against a store the *client* owns: `CreateTask`,
`AddMessagesToTask`, `UpdateTaskMessage` with a `FieldMask`,
`AppendToMessageContent` with a `FieldMask`, `BeginTransaction` /
`CommitTransaction` / `RollbackTransaction`, `StartNewConversation`,
`MoveMessagesToNewTask`. Applied by
`BlocklistAIHistoryModel::apply_client_actions`.

And the request carries `TaskContext { tasks }` — **the client's entire task
list, every turn**. So the server is not the keeper of the conversation; the
client is, and it re-presents the whole thing each time. That single fact is
what makes a local implementation possible: there is nothing to recover from a
server, because the server never held it.

A `Message` is one of 22 kinds — `UserQuery`, `AgentOutput`, `AgentReasoning`,
`ToolCall` (39 tools), `ToolCallResult`, `UpdateTodos`, `WebSearch`,
`ArtifactEvent`, and so on. Tool *results* return as request `Input`s, not as
a separate channel: the client executes, then replays.

The minimum well-formed stream, confirmed against upstream's own synthesizer
in `terminal::shared_session::replay_agent_conversations`:

    StreamInit
    ClientActions[ CreateTask { task, messages: [] } ]      (first turn only)
    ClientActions[ AddMessagesToTask { task_id, messages } ] (repeat)
    StreamFinished { Done }

A stream that ends without `StreamFinished` is turned into `UnexpectedEof` and
retried three times.

### T5.3 — shim at the transport layer, and the layer is one function

    ai::agent::api::generate_multi_agent_output(
        server_api: Arc<ServerApi>,
        params: RequestParams,
        cancellation_rx: oneshot::Receiver<()>,
    ) -> Result<ResponseStream, ConvertToAPITypeError>

In: the whole conversation plus what is new. Out:
`Stream<Item = Result<ResponseEvent, Arc<AIApiError>>>`. Everything the agent
surface does — blocks, diffs, todos, history, cost — hangs off this one call,
and nothing above it can tell whether the events came off a socket or a pipe.

So implementing the trait was never the choice. One `if` at the top of that
function is the entire integration.

### T5.4 as built

`app/src/ai/local_agent/` — a local implementation of that one function,
answering from the `claude` CLI. `fork::local_agent_enabled()`, and this one is
**default off**, unlike every other predicate in `fork.rs`: the others enlarge
what works, this one substitutes for something that already does. Opt in with
`WARP_FORK_LOCAL_AGENT=1`.

`local_agent::handles` claims only a plain `UserQuery`. Passive suggestions,
resume, code review and project init keep going upstream — they have
server-side behaviour this does not reproduce, and answering them locally
would be worse than not answering.

Session continuity needed no new state. `StreamInit.conversation_id` is stored
by the client as the conversation's server token and handed back as
`params.conversation_token` next turn, so reporting Claude's session id there
makes Warp's own round-tripping the session store: `--session-id <uuid>` on the
first turn, `--resume <uuid>` after. The id is read from Claude's `init` event
rather than reused from the spawn arguments, so that if `--resume` misses, the
token follows the session that actually exists.

**The mistake worth naming: never emit `ToolCall`.** A `ToolCall` message is an
*instruction* — Warp's action model executes it and returns a result. Claude
has already run the tool. Emitting one would run it a second time: a second
`rm`, a second push. Tool activity is therefore reported as `AgentOutput`
text. There is a test named after the failure it prevents.

13 tests over the translation layer, every fixture line copied from real
`claude --print --output-format stream-json --verbose` output rather than
invented — otherwise the test only checks that the code reads my guess.

What the spike does not do, and the shape of the next step: Claude runs its own
tools, so Warp's diff review and command approval do not participate. Wiring
Warp's own execution back in means `--input-format stream-json` so tool results
can be fed back mid-turn, and at that point the `ToolCall` messages become
correct rather than dangerous. Also absent: model selection, attachments, MCP
context.

#### Verified on Windows, 2026-08-19

In the real agent panel, in a **logged-out** client — the left panel still says
"Sign in to access Agent conversations" in both screenshots:

    New Agent Tab  ->  "New Warp Agent conversation"
    prompt         ->  "In one short sentence: what is the capital of France?"
    child process  ->  claude.exe (27800), parent warp-oss.exe (32260)
    rendered       ->  "Paris is the capital of France."

    follow-up      ->  "What did I ask you in my previous message? Quote it."
    rendered       ->  You asked: "In one short sentence: what is the capital
                        of France?"

The second turn is the one that matters. It proves the conversation token made
the full round trip — Claude's session id into `StreamInit`, stored by Warp as
the conversation's server token, handed back as `params.conversation_token`,
out again as `--resume` — through Warp's own storage rather than through
anything this fork keeps. There is no session state in `local_agent`, and the
agent still remembered.

Also confirmed by the same run: an account-free Warp *can* hold an agent
conversation. Upstream it cannot, and not because of a gate — because every
path leads to `{server}/ai/multi-agent`, which needs a bearer token.

**How the GUI was driven, since none of it is obvious.** No keyboard focus is
available from a background process on either platform, so this had to be done
without one:

| Mechanism | Works | Note |
| --- | --- | --- |
| `PostMessage` WM_KEYDOWN/UP | yes | plain keys reach Warp unfocused |
| the same, with Ctrl/Shift held | **no** | posted messages don't set the thread key state, so `Ctrl+Shift+Enter` arrived as a bare Enter and ran the prompt as a shell command |
| `PostMessage` WM_CHAR | **no** | typed characters never reached the editor |
| `warpctrl input replace` | yes | but it does not run the input classifier |
| command palette + Enter | yes | `warpctrl surface command-palette open --query ...` seeds it and Enter invokes the top entry |

So the way into agent mode without a modifier is the palette: seed it with
`New Agent Tab`, press Enter, then `input replace` and Enter again. Two facts
made that necessary — natural-language auto-detection is **off by default**
(`agents.warp_agent.input.ai_auto_detection_enabled`, default `false`), and
`input replace` sets the buffer without classifying it, so the prompt stays a
shell command however it is phrased.

WSLg is worse: `XGetInputFocus` returns `None` and `XSetInputFocus` does not
stick, because the RAIL window is not foreground on the Windows desktop and
Xwayland has no keyboard focus to give. Clicks work there, keys do not. That
is the WSLg counterpart of the Windows foreground lock, and it is why this was
verified on Windows.

### T5.5 — the sign-in gate over a history that was already here

Spotted by the user in the verification screenshots above: the left panel said
"Sign in to access Agent conversations / Create an account and enable AI to
access your conversation history", sitting next to a working local conversation
the whole time.

That sentence was true while the only agent was Warp's, because the history was
Warp's. T5 made it false. Conversations are written to the local database and
read back at startup, and `AgentConversationsModel::unfiltered_entries` ends
with a loop over `get_local_conversations_metadata` that touches no server. The
two auth-dependent paths in that model are the cloud half — pulling ambient
tasks and filling the creator filter — and neither is the list.

The fix is one call site, the same shape as `is_warp_drive_available` in T4.2:
route the anonymity check in `ToolPanelView::availability` through
`fork::is_anonymous_for_ui`. The second branch already passes, because
`is_any_ai_enabled` carries the account bypass.

Worth knowing: the gate was not only cosmetic. `on_conversation_list_view_
visibility_changed` calls `register_view_open` only when the panel is
`Available`, so the list was never registered as a data consumer either. And
opening it account-free costs no network traffic — the cloud fetch early-returns
without a user id, leaving the load state at `WaitingForCloud`, and `can_poll`
is false for that state.

#### And two bugs the unlocked panel exposed

Both mine, both the same omission, and neither was caught by any test I had
written — the local agent wrote the agent's half of the transcript and nothing
else.

    Untitled conversation
    C:\dev\warp                                    58 years ago

**No user turn.** Upstream the *server* echoes the query back as a `UserQuery`
message. Live it is inert — `convert_from` maps it to
`NoClientRepresentation`, because the input already drew the prompt — so it
looks redundant. It is not: on the way back out of the database
`convert_conversation` turns it into the exchange's `AIAgentInput::UserQuery`.
Without it a restored conversation has the answers and not the questions, and
no `initial_query` for the title to fall back to.

**1970.** The same message, unstamped. A restored exchange's `start_time` comes
from that query's context time, then from any message timestamp, and then from
`unwrap_or_default()` — which for a `DateTime` is the Unix epoch. Every message
now carries the time the turn started, one time per turn because one turn is
one exchange.

`Task.description` is now set from the prompt too, since
`AIConversation::title` reads it before falling back to the initial query. Cut
by character rather than byte: `String::truncate` inside a glyph panics, and a
pasted prompt is exactly where one turns up.

Conversations recorded before this keep their empty title and their 1970 —
nothing rewrites history rows, and the fix is in what gets written.

#### Verified on Windows, 2026-08-19

Signed out, panel open, one new conversation and one restart:

    ACTIVE  Name three colours, comma...   C:\dev\warp    Just now
    PAST    Untitled conversation          C:\dev\warp    56 years ago

The second line is the conversation recorded before the fix, left exactly as
it was — which is the clearest statement of what the fix does and does not do.

Then closed with `CloseMainWindow` and relaunched:

    PAST    Name three colours, comma...   C:\dev\warp    1 min ago

    /agent Name three colours, comma separated, nothing else.
           Red, green, blue

Both halves. Before the fix a restored conversation showed only the answers.

## T6 — WSL integration

User-stated high-priority feature-add. File explorer and remaining features
seamless across Windows and WSL2.

- [x] **T6.1** Scope what "seamless" means concretely; enumerate broken surfaces
- [ ] **T6.2** Path translation (`\\wsl.localhost\...` ↔ `/mnt/c/...`)
- [ ] **T6.3** File explorer across the boundary
- [ ] **T6.4** Decide the WSLg window-forwarding story or stay Windows-native.
      T1.11 changed the terms of this decision: the WSLg build works, so this
      is now a choice between two working options rather than a workaround for
      one broken one. What the Linux build costs is llvmpipe — software
      rendering, real CPU, no GPU passthrough — and what it buys is a Warp that
      is *inside* WSL, where the files and the shell already are.
      **T6.1 changed them again — see "What this means for T6.4" below.**

### T6.1 — as built

Scoped by running it, on Windows, with a live `bash` session in Ubuntu-WSL2,
following T1.7's method. The whole enumeration is empirical: every line below
is a thing that was watched happening, not a thing read in the source and
assumed.

**Setup.** Settings → Features → "Default shell for new sessions" → `Ubuntu`,
which is `session.new_session_shell_override` in `settings.toml`. Warp reads
the distribution list straight out of `HKCU\...\Lxss` (`terminal/wsl/model.rs`),
filtering `docker-desktop` and `rancher-desktop`; Ubuntu was offered and
launched first try. Restored sessions keep the shell they were saved with, so
the setting only shows up on a *new* tab. It was set back to Default afterwards.

**The surprise: most of it already works.** Upstream has been through this
area recently — `d46473504` routes Warp's internal `git` through `wsl.exe` for
UNC working directories, `136f451dc` matches `wsl$`/`wsl.localhost` hosts
case-insensitively, `aa873b543` removes canonicalizations that froze the UI on
WSL tabs. This is not a greenfield.

Verified working, WSL session, `cd ~/git/warp`:

| Surface | Result |
|:--|:--|
| Warpify (blocks, timing, exit codes) | works |
| cwd chip | `~/git/warp` — shell-native, correct |
| git branch + dirty count | `dev`, `± 0` — real, via `wsl.exe` |
| Code review panel | works, real diffs |
| Opening a WSL file in the editor | works (`warpctrl file open '\\wsl.localhost\...'` rendered the markdown) |

**Broken, in order of how much it costs the user.**

**(a) The project explorer looks empty for a WSL-native directory — and it is
not empty, it is unindexed, and it says nothing about that.** The root appears,
named correctly, expanded, with no children. No spinner, no message, no error.

The controlled experiment, same session, same window, one `cd` apart:

    cd ~/git/warp        -> root "warp", zero children
    cd /mnt/c/dev/warp   -> full tree, ~46 entries, instant

So it is not "WSL sessions"; it is the path. `/mnt/c/...` converts to `C:\...`
by `convert_wsl_to_windows_host_path` and everything downstream is ordinary.

**Correction, and it matters.** The first reading of this — "empty tree, a hard
failure" — was wrong, and the thing that showed it was wrong was leaving the
window open. On a later launch the same root filled in completely, about a
minute after the tab was activated, with nothing changed but time. So (a) is
the same fault as (d): the walk is running, over 9p, and until it lands the
panel is a root with no children and no indication that anything is happening.

Why it renders as an empty tree rather than a spinner is exact, in
`file_tree/view.rs`: the loading and "doesn't work in WSL" states are both
inside `if self.displayed_directories.is_empty()`. A root *had* arrived — from
the pane group's own working directories, which do not wait for an index — so
that branch was never taken and `render_file_tree` drew a root with nothing
under it. A directory that is being indexed is indistinguishable from a
directory that is empty.

That is the whole user-visible bug, and it is a small fix independent of
everything else: an unloaded root should say so.

**(b) Global search refuses outright, on the shell rather than the path.**

    Global search unavailable
    Global search doesn't currently work in Git Bash or WSL.

Same session, `cd /mnt/c/dev/warp` — an ordinary Windows directory, whose file
tree renders perfectly a panel away — and global search *still* refuses. The
gate asks what shell you launched, not what directory you are in. One line:

    app/src/workspace/view.rs
        let is_unsupported_session = is_wsl_session;

feeding `CodingPanelEnablementState::UnsupportedSession`. Three panels read it,
but only global search treats it as a hard block: the file tree
(`file_tree/view.rs`) and code review (`code_review_view.rs`) consult it only
when they have nothing to show, as a fallback *message*. That is why (a) shows
an empty tree rather than the "doesn't currently work in WSL" text — the tree
had a root, so it never reached the message.

**(c) Three spellings of one directory, in one window, at one time.**

    ~/git/warp                                          (cwd chip)
    \\WSL$\Ubuntu\home\effatha\git\warp                 (agent pane)
    \\?\UNC\WSL$\Ubuntu\home\effatha\git\warp           (code review header)

The third is a verbatim path leaking into the UI. It is also the fingerprint of
the underlying problem, and worth stating precisely, because it is not obvious
and it is the thing T6.2 has to be built on:

> `dunce::simplified` strips the `\\?\` prefix only for `VerbatimDisk`. Every
> other prefix is left alone. So `dunce::canonicalize` — which Warp uses as its
> normal-form function, in `StandardizedPath::from_local_canonicalized` and in
> `normalize_cwd` — turns `C:\dev\warp` into `C:\dev\warp` and turns
> `\\WSL$\Ubuntu\...` into `\\?\UNC\WSL$\Ubuntu\...`.

Measured on this machine with `CreateFileW` + `GetFinalPathNameByHandleW`,
which is exactly what Rust's `canonicalize` calls:

    C:\dev\warp                        -> \\?\C:\dev\warp        (dunce strips)
    \\wsl$\Ubuntu\home\...\warp        -> \\?\UNC\wsl$\...
    \\WSL$\Ubuntu\home\...\warp        -> \\?\UNC\WSL$\...
    \\wsl.localhost\Ubuntu\home\...    -> \\?\UNC\wsl.localhost\...

Two things follow, and both bite. Canonicalization is not idempotent-as-
identity for WSL paths: it is a pass-through with a prefix bolted on. And it
does not normalise case or host — `WSL$`, `wsl$` and `wsl.localhost` all name
the same directory and canonicalize to three different strings, where a drive
path canonicalizes to its real on-disk case. Any two code paths that reach the
same WSL directory by different spellings hold different map keys, for ever.
`parse_wsl_unc_path` compares hosts case-insensitively; a `HashMap<PathBuf, _>`
does not.

**(d) The first index of a WSL repo from Windows takes minutes, and can take
longer than anyone will wait.** The clean isolating experiment — a *PowerShell*
session (not WSL) whose cwd is `\\wsl$\Ubuntu\home\effatha\git\warp` — proves
the file tree is willing to index a WSL UNC path: it goes straight into a
proper loading skeleton, so this is not a refusal. It was still a skeleton
**ten minutes later**, and the git chip had disappeared meanwhile. The same
repo on the Windows disk: instant.

The 9p redirector is why, and it is measurable. Same 2247-file tree, three ways
in:

    inside WSL (native ext4)                        26 ms
    Windows disk (C:\dev\warp\crates)              101 ms
    Windows -> WSL over 9p (\\wsl$\...)           1323 ms

**13× the Windows disk, 50× native.** And Warp indexes ignored files too — the
tree renders them in italics — so the walk does not stop at `.gitignore`. The
WSL checkout is 209,644 files, of which 197,136 are under `target/` (76 GB).
`MAX_FILES_PER_REPO` is 200,000. At the measured 9p rate that budget alone is
two minutes of `stat` before anything else.

**(e) The local agent could not start at all in a WSL session — this fork's own
bug.** Found by running (d)'s sibling experiment, an agent pane in the WSL
session:

    Request failed with error: Other(Could not start `claude`. The local agent
    needs the Claude Code CLI on PATH (https://claude.com/claude-code).
    Caused by:
        The directory name is invalid. (os error 267))

`os error 267` is `ERROR_DIRECTORY`. T5's `Turn::from_request` took
`session_context.current_working_directory()` — which is deliberately
*shell*-native, so `/home/effatha/git/warp` — and handed it to `current_dir` on
a Windows process. Two faults in one message: the failure, and a first sentence
confidently blaming something else while the real cause sat in the `Caused by:`
line underneath.

Fixed in `efa59bf81`. Not by converting the path — that would start the process
and move the cost onto the 9p numbers above, and an agent is a file-reading
workload. Claude now runs *inside* the distribution, `wsl.exe --distribution X
--cd <linux path> --exec /bin/sh -lc 'exec claude "$@"' claude …`, which is the
same treatment `warp_util::git` already gives `git`, arrived at for the same
reason and with the same login-shell caveat. Four tests, on a pure function, so
the decision is assertable without a Windows host or a distribution.

**`@`-mentions work in WSL. The one report against them was the directory, not
the boundary.**

Reported from the keyboard: the `@` picker opens in both the terminal and the
agent input in a WSL session; the terminal offers files, and the agent panel
seemed to offer only folders.

It offers both. `AIContextMenu::get_categories_for_mode` picks between two
categories that share the label "Files and folders":

    if is_active_dir_in_git_repo { RepoFiles } else { CurrentFolderFiles }

— and that line is *identical* in the agent branch and the terminal branch, so
the two inputs cannot disagree about a directory. They were looking at
different ones. The terminal pane was in `~/git/warp`, a repository, so it got
`RepoFiles`: the whole recursive index. The agent pane was in `~/git`, which is
not a repository, so it got `CurrentFolderFiles`:
`std::fs::read_dir` of that one directory, which on this machine holds **39
directories and exactly one file**. Nothing filtered the files out; there was
one, and zero-state sorts reverse-alphabetically (`data_source.rs`,
`file_data_source_for_pwd`), so `Clipboard Text.txt` sorts to the bottom of a
list of folders.

Worth knowing rather than discovering: with a non-empty query, files are
deliberately ranked *above* directories — `match_result.score += 100` for
`!is_directory`. So `@` alone in a folder-heavy directory looks folder-only,
and `@` plus two characters does not.

The distinction that is actually load-bearing, and is invisible in the UI
because both categories carry the same label: `RepoFiles` is the recursive
repository index, `CurrentFolderFiles` is one non-recursive `read_dir`. Inside
a repo you can mention `app/src/fork.rs`; outside one you can only mention what
is in front of you.

**Also confirmed from the keyboard, and previously only assumed here:**
ctrl-clicking a file link in agent output opens the system file manager
(Directory Opus) on the WSL path. That is the `is_network_resource` carve-out
earning its keep — it excludes WSL UNC hosts precisely so `is_path_valid` does
not reject WSL file links, and there is an upstream test saying so. It is no
longer an assumption.

**Still not reached, and named rather than quietly skipped:** drag-and-drop
across the boundary.

**What this means for T6.2 and T6.3.** The work is not "add path translation" —
translation already exists and is used in a dozen places. It is, cheapest and
most valuable first:

1. ~~**Say that a root is loading.**~~ Done — see "T6.3 — as built" below.
2. ~~**Move the global-search gate from the shell to the path.**~~ Done — see
   "T6.3 — as built" below. The answer turned out to be simpler than "translate
   the path": search never needed the shell in the first place.
3. **One spelling.** Pick the canonical form for a WSL directory and hold every
   map key in it. `dunce::canonicalize` cannot be that function, since it
   preserves whatever case and host it was handed.
4. **Do not present a verbatim `\\?\UNC\…` path to a human.**

Note what is *not* on this list: making the index fast. Nothing in Warp can
make 9p cheap, which is why T6.4 is the more consequential decision.

**What this means for T6.4.** The decision was framed as "two working options,
and the Linux build costs llvmpipe". T6.1 puts a number on the other side of
that trade. A Warp *inside* WSL reads the files ~50× faster than a Warp on
Windows reaching in, and skips (a) through (e) entirely — there is no boundary
to be seamless across, so none of the five bugs can exist. Software rendering
is a cost paid once per frame on a machine with cores to spare; 9p is a cost
paid per file, by every index, search, diff and agent read. **T6.1's finding is
that the WSLg build is the stronger option, not the fallback** — and that the
Windows build's WSL support is worth fixing for the case where the files really
are on `C:`.

#### Verified on Windows, 2026-08-19

`efa59bf81`, Ubuntu-WSL2, warp-oss debug build. (a) and (b) reproduced by
screenshot; (c) read off three visible panels at once; (d) timed with
`Get-ChildItem -Recurse` on both sides and `find` inside the distribution; (e)
reproduced in the agent pane and fixed.

The (e) fix, verified after rebuild, in a restored WSL session at `~/git/warp`:

    /agent Run pwd and reply with just the directory path, nothing else.
           Bash
           /home/effatha/git/warp

Claude ran inside Ubuntu, in the session's own directory, and said so. Before
the fix the same prompt returned `os error 267`.

The same screenshot is also where (a)'s correction came from: the project
explorer, empty a minute earlier, was by then showing the whole WSL tree. Which
is the T5.5 lesson again — the verification screenshot is worth reading for
what it happens to contain, not only for the thing it was taken to prove.

Warp's own log was no help: the second `warp-oss.exe` of a pair takes
`warp-oss.log.recovery` when the first holds `warp-oss.log`, and that file
stayed zero bytes for the whole session. Everything above came from the UI and
from probes run beside it.

### T6.3 — as built

Item 1 of the list above: **an unread root is not an empty root.**

The panel already had a loading state and simply never reached it. Its guard is
`total_item_count() == 0`, and an unread root is not zero items — it
contributes its own header. So a root that had not been indexed yet drew
exactly what a folder with nothing in it draws: the name, expanded, no
children.

Measured rather than assumed, in the real view against the real model, with a
repository held in `IndexedRepoState::Pending`:

    total_item_count = 1
    items            = ["…/repo"]
    root entry loaded = Some(true)
    expanded          = true

That `loaded = true` was the lie the whole bug rested on.
`FileTreeEntry::new_for_directory` hardcodes it. That suits its other caller —
`remote_model.rs` fills the entry in on the next line — and suits none of the
four placeholders in `file_tree/view.rs`, each of which exists *because* the
contents have not arrived. They now build an unloaded entry, and the panel asks
whether any root has been read.

Any, not every. With a repository open in one pane and a slow root in another,
the tree that already has contents keeps showing them — and that mixed case is
the ordinary one here, where a `C:` root indexes instantly and a `~/` root does
not.

A read that fails is still a read: `IndexedRepoState::Failed` keeps the
loaded-and-empty entry so a root that cannot be indexed does not spin forever.
Only `None` — the model has not been asked yet, or the registration will be
retried — counts as pending.

#### Verified 2026-08-19

`dbe8c310b`. Three tests in `view_tests.rs`, run against the real
`RepoMetadataModel` and `DetectedRepositories`, not mocks:

| test | what it holds |
| --- | --- |
| `a_root_that_is_still_indexing_reads_as_loading_not_as_empty` | the fix. Fails without it — confirmed by reverting `create_unloaded_entry` to the old behaviour and re-running |
| `a_directory_that_is_genuinely_empty_does_not_spin_forever` | the regression the `loaded` flag exists to prevent |
| `a_root_with_contents_keeps_showing_them_while_a_sibling_loads` | why the predicate is "any", not "every" |

48 file-tree tests pass. `cargo clippy -p warp --lib` clean. Builds and links
on Windows.

**Not verified on screen.** The remaining link is `render_file_tree`'s two-line
branch into `render_loading_state` — read, not watched. Reproducing it live
needs a cold WSL repository in a WSL session, and the attempt ran aground: the
running window had cached metadata for every root it was pointed at, `warpctrl
tab activate` does not take an index, and the `cd` landed in an agent pane's
steering input instead of a terminal. Recorded here rather than papered over,
per T1.7. Next hands-on session should catch it by launching with the shell
override set to Ubuntu and `cd`-ing into a repository Warp has never indexed
(`~/git/lapce`, 12,372 files, is the one to use).

One thing that attempt did surface, unasked: steering a *resumed* local-agent
turn fails with `No deferred tool marker found in the resumed session`. That is
the already-recorded `--input-format stream-json` gap wearing a different face.

#### Item 2 — global search was refusing the shell, not the path

The list said "move the gate from the shell to the path". The gate turned out
not to need a path at all.

Global search is in-process ripgrep over `search_roots`
(`GlobalSearch::run_warp_ripgrep_cli` → `warp_ripgrep::search::search_streaming`)
— filesystem I/O and nothing else, with no shell anywhere in it. And
`left_panel.rs` hands global search and the project explorer **the same**
`active_directories`, so the two panels cannot disagree about what is there.
Which is exactly what T6.1 watched happen: the tree rendering a directory
perfectly while the search panel refused it, in one window.

So the refusal was wrong twice. A WSL session in `/mnt/c/...` is looking at a
Windows directory and searches at full speed. And a WSL session in `~/...`
searches correctly too — measured from Windows, same query, same repo:

| root | time | matches |
| --- | ---: | ---: |
| `C:\dev\warp` | 0.12 s | 40 |
| `\\wsl.localhost\…\git\warp` | 9.52 s | 40 |
| `\\wsl.localhost\…\git\lapce` (12,372 files) | 17.16 s | 54 |

Identical results, ~39× the wall clock, no benefit from a warm cache on the 9p
side. Results stream in batches, so slow and correct beats a wall.

The decision moved out of `render` into `blocker`, a pure function over the
enablement state and whether any root arrived — testable without a window.
`UnsupportedSession` now blocks search only when there is no directory at all,
and the message says that. The old one — "Global search doesn't currently work
in Git Bash or WSL" — was wrong about WSL and had never been true of Git Bash
either: the state is set from `Session::is_wsl`, and nothing else.

Scoped to global search on purpose. The file tree and code review read the same
`UnsupportedSession`, and the shared seam at `workspace/view.rs:17484` is still

    let is_unsupported_session = is_wsl_session;

Moving that moves three panels at once and needs its own evidence.

#### Verified on Windows, 2026-08-19

`68ec5d437`, debug build, restored WSL session. The session is genuinely WSL and
genuinely in a Linux-native directory:

    uname -sr; pwd
    Linux 6.18.33.2-microsoft-standard-WSL2
    /home/effatha/git

With `warpctrl surface global-search open`, the panel renders its ordinary zero
state — "Search in files across your current directories" — where T6.1 recorded
"Global search unavailable / doesn't currently work in Git Bash or WSL" for this
same session.

**Not verified: a query typed into the live box.** `warpctrl` has no action for
entering a search query (`surface.global_search.open` is the only search action
in the catalogue), the panel's query editor was occluded, and raising the window
would mean stealing focus from the user's own desktop. What the query *would*
do is the ripgrep table above, measured directly against the same roots.

Five tests over `blocker`, including that remote sessions are unchanged.

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
