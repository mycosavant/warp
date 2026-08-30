# Working in this fork

Personal fork of `warpdotdev/warp`. **Thesis: no telemetry, no account
requirement, agents driven by the user's own Claude subscription, API keys and
local models.** Work happens on `dev`. Fork-specific docs live in `.fork/`.

**Root docs describe upstream.** `AGENTS.md` is upstream's and is still right
about architecture, feature flags, testing and exhaustive matching — read it for
those. Where it disagrees with this file, this file wins, and the one place it
actively misleads is called out under *Formatting* below.

---

## Method: run it

**Scope and verify by running the thing, not by reading it.** This is the rule
the project is built on, and it is stated here because it keeps paying:

- **T1.7** documented the control surface by executing all 88 actions one at a
  time. Three documented facts were wrong, including the count. Only running it
  would have found any of them.
- **T5.6** chased a turn that ended `Cancelled` with "nothing" in the log.
  Reading the `CancellationReason` enum produced a confident, wrong suspect. The
  log had the answer: 205 `SelectText` actions and a `CtrlC` — a person trying
  to copy. Three claims were retracted.
- **2026-08-21** produced two more in one session: reading the running process
  gave the wrong display backend (the launch recipe matters), and remembered
  guidance gave the wrong formatting rule (see below).

When something here has only been read, say so. `.fork/IDEAS.md` marks its
unverified claims at the top of the file; keep that habit.

**And a claim marked *measured* is measured as of its date, not measured now —
re-run it before building on it.** This is the subtler failure, because "measured
2026-08-27" reads like ground truth and is treated as exempt from the doubt
applied to everything else. Twice on 2026-08-29, hours apart: T14.9's *"`agent
read` shows nothing until the turn ends"* was true when written and had stopped
being true, and T14.7's *"the ACP path writes no tool events at all"* had been
fixed by T14.9 without this file being updated. Each was about to fund a night of
building something that already worked, and the second misled an advisor who was
reading carefully and had no way to know. The fork's own docs are the most
dangerous stale input available, precisely because they are the ones written to
be trusted. A measured claim you are about to spend a day on costs ten minutes to
re-run.

**…but running it does not save you if you guessed one of its inputs. Name the
inputs you did not verify.** Measured 2026-08-24: the fork's divergence from
upstream was published as 1168 files and 515 commits when it is 204 and 141 —
wrong by 5x, because the base handed to `git diff <base>...dev` was an upstream
commit `dev` already contained. The command ran perfectly and reported honestly;
the error was one step upstream of it, in a value assumed rather than computed.
`git diff A...B` is *supposed* to protect against a stale base, but when `A` is
an ancestor of `B` the merge-base is `A` itself and the three-dot form silently
degrades to a plain two-dot diff — no warning, no protection. Compute bases
(`$(git merge-base upstream/master dev)`); never paste them. Full account in
`.fork/CONSOLIDATION.md` §1.1.

**GUI gestures are runnable now, so "needs a person" needs an argument.**
`use_computer drag` (T9.1) performs press-move-release against one window and
photographs the frame *before* the release, which is the only moment a drop
preview or a drag ghost exists. It works on Windows too, without taking the
user's cursor, as long as you pass `--pid`/`--window-id` and the window is
foreground (T9.2). Recipe under "Driving a gesture" in `.fork/README.md`. It
found a real bug on each of its first two runs. What still needs a person is
anything about how something *feels* — latency, smoothness — because no
capture answers that.

**Launch Warp on Windows with `Start-Process … -NoNewWindow`, or it writes no
log at all.** `warp-oss.exe` is a console-subsystem binary; `Start-Process`
gives it its own console, so `stdout_is_a_tty` is true and `warp_logging`'s
`use_logfile` is false. What hides this is that a log still appears — the
crash-recovery sibling has no console and does log, and its file is moved into
`warp-oss.log` when the parent dies. If a log starts with "Parent has crashed;
continuing execution", it is the sibling's and the interesting half was never
written. A person double-clicking the binary is unaffected.

A crash itself is usually Warp's deliberate `Failed to render a frame 3 times
in a row; exiting...`, and the sibling that appears was spawned at *startup*
and parked in `WaitForSingleObject` — its arrival means only that the parent
went away.

## Look for the gate first

The single most repeated finding in this fork: **the feature already exists,
complete and tested, and is switched off.** Grep for the flag before writing
anything.

| | what was already there | where the gate was |
|---|---|---|
| T1 | the whole local control plane (`warpctrl`) | `DOGFOOD_FLAGS` + a per-channel settings default |
| T4 | local Warp Drive sync | an account gate |
| T5 | the entire agent transport | one function — `generate_multi_agent_output` |
| T7 | agent fan-out for a run-scale graph | nothing; the verbs existed, only the *plan* was missing |
| I15 | screenshots, input, recording, window enumeration | `DOGFOOD_FLAGS` + a non-default cargo feature |
| I16 | the whole remote-development server | `RELEASE_FLAGS` behind `cfg!(feature = "release_bundle")` — a *packaging* gate, so it is off in every build you make yourself |

Gates come in pairs. `crates/warp_features/src/lib.rs` holds `DOGFOOD_FLAGS`
(runtime); `app/Cargo.toml`'s `default` list holds the compile-time half.
Opening one usually means touching both, plus `app/src/fork.rs`.

**Prefer `fork::FORCE_ENABLED` to editing a flag list.** It sets a *user
preference*, and `FeatureFlag::is_enabled` resolves override → user preference →
channel state. So it outranks every `#[cfg]` and every channel list without
touching an upstream file — which is why I16 needed no edit to
`warp_features/src/lib.rs` despite the flag being `#[cfg(not(windows))]` there.

## Prefer the smallest thing that is still the idea

`crates/warp_cli/src/local_control/graph.rs` is the standard: a run-scale task
graph that added **zero** new app surface — a TOML file and a `while` loop over
verbs that already existed. Reach for a file and a loop before a subsystem.

---

## The fork's seams

Fork behaviour is deliberately concentrated, so it stays reviewable against
upstream and rebasable.

| file | what it owns |
|---|---|
| `app/src/fork.rs` | **the policy seam.** `is_active()`, `FORCE_ENABLED`/`FORCE_DISABLED` feature flags, and ~a dozen predicates (`local_agent_enabled`, `local_drive_enabled`, `account_gate_bypassed`, …). Start here. |
| `crates/http_client/src/egress.rs` | the telemetry deny-list. The "nothing escapes" claim rests on this. |
| `app/src/ai/local_agent/` | a local implementation of the one agent-transport function, answering from the `claude` CLI. |
| `app/src/ai/acp_agent/` | the same function again, answering from **whatever agent `WARP_FORK_ACP_COMMAND` names**, over the Agent Client Protocol (T14.5). It denies every permission request it receives — but **that is not read-only and must never be described as it**: measured, an agent at its own defaults wrote a file and asked nothing, so Warp denied nothing. T14.8 names that mechanism: `claude-agent-acp` starts in session mode **`auto`**, which it describes itself as *"use a model classifier to approve/deny permission prompts"* — so the thing deciding was a model, and Warp was never in the loop. `session/set_mode` to `default` is what makes it ask. |
| `app/src/drive/local_sync/` | account-free Warp Drive: snapshot, apply, git-backed sync. |
| `app/src/ai/mcp/tool_digest.rs` | what each MCP server's tools claimed to be, hashed at connect. The tool rug-pull warning rests on this. |
| `app/src/local_control/console.*` | the console (T12) — the fork's **only** browser-reachable surface. Four unauthenticated routes serving four constants (page, script, manifest, icon), under `default-src 'none'; script-src 'self'`. The script never assigns `innerHTML` and a test pins that; keep it that way, because everything it draws was authored by an agent. **After editing it run `node --check app/src/local_control/console.js`** — it is `include_str!`d, so a syntax error compiles fine, passes every Rust test, and breaks the whole page at runtime. And remember the page draws from `PendingApproval`: a control there must be gated on what the *entry* permits, not only on what the device may do (T14.6). |
| `app/src/local_control/`, `crates/local_control/`, `crates/warp_cli/src/local_control/` | the `warpctrl` control plane, 114 actions. The count is pinned by **two** tests in different crates — update both, and never loosen either. **This line said 109 for two phases**: T11.2 took it to 110, T11.4 to 111 and T11.5 to 114, and each updated the pins without updating this table. Read the count off `catalog_has_exactly_N_retained_actions`, never off prose. |
| `app/src/remote_server/wsl_transport.rs`, `crates/remote_server/src/wsl.rs` | the second `RemoteTransport`: Warp's remote-development server, in a WSL distro instead of over SSH. |

Environment variables the fork adds: `WARP_FORK_ACP_COMMAND` (**name an agent and
it answers the agent panel** — `"opencode acp"`; naming the command *is* the
switch, there is no second flag, and it outranks `WARP_FORK_LOCAL_AGENT`. **The
session cwd comes from the pane, and that is where the agent finds its own
config — so the pane's directory decides whether the user's permission rules
load at all.** Measured: the same agent in a directory without its config file
ran a shell command in `$HOME` and sent no permission request; in a directory
with one it asked, and Warp denied. This corrects an earlier claim here that the
config came from wherever Warp was launched), `WARP_FORK_POLICY` (set `0`/`off`/`false`
to run stock upstream behaviour without rebuilding — use this to A/B a suspected
fork regression), `WARP_FORK_LOCAL_AGENT`, `WARP_FORK_AGENT_SPAWN_DEPTH`,
`WARP_FORK_ALLOW_TELEMETRY_EGRESS`, `WARP_FORK_QUAKE_VISOR` (the one that
defaults **on** — set it off to get upstream's terminal in the hotkey window),
`WARP_FORK_FRAME_LOG` (`on`, or a threshold in ms — slow-frame accounting to
the local log; **reach for this before theorising about why something feels
slow**), `WARP_FORK_EVENT_LOG` (`on`, or a directory — one JSONL file per
agent session, appended as events arrive; **reach for this before theorising
about what an agent did**. T14.9 gave the ACP path tool events, so the "no tool
events at all" recorded here from T14.7 is **no longer true**. It was also
briefly true-looking for a worse reason: until T14.15 an ACP turn wrote **two**
files that never named each other — `session_start`/`stop` under the
conversation id, tool events under the agent's session id — so opening the
obvious one showed a session with nothing between its ends. Now everything for a
turn is filed under **Warp's conversation id**, the way `local_agent` always
did, with the agent's own id on each line as `linked_session_id`. One turn, one
file), `WARP_FORK_CONTROL_BIND` (**the only one
that reaches off the machine** — one literal IP address, optionally with a port
(`192.168.1.5:41234`, `[fd00::1]:41234`; pin one if you want the console on a
home screen, because an ephemeral port makes a saved URL dead on the next
launch). A hostname, a wildcard, or a typo leaves the wide listener shut and
loopback serving, because refusing to start would take out `warpctrl window
close`),
`WARP_FORK_REMOTE_APPROVE` (lets a *paired* device run `agent.approve` — say
**yes** to a CLI agent's permission prompt from a phone. Off unless it is
literally `1`/`on`/`true`/`yes`; `agent.deny` needs no switch, because saying no
can only ever make less happen. Note the opposite parser shape to
`WARP_FORK_CONTROL_BIND`: there a typo must be *refused loudly* because it would
otherwise silently mean something, here a typo is simply not consent).
Tab→pane drag has no variable of its own; `WARP_FORK_POLICY=0` puts the tab's
horizontal-only drag axis back.

---

## Working rules

**Build with `--features gui,warp_control_cli`.** `warp_control_cli` is *not* in
`app/Cargo.toml`'s default list, and without it there is no `--warpctrl` — the
control plane this fork exists to open is simply absent from the binary.

**Stop a running Warp with `warpctrl window close`** (`CloseMainWindow` on
Windows). Killing the process leaves a stale discovery record and a
crash-recovery sibling holding the ports, so the next launch fails. Ordinary
shutdown cleans both up.

**…and cancel a wedged ACP turn first, or it will not close at all.** Measured
T14.10 against an agent built to stall: with a turn in flight that has stopped
answering, `window close` returns `ok: true` and Warp stays up — reproduced on
two separate instances, once after waiting 43 seconds. `agent cancel <id>` and
then `window close` exits in about five. So a wedge is not only a time cost; it
takes away the sanctioned way to stop, which is the one thing `kill` was already
ruled out for. `agent list` now reports `quiet_for_seconds` and `last_activity`
for a turn Warp is driving, which is how you tell there is one to cancel.

**A GUI Warp binds two loopback ports, and only one of them is this fork's.**
Measured 2026-08-24 with `ss -ltnp` against a running instance:

| port | owner |
|---|---|
| `127.0.0.1:9282` | **upstream's** `crates/http_server` — `PORT_BASE` 9277 plus the channel offset, and Oss is +5 |
| ephemeral (e.g. `:34969`) | `warpctrl`, which binds port **0** and publishes whatever it gets in the discovery record |

**This corrects a claim that stood here for two days: `warpctrl` never used
9282.** It also settles the open question below. Upstream's server is started by
`LaunchMode::should_start_local_http_server`, which is `!self.is_headless()` —
no feature flag, no channel gate, nothing fork policy touches. It serves the
routers listed at `app/src/lib.rs:2611` and answers **unauthenticated**: its CORS
layer restricts browsers to `warp.dev` origins and stops nothing else. Do not
put anything sensitive behind it; `warpctrl`'s server is the one with `auth.rs`,
the credential broker and the peer-UID check.

**…and `WARP_FORK_POLICY=0` is still a trap, because the same file tells you to
use that flag to A/B a regression.** Observed 2026-08-22: a policy-off instance
ran with a visible window and held a port, while the discovery directory stayed
**empty** — so `warpctrl window close` answered `no_instance` and there was no
sanctioned way to stop it. The discovery record carries the credential, so no
record means no client can authenticate, not merely that it cannot be found.
The port was upstream's 9282, ungated by policy; the empty directory was
`warpctrl` correctly staying off. Plan the shutdown before a policy-off run.

**…and often you do not need one.** `--warpctrl` runs `init_feature_flags`
before it dispatches, so `WARP_FORK_POLICY=0 warp-oss --warpctrl instance list`
resolves the whole flag set in a process that opens no window and binds no
port. That is enough to A/B any *flag*, which is most of what policy-off gets
used for. Save the GUI run for A/B-ing behaviour. (Put any probe **after**
`mark_initialized()` — `FeatureFlag::is_enabled` panics before it.)

**The agent driving this fork reads `AGENTS.md`, not this file — unless
`opencode.json` says otherwise.** Measured T14.7 by asking it: in this repo
`opencode` listed `AGENTS.md` alone. So an agent sent to build the fork gets
upstream's rules, which say never to `cargo fmt` (folklore this file corrects
below) and which have never heard of the eight-job cap — the rule whose
violation took the WSL VM down the same morning. `opencode.json` at the repo
root now carries `"instructions": ["CLAUDE.md"]`, and after it the agent quoted
both the capped build command and `warpctrl window close` back correctly. The
same file carries `permission: {edit: "ask", bash: "ask"}`, which is what makes
the fork's consent surface reachable at all: **Warp cannot make an agent ask.**
The agent's own config decides *when* to ask; Warp decides only where the ask
lands and who may answer it. Committing that config is how the repo stops
depending on ambient settings for its own safety.

**A request Warp will not answer is usually the agent asking to leave the
project directory — and that is the agent's setting, not Warp's bug.**
`acp_permission` says yes only to tool kinds whose spec meaning stops at the
call, so `other` is refused. Measured T14.8: `other` is exactly what `opencode`
sends *before* any call that would reach outside the project. `cat
.fork/GOAL.md` is one `execute` and is answerable; `cat ~/.bashrc` is an `other`
followed by an `execute`, and refusing the first means the second never arrives.
It resolves paths, so `../warp/...` back inside is a plain `execute`. The remedy
lives in the agent: opencode calls this permission `external_directory`, and
`"permission": {"external_directory": {"/path/*": "allow"}}` in the project's
`opencode.json` stops the ask being raised at all — verified by running.
`claude-agent-acp` sends the same command as one `execute` and never asks this,
so **which requests are answerable is a fact about the agent you named**. Check
that before suspecting Warp when a session stalls on ordinary work.

**This repo's `opencode.json` grants `external_directory: {"~/.cargo/**":
"allow"}`, and what that does is narrower than it reads.** It is there because
reading a dependency's source is ordinary work here and it was the measured
stall. **The grant is scope, not action**: measured 2026-08-29 with it in place,
a write to `~/.cargo/…` still raised an `edit` request and a shell command still
raised an `execute` one, both approvable, and neither ran. So it does not let the
agent do anything unasked — it converts requests Warp *cannot* answer into
requests it can. `~` expands, `**` alone is enough without a sibling `*`, and
the file parses with comments if you ever want to annotate it (all three run,
not assumed). Widen it only for somewhere you would also be content to answer
`edit` prompts about, and add `~/.rustup/**` if toolchain sources start stalling
— that one has not bitten yet, so it is not granted.

**An agent in the panel works in the *pane's* directory, and a fresh pane
starts in `$HOME`.** Not in the directory Warp was launched from. Both agent
paths read `session_context.current_working_directory()`, so this is identical
for `local_agent` and `acp_agent`, and the failure is quiet in the worst way:
measured T14.7, a first turn asked to work on this repo answered "not a git
repository", created `/home/effatha/target/` and wrote there, and reported
success. **`warpctrl input submit 'cd /home/effatha/git/warp'` before the first
prompt**, and for an ACP agent this decides more than the files — the agent
resolves its own permission config from there too.

**Leave the user's `settings.toml` alone.** For any run that needs different
settings, point `XDG_CONFIG_HOME`/`XDG_STATE_HOME` at a scratch directory —
noting that this relocates every other XDG-config tool too, `gh` included.

**…and a scratch profile means first-run onboarding, which looks exactly like a
broken control plane.** The window sits on "Welcome to Warp", so `window list`
reports `has_workspace: false`, `pane list` is empty, and `tab.create` answers
`missing_target`. Seed it instead:

```
$XDG_CONFIG_HOME/warp-oss/user_preferences.json   →   {"prefs": {"HasCompletedOnboarding": "true"}}
```

Both halves are load-bearing and each has burned a session on its own: the
directory is **`warp-oss`**, not `warp-terminal` — a file in the wrong one is
never read — and the key goes **inside `prefs`**, because a flat
`{"HasCompletedOnboarding":"true"}` is silently discarded. Launch once first if
the file does not exist, then merge the key into what Warp wrote; it has real
content. This recipe has cost three sessions a restart while being correctly
recorded in `.fork/TASKS.md` each time, which is why it is here.

**Read back a state-changing step before measuring what follows it.** The
merge-base trap has a second form and it bit on 2026-08-29: a driver script
answered an approval with the digest passed positionally instead of as
`--digest`, so nothing was delivered, the turn parked, and `quiet_for_seconds`
honestly reported 171 seconds of silence — which reads exactly like the wedge
that field exists to detect. As with `git diff A...B` against an assumed base,
the measurement was correct and the input to it was not. **After any mutation,
confirm the mutation before believing the next reading**: after `agent approve`,
check the request has left `agent approvals`. One extra call, and it turns a
three-minute misdiagnosis into a two-second one. Two dearer checks are worth it
before a *surprising* finding goes into a doc: calibrate a new instrument against
a known answer first (`wedged-agent.py` is the pattern — fire on the known
present, stay silent on the known absent), and confirm on a second instrument
when one exists, which is the general form of *take the screenshot before
believing `warpctrl agent read`*.

**Diff test-failure membership, not counts.** Measure a same-session baseline on
a stashed tree and compare *which* tests failed. There is a known pre-existing
failure set (`gh`-dependent git tests, flaky secret-redaction globals, terminal
view) whose members vary run to run — a count that matches can still hide a
regression, and a count that differs by one is usually the flaky set.

**Adding a `warpctrl` action? Run `-p warp --lib` too, not just `-p
local_control`.** The catalog count is pinned in *two* places: the fast one is
`catalog_has_exactly_N_retained_actions` in
`crates/local_control/src/protocol_tests.rs`, and its twin is
`capabilities_advertises_the_complete_catalog` in
`app/src/local_control/mod_tests.rs`. T8.6 updated the first, left the second
red, and shipped — because `cargo test -p local_control` takes a second and the
app crate does not. `crates/warp_cli` holds two more guardrails: an
exhaustive `match` over the CLI enum and a list requiring every action to have a
parseable example.

**Widening a shared type — or merging upstream — is gated by `cargo check
--workspace --all-targets`, not by the binary build.** Same failure mode as
above, one level up. When the fork adds a variant to an enum or a field to a
struct that upstream also constructs, the compiler finds every site *it
compiles* — and `--bin warp-oss` compiles neither test code nor `warp_tui`.
T10.1's merge landed three such breaks that git had merged perfectly cleanly:
two new upstream TUI files matching exhaustively over `BlocklistAIHistoryEvent`
(T8.3's `ConversationSettledChanged`), and six `AgentConversationData` literals
in `crates/persistence` missing T8.3's `settled`. The persistence one **was
already red before the merge** — T8.3 shipped a required field without ever
compiling that crate's tests. A clean `cargo build` proves nothing here.

**Cap the release build: `CARGO_BUILD_JOBS=8 cargo build --release …`.**
Measured 2026-08-29 on WSL: an uncapped release build **took the whole VM down**
— the guest came back at `up 1 min` with an empty `dmesg`, which is the
signature of the VM dying rather than Linux OOM-killing a process. A single
`rustc` compiling the `warp` crate holds **~8.1 GB RSS**, and cargo defaults to
one job per core (32 here), so several 8 GB-class crates reach codegen together
and exhaust the VM's 31 GiB. At `-j 8` the same build finished with 19 GiB still
free. `[profile.release]`'s own comment in `Cargo.toml` records this hazard from
the CI side — *"OOM-killing release builds"* — so it is one failure with two
faces, and the guest-side face is worse because it takes the session with it.
The host has 64 GB and had **no `.wslconfig` at all**, so the VM's 32 GB was
WSL2's default half-of-host rather than a chosen value; one now exists with more
headroom, real swap and `autoMemoryReclaim`.

**Never share `CARGO_TARGET_DIR` between two checkouts of this workspace.**
Measured 2026-08-24: running a baseline in a `git worktree` with the main tree's
target directory (to save disk) left artifacts that did not match either tree,
and the damage was *silent* — the next build failed with
`no variant or associated item named CtrlCCancelsThirdPartyHarness found` and
`no field 'inviteLink' on the GraphQL type 'Team'`, both pointing at source that
was correct on disk. Worse, it invalidates verification done before it: a
`cargo check --workspace --all-targets` that passed only proved the *cache* was
consistent. Give the worktree its own target directory and accept the disk, or
measure the baseline by stashing in place.

**A build script that reads a file it does not `rerun-if-changed` is a merge
trap.** `crates/graphql/build.rs` registers a schema from
`../warp_graphql_schema/api/schema.graphql` and watched only itself, so an
upstream merge that changed the queries *and* the schema together left a stale
registration in `OUT_DIR` and failed with "no field X on type Y" against a schema
that has the field. Fixed in both graphql build scripts (T11.1). If you add one,
declare every input.

**Merging upstream: watch the overlap, not the divergence.** `git merge-base
upstream/master dev` computed, never pasted (see *Method*), then the file sets
intersected — 39 of the fork's 204 files on 2026-08-24, of which 4 conflicted.
That number is the early warning, and it is what makes a soft fork cheap; the
cost is not paid when divergence is incurred but when a merge is deferred.

**Formatting: run `./script/format`, and disregard `AGENTS.md` on this point.**
Measured 2026-08-21: `cargo fmt` with the project's config wants to change **11
files, every one of them fork-authored, with no upstream drive-bys** — so the
workspace-wide command is safe and the "never `cargo fmt`" folklore is wrong.
The real hazard is per-file: tests live in sibling files pulled in with
`#[path]` (see `script/check_no_inline_test_modules`), and rustfmt follows those
edges, so `rustfmt some_mod.rs` silently rewrites `some_mod_tests.rs` too.
Whichever you run, check `git status` afterwards and revert files you did not
mean to touch.

**There is a browser, and it is on the Windows side.** `/mnt/c/Program Files`
holds Firefox, Brave and Zen, and Windows reaches the WSL wide listener — so a
page served by `WARP_FORK_CONTROL_BIND` can be loaded in a real engine and
photographed with `shot.ps1 -Process firefox`. Launch with a scratch profile
(`-profile 'C:\dev\…' -no-remote`, or `--user-data-dir=` for Chromium) so the
user's own session is untouched. **T12 filed "no browser on this machine" three
times as a blocker; it meant the WSL userland and was written as though it meant
the hardware.** Before naming something as needing a person, check that the
blocker is real.

**Screenshotting the Windows build: `shot.ps1 -Process warp-oss`.** It
captures one window via `PrintWindow(hwnd, hdc, PW_RENDERFULLCONTENT)` —
*without* raising or focusing it, and regardless of what is on top. Omit
`-Process` and it silently falls back to a grab of the entire virtual screen,
which on a working desktop is unreadable. `CopyFromScreen` cannot substitute
(it only sees what is displayed), raising the window first cannot substitute
(the foreground lock refuses it from a background process), and the `2` flag is
not optional (GPU-composited windows capture blank without it). Full reasoning
under "The scripts" in `.fork/README.md` — **this recipe has been lost to a
cleared session twice.**

**Running on WSLg:** `env -u WAYLAND_DISPLAY LIBGL_ALWAYS_SOFTWARE=1
./target/release/warp-oss`. Unsetting `WAYLAND_DISPLAY` puts winit on X11, which
is what screenshots and global hotkeys need. Synthetic clicks land there, and
**so do synthetic keystrokes — but only as far as the keymap.** Measured
2026-08-24, correcting a blanket "keystrokes do not land" that had been used to
block work: `use_computer drag --press 0xff1b` mid-drag produced
`EditorAction::Escape` → `PaneGroupAction::CancelDrag` in the log and visibly
cancelled the drag, while `use_computer text "echo …"` into a focused terminal
input produced nothing at all. So cancel keys and shortcuts are drivable and
typing is not. `Key::Keycode(n)` is an X **keysym** on this backend, not a
keycode — Escape is `0xff1b`.

**Screenshotting on WSLg: capture the window, never the root.** Same failure as
the Windows `CopyFromScreen` case and worth the two lines it costs:

```
DISPLAY=:0 xwininfo -root -children     # the app is the child with a real geometry
DISPLAY=:0 import -window 0x20006d /tmp/shot.png
```

`scrot`/`import -window root` return a **solid black frame** — the surface is
GPU-composited and the root window never held its pixels. `import -window <id>`
gets the real contents without raising or focusing anything. The id changes every
launch, so read it rather than remember it; Warp is the child sized like a window
(`1246x802+1089+596`), among Weston's own 1x1 and 10x10 stubs.

**And take the screenshot before believing `warpctrl agent read`.** Measured on
T14.6: a conversation whose panel was displaying a full error paragraph read back
through `agent read` as an exchange with **no output field at all**, because the
error renders as an error block rather than as output. The CLI is the faster
instrument and it is silent about a whole class of state.

---

## Where to read

Five files in `.fork/`, in the order a cold start wants them.

- **`GOAL.md`** — **read this first if it exists.** A dated, deliberately
  temporary horizon: what the fork is being driven toward right now and what
  "done" means as a *run*. It outranks the board's ordering while it stands, and
  it is meant to be deleted when met or abandoned. Absent means there is no
  standing horizon and `TASKS.md` is the plan.
- **`README.md`** — the operating manual: how to build and run on each platform,
  the full `warpctrl` surface, Warp Drive, WSL integration, and the gotchas that
  cost hours. Reach for it whenever you need to *use* something rather than
  change it. Large; navigate by heading.
- **`IDEAS.md`** — the idea board in front of the task board. Fifteen entries,
  each with what already exists underneath it and an argument for or against
  building it. Read before scoping any new feature, because roughly half of them
  turned out to be already built.
- **`TASKS.md`** — the board, T1–T8, plus an "as built" record for each item
  saying what was actually found. **Mostly historic**: read a section when you
  are touching that area, and treat the "as built" and "Decisions on record"
  parts as live. T8 is the current phase.
- **`SPEC.md`** — the original de-telemetry/de-account reasoning, Phases 0–4.
  Superseded as a plan by `TASKS.md` from Phase 5 on, but its survey findings —
  the request-path trace and the kill-switch seam analysis — are still the best
  explanation of *why* the fork is shaped this way.

`git log` is a real source here, not an afterthought: commit bodies carry the
reasoning, including retractions.

## Commits

`fork: <lowercase subject> (Txx)`, where `Txx` is the task from `TASKS.md`. The
body explains *what was found*, not what was typed — including when it
contradicts something previously recorded. Corrections belong in the commit that
makes them, and in the doc that was wrong.
