# Handoff: the window-focus bug, and what its session left open

Written 2026-09-12 at the end of the session that reviewed Sonnet's afternoon
run and found a real bug underneath a wrong one. **`HANDOFF-NEXT.md` is still
the standing board and is not superseded**; `HANDOFF-GHTESTS.md` is this file's
sibling, from the session before. This one adds to both. Retire it when its
list is empty.

One correction to `HANDOFF-NEXT.md` while you are there: its *read these first*
item 2 sends you to `.fork/GOAL.md`, which **does not exist**. Per `CLAUDE.md`,
absent means there is no standing horizon and `.fork/tickets/` is the plan.

---

## Read these first, in this order

1. **`CLAUDE.md`** — *Method: run it*, the stale-doc table, and *Working rules*.
   The file is over its local-tier budget; every addition needs a removal.
2. **`.fork/docs/warpctrl.md`**, the last section — *No focused window is the
   ordinary case, not an error*. Written this session. It is the account; the
   three commits below are the change.
3. **`.fork/runs/pockettts-pr-2026-09-12/README.md`**, **the header only**. The
   body is a wrong diagnosis kept per the never-rewrite rule. Read the
   superseding block at the top and do not act on the body.
4. **`.fork/HANDOFF-GHTESTS.md`** — its Task 4 list, now partly struck.

## State of the machine

Working tree clean at `aa406e7ca`, branch `dev`, **236 commits unpushed**
(normal and deliberate). Nothing running.

| binary | version | note |
|---|---|---|
| Linux `target/release/warp-oss` | **`v0.fork.7d8e21b76`**, 12:52 | **predates all three commits below** |
| Windows `target\debug\warp-oss.exe` | 12:56 | same |
| Windows `target\release\warp-oss.exe` | 14:35 | same |
| Windows checkout `/mnt/c/dev/warp` | `76d8b07e6` | **11 commits behind WSL `dev`** |

**That first row is an asset, not just a staleness warning.** Every binary on
this machine is a *pre-fix* build for both halves of the focus work, so you
have a free A/B control. **Copy it aside before you rebuild** — once
`build.sh` runs, the control is gone:

```bash
cp target/release/warp-oss /tmp/warp-oss-prefix-7d8e21b76
```

## What shipped this session

| commit | what |
|---|---|
| `8085cb919` | retracts the PR-drive diagnosis; adds a `user_facing_git_error` arm for gh's "must first push" message |
| `fc2a9bb46` | **the fix**: `metadata_config::select_window_ids` now uses `active_or_single_window_id`, so tab/pane *writes* resolve without OS focus. New `metadata_config_tests.rs`. `app.active` kept strict, deliberately |
| `aa406e7ca` | two test names that overclaimed; the focus account in `warpctrl.md` |

---

> **Progress, 2026-09-12 evening.** Task 1 done (`79e6ffe3f`). Task 2 done:
> driven live on Windows, the fix holds and `app.active` stayed null
> (`.fork/runs/focus-live-2026-09-12/`). Task 3: workspace check green, the
> ambiguous-branch test written and mutation-calibrated (`dcdc80494`); the
> twice-run full-lib baseline is the one item left. Task 4 done: the count
> (`bb0af075d`) and the GHTESTS audit (`41e77d9ef`). Windows checkout synced
> to `bb0af075d` and both Windows binaries rebuilt; pre-fix copies kept in
> `C:\dev\prefix-debug-7d8e21b76` and `C:\dev\prefix-release-76d8b07e6`.
>
> **New, found by the live run:** the Windows credential broker intermittently
> refuses a call with `0x80070558`, impersonating a pipe before reading from
> it. See the run record; not fixed. The order is read from the code, not
> guessed: `handle_credential_broker_connection` calls `ensure_same_user_peer`
> (`app/src/local_control/mod.rs:841`) before `read_broker_request` (`:842`),
> deliberately, per its doc comment. The likely fix is read the framed bytes,
> then impersonate, then decode, which still checks identity before anything
> caller-sent is interpreted. It reorders an authentication boundary, so send
> it through fable-reviewer and calibrate it live with the driver in the run
> record, which retries and logs this error.

## ~~Task 1 — the `manager_tests.rs` format drift (about a minute)~~ done, `79e6ffe3f`

`./script/format` rewrites `crates/remote_server/src/manager_tests.rs` on a
clean tree, every time. It moved `use warp_core::channel::Channel;` up into the
external-crate group — pre-existing drift in a file nobody in the last two
sessions touched, reverted three times so it would not ride along in unrelated
commits.

**`rustfmt --check` on that file alone reports nothing**, which is the trap:
the drift only appears under the project's own config.

```bash
RUSTC_BOOTSTRAP=1 cargo fmt -- --config imports_granularity=Module \
  --config group_imports=StdExternalCrate --check
```

Fix: run `./script/format` on a clean tree, commit *only* that file, subject
`fork: absorb a standing rustfmt import drift in manager_tests (FORMAT)`.
Check `git status` afterwards — if it touches anything else, that is a second
finding, not part of this task.

## Task 2 — the live pass, which is the one that matters

**Nothing about the focus fix has been driven live.** It is proven against the
test platform, whose `active_window_id()` returns `None` unconditionally
(`crates/warpui_core/src/platform/test/delegate.rs:100`). That is the same
*condition* as an unfocused window, not the same *run*.

The mechanism under test, on the real backend
(`crates/warpui/src/windowing/winit/window.rs:193`):

```rust
windows.find(|w| w.has_focus() && w.is_visible())
```

**Run this on the Windows build, not on WSLg.** The maintainer observed the
original symptom there, by clicking the desktop on a second monitor. Under WSLg
the compositor is Weston and its focus semantics are not the desktop's — a
green result there would not mean much, and a red one might be Weston. This is
an open question, not a settled one.

Recipe, with exactly **one** Warp window open:

1. Sync and rebuild Windows: `git -C /mnt/c/dev/warp fetch origin dev;
   git -C /mnt/c/dev/warp merge --ff-only FETCH_HEAD`, then `C:\dev\build.ps1 -Release`.
2. Launch via `warpdev.ps1` (never a bare `Start-Process` without
   `-NoNewWindow` — see `CLAUDE.md`, or you get no log).
3. **Click another application, or the desktop on the other monitor.** Warp
   must not be frontmost.
4. From a shell, three calls in this order:

```
warpctrl tab reset-name      # the subject (this said reset_name, which clap rejects)
warpctrl session inspect     # control: worked since 619c345a6
warpctrl app active          # control: MUST still report a null window_id
```

**The falsification criterion, stated before the run.** `tab reset_name`
answers `ok: true` with a `window_id` and `tab_id`; `session inspect` resolves;
and `app active` still reports **null** — because `active_chain` is a
deliberate exception and a fallback there would be a lie. If `app active`
starts naming a window, the exception was lost and that is a regression, not a
success.

Then the same three against `/tmp/warp-oss-prefix-7d8e21b76` (or the stale
Windows release). Expected pre-fix: `tab reset_name` fails
`missing_target: tab.reset_name requires an active Warp window`, `session
inspect` resolves. **A run where the pre-fix binary also succeeds means the
window was focused after all** — check that before believing the fix does
nothing.

Traps: stop Warp with `warpctrl window close`, never `kill`. Two discovery
registries exist (`$XDG_RUNTIME_DIR/warp/local-control` vs
`$HOME/.warp/local-control`) and a `no_instance` from the wrong one looks
exactly like an absent Warp. Use a scratch profile, never the maintainer's
`settings.toml`.

## Task 3 — the tests this session did not run

- **`cargo check --workspace --all-targets` was never run.** Only
  `-p warp --lib`. `CLAUDE.md` asks for the workspace check and `--bin warp-oss`
  proves nothing about `warp_tui` or test code. Run it first; it is the cheapest
  thing on this list.
- **No full-lib baseline.** Green here: `local_control::` 185, `code_review::`
  145, `util::git::` 26. Nobody ran `-p warp --lib` whole. Do it **twice** and
  diff *membership*, not counts — the known flaky set is mostly
  `ai::mcp::file_based_manager` login-state tests, which pass alone and serially.
- **The `ambiguous_target` branch of `active_or_single_window_id` has no test.**
  Two or more windows with no reported focus should refuse. Unknown whether the
  `App::test` fixture can open a second *window* at all — `mock_workspace` gives
  one, and `create_tab` adds tabs, not windows. If it cannot, say so in the test
  file rather than leaving the gap silent.

## Task 4 — the review's one unfinished item

The fable-reviewer pass produced ten findings; eight are closed in the three
commits above and one (its finding 9) was settled by running the mutation — the
`session_inspect` integration test does redden when the tab-widening is
reverted.

**What is left is a count.** `619c345a6`'s body says the fix covers *"all seven
read/inspect actions that route through it"* and enumerates none. The reviewer
counted eight plus `agent.rs`'s call; this session declined to assert any
number after getting one wrong. `select_window_entries` has exactly two call
sites (`metadata.rs:399` and `:769`), so the set is reachable by tracing — do
that, or delete the number from the record. Do not paste a third guess.

Also unaudited: the reviewer explicitly did not check the **rest** of
`HANDOFF-GHTESTS.md` for other stale claims, and this session found two in the
part it did read.

---

## Retracted this session — do not re-derive these

Four claims that read as measured and are false. Each cost real time; the whole
point of writing them down is that the next session does not pay again.

1. **"The commit chain's push never sets upstream."** It does —
   `app/src/util/git.rs:711`, upstream's code since `0dbd3d567`. The
   2026-09-12 PR drive failed because its clone was `--depth 1`, so the refspec
   was `+refs/heads/main:refs/remotes/origin/main` and the push wrote no
   `refs/remotes/origin/<branch>`. **gh decides "was this pushed" by looking for
   that ref, not by reading branch config.** `git branch -vv` prints its
   `[origin/x]` bracket from the ref too, which is why the wrong reading looked
   measured.
2. **Do not add `--head` to `gh pr create`.** A bare `--head` skips gh's remote
   interrogation and leaves the head repo to API resolution, which in a fork
   workflow resolves to the base repo rather than the user's fork. Checked in
   gh's source, not assumed.
3. **"`619c345a6` silently changed behaviour for six write actions."** False.
   `metadata.rs` and `metadata_config.rs` **each have a private
   `select_tab_entries`**, so the write chain never reached the changed code.
   This came from matching a grep on a function name instead of tracing which
   definition each call site bound to. It is also the reason the half-done
   unification hid for a day.
4. **The `--window 0` citation in `619c345a6`'s body.**
   `.fork/runs/version-skew-2026-09-12/` contains no such thing; it records
   `--tab` and `ambiguous_target`, which is the different bug fixed in
   `03bf98fd0`. And `write_oversized_file`'s comment said 160,891 bytes where
   the real figure is **124,891**, as that commit's own body already said.

**Two bugs share one symptom and are easy to re-confuse:**

| fixed in | error | cause |
|---|---|---|
| `03bf98fd0` | `ambiguous_target` | `session_inspect` did not default its own target. Nothing to do with restore. |
| `619c345a6`, `fc2a9bb46` | `missing_target` | the resolver demanded an OS-reported active window. |

## Loose, outside this work

`~/scratch-pockettts` is still on disk and **PR #11 on `mycosavant/pocket-tts`
is open**, left for the maintainer per their own instruction. It is a real doc
worth having; it is not a test artifact to clean up without asking.
