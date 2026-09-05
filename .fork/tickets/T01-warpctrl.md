> Ticket T1, split out of `.fork/TASKS.md` on 2026-09-05. History; read the section you came for.

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
- [x] **T1.7** Document the verified command surface in `.fork/docs/manual.md` —
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
      misdiagnosis. See above and `.fork/docs/manual.md`.

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

