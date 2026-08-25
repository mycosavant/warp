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
| `app/src/drive/local_sync/` | account-free Warp Drive: snapshot, apply, git-backed sync. |
| `app/src/ai/mcp/tool_digest.rs` | what each MCP server's tools claimed to be, hashed at connect. The tool rug-pull warning rests on this. |
| `app/src/local_control/`, `crates/local_control/`, `crates/warp_cli/src/local_control/` | the `warpctrl` control plane, 109 actions. The count is pinned by **two** tests in different crates — update both, and never loosen either. |
| `app/src/remote_server/wsl_transport.rs`, `crates/remote_server/src/wsl.rs` | the second `RemoteTransport`: Warp's remote-development server, in a WSL distro instead of over SSH. |

Environment variables the fork adds: `WARP_FORK_POLICY` (set `0`/`off`/`false`
to run stock upstream behaviour without rebuilding — use this to A/B a suspected
fork regression), `WARP_FORK_LOCAL_AGENT`, `WARP_FORK_AGENT_SPAWN_DEPTH`,
`WARP_FORK_ALLOW_TELEMETRY_EGRESS`, `WARP_FORK_QUAKE_VISOR` (the one that
defaults **on** — set it off to get upstream's terminal in the hotkey window),
`WARP_FORK_FRAME_LOG` (`on`, or a threshold in ms — slow-frame accounting to
the local log; **reach for this before theorising about why something feels
slow**), `WARP_FORK_EVENT_LOG` (`on`, or a directory — one JSONL file per
CLI-agent session, appended as events arrive; **reach for this before
theorising about what an agent did**). Tab→pane drag has no variable of its
own; `WARP_FORK_POLICY=0` puts the tab's horizontal-only drag axis back.

---

## Working rules

**Build with `--features gui,warp_control_cli`.** `warp_control_cli` is *not* in
`app/Cargo.toml`'s default list, and without it there is no `--warpctrl` — the
control plane this fork exists to open is simply absent from the binary.

**Stop a running Warp with `warpctrl window close`** (`CloseMainWindow` on
Windows). Killing the process leaves a stale discovery record and a
crash-recovery sibling holding the ports, so the next launch fails. Ordinary
shutdown cleans both up.

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

**Leave the user's `settings.toml` alone.** For any run that needs different
settings, point `XDG_CONFIG_HOME`/`XDG_STATE_HOME` at a scratch directory —
noting that this relocates every other XDG-config tool too, `gh` included.

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

---

## Where to read

Four files in `.fork/`, in the order a cold start wants them.

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
