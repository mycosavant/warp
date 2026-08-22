# Fork task board

Tracks the full scope agreed 2026-08-17. Ordered by value-per-line-of-code, not
by conceptual grandeur — see `SPEC.md` for the reasoning behind each.

Status key: `[ ]` not started · `[~]` in progress · `[x]` done · `[!]` blocked
· `[-]` dropped (with reason)

Phases 0–4 in `SPEC.md` are the original de-telemetry/de-account track. This
board supersedes it from Phase 5 on, and renumbers nothing.

> **Most of this file is history, and that is the point.** T1–T7 are done.
> Read a section when you are about to touch that area; do not read it front to
> back.
>
> Three kinds of content live here, and they age differently:
>
> * **Checklists** (`- [x] T1.4 …`) — a record of what shipped. Historic.
> * **"as built" sections** — what was actually found when the thing was run,
>   including where the plan was wrong. **These stay live**, because they are
>   the only place several findings are written down.
> * **"Decisions on record"** and **"Open questions"**, near the bottom — live,
>   and the first place to look before re-opening something.
>
> **T8 is the current phase.** `IDEAS.md` is the queue in front of it.
> `../CLAUDE.md` is the cold-start summary of the method and the invariants.

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

- [x] **T1.8** `input submit` action — upstream deliberately ships only
      `insert`/`replace`, so a seeded command is never auto-executed. Adding
      submit is a local patch, and it is the difference between "assist" and
      "autonomous". **Decided yes, and shipped** — the box was simply never
      ticked. `input.submit` has been `Implemented` in the catalog since T1.9,
      which verified it end-to-end over MCP and found the `executed`/`queued`
      bug in the process. Re-verified on Linux while closing this:
      `input submit "echo … > file"` returned `executed: true, queued: false`
      and the file was there.

      The decision is worth stating rather than leaving implied, because it is
      the one place the fork hands over something upstream withholds on
      purpose. The reasoning: the fork's whole premise is an agent driving
      Warp, and an agent that can open every surface but only *type* into one
      is a demo. The guardrail is not withholding the verb — it is that
      `input.submit` runs its text as a **shell command**, so it reaches
      `bash` and not the agent (T6.5), and reaching the agent needs the
      separate, later `agent.*` actions with their own depth and tool limits.
      One `input.submit` runs exactly one command, by construction.
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

### T1.12 — as built

Four actions, catalog 96 → **100**:

    drive.object.list    every object in the personal drive
    drive.object.get     one object, as the file an export would write
    drive.object.create  a workflow, notebook or folder
    drive.object.trash   trashed, not deleted

**Most of this already existed and was not reachable.** `snapshot` reads the
personal drive into a typed, app-independent form; `write_object` is the
thirteen constructors that put one back. The missing piece was writing *one*:
`apply` is reconciliation and reads absence as deletion, so handing it a single
object would trash the rest of the drive. Hence `apply::put` and `apply::trash`
beside it — the same machinery, with no opinion about what is not in front of
it.

#### The read side and the write side disagree about format, on purpose

`get` returns the object's file exactly as `drive.sync.export` writes it.
`create` does not accept one.

The file's header opens with a `uid` and an `owner`, and neither is a caller's
to choose — an identity supplied from outside is precisely how one object
silently overwrites another. A `create` that took a file would have to ignore
the first two lines of everything it was handed, which is a worse contract than
asking for the three things that genuinely *are* the caller's: what kind, what
it is called, what is in it. The action that writes a supplied identity on
purpose is `drive.sync.import`, where the identity comes from a file the user
has in git and can see.

The file is still the documentation, which is the T7.2 lesson reapplied: the
format explains itself by being the one already on disk. `drive object get` on
any workflow prints its `data` block, and that block is exactly what
`create --body` takes.

#### `drive.object.create` was pinned as unparseable, and taking it back is a decision

Upstream's `drive.*` group was twelve actions, **every one of them
`status: Stub, authenticated_user: true`** — specified, never implemented, and
gated on a sign-in. T4.4d removed them and pinned the names in
`malformed_and_removed_action_names_are_not_deserialized` so a new action could
not quietly inherit a retired contract. That test fired here, which is it
working.

The name is taken back deliberately, and now has its own test saying so, because
the parameters are **not** upstream's:

    upstream (stub)  { object_type, content, content_file }
    this fork        { object_type, name, body, folder }

An object needs a name and somewhere to live. A caller written against the old
spec would send `content` and be told `invalid_params` — the right answer, and
one that should be arrived at on purpose rather than by accident. The other
nine names stay pinned; they are still names with nothing behind them.

Worth noting what upstream's group *was*, because it is the fork's thesis in
miniature: twelve account-gated stubs for the object store. This fork now
implements four of them, account-free.

#### Refused rather than reparented

Creating into something that is not a folder is an error, not a quiet placement
at the top level. `tree`'s exporter reparents orphans and *names them in the
summary*; an action has no summary to hide in, so an object that landed
somewhere other than where it was asked to go would be a silent wrong answer
the user finds later, elsewhere.

Folder paths are reported as display names rather than the mirror's slugged
directory names, because the question is "where is it in the panel" and the
panel shows names. The walk carries the same cycle guard `tree` does, for the
same reason: `folder_id` is a plain string column with no referential integrity
behind it, so a loop is representable and would recurse until the stack ran
out. There is a test for it, and it has to close the loop *from underneath*
since nothing in the action surface can create one.

#### Verified by running it

    drive object list                      -> {} on a fresh store
    drive object create --type folder      -> Deploys
    drive object create --type workflow
      --folder <id> --body '{...}'         -> path: ["Deploys"]
    drive object list                      -> both, nested
    drive object get <workflow>            -> the file, data block and all
    restart                                -> both still there (persist really
                                              reaches SQLite)
    drive export                           -> deploys-434ea074/.warp-folder.json
                                              deploys-434ea074/ship-it-07470dec.json
    drive object trash <workflow>          -> trashed: true
    drive object list                      -> visible 1, trashed_hidden 1
    drive export                           -> the trashed file is still written,
                                              carrying "trashed": "2026-08-21T…"

The last line is the one worth having. It is the invariant the whole local-sync
design hangs off — a deletion travels as content, not as absence — and it now
holds for a deletion made through the catalog, not just one made in the panel.

The export also proves these are the *real* store rather than a parallel one,
which a unit test could not: the mirror is written by `snapshot`, and `snapshot`
found them.

Twelve handler tests, plus a CLI example per action (the coverage test demanded
them) and a positive parse test for the four names.

**One gap left, unchanged:** `warp_drive.local_sync.path` is still not in
`ALLOWLISTED_SETTING_KEYS`, so the live check above needed a scratch
`XDG_CONFIG_HOME` to point the mirror somewhere. That is deliberate — T4.4d's
argument is that an agent can ask for an export but must not decide where it
lands — and it is worth knowing it makes the export half awkward to exercise
from outside.

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
      a proxy capture during a real recording.

      **Still blocked, and now for a precise reason rather than a vague one.**
      The general egress question was closed with a rig that would answer this
      one too (see "Nothing escapes: measured, not argued"), so the only thing
      missing is a way to *start a recording*. There is none from outside the
      GUI: voice is behind the `voice_input` cargo feature, has no
      local-control action, and is triggered by a keybinding — and keystroke
      synthesis does not work under WSLg (T5.4: clicks land, keys do not). A
      microphone does exist here, WSLg forwards one as PulseAudio `RDPSource`,
      so the hardware is not the obstacle.

      **Update 2026-08-21: the keystroke half of that blocker is narrower than
      it looked.** T5.4's claim that `XSetInputFocus` does not stick is wrong —
      it does. See "The WSLg input wall is narrower than T5.4 recorded" under
      T8. Keys still do not arrive, but the remaining gap is activation rather
      than X focus, and `crates/computer_use` ships a real input stack that
      nobody here has tried against it. If that comes loose, this task
      unblocks itself with no person at the keyboard.

      **Recipe for whoever is at the keyboard**, since the rig is now written
      down and this is ten minutes of work:

          mitmdump --listen-host 127.0.0.1 --listen-port 8899 \
                   --set confdir=<dir> --flow-detail 2
          # then launch with HTTPS_PROXY/HTTP_PROXY at 8899 and
          # SSL_CERT_FILE=<dir>/mitmproxy-ca-cert.pem, hold the voice key,
          # say anything, and read the flow list.

      Expected: the configured loopback endpoint and nothing else. The words do
      not matter — this is a question about destinations, so silence into the
      mic would answer it just as well as speech.

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

- [x] **T1.12** Add Warp Drive object actions to the local-control catalog.
      Surfaced by T4.2 verification: the catalog can drive every part of the
      app *except* its object store, which makes exactly the fork's own
      headline feature the one thing an agent cannot exercise. 85 actions
      across app, window, tab, pane, session, input, surface, setting, theme,
      appearance, keybinding and file, and nothing that creates a workflow,
      rule or folder. `input.*` writes to the terminal's input editor rather
      than to whatever UI has focus, so the `+` button is unreachable. Same
      shape as the `setting.get/set` allowlist gap recorded under T2.

      Done as four actions — `drive.object.list`, `.get`, `.create`,
      `.trash` — bringing the catalog to **100**. T4.4d's two `drive.sync.*`
      actions were recorded as closing this, and they did close the half of it
      that is "an agent can move the store"; they left the half that is "an
      agent can make one thing". See "T1.12 — as built" below.

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
- [x] **T5.6** Find out what cancelled a turn nobody cancelled. Somebody did:
      a person pressed ctrl-c, meaning to copy the agent's answer they had
      just selected with the mouse. Warp does not copy an AI block's selection
      on ctrl-c — on any platform — so it cancelled the turn instead. Both
      halves of the premise were wrong, the log was not empty, and the fix is
      two lines and a predicate. See "T5.6 — the mystery was the user, again"
      below.

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

> **Corrected 2026-08-21.** The conclusion holds — keys still do not arrive —
> but two of the three facts above do not. `XGetInputFocus` returns the **root
> window** (`0x438`), not `None`; and `XSetInputFocus` on the Warp toplevel
> **does** stick, confirmed by reading it back. The remaining gap is one layer
> higher than X focus. See "The WSLg input wall is narrower than T5.4 recorded"
> under T8, which also records that window-targeted screenshots work.

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

### T5.6 — the mystery was the user, again

The task was written as "find out what cancelled a turn nobody cancelled",
with two supporting claims and a named suspect. All three were wrong, and the
way they were wrong is the useful part.

**The evidence was on disk the whole time.** `~/.local/state/warp-oss/` keeps
five rotated logs, and `warp-oss.log.old.0` is the instance that ran the failed
T7.2 turn. Reading it end to end instead of grepping it for "cancel":

    08:38:55   turn starts
    08:40:04   AIBlockAction::SelectText
    08:40:04   AIBlockAction::SelectText
               ... 205 of them, over four seconds
    08:40:12   EditorAction::CtrlC          <- the turn stops here
    08:40:27   BlockListContextMenu(RichContentTextRightClick { .. })
    08:40:28   ContextMenu(CopySelectedText)
    08:40:33   the poll that reported `cancelled`

Two hundred and five `SelectText` actions in four seconds is a mouse dragging
across the agent's output. Then ctrl-c. Then — fifteen seconds later, and this
is the line that settles it — right-click, context menu, **Copy selected
text**. That is what somebody does when ctrl-c did not copy.

So the turn was cancelled, honestly, with `ManuallyCancelled`, by the user. It
reported exactly what happened. The bug is upstream of the report: **ctrl-c
over selected agent output cancels the turn instead of copying it.**

This is the second time in this fork that an unexplained mid-run stop turned
out to be a person at the keyboard (`a3065d993`, "the mid-sweep exit was a
person clicking a button"). Both times the tempting explanation was a race in
code I had just written. Both times it was somebody using the app.

#### Why ctrl-c does not copy an agent's answer

`TerminalView::ctrl_c` asks "is anything selected?" through
`model.block_list().selection()` — the point-based selection. An AI block does
not use it. `set_rich_content_selection` records the selection in
`rich_content_selections` **and sets `self.selection = None`**, so the field
ctrl-c consults reads empty *precisely* when the user has selected agent text.
Not merely unset — actively cleared, by the code that knows about the
selection.

The consequence is worse than a missing feature. There is a `#[cfg(windows)]`
arm of `ctrl_c_internal` whose comment reads "Windows users expect ctrl-c to
copy if there is selected text" — and it is gated on the same wrong field, so
it never fires for agent output either. On Linux there is no copy branch at
all: the selection is cleared and the turn is cancelled.

The fix adds `BlockList::has_rich_content_selection()` and, in `ctrl_c`, one
early branch: copy, clear, return. Unconditional rather than `#[cfg(windows)]`,
because the Linux reading of ctrl-c is "interrupt the foreground process" and
in agent view there is no foreground process — the only thing ctrl-c can do to
a streaming turn is destroy it.

Clearing afterwards is load-bearing, not tidiness. A second ctrl-c has to still
stop the agent, and `clear_selections_when_shell_mode_without_focusing_input`
is a no-op while `AgentView` is enabled, so nothing else would drop the
selection and the keyboard could never reach Stop again. Copy, then stop, is
also what the Windows arm already does.

#### And the log was not empty — the wrong half of it was

`try_cancel_streams_for_conversation` logs the reason *and a full backtrace*.
`try_cancel_stream` — same file, twenty lines up — logged nothing. The plural
one is the conversation-wide path; the singular one is what
`status_bar::cancel_active_request_or_action` calls, and grepping for callers
says that is the **only** caller. So:

> The one cancellation a person can actually cause was the only one that left
> no trace.

Which is why the turn read as unexplained, and why the answer had to be
reconstructed from `dispatching typed action` lines that happen to record every
keystroke and are not a cancellation log. That is now one `log::info!`,
mirroring its sibling.

#### What was verified, and how

Reproduced as a unit test, which is the part that matters: with the fix
removed, `ctrl_c_over_selected_agent_output_copies_then_stops_on_the_second_
press` fails with `left: Some(Cancelled), right: Some(InProgress)` — the T7.2
failure, exactly, from a keystroke. The test drives the whole path
(`handle_action(&TerminalAction::CtrlC)`) against a live agent-monitored
command, so cancellation is genuinely reachable; without that it would pass for
want of anything to cancel, which is the failure mode a regression test exists
to avoid. It also asserts the trap directly — `selection().is_none()` *and*
`has_rich_content_selection()` at the same moment — and that the second press
still cancels and still writes `ETX` to the pty.

Separately, the T7.2 prompt was re-run under the same conditions on a scratch
`XDG_STATE_HOME`, and the turn finished `success`. That is the control: the
local agent does not stop on its own. Nothing in `app/src/ai/local_agent/` was
implicated or changed.

The `log::info!` was not exercised live: no `warpctrl` action reaches the
status-bar path (`agent cancel` goes through the conversation-wide sibling that
already logged), and driving ctrl-c through the GUI needs keystroke synthesis,
which does not work under WSLg.

#### Three claims retracted

| Claim in the task | Actually |
|:--|:--|
| "nobody cancelled it" | the user did, with ctrl-c, trying to copy |
| "an empty log" | the log had the keystroke; it lacked the cancellation |
| "`Cancelled` requires a real `StreamCancellation`" | true, and it was one — `ManuallyCancelled`, from the status bar |

The named suspect, `UserCommandExecuted`, was wrong. It was reasoned from the
enum ("which reason *could* fire unprompted") rather than from the log, and
reasoning about which arm of a match is plausible is not evidence. T1.7's rule
holds: run it, or in this case, read what it already wrote down.

#### Two things found and left alone

Both real, both out of scope, both untouched:

* **Restore relabels incomplete exchanges as cancelled.** `create_exchange_
  from_messages` reconstructs an exchange with no outputs — or a trailing tool
  call with no result — as `Cancelled { reason: ManuallyCancelled }`, and
  `derive_status_from_root_task` turns that into `ConversationStatus::
  Cancelled`. The reason is not persisted (`AgentConversationData` has no field
  for it), so it is hardcoded. Any turn interrupted by a crash or a quit reads
  back after restart as "you cancelled this". Ruled out as the T7.2 cause: that
  read was live, 41 seconds before the instance exited.
* **`cancel_conversation_progress` can stamp `Cancelled` with no stream and no
  log**, via the action model, when the conversation is not in
  `TransientError`. Not reached here, but it is the other silent path.

## T6 — WSL integration

User-stated high-priority feature-add. File explorer and remaining features
seamless across Windows and WSL2.

- [x] **T6.1** Scope what "seamless" means concretely; enumerate broken surfaces
- [x] **T6.2** Path translation (`\\wsl.localhost\...` ↔ `/mnt/c/...`). Reframed
      by T6.1: translation already existed. What was missing was *one spelling*
      — see "T6.2 — as built" below.
- [x] **T6.3** File explorer across the boundary — see "T6.3 — as built" below.
- [x] **T6.4** Decided: **run the Linux build when your code is in WSL, keep the
      Windows build for code on `C:`.** Both sides are now measured rather than
      argued. See "T6.4 — decided" below.
- [x] **T6.5** `warpctrl` can talk to the agent. Four actions — `agent.list`,
      `agent.prompt`, `slash.list`, `slash.run` — see "T6.5 — as built" below.
      The original framing (find a keystroke injection that carries modifiers)
      was abandoned for the better half of the same task: give the app an action
      rather than teach the harness to fake a keyboard.
- [x] **T6.6** Orchestration: one agent running several. Four more actions —
      `agent.read`, `agent.spawn`, `agent.cancel`, `agent.reveal` — plus the
      two guardrails and the local-agent tool mapping without which the first
      of them would have been decorative. See "T6.6 — as built" below.
- [x] **T6.7** Local summarization, so `/compact` works without an account.
      Found by T6.5: `/compact` submits correctly and then fails with
      `missing authentication credentials`, because summarization is an
      `AIAgentInput::SummarizeConversation` and `local_agent::handles` takes
      only `UserQuery`. Answered by running Claude's *own* `/compact` against
      the session it already holds — see "T6.7 — as built" below.

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
3. ~~**One spelling.**~~ Done — see "T6.2 — as built" below. `dunce::canonicalize`
   indeed could not be that function.
4. ~~**Do not present a verbatim `\\?\UNC\…` path to a human.**~~ Done, by the
   same function: keying a directory one way and displaying it another is what
   put three spellings on screen in the first place.

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

### T6.2 — as built

Items 3 and 4 of T6.1's list, which turned out to be one function: a directory
that is *keyed* one way and *displayed* another is how three spellings ended up
on screen at once.

#### Item 3 — canonicalization normalizes a WSL path to nothing at all

Canonicalizing is supposed to give one name to one directory. Measured with
`CreateFileW` + `GetFinalPathNameByHandleW`, which is exactly what Rust's
`canonicalize` calls — one directory, every spelling Windows accepts:

| input | `GetFinalPathNameByHandleW` | opens |
|:--|:--|:--|
| `\\wsl$\Ubuntu\home\…\warp` | `\\?\UNC\wsl$\Ubuntu\…` | yes |
| `\\WSL$\Ubuntu\home\…\warp` | `\\?\UNC\WSL$\Ubuntu\…` | yes |
| `\\wsl$\ubuntu\home\…\warp` | `\\?\UNC\wsl$\ubuntu\…` | yes |
| `\\wsl$\UBUNTU\home\…\warp` | `\\?\UNC\wsl$\UBUNTU\…` | yes |
| `\\wsl.localhost\Ubuntu\…` | `\\?\UNC\wsl.localhost\Ubuntu\…` | yes |
| `\\wsl.localhost\ubuntu\…` | `\\?\UNC\wsl.localhost\ubuntu\…` | yes |
| `\\WSL.LOCALHOST\Ubuntu\…` | `\\?\UNC\WSL.LOCALHOST\Ubuntu\…` | yes |
| `\\wsl$\Ubuntu\home\…\WARP` | — | **no**, `ERROR_FILE_NOT_FOUND` |
| `C:\dev\warp` | `\\?\C:\dev\warp` | yes |

Seven spellings of one directory, seven distinct "canonical" strings. A drive
path normalizes to its real on-disk case; a WSL path normalizes to nothing —
it is a pass-through with a prefix bolted on.

That last-but-one row is the boundary, and it is why this cannot be a blanket
`to_lowercase`: **the Linux path components are case-sensitive.** `…\git\WARP`
does not exist where `…\git\warp` does. The host and the distribution are
case-insensitive; everything after them is not.

`canonicalize_wsl_unc_path` (`warp_util::path`) is the part `dunce` cannot do,
because only Warp knows the host is the local WSL redirector rather than a
machine on the network. Host and distribution fold to lower case, the Linux
path is left exactly as given. Wired at both normal forms
(`StandardizedPath::from_local_canonicalized`, `normalize_cwd`) *and* at the
producer (`convert_wsl_to_windows_host_path`, which emitted `\\WSL$\` before),
so a path that reaches a map without passing through canonicalization is still
the same key.

Folding the distribution is safe past the filesystem too: `wsl.exe
--distribution ubuntu` and `--distribution UBUNTU` both start Ubuntu, so
`git.rs`, which parses the distribution back out of the path, is unaffected.
It already compared distributions with `eq_ignore_ascii_case`; this only makes
the map keys agree with a decision the tree had already taken.

#### Item 4 — the verbatim spelling is not a path you can hand back

Same function at the display seam (`user_friendly_path`, which every path Warp
shows already goes through), on purpose rather than a second one.

Not only cosmetic. `\\?\UNC\…` is what `dunce::canonicalize` returns, leaked —
nobody typed it, and it does not generally work if copied back out:

    cmd /c dir "\\wsl$\Ubuntu\home\effatha\git"        -> OK
    cmd /c dir "\\?\UNC\wsl$\Ubuntu\home\effatha\git"  -> "UNC paths are not supported"

PowerShell accepts both. So the fold is the difference between a string the
user can paste somewhere and one that fails when they do.

#### A third spelling, found by running it: PowerShell reports a *location*

Not on T6.1's list, because T6.1 never put a PowerShell session in a WSL
directory long enough to read its window title. `(Get-Location).Path` is
provider-qualified, and for a UNC path the qualifier is part of the string:

    C:\dev\warp        -> C:\dev\warp
    \\wsl$\Ubuntu\home -> Microsoft.PowerShell.Core\FileSystem::\\wsl$\Ubuntu\home

Warp took it literally. The most direct evidence is the OS window title, which
is set from the same string:

    MainWindowTitle: Microsoft.PowerShell.Core\FileSystem::\\WSL.LOCALHOST\Ubuntu\home\effatha\.clau…

In the chip log the only three working directories ever recorded were
`C:\Users\onemind`, `C:\dev\warp`, and the qualified WSL one — drive paths come
back bare, every UNC path carries the prefix.

Fixed in the bootstrap (`app/assets/bundled/bootstrap/pwsh.ps1`) with
`$PWD.ProviderPath`, falling back to the qualified form on the non-filesystem
drives (`Env:`, `Function:`) where `ProviderPath` is empty — Warp cannot
canonicalize either, and a literal `Env:\` is a better thing to hand it than an
empty string. Two call sites, both of which pass the string on: the `pwd` in
the precmd message and the window title.

Deliberately not changed, having checked rather than assumed: the node-version
cache key, which only compares the string with itself, and the inner runspace's
`Set-Location`, which round-trips through PowerShell's own location parser.
`Get-Item -LiteralPath` accepts the qualified form and returns a clean
`FullName`, so the node chip's directory walk was never affected.

#### Verified on Windows, 2026-08-20

`746bbc1ab`, debug build. The isolating experiment is two panes of one tab in
**one** directory reached by **two** spellings — `\\WSL.LOCALHOST\Ubuntu\…` in
the left pane, `\\wsl$\Ubuntu\…` in the right — with the project explorer open.

| | project explorer |
|:--|:--|
| without the fold | **`t6repo` twice**, each with its own `.git` and `a.txt` |
| with the fold | `t6repo` once |

That is the symptom T6.1 predicted from the source ("any two code paths that
reach the same WSL directory by different spellings hold different map keys"),
watched happening and then watched stopping.

The counterfactual build is worth describing because the confound is real:
reverting the whole commit would also revert the PowerShell fix, and *then*
neither pane produces a root at all, which proves nothing about spelling. So
the counterfactual build kept the new `pwsh.ps1` — read from disk at runtime in
a debug build, since `rust-embed`'s `debug-embed` is enabled only for wasm — and
put back only the four Rust files. Two spellings were the only variable.

Item 4 in the same window, in one line of the block header. The command typed
was `Set-Location '\\WSL.LOCALHOST\Ubuntu\home\effatha\…'`; what Warp renders
above the block is

    \\wsl$\ubuntu\home\effatha\.claude\jobs\9f032504\tmp\t6repo git:(main)

— host folded, distribution folded, `.claude`/`9f032504`/`t6repo` untouched, no
provider prefix, no `\\?\UNC\`. Before the fix the same element read
`Microsoft.PowerShell.Core\FileSystem::\\WSL.LOCALHOST\Ubuntu\home\…`.

Incidentally confirmed, having been broken in the same session: the git branch
chip and the diff-stats chip. `ShellGitBranch` went 11 executions / 11 failures,
all `phase: value`, `status: failure`, empty stdout, `exit_code: <none>`; after
the fix, `status: success` / `* main` in the same directory, and the panel shows
`⎇ main` and `1 ● +1 −1`. **The mechanism is inferred, not traced:** the absent
exit code is the shape of a process that never spawned, which fits Warp handing
the provider-qualified string to a child process as its working directory, but
the chip execution path was not read.

**Not verified: a WSL *session*.** Everything above is a PowerShell session
sitting on a WSL UNC path, which is what exercises `normalize_cwd`. The WSL-
session route runs through `convert_wsl_to_windows_host_path` instead — covered
by tests on Windows, not on screen. Reaching it needs the Settings shell
override (T6.1's recipe), which is not in `warpctrl`'s allowlisted settings.

Ten new tests; 107 pass in `warp_util` on Windows against 103 on Linux, the
difference being the `#[cfg(windows)]` ones. Three existing expectations moved
to the canonical spelling, which is the point of changing the producer.

#### And the last surface T6.1 named: drag-and-drop

Dragging a file out of Explorer's `\\wsl$\Ubuntu\…` view into a WSL session
inserted `//wsl$/Ubuntu/home/…`, because `convert_windows_path_to_wsl` knows
only about drive letters and swaps separators for everything else. Linux
collapses the leading `//`, so the shell looks for `/wsl$/Ubuntu/…`:

    $ ls '//wsl$/Ubuntu/home/effatha/git/warp/.fork/README.md'
    ls: cannot access ...: No such file or directory
    $ ls /home/effatha/git/warp/.fork/README.md
    /home/effatha/git/warp/.fork/README.md

The answer was already in `parse_wsl_unc_path`. What it needed was the
session's distribution, which `Session::windows_path_converter` could not
supply because it returned a bare `fn` pointer with nowhere to put one; it now
returns a boxed closure. A path in another distribution falls through to the
generic conversion, there being no path from inside one distribution to
another's filesystem.

Both drop seams covered — the terminal and the rich input — and the
session-level test was confirmed to fail without the fix. **Not verified on
screen, and it cannot be from here:** winit takes file drops through OLE
`IDropTarget`, which needs a real drag over the window rather than a message
that can be posted to it.

### T6.4 — decided

**Run the Linux build when your code is in WSL. Keep the Windows build for
code on `C:`.** One thing is untested rather than working: the local agent in
the Linux build — see the end of this section for what that does and does not
mean.

T6.1 argued this from the 9p numbers. What was missing was the other half —
whether the Linux build is actually usable at *current* code, since T1.11
verified it five commits and two sessions ago. Checked by running it at
`342867ee6`, under WSLg with the documented `env -u WAYLAND_DISPLAY
LIBGL_ALWAYS_SOFTWARE=1`:

| | |
|:--|:--|
| `window list` | `has_workspace: true` straight from launch |
| UI | renders fully; no grey rectangle, no onboarding wall (the flag persisted) |
| Project explorer | the whole `~/git/warp` tree, ignored dirs in italics |
| git chip | `⎇ dev  1 ● +98 −8`, native, no `wsl.exe` wrapper |
| Global search | **`blocker` typed into the live box → 173 results in 41 files** |

That search result closes the gap T6.3 recorded and could not close: "a query
typed into the live box". It could not be done on Windows, where `warpctrl` has
no action for it and raising the window would steal the user's focus. Under
WSLg it is reachable, because XTEST posts to a specific window: `warp-xin.py
click 345 113`, then one `key` per character. `_` needs a modifier and came out
as `-` on the first attempt, which is its own small proof that the box is live —
`wsl-unc` returned "No results found" and `blocker` returned 173.

And the number that decides it. Same repository — `~/git/warp`, 209,644 files —
indexed from a `cd`:

| Build | Time to a populated project explorer |
|:--|:--|
| Windows, over 9p (T6.1(d)) | still a skeleton at **10 minutes** |
| Linux, native ext4 | **populated at the first capture, 10 s** |

10 s is an upper bound, not a measurement: the first screenshot was taken at
t+10 s and the tree was already there. Two caveats stated rather than buried —
the page cache was warm from building in that tree, and the poll interval is
coarse. Neither dents a ratio of at least 60×, and T6.1's controlled tree walk
(26 ms native / 101 ms Windows disk / 1323 ms over 9p, same 2247 files) is the
clean version of the same comparison.

What the Linux build costs, all of it named:

- **Software rendering.** Measured in T1.11, not assumed: 0% CPU at idle, ~280%
  of one core while painting 50,000 lines of scrollback, back to zero within
  two seconds.
- **X11, not Wayland.** With `WAYLAND_DISPLAY` set the window is created and
  never paints. Unsetting it routes through Xwayland and works.
- **A separate profile.** Settings, themes and the Drive store do not carry
  over from the Windows install; T1.11 saw the Linux profile report 0 objects.
  This is the real switching cost, and it is a one-off.

**The local agent in the Linux build: still untested, and the first answer here
was wrong.**

What was written first — and committed — was that the agent "does not work in
the Linux profile", on the evidence that every route to it did nothing:
`warpctrl input submit` ran the prompt in bash; typing it into a
`tab create --type agent` composer and pressing Return ran it in bash
(`Command 'what' not found, did you mean: chat / phat / jhat / wham`);
`/agent …` was not intercepted; `surface agent-management open` returned `ok`
and rendered nothing. The hypothesis attached to it was that the Linux profile
has its own store and had never been through this fork's agent setup.

**The control disproves it.** Running the identical sequence on the *Windows*
build — where the local agent is known to work — produced the identical
failure, PowerShell's version of it: `what: The term 'what' is not recognized
as a name of a cmdlet`. Nothing about the Linux profile was being observed. The
sequence was simply wrong.

The route is `input replace` to put the prompt in the composer **without**
running it, then **`ctrl`+`shift`+`Return`** — the chord the agent tab's own
header advertises and which had been read past four times. On Windows that
works, and the fork's local agent answers:

    /agent what is 6 times 7 answer with only the number
           42

On Linux it still could not be sent, for a reason that says nothing about the
build: XTEST delivers plain keys to Warp — `Return` submits, and every
character of the search query above arrived — but a *modified* key does not
register with Warp's keybinding matcher, with 0.03 s or 0.2 s modifier holds.
So the agent is **untested in the Linux build**, and there is no evidence
either way. The blocker is the injection tool.

What is established, by inspection rather than by running: the Linux build
takes the *simplest* agent path. `spawn_for` runs plain `claude` with the
working directory when there is no distribution to cross, which is exactly the
Linux case; the whole `wsl.exe --distribution … --cd …` wrapper T6.1(e) had to
add exists only because Warp-on-Windows is outside the distribution. `claude`
2.1.234 is on `PATH` at `~/.local/bin/claude`, and `WARP_FORK_LOCAL_AGENT=1`
was set on the launch. Every ingredient is present and none of it was watched
working.

The generalisable bit, and the reason this is written up rather than quietly
fixed: **a failure observed only in the new environment is not evidence about
the new environment until it has been tried in the old one.** Four symptoms all
pointed at "the Linux profile", and all four were the harness.

And a correction to something this file has implied twice, now that the Linux
side has been tried properly:

> **`warpctrl` can open any surface but can only type into the terminal.**
> `input insert|replace|submit` all reach the terminal input. There is no
> action that enters a global-search query and none that sends a prompt to the
> agent composer — `action list` has `surface.conversation_list.open` and
> `surface.agent_management.open` and nothing that writes. On Windows that is
> the end of it. Under WSLg it is not: XTEST posts to a chosen window without
> touching focus, so `warp-xin.py` gets past it, which is how the search query
> above was finally typed. The wall is `warpctrl`, not the app.

**Why this is not "the Windows build is a failure".** Software rendering is a
cost paid once per frame on a machine with cores to spare. 9p is a cost paid
per file, by every index, search, diff and agent read. The Windows build is
the faster one whenever the files are on `C:` — 101 ms against 1323 ms, the
same comparison pointing the other way — and T6.2 and T6.3 were worth doing
for exactly that case. What changed is which one is the fallback.

### T6.5 — as built

`warpctrl` could open every surface and type into exactly one of them, so an
agent driving it could start a shell command and nothing else. Four actions,
all on seams that already existed:

| | |
|:--|:--|
| `agent.list` | live conversations: id, title, status, `is_busy` |
| `agent.prompt` | a prompt to a new conversation or an existing one |
| `slash.list` | the registry, with `is_orchestration`, `is_available`, `submits_prompt` |
| `slash.run` | any slash command, argument and all |

**`agent.prompt` addresses a conversation, not a pane.** That is the unit work
is handed to: the pane can be split, moved between tabs, or closed underneath
it. It returns the conversation id, which the keyboard path throws away — an
orchestrator that started three agents has no other way to tell them apart.

**`slash.run` is one action for the whole registry** because
`Input::execute_slash_command` is one function for the whole registry.

#### The allowlist

The user's call, taken as a question rather than assumed: orchestration
commands run freely, everything else needs `force`. `/exit` and `/logout` sit
in the same registry as `/compact`, and an agent should not end its own session
by mistyping a command name.

An allowlist rather than a deny-list, because the registry is upstream's and
grows: a command added tomorrow is excluded by default. A test asserts that
property rather than trusting it — it walks every command the running build has
and fails if one is admitted that is not on the list.

29 of 63 commands are admitted in this build. Off the list on purpose:
`/clear` (discards a conversation, no undo), `/auto-approve` (an agent widening
its own permissions is a decision for a person), and the account and appearance
verbs.

#### Four things found by running it

**1. `input.submit '/agent …'` was never going to work,** and neither was any
amount of cleverness about it. `input.submit` runs its text as a command.

**2. The registry stores names with the slash** — `StaticCommand::name` is
`"/compact"`, not `"compact"`. The first version stripped the slash from the
caller's input only, so every lookup missed.

**3. Which commands exist is a property of the build, not the source.**
`/compact` is behind `SummarizationConversationCommand`; `/queue`,
`/fork-from`, `/rewind`, `/profile`, `/host`, `/harness`, `/environment` behind
their own flags. In a unit-test process none of them are registered at all,
which is why the allowlist is tested against `SlashCommandKind` rather than
against the registry. It is also why `slash.list` exists.

**4. A staged prompt is not a sent prompt.** The first `agent.prompt` created
the conversation, returned its id, reported `in_progress` — and left the text
sitting in the composer. Every JSON field said success; the screenshot did not.
`try_enter_agent_view` asks the *origin* whether to submit or stage, and
`AgentViewEntryOrigin::Input` answers "only if already in agent view", which
from `warpctrl` is never. `AgentViewEntryOrigin::Cli` is `Always`, and is the
case this already had a name for.

#### And `/compact`, which is the one that does not work

`slash.run compact` reported `handled: false`. Twice, for two different
reasons, and the second is the interesting one.

First it was availability — a property of the *pane*, not the build. That is
now visible (`slash.list` reports `is_available` against the pane you target)
and `slash.run` refuses up front instead of executing into a `false`. Three
failure modes, distinguishable, which is what an orchestrator can act on:

    not in this build   invalid_params            -> slash.list has the list
    not in this pane    target_state_conflict     -> target another pane
    not allowlisted     insufficient_permissions  -> re-run with force

Then it was still `false`, and upstream's own comment says why:

> Some slash commands (e.g. /plan, /compact) return false to indicate the full
> text should be sent as a regular AI query — fall through in that case.

`/compact` is not an action. It is a prompt whose *prefix* is the instruction.
`slash.run` now falls through the same way, reconstructing `/compact
<argument>` into the conversation the pane is showing.

And then it failed for real:

    I'm sorry, I couldn't complete that request.
    Request failed with error: Other(missing authentication credentials)

Summarization is `AIAgentInput::SummarizeConversation`, and
`local_agent::handles` takes only `AIAgentInput::UserQuery` — deliberately, per
T5: "silently answering them from a local model would be worse than not
answering". So the request goes upstream, and this fork has no account. **The
plumbing is right and the feature is absent.** Tracked as T6.7.

#### Verified on Windows, 2026-08-20

`fa6f25092`, debug build, every claim from the CLI with no keyboard involved.

    warpctrl agent prompt 'Reply with exactly one word: warpctrl-ok'
      -> conversation_id 84ee4216…, created: true
      -> on screen: /agent Reply with exactly one word: warpctrl-ok
                    warpctrl-ok

    warpctrl agent prompt 'Now reply with exactly: second-turn-ok' --conversation 84ee4216…
      -> created: false, and the same conversation now has two turns

    warpctrl agent list      in_progress -> success, title taken from the prompt
    warpctrl slash run logout
      -> insufficient_permissions: refused: `logout` is not an orchestration
         command. Re-run with force if you meant it.
    warpctrl slash run copy-debugging-id --force
      -> handled: true, and the toast appears
    warpctrl slash run not-a-real-command
      -> invalid_params, naming `slash.list`

Handoff, which is the point of all of it:

| Target | Result |
|:--|:--|
| New tab | `tab.create` then `agent.prompt` — works |
| New pane | `pane.split` then `agent.prompt` — works; agent right, terminal left |
| New window | `window.create` then `agent.prompt` — same composition, untested |
| Background | not attempted; see T6.6 |

Ten tests. `warp_util` unaffected; `local_control` 40, `warp_cli` 244.

### T6.6 — scope

**One agent running several, from `warpctrl`.** T6.5 makes an agent
addressable; this makes a fleet of them manageable. The finding that shapes it:
**almost all of the machinery already exists**, because `/orchestrate` needs
it. What is missing is the `warpctrl` surface onto it.

**Background agents are real and already have a name.**
`HiddenPaneReason::ChildAgent`, in `pane_group/tree.rs`:

> Pane is a child agent spawned by an orchestrator. It stays hidden until the
> user explicitly reveals it from the status card.

That is exactly the "create the surface but set the pane to hidden" idea, built
and in use. `create_hidden_child_agent_conversation` takes a
`HiddenChildAgentConversationRequest` — parent pane, name, parent conversation,
harness, env vars, task context — and returns the conversation. The reveal side
exists too: `Event::RevealChildAgent`, `OpenChildAgentInNewTab`,
`OpenChildAgentInNewPane`.

So the work is wrapping, not inventing:

1. **`agent.spawn`** — a child conversation under a parent, with a name and a
   prompt. `--background` uses the hidden pane; `--pane`, `--tab`, `--window`
   place it visibly. All four targets the user asked for, and three of them are
   already reachable by composing T6.5 with `pane.split` / `tab.create` /
   `window.create` — `agent.spawn` is the one that makes the fourth possible
   and the other three atomic. Carries the guardrails below: `--allow-tools`
   and the depth cap.
2. **`agent.reveal <conversation> [--pane|--tab]`** — the toggle. Three events
   already exist for it.
3. **`agent.cancel <conversation>`** — stop a turn. An orchestrator that cannot
   stop a runaway child is not in charge of it.
4. **`agent.read <conversation>`** — the transcript, or the last message. The
   gap that makes the rest hard to use: `agent.list` says *that* a conversation
   finished, never *what it said*. Handing work back needs the answer.
5. **`agent.list` should report the pane and tab.** Left `None` in T6.5 — the
   fields are in the protocol and unpopulated — because it needs the terminal
   surface id mapped through the pane group. Cheap, and it is what makes
   "reveal the one that is blocked" possible.

#### Guardrails: what a child agent is allowed to do

Decided with the user, 2026-08-20, and the reason both are needed is that
**there are two different spawn paths and one lever does not cover both.**

**1. A tool allowlist per child.** The seam exists and is already load-bearing:
`RequestInput::with_supported_tools(Vec<ToolType>)` sets
`supported_tools_override`, and `generate_multi_agent_output` uses it *instead
of* `get_supported_tools`. There is exactly one caller today — passive
suggestions, which are read-only — so the mechanism is proven and has room for
a second consumer.

The vocabulary is `ToolType` in `task.proto`, 34 entries. The safety-relevant
ones are the obvious ones: `RUN_SHELL_COMMAND`, `APPLY_FILE_DIFFS`,
`EDIT_DOCUMENTS`, `CREATE_DOCUMENTS` are the write half; `READ_FILES`, `GREP`,
`FILE_GLOB`, `SEARCH_CODEBASE` are the read half. So "read-only child" is
expressible exactly, which is the case the user named.

**2. A spawn-depth cap, configurable.** Because the tool list only governs what
the *model* may do. `warpctrl` itself is a second spawn path, and a lead agent
that can run `warpctrl agent spawn` can run it in a loop regardless of its own
tool list. The cap is the backstop for the path the allowlist cannot see.

Note which of these is the stronger control, because it is not the obvious one:
**`SUBAGENT` and `RUN_AGENTS` are entries in the tool list.** Withholding them
from a child forbids further fan-out at the point the request is built, which
is a harder guarantee than a counter someone has to remember to increment. The
depth cap is the belt; the tool list is the braces, and the braces are load
bearing.

**The trap, and it would have shipped a guardrail that does nothing.** In this
fork the local agent intercepts *before* the tool list is read:

    generate_multi_agent_output(...)
        if local_agent_enabled() && local_agent::handles(&params) {
            return local_agent::generate(...)      // <- returns here
        }
        let supported_tools = params.supported_tools_override.take()...

So a tool allowlist set on the Warp side is silently ignored for every request
the local agent answers — which, in this fork, is every plain user query. The
child would be told it is read-only and would have a shell.

The fix is available rather than theoretical: `claude` takes `--allowedTools`,
`--disallowedTools`, `--tools` and `--permission-mode`, so the local agent can
honour the same restriction once the vocabulary is mapped. The safety-relevant
correspondences are clean — `RUN_SHELL_COMMAND`↔`Bash`, `READ_FILES`↔`Read`,
`APPLY_FILE_DIFFS`↔`Edit`/`Write`, `GREP`↔`Grep`, `FILE_GLOB`↔`Glob`,
`SUBAGENT`↔`Task` — which is what matters, since a guardrail only needs to be
exact about the things it forbids.

**These two ship together or not at all.** A tool allowlist that the local
agent ignores is worse than no allowlist, because it reads as a guarantee.

**And it is a guardrail, not a sandbox.** It stops the model *calling* a tool.
It does not stop a long-running shell command that a tool already started, and
it is not a boundary against a determined prompt injection — the child is still
a process on this machine with the user's credentials. Worth saying plainly in
the docs when this ships, because "read-only agent" invites the stronger
reading.

**What is *not* in scope**: making `/compact` work, which is T6.7 and about
where summarization runs, not about `warpctrl`. And dependency chains between
handoffs, which is T7 and is not a `warpctrl` feature at all.

### T6.6 — as built

Four actions, two guardrails, and one mapping that had to ship with them.
`warpctrl` now has 96 actions; `agent` has six.

| | |
|:--|:--|
| `agent.read` | the transcript, or the last N exchanges |
| `agent.spawn` | a child agent in a hidden pane |
| `agent.cancel` | stop a turn |
| `agent.reveal` | put a hidden child on screen |

and `agent.list` now fills in `pane_id`, `tab_id` and `is_hidden`, which T6.5
left as `None`.

#### `agent.read` was the piece everything else needed

`agent.list` reports *that* a conversation finished and never *what it
produced*, so before this an orchestrator could dispatch work, watch it
complete, and have no way to collect the result. That is why T7.1 was blocked
on T6.6 rather than on T6.5: a graph without this can sequence work but cannot
hand anything along it, which is half the point.

Built on `AIConversation::root_task_exchanges` and the two formatters the
copy-to-clipboard path already uses, so it reports the text a person would get
from the overflow menu.

Input and output are **separate fields**, not one `USER:`/`AGENT:` transcript.
The caller is a program; the thing it wants is the last `output`, and making it
parse a formatted transcript to find that would repeat the mistake
`input.submit` makes with `/agent`.

Tool results are **off by default**, and `included_tool_results` reports what
actually happened rather than echoing the request — they need the action model
of the surface that owns the conversation, and that surface can be closed while
the conversation survives.

#### Hidden panes: two questions, two answers

`pane.list` reports `visible_pane_ids` and is right to — a hidden pane is not
addressable as a pane. But "which pane holds this conversation" is a different
question, and for a background child the answer is a real pane that happens to
be hidden. So `agent.list` walks `pane_ids()` and reports visibility as a
field. Verified: with six conversations in one tab, `agent.list` saw all six
and `pane.list` saw the two that were visible.

`is_hidden` is reported rather than inferred from a missing `pane_id`, because
the two are different situations and only one of them can be revealed.

#### `agent.spawn` skips the server, deliberately

`/orchestrate` spawns the same thing through `StartAgentRequest` and
`launch_local_no_harness_child`, which opens with `AIClient::create_agent_task`
— an authenticated call that mints the server-side `ai_tasks` row a cloud run
is *reported* against. Account-free that fails before a pane exists, and the
row is for reporting, not for running. So `agent.spawn` calls
`create_hidden_child_agent_conversation` directly.

What that costs, recorded so it is not rediscovered: the child has no
`task_id`, does not appear in Warp's cloud task list, cannot be cancelled as a
cloud task, and a shared session it started would have no server-side run to
attach to.

#### The guardrails, and the trap that would have made one of them a lie

The tool allowlist lives in `ChildAgentToolPolicy`, keyed by terminal surface —
the same shape `apply_child_agent_model_override` uses for a child's model —
and is read by `RequestInput::new_with_common_fields`, so every turn of that
child carries it rather than only the turn that set it.

**And it governed nothing that mattered until the local agent was taught it.**
`generate_multi_agent_output` reads `supported_tools_override` *after* the
local-agent intercept, so with the local agent on, the restriction applied to
no request at all. `ai::local_agent::tools` maps the vocabulary onto
`claude --allowedTools` / `--disallowedTools`. Both halves are emitted and the
second is the one that forbids: `--allowedTools` alone leaves everything else
*prompting*, and in `--print` mode a prompt cannot be answered, so the symptom
would have been a child that hangs rather than one that refuses.

The mapping is partial on purpose and fails closed. `SEARCH_CODEBASE` grants
nothing — mapping Warp's semantic index to `Grep` because both are "searching"
would hand out a tool nobody named — and `WebFetch`/`WebSearch`, which no
`ToolType` names, can only ever be forbidden.

The depth cap is `WARP_FORK_AGENT_SPAWN_DEPTH`, default 2. It is the weaker
control and exists only because `warpctrl` is a second spawn path that no tool
list can see. It bounds depth, not breadth.

#### Verified on Linux, 2026-08-20

**The first time this fork's `warpctrl` has been driven against the Linux
build** — T6.5 was verified on Windows. `3ecf8d0bb`, debug build, WSLg,
`WARP_FORK_LOCAL_AGENT=1`, every claim from the CLI with no keyboard involved.

    agent prompt 'Reply with exactly: parent-ok'   -> conversation dcae3b26…
    agent read dcae3b26… --last 1                  -> output "parent-ok"
    agent list      pane_id "Pane Pane Terminal (4225)", tab_id 3851
                    matching what `pane list` reports for the same pane

    agent spawn 'Reply with exactly one word: child-ok'
        --name reviewer --allow-tools read-only
      -> depth 1, allowed_tools [READ_FILES, GREP, FILE_GLOB, …]
      -> agent list: is_hidden true, same tab as its parent, absent from
         `pane list`
      -> agent read: output "child-ok"

The guardrail, proved by contrast rather than by assertion — two children, the
same prompt, one restricted:

    'Run the shell command: echo GUARDRAIL_PROBE_OUTPUT — then reply with
     exactly what it printed, or say you cannot.'

    --allow-tools read-only:
      "I can't run it. There's no shell/Bash tool in this session — the
       available tools are Glob, Grep, Read, Skill, …"
    no restriction:
      ran it, and reported GUARDRAIL_PROBE_OUTPUT

The rest:

    agent spawn --parent <a depth-2 child>
      -> insufficient_permissions: would sit at depth 3, limit is 2
    agent spawn --allow-tools Bash
      -> invalid_params: `Bash` is not a tool. Use `read-only`, or a
         ToolType name such as READ_FILES or RUN_SHELL_COMMAND.
    agent cancel <in_progress>   -> was_running true; status -> cancelled
    agent cancel <same, again>   -> ok, was_running false
    agent reveal <child>         -> was_hidden true; pane appears in
                                    `pane list` beside its parent
    agent reveal <child> --as tab -> moved to a new tab, is_hidden false
    agent reveal <non-child>     -> target_state_conflict, naming `swap`
    agent reveal <child> --pane <a pane in another tab>
      -> target_state_conflict, naming the tab it does live in

Nineteen tests. App `local_control::` 61, `pane_group::` 120, `local_agent` 25,
`fork` 25; `local_control` 40, `warp_cli` 244. Full app suite 21 failures, all
pre-existing flakes in the known families — the two AI ones fail identically on
a stashed tree, and the leak test passes in isolation on both.

#### Two things only running it found

**`agent reveal <id>` failed whenever you had looked at another tab.** The pane
selector resolves inside the *active* tab, so the default target was in the
wrong pane group as soon as the person had moved. This action, unlike every
other one, already knows where its subject is, so with no selector it now hosts
the reveal from the tab that holds the conversation. The workaround otherwise —
passing both `--tab` and `--pane` — is something a caller could only learn by
hitting it.

**The allowlist could fail open.** `ChildAgentToolPolicy::handle` panics when
the singleton is not registered, so the first version guarded every call site
with `has_singleton_model` — turning a panic into a child spawned
*unrestricted*, which is exactly the failure the feature exists to prevent,
reintroduced by the fix for a different one. The spawn now checks before it
creates anything and refuses. The guard stays on the release path, where a
missing singleton means there is nothing to release.

That second one was found by `pane_group::tests::completed_shared_session_
child_with_edit_access_uses_continuation_pane` — a test with nothing to do with
any of this, which builds a narrow singleton set and started panicking in a
code path the change had walked into.

#### Not done, and why

* **`--pane` / `--tab` / `--window` at spawn time.** All three already work as
  compositions (`pane.split` / `tab.create` / `window.create` then
  `agent.prompt`) and are verified. Doing them inside `agent.spawn` would mean
  spawning hidden and then revealing, and reveal is event-driven — nothing
  comes back from emitting an event, so the combined call could not honestly
  report whether the second half worked.
* **Hiding a revealed child again.** The toggle only goes one way. Nothing in
  the reveal events reverses cleanly, and closing the pane kills the child.

### T6.7 — as built

`/compact` works account-free. The whole change is in `ai::local_agent`; no
action was added, no protocol touched, and `slash.run compact` is unchanged
from what T6.5 left.

#### The fix is not "summarize the conversation with Claude"

That was the obvious reading and it is wrong. Upstream, `/compact` summarizes
the message list the client uploads, because upstream that list *is* the
model's context. Here it is not. This fork sends Claude a prompt and Claude
keeps the transcript — that is the whole of T5's session design — so **the
context under pressure is Claude's, and compacting Warp's copy would free
nothing at all.**

So the local agent runs Claude's own `/compact` against the session it is
already holding. `Ask::Compact` is a second kind of turn beside `Ask::Query`,
and its prompt is the literal string `/compact`.

#### `/compact` works in `--print` mode, which had to be established first

Not documented anywhere, so it was run:

    echo "/compact" | claude --print --output-format stream-json --verbose \
      --resume <session>

It does, and it reports itself on events this fork had never seen:

    system/status   status: "compacting"
    system/status   compact_result: "success"
    system/init                                    <- the *second* one
    system/compact_boundary   pre_tokens: 22988, post_tokens: 2156,
                              duration_ms: 18962
    user            the summary
    user            "<local-command-stdout>Compacted </local-command-stdout>"
    result          result: ""                     <- empty

Three of those changed the design:

**The `init` is in the middle**, for the session Claude has just rewritten —
and on a *refused* compaction there is none at all. A translator that opens its
stream on `system/init`, as the query path does, would put the opening event two
thirds of the way through, or emit none and have the client report a dropped
connection. So a compaction opens on the session id `--resume` was given, which
is known before Claude says anything. Safe because the session id does not move
across a compaction — verified either side of one.

**The result is empty.** The summary is not in it. It arrives as a `user`
message, and the flag that identifies it on disk — `isCompactSummary` — is
*not* on the stream. So the summary is identified by position: the first user
message after `compact_boundary`. Everything after that is the CLI talking to
itself.

**"Not enough messages to compact" is an answer, not an error.** It comes back
as a synthetic assistant message with `is_error: false`, and is shown as
ordinary agent output.

#### What Warp is told

A `Summarization` message carrying a `ConversationSummary`, not agent output.
The difference is not cosmetic: Warp renders it as a collapsible "Conversation
summarized" block and leaves it out of a copied transcript, and only if it is
told. `token_count` is `post_tokens` and `finished_duration` is `duration_ms`.

The request is recorded as a `SystemQuery(SummarizeConversation)`, which is
what upstream writes and which `convert_conversation` deliberately does not
render as user input — without it, a restored conversation has a summary that
nobody asked for.

The summary's own preamble is stripped:

> This session is being continued from a previous conversation that ran out of
> context. …

That paragraph is a prompt addressed to the next model, and it says "ran out of
context" whether or not anything did. Under a heading that already reads
"Conversation summarized" it is misleading noise. Both halves of it must be
present before either is dropped, so a rewording upstream costs a stray line
rather than a truncated summary.

#### Verified on Linux, 2026-08-20

Six turns in one conversation, then `slash run compact`, then a seventh turn:

    agent read <id> --last 1
      -> "What words did I ask you to remember? …"
         ALPHA-7, BRAVO, CHARLIE, DELTA, ECHO, FOXTROT

which is the whole feature in one line: the conversation survived compaction
with its content intact, recalled from the summary, in the same session.

Claude's session file shows the `compact_boundary` and the `isCompactSummary`
message. Warp's own `agent_tasks` row — decoded from the protobuf — shows field
16, `Summarization`, with the preamble stripped, `token_count: 2481` and
`finished_duration: 28.809s`.

Both other paths, in a second conversation:

    slash run compact                      (one turn only)
      -> status success, output "Not enough messages to compact."
    slash run compact 'keep only the list of codewords'
      -> SystemQuery prompt: "keep only the list of codewords"
      -> summary: "Codewords to remember, in the order given: 1. ZULU …"

The instructions reached Claude and the summary obeyed them. Note the second
summary has no preamble at all — a directed compaction does not write one — and
`readable_summary` correctly left it alone.

`agent.read` shows the compaction exchange with **no input and no output**,
which is right rather than a bug: the request is a system query and the answer
is a `Summarization`, and both are excluded from the copy formatter that
`agent.read` reports through.

#### The half-hour this cost, so it costs nobody else that

Every AI slash command reported `is_available: false`, including `/agent`, in a
pane that was demonstrably running an agent. The cause is not in this fork's
code: `agents.warp_agent.is_any_ai_enabled = false` in `~/.config/warp-oss/
settings.toml`. `Availability::AI_ENABLED` is gated on it, so the entire slash
menu goes dark — while `warpctrl agent prompt` keeps working, because it does
not consult that setting.

Worth knowing in both directions. The fork's account bypass
(`fork::account_gate_bypassed`) covers the *account* half of
`is_any_ai_enabled` and cannot cover the stored value, which is the user's own
switch. And `agent.prompt` bypassing it is left alone deliberately: enforcing
it there would remove function rather than add it, which is the wrong direction
for this fork.

Verified against a scratch profile — `XDG_CONFIG_HOME` pointed at a copy with
the one flag flipped — so the user's own `settings.toml` was never edited.

#### Not done, and why

* **Reporting `context_window_usage`.** Claude gives the numbers on every turn
  (`input + cache_read + cache_creation` against `modelUsage.contextWindow`),
  and the agent input footer draws an icon from them, so compaction could be
  made *visible* rather than merely effective. It is a separate feature from
  "make `/compact` work", though, and doing it only on compaction would make
  the meter appear once and then vanish — worse than not doing it.
* **Warp-side message replacement.** `MoveMessagesToNewTask` is upstream's
  mechanism for shrinking the client's own copy. Nothing here needs it: the
  client's copy is not what feeds the model, and upstream's handler is explicit
  that it leaves the UI unchanged.

## T7 — Work that outlives a turn

Raised by the user, 2026-08-20, as three ideas that felt related and were hard
to separate: telling a child agent what it may do, telling child agents what
order to run in and who to hand results to, and planning a release a year out.
The first is a guardrail and belongs in T6.6. The other two are the same shape
at two scales, and **the scales want different homes**. This section is the
argument for which.

### The runtime already exists. The *plan* does not.

Worth establishing first, because it changes what is left to build. Warp can
already do all of this at runtime — these are entries in `ToolType`:

    RUN_AGENTS = 32              spawn a batch of children
    SUBAGENT = 16                spawn one
    SEND_MESSAGE_TO_AGENT = 27   pass a result to a named agent
    WAIT_FOR_EVENTS = 33         block until something arrives

with `AIAgentInput::MessagesReceivedFromAgents` and `EventsFromAgents` on the
receiving side, and `ConversationStatus::WaitingForEvents` as the visible
state. That is a message-passing concurrency substrate, and "B waits for A,
then A hands B its result" is expressible in it today.

So what is missing is not the mechanism. **It is that the sequencing is a
decision the model makes in the moment, rather than a declaration made before
the run.** That is the whole of the user's instinct that this should be "more
deterministic than saying plan-then-spawn-a-reviewer, but not programmatic in
the same way". The difference between the model *choosing* to wait and the
plan *saying* it must is the feature.

### Two edges, or one?

The user described `depends-on` (ordering) and "dictate what and where the
child hands off to" (routing) as separate ideas. They collapse:

> **A dependency is an edge that carries a payload.** `hands-to` is
> `depends-on` plus "and here is what to pass".

One edge type with an optional payload spec covers both, and the collapse is
worth taking because two edge types would have to be kept consistent — a graph
where B `hands-to` C but C does not `depends-on` B is a bug you can draw.

Which makes the unit:

    node: id, prompt, agent/harness/model, tool allowlist, working directory
    edge: from -> to, optional payload ("pass your diff", "pass your summary")

Nodes are the T6.6 spawn parameters, exactly. **`RunAgentsAgentRunConfig`
already carries `name`, `prompt`, `title` and a per-child `model_id`** — so the
node is two fields short (tool allowlist, cwd) of something that exists.

### Where the graph lives, and why not in Warp

Three candidates. The middle one is what happens today, and it is the one that
fails.

**In Warp, as a model.** Rejected. Warp's job is running agents; holding your
project plan is not that. And it dies with the process — the plan would not
survive a restart, which is the one thing a long-horizon plan must do.

**In the lead agent's context.** This is the status quo, and it is why the
question gets asked. The plan lives in the context window, so it degrades
exactly as the work gets long enough to need it. **This is the real reason
`/compact` matters** (T6.7): compaction is the moment the plan is most at risk,
and no amount of careful summarization makes a context window a durable store.

**In a file, with a small runner.** Recommended. The plan is a document in the
repository; a runner — an external process, or the lead agent in a loop — polls
`agent.list`, fires `agent.spawn` when a node's dependencies are satisfied, and
reads results with `agent.read`. Everything it needs is T6.5 plus T6.6.

The argument for the file, beyond durability: it is diffable, reviewable, and
lands in a commit next to the work it describes. And **the fork is already
doing this by hand** — `.fork/TASKS.md` is a dependency graph in prose, with
T6.1 blocking T6.2 and T6.5 opening T6.6 and T6.7. Making that machine-readable
is the entire feature, and the fact that it was written by hand first is
evidence the shape is right.

Note what this buys that is easy to miss: **the plan surviving compaction is
the actual requirement**, not the scheduling. Scheduling is a `while` loop over
statuses. Durability is the hard part, and a file solves it completely.

### And the year-long version: don't build it

The user's EOY26 example — many workstreams, some parallel, some blocking — is
the same graph and a different system, and the tell is the failure mode:

| | Run-scale | Project-scale |
|:--|:--|:--|
| Horizon | minutes to hours | weeks to months |
| Lives in | one session | many, and outlives all of them |
| A node fails and | you retry it in 30 seconds | someone renegotiates a date |
| Readers | one lead agent | people, in a meeting |

Those want different storage, different UI, and different notions of "done".
Merging them produces something that is a bad scheduler *and* a bad issue
tracker.

**Recommendation: the project tracker owns *what and when*; the orchestrator
owns *how, right now*.** GitHub Issues, Linear or a milestone file already do
the first, adequately and — the part a home-grown one cannot match — somewhere
people already look. The join is one sentence of the lead agent's job:

> Given this milestone, emit a run-scale graph.

That keeps the run-scale graph small enough to be worth trusting, and keeps the
project-scale record somewhere a human can argue with it.

- [x] **T7.1** A run-scale task graph: schema, and a runner over T6.5/T6.6.
      Unblocked by T6.6, which supplies `agent.spawn`, `agent.read` and
      `agent.cancel` — without `agent.read` a graph can sequence work but not
      hand anything along it, which is half the point. Shipped as
      `warpctrl graph`, adding no actions — see "T7.1 — as built" below.
- [x] **T7.2** Read a milestone from an issue tracker and emit a T7.1 graph.
      Deliberately last, and deliberately thin: the moment this grows a
      scheduler of its own it has become the thing this section argues against.
      It stayed thin — one command, `graph schema` — because running it showed
      that the interesting half cannot be parsed. See "T7.2 — as built" below.

### T7.1 — as built

`warpctrl graph check <plan.toml>` and `warpctrl graph run <plan.toml>`.

**The catalog is 96 actions before and after.** That is the headline. T6.6
built the verbs; a graph is a composition of them, so the runner is a `while`
loop in the CLI over `agent.spawn` and `agent.read` and the app gained nothing.
A feature that adds no surface is one that cannot break the surface.

#### The format

```toml
[defaults]
allow_tools = ["read-only"]

[[node]]
id = "survey"
prompt = "List every file under src/ that still calls the old API."

[[node]]
id = "fix"
prompt = "Migrate those files to the new API."
allow_tools = ["read-only", "APPLY_FILE_DIFFS"]
needs = [{ node = "survey", pass = "the list of files" }]
```

A node is the `agent.spawn` parameters — prompt, name, tool allowlist — and
nothing else. `[defaults]` exists so a plan whose every node is read-only says
so once: a plan that repeats the restriction on every line is a plan where one
line will eventually be missing it.

**One edge type**, as the T7 argument concluded. `needs = ["survey"]` is
ordering; `needs = [{ node = "survey", pass = "…" }]` is ordering *and* a
handoff. The `pass` phrase is a label rather than a filter — the whole of the
upstream answer is appended either way, under `--- From \`survey\` (the list of
files):`, because a wall of text under no heading is the kind of context an
agent quietly ignores.

Edges are written on the node that waits, because that is the direction a
reader asks the question in: standing at `fix`, what does it need?

#### What is refused before anything spawns

Both of these are otherwise discovered halfway through a run, with children
already running and a partial result to reason about.

* **A cycle**, with its members named. At runtime a cycle is invisible — the
  scheduler stops finding work, which reads exactly like a hang. Found by
  Kahn's algorithm, where what *cannot* be drained is the answer.
* **A misspelled field**, via `deny_unknown_fields`. This matters more here
  than in most places, because the fields are the guardrails:
  `allow_tool = ["read-only"]` silently accepted is a node that runs with no
  restriction at all, discovered by reading what it did.

Also refused: an unknown node in a `needs`, a duplicate id, a node that needs
itself, an empty plan.

`graph check` needs no running Warp and prints the plan in waves:

    4 nodes, 3 in sequence
      1. colours
      2. count, shout
      3. report

Waves rather than a flat list because the parallelism is the interesting part —
a plan whose every node is in its own wave will run one agent at a time, which
is usually not what its author drew.

#### Failed is not skipped

A run that reports six failures when one node failed and five were waiting on
it has buried the only fact worth acting on. So `Skipped { blocked_by }` is its
own state, and it names the *nearest* blocker; the chain back to the root cause
is readable from the other entries.

A failed node stops its own dependents and nothing else. Branches with nothing
wrong with them run to completion, and the process exits non-zero at the end.

#### The race that would have made the whole thing silently useless

A conversation polled in the instant after `agent.spawn` is not busy *yet*. Read
as "finished", it hands the next node an empty string, and the graph runs to
completion having done nothing — with every node reporting success. The test is
therefore `!is_busy && last_exchange.is_complete == Some(true)`: an exchange
that exists and has a finish time cannot be a turn that has not started.

#### Verified on Linux, 2026-08-20

A diamond, run against a lead conversation with `--parent`:

    colours: done — Crimson, teal, amber
    count:   done — 3                              <- these two started
    shout:   done — CRIMSON, TEAL, AMBER              together
    report:  done — COUNT=3 LIST=CRIMSON, TEAL, AMBER

`report` joining two handoffs into one line is the proof that both edges
delivered. `agent.list` afterwards showed all four as hidden children of the
lead.

Then the part that matters more, with a node given `allow_tools = ["Bash"]` —
which is Claude's name for it, not Warp's, so `agent.spawn` refuses:

    bad:       failed — `Bash` is not a tool. Use `read-only`, or a ToolType
                        name such as READ_FILES or RUN_SHELL_COMMAND.
    after-bad: skipped — `bad` did not finish
    unrelated: done — still-here
    exit=1

#### Decisions worth recording

* **`--max-parallel`, default 4.** Every node is a real agent with a real model
  behind it. The graph decides what *may* run together; this decides how much
  of that actually does.
* **No timeout by default.** A node can legitimately sit for a long time —
  `blocked` means it is waiting for a person to approve something — and killing
  it throws the work away to no purpose. `--timeout` is there for unattended
  runs, and the progress output carries the conversation's own status so a
  stuck node reads as `blocked` rather than as nothing happening.
* **No retries.** A node fails once and stays failed. Retrying an agent turn is
  not idempotent — it may already have written files — so it is the plan
  author's call, not the runner's.
* **`agent.read --last 1`.** A node is one prompt and its answer. Reading the
  whole transcript would hand the next node the handoff it already received,
  wrapped in its own reply.

### T7.2 — as built

One command, `warpctrl graph schema`, and a verified loop. The reason it is
one command rather than a `graph from-issues` parser is the finding below,
which is the whole of this task.

#### A real milestone contains no edges. None.

This was scoped by reading actual trackers rather than by imagining one.
`mycosavant/warp` has issues disabled — the GitHub default for a fork — so the
tracker read was upstream's, which is real and public. Two milestones, 21
issues, checked for every convention a parser could key on:

| Convention | Hits |
|:--|:--|
| `depends on #N` / `blocked by #N` / `requires #N` | 0 |
| Task-list issue references (`- [ ] #N`) | 0 |
| Sub-issue / dependency links | 0 |

The only task-list checkboxes that matched at all were `- [x] Yes`, the "have
you searched for existing issues?" box in the bug-report template.

And the bodies are not task specifications. They are user-submitted prose in an
issue template — `### Discord username (optional)`, `### Describe the bug`,
`### To Reproduce`. A milestone named `SSH V2` is seventeen unrelated SSH bug
reports; `Emacs` is four unrelated Emacs ones.

**So a mechanical generator would emit N nodes and zero edges, every time.**
It would be deterministic and it would be confidently wrong about the only
part that matters, because the ordering information is not in the tracker to
extract. That vindicates the T7 argument exactly: the tracker owns *what*, and
it genuinely does not own *when* in any machine-readable form. Deciding what
depends on what means reading prose and exercising judgment, which is an
agent's job and not a parser's.

Hence no `graph from-issues`. Fetching is already one `gh` command; wrapping it
would add a dependency on a tracker's shape in exchange for the boring half.

#### What was actually missing

That the agent had no way to learn the plan format except a human pasting
documentation into its prompt — which is the human this was supposed to
remove. `warpctrl graph schema` prints the format as an annotated plan that is
itself valid and runnable, asserted by a test rather than claimed in a comment.

It leads with the thing a generating agent gets wrong: a child does not inherit
the transcript of whatever wrote the plan, so every prompt has to stand alone.
That mistake is invisible until the run produces confidently context-free
answers.

#### The loop, end to end, on a real milestone

One sentence of the lead agent's job, exactly as the T7 argument predicted, with
no format pasted into it — the agent was told to run `warpctrl graph schema`
itself:

> Read the `Emacs` milestone of `warpdotdev/warp`. Run `warpctrl graph schema`
> to learn the plan format. Emit a task graph that triages that milestone: one
> node per issue that summarises the bug and names the area of a terminal
> emulator it touches, and a final node that proposes an order to fix them in.
> Write it to `triage.toml`, validate it with `warpctrl graph check`.

One turn, four tool calls — `gh`, `graph schema`, `Write`, `graph check` — and
`OK`. The plan it wrote:

    5 nodes, 2 in sequence
      1. issue_1860, issue_2064, issue_402, issue_450
      2. order

with `allow_tools = ["read-only"]` inherited from the schema's default, each
issue's full text embedded in its own prompt (the standalone rule, followed),
a fixed reply format so the join could read them, and four labelled handoffs
into `order`.

Then `graph run`, which ran the four triages in parallel and joined them:

    1. #1860 — M-backspace sends M-b "ackspace" — highest severity, contained
       to the Option-as-Meta encoding path.
    2. #2064 — Command key combo not going to Emacs — same keyboard layer, so
       the person is already in the key-dispatch code.
    3. #402 — split pane doesn't draw properly — equally severe but sits in
       the grid/renderer, a separate area.
    4. #450 — ctrl+z shown as an error — cosmetic, isolated, natural cleanup.

    RATIONALE: … if two people are available, #402 should run in parallel with
    the #1860/#2064 pair, since the code is disjoint.

Worth noticing what the edges in that plan *are*. The milestone had none, and
the agent did not invent dependencies between the bugs — it drew edges between
the units of **work it proposed to do**: analyse each, then read the analyses.
The tracker supplied the nodes; the agent supplied the shape. That is the
division the T7 argument asked for, arrived at without being told.

And the last line of the rationale is a graph: the join node reasoned about
what could run in parallel, which is the plan it would emit next.

#### Two things running it turned up

* **`XDG_CONFIG_HOME` broke `gh`.** The scratch-profile trick from T6.7 —
  used to get `is_any_ai_enabled = true` without editing the real
  `settings.toml` — also relocates every other XDG-config tool, `gh` included,
  so the agent found itself unauthenticated. A rig defect, not a product one,
  and it did not need the rig at all: `agent.prompt` and `graph` never consult
  `is_any_ai_enabled`, so only T6.7's slash-command path needed the override.
* **One turn came back `cancelled`, and it is not explained.** The first
  attempt — the one with `gh` broken — ended `ConversationStatus::Cancelled`
  after four tool calls, with nothing in the log. Nothing cancelled it from
  this side. `Cancelled` comes from a real `StreamCancellation`, and the only
  `CancellationReason` that could plausibly fire unprompted is
  `UserCommandExecuted`: the pane's shell is live while the agent streams. That
  is a candidate, **not a diagnosis** — it did not recur, and it was not
  reproduced. Recorded as T5.5 rather than guessed at.

---

## T8 — The app you actually use  ← NEXT

Phases 0–7 made the fork *correct*: no telemetry, no account, your agents on
your keys, all of it measured rather than asserted. T8 is the first phase aimed
at making it **pleasant**, and it starts from a release build that exists to be
lived in rather than tested.

The full brain dump, all fourteen ideas, with what already exists under each
and an argument for or against, is in **`.fork/IDEAS.md`**. That file is the
holding pen; this section is only what has been selected out of it.

Selection rule, unchanged from the rest of this board: value per line of code.
The five below were picked because in every case **the mechanism already exists
and is wired to the wrong thing** — which is the same finding that has driven
T1, T4, T5 and T7, and by now should be the prior rather than the surprise.

- [x] **T8.0** Linux release build, for daily use rather than for tests.
      `cargo build --bin warp-oss --features gui,warp_control_cli --release`.
      Verified 2026-08-21 by running: `--warpctrl` dispatches out of the
      release binary, the GUI window opens, discovery registers, and
      `window list` / `tab list` / `action list` all answer. `--release` means
      `debug_assertions` is off, so `UserInput`'s redaction is live and the log
      stops carrying what you typed (see "Your development build's log contains
      what you typed" in `README.md`).

- [ ] **T8.1** Quake visor for the lead agent. (`IDEAS.md` I8)
      Quake mode is a finished, cross-platform feature nobody has pointed at an
      agent: `GlobalHotkeyMode::QuakeMode`, `toggle_quake_mode_window`
      (`root_view.rs:1479`), `WindowStyle::Pin` handled in the winit backend.
      `PanesLayout` already has an `AmbientAgent` variant.
      **The blocking question is answered: quake mode works on Linux under
      WSLg.** Bound to `ctrl-shift-Q` via Settings → Features → Global hotkey →
      "Dedicated hotkey window", it opens — confirmed by `warpctrl window list`
      going from one window to two, by X11 geometry that is unmistakably quake
      (`(-32,-32) 1451x324`, full width and top-anchored), and by a screenshot
      of that window showing a complete workspace. The X11 global grab works
      under XWayland when Warp is launched with the `env -u WAYLAND_DISPLAY`
      recipe, so the doc comment's "thanks to it using an AppKit NSPanel" is
      misleading about platform support.

      What remains is small: the quake window opens a *terminal*, and the visor
      should open an *agent*. `PanesLayout` already has an `AmbientAgent`
      variant and `toggle_quake_mode_window` picks the layout at `add_window`
      time, so this is a setting plus a match arm.

- [ ] **T8.2** Tab → pane drag, with a drop target you can see. (I3)
      Quadrant split-on-drop is *implemented*
      (`pane_group/pane/view/header/mod.rs:853`, with an ASCII diagram) and the
      tree surgery exists (`tree.rs:260`). Two things make it feel unintuitive,
      and both are precise: the drag source is the **pane header** rather than
      the tab, and the split is emitted from `PaneHeaderDragged` rather than
      `PaneHeaderDropped`, so the layout reflows live under the cursor and
      there is no preview distinct from the result. Split preview from commit,
      add the tab as a drag source, and add right-click → Split.

- [ ] **T8.3** The thread inbox, and `settled`. (I1)
      `ToolPanelView::ConversationListView` already exists with 2,198 lines
      behind it, and `AgentConversationEntry` already carries every field an
      inbox row wants. `settled: bool` mirrors the existing
      `AgentConversationData.pinned` — a JSON blob column, so **no migration**.
      **The trap:** `MAX_PERSISTED_CONVERSATION_COUNT = 200` with tree-wise
      eviction (`persistence/agent.rs:41`). An archive that evicts is not an
      archive; exempt settled rows or the feature silently loses work at
      conversation 201.

- [ ] **T8.4** Pin what a tool claims to be. (I11)
      Hash each MCP tool's `(name, description, input_schema)` at connect,
      store it, and say so when a digest changes under an existing name. This
      is the tool rug-pull defence: a server can rewrite the prompt the model
      reads, after you approved it, and nothing currently notices. `sha2` is
      already a workspace dependency — **no new dep**, and no blake3. Warn in
      v1; do not auto-block until the noise level is known.

- [ ] **T8.5** A main pane, and the CWD following it. (I13 + I6)
      One `Option<PaneId>` on `PaneGroup`, set from the pane's overflow menu,
      `None` meaning today's behaviour. Then consumers one at a time: CWD
      follow first (`working_directories.rs`, `startup_directory.rs`,
      `ActiveFileModel` all exist), layout second, orchestration third.
      **This supersedes the "follow the focused pane" scoping**, which was
      wrong: a pane that merely has focus makes the file tree thrash every
      time you glance at a split. A pane you *named* is stable by
      construction. `main` is also the natural answer to a question T6.6 and
      T7.1 both leave implicit — which pane is the lead agent.

- [~] **T8.6** WSL as a remote target, the way Zed does it. (I16)
      Promoted off the idea board because it is mostly built. Zed treats WSL as
      a remote host — Windows client, headless server inside the distro — so
      files, language servers and terminals live on the fast side of 9p while
      themes, rendering and keymaps never leave Windows.

      Built and verified:

      - [x] `WslTransport`, all seven `RemoteTransport` methods
            (`app/src/remote_server/wsl_transport.rs`) plus its command layer
            (`crates/remote_server/src/wsl.rs`). **A credential-free
            `Initialize` handshake has completed over `wsl.exe`** — daemon
            spawned, stdio bridged, `InitializeResponse` with a real host id.
      - [x] The account question, settled by running it: nothing on the path
            needs one. `handle_initialize` stores the bearer token and replies
            without validating it, the only credential check in the daemon is
            scoped to remote codebase indexing, and the proto documents
            `user_id` as "Empty when not logged in".
      - [x] The gate: `RELEASE_FLAGS` behind `cfg!(feature = "release_bundle")`,
            so the whole stack is compiled into every self-built binary and
            switched off. Opened via `fork::FORCE_ENABLED`, which outranks the
            `#[cfg(not(windows))]` on the same flag — see the note under "Look
            for the gate first" in `../CLAUDE.md`.
      - [x] `remote.wsl.list` and `remote.wsl.connect` (catalog 100 → 102), a
            command-palette entry sharing one `start_wsl_remote_server` helper
            with them, and the pane → session resolver that `connect` needed.
      - [x] Server binary and install path: staged locally at the bare OSS
            path, so `check_binary` short-circuits the CDN fetch this fork
            deny-lists. **The absent install prompt is the success signal.**

      Remaining: **run it on Windows.** The runbook is in `README.md` under
      "Warp's remote server, in a WSL distribution". Everything above was
      verified on Linux, where `wsl.exe` is reachable through interop — which
      exercises the same code path a Windows client would, but is not the
      arrangement the feature is for.

      Then the ambient path. `wsl` is *already* a warpify subshell command on
      Windows, so a WSL session gets warpified exactly as an `ssh` one does;
      what it does not get is a remote server, because that attach is keyed on
      `IsSSHWrapperSession::Yes`, whose payload is a ControlMaster socket path
      a WSL session cannot have. A WSL arm beside the SSH one is the work, and
      `Session::wsl_name()` already carries the distribution.

### Deliberately not selected, and why

Recorded here because the arguments matter more than the verdicts; the full
version of each is in `IDEAS.md`.

* **Context pruning / relevance scoring** (I9) is the most interesting idea on
  the list and the one most likely to be built wrong. T5.2 already established
  that the client holds the entire transcript and re-sends it every turn
  through one function, so this is a *filter*, not a port of
  `vscode-prompt-tsx`. The caching answer is concrete: pruning invalidates the
  cache from the first pruned message onward, so **prune rarely and in large
  chunks** — sixty small prunes cost sixty uncached turns for the same tokens
  removed. But nobody here has measured what a long conversation actually
  contains, and pruning the wrong thing does not throw, it silently degrades.
  **First task is an inspector, not a pruner.**

* **Integrated browser** (I10) is argued against on this fork's own terms.
  There is no web engine anywhere in the tree, and adding one introduces a
  second network stack outside `crates/http_client/src/egress.rs` — which is
  the thing the "nothing escapes" measurement rests on. The claim would become
  conditional the day a webview lands. The `monitor` third of the idea already
  exists as `network_log_pane.rs`.

* **Per-pane zoom / font size** (I4) needs a count before a scope: the running
  build reports exactly one `appearance.text.font_size` and one
  `appearance.window.zoom_level` for the whole app.

* **Computer use** (I15) is the strongest unselected item and was not in the
  brain dump at all — it turned up while costing the browser question.
  `crates/computer_use` is a complete screenshot / input / window-enumeration /
  video-recording stack with mac, windows, X11 and Wayland implementations, an
  XInput2 MPX "agent seat" that drives a window without stealing the cursor,
  agent tools (`use_computer.rs`, `request_computer_use.rs`,
  `start_recording.rs`) and a manual CLI. `FeatureFlag::LocalComputerUse` is in
  **`DOGFOOD_FLAGS`** — the same list `WarpControlCli` was in before T1.1 — and
  its meaning is exactly this fork's thesis: without it, computer use runs only
  when the agent is sandboxed in someone's cloud. Two gates again, runtime flag
  and cargo feature, the T1.1/T1.2 shape. Wants its own scope.

* **Composer** (I2) and **`view-as`** (I7) are waiting on specifics — the first
  from a week of use, the second until the gripe resurfaces.

### The WSLg input wall is narrower than T5.4 recorded

Tested 2026-08-21 against the release build on X11, using
`cargo build -p computer_use --bin use_computer`. Two findings, one of which
corrects the record.

**Window-targeted screenshots work.** A 1400×693 PNG of the Warp toplevel,
fully rendered — not the black frame the Wayland path gives. `use_computer
windows` finds the toplevel and its bounds; `pid`, `class` and `title` come
back empty, which is the known Weston reparenting quirk rather than a defect.

**Keystrokes still do not land** — tried window-targeted, screen-targeted,
after a click, and after explicitly setting X input focus.

But T5.4 says:

> `XGetInputFocus` returns `None` and `XSetInputFocus` does not stick

and **half of that is wrong**. `XSetInputFocus` on the Warp toplevel *does*
stick; `XGetInputFocus` reports the window back immediately after. And its
default return is not `None`, it is `0x438` — the **root window**. "Focus is
nowhere" is true in effect, but it is a different fact, and the operation
assumed impossible works.

So the wall is: pointer motion arrives (hover states appear under a synthetic
cursor), X focus can be set and holds, and **activation and key delivery still
do not happen** — which points one layer above X focus, at winit's own focus
tracking or XWayland's activation model, rather than at "WSLg cannot do input".

Worth an afternoon, because **two blocked items sit behind it**: T2.5's audio
egress test and T8.1's quake-mode press. Neither needs a person if this comes
loose.

#### Follow-up 2026-08-22: it is not Warp, and X11 is exhausted

The afternoon above was spent, with a control that removes Warp from the
picture entirely — `xev`, a trivial X11 client that prints every event it
receives.

Setup: `xev` running and logging, `XSetInputFocus` pointed at its inner window,
focus confirmed two ways (`XGetInputFocus` reads it back, **and `xev` itself
logs `FocusIn`** — so events demonstrably flow to this client).

Then both synthetic-input mechanisms X11 offers:

| mechanism | server accepted it | `xev` saw it |
|---|---|---|
| **XTEST** (`xtest_fake_input`, what `computer_use` uses) | yes | **no** |
| **XSendEvent** (synthetic event straight to the window queue) | `rc=1` | **no** |

**So synthetic keystrokes reach no X11 client at all under WSLg** — not Warp,
not a 200-line event printer. This was never a winit or Warp problem; it is
XWayland/Weston, one layer below every application.

Two consequences:

* **`xdotool` cannot help**, and neither can `wmctrl`. `xdotool key` is XTEST
  and `xdotool key --window` is XSendEvent — precisely the two rows above. Do
  not install them expecting this to move.
* **X11 is exhausted as an approach.** What remains is either below the display
  server (`ydotool`, which writes to `/dev/uinput` as a kernel-level input
  device, so Weston would see it as real hardware) or beside it (drive the
  **Windows** build with `C:\dev\keys.ps1`, which already works — see
  "Driving the Windows build from WSL" in `README.md`).

The narrower claim from the section above still stands and is worth keeping:
`XSetInputFocus` does stick, and `XGetInputFocus` returns the root window
rather than `None`. Both of T5.4's stated facts are wrong; its conclusion was
right for a reason nobody had identified.

#### Decided: stop trying to fix WSLg input

`ydotool` was installed and does not close the gap either. Ubuntu 24.04's
package ships `/usr/bin/ydotool` and **no `ydotoold`**, and `/dev/uinput` is
`crw------- root root`, so it aborts with `failed to open uinput device`.
Reaching it would mean building the daemon from source and granting uinput
access — for a mechanism that still might not be picked up by Weston.

#### First, a correction: three input paths, and only one is broken

An earlier version of this section said Zed's imperative removes "hotkeys,
cursor, input synthesis and rendering" as problems, which reads as though
keyboard shortcuts are broken under WSLg. **They are not**, and conflating
these three cost a wrong claim:

| path | what it is | under WSLg |
|---|---|---|
| **App keybindings** | a person presses `ctrl-shift-c`; the compositor routes it to the focused window | **works** — always has |
| **Global hotkeys** | a system-wide grab that fires when Warp is *not* focused (`GlobalHotKeyManager`, X11-only) | **untested** — see T8.1 |
| **Synthetic injection** | an agent fabricating key events via XTEST or XSendEvent | **broken**, per the measurements above |

Everything in this file about "keystrokes not working" means the third row.
T5.4 was about driving the GUI *from an agent*, and that framing should be
preserved whenever it is quoted.

The second row is the interesting one and nobody here has ever tested it,
because testing it requires a person to press the key — the exact thing the
third row rules out. Settings → Features → Global hotkey exposes it
(`Disabled` by default, then `Dedicated hotkey window` = quake, or
`Show/hide all windows`).

#### The decision itself

**Borrow Zed's imperative** — do not run the GUI inside the distro. This is not
because shortcuts break; it is because file I/O crosses 9p per file, and
because agent-driven verification of anything with pixels needs synthetic input
that WSLg will not deliver. This fork has confirmed both halves of the second
point: synthetic keys reach nothing under WSLg, and the Windows build's
`C:\dev\keys.ps1` already posts keystrokes to a window without stealing focus.

Consequences, all of them simplifications:

* **T2.5** (audio egress) moves to Windows. The rig is already written down.
* **T8.1** (quake mode) moves to Windows, where `RegisterHotKey` has the fewest
  ways to fail — which was already the recommendation.
* **I16 stops being one feature among fifteen.** It is the architectural answer
  to the whole category: run the client on Windows, keep the code in WSL, and
  the input problem is not solved so much as never encountered.

**Synthetic clicks still work**, and remain useful — two quit-confirmation
dialogs were dismissed with `use_computer click` during the T8 remote-server
run, which is how those sessions were closed without killing the process.

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

- [x] ~~Log spam on window move (`workspace:save_app` per window event).~~
      **Measured, and the premise was wrong. Nothing to silence.** See "The
      log-spam question, answered by counting" below.
- [ ] Windows Developer Mode so `.claude/skills` resolves as a symlink on the
      Windows checkout.
- [x] ~~Proxy-based verification that nothing escapes under real activity — only
      idle runs observed so far.~~ **Done, two ways, with a control for each.**
      See "Nothing escapes: measured, not argued" below.

### Nothing escapes: measured, not argued

The fork's headline claim, and until now the least verified thing in it — every
previous observation was of an idle app. Done properly on Linux, under real
load, by two methods that fail in different ways.

**Method 1 — every socket the process opens.** `ss -tnp state all` plus
`ss -unp`, polled five times a second for the life of the run. Proxy-independent:
an app that ignored `HTTP_PROXY` could not evade it, and `state all` includes
`SYN-SENT`, so even a connection that is *attempted and refused* would appear.

    workload: every panel and modal toggled; theme and appearance;
              setting list/get; tab create; pane split; a shell command
              via input.submit; drive object create x3 (folder, workflow,
              notebook); drive status; slash list; a full local-agent turn;
              then ten minutes idle

    7918 poll samples, ~25 minutes of uptime

    every socket warp-oss held, for the entire run:
      LISTEN 127.0.0.1:9282    local control
      LISTEN 127.0.0.1:33711   local control

**Two loopback listeners. Zero outbound TCP. Zero UDP** — so not even a DNS
lookup: warp-oss never resolved a hostname, let alone contacted one.

*The control that makes that negative mean something.* A poller that detects
nothing is worthless unless it can detect something, so the same poll was run
watching the `claude` child during a turn:

    ESTAB 172.22.45.116:48878 160.79.104.10:443  users:(("claude",pid=383426,…))
    ESTAB 172.22.45.116:48892 160.79.104.10:443  users:(("claude",pid=383426,…))
    ESTAB 172.22.45.116:48908 160.79.104.10:443  users:(("claude",pid=383426,…))

`160.79.104.10` is `api.anthropic.com`. The method catches real traffic; the
traffic belongs to the child process, on the user's own subscription, which is
exactly the design. **warp-oss appears nowhere in that list.**

**Method 2 — a decrypting proxy, because polling samples.** Method 1's gap is
real: a beacon that opens, POSTs and closes inside 200ms could fall between two
samples. So the whole run again behind `mitmdump` on loopback, with its CA
trusted so TLS succeeded and bodies were readable rather than merely counted.

    every server connection the proxy saw, whole session:
      example.com:443        <- the curl that proved the proxy captures at all
      api.anthropic.com:443  <- GET  /v1/mcp_servers?limit=1000
      api.anthropic.com:443  <- POST /v1/messages?beta=true

    grep -icE 'warp\.dev|firebase|googleapis|segment|rudder|datadog|sentry|
               amplitude|posthog|mixpanel|statsig|bugsnag|crashlytics'
      -> 0

Two requests in the entire session, both the agent turn, both the `claude`
child. The proxy log is 105 lines.

**Why two methods and not one.** Each covers the other's blind spot, and
neither is sufficient alone:

| | misses | covered by |
|:--|:--|:--|
| `ss` polling | a connection shorter than the poll interval | the proxy, which sees every request |
| proxy | anything that ignores `HTTP_PROXY` | `ss`, which sees the socket regardless |

`ss` says there were no sockets; the proxy says nothing was requested. Together
they close both doors. The `claude` child routing through the proxy also proves
the environment was honoured by the process tree, so "nothing in the proxy log
from warp-oss" is not simply "warp-oss ignored the proxy".

**Why there is nothing to send, from the log rather than the source.** The
startup line names the channel config:

    channel: Oss, … telemetry_config: None, autoupdate_config: None,
    crash_reporting_config: None

Structurally absent rather than flagged off. `server_root_url`, the RTC URL and
a Firebase key are still *present* in that config — they are simply never
contacted, which is what the two captures above establish and what reading the
config alone could not.

#### What was deliberately not done

**The contrast run — fork policy off, to watch telemetry appear — was not
done.** It is the most persuasive demonstration available and it would mean
transmitting this user's data to a third party to make a rhetorical point.
Not my call to make. The argument that fork policy is what silences this is
covered by unit tests instead.

**The claim is "no telemetry", not "nothing leaves".** The agent's prompt goes
to Anthropic, in the clear in that `POST /v1/messages` body, because that is
what an agent on your own subscription *is*. What does not happen is Warp
learning anything about you.

**Still unverified: T2.5, audio.** No proxy capture during a real recording;
that needs a microphone and someone to speak into it. Unchanged by this.

**Platform: Linux only.** Windows is unmeasured, and `tcpdump` was unavailable
here (it needs root), so packet-level capture was not part of this.

### The log-spam question, answered by counting

`workspace:save_app` is **29 to 46 lines per run**, and the number barely moves
between a 904-line log and a 4197-line one. It is not the spam. Six rotated
logs:

    warp-oss.log         total 1115   dispatching   67   save_app 46
    warp-oss.log.old.0   total 1626   dispatching  301   save_app 43
    warp-oss.log.old.1   total 1029   dispatching   62   save_app 44
    warp-oss.log.old.2   total 4197   dispatching 3078   save_app 41
    warp-oss.log.old.3   total  904   dispatching   47   save_app 29
    warp-oss.log.old.4   total 1110   dispatching   60   save_app 34

The volume is `warpui_core::core::app` logging **every dispatched action at
`INFO`**, and in `old.2` that is 2975 lines of
`EditorAction::UserInsert(UserInput(" "))` — one per character, over two
minutes, at 25–40 a second. A held space bar, or WSLg key repeat. An
environmental burst rather than a Warp defect, and not what the question was
about.

**Leave it.** That trace is not noise, it is the only record of what a person
did in the window, and T5.6 was solved entirely from it — two hundred
`SelectText` actions and a `CtrlC`, which no other log line in the app would
have told us. Silencing the dispatcher to save forty lines a run would trade
the app's only forensic trail for nothing.

#### The thing worth having found

`UserInsert` carries the character that was typed, so the obvious next thought
is that the log is a plaintext keylog. **It is not, and upstream had already
thought about it**: `warp_util::user_input::UserInput<T>` has a hand-written
`Debug` that prints the value only under `cfg!(debug_assertions)`.

What it did not have was a test. One `cfg!` in a hand-written impl is the whole
of the guarantee — a careless `#[derive(Debug)]` would compile, pass every
other test in the tree, and start writing what people type into
`~/.local/state/warp-oss/warp-oss.log`. Now pinned in both profiles, and
verified the way T5.6's was: with the redaction removed, the release run fails.

    debug    shows "hunter2"    (deliberate; the doc comment promises it)
    release  redacts it
    release, redaction removed  -> FAILED

Worth knowing while working on this fork, since `.fork/README.md` tells you to
run `target/debug/warp-oss`: **your development build's log does contain what
you typed.** That is upstream's stated intent for a dev build, not a leak, but
it is a reason not to hand that file to anyone without reading it first.
