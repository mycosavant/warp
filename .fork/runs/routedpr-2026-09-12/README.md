# The routed PR paths, driven live

**2026-09-12, 16:57–17:02 UTC.** Closes Task 3 of `.fork/HANDOFF-GHTESTS.md`.
Route 1 as recommended there: a real `gh`, a bare `origin` on disk, and a run
that stops the moment `gh` is invoked. **No pull request was created, and none
could be** — `gh` refuses a repository with no GitHub remote before it opens a
socket, confirmed by hand before the run started.

`35a29d6d6` made the client generate PR title and body on a routed pane. Until
this run that fix had only ever executed in unit tests.

## Configuration

| | |
|---|---|
| GUI | Windows `target\debug\warp-oss.exe`, `v0.fork.7d8e21b76` |
| daemon | Linux `target/release/warp-oss`, `v0.fork.7d8e21b76`, via `~/.warp-dev/remote-server/warp-oss` |
| model | `gemma-4-12b` on `127.0.0.1:8080`, `C:\dev\llama\serve.ps1` |
| profile | `WARP_DATA_PROFILE=localai` — debug binary, because release ignores it and would run on the maintainer's real profile |
| repo | `~/scratch-localai`, branch `pr-probe-20260912`, `origin` = `/home/effatha/scratch-pr-remote.git` (bare, on disk) |

**Both binaries were built for this run, one side at a time** — Linux first
(4m 46s), then Windows (3m 41s). The Windows checkout was two commits behind
and was fast-forwarded first; `.fork/docs/build.md`'s WSL-side fetch command.

The pane was routed and that is the whole premise:

```json
"filesystem": {"where": "host", "host_id": "9fcd8b1e-..."}
```

## What was measured

The instrument is `grep -c launch_slot_ server.err` — one line per request the
model served — sampled before and after each click.

| moment | model requests | evidence |
|---|---|---|
| pane created, `cd`, `git status` | 0 → 4 | ambient, not the PR path |
| code review panel opened | 4 | no request |
| **Create PR** clicked (dialog opens) | 4 | no request — the dialog is built from the daemon's diff summary, `1 file +6 -0` |
| **Create PR confirmed** | **4 → 6** | `pr-4-confirmed.png` |
| commit dialog opened, chain | 8 → 9 | the commit message |
| **Commit and create PR confirmed** | **9 → 11** | `pr-8-chain-confirmed.png` |

The two requests at each confirm are a title and a body, and the server log
says so without being asked — same prompt size, wildly different output
lengths:

```
task 211 | prompt eval 369 tokens | eval  10 tokens     <- title
task 212 | prompt eval 369 tokens | eval 118 tokens     <- body
```

They launched 143 ms apart. A 10-token completion is not a PR body.

## Both paths reached `gh` with content, and stopped there

**Standalone Create PR.** Confirm at 16:58:28, failure at 16:58:34:

```
[ERROR] gh command failed: none of the git remotes configured for this
repository point to a known GitHub host.
```

**The chain.** Confirm at 17:01:18, failure at 17:01:25, and the commit and
push both landed first:

```
3c46a5a Add chain_probe_20260912_165931 function
$ git ls-remote --heads origin
3c46a5a...  refs/heads/pr-probe-20260912
```

This is the falsification criterion the handoff set, and it passes: the failure
names the **GitHub host**, not a missing title. A complaint about an empty
title would have meant content was sent half-specified.

**The dialog resolved rather than hanging** — the panel returned to "No open
changes" with the file badge at 0. That was the specific worry about the chain:
a deferred completion event leaving it open forever. Seven seconds, closed.

## The commit message re-proved itself, unasked

`pr-6-commit-dialog.png` shows the commit box filled with

> Add chain_probe_20260912_165931 function
>
> Include a new probe function for the routed PR drive.

`chain_probe_20260912_165931` was written into `main.rs` at 16:59:31, four
minutes before. The model cannot have recalled it, so the diff travelled from
the daemon to the client and the client called the model — a24's fix,
re-measured on a new pair of binaries with a marker that cannot predate the run.

## A defect found on the way: the chain mislabels its own failure — **fixed 2026-09-12**

Fixed the same day, and it turned out to be two defects rather than one. See
the section as written below for what was observed; what follows is what was
done.

**The stage now travels.** `run_commit_chain` tags each failure with a
`CommitChainStage` — `NotCommitted`, `Committed`, `Pushed` — and the dialog
says which: *"Committed and pushed, but the pull request failed"*, *"Committed,
but the push failed"*, or the cause alone. Three states rather than a
`committed` flag, because a **push** failure is the case a flag gets wrong in
the other direction: the commit is in the repository, and "Commit failed" would
have the user remake it and be answered *"nothing to commit"*. The daemon still
reports a chain failure as a bare string, so a failure inside the distribution
arrives with no stage and is reported exactly as before — carrying it across
means widening the proto.

**The second defect was in the toast and is worse than the label.**
`user_facing_git_error` matched on the substring `gh auth login` and answered
*"GitHub CLI not authenticated."* — but `gh`'s message for a repository with no
GitHub remote *ends by suggesting that command*. So anyone whose remote is
GitLab, or self-hosted, or in this run a bare repo on disk, was told to
authenticate, which cannot help. A new arm above it answers *"No GitHub remote
for this repository."*

Calibrated by breaking each fix and predicting the direction first. One break
reddened **nothing** — the `Committed` stage had no test through the chain at
all — and that gap is now closed by
`a_chain_whose_push_fails_says_the_commit_was_made`, which fails when the break
is re-applied.


```
[ERROR] Commit failed: gh command failed: ... known GitHub host
```

The commit did not fail. It was made, and pushed, and both survive in the repo;
only the third step failed. A user reading *"Commit failed"* would go looking
for a commit that is already there — and on a real GitHub repo the same wrapper
would report a PR-creation failure as a commit failure. Not fixed here.

## Not established

- **No real pull request was created, so nothing proves `gh` accepts the
  generated title.** The argument list is pinned by the `pr_create_args` unit
  tests; the wire between them and a live GitHub repo is untested and needs the
  maintainer's say-so.
- **The title and body text was never read.** `run_gh_command`'s argv logging
  is `log::debug!` and this profile logs at INFO, so the evidence for content
  is the two model requests and their shapes, not the strings themselves.
- **Version skew is still unexercised** — both halves were deliberately the
  same commit, as in the commit-message run.
- **The truncation budget is still untried.** These diffs were six lines;
  `MAX_DIFF_CHARS_FOR_AI` was never approached.
- Only one repository, one branch, one file.
