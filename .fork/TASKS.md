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
> **T12 is the current phase.** `IDEAS.md` is the queue in front of it.
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

## T1 — `warpctrl` local control plane  ← DONE

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

## T8 — The app you actually use  ← DONE (2026-08-23)

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

- [x] **T8.1** Quake visor for the lead agent. (`IDEAS.md` I8)
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

      **As built (2026-08-22).** Smaller than the plan, and the plan named the
      wrong mechanism.

      *Retraction: `PanesLayout::AmbientAgent` is not the variant to reach
      for.* It is the **Cloud Agent setup tab** —
      `initial_ambient_agent_pane` builds a cloud-mode terminal and calls
      `enter_ambient_agent_setup`. "Ambient" is upstream's word for the cloud
      agent, not for a local one, so pointing the visor at it would have wired
      the hotkey to the account-gated path this fork exists to avoid. Reading
      the enum's variant names gave a confident wrong answer; reading the arm
      they resolve to gave the right one.

      What it actually took: the quake window is built from
      `NewWorkspaceSource::Empty`, which lands in `configure_empty_workspace`
      → `add_new_session_tab_with_default_mode` — and *that* already enters
      agent view when the global default session mode is `Agent`. So the
      machinery existed; the only thing missing was making the visor's mode
      independent of the setting that governs every other new tab. One
      predicate and one call, both in `root_view.rs`:

      * `fork::quake_visor_opens_agent()` — env `WARP_FORK_QUAKE_VISOR`,
        **default on**, the only predicate in `fork.rs` that defaults that way.
      * `root_view::open_agent_visor`, called inside `add_window`'s builder
        after `RootView::new` — the one point where the window's first session
        exists and nothing has been painted, so there is no visible flash of a
        terminal becoming an agent.
      * `root_view::visor_opens_agent(ctx)` is the *effective* answer, shared
        by the behaviour and by `window.visor.status` so the two cannot drift.

      **Two things outrank fork policy, in opposite directions**, and both were
      found by running it rather than by reading:

      * *Default mode already `Agent`* → the workspace has entered agent view
        on the way here, and forcing it again starts a **second conversation in
        the same pane**. Guarded; measured as exactly one conversation, not
        two.
      * *No AI enabled* → agent view has nothing behind it, so the visor stays
        a terminal. `default_session_mode()` already collapses to `Terminal`
        in that case, which is what made this one line rather than a special
        case.

      **`opens_agent` reports the effective answer, not the policy** — the
      first version reported `fork::quake_visor_opens_agent()` directly and so
      answered `true` while opening a terminal, because AI was off. Same shape
      of bug as T8.5's `pane_contents` vs `visible_pane_ids`: two notions of
      the same question, one in the report and one in the behaviour. Fixed by
      giving both a single function.

      **Two new `warpctrl` actions, 105 → 107**: `window.visor.toggle` and
      `window.visor.status`. Not decoration — synthetic keystrokes reach no
      X11 client under WSLg (see "it is not Warp, and X11 is exhausted"), so
      without a second entry point this feature could not be exercised here at
      all. `toggle` dispatches the same global action the shortcut does, so it
      works with nothing bound.

      *`toggle` deliberately does not report post-call state*, unlike
      `pane.main.*`. `ModelContext::dispatch_global_action` **queues an
      effect** that runs after the request returns, so any state read beside it
      is the state from before the toggle. Reporting it would be a confident
      wrong answer; `status` is a separate call for that reason.

      **Nothing was needed for the command palette.** Upstream already
      registers "Show Dedicated Hotkey Window" and its hide twin as
      `FixedBinding`s, gated on `QUAKE_MODE_ENABLED_CONTEXT_FLAG`. The gate,
      again, was a setting.

      Verified 2026-08-22 across four fresh launches on WSLg, each checked two
      independent ways — the X11 window title and `warpctrl agent list`. Full
      table under "The visor: a drop-down agent on a hotkey" in `README.md`.
      `state` reads `pending_open` rather than `open` from a script, because
      Warp is not the focused app; that is documented upstream behaviour and
      not a failure.

      **Left undone**: the visor is a *window*, and T8.5's main pane is
      per-tab, so "point the visor at the lead agent" is still only half true —
      it opens an agent, but not one that knows about another window's
      designated pane. That is cross-window agent context and a separate idea.

- [x] **T8.2** Tab → pane drag, with a drop target you can see. (I3)
      Quadrant split-on-drop is *implemented*
      (`pane_group/pane/view/header/mod.rs:853`, with an ASCII diagram) and the
      tree surgery exists (`tree.rs:260`). Two things make it feel unintuitive,
      and both are precise: the drag source is the **pane header** rather than
      the tab, and the split is emitted from `PaneHeaderDragged` rather than
      `PaneHeaderDropped`, so the layout reflows live under the cursor and
      there is no preview distinct from the result. Split preview from commit,
      add the tab as a drag source, and add right-click → Split.

      **As built (2026-08-22).** All three items, plus a control-plane verb so
      the operation can be checked without a mouse. One of the three is
      built-but-unverified and is called out below.

      **1. Split preview from commit — done and seen.** `MovePaneWithinPaneGroup`
      was emitted from `PaneHeaderDragged`; it now fires only from
      `PaneHeaderDropped`. On drag the header records a `PaneDropPreview`
      (target pane + direction) and the pane group draws it as a translucent
      accent overlay across the half the drop will take.

      The overlay needs **no measurement**: a `Flex` with two equal `Expanded`
      children lands exactly on the split the drop produces. That matters
      because `render` is handed an `&AppContext` and `element_position_by_id`
      lives on `ViewContext` — the pixel maths stays in the drag handler, where
      a `ViewContext` exists. `drop_preview` threads through
      `PaneTree::render → PaneNode::render → PaneBranch::render` exactly the
      way `hidden_panes` already did.

      *The dead zone became a real state.* `calculate_pane_move_direction`
      returns `None` within 18% of a pane's centre. That used to mean "emit
      nothing and leave the last move standing", so it was invisible; now it
      clears the preview, so the overlay disappears and releasing there moves
      nothing. Pinned by a table test.

      *The header owns the pending direction*, because `PaneHeaderDropped`
      carries a target but no position — so the direction has to be remembered
      from the last drag event. That memory **is** the preview, which is why
      the two ended up as one field.

      **2. Right-click a tab → "Move into active tab, left/right/above/below"
      — done and run.** Four flat entries rather than a submenu; a submenu
      needs the sidecar machinery "Move to group" uses and four short labels
      cost less than that buys. The section vanishes entirely when the move
      would be ambiguous: a tab cannot merge into itself, and a tab holding
      more than one pane has no single "this tab" to move —
      `remove_pane_for_move` takes one pane, and silently moving the first of
      several is a worse answer than not offering it.

      **`tab.merge` (107 → 108 actions)** is the same operation for scripts,
      and the reason any of this could be checked at all. Verified live: pane
      `3527` moved out of tab `3139` into tab `1818` at index 2 and took focus,
      **and the source tab closed itself** — `remove_pane_for_move` emits
      `Event::Exited` when it takes a group's last pane, the same path a pane
      dragged to the tab bar takes. Both refusals fire with the reason in the
      message.

      **3. The tab as a drag source — built, NOT verified by running.** The
      gate was **an axis**, and it is a compile-time one: upstream pins each
      tab's `Draggable` to `DragAxis::HorizontalOnly` unless
      `FeatureFlag::DragTabsToWindows` is on, and that flag sits in
      `RELEASE_FLAGS` under `cfg!(any(target_os = "macos", target_os =
      "windows"))`. On Linux a tab physically cannot leave the tab bar.

      `fork::tab_pane_drag_enabled()` relaxes **only the axis**, not the flag —
      the same flag gates cross-window tab detach at four other sites, which
      spawns a ghost window nobody has exercised here. Opening the axis is all
      a tab-to-pane drag needs.

      The rest is additive and provably so: upstream sets **no**
      `with_accepted_by_drop_target_fn` on the tab at all, so the `data`
      argument to its `on_drag`/`on_drop` was unconditionally `None` and the
      tab-bar path resolves its drop from cursor geometry. Accepting
      `PaneDropTargetData` makes `data` `Some` for panes and changes nothing
      else.

      **This one needed a person** — a drag is press-move-release, and
      synthetic input does not reach X11 clients under WSLg, so it shipped
      compiled but not performed. **Performed by hand on Windows 2026-08-22;
      it works.** What that run found is below, and it is enough to reopen the
      item.

      > **It no longer needs a person, and the sentence above is why T9
      > exists.** `use_computer drag` (T9.1) performs the gesture, and driving
      > it on 2026-08-23 found that the tab-as-drop-source work reached only
      > `tab.rs` — the horizontal strip — and not `vertical_tabs.rs`, which is
      > what the Linux build renders. Fixed in `93895f796`; both halves of
      > this item are now confirmed by a performed gesture rather than by a
      > compiler.

      **Left undone:** merge semantics for a multi-pane tab, and changing the
      tab-out-to-new-window behaviour. Both were out of scope in `IDEAS.md` I3
      — and the second one no longer can be, for the reason in the next
      section.

      #### REVISIT SOON — what a mouse found, 2026-08-22

      The gesture works. Four things about it do not, and one of them is a
      consequence of the change itself. Nothing here is fixed yet; this section
      is the brief for the next pass.

      **1. Tab-out-to-new-window does not work — and T8.2 did not break it.**

      > **Retraction, same day.** The first version of this section said the
      > axis relax made the gesture "possible and inert", and called that a
      > regression T8.2 introduced. It was written from reading, it is wrong for
      > the layout the user actually runs, and it is wrong in the commit that
      > recorded it (`f22cc9e3c`). Two facts kill it. **The user runs vertical
      > tabs exclusively** — `vertical_tabs` is in `app/Cargo.toml`'s `default`
      > list, and the sidebar is in every screenshot taken this session. And
      > **`vertical_tabs.rs` was never touched**: `tab.rs` is the *horizontal*
      > tab bar, and no commit in T8 modifies the vertical path. Its axis lock
      > at `vertical_tabs.rs:2554` still reads `DragTabsToWindows` alone,
      > exactly as upstream wrote it.

      **Measured, not read.** A temporary `eprintln!` in `init_feature_flags`,
      built and run:

      ```
      FORKDBG DragTabsToWindows=false is_release_bundle=false
      ```

      The flag has never been on in a build made here, for **two independent
      reasons**. `RELEASE_FLAGS` is only extended when
      `ChannelState::is_release_bundle()`, which is
      `cfg!(feature = "release_bundle")` (`channel/state.rs:84`); and the
      app-side entry at `features.rs:112` sits behind
      `#[cfg(feature = "drag_tabs_to_windows")]`. Neither cargo feature is in
      `default`, and `oss.rs` sets `Channel::Oss` with only `DEBUG_FLAGS`. The
      installed stable Warp *is* a release bundle, which is why the behaviour
      exists there and not here — the comparison being made is fork-build
      against stock, not before-T8.2 against after.

      So the correct statement is the boring one: **tab-out-to-new-window has
      never worked in this fork's builds, in either tab layout, and still does
      not.** A gap against stock, not a regression. Expected behaviour, per the
      user: release outside the window creates a new window at the release point
      carrying the dragged tab/group.

      **The fix is still one line, but it is an enablement rather than a
      repair.** `fork::FORCE_ENABLED` sets a *user preference*, and `is_enabled`
      resolves override → user preference → channel state, so it outranks both
      cfgs — the I16 precedent exactly. Adding `DragTabsToWindows` there lights
      up the detach (`workspace/view.rs:28693`), the ghost chip
      (`view.rs:27806`), the focus call (`pane_group/mod.rs:3303`) **and both
      axis locks at once** — which is the part that matters, because
      `vertical_tabs.rs:2554` is already flag-gated, so forcing the flag fixes
      the layout the user runs without touching that file. It also makes the
      `tab.rs:2280` axis relax redundant, so that should come back out.
      **Unverified**: it exercises `cross_window_tab_drag.rs` (~1,800 lines)
      that nothing in this fork has ever run. Do not assume; run it.

      Note that `FORCE_ENABLED` is a code const applied by
      `apply_feature_preferences`, so this is a rebuild — there is no env var
      that flips it, and `WARP_FORK_POLICY=0` would turn it *off* along with
      everything else fork policy does.

      **What survives from the original claim**, scoped correctly: in the
      *horizontal* tab bar T8.2 did relax the axis without opening the detach,
      so there a tab can be pulled out of the strip and land nowhere. Real, but
      in a layout nobody here uses — and forcing the flag on removes it too.

      **2. Nothing cancels a drag, and Esc is not the exception — it only looks
      like one.** Reported symptom: mid-drag of an *agent* pane, Esc appears to
      cancel, and the pane comes back as an empty terminal. It is not a cancel.
      Esc pops agent view, which is upstream behaviour with upstream tests
      (`escape_pops_nested_cloud_agent_view_with_long_running_command`,
      `escape_does_not_exit_root_cloud_agent_view_...`,
      `escape_does_not_exit_local_agent_view_...`, all in
      `terminal/view_tests.rs`). The path is `input.rs:9454`
      `ctx.emit(Event::Escape)` → `terminal_pane.rs:907` →
      `pane_group::Event::Escape` → `workspace/view.rs:16144`. The drag is not
      consulted anywhere along it, so Esc falls *through* the drag and does its
      normal job to the pane underneath. The conversation is popped, not
      destroyed — **read, not run: confirm it is still in `agent list` before
      repeating this claim.**

      The seam for a real cancel is that same arm, `workspace/view.rs:16144`.
      The workspace owns the drop preview, `DraggableState::cancel_drag` already
      exists (`draggable.rs:89`, and its only callers today are
      `cross_window_tab_drag.rs:1456` and `:1777`), and the arm already runs a
      priority chain — resource center, then feature intro. A first branch that
      clears the preview, cancels the drag and consumes the event buys both
      halves at once: the cancel key that is missing, **and** the suppression of
      the agent-view pop that made its absence look like a bug.

      **3. A modifier key for drags is wanted, and would decide item 1 cleanly.**
      The user is open to gating the perpendicular pull behind a modifier rather
      than making it free. That also resolves the tension in item 1 without
      choosing between "reorder within the strip" and "tear out to a window" —
      the modifier picks.

      **4. Header-drag lag on the Windows build — still unmeasured, and the
      first explanation offered was not good enough.** Reading the path a second
      time only strengthened the wrong half of the argument: both
      `set_drop_preview` implementations (`pane_group/mod.rs`,
      `header/mod.rs:~1017`) return early when the value is unchanged, the
      overlay wrapper is a `.filter()` that wraps at most one pane, and the
      `PaneGroup` arm *replaced* per-drag-event tree surgery with a field write.
      Over a pane, the new path is strictly cheaper than the old one.

      Which means the cause is somewhere this reasoning does not reach, and
      "should be cheaper" is not a measurement.

      **The build was a debug build** — confirmed by the user, who was watching
      its output in a terminal at the time. That is now the leading explanation
      and it costs one `--release` build to confirm or kill; T8.0 exists
      precisely because a debug binary is a different animal. Note the terminal
      output is *not* itself the cost: nothing on the drag path logs per event
      (`draggable.rs` has no logging at all, and the only `log::trace!` under
      `elements/gui/` is in `new_scrollable`). It is the unoptimised layout and
      render, not the writes.

      Next after that, if release still drags badly: **the path that did not
      change.** A pane header dragged *toward the sidebar* is
      `PaneDragDropLocation::TabBar`, which emits `Event::DraggedOverTabBar`
      unconditionally on every drag event and recomputes a hover index each
      time — upstream, untouched. `PaneDragDropLocation::Other` likewise emits
      `PaneDraggedOutsideTabBarOrPaneGroup` every event. Only after those: the
      ghost's composite and the `Stack`+`Flex` overlay rebuild.

      `WARP_FORK_POLICY=0` A/Bs fork behaviour without a rebuild, but it does
      **not** isolate T8.2 — it turns off every fork predicate at once.

      **And there is a blind spot to fix first.** None of this can be measured
      today, because the only frame-cost instrumentation upstream ships is
      `LogExpensiveFramesInSentry` — and fork policy force-disables it
      (`fork.rs:46`) along with the rest of the telemetry flags. The reasoning
      was right (it reports to Sentry) but the consequence was not intended:
      **the fork has blinded itself to its own frame performance**, and the
      first time that mattered is the first time somebody said a gesture felt
      slow.

      The established fork answer to exactly this shape of problem is a local
      replacement rather than a re-enablement — `LocalTranscriber` for voice,
      the local agent for the transport.

      **Built the same day: `WARP_FORK_FRAME_LOG`.** `crates/warpui/src/frame_log.rs`
      holds the accounting and no policy; `fork::slow_frame_threshold` holds
      the policy and no accounting; the hook is four lines in
      `redraw_window`, timing the scene-build-plus-render closure rather than
      the whole function. `on` means 33ms (two frames at 60Hz), a bare number
      is a threshold in ms, and it is off by default.

      Two decisions worth keeping. **It summarises once per second rather than
      logging per frame** — a line per slow frame is its own performance
      problem during exactly the stutter it is describing, and would change
      what it measures. And **an unparseable value takes the default rather
      than switching off**, the opposite call from `WARP_FORK_QUAKE_VISOR`,
      because here the default is *off* and a typo answered with silence is
      indistinguishable from a broken feature.

      Verified by running, three ways: with `on`, five summary lines and a
      `worst 246.2ms` on the WSLg debug build; with the variable unset, zero
      lines; with `WARP_FORK_POLICY=0` and the variable set, zero lines.

      **And then it answered item 4.** Same workload (eight `tab create`s,
      WSLg, software GL), same threshold of 1ms so every rendered frame is
      counted, one build against the other:

      | build | frames/sec | mean frame | worst |
      |---|---|---|---|
      | debug | ~27 | 13.9–20.5ms | 44.9ms |
      | release | ~54 | 6.1–7.9ms | 15.0ms |

      **A debug build cannot hold a 16.7ms frame budget and a release build
      sits well inside it** — 2.4× cheaper per frame, twice the frame rate.
      That is sufficient to explain a drag that feels laggy without any
      contribution from the drop preview, and it retires the suspicion of the
      overlay unless a `--release` build still stutters.

      Absolute numbers here are pessimistic for both (software GL under WSLg);
      the ratio is the finding, not the milliseconds.

      **Answered: it was the vertical panel.** The open question this section
      first carried — horizontal strip or vertical sidebar — is closed. The
      user has never used the horizontal layout, which is what turned item 1
      from a regression report into a retraction. Worth keeping as a habit:
      **`vertical_tabs.rs` and `tab.rs` are two separate implementations of
      "the tab strip"**, and a change to one says nothing about the other. The
      X/Y labels in the report are transposed relative to the code, which is
      why everything above says "along the strip" and "out of the strip".

      #### Answered 2026-08-23 — all four, and one retraction

      **1. Tab-out-to-new-window: enabled, by forcing the flag.**
      `FeatureFlag::DragTabsToWindows` joins `fork::FORCE_ENABLED`, and the
      hand-rolled axis relax in `tab.rs` is deleted as redundant — forcing the
      flag opens `tab.rs`'s axis lock, `vertical_tabs.rs:2554`'s, and the
      detach they both feed, from one line. `fork::tab_pane_drag_enabled` now
      owns only the half that has no upstream flag: the drop-target acceptance
      that lets a tab land on a pane at all.

      **Measured, both ways, in the same build:**

      ```
      FORKDBG DragTabsToWindows=true  is_release_bundle=false   # fork policy on
      FORKDBG DragTabsToWindows=false is_release_bundle=false   # WARP_FORK_POLICY=0
      ```

      That is the exact inverse of 2026-08-22's measurement, in a build that is
      still not a release bundle — which is the I16 claim about user
      preferences outranking `cfg` demonstrated rather than asserted.

      Two technique findings from taking that measurement, both worth keeping:

      * **`--warpctrl` runs `init_feature_flags` too.** So a flag can be A/B'd
        with `warpctrl instance list` in a process that opens no window and
        binds no port — which is the way around the `WARP_FORK_POLICY=0`
        shutdown trap this file warns about, rather than the way into it.
      * **`FeatureFlag::is_enabled` panics before `mark_initialized()`**, so a
        probe placed between `apply_feature_preferences` and that call takes
        the process down. Put it after.

      **And one thing this does not fix**, found while reading the axis locks:
      `vertical_tabs.rs:3206` pins the *tab group* draggable to
      `DragAxis::VerticalOnly` **unconditionally** — no flag. So dragging a
      whole group out of the sidebar to a new window still cannot happen, and
      the flag has nothing to say about it. The report said "tab/group"; this
      answers the tab.

      **2. Escape cancels a drag — and the seam named above was wrong.**

      > **Retraction.** This section said a first branch in
      > `workspace/view.rs`'s `pane_group::Event::Escape` arm would buy both
      > the cancel and the suppression of the agent-view pop. It buys neither.
      > The pop happens in `TerminalView`'s `InputEvent::Escape` handler and
      > *then* emits `Event::Escape`, so the workspace arm runs after the thing
      > it was supposed to prevent. And keystrokes never reach that arm as
      > keystrokes at all: `app.rs:3538` matches bindings along the responder
      > chain **before** the element tree is offered the event, and
      > `dispatch_keystroke` walks that chain innermost-first, so the focused
      > editor claims Escape and an ancestor view never sees it. Written from
      > reading; wrong on both counts.

      What was actually missing is smaller and further down: **nothing in the
      app can answer "is a drag happening?"** A `DraggableState` belongs to the
      view that renders the element, so the code that sees the keystroke is
      nowhere near the code that owns the drag. `warpui_core`'s new
      `elements::gui::drag::in_flight` is that answer — a process-global
      register of drags that have started and not finished, with `any_in_flight`
      and `cancel_all`. Entries are pruned lazily by asking each one whether it
      is still dragging, so a missed deregistration cannot leak a permanent
      "yes".

      The cancel is then four lines at the top of `TerminalInput::editor_escape`
      — the first handler, before any branch — and two typed actions,
      `PaneGroupAction::CancelDrag` and `WorkspaceAction::CancelDrag`, that tell
      the two views which accumulate state on the way to drop it: the half-pane
      overlay, a pane hidden in anticipation of a tab-bar move, the hover index,
      `is_tab_being_dragged`, and the per-tab `detached` flag.

      Three decisions inside it:

      * **Cross-window tab drag is excluded.** It owns a second window, a ghost
        and a handoff protocol, and already calls `cancel_drag` along its own
        paths; stopping its `DraggableState` from outside would abandon the
        rest. The guard uses `has_singleton_model` first, because
        `CrossWindowTabDrag` is *not registered in the app's own test harness*
        and `as_ref` panics rather than answering — on a line that now runs on
        every Escape.
      * **A cancel stops a drag; it does not rewind one.** For a pane header,
        which previews and commits on drop, nothing had happened, so it is a
        true cancel. The tab strip reorders live as you drag — upstream — so
        there the movement so far stands. Pretending otherwise would need an
        undo stack for a gesture.
      * **The header clears its preview on drag *start*.** Without a drop it
        never clears, and `set_drop_preview` is silent when unchanged — so a
        leftover from a cancelled drag would swallow the first identical
        preview of the next one and the overlay would simply not appear.

      Verified through the real handler, not a mock:
      `a_drag_in_flight_takes_the_escape_key` opens a history menu, registers a
      drag, presses Escape and asserts the menu is *untouched* and the drag
      gone — then presses it again with no drag and asserts the menu closes.
      The second half matters as much as the first. Five more tests cover the
      register.

      **3. A modifier key for drags: decided against, for now.**
      The tension it was meant to resolve is gone. With the flag forced on,
      one gesture has three outcomes and geometry already picks between them —
      along the strip reorders, over a pane splits, outside the window
      detaches. Those are disjoint regions of the screen, so a modifier would
      not be disambiguating anything; it would add a key to the two cases that
      work today in order to reach the third. Revisit if the detach fires by
      accident in use, which is the evidence that would change the answer.

      **4. Header-drag lag: answered as far as this machine can answer it.**
      The frame log settled the mechanism — debug ~27fps against release ~54,
      2.4× the cost per frame — and that is sufficient to explain the report
      without the drop preview contributing anything. What was left was
      confirmation on the machine that saw it: a `--release` build on Windows,
      dragged by hand.

      > **Confirmed 2026-08-23, and it was the build.** Release, driven by
      > hand: "much snappier… nice and smooth." The next suspect — the path
      > that did not change, `PaneDragDropLocation::TabBar` recomputing a hover
      > index on every drag event — does not need investigating. The same
      > session confirmed the drop preview itself reads as intended; its colour
      > is the theme accent, and it is T8.2's, not upstream's.

      **Not verified by running, and cannot be here:** the gestures. A drag is
      press-move-release and synthetic input still reaches no X11 client under
      WSLg. The flag is measured, the cancel is tested through the handler that
      would run, and the drag itself has been performed by a person exactly
      once — on Windows, before either change.

      #### Three side findings from the same session

      **`is_busy` was true for a conversation nobody had asked anything.**
      `AIConversation` is constructed `InProgress` (`conversation.rs:420`) and
      only leaves that state when a turn *finishes*, so a freshly opened agent
      tab reports `in_progress` indefinitely — measured still true at t+60s,
      in the visor and in a plain `tab create --type agent` alike. `graph.rs`
      is unaffected (`turn_is_finished` also requires
      `last_exchange_is_complete == Some(true)`, which a never-prompted thread
      never has), but **T8.3's inbox would have put empty threads in the wrong
      bucket**. `agent.list`'s `is_busy` now also requires a non-empty
      conversation; `status` still reports upstream's word.

      **`window.list` now says which window is the hotkey window.**
      `is_hotkey_window`, one field, so "close the window I opened" stops being
      a guess that needs a join against `window.visor.status`.

      **Hide-on-blur for the visor: tried on Linux, put back.**
      `QUAKE_WINDOW_AUTOHIDE_SUPPORTED` is `cfg!(any(macos, windows))` and the
      implementation (`update_quake_mode_state`) is entirely
      platform-independent, so the constant looks over-cautious. It is not.
      Measured on X11/winit: a visor opened by `warpctrl window visor toggle`
      never becomes the key window, so the very next focus event sees a
      different active window and hides it — `Map State: IsUnMapped` within
      four seconds of opening, with no focus change from me. The autohide
      works; what does not survive it is the only entry point this platform
      has. Reverted, with the measurement recorded in the constant's doc
      comment so the next person does not repeat it.

- [x] **T8.3** The thread inbox, and `settled`. (I1)
      `ToolPanelView::ConversationListView` already exists with 2,198 lines
      behind it, and `AgentConversationEntry` already carries every field an
      inbox row wants. `settled: bool` mirrors the existing
      `AgentConversationData.pinned` — a JSON blob column, so **no migration**.
      **The trap:** `MAX_PERSISTED_CONVERSATION_COUNT = 200` with tree-wise
      eviction (`persistence/agent.rs:41`). An archive that evicts is not an
      archive; exempt settled rows or the feature silently loses work at
      conversation 201.

      **Sequencing changed 2026-08-23, by measurement (see `IDEAS.md` I17).**
      A collector was pointed at a real local-agent turn to find out what an
      inbox row could show. Two things came back. `is_busy` was already known
      to be wrong for empty threads and is fixed. The new one:
      **there is no trajectory to show.** The persisted record holds a tool's
      *name* and nothing else — `translate.rs:139` keeps `name` from Claude's
      `tool_use` and drops `id` and `input` at parse time, and `tool_result` is
      not handled at all. Tool results in `agent read --tools` come from the
      live surface's action model, so they do not survive the pane.

      **Amended twice the same day; this is the version that was run.**
      **No capture work blocks this, for either kind of session.** Claude
      writes a complete JSONL transcript — every tool call with its full
      input, and an `Edit` call's `{old_string, new_string}` *is* the diff —
      and Warp can already name the file two different ways:

      - a *CLI-agent* session (Claude in a pane, with the
        `warp@claude-code-warp` plugin upstream already installs) reports a
        **`transcript_path`** on every OSC 777 event;
      - a *local-agent* conversation stores Claude's session id as
        **`server_conversation_token`**, because `translate.rs:401` puts
        `session_id` into `StreamInit.conversation_id`. Verified: token
        `90115094-…` ↔ `90115094-….jsonl`, holding the `Bash` call whose Warp
        record was the single string `` `Bash` ``.

      So T8.3 needs a *reader*, not a pipeline. The row must degrade when the
      transcript is missing — cleaned up, or a conversation predating the
      token — to whatever Warp's own record holds. `translate.rs` and
      `event/v1.rs` both still drop fields and are both still worth fixing, but
      neither is on the path here. Nothing else about the plan above is
      affected.

      **As built (2026-08-23).** The plan held: `settled` really was one field,
      and the inbox really was a sort mode on a list that already renders.
      `ConversationSection` already existed with `Active` and `Past`, already
      collapsible, already header-rendered — so the inbox is a **third
      variant**, not a second implementation. Four things were bigger than the
      plan said, and one of them was a real defect.

      **1. `settled` on `AgentConversationData`, mirroring `pinned`.** No
      migration, as claimed — verified by reading the column back: settled rows
      carry `"settled":true` and every other row has **no `settled` key at
      all**, because `skip_serializing_if` keeps it out. Old rows parse as
      unsettled and are pinned by a test.

      **2. The trap is closed.** `select_conversations_to_evict` exempts any
      tree containing a settled conversation, and does not count it against
      the cap. Exemption is **tree-wise because eviction is** — trees are
      dropped whole, so a per-row check would take a settled child along with
      its parent. Four tests, the sharpest being a settled row that is *by far*
      the oldest, so every ordering rule votes to drop it.

      **3. A thread can be settled without being loaded, and that is the normal
      case.** `set_conversation_pinned` warns and gives up when a conversation
      is not in memory — fine for a pill bar, useless for an inbox, where the
      rows most worth settling are the ones nobody opened this session. So
      `ModelEvent::UpdateAgentConversationSettled` patches the one field
      without a task snapshot, and both loaded and unloaded threads take the
      same write.

      **4. `agent.settle` (108 → 109 actions)**, which is how any of this was
      checked without a mouse. Verified live: settle a live thread, restart,
      read it back settled; settle a thread that was never opened; `--undo`
      removes the key entirely; settling twice answers `changed: false` rather
      than erroring.

      #### The defect, found by looking rather than reading

      Every settled row in the inbox said **"2 min ago"** — which was when I
      settled them. `agent_conversations` has an AFTER UPDATE trigger,
      `update_last_modified_at_for_agent_conversations`, that stamps
      `CURRENT_TIMESTAMP` on any write leaving the column alone. Writing the
      old value in the same statement does not help: the trigger's guard is
      `NEW.last_modified_at IS OLD.last_modified_at`, which that satisfies.

      **Not cosmetic.** Eviction orders trees by `last_modified_at`, so a
      bumped row outranks genuinely newer conversations — harmless while it
      stays settled and exempt, and **a way to evict live work the moment it is
      unsettled**. Tidying up would have deleted things.

      Fixed by letting the trigger fire and then putting the timestamp back:
      the restoring update differs from the bumped value, so the guard fails
      and it does not re-fire. Pinned by a test in both directions. The same
      finding is why settling no longer routes a loaded conversation through
      `write_updated_conversation_state` — that is a full upsert and would let
      the trigger stamp the row, so settling an *open* thread bumped it while
      settling a closed one did not. Same action, same write, same timestamps.

      #### A second thing reading would not have caught

      Settling from `warpctrl` changed the database and **the inbox did not
      move**. `AgentConversationsModel` explicitly ignores
      `UpdatedConversationMetadata`, so the emit went nowhere. Widening that
      would rebuild the list on every title and token-count update, so
      settling emits its own `ConversationSettledChanged` and only that is
      translated into a rebuild.

      **Verified by running:** SETTLED renders at the bottom, collapsed, with
      PAST correctly emptied as threads moved out; temporarily starting it
      expanded confirmed the rows are *in* the section rather than lost, and
      the temporary change was reverted.

      **Left unverified:** the row context-menu item ("Settle thread" / "Bring
      back to inbox") is built and compiles but has not been clicked —
      synthetic input still reaches no X11 client under WSLg. `agent.settle`
      exercises the same `set_conversation_settled` underneath it.

      **Left undone deliberately:** no keybinding, per `IDEAS.md` I1 — "not
      until you have used it for a week and know what it should be." And the
      transcript reader from I17 is still a separate piece of work; this ships
      the sections and the bit, not the trajectory.

- [x] **T8.4** Pin what a tool claims to be. (I11)
      Hash each MCP tool's `(name, description, input_schema)` at connect,
      store it, and say so when a digest changes under an existing name. This
      is the tool rug-pull defence: a server can rewrite the prompt the model
      reads, after you approved it, and nothing currently notices. `sha2` is
      already a workspace dependency — **no new dep**, and no blake3. Warn in
      v1; do not auto-block until the noise level is known.

      As built. One new file, `app/src/ai/mcp/tool_digest.rs`, plus a
      twelve-line call site. No new dependency, no schema, no catalog action.

      **Three findings changed the design, all from reading the code the plan
      assumed:**

      1. **Connect is not one of several checkpoints; it is the only one.**
         Nothing in this client handles `notifications/tools/list_changed`, and
         `crates/mcp/src/runtime.rs:324` calls `tools/list` exactly once per
         spawn. The tool list is a snapshot taken at connect and never
         refreshed. So a mid-session rewrite is not detected — and is also not
         *acted on*, because the client keeps using the snapshot it already
         has. That is a much better answer than it first looks, and it is only
         true as long as nobody adds `list_changed` handling without adding a
         digest check beside it.
      2. **The installation id is not an identity.** `parsing.rs:322` and
         `:363` mint a fresh `Uuid::new_v4()` for every file-based server on
         every parse, so a `.mcp.json` server has a different installation id
         at each launch. Keying the store on it would have made every connect a
         first connect — a feature that runs, writes a file, and never once
         reports anything. Confirmed by running: three consecutive launches of
         the same server gave `a74fd341…`, `2592fda1…`, `d42814b5…`. Keyed on
         the server **name** instead, which is the key in the config file.
      3. **`(name, description, input_schema)` is not the whole claim.** It
         also hashes `title`, `output_schema` and `annotations`, each
         separately so a change is attributed rather than merely detected.
         `annotations.readOnlyHint` earned its place on its own: it is a claim
         that a tool is safe, and flipping it is a rug-pull that touches no
         prompt text at all.

      Trust on first use: the first connect of a server records what it
      advertises and says nothing, because there is no prior approval to
      compare against. After that, a redefinition is a `[warn]` in the server's
      MCP log — with the definition it advertises *now*, pretty-printed, since
      the store keeps hashes and the old text is gone — plus one toast however
      many tools changed. A new or removed tool is `[info]` only. Then the
      record is updated, so a change is reported once rather than at every
      launch.

      **Verified by running**, with `script/mcp_probe_server.py` — a
      dependency-free stdio MCP server that re-reads its own tool definition
      from a file at every `tools/list`, so changing that file and reconnecting
      *is* the attack. Eight launches against a scratch `HOME`:

      - first connect → store written, nothing said;
      - description and schema rewritten → `[warn]`, both fields named,
        the new definition in the log;
      - relaunched unchanged → silent;
      - a second tool added → `[info]` in the MCP log, nothing in the app log.

      **Not verified:** the toast. `import`/`xwd` cannot capture a root window
      under WSLg — there is no composited root to read — so it is built,
      compiles, and uses the same `add_ephemeral_toast` call as the MCP PATH
      error beside it, but nobody has seen it.

      **Left undone deliberately:** no auto-block, per the plan. And no
      `warpctrl` action to read or reset the store: the file is JSON in
      `~/.local/state/warp-oss/fork/`, and a verb to reset an approval record
      is a verb an attacker would like.

- [x] **T8.5** A main pane, and the CWD following it. (I13 + I6)
      One `Option<PaneId>` on `PaneGroup`, `None` meaning today's behaviour.
      Then consumers one at a time: CWD follow first, layout second,
      orchestration third. **This supersedes the "follow the focused pane"
      scoping**, which was wrong: a pane that merely has focus makes the file
      tree thrash every time you glance at a split. A pane you *named* is
      stable by construction. `main` is also the natural answer to a question
      T6.6 and T7.1 both leave implicit — which pane is the lead agent.

      Built and verified:

      - [x] `PaneGroup::main_pane` / `set_main_pane`, `Event::MainPaneChanged`,
            and `PaneGroupAction::ToggleMainPane` behind a command-palette
            entry with no default keystroke.
      - [x] First consumer: `cwd_anchor_session_view` replaces
            `active_session_view` at the one call site that decides a tab's
            repository (`workspace/view.rs`,
            `refresh_working_directories_for_pane_group`).
      - [x] `pane.main.get`, `.set` and `.clear` (catalog 102 → 105), so the
            effect is drivable and observable without a screenshot.
      - [x] **Run it**, two panes in different repos with focus deliberately on
            the *other* one. Designating pane 0 moved the anchor:
            `old_focused_repo=…/NeuralAudio new_focused_repo=…/warp`.

      **The overflow menu was dropped from scope, deliberately.** Its action
      type is generic per pane type (`P::PaneHeaderOverflowMenuAction`, built
      per child view by `pane_header_overflow_menu_items`), so one shared entry
      means touching every pane type. The palette entry is the intermediate
      surface, exactly as for T8.6 — real UI when more than one thing needs it.

      Two findings from running it:

      - **`pane_contents` is not "the panes that exist".** It outlives a close
        so a pane can be restored, so a pane gone from `pane list` is still in
        it. Validating the designation against `pane_contents` reported a
        closed pane as still main; `visible_pane_ids` is the right notion.
        Pinned by
        `test_main_pane_designation_does_not_survive_closing_that_pane`.
      - **The code review panel will not visibly follow it**, because its repo
        dropdown is a sticky per-pane-group selection that survives close and
        reopen. Pre-existing and not about the main pane: measured in the same
        session, it does not follow *focus* either. The anchor underneath does
        move — the toolbar diff badge tracks it.

      **Second consumer, 2026-08-23 — orchestration.** An unqualified
      `warpctrl` target now means the main pane when the group has one, and the
      active session only when it does not. One `or_else` in
      `local_control::resolver::input_target_pane_id`.

      This is the consumer `main` was promoted for. T6.6 and T7.1 both built
      agent fan-out and both left the same question implicit — which pane is
      the one you are talking to — and the answer was "whichever has focus",
      which is fine for a person with a mouse and wrong for a script: a graph
      that runs for twenty minutes addressed a pane that moved every time
      somebody clicked. A pane you named does not move.

      **Deliberately not scoped to agent actions.** Making `agent prompt`
      follow `main` while `input submit` followed focus would put two panes in
      play for one script, which is worse than either rule on its own.

      **Verified by running**, focus and main deliberately on different panes,
      each shell carrying a different `WHICH_PANE` so the answer is decisive:

      | main | focus | unqualified `input submit` ran in |
      |---|---|---|
      | none | pane 0 | pane 0 |
      | pane 1 | pane 0 | **pane 1** |
      | none | pane 0 | pane 0 |

      Pinned by `test_an_unqualified_control_target_is_the_main_pane_when_there_is_one`,
      which is also why `local_control::resolver` is now `pub(crate)`.

      **The layout consumer is deliberately not built.** `IDEAS.md` I13 already
      says a new layout algorithm is out of scope for v1, and nothing found
      since changes that: the only honest version of "main gets the large flex"
      is a policy about `PaneFlex` that competes with the flex the user set by
      dragging a border, and with the one restored from app state. There is no
      small version that is still the idea, only a small version that fights
      two existing sources of truth. `PaneTemplateType` is already a
      serializable pane tree, so a master/stack layout stays expressible later
      without new machinery. Ordering therefore went CWD → orchestration, not
      the CWD → layout → orchestration this entry proposed.

      Still open: making the code review panel honour the anchor, if that turns
      out to be wanted. Its repo dropdown is a sticky per-pane-group selection
      that does not follow focus either, so it is a pre-existing choice rather
      than a gap in `main`.

- [x] **T8.6** WSL as a remote target, the way Zed does it. (I16)
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

      - [x] **Run on Windows, end to end** (2026-08-22). A Windows
            `warp-oss.exe` client, `warpctrl tab create`, then
            `remote wsl connect --tab <id>` → `{"distro": "Ubuntu",
            "distro_from_pane": true}`. Twenty seconds later, inside Ubuntu:
            proxy, `remote-server-daemon` and its `terminal-server` child, all
            sharing identity key `2dea4f26…`, with a state directory of that
            name freshly created. No SSH, no account, no install prompt.

      **The warpify step turned out to be unnecessary.** The runbook said to
      type `wsl` into a pane and accept the subshell prompt. That works, but
      setting *Default shell for new sessions* to the distribution is simpler
      and makes every new tab a WSL session: `SessionInfo::wsl_name()` falls
      back to `ShellLaunchData::WSL { distro }`, so launch data alone satisfies
      `connect`. Confirmed by `"distro_from_pane": true` on a tab created with
      `warpctrl tab create` and nothing else. README step 4 now leads with it.

      Two traps worth carrying forward, both found by running it:

      - **A daemon being present proves nothing on its own.** They outlive the
        GUI that spawned them, so a stale one is indistinguishable from a fresh
        success in `pgrep` output. Check `ps -o etimes`.
      - **The proxy and daemon report different paths for the same binary** —
        `~/.warp-dev/remote-server/warp-oss` is a symlink, and the daemon is
        spawned via `current_exe()`, which resolves it.

      Remaining, and now the only part left: **the ambient path.** `wsl` is
      *already* a warpify subshell command on Windows, so a WSL session gets
      warpified exactly as an `ssh` one does; what it does not get is a remote
      server, because that attach is keyed on `IsSSHWrapperSession::Yes`, whose
      payload is a ControlMaster socket path a WSL session cannot have. A WSL
      arm beside the SSH one is the work, and `Session::wsl_name()` already
      carries the distribution — as the default-shell finding above confirms,
      including for sessions that were never warpified at all.

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

## T9 — Verifying the pixels  ← DONE (2026-08-24)

> Every GUI claim in this file was verified by a person clicking, by SQLite, or
> by a log. That is the fork's one standing exception to *run it*, and T8 spent
> it three times: "needs a person on Windows" is written into T8.2 twice and
> into the closing report once.
>
> T9 removes the exception. It is `IDEAS.md` I15 (computer use), scoped down to
> the half this fork actually wants.

- [x] **T9.1** A drag an agent can perform. (`IDEAS.md` I15)
- [x] **T9.2** The same on Windows, without taking the user's mouse.
- [x] **T9.3** The ghost a cross-window drag leaves behind. It was T8.2's
      tab-to-pane drop answering a release that belonged to the cross-window
      drag; fixed and A/B'd.
- [x] **T9.4** Three things a person found in the release build: a dead zone
      that was both too big and measured from the wrong point, a cancelled drag
      that left a pane looking empty, and a modifier that was never wired.
      Two fixed, the third answered — and a fourth found underneath it.

### T9.1 — as built

**`use_computer drag`, and nothing else.** `crates/computer_use` already had
screenshots, clicks, typing, window enumeration and an XInput2 MPX agent seat.
The one gesture missing from its CLI was the one every blocked item needed. The
verb is built out of `Action::{MouseDown, MouseMove, Wait, MouseUp}` — variants
that already existed — so the library, the crate's dependencies and the app
surface are all unchanged.

#### The scope correction: the flag gates the wrong half

`IDEAS.md` I15 says opening computer use means `fork::FORCE_ENABLED` **and**
the `local_computer_use` cargo feature, "T1.1 and T1.2 again". For the thing
this fork wants, **neither is needed**, and the entry was scoped from the
wrong end.

`FeatureFlag::LocalComputerUse` gates whether **Warp's own agent** is offered
computer-use tools (`app/src/ai/agent/api.rs:361`, one term of a four-way
`&&`). But this fork's agent is Claude Code driving the `claude` binary, and it
reaches the computer through its own Bash tool. It does not need Warp to hand
it a `use_computer` action.

`use_computer` is a separate binary that checks no feature flag at all. It was
built with `cargo build -p computer_use --bin use_computer` and driven against
a running Warp with no flag touched, no preference seeded and no cargo feature
added. The gate was never in the way of the half that matters.

Whether to open the in-app tool as well is a separate question and is not
answered here.

#### Three things the crate had already decided correctly

* **Window-target coordinates are window-local pixels**, translated to root by
  `windows::window_local_to_root`. So a drag is expressed in the coordinates a
  screenshot of that window shows you, which is the only sane arrangement and
  not the one the flag names suggest.
* **The agent seat is actor-scoped, not call-scoped** — a private XInput2
  master pointer/keyboard pair, with a comment saying "a drag that spans
  batches keeps its button held". That is what makes `--screenshot` possible:
  the drag runs as two `perform_actions` calls on one actor, the capture
  happens between them, and the button is still down for it.
* **None of it touches the real cursor.** The MPX seat has its own pointer.
  The user's mouse does not move and their focus does not change.

#### The frame that only exists mid-drag

`perform_actions` takes its screenshot at the end of a batch, so a naive drag
photographs the *result*. The result was already reachable — `warpctrl pane
list` answers it better than a PNG does. What was not reachable is the drop
preview, the floating tab ghost and the detach chip, none of which survive the
mouse-up.

So `--screenshot` writes its PNG **before** the release, and that is the whole
reason the verb is shaped as two batches instead of one.

#### What it found, immediately

The first drag it performed was supposed to split a pane. It detached a tab
into a new window instead, and **that was correct behaviour for the code as it
stood** — see the T8.2 entry above. T8.2's tab-as-drop-source work went into
`tab.rs`, the horizontal strip, and never reached `vertical_tabs.rs`, which is
what the Linux build renders. One tool, one gesture, one real gap, on the first
run.

#### Verified by running, 2026-08-23 (Linux, X11 under WSLg)

* **Detach works.** Dragging a tab out of the vertical panel and releasing:
  `begin_multi_tab_drag source_wid=0 preview_wid=1 source_tab_index=2` →
  `finalize_preview_as_new_window (CREATES NEW WINDOW)`, and `window list`
  goes from one window to two. **This is the T8.2 item that shipped compiled
  but never performed** — `DragTabsToWindows` in `fork::FORCE_ENABLED` is the
  gate, and it is now confirmed by the gesture rather than by an `eprintln!`.
* **Tab → pane split works**, after the fix above: five `DragTabOverPane` and
  one `DropTabOnPane`, one window, the pane moved into the target tab and the
  source tab closed itself.
* **The drop target is visible.** The mid-drag capture shows the translucent
  accent overlay across exactly the half the drop takes. T8.2 built that and a
  person confirmed it on the horizontal strip; this is the first time it has
  been photographed.

#### One observation that did not survive being tested

A first run left the app firing tab drags on its own for two minutes after the
CLI had exited — eight `StartTabDrag` with nothing driving the pointer, all
originating from windows the detach had created.

The obvious suspect was a button leaked by the agent seat. **It reproduced
zero times.** A controlled run — one drag, then forty seconds during which no
command was issued at all — recorded exactly one `StartTabDrag` and nothing
after it. The cause is not established; the most likely remaining explanation
is the physical mouse over the windows the detach had just put on the desktop.
Recorded because it looked exactly like a tool bug and was not one.

### T9.2 — as built

**`background_supported()` answered `false` on Windows, and its own comment
gave the reason: "The Windows input stack drives the screen / foreground
window."** That is true of `SetCursorPos` + `SendInput`, which is what the
backend was. It is not true of Windows.

`PostMessage` to one `HWND` was already the fork's answer for *clicks* and
*keys* — `click.ps1` and `keys.ps1` have used it for months, and README
already records which of the four obvious variants work. T9.2 is that same
mechanism extended to press-move-release and moved into the crate, so
`use_computer drag --pid --window-id` means the same thing on both platforms.

Three parts, and the first was blocking the other two.

**`use_computer windows` did not work on Windows**, so there was no way to
*obtain* an id to pass to `--window-id`. It answered "only supported on macOS
and Linux (X11)" on the one platform this fork's GUI is used on.

`EnumWindows`, deliberately **not** `Process::MainWindowHandle`. Warp puts
every window in one process and Windows nominates exactly one of them as
"main", so a tab torn out into a second window is invisible to that API — and
the nomination *moves*, which is worse than it being wrong, because the same
call answers a different window before and after a tear-out. That cost one
confusing run before it was noticed.

**Window-targeted input.** Mouse and keyboard as posted messages, with the
held-button state carried in every `wParam`, because a move mid-drag with an
empty `wParam` reads to the receiving application as "the button came up
somewhere I did not see". Two details earned their comments: the wheel
messages carry **screen** coordinates while every other mouse message carries
client ones, and posted `WM_CHAR` never reaches Warp's editor while posted
virtual-key messages do.

**Window-targeted screenshots, via `PrintWindow(hwnd, hdc,
PW_RENDERFULLCONTENT)`** — `shot.ps1`'s recipe, which README says has been lost
to a cleared session twice, now in the crate. `PrintWindow` needs the
`Win32_Storage_Xps` cargo feature, because the Win32 metadata files it with
the printing APIs rather than the windowing ones.

#### Verified by running, 2026-08-23 (Windows release build)

```
cursor before   (998, 480)
drag            750,16 -> 250,16, window-targeted
cursor after    (998, 480)        <- unchanged
result          the "Settings" tab moved from the sixth slot to the second
```

And `use_computer windows` lists both Warp windows plus every other toplevel
on the desktop, with bounds, class and title.

#### Two limits, both measured

* **The target window must be the foreground window.** A/B'd both ways: focus
  window 0 and drag it, the tabs reorder; focus window 1 and repeat the
  identical drag on window 0, nothing moves. A posted *click* on an inactive
  window does work — it selects the tab under it — so this is specific to
  drags. The cursor is still never touched, which is the part that matters,
  but this is not a way to drive a window nobody is looking at.
  `warpctrl window focus` supplies the activation without a mouse.
* **Modifiers are not expressible.** Posted messages do not set the thread's
  key state, so `ctrl-shift-<key>` arrives as a bare `<key>` — the same wall
  `keys.ps1` documents. `Target::Screen` keeps the `SendInput` path for the
  cases that need it.

### T9.3 — as built

**The user's report, 2026-08-23:** "a tab torn out of the strip to create a new
window, then dragged back got hung on the seam of the pane and the strip, in
the ghost state. resizing the window fixed it."

Reproduced on the first attempt with `use_computer drag`. The target window
draws two tab labels on top of each other in one slot and leaves a gap at the
next; `warpctrl tab list` still shows the tab in its own window, so nothing
merged.

#### The first explanation was wrong, and the log had already said so

An earlier version of this section — and the commit that recorded it,
`6dd5378c9` — said "the release never arrives, so the ghost stays". **It
arrives.** What the log actually shows at the moment of release is:

```
tab_drag: on_drag_while_floating -> GhostInTarget target_wid=0 insertion_index=4 caller_wid=2
dispatching typed action: WorkspaceAction::DropTabOnPane { tab_index: 0, ... }
```

`DropTabOnPane`, not `DropTab`. The release was delivered and *answered by the
wrong handler*, which is a different bug with a different fix, and the line was
sitting in the log the whole time under a heading that said the opposite.

#### The cause is T8.2's tab-to-pane drop source

The fork made a tab accept `PaneDropTargetData` so it could be dropped on a
pane. `Draggable` resolves that target by intersecting the drag rect with the
drop targets **of the window dispatching the event**, and picking the smallest.

During a single-tab cross-window drag the source window *is* the floating
preview and is repositioned under the cursor on every frame — so its own pane
is right there, intersecting its own tab's drag rect. The tab's `on_drop`
closure sees `Some(PaneDropTargetData)`, dispatches `DropTabOnPane` and returns
early, so `WorkspaceAction::DropTab` is never dispatched and
`CrossWindowTabDrag::on_drop` is never called. The drag stays live and the
ghost stays drawn.

The two paths are normally mutually exclusive by construction: a tab dragged
straight down onto a pane dispatches `DragTabOverPane` from the first frame, so
`on_tab_drag` never runs and no cross-window drag ever begins. Drag the tab
*out* first and the order inverts — which is exactly the gesture the user
performed, and exactly why it took a person to find.

#### The fix

`fork::tab_pane_drop_target_accepted(app)` — refuse pane drop targets while a
cross-window tab drag is in flight. `data` goes back to `None`, `on_drag` and
`on_drop` fall through to `DragTab`/`DropTab`, and upstream's state machine
runs as it did before any of this was added. Both strips, one predicate.

Pinned by `a_tab_in_flight_between_windows_refuses_pane_drop_targets` in
`workspace/cross_window_tab_drag_tests.rs`, which asserts against a real
`CrossWindowTabDrag` rather than the predicate's arithmetic.

#### Verified by A/B in one binary, 2026-08-23 (Windows release)

A temporary `FORKDBG_T93_OFF` env switch, so the two legs differ only in the
guard — same binary, same script, same machine:

```
off   after tear-out:  windows=2  tabs_in_window0=7
      after drag-back: windows=2  tabs_in_window0=7   NOT MERGED, ghost visible
on    after tear-out:  windows=2  tabs_in_window0=6
      after drag-back: windows=1  tabs_in_window0=7   MERGED, strip clean
```

`shots/t93_off.png` is the doubled label and the gap; `shots/t93_on.png` is the
same strip after the fix. The switch was removed before commit and the shipping
binary rebuilt.

And once the logging problem below was understood, the same gesture was run
against a session that actually logs, which shows the repair line by line:

```
tab_drag: begin_single_tab_drag source_wid=1 (source window IS preview)
tab_drag: on_drag_while_floating -> GhostInTarget target_wid=0 insertion_index=3 caller_wid=1
dispatching typed action: WorkspaceAction::DropTab          <- was DropTabOnPane
tab_drag: on_drop GhostInTarget -> DropResult::DropInto target_wid=0 insertion_index=2
tab_drag: perform_handoff branch=single_tab_source->other target_wid=0 caller_wid=1
tab_drag: execute_handoff_single_tab_to_other target_wid=0 insertion_index=2
tab_drag: finalize branch=InsertedInTarget target_wid=0 insertion_index=2 source_wid=1
tab_drag: finalize_handoff -> CloseSourceWindow transferred_tab_index=0
```

One line is the whole bug and the whole fix: `DropTab` where `DropTabOnPane`
used to be. Everything after it is upstream's state machine, which was never
broken — it was never called.

**The regression risk was the other direction, and it was checked.** The guard
must not switch off tab-to-pane dropping for ordinary drags. Driven after the
fix on both strips: Windows horizontal, a tab dragged onto a pane still splits
it (one window, the target tab goes to two panes); Linux vertical panel, the
same — five `DragTabOverPane` then `DropTabOnPane`, tab `2732` ends with two
panes.

**What was not driven is the cross-window gesture on the vertical panel.** A
cross-window drag repositions the source window under the cursor every frame,
so window-local coordinates drift and the X11 window-targeted path cannot
express it. The predicate and the render wiring are shared with the horizontal
strip and the unit test covers the decision, but nobody has performed *that*
gesture on the Linux build.

#### Two things that were true and stay true

* **Escape does not rescue a stuck cross-window drag.** T8.2's cancel excludes
  them on purpose, and pressing it changed nothing — confirmed before the fix.
  Moot now for this bug, but the exclusion is still there for any other way in.
* **Resizing did not clear it here**, though it did for the user. Their case
  and this one differ in some way still not identified.


### The crash, and what is and is not known

The user crashed the release build while stress-testing cross-window tab
drags. **The crashed process's log is gone**, so the cause is not known.

What was established:

* **Warp's "crash" here is a deliberate exit.** Reproduced one — a window
  resized to an absurd height, which clamps the surface to 65535, fails to
  configure, and trips the existing policy: `Failed to render a frame 3 times
  in a row; exiting...`. The crash-recovery sibling then takes over, which is
  the "child spawned immediately" the user saw. That sibling is spawned at
  *startup* and blocks in `WaitForSingleObject` on the parent, so its
  appearance is not evidence of anything beyond "the parent went away".
* **The crashed parent never wrote a log, and the reason is how we launch it.**
  See the correction below — an earlier version of this bullet blamed a
  best-effort rename in `on_parent_process_crash`, and that was wrong.
* **Not reproduced by dragging.** Six rounds of tear-out-and-drag-back, eight
  windows created in a burst, and twelve create/close cycles, all on the
  release build: no crash. The one lead that remains is Warp's own startup
  warning on this machine — "Newer NVIDIA drivers can crash if multiple
  windows are created if the `Vulkan / OpenGL Present Method` NVIDIA setting is
  set to `Auto` or `Prefer layered on DXGI Swapchain`" — which fires on every
  window creation here, and which the user's stress test was doing a lot of.
  Untested; it points at a driver control panel, not at this code.

**Next time it crashes, copy `warp-oss.log.old.0` before doing anything else.**

#### `Start-Process` silently turns Warp's log file off

**This is the correction, and it explains every missing log above.**

`init_internal` decides with
`use_logfile = !stdout_is_a_tty && !in_ci && !integration_test`. `warp-oss.exe`
is a console-subsystem binary, and **`Start-Process` without `-NoNewWindow`
gives it a console of its own** — so `stdout_is_a_tty` is true, `use_logfile`
is false, and the process writes no log file at all. Not a rename failure, not
a rotation bug: the parent never wrote one.

What made this hard to see is that a log *did* keep appearing. The
crash-recovery sibling is spawned by the parent rather than by a shell, so it
has no console, so it logs — to `warp-oss.log.recovery`. When the parent dies,
`on_parent_process_crash` moves that file into `warp-oss.log`. Every log this
machine produced on 2026-08-23 under `Start-Process` therefore begins:

```
2026-08-23T20:26:08Z [INFO] Parent has crashed; continuing execution.
2026-08-23T20:16:18Z [INFO] Parent has crashed; continuing execution.
2026-08-23T19:34:14Z [INFO] Parent has crashed; continuing execution.
```

…which reads like three crashes' worth of evidence and is in fact three
recovery processes' logs with the interesting half missing.

Proved by changing one flag. `Start-Process … -NoNewWindow`, same binary, same
everything else:

```
warp-oss.log        32455  6:32 PM     <- fresh, rotated the old one to .old.0
first line: 2026-08-23T22:29:02Z [INFO] Using DXC for DirectX shader compilation
```

A normal startup line rather than "Parent has crashed". **So: launch Warp with
`-NoNewWindow` whenever you might want to know what it did.** A person
double-clicking the binary is unaffected — Explorer gives it no console — which
is why this only ever bit the agent.

Consequence for the record above: the user's crash log was never written, so
there is nothing to recover. The T9.3 A/B was measured from window and tab
counts and screenshots because of this; once the launch flag was fixed, the
same gesture was re-run against a logging session and the trace is in T9.3.

### T9.4 — as built

**Three reports from a person using the Windows release, 2026-08-23.** Two were
defects in fork code; the third was a question, and answering it turned up a
fourth defect nobody had run.

#### T9.4a — the dead zone was two problems wearing one coat

**The report:** "the dead space in the center of the panes when dragging to
split is too large… for each of the panes to be split again by dragging, i have
to drag to the relative middle of each of the 4 quads."

Measured rather than guessed, with `use_computer drag --screenshot
--release-at` sweeping the drop point across a pane and photographing each
frame with the button still down. On a 544x644 pane the overlay appeared only
outside a 196x232px box — a third of the pane in each axis, exactly what
`DRAG_SPLIT_THRESHOLD = 0.18` says it should be, since the test is `max(|nx|,
|ny|) < 0.18` against the *half*-extent.

That is upstream's number and it was fine while the split happened immediately
on every drag event: you found the boundary by crossing it. T8.2 made the drop
a preview that only commits on release, and a dead zone you have to hunt for is
a different object from one you fall through — the overlay is simply absent and
nothing says why.

**The second problem is the one the arithmetic hides.** `calculate_pane_move_
direction` took the *drag rect*, and for a pane drag that rect is the 212x40
placeholder chip, not the pointer. `Draggable` keeps the grab point in the same
relative position inside the chip, so the chip's centre sits at a fixed offset
from the pointer **set by where along the header you pressed**. From the old
binary's own log, grabbing a 586px-wide header at x=200 put the reference point
46px to the *right* of the pointer; grabbing near the right end puts it ~60px
to the left. The whole quadrant map slides by up to half a chip depending on
where you happened to grab, which is unlearnable, because it changes every
time.

**The fix is both halves.** `calculate_pane_move_direction(target_pane, at:
Vector2F)` now takes the pointer, threaded from `DraggableState::
dragging_mouse_position()` in the drag closures — the same shape
`vertical_tabs.rs` already used for the group draggable. `DRAG_SPLIT_THRESHOLD`
goes 0.18 → 0.10. The zone still exists, because releasing over the middle of a
pane means "do nothing" and the overlay disappearing is how you learn that
before letting go.

**Verified by running, 2026-08-24 (Linux, X11 under WSLg).** Sweeps at 14px
resolution through the centre of a 544x644 pane, with the exact pane rect and
normalized values read out of a new `log::debug!` (`RUST_LOG=warp::pane_group::
pane::view::header=debug`):

```
vertical    Up   <= 318      none 332..448     Down >= 462     band 144px = 22%
horizontal  Left <= 244      none 258..352     Right >= 366    band 122px = 22%
before                                                         band       = 36%
```

And the part that matters more than the number — **the same sweep from two
different grab points now gives the same seven verdicts**, boundary for
boundary:

```
grab x=200   up up none none none down down
grab x=480   up up none none none down down
```

A committing drop was run afterwards to confirm the preview and the commit
still agree: pointer normalized `(0.0005, 0.264)` → Down, and the layout
became stacked.

#### T9.4b — Escape left the pane blank, and it was T8.2's dim

**The report:** "if you do hit the esc key when dragging a pane/tab, the pane
goes blank. sometimes i can hit / and get it to render, but sometimes it
appears that its just gone."

Nothing is gone. `PaneView::is_being_dragged` paints an opaque `surface_2`
overlay across the pane's contents, and it is cleared by exactly three events —
`PaneDroppedWithinPaneGroup`, `DroppedOnTabBar`, `PaneDroppedOutsideofTabBar
OrPaneGroup`. All three are *drops*. A cancelled drag reaches none of them.

This is T8.2's exposure, not upstream's. Upstream set the flag from the move
event, which only fired at the very end, so a cancel had almost nothing to
undo. T8.2 moved it to `DropPreviewChanged` — deliberately, to keep the pane
dimmed for the whole drag instead of only at the moment it committed — and in
doing so made the flag live for the entire gesture without extending the
cleanup to the gesture's other ending.

`PaneGroup::cancel_drag` could not fix it directly: its pane tree stores
`PaneId`s and its `pane_contents` are `dyn PaneContent`, so it holds no handle
on either `PaneView` or `PaneHeader`. The message travels instead on the
configuration model those two views already share, as
`PaneConfigurationEvent::DragCancelled` — reachable from
`PaneContent::pane_configuration`, and therefore no new trait method on all ten
pane types.

Pinned by `a_cancelled_drag_undims_the_pane_it_was_dragging`, which fails
against the unfixed arm and passes against the fixed one.

**Verified by driving Escape mid-drag**, which needed a new `use_computer drag
--press` (a cancel key is by definition a keystroke that arrives while the
button is still down, which no click-then-type sequence can produce). Two runs
of the same drag, differing only in `--press 0xff1b`:

```
control   dragged pane dimmed (diff 35-45 vs baseline), target pane carries the
          Down overlay (diff 32)
--press   every sampled region 0.0 against the pre-drag baseline
```

and the log says why:

```
PaneHeaderDragged ...
EditorAction::Escape
PaneGroupAction::CancelDrag
WorkspaceAction::CancelDrag
```

#### T9.4c — alt+drag does not merge tabs, and cannot

**The report:** "if i hold alt+drag a tab, i have grown accustomed to that
merging tabs… i'm not sure what this is."

It is not a matter of hitting the right spot. **No drag callback in the app
ever sees a modifier.** `Draggable` destructures `Event::LeftMouseDragged {
position, .. }` and `LeftMouseUp { position, .. }`, dropping the
`ModifiersState` both events carry. The word `modifiers` does not appear in
`draggable.rs`.

The horizontal strip *is* modifier-aware, but only at mouse-down and only for
selection: `tab.rs`'s `on_mouse_down_with_modifiers` gives shift = extend
range, cmd = toggle multi-selection, both behind `FeatureFlag::GroupedTabs`.
Alt is not among them, and none of it survives into the drag.

What merges today is the *pane* drag: drag a pane by its header onto a tab in
the strip and the middle half of that tab merges the pane into it
(`TabBarHoverIndex::OverTab` → `SwitchTabFocusAndMovePane`). Making alt+drag do
it from the tab side is a feature, not a gate — it needs modifiers recorded in
`DraggableState`, a branch in the strip's drag handler, and a merge that can
target a tab other than the active one, which `merge_tab_into_active_tab`
cannot express. Not built.

#### Found on the way: tab → pane cannot fire on the horizontal strip

T8.2 made a tab a drop source for panes, and T9.1 verified it working — **in
the vertical panel**. On the horizontal strip it cannot work at all, and the
reason is three lines apart from the feature.

`tab.rs:2154` activates a tab on **mouse-down**. `vertical_tabs.rs:3312`
activates on **click**, i.e. mouse-up. So on the strip, pressing a tab to drag
it makes it the active tab *before the drag begins* — and
`tab_can_merge_into_active_tab` opens with `index != self.active_tab_index`.
The guard is right: once the dragged tab is active, the panes on screen are its
own, and dropping it on one of them means nothing.

Observed, 2026-08-24: click tab 0, drag tab 1 onto a pane.

```
ActivateTab(0)          <- the click
ActivateTab(1)          <- the mouse-down that starts the drag
StartTabDrag
DragTabOverPane x13
DropTabOnPane
```

Thirteen drag events, a drop, **zero** calls to `calculate_pane_move_direction`
(the new debug line never appears), no preview drawn, nothing merged, tab and
pane counts unchanged.

Not fixed here, because every fix is a decision about tab activation that
belongs to the user: activate on mouse-up like the vertical panel (a real
change to how every tab click feels), or restore the previously active tab when
a press turns into a drag (which changes where a plain reorder leaves you).

#### Correction: synthetic keystrokes *do* land on X11

`CLAUDE.md` says "synthetic clicks land there; synthetic keystrokes still do
not, which is why two tasks are blocked on a person." Too strong, measured
2026-08-24 through the actor's own XInput2 seat:

* **Keymap actions land.** `--press 0xff1b` mid-drag produced
  `EditorAction::Escape` → `PaneGroupAction::CancelDrag` in the log and a
  visibly cancelled drag.
* **Text does not.** `use_computer text "echo hello-from-the-agent"` into a
  focused, accent-bordered terminal input produced nothing at all.

So cancel keys and shortcuts are drivable; typing is not. `Key::Keycode(n)` is
an X **keysym** on this backend, not a keycode — Escape is `0xff1b`.

#### Two tool additions, both small

* `use_computer drag --release-at x,y` — move somewhere inert *after* the
  screenshot and before the release, which turns a drag into a probe. A sweep
  that commits every hit has to rebuild the layout between samples, and the
  layout is where the next sample's coordinates come from.
* `use_computer drag --press <key>` — press and release a key mid-drag, before
  the screenshot.

#### An operational note

The first release build of this pass died in the linker with `ld terminated
with signal 7 [Bus error]`. The disk was **100% full**: `target/` was 134GB, of
which `target/debug/incremental` alone was 66GB. Deleting the incremental cache
(cargo regenerates it) freed it and the build went through in 5m 44s.

### Corrections to T8, from this pass

**A tab *group* cannot be dragged out to a new window, and the axis is not
why.** The T8 closing report named `vertical_tabs.rs:3206` — the group
draggable's unconditional `DragAxis::VerticalOnly` — which reads as though a
flag would open it. It would not. `WorkspaceAction::DropGroup` is
`send_telemetry` plus `ctx.notify()`, and `CrossWindowTabDrag` has no concept
of a tab group at all (its `pane_group` is the split layout *inside* a tab, a
different thing wearing a similar name). Relaxing the axis would produce a
group that leaves the panel and lands nowhere — precisely the bug T8.2 already
hit once and fixed for tabs. This is a feature, not a gate, and it is not
small.

**A cold Windows release build is 19 minutes, not an hour.** `build.ps1`'s own
comment says "a release build from cold is roughly an hour", which is why the
debug profile is its default. Measured 2026-08-23 with no `target\release`
directory present: **18m 48s**, 351,897,600 bytes. Corrected in the script.

---

## T10 — Staying current  ← ONGOING

> Decided 2026-08-24: **this stays a soft fork.** The rename is deferred, and
> the reason is measurable rather than aesthetic — renaming every `warp-oss`,
> `WARP_*`, `WarpOss` and `warpctrl` symbol converts ground the fork currently
> *shares* with upstream into permanent conflict. Until then, upstream merges
> are cheap and the only thing that makes them expensive is letting them lapse.
>
> `CONSOLIDATION.md` §1 records the corrected divergence figures and the
> measurement error that produced the wrong ones.

- [x] **T10.1** Merge `upstream/master` through 2026-08-24 (`6696954c6`).

### T10.1 — as built

**The overlap is the number to watch, not the file count.** The fork touches
204 files; the 110 upstream commits touched 465; they overlap in **39**, and
only **4** of those conflicted. Both figures are cheap to compute, and the first
is the early warning:

```bash
MB=$(git merge-base upstream/master dev)
comm -12 <(git diff --name-only $MB...dev | sort) \
         <(git diff --name-only $MB..upstream/master | sort) | wc -l
```

**The four conflicts, and what each taught.**

* `input_tests.rs` — a fork test and three upstream tests at the same offset.
  Kept both. Purely positional; no judgement needed.
* `user_workspaces/mod.rs` — upstream `8cbb01d45` split the file into a module.
  Took upstream's side and re-applied both account-gate bypasses at their new
  home, where `is_custom_inference_enabled` is now `is_byo_endpoint_enabled`.
  **A rename inside a moved file is the shape most likely to lose a fork edit
  silently**, because the conflict shows the fork's code against *nothing*.
* `warp_agent_page.rs` — upstream's APP-5559 refactor turned a widget `render`
  method into a free function. Took upstream's shape, re-applied
  `fork::is_anonymous_for_ui`.
* `pwsh.ps1` — upstream's lint pass rewrote `(Get-Location).Path` to
  `$PWD.Path` on the two lines T6.2 had replaced with `Warp-Get-Location`.
  **Upstream's new spelling carries the identical UNC bug T6.2 fixed** — both
  are provider-qualified. Kept the fork's. Upstream's three other new
  `$PWD.Path` sites were deliberately left: a cache key, a `Get-Item
  -LiteralPath`, and a `Set-Location` argument, none of which are handed to
  Warp, which is the only place the qualifier hurts.

**The dangerous half was the part no conflict marked.** Git merged three files
cleanly that could not compile, because upstream added code constructing a type
the fork had widened:

* `warp_tui/src/orchestration_model.rs` and `transcript_view.rs` — new upstream
  files with exhaustive matches over `BlocklistAIHistoryEvent`, which T8.3
  extended with `ConversationSettledChanged`.
* `persistence/src/model_tests.rs` — six exhaustive `AgentConversationData`
  literals with no `settled` field.

**That last one was already red before the merge.** T8.3 added a required field
and never compiled `crates/persistence`'s tests; the merge only moved the line
numbers. Same failure mode as the `warpctrl` catalog count pinned in two crates
where only the fast one gets run.

**So the gate after any merge — or any fork change to a shared type — is
`cargo check --workspace --all-targets`.** Not the binary build, which passes
happily: every one of these three lives in test or TUI code the `warp-oss`
target never compiles.

---

## T11 — Observability first, then the surface  ← DONE (2026-08-26)

> Ratified 2026-08-24. The goal, in the maintainer's words: a project you can
> *"spin up, check in on, fire a session from my phone and keep things moving
> semi-autonomously"* — in the pilot's seat rather than the labourer's.
>
> **The ordering is load-bearing and has two independent reasons.**
> `tusk/docs/handoffs/remote-control-feature-mining.md` argues read-only-first
> because nearly all the *value* is in the read path and nearly all the *risk* is
> in the write path — the engine spawns agents and runs tools, so the write path
> is RCE. This fork adds a second: the failure that cost the maintainer a month
> of kode-rs work was a **silent** one — a swarm of agents running without
> permissions, nothing surfacing it, features implemented but never wired, and
> docs written as though they were. An event taxonomy is the detector for exactly
> that class, and a stream with nothing structured on it is a pipe with no
> protocol. So events come before the surface that carries them.
>
> Migration tiers and their reasoning: `CONSOLIDATION.md` §4.1.

- [x] **T11.1** A structured event taxonomy (`TR-EVENTS`). The taxonomy already
      existed; what was missing was anywhere to keep it. Built as a projection:
      `WARP_FORK_EVENT_LOG`, one JSONL file per session.
- [x] **T11.1b** Warp's own agent into the same log. World 1
      (`BlocklistAIHistoryEvent` / `BlocklistAIActionEvent`) projected onto the
      same vocabulary, so one `jq` filter answers for every agent in the window.
- [x] **T11.1c** The local agent's tools into the log. **Found by running
      T11.1b, not planned**: on this fork's primary agent path the log carried
      the turn frame and no tools at all, because neither world sees them. Built
      as a third source — `source: "local_agent"` — read off the `stream-json`
      `translate.rs` was already parsing, filed under Warp's conversation id so
      a turn's frame and its tools are one file. It carries `parent_call_id`,
      which no other source can: subagent nesting, also found by running it.
- [x] **T11.2** The first slice of the read surface, end to end and deliberately
      small: **one** `GET` route on `warpctrl` returning current agent/task
      state, **one** SSE endpoint carrying T11.1's events, still bound to
      `127.0.0.1`. No LAN bind, no QR pairing, no web page. It proves transport,
      auth and fan-out against a running app; everything after it is additive.
- [x] **T11.3** Constant-time token comparison — filed as a **prerequisite for
      any bind wider than loopback**, and **the reason given for it was wrong**.
      As filed: `AuthToken(String)` derives `PartialEq`, so
      `verify_authorization_header` short-circuits on the first differing byte
      and leaks a prefix. Two things were assumed rather than checked, and both
      failed:
      - **`verify_authorization_header` has no production callers.** It is
        exercised only by `auth_tests.rs`. The live path is
        `app/src/local_control/mod.rs::lookup_credential`, a
        `HashMap<String, CredentialGrant>::get(secret)`.
      - **A hash lookup is not a prefix oracle.** `RandomState` seeds SipHash
        per process, so a near-miss hashes to somewhere unrelated and reveals
        nothing about how many leading bytes were right. `String == String` is
        not a byte loop either; it lowers to `bcmp`.

      Done anyway, and the entry is corrected rather than dropped, because the
      function is public crate API named exactly what an auth check is named,
      and T11.2 and T11.4 are precisely when someone reaches for it. See the
      as-built for what the test does and does not pin.
- [x] **T11.4** LAN bind behind an explicit flag, plus QR pairing. `warpctrl`
      hardcoded `[127, 0, 0, 1], 0`. Built as a **second** listener rather than a
      moved one, and as a three-step pairing flow whose only displayed secret
      lives two minutes. The must-have that was not on the list turned out to be
      the important one: the catalog contains `input.submit` and `agent.prompt`,
      so a pairing path that mints any credential *is* the RCE the ticket was
      trying to prevent. A paired device gets three read actions.
- [x] **T11.5** `GB-APPROVE` — answer a waiting-input approval remotely. The
      first *write* capability. Two findings reshaped it. **The approval that
      matters was invisible**: `agent.list` reports Warp's own conversations, and
      on this fork the agent a person actually blocks on is a `claude` in a pane,
      which has none — so `agent.approvals` had to reach a different map
      entirely. **And approval here is a keystroke, not a verdict**: Warp has no
      channel to tell a CLI agent "yes", so `agent.approve` presses Return and
      `agent.deny` presses Escape, which is why they are two actions — a paired
      device holds a list of actions, and `deny` travels to a phone while
      `approve` does not without `WARP_FORK_REMOTE_APPROVE`.
      **`GB-GRANTS` was not built, and the as-built argues it should not be.**

**T11 is closed.** All five items shipped and the phase's own framing —
*observability first, then the surface* — has had its first half delivered and
its second half only half-built: there are routes and there is no client.
That is what T12 is.

---

## T12 — The console: the client T11 was built for  ← DONE (2026-08-27)

> Scoped 2026-08-26. **The argument for doing this before anything else is the
> fork's own anti-goal.** T11 shipped an event log, a state snapshot, an SSE
> stream, a LAN listener, QR pairing and remote approve/deny. Every one of them
> is reachable only by `curl`. The failure this fork was started over — recorded
> in `CONSOLIDATION.md` and restated in T11's framing — is *"features
> implemented but never wired, and docs written as though they were."* T11 is
> currently in exactly that state, and it is the one state this phase exists to
> detect. Five tickets deliver none of the ratified goal until something renders
> them.
>
> **The second reason is that rendering is a test.** Whether `/v1/state` carries
> the right fields is not answerable by reading it; it is answerable by trying to
> draw a screen from it and finding out what is missing. Expect T12 to send work
> back into T11's shape, and expect that to be the valuable part.

**Gate check, run 2026-08-26 before scoping.** `include_str!(*.html)`, `Html(`,
`ServeDir` and `text/html` across `app/src`, `crates/local_control` and
`crates/http_server` return exactly one hit — a MIME-extension table in
`app/src/ai/artifact_download.rs`. **Nothing serves a page anywhere in this
fork.** Unusually for this board, the answer is "not already built". `axum` is
already a workspace dependency of `app`, so the route itself costs nothing.

**And the web-surface question from `CONSOLIDATION.md` §10 step 3 is hereby
settled, by reading both.** They are different needs and neither covers the
other: Warp's remote-development server (`wsl_transport.rs`) puts a *shell* on
another machine, while `tusk/engine/src/serve.rs` puts a *view of a running
session* on a phone. The fork now has the second one's entire backend and no
front end. Tusk's front end is also the precedent for the size: a 162-line
`serve_index.html` served as an embedded fallback, with an optional built Svelte
client behind `--web-dir`. **Take the 162-line half and not the Svelte half** —
a build step in this tree would be a new toolchain for one page.

- [x] **T12.1** One route, one embedded page, read-only. `GET /` on `warpctrl`'s
      listener returning a single self-contained HTML file: no build step, no
      npm, no framework, no external fetch. It primes from `/v1/state` and then
      follows `/v1/events`. **The hard part is not the page, it is that this is
      the first browser-reachable surface on the authenticated control plane** —
      so the design questions to answer *before* writing markup are: where the
      device token lives (fragment, never the query string, which lands in logs
      and referrers), what the CORS and origin policy is, and what escaping rule
      applies to agent-authored text, which is attacker-influenced by
      construction.
- [x] **T12.2** Approvals on the page. Render `agent.approvals`, and wire the
      two answers with the asymmetry T11.5 established: `deny` is pairable and
      always present, `approve` appears only when the instance advertises it.
      The page must learn that from the server rather than assuming it — a
      button that 403s is worse than a button that is absent.
- [x] **T12.3** Installable, and the QR points at it. A manifest and an icon so
      it is a home-screen app rather than a tab, and `control.pair`'s QR encodes
      the *page* URL rather than a bare token — pairing that ends at a page a
      person can use is the difference between a demo and a tool.

## T13 — The run gate (ratified Tier 1, items 4 and 5)  ← ACTIVE

> `CONSOLIDATION.md` §4.1 orders these fourth and fifth, and they are the other
> half of the maintainer's sentence: T12 delivers *"check in on"*, T13 delivers
> *"keep things moving semi-autonomously"*. They land as validators over
> `crates/warp_cli/src/local_control/graph.rs`'s TOML — §5's *"migrate toward
> the file, not the schema"* — because that file is already the fork's best
> expression of the smallest-thing rule and it added zero app surface.
>
> **Constraint carried from §12: write these fresh, do not copy Tusk's.**
> Migrating a *concept* is free; migrating Tusk source into an AGPL tree before
> the §10-step-1 extraction is the one-way door that step exists to hold open.

- [x] **T13.1** `ZB-PLAN` — the sealed-subgraph guard, as `warpctrl graph check`
      over an existing plan file. **The guard had nothing to guard, and finding
      that was the ticket**: a plan that re-runs from scratch every time can
      never reuse evidence, so no edit to it can invalidate any. What was
      missing was Tusk's own §7 promotion trigger, and here that trigger is
      `--resume`. Built as record → resume → guard.
- [x] **T13.2** `ZB-CONTRACT` — per-assertion verdicts. A node declares what
      must hold after it, and the runner records a verdict *per assertion*
      rather than one pass/fail per node. This is the same detector T11.1 built
      for events, applied to work instead of to activity. **An assertion is a
      command, not a sentence** — the statement and the evidence are the same
      string — and a node whose assertion fails is `rejected`, a fifth state.
      Tusk's two open decides are both answered by the file: the contract lives
      on the node, and nothing produces a verdict except the command itself.
- [x] **T13.3** `ZB-REVIEW` — an independent completion-review gate: the agent
      that checks is not the agent that did the work. **There was no reviewer to
      build** — a review is a *node shape*, not a node kind, and Tusk's whole
      no-transcript overlay collapses into `agent.spawn`, which has no other
      mode. What was built is the fence: `review = true`, three refusals in
      `validate`, and the recipe in the schema. The tension with T13.2 resolves
      by demoting the verdict — **a model's answer may narrow acceptance, never
      widen it.**

## T14 — ACP as the adapter contract  ← DECIDED 2026-08-27, and it is now a build

> **As filed:** a decide. The reasoning in §4.1 was that the fork speaks the
> *client* side, and every ACP agent — Gemini CLI, Claude Code via Zed's adapter
> — arrives without a per-agent integration. The crate is Apache-2.0, which an
> AGPL work may depend on, and it was "already on this machine" at
> `~/git/agent-client-protocol-main`. The slots exist twice (`CLIAgent`,
> `Harness`). **Unverified:** that the crate's client side is complete enough to
> drive an agent, which is a reading job before it is a building one.

**Four of those sentences were wrong, and the sync that found it took ten
minutes.** The one input nobody thought to check was the dependency's own
freshness — the ticket said "already on this machine" and treated that as
currency.

### What the sync found

`~/git/agent-client-protocol-main` **was not a git repository**. It was an
unzipped snapshot of `main` at **v0.3.0**, file mtimes **2025-09-12**. Because
it was not a clone there was no `git pull` that would ever have corrected it,
and no `git status` that would ever have looked stale. It has been deleted;
fresh clones are at `~/git/agent-client-protocol` (spec) and `~/git/acp-rust-sdk`
(SDK).

| the ticket said | measured 2026-08-27 |
|---|---|
| the crate is at `agent-client-protocol-main` | that path held v0.3.0 from 2025-09-12, **two major versions** stale |
| (implied) the SDK is in the spec repo | the spec repo **deleted the SDK** in `935857f` ("Remove SDK code (#155)") |
| Zed's protocol | moved org to `agentclientprotocol/`, now carries `GOVERNANCE.md`, `MAINTAINERS.md` |
| Apache-2.0 | **still true** — the one premise that held |
| "Gemini CLI, Claude Code via Zed's adapter" | **39 agents** in a stabilized registry |
| the slots exist twice (`CLIAgent`, `Harness`) | both are the wrong seam — see below |

The Rust SDK now lives at `agentclientprotocol/rust-sdk`, published as
`agent-client-protocol` **v2.0.0** (2026-07-23, Apache-2.0), a ten-crate
workspace. The `claude-acp` registry entry lists **Anthropic, Zed and JetBrains**
as authors, so "betting on one vendor's protocol" is no longer the objection it
was when this was filed.

### The unverified input, settled by running rather than reading

The ticket called this "a reading job before it is a building one". It was a
*running* job, and it took two commands.

- **`testy`** (`agent-client-protocol-test`) driven by the SDK's
  `yolo_one_shot_client`: `initialize` → `session/new` → `session/prompt` →
  streamed `AgentMessageChunk` → `EndTurn`. Offline, deterministic, no quota.
- **The real Claude agent**, `npx -y @agentclientprotocol/claude-agent-acp@0.70.0`:
  same loop, answered on the maintainer's own subscription with **no Warp
  account involved**, `EndTurn`, $0.21.

So the client side is not merely "complete enough" — it is a builder API, and
the whole client in the SDK's example is 113 lines including clap and comments.

**Three things came back that the fork has no equivalent for today**, and they
are the actual argument for adopting ACP:

- `Implementation { name, title, version }` — the agent **identifies itself**,
  which is precisely what replaces a closed `Harness` enum.
- `UsageUpdate { used, size, cost, rateLimit }` — live token, context, cost and
  rate-limit status *pushed from the agent to the client*.
- `AvailableCommandsUpdate` — the agent's own slash commands, enumerated.

**And the gate is open.** `cargo add agent-client-protocol@2.0.0 -p warp_cli`
resolves in **14 new packages**, disturbing existing deps only by a `futures`
patch bump (0.3.31 → 0.3.34), and `cargo check -p warp_cli` compiles it in 13s.
`cargo check --workspace --all-targets` passes with it added. That was the one
claim the advisor could only read; it has now been run.

**The one cost measured rather than assumed:** that `futures` bump deprecates
`UnboundedReceiver::try_next`, which **upstream** code uses — 4 new warnings in
`app/src/ai/blocklist/orchestration_event_streamer.rs`. Confirmed by diffing
against a same-session baseline: **0** occurrences before, **4** after. Warnings,
not errors, and the fix is `try_recv`; but it means adopting ACP puts warnings in
files the fork does not own, which is a merge-noise cost the dependency count
alone does not show.

### The decide

**ACP is the adapter contract for every agent that is not Claude;
`app/src/ai/local_agent/` stays the Claude path.** Reaching Claude over ACP means
an `npx`-launched, `license: proprietary` shim in front of a CLI this fork
already drives directly — a regression on the flagship agent for a fork whose
thesis is the user's own subscription with no intermediaries. Verified: `claude`
2.1.247 has **no `--acp` flag**.

**The premise that survives the challenge: ACP does not delete `translate.rs`.**
The expensive half of the fork's agent work is the *output* side — Warp's
`ResponseEvent` mutation log. In `app/src/ai/local_agent/translate.rs` (830
lines) the Claude wire types are lines 35–262, ~28%; the other ~72% is
Warp-protocol construction that any input front-end still needs. **ACP is a
better input, not a smaller problem.**

**Both named slots are the wrong seam.** `CLIAgent`
(`app/src/terminal/cli_agent.rs:140`) is terminal *decoration* — prefixes, icons,
brand colours — with no transport behind it to swap. `Harness`
(`crates/warp_cli/src/agent.rs:227`) is a cloud-run selector whose local half
reaches `ThirdPartyHarness::build_runner`; that *is* an adapter slot, and it is
the expensive one — **8,148 lines for three agents**. Upstream hand-wrote drivers
for 3 of the 39 registry agents; `CLIAgent` names 15 third-party agents, of which
**13 are in the registry**.

**The seam is the one the fork already owns** — `app/src/ai/agent/api/impl.rs:20`,
the single `if` in front of `generate_multi_agent_output` that T5 opened. An ACP
agent is a second arm of that same condition, behind `fork::acp_agent_enabled()`
next to `local_agent_enabled()` at `app/src/fork.rs:196`, with
`app/src/ai/acp_agent/` as a sibling of `local_agent/`. The reuse is **not** free:
`init()`, `add()`, `message()`, `timestamp()` lift cleanly, but `assistant()`
takes a Claude-typed `AssistantMessage` — a deliberate ~400-line extraction, not
a split.

**Schema v1, and not because v2 is alpha.** `docs/protocol/v2/migration.mdx` is
unambiguous: **v2 removes `fs/read_text_file`, `fs/write_text_file` and all five
`terminal/*` methods from the client**, replacing them with client-provided MCP
servers. Those are exactly the capability that makes ACP interesting to a
*terminal*. Maturity is a free second argument — all 39 agents ship v1 today, and
in the SDK v1/v2 is a cargo feature (`unstable_protocol_v2`), so v2 later is a
flag rather than a rewrite.

**The registry: consume it as data, never as an installer.** The `distribution`
block carries per-platform GitHub release URLs with sha256 — implementing it
means downloading and executing third-party binaries from the network, on the
project whose thesis rests on `crates/http_client/src/egress.rs`. Use it for
*recognition and configuration* of an agent the user already installed: the `id`,
the display name, and the args that put it in ACP mode (`--acp` for Gemini, `acp`
for goose and OpenCode). A **vendored snapshot refreshed deliberately**, not a
fetch at startup.

### The prize, which is bigger than "more agents arrive free"

`local_agent/mod.rs`'s own doc comment names its limit: Claude runs its own
tools, so Warp's diff review, command approval and block UI *do not participate*.
ACP v1's client side — `session/request_permission`, `fs/read_text_file`,
`fs/write_text_file`, `terminal/create|output|release|kill|wait_for_exit` — is
exactly that surface, **as a published spec**. Warp is a terminal with a
permission model and a diff reviewer. This is the one route where tool
participation is a document to implement rather than a private protocol to
reverse-engineer per agent.

- [x] **T14.1** The probe, and no app surface: a hidden `--warpctrl` subcommand
      (**not** a catalog action — that pays the two-test pin tax for something
      whose job is to be deleted or promoted) that runs `initialize` →
      `session/new` → `session/prompt` and prints every `SessionUpdate` as JSON.
      ~80 lines. Its real output is **the mapping table** the app work needs:
      real `SessionUpdate` variants matched arm-by-arm against `translate.rs`'s
      existing `ResponseEvent` constructors. `testy` is the gate; the npx Claude
      shim is the evidence. Composes with `graph.rs` for free — an assertion is
      a command, so the probe is directly assertable in a plan node.

**Decided by the maintainer 2026-08-27:** (A) **No** — "no intermediaries" does
not tolerate an npx-launched proprietary shim on the Claude path, which is what
buys the two-path split above. (B) remains open, and is now sharper than when it
was asked; see T14.1's as-built.

### T14.1 — as built

**The probe is `warpctrl acp probe`**, hidden, not a catalog action, a sibling of
`mcp` and `completions` in `ControlCommand`. `crates/warp_cli/src/local_control/acp.rs`,
~170 lines with the doc comments, plus 7 tests. It runs `initialize` →
`session/new` → `session/prompt` and prints one JSON object per line.

**No async runtime was added, and one nearly was.** `warp_cli` deliberately has
none — `mcp.rs:17` says so — and the SDK's own example is `#[tokio::main]`, which
made it look like adopting ACP meant adopting a runtime. It does not:
`agent-client-protocol` reaches the OS through `async-io` and `blocking`, both of
which drive their own threads, so `futures::executor::block_on` hosts the entire
exchange. Reading the SDK's *example* would have given the wrong answer; reading
its `Cargo.toml` gave the right one.

**Permission requests are denied unless `--approve` is passed**, following the
asymmetry from T11.5 and T13.3 — saying no can only ever make less happen.

**What running it against a real agent found, and it changes (B).** Against
`npx -y @agentclientprotocol/claude-agent-acp` in a scratch directory, asking it
to list files and read one: **20 updates, and exactly five variants** —
`usage_update` (8), `tool_call_update` (6), `tool_call` (2),
`available_commands_update` (2), `agent_message_chunk` (2). `tool_call` carries a
vendor-neutral `kind` (`execute`, `read`) with the vendor's own name tucked in
`_meta.claudeCode.toolName`, so Warp could render blocks by kind **without
knowing which agent it is talking to**. That is the mapping table, measured
rather than guessed.

**And zero permission requests arrived — while the agent ran `ls -la` and read a
file.** That is the finding. Because the probe advertises no `fs/*` and no
`terminal/*` client capabilities, Claude simply ran its own tools and reported
what it had done. So the open question (B) is not "should the fork implement the
client side eventually" but **"without it, ACP buys nothing over `local_agent`"**
— it delivers the same read-only view of an agent doing its own thing that
`local_agent/mod.rs`'s doc comment already names as its limitation. The
ecosystem argument (39 agents, no per-agent integration) survives intact and is
the reason to adopt; the tool-participation argument is entirely contingent on
(B), and this run is what proves it rather than assuming it.

**Named unverified.** The `--approve` path has **never been exercised against a
live agent**, because no agent asked — and none will until the fork advertises
the capabilities that make asking meaningful. Its logic is tested, its behaviour
is not. Nothing here has run on Windows. And the mapping table is one agent, one
prompt: `usage_update` dominating the traffic is a Claude-wrapper trait that may
not generalise, and no `plan`, `diff` or elicitation update appeared at all.

## T15 — Loose ends carried, not forgotten

- [ ] **Re-check `ALLOW_VERIFIED_AGENTS` against a real prompt** (from T11.5).
      The named unverified input: the permission prompt that proved the path was
      synthesised, so the claim that Return means yes rests on Claude Code's
      documentation rather than on this fork having watched one. The cheapest
      check is answering one real prompt with `warpctrl agent approve`.
- [ ] **The discovery record that outlives the process.** Across three clean
      `warpctrl window close` shutdowns during T11.5 the discovery record and
      broker socket were left in the scratch directory with no process alive —
      which contradicts `CLAUDE.md`'s claim that ordinary shutdown cleans both.
      Unbisected. Nothing in T11.5 touches discovery.
- [ ] **`kode-engine` and Tusk's pure cores extracted as MIT/Apache crates**
      (§10 step 1). Not work in this repo, but a *dependency* of T13 and of any
      later migration, and §12 forbids the migration until it is done.

### T12 — the browser pass, and the "no browser on this machine" claim retracted

**Three as-builts below say a real browser has never loaded this page. That was
true of the WSL side and false of the machine, and the difference cost nothing
only because it was caught the same week.** `/mnt/c/Program Files` holds Firefox,
Brave and Zen; Windows reaches the WSL wide listener (`Invoke-WebRequest` →
`200`); and `C:\dev\shot.ps1` has been in the operating manual since T9. So the
item filed three times as *"one session with a phone"* was, for its most
important half, one command away the whole time.

The mistake was a scope word. "No browser on this machine" meant the Linux
userland the agent runs in, and got written as though it described the hardware.
The fork's own rule — *name the inputs you did not verify* — was followed; what
was not done was checking whether the named blocker was real.

**What a real browser proved that `node` could not, 2026-08-27.** Scratch
profiles for both browsers, so nothing touched the maintainer's own session.

| | result |
|---|---|
| Firefox, page load | renders; `<title>` correct; paired from a scanned code; badge `live` |
| Brave (Blink), page load | same, at a 430px viewport — the approval card, both buttons, full-width targets |
| **markup in agent text** | an agent asking to run `` rm -rf build/ && echo <b>not markup</b> `` drew the angle brackets **as text** in both engines |
| the two-tap `Yes` | clicked by hand by the maintainer; agent read `0d` |
| `agent.deny` from the CLI | agent read `1b` |

**The escaping row is the one that mattered.** T12.2's as-built says outright
that its `<img onerror>` payload "looks like a demonstration and is not one",
because a DOM shim with no HTML parser reports any string verbatim. A real Blink
and a real Gecko parser have now received attacker-shaped text through the same
path and rendered it inert. That claim is no longer resting on the test alone.

**And the two-tap arming was confirmed by a person, which is the only way it
could be.** The maintainer clicked `Yes` twice without being told to, describing
it as "twice (per design)" — so the armed state reads as deliberate rather than
as a button that failed the first time. No capture answers that question;
T12.2's as-built said so and was right to.

**One false alarm, recorded because the reasoning was worth more than the
result.** The first Firefox screenshot showed the approval already answered, and
the fake agent had read `0d` after the point where nothing of ours had written to
that PTY. The suspicion — that Warp itself writes a stray `\r`, which for a real
agent would be an *accidental approval* — was serious enough to stop and isolate
rather than wave off. It was the maintainer clicking `Yes`. The escalation was
still correct: an unexplained byte reaching an agent's stdin is exactly the
silent-failure class this phase exists to detect, and "probably nothing" is not
an answer to it.

**What is still unverified, and it is now a short list.** Nothing about desktop
browsers. What remains is *installation on the phone that will actually be
used* — **Firefox on Android**, with DuckDuckGo as the Chromium fallback. The
install rows in `README.md` were originally written around iOS Safari and Android
*Chrome*, neither of which is this maintainer's phone; they are corrected there
and the Firefox row is the one still open.

### T13.3 — as built

**The ticket asks for a reviewer and there was none to build.** Tusk's
`ZB-REVIEW` is a *run mode*: a fresh engine run given the original prompt and
the worktree, with a forced read-only overlay and the prior transcript
deliberately withheld. Every one of those is a thing Tusk had to add, because
its runs inherit context by default.

Here a review is a **node shape, not a node kind**. `agent.spawn` starts a fresh
conversation knowing only its prompt — `parent_conversation_id` writes a link in
the parent/child index and copies nothing (`history_model.rs:583,599`), which is
why an empty prompt is refused outright. `allow_tools = ["read-only"]` already
exists and resolves to ten read tools, mapping on the fork's primary path to
`--allowedTools Read,Grep,Glob --disallowedTools Bash,Write,Edit,…`. An ordering
edge that hands nothing along already exists. **So Tusk's overlay collapses into
the spawn primitive: the no-transcript construction it had to build and test is
the only mode this fork's spawn has.**

**Three for three.** T13.1's sealed-subgraph closure became a filter, T13.2's
coverage invariant became a uniqueness check, and now Tusk's independence
overlay becomes nothing at all. The common cause each time is that this fork
writes the relationship on the thing itself rather than in a side table — and
here the thing written on itself is *context*: there is no side table of history
for a child to be accidentally handed.

**The tension with T13.2, and how it resolves.** T13.2 ruled that asking a
second model whether the first model's claim is true is a claim about a claim,
and named it a non-goal. T13.3 looks like exactly that. It is not, and the
distinguishing variable is *what the judge reads*: a reviewer denied the
transcript has no claim in front of it, so it produces a fresh, **uncorrelated**
claim about the world. The failure T13.2 named is correlated error — a judge
inheriting the builder's frame — and independence severs it.

What does **not** escape is that the reviewer's *verdict* is still model
judgement rather than falsifiable evidence. So it is stripped of authority:

> **A model's answer may narrow acceptance, never widen it.**

The review composes with assertions as AND-only — a detector wired to an exit
code, never an approver. Its unreliability is then asymmetric-safe: a false
"gaps" costs one human read, and a false "complete" leaves you exactly where
"no assertion failed" already left you. **A review can only usefully fail**, and
its gaps — unlike its verdict — are individually falsifiable by looking. That is
the same asymmetry the fork already ratified in T11.5, where `agent.deny` needs
no switch because saying no can only make less happen.

**So what shipped is a fence.** `review = true` on a node, read only by
`validate`, refusing the three edits that silently turn a reviewer into a rubber
stamp — each of which leaves a plan that still runs and a gate that still says
yes:

| refused | because |
|---|---|
| a `pass` edge into a review | a handoff appends the upstream answer to the prompt, so one `pass = "what I did"` hands the reviewer the exact claim it exists not to see |
| a review naming its own `allow_tools` | there is one right answer, and a reviewer that can write can make its own verdict true |
| a review not downstream of every working node | its input is the *working tree*, which is global, so an early review reads a workspace mid-edit and does it differently every time |

`review` joins the fingerprint, because un-marking a reviewer is one word long
and changes what its answer meant.

**One deliberate divergence from the advice taken.** The recommendation was to
refuse an allowlist *wider* than read-only; this refuses **any** `allow_tools` on
a review, and resolves it to `read-only` regardless of `[defaults]`. Offering the
choice is what invites the wrong one, and a plan whose `[defaults]` are wide was
the case a fence over the node's own field would have missed. Pinned by a test
that refuses even `allow_tools = ["read-only"]` itself.

**Gap handling composes with T13.1 with no glue, and this is the good part.** A
rejected review is not sealed, so the fix pattern is *append a node*: add the
fix downstream, add it to the review's `needs`, `--resume`. Sealed work is
reused, the fix runs, the review re-runs. That is Tusk's "gaps spill as proposed
tasks" achieved as a workflow instead of an object model — the supersede/patch
door T13.1 kept shut stays shut, nothing auto-edits the plan, and the person is
the gap-materialiser, which is Tusk's own posture too.

**Rejected:** an assertion whose command spawns an agent (wrong on scope — an
assertion gets one node's output, a review needs the plan's intent; on budget —
`ASSERT_TIMEOUT` is 120s and "an assertion is a check, not the work"; and on
authority — it would let a model verdict borrow command-grade authority). A
`warpctrl graph review` subcommand (a second runner for a one-node graph). A
plan-level `[plan] intent` field (restating intent in the review's prompt is the
same authoring work, and needs no new field). A `git` baseline in the run record
(the completion question is "does the tree satisfy the requirement", which the
tree answers alone; a baseline distinguishes "the agent did it" from "it was
already so", and for a completion gate those are the same verdict).

**Verified by running, 2026-08-27**, and the first run failed in the way that
mattered.

| | result |
|---|---|
| all three fence refusals | each refused by name, no Warp needed |
| the schema's own review node | still parses, validates, and lands last in `waves` |
| run 1 — worker claims "I migrated every file", `src/b.rs` still calls `old_api()` | **reviewer read the wrong tree** — see below |
| run 2, after the fix | reviewer named `/tmp/t133/work/src/b.rs` exactly, gate failed, node `rejected` |
| gap closed by hand, `--resume` | `fix` reused, review re-ran, `NO GAPS FOUND`, `done` — 16 s |

**The finding, and it is the reason to run things.** `agent.spawn` takes no
working directory (`AgentSpawnParams` is four fields), so a spawned child starts
in the **pane's** cwd, which has nothing to do with the directory `graph run`
was invoked from — where the assertions run. The first live review, launched
with Warp started from the repo and the plan run from `/tmp/t133/work`, read the
repo and said so: *"There is no `./src` directory in the working tree
(`/home/effatha/git/warp`)"*. **Its gate failed — for the wrong reason.** A
reviewer whose entire input is the workspace had been pointed at a different
one, and every check still went green-then-red in a way that looked like
success. `compose_prompt` now appends one line to a review node naming the
absolute workspace, and the re-run found the planted gap. Nothing but running it
would have caught this: the fence, the tests and the schema were all correct.

Not in the fingerprint, deliberately — the fingerprint is about *plan* edits and
a directory is environment, the same reason `assert = ["cargo check"]` has one
fingerprint wherever it runs.

**Named unverified inputs.** The **residual independence leak is real and is not
fixable by `validate`**: the reviewer has read tools and `plan.toml.run.json`
holds every node's answer verbatim, so it *can* read the claims if it goes
looking. Independence here is structural at spawn and only instructed at
runtime — the same residual Tusk carries, whose worktrees hold the agent's own
notes. The mitigation is the prompt line telling it not to, plus keeping the
record out of the reviewed tree; neither is enforcement. Also unverified: the
sentinel is a **protocol, not a truth check** — `grep -qx 'NO GAPS FOUND'` is
format-fragile, though it fails safe, since a mangled sentinel is a false
rejection costing one read. And nothing here has run on Windows.

**A correction this task turned up, unrelated to it.** `CLAUDE.md` and
`.fork/README.md` both said the catalog holds **109 actions**. The pins say
**114**: T11.2 added `events.subscribe`, T11.4 added `control.pair`, and T11.5
added `agent.approvals`, `agent.approve` and `agent.deny` — each updating both
count tests and neither doc. Corrected in both, and the README now separates the
catalog size from the 109 that were actually run in the enumerated live-build
campaign, because bumping that number would have silently extended a
verification claim to five actions the sweep never touched.

### T13.2 — as built

**Tusk filed this as `decide → build` with two gating questions, and the file
answers both.** *"Where does the contract attach — a task field, a Harness
Profile section, or a new `acceptance` config domain?"* and *"who produces
verdicts — the ZB-REVIEW reviewer voting per-assertion, or a separate run?"*
Those are hard because Tusk has a config surface, a database and a UI to place
it in. Here there is a TOML file, so the contract goes on the node, and §5's
*"migrate toward the file, not the schema"* did the deciding.

**The second question got the answer the ticket did not offer, and it is the
whole design.** Both of Tusk's options produce a verdict by asking a model. This
fork's answer is that **an assertion is a command**:

```toml
assert = [
  { id = "compiles",   run = "cargo check --quiet" },
  { id = "no-old-api", run = "! grep -rq old_api src/" },
]
```

The reasoning is one line long. An acceptance contract exists to be
*falsifiable*, so **the statement and the evidence are the same string.** A node
that reports "the tests pass" is making a claim, and asking a second model
whether the first model's claim is true is a claim about a claim — the exact
shape of the failure this fork was started over. `cargo check --quiet` cannot be
talked around. A model-judged assertion is therefore not a smaller version of
this, it is the degraded one, and it is a stated non-goal rather than a
deferral.

**Two spellings and one concept**, taking the shape `needs` already has:
`assert = ["cargo check --quiet"]` names itself, and the `{ id, run }` form
exists for when the command is too long to read as a label.

**The coverage invariant collapses, and this is the second time.** Zenith's
*"exactly one active owner per assertion"* has two real failure modes — an
assertion nobody owns, and one two tasks both own — because its contract lives
beside the plan and tasks *claim* entries from it. Here the assertion is written
inside the node that owns it, so it has exactly one owner by construction and
neither failure mode is expressible. What survives is that two assertions on one
node must not share an id, or a verdict could not say which one it is about, and
`validate` refuses that. T13.1 deleted the sealed-subgraph closure the same way.
**Twice now, writing a relationship on the thing itself rather than in a side
table has turned an invariant into a type.** That is worth watching for in
T13.3.

**A fifth `NodeState`, argued rather than assumed.** `Rejected` is not a second
kind of `Failed`, for exactly the reason the enum already gives for `Skipped`
not being one: a reader acts differently. *Failed* is the agent erroring and is
usually worth running again. *Rejected* is the agent finishing and an assertion
disagreeing — running it again unchanged produces the same thing, and what needs
editing is the prompt or the gate. It keeps the node's `output`, because the
claim is what you debug from, and that was the argument that settled it: the
alternative shape lost the answer at the moment it became interesting.

It composes with T13.1 without a line of glue. `Rejected` is settled, so it is
recorded; it is not `Done`, so it is not sealed, so `--resume` runs it again and
editing its assertion is not a violation — **which is exactly the workflow**:
the gate says no, you fix the gate or the prompt, you resume. Verified live.

**And the assertions are in the fingerprint**, which is the other half. Loosening
a gate on a node that already passed is the single most invalidating edit
anybody can make to a plan, and it was the one edit T13.1's guard would otherwise
have been blind to.

**Three threads per assertion, and that is not belt-and-braces.** A child that
never reads stdin blocks *us* once the pipe fills; a child that writes more than
a pipeful blocks *itself* while we poll for its exit. Either is a hang, a real
agent answer is bigger than a pipe buffer, and a `cargo check` on a broken tree
emits far more than one. Both are pinned by tests that pass a megabyte through.
`ASSERT_TIMEOUT` is fixed at 120s and deliberately not a knob — an assertion is a
*check*, not the work, and a plan needing longer has put the work in the wrong
place.

**Verified by running, 2026-08-27.** Release build, WSLg, scratch XDG,
`WARP_FORK_LOCAL_AGENT=1`, a three-node plan against a real `claude`.

| | result |
|---|---|
| `graph check` on a plan asserting the same id twice | refused by name, no Warp needed |
| the schema, which now contains assertions | still parses and validates as a plan |
| run 1 | `hello` **done** (2 gates ok), `strict` **rejected**, `after` **skipped** |
| the record | per-assertion verdicts under both nodes, exit code and the failing gate's first line of stderr |
| a passing verdict in the file | no `detail` key at all — an empty one is omitted |
| `graph check` after the rejection | `1 sealed` — only `hello`; work that did not hold up is not evidence |
| editing the rejected node's gate | **no violation** — that is the fix, not a reach-back |
| `--resume` after that edit | `hello` reused, `strict` re-ran and passed, `after` finally ran (8 s) |
| loosening `hello`'s gate to `true` | **refused** — the assertion is in the fingerprint |
| `--output-format json` | verdicts on the live event *and* in the summary; exit 1 |

**One thing running found that no test would have.** The per-assertion lines are
printed as they happen — minutes earlier, interleaved with every other node — and
the block after `---` is the part anyone actually reads. In the first live run
that block said `strict: rejected — an assertion says otherwise, and it said: ok`,
which names the *agent's answer* and not the gate: the reader learns only that
something disagreed, which is the fact they already had. The summary now prints
each failing verdict under its node, and the state line no longer quotes the
output. Rebuilt and re-run to confirm.

**Named unverified inputs.** **Windows is the big one**: `shell()` picks
`cmd /C` there and no `cmd` has ever run one of these. The command-running tests
are `#[cfg(unix)]` on purpose — pinning `cmd` spellings from a machine that
cannot execute them would assert a guess, which is the failure mode
`.fork/IDEAS.md` marks its own claims for. Also unverified: any assertion that
is slow enough to meet `ASSERT_TIMEOUT` (the live gates were `grep` and `exit
4`), and the `code: None` path, which only a killed or unstartable command
reaches — that one is held by a unit test.

**The obvious next thing, deliberately not built.** Assertions are commands, so
re-running them costs nothing, and a `graph check --verify` that re-ran every
sealed node's gates would catch a pass that has since gone stale. It is not here
because `reusable` carries verdicts rather than re-deriving them, on the
principle that **the record is a record of the run** — a verdict is part of what
happened, not a live probe, the same as the node's answer. The command that asks
the world again is `graph run` with no `--resume`. If that principle turns out
to be wrong, `--verify` is where it gets fixed, and it is a small addition.

**A fifth sighting for T15's leaked discovery record, and the hypothesis held.**
T13.1 noticed that of two cleanly-closed instances only one leaked, and guessed
the difference was whether the instance had run agents. This session ran the
same shape and got the same split: the onboarding-only launch cleaned up, the
launch that spawned agents left both `inst_….json` and `….broker.sock` behind
with no process alive. Two for two is still not a cause, but it is now a
bisection with a direction.

### T13.1 — as built

**The gate check came back empty for once, and the ticket was still mostly
wrong.** `seal|subgraph|verdict|precondition` across
`crates/warp_cli/src/local_control/` returns nothing, so unusually for this
board the answer to "is it already built" was no. What the scoping found
instead was one level up: **the fork had the plan substrate and nothing for the
guard to guard.**

Tusk's design note refuses to ship this in exactly that state — *"With no
persistent DAG, every function in §4 would operate on an empty or one-node graph
— the guard would guard nothing. Per the honest-knob rule, we record the design
and stop rather than ship inert machinery."* Its §6 precondition is a persistent
plan with `depends_on` edges, and this fork has had one since T7.1. But its
§7 trigger is subtler than the precondition, and reading `graph.rs` is what
surfaced it: `run` seeds every node `Pending` (`graph.rs:413`), prints, and
exits. **Nothing is written down, so a second run re-runs the whole plan, so an
edit between runs invalidates nothing.** The seal in Tusk is load-bearing
because the run *continues* from it; here there was nothing to continue from.

So T13.1 is Tusk's §7 trigger, then Tusk's §2.1 guard:

| | what it is |
|---|---|
| **record** | `graph run` writes `plan.toml.run.json` — every settled node, plus a SHA-256 of the node *as it ran*. `--record` moves it, `--no-record` suppresses it. |
| **resume** | `--resume` seeds finished nodes from that record instead of spawning them. Failed and skipped nodes run again. |
| **guard** | `graph check` picks the record up and refuses a plan the record no longer fits. |

**The sealed subgraph collapses here, and saying why is the interesting part.**
Tusk has two node kinds — a *gate* clears while the work upstream of it sits in
any state — so its seal has to be the transitive upstream closure of every
cleared gate. This fork has one kind, and `ready` refuses to start a node until
every edge is `Done`, so **a finished node's ancestors are finished by
construction** and the closure is the set itself. `sealed()` is therefore a
filter, not a walk, and it says so in its own doc comment. The closure still
exists — it moved into `violations`, where the plan may have *grown* an ancestor
since the record was written, which is the one case the collapse does not cover.

**Two rules, and a third that turned out to be unnecessary.** Both are stated
relative to what a resume would reuse, which is what makes them checkable:

1. **edited** — a finished node's own definition changed, so the answer on file
   was produced by a different prompt, allowlist, name or set of edges;
2. **reached back** — a finished node now waits on something that never ran, so
   a resume would run the new node and then skip the one meant to consume it.

The third rule anyone would write — *a finished node was deleted* — is not
there, because deleting one rewrites the `needs` of everything downstream and
that is rule 1 on those nodes. A test pins the claim rather than the comment
asserting it.

**One thing the tests got wrong before the code did.** The first expectations
had a node inserted upstream producing both an `edited` and a `reached back`
entry for the same node. That is accurate and it is one edit counted twice: an
un-run node among a node's *own* `needs` can only have arrived by an edit, so a
direct reach-back always implies a fingerprint change. Suppressed, with the
reasoning inline — the reach-backs worth printing are the ones on nodes nobody
touched, which is exactly how the guard earns its keep.

**The advice the guard is really giving:** *edit the failure, not the evidence.*
A failed or skipped node is not sealed, is yours to rewrite freely, and is the
whole reason you came back to the plan. That is a test name, not a slogan.

**Verified by running, 2026-08-27.** Release build, WSLg, scratch
`XDG_CONFIG_HOME`/`XDG_STATE_HOME`/`XDG_RUNTIME_DIR`, `WARP_FORK_LOCAL_AGENT=1`,
a two-node plan against a real `claude`.

| | result |
|---|---|
| `graph check` with no record | unchanged output, exit 0 — the pre-T13.1 behaviour is intact |
| `--against` a file that is not there | refused; a missing *sibling* is not an error |
| a record claiming to be something else, and a version 99 record | both refused by name |
| run 1, `--timeout 1` | `hello` failed, `after` skipped; record written holding both |
| `check` against that record | `0 sealed` — a failure is not evidence |
| run 2, `--resume` | nothing to reuse, both nodes ran, both `done` |
| run 3, `--resume` | `hello: reused`, `after: reused`, **5 ms, zero new conversations** |
| edit `hello`'s prompt, `check` | refused, naming `after` as having been handed its answer |
| the same edit, `run --resume` | refused before contacting Warp at all |
| the same edit, `run` with no `--resume` | **ran, ungated** — a run that reuses nothing can invalidate nothing |
| `--no-record` | the earlier record's fingerprint untouched |
| insert `lint` in front of both | `hello` edited, **`after` reached back through `hello` to `lint`** |
| append `report` after both, `--resume` | two reused, one spawned, 4 s |

**A gap the first run found, which no unit test would have.** `agent.spawn`
needs an existing conversation to parent to, so `graph run` against a pane whose
agent has never been prompted fails *every node* with *"the targeted pane has no
agent conversation to parent a child to"*. The plan is fine; the pane is not.
`--parent` or one `agent.prompt` first. Not new in T13.1 — T7.1 has always
worked this way — but it is the first thing a person hits and it was written
down nowhere.

**Named unverified inputs.** The guard has never been run against a plan large
enough for the *nearest-ancestor* reporting to matter — the live plan was three
nodes, and the chain case is held by a unit test only. And nothing here has been
run on Windows; the record is written with `std::fs::write` and a path built by
appending to an `OsString`, so a UNC or extension-less plan path is read, not
run.

**And a fourth sighting for T15's open item, with one new detail.** After a clean
`warpctrl window close` and no process alive, `inst_….json` and `….broker.sock`
were still in the scratch `XDG_RUNTIME_DIR`. New: **two instances were closed in
this session and only one leaked** — the first, which never got past onboarding,
cleaned up; the second, which ran agents, did not. That is a difference to bisect
against, not a cause; nothing in T13.1 touches discovery.

### T12.3 — as built

**Half the ticket was already done, and the other half turned out to be two
things the ticket did not mention.** *"`control.pair`'s QR encodes the page URL
rather than a bare token"* landed in T12.1, because a QR pointing at a `POST`-only
route was not a client. What was left was the manifest and the icon — and
scoping that surfaced two problems that make the difference between an icon that
works and an icon that is a decoration.

**1. The wide listener bound port 0, so a home-screen icon died on every
restart.** An installed app is a saved URL. The wide listener took an ephemeral
port for the same reason the loopback one does — *"the address is the part a
person chose, and the port is this instance's to pick"* — which is right until
the thing being saved is the address. `WARP_FORK_CONTROL_BIND` now takes an
optional port: `192.168.1.5:41234`, `[fd00::1]:41234`.

**This reverses a case `a_bind_wider_than_loopback_has_to_be_named_exactly`
pinned as refused.** `192.168.1.5:8080` sat in that test's ambiguous list, which
was correct while the only ambiguous thing was an address. It is not a widening:
a wildcard address is refused because it is *unanswerable* — nothing can say
which networks it covers, so the `Host` check has nothing to check against — and
a port is one number typed on purpose, compared like any other part of the
authority. Verified live: `0.0.0.0:8080` is still refused, logs
*"must name one address, not a wildcard"*, opens no wide listener and leaves
nothing on 8080.

**One case no parser can catch, asserted rather than papered over.**
`fd00::1:8080` without brackets is a *valid IPv6 address* — `fd00:0:0:0:0:0:1:8080`
— so a person who meant "fd00::1 port 8080" has typed something else that is
real. This was found by writing it into the refusal list and watching the test
fail. It still fails closed: the machine does not hold that address, the bind
fails, and loopback keeps serving. Brackets are the disambiguation and that is
what they are for.

**2. `sessionStorage` and "installable" are incompatible by definition.** T12.1
chose `sessionStorage` for the device token — per tab, so a bearer for a control
plane never touches the disk of a phone that may not be only yours — and said it
should change only for a *measured* reason rather than a guessed one. This is
that reason, and it is structural rather than a matter of taste: **a home-screen
launch is a new browsing context every cold start**, so `sessionStorage` is empty
by definition and an installed app would demand a fresh QR scan on every single
launch.

Now `localStorage`, bounded by the twelve hours the server already gives the
token, cleared on a 401, and endable from the device with `unpair` in the header
— which had to come with it, because a token that outlives the tab needs a way to
be ended other than waiting. Measured with a disk-backed store, which is the only
way to simulate a cold launch without a phone: **launch 2, with no code in the
URL and a fresh process, came up `live` and paired.**

**A third thing `unpair` needed, found by running it.** The five-second pollers
outlive an unpair, so without a guard the page replaced a working view with a red
"not paired" error every five seconds — which reads as a fault rather than as
what was just asked for. `refreshApprovals` and `refreshState` now return early
when there is no device. Checked by snapshotting six seconds after the tap.

**What installability actually buys, stated honestly because the ceiling is set
by the transport.** A service worker requires a secure context and `http://` at a
LAN address is not one — so no service worker, no install prompt, no WebAPK, no
offline. **iOS Safari's *Add to Home Screen* is the one path to a standalone
launch here**: it is a manual user action needing neither HTTPS nor a service
worker, and it takes its icon from `apple-touch-icon`, which does not render SVG.
That is why the icon is a PNG and not the smaller, diffable thing it would
otherwise be. Android Chrome gets a shortcut in browser UI.

**And pairing still does not survive a restart, which is fine.** Codes and device
tokens are in memory and die with the process, so after a Warp restart the icon
opens a page saying *"run `warpctrl pair show` … then scan the QR"*. The fixed
port is what makes that possible at all: an app that tells you what to do beats a
URL that refuses to connect.

**Verified by running, 2026-08-27.** Release build, WSLg, scratch XDG,
`WARP_FORK_CONTROL_BIND=172.22.45.116:41234`.

| | result |
|---|---|
| wide listener | bound `172.22.45.116:41234` — the port it was told |
| after `window close` and relaunch | wide still `:41234`; loopback moved `43173` → `46503` |
| the saved icon URL across that restart | `GET /` → `200` |
| `warpctrl pair show` | `http://172.22.45.116:41234/#<code>`, both times |
| `GET /manifest.webmanifest` | `200 application/manifest+json`, `start_url /`, `display standalone`, icon `/icon.png` `512x512` |
| `GET /icon.png` | `200 image/png`, decodes as 512×512 RGBA |
| CSP on every route | now also `img-src 'self'; manifest-src 'self'` — no external host under any directive |
| launch 1 (QR scanned) | `live`, paired, token on disk |
| launch 2 (**no code in URL**, new process) | `live`, paired — the case `sessionStorage` could not serve |
| tap `unpair` | token gone from disk, pairing screen shown, still clean six seconds later |
| `WARP_FORK_CONTROL_BIND=0.0.0.0:8080` | refused, logged, no wide listener, nothing on 8080 |

**Named unverified input, and T12.3 makes it the largest it has been.** Every
claim above about *what a browser does* is read, not run: there is still no
browser on this machine, so `console.js` executed under `node` with a DOM shim.
Specifically unverified — **that iOS Safari installs this manifest and shows this
icon**, that a standalone launch looks right, that the secure-context rule bites
exactly as described on the phone in question, and everything T12.2 already
listed about thumb reach and the arming window. The code does not depend on the
secure-context reasoning being right: the manifest is correct either way, and if
a service worker ever becomes possible the manifest is what would be waiting for
it. **One session with a phone closes T12.1, T12.2 and T12.3's open items at
once**, and it is now the highest-value hour available on this phase.

### T12.2 — as built

**The gate check found the ticket already written, for the third task running.**
T12.2's requirement was *"the page must learn that from the server rather than
assuming it — a button that 403s is worse than a button that is absent."*
`PairedDeviceResult.actions` already exists, and T11.4's doc comment on it reads:
*"Given so a client can present a truthful capability list rather than
discovering the boundary one refusal at a time."* Same sentence, written eight
days earlier. **Nothing was added server-side** — `console.js` already persisted
the whole pairing response, so `device.actions` was in `sessionStorage` before
anything read it.

It also cannot go stale, which is worth writing down because it looks like it
could: `pairable_actions` consults an environment variable, a process cannot
change its own, and a restart drops the in-memory pairing map and forces a fresh
scan. So the list is fixed for an instance's life.

**`Yes` takes two taps, and that is a decision rather than a default.** The first
tap arms the button, which says so, and disarms itself after four seconds. `No`
stays one tap. It is the same asymmetry T11.5 argued for the pairing allowlist —
saying no can only ever make less happen — applied one layer up, where the
failure mode is a pocket rather than an attacker. The cost is one extra tap on
the only action that can make something happen.

**Three bugs, all found by running it, and the third is the one worth reading.**

**1. "nothing is waiting on you" printed above a request that was.**
`renderApprovals` set its note only on the empty branch, so the message from the
last empty refresh survived into a render with one approval. The single sentence
this page must never get wrong, wrong.

**2. An answer's error was wiped by the refresh that followed it.** Answering
and listing shared one note element, and every answer ends in a refresh, so the
reason an answer was refused appeared for roughly one heartbeat. They now have
separate lines with separate lifetimes, because they fail differently and are
cleared by different things.

**3. Half the server's errors were unreadable, including the one that
matters.** With the first two fixed, a deliberately stale answer produced
`HTTP 400` and nothing else. `describeFailure` read `ErrorResponseEnvelope`,
which carries `error` at the top level — but a *typed action* that is accepted
and then fails answers with a `ResponseEnvelope`, which nests it under
`response`. Everything the console can say about an approval comes back in the
nested shape, so the client understood exactly the errors that did not matter
and swallowed exactly the ones that did. **The bug was inherited from T12.1 and
survived that task's verification**, because T12.1 never provoked a typed-action
failure — its only errors were auth and routing, which are the shape it did
understand.

The pattern across all three: each was visible only in a state the happy path
does not reach — a non-empty list after an empty one, an error after a success,
a refusal after a grant. `both_of_the_servers_error_shapes_are_read` and
`an_answer_carries_the_digest_of_what_was_shown` pin the two that fail silently.

**Verified by running, 2026-08-26.** Release build, WSLg, scratch XDG, wide bind,
a fake CLI agent that emits a real `permission_request` over OSC 777 and then
blocks on one raw byte of its PTY — so the byte it reads is the proof.

| | result |
|---|---|
| `WARP_FORK_REMOTE_APPROVE` unset — pairing advertises | `app.ping agent.list events.subscribe agent.approvals agent.deny` |
| …and the page draws | `No` only, plus the line naming the variable |
| tap `No` | buttons disable, agent reads **`1b`** (Escape), list returns to 0 |
| `WARP_FORK_REMOTE_APPROVE=1` — pairing advertises | the same five plus `agent.approve` |
| …and the page draws | `Yes` and `No` |
| **one** tap on `Yes` | button reads *"tap again to allow"*, **nothing sent**, agent still blocked |
| four seconds later | button disarmed itself back to `Yes` |
| two taps on `Yes` | agent reads **`0d`** (Return), list returns to 0 |
| answer a request that moved | `HTTP 400: nothing is waiting on pane …; ` `agent.approvals` ` reports the requests that exist right now` — on its own line, surviving the refresh |
| scratch state directory after all of it | no `Bearer`, no `device_token`, no `bearer_token` |
| shutdown | `warpctrl window close`, no surviving process |

**Named unverified input, unchanged from T12.1 and now larger.** There is still
no browser on this machine; `console.js` ran under `node` with a DOM shim, which
now also stands in for `addEventListener` and `disabled`. So the buttons were
*invoked*, not *tapped*: nothing here proves a 2.75rem touch target is reachable
with a thumb, that the armed state is legible at arm's length, or that a real
browser fires these handlers in this order. **The arming behaviour in particular
is a claim about human timing that no capture can check.** One session with a
phone would settle all of it, and it is the same open item T12.1 filed.

### T12.1 — as built

**The gate check came back "no", which is unusual for this board.** `Html(`,
`ServeDir`, `include_str!(*.html)` and `text/html` across `app/src`,
`crates/local_control` and `crates/http_server` return one hit, and it is a MIME
table in `artifact_download.rs`. Nothing in this fork or upstream's control plane
serves a page. `axum` was already a workspace dependency of `app`, so the route
cost nothing.

**And T11.4 had already written the ticket.** The doc comment on
`validate_endpoint_headers` said, in the commit that shipped the wide listener:
*"When a page does exist, the allowlist belongs in the same commit as the page,
with the exact origin it serves from."* So did `pair_url`'s, about the fragment
being a convention until a page enforced it. Both are discharged here. Finding
the ticket already written by the previous task is the cheapest form of the
gate check and it is worth doing deliberately.

**Two findings, both from running it.**

**1. The QR was never scannable.** `control.pair` built its URL from `PAIR_PATH`,
so a phone following it arrived at `POST /v1/pair` with a `GET` and got `405`.
Nobody had noticed because nobody had scanned one — every T11.4 verification
drove the routes with `curl`, which never follows the URL it was handed. Fixed
by pointing the QR at `CONSOLE_PATH`; the code still ends up POSTed to
`/v1/pair`, by the page, from the fragment. **This is the second time in two
tasks that "the backend works" and "a person can use it" came apart**, and both
times the gap was invisible to the tool used to verify.

**2. The first origin rule was `Origin ∈ expected_hosts`, and probing it live
showed why it should be `Origin == Host`.** With two listeners bound, the wide
one accepted `Origin: http://127.0.0.1:<loopback port>` — an address this server
had bound, so it passed the list. Nothing could exploit it: both origins are
ours, no `Access-Control-Allow-Origin` is ever sent so neither can read a reply,
and a JSON `POST` would need a preflight this server does not answer. But the
rule was then "an origin we serve" when the property wanted is "the origin that
served this page". Comparing to `Host` — already checked for membership one line
earlier — is stricter, shorter, and needs no list. The unit test carries the case
that found it.

**What the origin change actually is, stated so it is not later "fixed" into
something weaker or something wider.** No route sends any CORS response header.
A cross-origin page therefore cannot read a response no matter what this check
decides. The only thing that changed is that a **same-origin** request stopped
being collateral damage — browsers send `Origin` on same-origin `POST` too, so
before this commit the console's own `fetch` to `/v1/control` would have been
refused by the server that served it.

**Deliberately not built.** A feature gate. The page is a constant with no
secret, reachable only by whoever can already reach the listener, and it does
nothing at all without a credential. A `WARP_FORK_CONSOLE` variable would be
ceremony around a static string, and one more thing to be off when someone needs
it. What is disclosed by serving it is that this machine runs the fork — and a
port that answered `403` to everything disclosed that too.

**Verified by running, 2026-08-26.** Release build, WSLg, scratch
`XDG_CONFIG_HOME`, `WARP_FORK_CONTROL_BIND=172.22.45.116`,
`WARP_FORK_EVENT_LOG=/tmp/t121/events`.

| | result |
|---|---|
| `GET /` and `GET /console.js`, both listeners | `200`, correct content types, full policy on both |
| every security header | CSP, `X-Frame-Options: DENY`, `nosniff`, `no-referrer`, `no-store` |
| `warpctrl pair show` | URL is now `http://172.22.45.116:45319/#<code>` |
| unpaired boot | *"run `warpctrl pair show` … then scan the QR"* — and the wording was **wrong first**, saying `warpctrl control pair`, which is not a command |
| paired boot | code redeemed, device token stored, credentials minted for `agent.list` and `events.subscribe`, badge `live` |
| `/v1/state` | rendered `0` with the CLI-agent note |
| `/v1/events` | four live OSC 777 events rendered newest-first with timestamps |
| origin: none / same / other listener / `evil.example` / `https:` / `null` / other port / suffix | `401` (reached auth) / `401` / **`403`** / `403` / `403` / `403` / `403` / `403` |
| shutdown | `warpctrl window close`, no surviving process |

**How the script was run, and what that does not prove.** There is no browser on
this machine, so `console.js` was executed by `node` against the live server with
a ~90-line DOM shim (`/tmp/t121/harness.js`, not committed) providing the dozen
DOM calls the file actually makes. That exercises the real file — pairing,
credential minting, state rendering, SSE frame parsing, reconnect — against a
real Warp.

**It does not prove the escaping.** One event carried
`<img src=x onerror=alert(1)>` as its summary and the harness reported it
verbatim, which looks like a demonstration and is not one: the shim has no HTML
parser, so it could not have rendered that string any other way. The escaping
rests on `textContent` semantics, on `the_script_never_assigns_markup`, and on
`script-src 'self'` — not on that run. **Named as the unverified input: no phone
browser, and no browser at all, has loaded this page.** The cheapest check is to
open it on a phone once.

Also unproven for the same reason: that a `<meta viewport>` layout reads well on
a small screen, and that `sessionStorage` per-tab is tolerable rather than
annoying in daily use. Both want a person, not a capture.

**A method note, and it is about this session rather than about the code.** The
scratch-profile launch cost a restart because `HasCompletedOnboarding` was
written to `$XDG_CONFIG_HOME/warp-terminal/user_preferences.json`, which nothing
reads — the directory is `warp-oss`. The symptom was the documented one:
`has_workspace: false`, empty `pane list`, `tab.create requires a workspace`.

**The correction that nearly got written here was that T11.5 recorded the wrong
path. It did not.** T11.5's as-built names `$XDG_CONFIG_HOME/warp-oss/…`
correctly, and T11.1c's notes add the other half — the file is `{"prefs": {…}}`,
so a flat key is silently discarded. Both were right; the path used this session
came from memory instead of from the file. Writing that up as a doc bug would
have turned two true lines into false ones, which `CONSOLIDATION.md` §11 names
as worse than leaving an error alone, because the next reader has no reason to
doubt a fresh line. **Recorded as what it is: the recipe has now cost three
sessions a restart while being correctly written down each time**, which is an
argument for it living somewhere a cold start reads — it is now in `CLAUDE.md`.

### T11.1 — the gate check (2026-08-24)

Read, not run, except where noted. **Warp already has a structured, versioned
CLI-agent event protocol. What it does not have is anywhere to keep it.**

**There are two event worlds, and only one of them survives a restart.**

| | world 1 — Warp's own agent | world 2 — third-party CLI agents |
|---|---|---|
| type | `BlocklistAIHistoryEvent`, **26** variants | `CLIAgentEventType`, 10 variants |
| transport | in-process `ctx.emit` | **OSC 777** on the PTY, sentinel-gated (OSC 9 fallback for Codex) |
| wire type | — | `warp_core::cli_agent_protocol::CLIAgentNotification` (`Serialize + Deserialize`, `skip_serializing_none`) |
| versioning | Rust enum | `v` field + `VERSIONED_PARSERS` array; `current_protocol_version()` is derived from its length, and the PTY exports `WARP_CLI_AGENT_PROTOCOL_VERSION` so plugins negotiate |
| unknown values | `#[serde(other)]` on `Harness` | `CLIAgentEventType::Unknown(String)`, and an unsupported `v` is a `report_error!` rather than a panic |
| **persisted?** | **yes** — SQLite, via `AgentConversationData` (the blob T8.3 added `settled` to) | **no** — `CLIAgentSessionsModel` is three in-memory `HashMap`s |

World 2 is the one that matters for driving other people's agents, and it is
**ephemeral**: an event is parsed from the PTY, updates a `HashMap`, paints the
UI, and is gone. Nothing survives a restart, nothing can be replayed, and
nothing can be handed to a second client that connected later — which is exactly
what an SSE subscriber is.

**The taxonomy is better than the one `TR-EVENTS` proposed building**, and in
the place that matters most here: `PermissionRequest` and `PermissionReplied`
are first-class event types. The kode-rs incident this phase exists to prevent —
a swarm running without permissions and nothing surfacing it — is a *recorded
event* in this protocol. It just is not recorded anywhere durable.

**And the projection already has a reference implementation.**
`crates/warp_tui/src/cli_agent_osc_event_publisher.rs` maps world 1 →
world 2's wire format: Warp's own headless TUI publishes `BlocklistAIHistoryEvent`
as OSC 777 notifications, so a mapping from the rich internal enum onto the
serializable one is already written and shipping.

**So T11.1 is a projection, not a taxonomy.** The smallest thing that is still
the idea: subscribe to events that are already emitted and append them to a
per-run file, reusing `CLIAgentNotification` as the line format.

**Two gaps in the wire type for use as a log**, both additive and both
backward-compatible under `skip_serializing_none`:

* **no timestamp** — an append-only log whose entries cannot be placed in time is
  most of the way to useless.
* **no per-event or per-call id** — this is `TR-EVENTS-B` exactly. `session_id`
  exists; a stable id per tool call does not, so a `tool_complete` cannot be tied
  to the `permission_request` that preceded it.

**Also checked, and recorded so it is not re-checked:**

* **`crates/warp_web_event_bus` is not a candidate surface.** It is
  `#![cfg(target_family = "wasm")]`, five variants, and emits to a host
  JavaScript app whose type lives in `warpdotdev/warp-server`. The wasm build is
  a client of Warp's server, not an observer of a local session.
* **`WB-SLEEP` was wrongly recorded as absent in `CONSOLIDATION.md` §4.1.**
  `crates/prevent_sleep` exists with mac/windows/noop backends and **is** wired —
  but only in `crates/http_client`, guarding HTTP requests and streams. So the
  mechanism is present and the run-scoped use is not. The earlier "not found"
  was a malformed grep (`-i` glued to the pattern), which is the same class of
  error as §1.1: the command ran and reported honestly about the wrong question.

### T11.1 — as built

`app/src/event_log.rs`, ~200 lines, plus one call at the single choke point.
Off unless `WARP_FORK_EVENT_LOG` asks: `on` writes under `fork::state_dir()`,
anything else is taken as the directory, so a run can be pointed at a scratch
path. Policy in `fork::event_log_dir`, mechanism in the module, per the same
split as `WARP_FORK_FRAME_LOG`.

**The hook is `CLIAgentSessionsModel::update_from_event`, and it records
*before* the early return, not after.** That function drops any event arriving
for a terminal it has no session for. Logging after the drop would produce a
file containing only what succeeded, which cannot show the one that did not —
so the record carries `applied: false` instead and the event survives.

**One flat JSON object per line.** Not an envelope wrapping the event: every
reader of this file is a filter, and `jq 'select(.event=="permission_request")'`
should not have to know which fields live a level down. A test asserts no field
nests.

**Warp's clock, not the agent's.** The protocol carries no timestamp and the
agent's clock is not ours to trust; `ts` answers "when did Warp know". `seq` is
process-global rather than per-file, so ordering can be reconstructed *across*
concurrent sessions and a gap in one file is visibly an event that went to
another.

**Verified by running** (Linux/X11, 2026-08-24), by emitting the real OSC 777
form a plugin uses — `ESC]777;notify;warp://cli-agent;{json}BEL` — from a shell
inside a Warp pane, through Warp's own parser:

```
{"ts":"2026-08-25T02:24:10.367Z","seq":0,"v":1,"agent":"claude",
 "event":"permission_request","source":"rich_plugin","session_id":"probe-1",
 "cwd":"/tmp","tool_name":"Bash","tool_input_preview":"rm -rf /","applied":true}
```

Three properties were driven rather than asserted:

* **A plugin cannot choose where Warp writes.** `session_id` becomes a
  filename, and the agent supplies it. `"../../../../../../tmp/pwned"` produced
  `___________tmp_pwned.jsonl` *inside* the log directory; nothing appeared at
  `/tmp/pwned`. The unit test for this **failed first**: the sanitizer stopped
  traversal (no separator survives) but left `..` in the name, which the test
  called a traversal segment. The test's intent was right and the code was
  weaker than it, so the code changed.
* **An event from a newer plugin reads as itself.** `subagent_spawned`, a type
  this build has never heard of, is recorded under its own name rather than as
  "unknown", because `CLIAgentEventType::Unknown` round-trips its string.
* **`seq` shows the gap.** The three surviving lines are `seq` 0, 2, 3 — number
  1 is the traversal event, in the other file.

**Two bugs found on the way, both upstream, both fixed:**

* **`crates/graphql/build.rs` and `crates/warp_graphql_schema/build.rs` read a
  schema they never declared as an input.** Only `rerun-if-changed=build.rs`,
  so an upstream merge that changes the queries *and* `schema.graphql` together
  leaves a stale registration in `OUT_DIR`, and every query in the crate is
  validated against the old schema. It fails as "no field `inviteLink` on the
  GraphQL type `Team`", pointing at Rust that is correct, about a schema on disk
  that already has the field. Verified fixed by touching the schema and watching
  both crates rebuild, which before the fix they did not.
* Not a code bug but worth the same weight: **sharing `CARGO_TARGET_DIR`
  between two checkouts silently corrupts it.** T10.1's baseline was measured in
  a worktree pointed at the main target directory to save disk, and the artifacts
  left behind matched neither tree. It also retroactively weakens the
  verification that preceded it — a `cargo check --workspace --all-targets` that
  passes against a poisoned cache has proved nothing. Recorded in `CLAUDE.md`.

**Not done here, and deliberately:** world 1 (`BlocklistAIHistoryEvent`, Warp's
own agent) is not yet projected into the log. The mapping exists as a reference
in `crates/warp_tui/src/cli_agent_osc_event_publisher.rs`; wiring it is T11.1b.
Also still open from the gate check: **no per-call id**, so a `tool_complete`
cannot be tied to the `permission_request` before it (`TR-EVENTS-B`). That one
needs a protocol version bump, because the id has to come from the plugin.

**Filed as decides, not builds:** ACP as the adapter contract
(`CONSOLIDATION.md` §4.1 — the fork as client, kode-rs as agent), and `WB-SLEEP`
(inhibit system sleep for the duration of a run), the one `WB-` ticket this
substrate does not already answer.

### T11.1b — as built

`app/src/event_log/warp_agent.rs`, plus one call in `BlocklistAIController::new`
and two small accessors on `BlocklistAIActionModel`. `event_log.rs` became
`event_log/mod.rs`; the record shape gained a caller-supplied `Entry` so both
worlds go through one writer and produce one vocabulary.

**Two fields changed meaning, and both are deliberate.**

* **`v` is now optional, and absent means "did not come off a wire".** World 1
  has no protocol version; stamping `1` on it would claim a compatibility
  guarantee that does not exist. Verified in the live log: the world-2 line
  carries `"v":1` and the world-1 lines carry no `v` at all.
* **`call_id` is new, and it is half of `TR-EVENTS-B` delivered for free.**
  Warp's own agent has always had a stable per-action id (`AIAgentActionId`), so
  `permission_request` → `tool_start` → `tool_complete` join on it — and a
  `tool_start` sharing no id with any ask *is* an action that ran unasked. The
  hosted-agent half still needs the protocol bump, because there the id has to
  come from the plugin.

**`tool_start` is added to the vocabulary.** Not in the wire protocol, and
needed: without it an action that begins and never returns leaves no trace, and
"started and never finished" is the shape of the failure this phase is for.
`CLIAgentEventType::Unknown(String)` already round-trips names Warp does not
know, so the log was always a superset of the protocol.

**Where it departs from the TUI reference, and why.**
`cli_agent_osc_event_publisher.rs` maps the same two worlds already, so it is
the starting point — but it feeds a *notification* and this feeds a *log*, which
want opposite things. It reports only the selected conversation; this reports
every one, because orchestrated children are never selected and are the case the
fork cares most about. It emits `tool_complete` only for `AskUserQuestion`,
because the rest would be noise; noise is what a log is for.

#### Three things running it corrected

**1. A bug found by reading, before the run: `Changed` does not mean changed.**
`AIConversation::update_status_with_error` emits unconditionally, and
`update_conversation_in_progress_status` calls it as *every action starts*. The
first draft mapped `InProgress → InProgress` to `prompt_submit`, so a single busy
turn would have written a long run of lines claiming a person was typing
throughout a turn nobody was watching. Now guarded on `prev != new`, which also
kills a duplicate `stop` for one ending, and pinned by a test.

**2. An assumption in a comment that was simply false.** The draft excluded
`session_start` from the query lookup, with a confident comment that it "fires
before anything has been asked". Running it showed the first turn producing
`session_start` and `stop` and **no `prompt_submit` at all** — because
`AIConversation::new` starts at `InProgress`, so the first request's status write
is the no-op above. The fix was to stop asserting and measure: the query *is*
available at `session_start`. It now carries it, five seconds before the turn
ended rather than after:

```
{"ts":"…:36:14.864Z","seq":0,"agent":"warp","event":"session_start","source":"in_process",
 "session_id":"af35bf30-…","cwd":"/home/effatha","project":"effatha",
 "summary":"Reply with exactly the word: marker-one","applied":true}
{"ts":"…:36:19.853Z","seq":1,"agent":"warp","event":"stop", … same summary … }
```

So turn 1 is marked by `session_start` and every later turn by `prompt_submit`,
both carrying what was asked. A second turn in the same conversation was driven
to confirm the `prompt_submit` path independently.

**3. The claim that the two worlds meet on the primary path was wrong, twice
over.** A doc comment asserted that a local-agent turn's tool detail arrives as
world 2 "into the same file, under the same session". Neither half survives:

* Warp's action model sees nothing, because `translate.rs` turns a `tool_use`
  block into *text* rather than a `ToolCall` — deliberately, since a ToolCall is
  an instruction and Warp would run the command a second time.
* The plugin's OSC 777 does not arrive either. `local_agent` spawns `claude`
  with `Stdio::piped()` and reads its JSON directly, so there is no Warp PTY and
  nothing reaches the terminal parser world 2 hangs off.

**Hence T11.1c, which is a real gap and not a design.** On the fork's primary
agent path the log carries the turn frame and no tools. The stream that has them
is already being parsed — `ContentBlock::ToolUse` in `translate.rs` — but filing
those lines under the *run* needs Warp's `AIConversationId`, and `RequestParams`
carries only Claude's session token. That plumbing is the whole of T11.1c. It
was scoped rather than half-built.

#### Verified by running (Linux/X11)

Driven with `warpctrl agent prompt` against a scratch `XDG_CONFIG_HOME`, with
`WARP_FORK_LOCAL_AGENT=1` so the turn is answered by the `claude` CLI. Both
worlds landed in one directory, on one vocabulary, sharing one process-global
`seq`:

```
in_process   warp   session_start                  Reply with exactly the word: marker-one
in_process   warp   stop                           Reply with exactly the word: marker-one
in_process   warp   prompt_submit                  Now reply with exactly: ping
in_process   warp   stop                           Now reply with exactly: ping
in_process   warp   stop_failure                   (error_type: error)
rich_plugin  claude permission_request  Bash       rm -rf /
```

**The action half was not driven, and is held by unit tests instead.** It is
reached only by Warp's own server-backed agent, which this fork has no account
for — the first attempt errored with `missing authentication credentials`, which
is itself the honest demonstration. The decision table is split into a pure
`action_event` for exactly that reason: a test is the only thing that can hold
it. Said plainly rather than implied.

**Two notes for whoever runs this next.** A scratch `XDG_CONFIG_HOME` means
first-run onboarding, and the window sits on "Welcome to Warp" with
`has_workspace: false` until it is dismissed — `warpctrl` answers
`missing_target` and it looks like a control-plane fault. Seed
`HasCompletedOnboarding` = `"true"` in
`$XDG_CONFIG_HOME/warp-oss/user_preferences.json` instead. And on WSLg a
root-window screenshot is **black**: capture the window id from
`xwininfo -root -children` and use `import -window <id>`.

### T11.1c — as built

`app/src/event_log/local_agent.rs` plus tool-event accumulation in
`translate.rs`, and **one field of plumbing**: `RequestParams` now carries
`conversation_id`. Upstream had it in hand the whole time — `RequestParams::new`
is handed the entire `ConversationData` and keeps only what the *server* needs,
and the server does not need this one, because it knows a conversation by its
token. On the local path that token is Claude's session id, so filing tool
events under it would have put a turn's tools in a different file from its
frame. Three lines, and the reason T11.1b scoped this rather than half-building
it.

**A third `source`, not a third vocabulary.** `local_agent` sits next to
`in_process` and `rich_plugin`; `agent` is `claude` and `event` is `tool_start`
/ `tool_complete` exactly as everywhere else, so the queries written for the
other two work unchanged. `v` is absent, which now means what it always claimed
to mean: this did not cross the OSC 777 wire.

**The translator accumulates, the caller writes.** `translate.rs` opens with a
promise — "no process, no clock, no network" — and calling the log from it would
have cost that. So it pushes a `ToolEvent` per `tool_use` and per `tool_result`
and `run()`'s stream drains them. A `HashMap<call_id, name>` in the translator
lets a `tool_complete` say *what* finished; it is not decoration, because Claude
issues parallel tool calls and the starts interleave with the completions
(measured: `seq` 11, 12, 13, 14 on one turn — two starts, then two completions
in order). A single "last tool" slot would have mis-attributed every one of them.

**`input_preview` is untyped on purpose, and this is the interesting bug that
was avoided rather than fixed.** `input` is whatever a tool's schema declares,
so a typed `command: Option<String>` would fail to deserialize the moment an MCP
server declared `command` as an array — and because `on_line` drops any line it
cannot parse, that failure would have taken **the whole assistant message**, not
just the preview. The answer to the user would have vanished to log a field
nobody asked for. Pinned by
`an_input_of_an_unexpected_shape_costs_the_preview_and_not_the_message`.

Which two keys? `command` and `file_path`, matching `warp_agent`'s rule exactly:
the field is grepped for what *ran*, and widening it to every argument makes
that grep unreliable across sources as well as putting more of a tool's payload
— which is where its secrets are — on disk. `excerpt`, `project_name` and
`MAX_TEXT_LEN` moved up to `event_log/mod.rs` for the same reason: three
adapters truncating at three lengths would defeat the comparison the field
exists for.

#### `parent_call_id`, found by running it

Not planned. The first `Task` turn driven through the finished feature produced
this, which *looks* complete:

```
21 local_agent tool_start    Agent toolu_01K1rS…
22 local_agent tool_start    Read  toolu_01Nhnd…
23 local_agent tool_complete Read  toolu_01Nhnd…
24 local_agent tool_complete Agent toolu_01K1rS…
```

The nesting is there, and it is *entirely an inference from interleaving* — the
subagent's `Read` happens to fall between its parent's two lines. Claude's
stream carries `parent_tool_use_id` on the event and it was being thrown away.

Driving two subagents concurrently showed why the inference is not good enough:

```
1 tool_start    Agent toolu_013dvFLd  parent -
2 tool_start    Agent toolu_0152hYmB  parent -
3 tool_start    Read  toolu_015hja3e  parent toolu_013dvFLd  …/f1.txt
4 tool_complete Read  toolu_015hja3e  parent toolu_013dvFLd
5 tool_start    Read  toolu_011UBWMh  parent toolu_0152hYmB  …/f2.txt
6 tool_complete Read  toolu_011UBWMh  parent toolu_0152hYmB
7 tool_complete Agent toolu_0152hYmB  parent -
8 tool_complete Agent toolu_013dvFLd  parent -
```

Both parents are open across the whole middle, and they finish in the **opposite
order** they started. Position says nothing; the field says everything. Fan-out
is the case this fork most wants to watch, so the field is recorded rather than
inferred — and `local_agent` is the only one of the three sources that can, since
world 1 has no nesting and world 2's protocol has no such field.

**What it deliberately does not claim.** There is no `permission_request` on this
path. Claude in `--print` mode does not report one: a refused tool comes back as
an ordinary `tool_result` with `is_error`, indistinguishable on the wire from a
tool that ran and failed. Both are `tool_complete` with `error_type: "error"`,
which is what the stream said. Driven both ways — a successful `Read`, and a
`Read` of a file that does not exist.

Also measured, and worth writing down because it would otherwise be guessed
wrong: **`is_error` is absent on some successful results and `false` on others,
from the same CLI build.** `#[serde(default)]` there is required, not defensive.
Every fixture in `translate_tests.rs` is a captured line for this reason; the
`tool_use` block also carries a `caller` object that no remembered version of the
shape had.

**Verified by running** (Linux/X11, 2026-08-25/26), with `WARP_FORK_LOCAL_AGENT=1`
against a scratch `XDG_CONFIG_HOME` — and this time a scratch `XDG_RUNTIME_DIR`
too, which the T11.1b recipe omitted, so those test instances published into the
*shared* discovery registry. Four turns: one tool, one failing tool, three
parallel tools in a follow-up turn, and two concurrent subagents. Frame and
tools landed in one file under one conversation id, and the same lines arrived
over T11.2's SSE stream (`warpctrl events tail`) with no extra work, since the
fan-out is downstream of `record`.

Both new assertions were mutation-tested: replacing the `call_id` → name lookup
with `None` fails exactly `a_tool_call_and_its_result_are_recorded_as_a_matched_pair`,
and dropping the parent threading fails exactly
`a_subagents_tools_name_the_call_that_spawned_them`.

**Still open, and now the only gap on this path:** a subagent's *turn* has no
frame of its own — no `session_start`/`stop` per child — because Claude does not
emit one and Warp does not know a child exists. `parent_call_id` gives
containment, not lifetime.

### T11.5 — as built

Three catalog actions (111 → **114**), one new terminal-view seam, one env var,
and two things the ticket got wrong that only running found.

#### The gate check: the seam exists, and it is unreachable

`BlocklistAIActionModel` has exactly the API `GB-APPROVE` describes:
`execute_action(action_id, conversation_id, ctx)` accepts and
`cancel_action_with_id(…, ManuallyCancelled, ctx)` rejects, and
`crates/warp_tui/src/tui_permission_prompt.rs` already drives both from a
yes/no selector. `ConversationStatus::Blocked { blocked_action }` already
exists, `agent.list` already reports it, and T11.1b already projects
`ActionBlockedOnUserConfirmation` into the event log.

**And none of it can happen on this fork.** The agent panel is served by
`ai::local_agent`, whose own module docs say it: *"Claude runs its own tools.
Tool activity is reported to Warp as text, never as a `ToolCall` message — a
ToolCall is an instruction, and Warp's action model would execute a tool Claude
had already run."* No `ToolCall` → no queued action → no confirmation → the
whole path is dead code on the fork's primary agent. Upstream's other producer
of `ToolCall`s is the account-backed server this fork removes.

So the branch was **not** built. Wiring it would have produced the exact
artefact the maintainer lost a month to: a feature that exists, is tested, is
documented, and is never reached. This is recorded rather than silently skipped
because the next person will find `execute_action` and assume it was an
oversight.

#### The finding: `warpctrl` could not see the thing that blocks

A `claude` running in a Warp pane is not an `AIConversation`. It is tracked in
`CLIAgentSessionsModel` — a different map, keyed by terminal view, holding
`status`, `tool_name`, `tool_input_preview`, `summary`, `cwd`. Nothing in
`warpctrl` read it. Measured on a live instance with a blocked agent in a
visible pane:

```
$ warpctrl agent list       → "conversations": []
$ warpctrl agent approvals  → the blocked claude, with the command it wants to run
```

That is the T11 failure mode by construction — an agent blocked on a permission
nobody surfaced — and it survived T11.2 *and* T11.4 because both of them
answered `agent.list`. The live event stream did carry the
`permission_request`, so a phone already watching saw it arrive; a phone that
connected afterwards primed itself from `/v1/state` and saw nothing waiting.

#### The second finding: approval is a keystroke

There is no way to tell a CLI agent "approved". It drew a prompt on its own
terminal and reads its own stdin. So the write half is
`TerminalView::press_key_for_local_control`, writing `\r` for approve and
`\x1b` for deny through `write_user_bytes_to_pty` — chosen over `write_to_pty`
for the check it carries: a block under Warp's own agent control returns
`false`, which is reported as a failure instead of a success nobody can verify.
The result reports `"keystroke": "enter"` / `"escape"` and never claims the
agent acted; confirming that is a second `agent.approvals`.

Consequences, both of which are the shape of the feature rather than caveats:

- **`agent.approve` is refused for agents whose prompt was not watched.** Return
  takes the *highlighted* option, which is a fact about someone else's TUI.
  `ALLOW_VERIFIED_AGENTS` is one entry and the refusal names the agent.
  `agent.deny` is allowed for all of them: Escape's worst case is that nothing
  happens, and the caller can see that nothing happened.
- **Two actions, not one with a `decision` field.** A paired device is granted a
  *list of actions*, so a field would have put yes and no behind one grant. The
  split is what makes the pairing line expressible at all.

#### The bug the digest did not catch until it was run

Every approval carries a SHA-256 over what was displayed, length-prefixed
per field, and both answering actions require it back — so an answer that
arrives after the agent moved on is refused rather than misapplied.

The first live run found the digest hashing a field that goes stale underneath
it. `question_asked` sets `Blocked` **without** calling
`clear_permission_scoped_state`, which only runs on `tool_complete`,
`permission_replied`, `prompt_submit` and `stop`. Observed:

```
after permission_request  → permission | Wants to run Bash: rm -rf build/ | rm -rf build/ | 97b73f56…
after question_asked      → permission | Wants to run Bash: rm -rf build/ | rm -rf build/ | 97b73f56…
```

The agent was asking "which database should I use?" and a remote yes taken from
the first screen would have been **accepted**, because the digest had not moved.
Fixed by reading the summary from `Blocked { message }` — set by whatever caused
the *current* wait — and reporting the retained tool fields only when they agree
with it. After:

```
after question_asked      → question | Which database should I use? | (no tool) | b1d4b8d3…
```

Pinned by `a_question_after_a_permission_does_not_inherit_the_command`, and its
mirror `a_permission_with_no_summary_still_reports_its_command`, because the
check compares `Option`s rather than testing for presence: a permission request
may genuinely carry no summary, and `None == None` has to stay a match.

#### The pairing argument T11.4 asked for, and it came out asymmetric

T11.4's as-built required any widening of `PAIRABLE_ACTIONS` to arrive with an
argument. The write half was expected to be one decision; it is two.

| | pairable | argument |
|---|---|---|
| `agent.approvals` | yes | a read reporting strictly less than `events.subscribe` already streams live — withholding the snapshot while granting the stream only punishes a client that connected late |
| `agent.deny` | yes | monotone: Escape on an agent already waiting, so the most a stolen device token achieves is that something proposed does not happen |
| `agent.approve` | `WARP_FORK_REMOTE_APPROVE` only | a yes to whatever the agent thought of, which through a permission prompt is arbitrary code. The digest binds it to the request it was shown; it does not make that request safe |

The honest position on the third is that it **cannot be made safe by
mechanism — only chosen**, so it is chosen per machine and defaults to no. Its
parser is deliberately the opposite shape to `control_bind_from`: there an
unparseable value must be refused loudly because it would otherwise silently
mean something, here anything that is not an affirmative word is simply not
consent, and the safe side and the default side coincide.

#### `GB-GRANTS`: not built, and the argument against it

"Remembered grants" means Warp pressing Return on the user's behalf when a
matching request arrives. Four reasons that is the wrong thing here, three of
them checkable without running anything:

1. **The mechanism cannot be made to key on the request.**
   `tool_input_preview` takes `command` **or** `file_path` out of the tool
   input and drops everything else (`event/v1.rs`). For `Bash` that is the whole
   command; for `Write` it is the path and *not the contents*. A grant keyed on
   "the same request" would re-approve writing entirely different bytes to the
   same file. Sound for one tool and unsound for the rest is a footgun with a
   reassuring name.
2. **It would race the repaint.** A person pressing Return has seen the prompt.
   An auto-press fires on the OSC notification, and nothing tells Warp whether
   the agent's TUI has drawn its selector yet. A stray `\r` into a prompt that
   is not there goes into the agent's *message* input.
3. **It rebuilds the failure this phase exists to detect.** The stated burn was
   a swarm of agents running without permissions and nothing surfacing it. An
   auto-approver is that by construction; logging every auto-grant is the
   mitigation, and the *value* of a grant is that you stop reading the log.
4. **The gate rule points the other way.** Claude Code already has
   `--permission-mode`, `permissions.allow` rules in its settings, and a "don't
   ask again" option in the prompt itself — all keyed on the real tool input and
   applied by the process that knows what it is about to do. A coarser
   re-implementation one layer up is strictly worse than the thing that exists.

The tempting middle path — send `2` for the prompt's own "yes, and don't ask
again" — fails the same test that refuses `approve` for Gemini: option 2 is
"don't ask again" for `Bash`, "allow all edits this session" for `Write`, and on
a two-option prompt it is *No*. Pressing a digit whose meaning varies by tool is
precisely what this task declined to do everywhere else.

**If a future task wants grants, the place to put them is Claude's own settings,
not Warp's memory** — and the useful `warpctrl` verb would be one that *shows*
what the agent has already been granted, not one that grants.

#### Verified by running it

Linux/X11, scratch `XDG_CONFIG_HOME`/`XDG_STATE_HOME`/`XDG_RUNTIME_DIR`/
`WARP_LOCAL_CONTROL_DISCOVERY_DIR`. The agent was simulated by a script emitting
the same OSC 777 `permission_request` a real plugin does, and reading one raw
byte back — chosen over a real `claude` deliberately: it records the exact byte
that reached the PTY, and it does not touch the user's `~/.claude`.

| | |
|---|---|
| a blocked CLI agent, reported with its command, cwd and session id | yes; `agent list` reported `[]` for the same instance |
| two agents in two panes, attributed and ordered stably | yes — `claude` and `gemini` side by side |
| stale digest | refused, naming the fix |
| unknown pane / nothing waiting | refused, naming `agent.approvals` |
| `deny` | pane read byte `1b` |
| `approve` | pane read byte `0d` |
| `approve` on an unwatched agent (`gemini`) | refused by name; `deny` still worked (`1b`) |
| `question_asked` after `permission_request` | tool fields dropped, digest moved (the bug above) |
| pairing offer, switch off | `app.ping, agent.list, events.subscribe, agent.approvals, agent.deny` |
| paired credential for `agent.approve`, switch off | refused, naming `WARP_FORK_REMOTE_APPROVE` |
| paired credential for `agent.prompt` / `input.submit` | refused |
| `agent.approvals` then `agent.deny` over `172.22.45.116` as a paired device | worked; pane read `1b` |
| `agent.approve` over the LAN with `WARP_FORK_REMOTE_APPROVE=1` | worked; pane read `0d` |
| device token anywhere under the scratch state dir, `warp.sqlite` included | 0 files |
| the only new log line | `local-control wide listener started at 172.22.45.116:42917` — an address, no secret |

**Inputs not verified, named rather than glossed.** The permission prompt was
*synthesised*, not produced by a real `claude`, so what is proven is that Warp's
session model, the approvals surface, the digest and the PTY write all behave —
not that a real Claude Code prompt accepts `\r` as yes. `ALLOW_VERIFIED_AGENTS`
therefore rests on Claude Code's documented prompt (option 1, *Yes*, highlighted;
Escape labelled on the reject option), not on this fork having watched one.
**That is the claim to re-check first**, and the cheapest way is to answer one
real prompt with `warpctrl agent approve` and see whether the tool runs.
`172.22.45.116` is also WSL2's NAT address, not a physical LAN, and the client
was `curl` on the same host.

**Unrelated observation, not bisected:** across three clean `warpctrl window
close` shutdowns the discovery record and broker socket were left behind in the
scratch directory, though no process survived. `CLAUDE.md` says ordinary
shutdown cleans both. Nothing in T11.5 touches discovery, so this is either
pre-existing or environmental; recorded here rather than acted on.

### T11.4 — as built

A second listener, three secrets with different lifetimes, one new catalog
action (110 → **111**), and an allowlist that is the actual security boundary.

**The gate check paid off twice, in opposite directions.** `qrcode` is already an
`app` dependency and `drive::sharing::qr_code` already encodes an arbitrary URL
to a matrix *and* a PNG — built for Warp Drive share links, generic, tested. The
QR half of this ticket needed one word (`mod` → `pub(crate) mod`) and a
half-block renderer. But the *bind* half had no gate at all: `[127, 0, 0, 1], 0`
is a literal in `LocalControlServer::start`, ungated by anything. Worth recording
that the rule has a failure mode — "look for the gate first" found a whole
subsystem in one half of the task and nothing in the other, and the second half
is where the design work was.

**The ticket's four must-haves, and the one it did not list.**

| asked for | shipped |
|---|---|
| fail closed on an ambiguous config | `WARP_FORK_CONTROL_BIND` takes one literal IP; a hostname, a wildcard, or a typo leaves the wide listener shut |
| refuse a wide bind without a strong token | the pairing state only exists when a wide listener does, and a device gets nothing without spending a 32-byte `OsRng` code |
| never log the token | the only `log::` line names the address; the code reaches a person through `warpctrl pair show` and the QR |
| CORS allowlist, never `*` | **deliberately not done** — see below |
| — | **a paired device may reach three actions**, and this is the one that mattered |

**The must-have that was missing is the one the catalog forced.** The four listed
above are all about *reaching* the server. None of them constrains what a client
does once it is in, and `ActionKind` has 111 entries including `input.insert`,
`input.submit`, `agent.prompt`, `agent.spawn`, `slash.run` and
`remote.wsl.connect`. `input.insert` followed by `input.submit` is typing a
command into a terminal and pressing return. A pairing path that could mint a
credential for any implemented action would therefore have satisfied every stated
requirement and still been remote code execution reachable by photographing a
screen — which is *precisely* the vibe-kanban failure the ticket names, arrived at
through the front door instead of the back. So `PAIRABLE_ACTIONS` is
`app.ping`, `agent.list`, `events.subscribe`, checked before `issue_credential`
is even called, and stated as an allowlist because a denylist is a promise to
remember every future catalog entry.

**Refusing the CORS allowlist, on purpose.** The requirement assumes a server
that answers browsers and must choose which. This one answers none: any request
carrying `Origin` is refused outright, which *is* the empty allowlist. Adding an
allowlist now would be a widening with nothing to widen to, since there is no
page — the pairing client is a fetch from something holding a device token, not
a document with an origin. Recorded here and in the renamed function's doc
comment so it is not later "fixed" into the weaker thing.

**Two listeners, not one moved one, and the discovery record is why.**
`InstanceRecord::validate_local_control_authority` requires `endpoint.host ==
"127.0.0.1"` and every client calls it. Moving the listener to a LAN address
would have made the instance invisible to `warpctrl` *on the machine running
it* — including `warpctrl window close`. Keeping loopback also means the wide
address is never written to the filesystem at all, so the check that stops a
record from redirecting a client somewhere else keeps its full strength rather
than being relaxed to accommodate this feature.

**`0.0.0.0` is refused, and the reason is narrower than "wildcards are
dangerous".** A wildcard is *unanswerable*: nothing can say which networks it
joined, and the server cannot name a `Host` for clients to present, so the
header check degrades exactly when it starts mattering. `expected_host: String`
became `expected_hosts: Arc<Vec<String>>` — still exact string membership over a
two-entry, server-chosen list. The obvious way to make two listeners work would
have been to compare ports and ignore the address, and
`the_host_check_accepts_the_addresses_bound_and_no_others` refuses that
explicitly.

**A refusal leaves loopback serving, which is the fail-closed reading and not a
softening of it.** The dangerous thing is the wide listener, so the closed state
is "do not open it". Refusing to start the server outright would take out
`warpctrl window close` — and this fork has already been bitten by exactly that
shape, when a `WARP_FORK_POLICY=0` instance published no discovery record and
held a window and a port that nothing could authenticate to. A mistyped
environment variable must not be able to produce an instance nothing can stop.

**Three secrets rather than one, because of where the QR ends up.** A single
long-lived bearer would have to be *in* the QR, therefore also in the scrollback,
screenshot or photograph the QR appeared in, and stay valid. Split: a pairing
code (2 minutes, single-use, the only thing displayed), a device token (12 hours,
returned once over the connection that spent the code), and ordinary 5-minute
action-scoped credentials minted through the *same* `issue_credential` a local
client uses. `warpctrl pair show` is the one command in the CLI that prints a
secret, and that is now a stated property rather than an accident.

**A bug found while writing the renderer, not by a test.** `QrMatrix::is_dark`
indexes `y * width + x` into a flat `Vec`, so an `x` past the right edge does not
read out of bounds and return `false` — it **wraps into the next row**. The quiet
zone, drawn by asking for coordinates outside the matrix, would have been a strip
of the following row's modules printed where the margin belongs, and no scanner
would have read it. Caught by reading `is_dark` before trusting it;
`the_quiet_zone_is_actually_quiet` is the assertion that now holds it, and it
checks the *right* margin specifically because that is the one that wrapped.

**Light modules are painted, not skipped.** A QR needs its light modules light,
and a terminal background is usually dark — "leave it blank" would have produced
a code that looks fine in a diff and does not scan at all.

**Verified by running** (Linux/X11, 2026-08-26), against a scratch
`XDG_CONFIG_HOME`, `XDG_STATE_HOME`, `XDG_RUNTIME_DIR` *and*
`WARP_LOCAL_CONTROL_DISCOVERY_DIR` — the user's own registry was never written
to, and `ls` confirmed it afterwards.

Three listeners, which is the shape the design predicted:

```
172.22.45.116:34983   the wide listener
127.0.0.1:41513       warpctrl's loopback listener
127.0.0.1:9282        upstream's http_server — unrelated, and still unauthenticated
```

and the discovery record on disk said `{"host": "127.0.0.1", "port": 41513}`.
**The wide address is not in it**, which is what leaves
`validate_local_control_authority` at full strength.

| checked | result |
|---|---|
| redeem the code | device token, 12h, actions `app.ping, agent.list, events.subscribe` |
| redeem the same code twice | `unauthorized_local_client: pairing code is not valid` |
| device token → `agent.list` | credential issued |
| device token → `input.submit` | `insufficient_permissions`, and the message names what *is* allowed |
| `GET /v1/state` over the LAN address | the snapshot |
| `GET /v1/events` over the LAN address | `200`, `content-type: text/event-stream`, `: keepalive` frames |
| an `events.subscribe` credential on `/v1/state` | refused — T11.2's scope split still holds on this path |
| forged `Host: evil.example:34983` | refused |
| `Origin: https://evil.example` | refused |
| `warpctrl agent list` on loopback | unchanged |

**No secret reached any log.** All three live secrets — pairing code, device
token, credential — were grepped for across every file under the scratch state
directory, including `warp-oss.log` and `warp.sqlite`: **zero hits each**. The
one line the wide bind writes is
`[INFO] local-control wide listener started at 172.22.45.116:34983`.

**Both fail-closed cases run, and the important half is what still worked.**

| | listeners bound | log | `warpctrl` |
|---|---|---|---|
| `WARP_FORK_CONTROL_BIND=0.0.0.0` | loopback only | `WARN … must name one address, not a wildcard` | `app ping` **and `window close`** both fine |
| `WARP_FORK_CONTROL_BIND=my-laptop.local` | loopback only | `WARN … must be a literal IP address` | same |

and `warpctrl pair show` answered
`local_control_disabled: this instance has no wide listener … set
WARP_FORK_CONTROL_BIND to the address to listen on and restart` — the error is
the feature's discovery path, so it names the variable rather than just refusing.

**One input not verified, and it is named rather than glossed.** `172.22.45.116`
is WSL2's NAT address on `eth0`, not an address on a physical LAN, and the client
was `curl` on the same host rather than a phone. What that exercises is every
line of the code — a non-loopback bind, the `Host` set, the pairing exchange, the
allowlist — and what it does not exercise is a packet that actually crossed a
network, or a real QR scanned by a real camera. The QR *encoding* is
`drive::sharing::qr_code`, already shipped and used by Warp Drive, so the
untested part is the terminal rendering of it, which was eyeballed and is pinned
by three tests but has not been photographed.

**Not done, and deliberately.** No web page, so nothing consumes the pairing URL
yet except by hand; the fragment convention is written down for when one exists.
No revocation beyond expiry — `warpctrl pair` has `show` and nothing else, so a
lost phone is a 12-hour window, not something you can cut short. Both belong with
the client that would use them.

### T11.3 — as built

`AuthToken` now hand-writes `PartialEq` over `subtle::ConstantTimeEq` instead of
deriving it. `subtle` was already in the lockfile transitively, so this adds an
edge rather than a build. The change is four lines; **the finding is the
entry**.

**What the ticket got wrong, found by grepping for the callers instead of
trusting the name.** T11.3 said the derived `PartialEq` made
`verify_authorization_header` a timing oracle on the request path. That function
is not on the request path — it has **no production callers**, only tests.
Control requests authenticate in `lookup_credential`, which is a `HashMap` lookup
keyed by the secret, and that is not a prefix oracle: SipHash is seeded per
process by `RandomState`, so a near-miss hashes somewhere unrelated. The premise
that made this a *prerequisite* for T11.4 does not hold, and the ticket has been
corrected in place.

**Why it still shipped.** `verify_authorization_header` is public API of
`local_control`, it is named exactly what an auth check is named, and the next
two tasks are a new read route and a bind wider than loopback — the moment
someone reaches for the function that looks like the answer. Cheap now,
load-bearing later.

**A second overclaim, caught while writing the doc comment.** The first draft
said the derived comparison "returns at the first difference". It does not:
`String == String` lowers to `bcmp`, and a `bcmp` over 43 bytes may be a single
vectorised compare with no data-dependent timing at all. The real argument is
narrower — nothing *guarantees* that, it is a property of the libc and codegen
on the day, and the failure would be silent.

**What the test pins, and what it does not.**
`a_token_equals_itself_and_nothing_else` checks that the hand-written comparison
is still equality — first-byte, last-byte, prefix and suffix mismatches all
reject. It does **not** pin constant time, and does not claim to: that needs a
statistical timing measurement, which would be flaky in exactly the way that
gets a test deleted. The likelier regression is a hand-rolled comparison
quietly disagreeing with the derived one it replaced, and that is what is held.

**Not done:** `lookup_credential` itself. A constant-time lookup would mean
iterating every credential and `ct_eq`-ing each. It is not obviously wrong to
want that before T11.4, but the `HashMap` does not leak a prefix today, so it
was left alone rather than changed on speculation.

### T11.2 — as built

Two `GET` routes on the existing `warpctrl` server, one new catalog action, and
a `tokio::sync::broadcast` fan-out in `event_log`. Catalog goes 109 → **110**;
both count pins updated, and the first one caught the omission exactly as it is
supposed to.

| | |
|---|---|
| `GET /v1/state` | the snapshot. Body is verbatim `agent.list`, and it requires an `agent.list` credential, because it *is* `agent.list` — the route exists so a browser need not compose a request envelope, not to expose anything new. |
| `GET /v1/events` | the stream. One SSE `data:` frame per event log line. |
| `events.subscribe` | the new action. Its `POST` form answers *where* the stream is and when the credential dies. |
| `warpctrl events tail` | the reader. |

**The new action is not bureaucracy, and this is the one design argument worth
keeping.** It would have been cheaper to let an `agent.list` credential open the
stream. It would also have been wrong: `agent.list` returns titles and busy
flags, while the stream carries tool names, input previews and working
directories for *every* agent in the instance. Granting the second because
someone asked for the first is precisely the scope-conflation T11.4 exists to
avoid. Both directions are now refused, and that was checked rather than
assumed — see the runs below.

**`is_enabled()` stopped being a constant.** Before this it meant
"`WARP_FORK_EVENT_LOG` named a directory", fixed for the life of the process.
A subscriber is a consumer with no file behind it, so it now means "a file **or**
a live subscriber" and can flip either way at any moment — callers must not cache
it. `seq` moved out of `Sink` and became a process-global static for the same
reason: it is documented as process-global and should not restart or vanish
depending on whether a directory was named. Subscribing is therefore enough to
turn the log on; you do not also have to set the environment variable.

**The stream ends when the grant does.** A credential is good for five minutes,
and a connection authorized once at open would outlive its own authority — the
"localhost, therefore fine" reasoning this phase is trying not to ship. Expiry
is re-checked before every frame and on a 15-second tick, so an idle stream
closes on time rather than at the next event.

**No token ever leaves the process.** `events subscribe` deliberately does not
echo the bearer, which left the stream unreadable by anything — a gap the
design created and the first run exposed. `warpctrl events tail` is the answer:
it fetches the URL, opens the stream and prints lines, keeping the secret
internal. The alternative, a `--print-token` flag, puts a credential in a shell
history, a scrollback and a `ps` listing.

#### Found by running it: a Tokio runtime with no clock

The keepalive is a `tokio::time::interval`, and the local-control runtime is
built `.enable_io()` — **no `.enable_time()`**. That combination does not fail to
compile and does not fail to build the runtime. It panics on the first timer,
*inside the connection task*, where axum swallows it and the client sees a
dropped connection with no status:

```
error: transport_unavailable: failed to open the local-control event stream
details: error sending request for url (http://127.0.0.1:43477/v1/events)
```

Every check up to that point was green: `cargo check --workspace
--all-targets`, 272 `warp_cli` tests, four new handler tests, a clean release
build. The real message was only in Warp's own log — `A Tokio 1.x context was
found, but timers are disabled`. Fixed by enabling the time driver, and worth
recording because the *shape* of it recurs: a runtime feature flag is not a
compile-time contract, and a panic inside a spawned connection task reaches the
client as a transport error rather than as a panic.

Two smaller traps for whoever runs this next. `tokio`'s `sync` and `time`
features had to be added to `app/Cargo.toml`, and the code compiled without them
because another workspace crate enables them and cargo unifies features — so the
app crate's own manifest was wrong while everything built. And the scratch
`XDG_CONFIG_HOME` recipe in the T11.1b notes is subtly incomplete: the
preferences file is `{"prefs": {...}}`, so a flat
`{"HasCompletedOnboarding":"true"}` is silently discarded and the window sits on
onboarding with `has_workspace: false`. Patch the key *inside* `prefs`.

#### Verified by running (Linux/X11)

Live app, scratch config, `WARP_FORK_LOCAL_AGENT=1`. The stream received the
turn as it happened:

```
{"ts":"...T20:54:38.733Z","seq":0,"agent":"warp","event":"session_start","source":"in_process",...}
{"ts":"...T20:54:42.527Z","seq":1,"agent":"warp","event":"stop_failure","source":"in_process",...}
```

**Byte-identical to the file log**, which is the point of broadcasting the
rendered line rather than re-serializing: a subscriber re-broadcasts bytes it
never parsed and cannot drift from the on-disk format.

The authorization boundary, driven with `curl` against credentials obtained
from the broker socket the way any client obtains them:

| request | answer |
|---|---|
| `/v1/state` + `agent.list` credential | **200**, real conversation state |
| `/v1/state`, no credential | 401 `unauthorized_local_client` |
| `/v1/state` + `events.subscribe` credential | 403 `credential for events.subscribe cannot open agent.list` |
| `/v1/events` + `agent.list` credential | 403 `credential for agent.list cannot open events.subscribe` |
| `/v1/state` + forged token | 401 `local-control credential is invalid` |
| `/v1/state` + browser `Origin` | 403 `browser-origin ... not allowed` |

Expiry was waited out rather than reasoned about: a tail opened at `20:56:47`
was still live at `20:57:58` and had closed with `credential expired; re-run to
reconnect` by `21:03:14` — the five-minute grant plus at most one 15-second
tick, and the stream does not outlive its own authority.

**Not verified by running:** the `lagged` frame. It needs a subscriber that
stops reading while 256 events go past, which no natural run produces; it is
held by construction and by the bounded channel's own contract, and that is
stated rather than implied.

---

## Decisions on record

- **This fork is the product, and it stays a soft fork** (2026-08-24). Tusk
  retires as an app, `kode-rs` becomes one harness among several the fork drives,
  and the rename that would make this a *hard* fork is deferred — every
  `warp-oss`/`WARP_*`/`WarpOss`/`warpctrl` symbol renamed converts ground shared
  with upstream into permanent conflict. Merge cadence instead: T10.
  Reasoning and the corrected divergence measurement in `CONSOLIDATION.md` §1.

- **Observability precedes the remote surface** (2026-08-24, T11). Two
  independent reasons, and the second is the one that decided it: read-only-first
  because the value is in the read path and the risk is in the write path; and
  events-first because the failure that cost a month of kode-rs work was
  *silent*, and an event taxonomy is the detector for that class. A stream with
  nothing structured on it is a pipe with no protocol.

- **The web surface goes on `warpctrl`, never on 9282** (2026-08-24, T10.2).
  Upstream's `crates/http_server` answers unauthenticated and is ungated by fork
  policy. `warpctrl` is the server with `auth.rs`, the credential broker and the
  peer-UID check.

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
      LISTEN 127.0.0.1:9282    upstream's http_server
      LISTEN 127.0.0.1:33711   warpctrl

**Two loopback listeners. Zero outbound TCP. Zero UDP** — so not even a DNS
lookup: warp-oss never resolved a hostname, let alone contacted one.

*Both were labelled "local control" here until 2026-08-26, and only one is.*
9282 is upstream's `crates/http_server` (`PORT_BASE` 9277 + the Oss channel
offset), started ungated by fork policy and answering unauthenticated; the
ephemeral port is `warpctrl`, which binds 0 and publishes what it gets. The
egress finding is unaffected — the count and the direction are what it rests
on — but the attribution mattered enough to correct, because T12 puts a page
on one of these two and picking the wrong one would undo T10.2.

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
