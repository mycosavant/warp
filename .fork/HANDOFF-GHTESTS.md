# Handoff: the `gh`-test gap, the live PR drive, and what 2026-09-12 left open

Written at the end of the session that fixed the routed commit message and the
two PR-content paths. **Those are done and in `git log`** (`036bcccac`,
`848427cfc`, `35a29d6d6`); this file is only what they left open. Retire it
when its list is empty.

`.fork/HANDOFF-NEXT.md` is still the standing board and is **not** superseded —
its items 1, 3, 5 and 6 were not touched. This file adds to it.

---

## Read these first, in this order

1. **`CLAUDE.md`** — in particular *Method: run it*, the stale-doc table, and
   the new rule under the seams table about a routed pane's daemon.
2. **`.fork/runs/routedcommit-2026-09-12/README.md`** — the live run of the
   commit-message half, including its stated limits. The rig you will reuse.
3. **`.fork/runs/run-live-2026-09/friction.md`**, agent rows **a24** and
   **a25**. a25 carries the `gh` finding *and its own correction*; read the
   correction, not the first clause.
4. **`.fork/docs/wsl.md`**, the diff-panel row.

## State of the machine, end of session

Nothing running — no `cargo`, `rustc`, `rust-analyzer`, `warp-oss`,
`llama-server`. Discovery directories empty. Working tree clean.
**219 commits unpushed**, by design.

| binary | version | note |
|---|---|---|
| Linux `target/release/warp-oss` | `v0.fork.036bcccac` | the WSL daemon, via the symlink in `~/.warp-dev/remote-server/` |
| Windows `target\debug\warp-oss.exe` | `v0.fork.036bcccac` | built this session, for the scratch-profile rig |
| Windows `target\release\warp-oss.exe` | **`v0.fork.7a5a3028e`**, 2026-09-10 | **stale** |

**That last row is the first thing to decide.** `warpdev.ps1`'s product profile
launches the *release* binary, so the build the maintainer actually lives in
has **neither** fix in it. Nothing is broken by that — it behaves as it did
last week — but "the commit dialog fills in now" is not true of their daily
binary until it is rebuilt. Neither fix has ever run in a release build.

`~/scratch-localai` is the test repository: commit `b482877` plus an
uncommitted `routed_commit_probe` in `main.rs`, which is the 2026-09-12
marker. Reusable as-is; if you re-run, add a *new* dateable marker rather than
reusing that one — the point of it is that it cannot predate the run.

`CLAUDE.md` is 78,530 chars. It passes the cloud tier and fails the local tier
by ~60k, which is standing and not something this session changed materially
(+306 across both fixes, each paid for by moving an account out).

---

## Task 1 — five tests that never run the thing they are testing — **DONE 2026-09-12**

**Closed the same day, route 2 as recommended.** `run_gh_command` resolves `gh`
against `path_env` with `warp_util::path::resolve_executable_in_path`, which
already existed and which `docker_sandbox.rs` was already using for this.
`util::git` is **24 passed, 0 failed**; both calibrations are in
`.fork/runs/ghpath-2026-09-12/`. The `run_git_command` question this task
raised is answered there too: same mechanism, and it is **fine**, because git's
`path_env` exists for hooks and hooks do inherit the child environment
(measured). Two upstream call sites still carry the defect and were left alone
— `node_runtime` and `crates/lsp/command_builder.rs`, neither measured to fail
for a real user. The account below is kept as written.


**The finding, measured 2026-09-12.** `app/src/util/git_tests.rs` has five
tests that write a fake `gh` into a temp directory and pass that directory at
the front of a `path_env` string. **The fake is never executed.**
`run_gh_command` does:

```rust
let mut cmd = Command::new("gh");
if let Some(path_env) = path_env { cmd.env("PATH", path_env); }
```

Program resolution happens against the **parent** process's `PATH`. Setting
`PATH` in the child's environment changes what the child sees *after* it
starts; it does not change which file gets executed.

**How it was established, because the first reading was a guess.** Narrow
`path_env` to the fake directory *alone* and run it. If lookup used the child's
`PATH`, the fake is the only candidate and must run. What actually happened:
the real `gh` ran and failed with *"unable to find git executable in PATH"* —
the real binary, executed via the parent's `PATH`, then unable to find `git`
because the child `PATH` it inherited held only the fake directory. That
message is the proof; it cannot come from the fake, which is a four-line shell
script that never mentions git.

**The five:**

- `get_repository_info_returns_none_when_gh_cannot_resolve_github_repo`
- `get_repository_info_reads_gh_repo_view`
- `get_pr_for_branch_does_not_require_origin_remote`
- `get_pr_for_branch_returns_none_when_gh_finds_no_pr`
- `get_pr_for_branch_returns_none_when_gh_cannot_resolve_github_repo`

All five are currently **failing** on this machine, at HEAD as well as with
this session's changes — membership diffed against a stashed baseline, not
counted. They pass on a machine whose real `gh` happens to produce what the
fake was written to imitate, which is what makes this the interesting shape:
**a test that passes for a reason unrelated to its subject, and fails only
when the environment shifts under it.**

**A caution about the count, and it is this session's own mistake.**
a25 first recorded *"15 tests"*. Fifteen is `grep -c fake_bin`, and each test
names `fake_bin` three or four times. The number of tests is five. I wrote the
grep's answer down as the tests' answer. Check any count you inherit from this
file the same way.

### What to actually do

Three routes, and I would take the second.

1. **Delete them.** Honest, cheap, loses coverage that was never there anyway.
   Weak: it removes the only place anyone thought about these paths.
2. **Give `run_gh_command` a resolvable program.** The smallest honest change
   is to let the caller supply the binary — resolve `gh` through the
   `path_env` explicitly (find the first `gh` on that string and exec it by
   absolute path), or add a `gh_program: Option<&Path>` the tests can set.
   Then the fakes work as intended and the five tests test what they claim.
   **This also changes production behaviour**, and in the right direction: the
   daemon computes an interactive-shell `PATH` precisely so tools resolve the
   way the user's shell resolves them, and today `gh` is the one tool that
   ignores it. Check whether the same bug affects `run_git_command` before
   deciding — if it does, that is the more interesting half of this task.
3. **Rewrite them as pure-function tests**, the way `pr_create_args` was
   extracted in `35a29d6d6`. Best where the thing under test is a mapping;
   useless where the thing under test is the subprocess contract.

**Definition of done.** Each surviving test fails when the fake's behaviour
changes. Prove that by editing a fake to answer differently and watching the
test redden — not by watching it pass.

**The trap to avoid.** Do not "fix" these by making them pass on a machine
with no `gh` installed. They already pass three different ways (real `gh`
erroring, fake erroring, `gh` absent and `is_gh_missing_error` catching it);
adding a fourth is not progress.

### The sixth failure is not part of this — **DIAGNOSED 2026-09-12, environmental**

Not a product defect. The maintainer's global `tag.gpgsign = true` turns the
test's `git tag v1.0` into an annotated tag, which fails non-interactively with
`fatal: no tag message?`; the tag is never created, the checkout never
detaches, and `detect_current_branch_display` correctly reports `main`. Both
`init_repo` helpers now disable commit and tag signing locally. The paragraph
below is the original, which was right to ask for a diagnosis first.


`detached_tag_display_returns_short_sha` fails for an unrelated reason and
touches no `gh`: after `git tag v1.0 && git checkout v1.0`,
`detect_current_branch_display` returns **`main`** where the test expects a
short sha. Either the checkout is not detaching under this git version, or the
function resolves a branch pointing at the same commit. **Unexplained. Diagnose
before assuming it is environmental** — if the function really does report a
branch name on a detached HEAD, that is a product defect, not a test defect,
and it would show in the branch chip.

---

## Task 2 — read this before Task 1, because it bit me

**Calibrate every test by breaking it, and make the break script prove the
break landed.** This session ran a calibration whose Python patch did not
match (a typo: `vec[` for `vec![`). The script printed the tests as `ok` and
exited 0, and those three "passes" meant nothing at all — the code was
unmodified. It is this repository's favourite defect, *reported success while
sending nothing*, wearing a calibration script's clothes.

The shape that cannot do that:

```bash
set -eu
python3 - <<'PY'
s = open(p).read()
assert OLD in s, "pattern not found -- break NOT applied"
open(p,'w').write(s.replace(OLD, NEW, 1))
print("break applied: <what it now does wrongly>")
PY
cargo test ... | grep -E '^test .*\.\.\. (ok|FAILED)|^test result'
cp /tmp/backup <file>      # restore, then confirm with git diff --stat
```

And **predict each test's direction before running**. In this session's last
calibration, one of three tests was *supposed* to stay green (the break made
the `--fill` fallback universal, which is what that test asserts). A break
where everything reddens is usually a break that is too broad to mean much.

---

## Task 3 — drive the PR path live — **DONE 2026-09-12**

**Route 1, as recommended, and both paths driven.**
`.fork/runs/routedpr-2026-09-12/`. Model requests rose at each confirm (4 → 6
standalone, 9 → 11 for the chain), each pair being a 10-token title and a
118-token body from an identical 369-token prompt; both runs then stopped at
`gh` with *"none of the git remotes ... known GitHub host"*, which is the
failure the handoff named as passing rather than falsifying. The chain's dialog
resolved in seven seconds rather than hanging, and its commit and push both
landed first. No pull request was created and none could be.

~~One defect found on the way and **not fixed**~~ — **fixed the same day, and
it was two.** The chain now carries a `CommitChainStage` and says which stage
it reached; and `user_facing_git_error` no longer answers *"GitHub CLI not
authenticated"* to `gh`'s no-GitHub-remote message, which merely *suggests*
`gh auth login`. `.fork/runs/routedpr-2026-09-12/`.

The account below is kept as written.


**What is already proven**, and do not re-prove it: the commit-message half is
measured end to end (`.fork/runs/routedcommit-2026-09-12/`), and the
content-to-`gh` mapping is pinned deterministically by `pr_create_args` tests.

**What is not proven**: that on a routed pane, clicking Create PR causes *this
side* to call the model. That is the fix, and it has only ever run in tests.

**The constraint that shapes the whole task: no real pull request.** The
standing rule forbids opening one without an explicit say-so, and the last
session did not have it. So the run must stop after `gh` is invoked.

### Three routes

1. **Local bare remote, real `gh`, expected failure.** Give a scratch repo an
   `origin` that is a bare repo on disk (`git init --bare`), push the branch,
   click Create PR with generation on. The model request fires on the GUI side,
   then `gh pr create` fails with *"none of the git remotes configured for this
   repository point to a known GitHub host"*. **This is the recommended route.**
   It proves the half that changed and creates nothing. `git_actions_tests.rs`
   has `init_repo_with_origin`, which sets exactly this up — copy its shape.
2. **Shadow `gh` in the distribution.** Because of the Task 1 finding, a fake
   `gh` must be on the **daemon's own inherited `PATH`**, not on the `path_env`
   it passes down. `crates/remote_server/src/wsl.rs` spawns the daemon through
   `wsl.exe` with **no env hook** (checked: no `.env(` / `.envs(` / `WSLENV` in
   that file), so the only lever is a directory already early on the login
   shell's `PATH`. That means shadowing the maintainer's real `gh` for every
   process, which must then be undone. **Do not take this route casually**, and
   if you do, remove the shim in the same command that creates it.
3. ~~**A throwaway private GitHub repo.** Needs the maintainer to say yes. It is
   the only route that proves a PR is actually created with the generated
   title. Ask; do not assume.~~ **Done 2026-09-12, with say-so**, against the
   maintainer's own `mycosavant/pocket-tts` rather than a throwaway repo (a
   real doc worth having committed, not a disposable test artifact). Found a
   real bug on the way: the chain's push never sets upstream tracking and its
   `gh pr create` call never passes `--head`, so `gh` refuses a branch that
   is genuinely already on the remote with the right content — reproduced
   twice, fixed manually both flags at once by calling `gh pr create --head`
   directly. **The generated PR title/body itself was lost** when the chain
   failed before displaying or persisting it; the resulting PR
   (https://github.com/mycosavant/pocket-tts/pull/11, left open) carries
   content assembled from the real AI-generated commit messages instead.
   Full account: `.fork/runs/pockettts-pr-2026-09-12/`.

### The recipe, reusing what exists

`.fork/runs/routedcommit-2026-09-12/run.sh` is the driver — launch, routed
check, panel, click, capture. Adapt it rather than writing a new one. The
non-obvious parts it already handles:

- **The Windows *debug* binary, on purpose.** `WARP_DATA_PROFILE` is honoured
  by debug builds only; a release build runs on the maintainer's real profile.
- The `localai` scratch profile already declares the loopback endpoint.
- `C:\dev\llama\serve.ps1` starts the model; `grep -c launch_slot_
  /mnt/c/dev/llama/server.err` is the request counter that makes "the model was
  called" observable.
- `warpctrl surface code-review open` beats the keyboard shortcut (posted
  modifier key-downs are not modifier state — friction a5).
- **Both binaries must carry the commit**, and they are built one side at a
  time. Never both at once.

### What to look for, and what would falsify the fix

- Model requests increase **at the moment Create PR is clicked**, with the pane
  reporting `"where": "host"`. If they do not, the client is not generating.
- The `gh` failure names the *GitHub host*, not a missing title. A failure
  about an empty title would mean content was sent half-specified.
- **The commit chain is the case worth driving, not just the standalone
  create.** It is the one with the ordering constraint, and it is the one where
  a deferred completion event could leave the dialog open forever. Drive
  *Commit and publish*, and watch that the dialog resolves rather than hangs.

---

## Task 4 — loose ends, smallest first

~~**Version skew is unexercised**, still — the 2026-09-12 PR drive also ran
  both halves at the same commit deliberately. Both halves were deliberately the same
  commit. The claim that an older daemon degrades to upstream behaviour rests
  on prost skipping an unknown field — read, not measured. Cheap to test: run
  a new GUI against a daemon binary built before `036bcccac`.~~ **Measured
  live 2026-09-12**, `.fork/runs/version-skew-2026-09-12/`: an old daemon
  (the commit before `return_diff_only`) against the current GUI produced
  exactly the predicted pre-fix behaviour — *"No AI endpoint is
  configured"*, dialog resolved rather than hung, one informational
  version-mismatch WARN in the log and nothing else. Only the commit-message
  path was driven; the PR-content path has the same shape but wasn't
  separately tested.
~~**The truncation budget is now computed on one side and never rechecked on
  the other.** `get_diff_for_commit_message` truncates to `MAX_DIFF_CHARS_FOR_AI`
  on the daemon, and the client sends what it is given. A diff that large has
  not been tried, on either the commit-message or the PR path.~~ **Corrected
  and closed 2026-09-12**: not daemon-exclusive — `git_actions.rs` is
  explicitly backend-agnostic, so the same truncation runs in-process for an
  unrouted session too. Both paths now have a real oversized-diff test
  (`a_huge_commit_message_diff_is_truncated_within_budget`,
  `a_huge_pr_diff_is_truncated_within_budget`), plus a direct unit test on
  `truncate_on_char_boundary` for the multi-byte cut-point case neither
  integration test could reach precisely. All four calibrated by breaking:
  each redress reddened only the test it targets.
- **`warpctrl session inspect` with no argument answers `ambiguous_target`
  whenever a profile restores several tabs**, because *every* restored tab
  reports `is_active: true`. Six tabs, six actives. That reads as a routing
  failure and is a targeting failure. Worth deciding whether more than one
  active session is itself the bug.
- ~~**`detached_tag_display_returns_short_sha`**~~ — done, see Task 1's last
  section.
- **The Windows release build is stale**, see the state table.

## Still open from `.fork/HANDOFF-NEXT.md`, with the maintainer's 2026-09-12 steer

- **`authenticate` half 2** — **Gemini CLI and gemini-acp are parked**, the
  maintainer's call: Gemini models are reachable through opencode or codex on
  OpenRouter, so the credential door is not worth opening yet.
- **kode-rs** — parked, the maintainer's call.
- **CLAUDE.md reduction** — **the maintainer considers this largely done**, with
  possible polish later. Do not start a fresh reduction pass. Keep paying for
  additions with removals.
- ~~**Voice** — **split, and only one half is available.** Read-aloud, TTS and
  the mobile entry point are the maintainer's own current work: do not touch
  them. The half that is open is **openwhispr, fully local dictation into the
  GUI and the TUI**. Note before starting: `crates/warp_tui` already has a
  `voice_input` cargo feature with rendering tests, and the fork's
  `LocalTranscriber` already has `http` and `command` backends that openwhispr
  may fit unchanged — so **measure what already works before building
  anything**.~~ **Narrowed and corrected 2026-09-12, in the maintainer's own
  words.** "Do not touch" was about `pocket-tts`'s own codebase specifically —
  they were reviewing it themselves when another session wanted to merge PRs
  into it, not a ban on this fork's dictation work or on discussing voice
  architecture here. Separately: the maintainer would "highly prefer that we
  use what is already built and on-thesis here rather than mess with another
  fork" — own the stack end-to-end where reasonably possible. `openwhispr`
  isn't theirs (no commits made to it); `franken_whisper`, checked as an
  alternative, is a friend's project and stale. Neither is worth chasing as a
  dependency. **The dictation half is already done and already
  self-contained**: `LocalTranscriber` has generic `Http`/`Command` backend
  contracts that point at *any* local engine the maintainer runs themselves —
  there is no fork-side integration task waiting here. `pocket-tts` (Android,
  sherpa-onnx, voice "alba") stays a separate personal workflow project; the
  only bridge this fork needs is the already-built `warpctrl agent trace` read
  primitive. `.fork/docs/voice.md` and `.fork/tickets/T02` remain the index.
- ~~**End-to-end park-and-approve inside a TUI session** — never verified.~~
  **Verified live 2026-09-12**, `.fork/runs/tui-approve-2026-09-12/`.

---

## Standing constraints — all still in force

- **No push, no PR, no upstream merge without an explicit say-so.**
- **Permission posture is frozen.** Do not measure it further.
- **Never build on both sides of the VM at once.** This is the only build rule.
- **Never `wsl --manage --move` this distro.**
- **Leave the maintainer's config alone** — `settings.toml`, `~/.codex`, and
  their real Warp profile. Use `WARP_DATA_PROFILE=localai` and the debug binary.
- **Leave no Warp or agent processes running**, and say which if you leave one.
- **Credentials: print provider and field *names*, never values.**

## Gates

```
cargo check --workspace --all-targets            # --bin compiles neither tests nor warp_tui
./script/format                                  # then git status; revert drive-bys
cargo test -p warp --lib fork::                  # 53 at last count
cargo test -p warp --lib code_review::git_actions # 5
cargo test -p warp --lib util::git               # 24 pass, 0 fail since 2026-09-12 (was 18/6)
```

**`./script/format` reformats `crates/remote_server/src/manager_tests.rs`
every single time**, because rustfmt follows the `#[path]` edge from
`manager.rs`. It is an unrelated import reorder. Revert it; it caught me twice
in one session.

**Diff test-failure membership, not counts.** The `util::git` six were the
baseline until 2026-09-12; all six are now fixed, so any failure there is
yours. Stash and re-run if you are
unsure whether a failure is yours — it costs two builds and settles it.

## What "done" looks like for a finding here

A **rule** in `CLAUDE.md`, in as few lines as the rule needs and paid for by a
removal; its **account** on a `.fork/docs/` page or in a `.fork/runs/` record;
a line in the agent's friction table; and a commit whose body says *what was
found*, including what it contradicts. **Say what you did not establish** — a
finding with no stated limit reads stronger than it is.
