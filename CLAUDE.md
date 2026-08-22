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

Gates come in pairs. `crates/warp_features/src/lib.rs` holds `DOGFOOD_FLAGS`
(runtime); `app/Cargo.toml`'s `default` list holds the compile-time half.
Opening one usually means touching both, plus `app/src/fork.rs`.

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
| `app/src/local_control/`, `crates/local_control/`, `crates/warp_cli/src/local_control/` | the `warpctrl` control plane, 100 actions. |

Environment variables the fork adds: `WARP_FORK_POLICY` (set `0`/`off`/`false`
to run stock upstream behaviour without rebuilding — use this to A/B a suspected
fork regression), `WARP_FORK_LOCAL_AGENT`, `WARP_FORK_AGENT_SPAWN_DEPTH`,
`WARP_FORK_ALLOW_TELEMETRY_EGRESS`.

---

## Working rules

**Build with `--features gui,warp_control_cli`.** `warp_control_cli` is *not* in
`app/Cargo.toml`'s default list, and without it there is no `--warpctrl` — the
control plane this fork exists to open is simply absent from the binary.

**Stop a running Warp with `warpctrl window close`** (`CloseMainWindow` on
Windows). Killing the process leaves a stale discovery record, and a
crash-recovery sibling re-binds `127.0.0.1:9282` so the next launch fails.
Ordinary shutdown cleans both up.

**Leave the user's `settings.toml` alone.** For any run that needs different
settings, point `XDG_CONFIG_HOME`/`XDG_STATE_HOME` at a scratch directory —
noting that this relocates every other XDG-config tool too, `gh` included.

**Diff test-failure membership, not counts.** Measure a same-session baseline on
a stashed tree and compare *which* tests failed. There is a known pre-existing
failure set (`gh`-dependent git tests, flaky secret-redaction globals, terminal
view) whose members vary run to run — a count that matches can still hide a
regression, and a count that differs by one is usually the flaky set.

**Formatting: run `./script/format`, and disregard `AGENTS.md` on this point.**
Measured 2026-08-21: `cargo fmt` with the project's config wants to change **11
files, every one of them fork-authored, with no upstream drive-bys** — so the
workspace-wide command is safe and the "never `cargo fmt`" folklore is wrong.
The real hazard is per-file: tests live in sibling files pulled in with
`#[path]` (see `script/check_no_inline_test_modules`), and rustfmt follows those
edges, so `rustfmt some_mod.rs` silently rewrites `some_mod_tests.rs` too.
Whichever you run, check `git status` afterwards and revert files you did not
mean to touch.

**Running on WSLg:** `env -u WAYLAND_DISPLAY LIBGL_ALWAYS_SOFTWARE=1
./target/release/warp-oss`. Unsetting `WAYLAND_DISPLAY` puts winit on X11, which
is what screenshots and global hotkeys need. Synthetic clicks land there;
synthetic keystrokes still do not, which is why two tasks are blocked on a
person.

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
