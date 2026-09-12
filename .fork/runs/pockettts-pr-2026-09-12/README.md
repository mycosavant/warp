# The real PR drive, against a real GitHub repo, and the bug it found

Closes the last unverified piece of the routed commit/PR work
(`.fork/runs/routedpr-2026-09-12/` proved the chain against a local bare
remote deliberately, to avoid opening a real PR without sign-off; that
sign-off was given 2026-09-12 for this run specifically).

## Setup

- Target repo: `mycosavant/pocket-tts` (the maintainer's own, chosen over a
  friend's third-party repo and over the fork's own weekly drift-check
  report, to keep the test self-contained and low-stakes).
- Content: the published "Eleven Utterances" artifact, converted from its
  HTML/CSS artifact form to plain Markdown (appropriate for a doc committed
  into a code repo, not the artifact's own presentation layer), placed at
  `android/docs/eleven-utterances.md` alongside the repo's existing docs of
  the same shape.
- Rig: the same `localai` scratch profile, Windows debug GUI
  (`v0.fork.7d8e21b76`), llama-server on `127.0.0.1:8080`, WSL-routed pane,
  reused from `.fork/runs/routedcommit-2026-09-12/` and
  `.fork/runs/routedpr-2026-09-12/`. Shallow-cloned pocket-tts to
  `~/scratch-pockettts` rather than the maintainer's own working copy.

## What happened, in order

1. Staged the new file, opened the commit dialog. The local model generated
   a real commit message (`docs: add eleven utterances test results and
   analysis`, three-line body). Confirmed "Commit and create PR".
2. Commit succeeded. PR creation failed:
   `Commit failed: gh command failed: aborted: you must first push the
   current branch to a remote, or use the --head flag`.
3. **Diagnosed rather than assumed.** The obvious read — "the chain never
   pushed" — is wrong, and assuming it would have wasted the rest of this
   run. `git ls-remote` against the real repo showed the branch already
   present on GitHub at the exact local commit sha, on both attempts. The
   push succeeded. `git branch -vv` showed why `gh` still refused: no local
   upstream tracking (`branch.docs/eleven-utterances.remote`/`.merge`) was
   ever set, because the chain's push doesn't pass `-u`/`--set-upstream`.
   `gh pr create` without an explicit `--head` relies on that tracking
   metadata to know what branch it's looking at; without it, a branch that
   is genuinely on the remote with the right content is invisible to it.
4. **Confirmed by fixing it the narrow way**: `gh pr create --head
   docs/eleven-utterances` (bypassing the auto-detection entirely) succeeded
   immediately, first try, no other change needed. That is the whole bug —
   one missing flag on the push, one missing flag on the create.
5. Repeated the chain a second time (a small genuine addition — a
   provenance note — staged, committed, "Commit and create PR" again) to
   rule out "only fails on a brand-new branch": **identical failure**, even
   though the branch now had commits already on the remote from the first
   attempt. This rules out "never-pushed-before" as the condition; it fails
   on every attempt, tracked or not, because the tracking metadata is what's
   missing, not the remote content.

## The PR

**https://github.com/mycosavant/pocket-tts/pull/11** — open, not merged,
left for the maintainer to review or close per their own instruction.

**Its body is not what the fork generated for it.** The two model calls for
PR title/body generation did fire each time (confirmed by llama-server's
own request log: two requests ~230ms apart following the single
commit-message request, matching the shape `.fork/runs/routedpr-2026-09-12/`
recorded for a title+body pair) — but that generated text was never
displayed or persisted anywhere before the `gh` failure discarded it, and
there is no way to recover it after the fact. The PR was opened manually
(`gh pr create --head`, title and body assembled from the two real,
AI-generated commit messages) once the diagnosis was confirmed, so the
maintainer would have something to review rather than nothing. **The
generation mechanism is verified working** (real model calls, real commit
messages landed); **the last mile — that exact generated text reaching a
real PR unattended — is not**, and can't be, until the bug below is fixed.

## The bug, precisely

`create_pr`'s push step (or whatever calls it) does not set upstream
tracking on push, and the `gh pr create` call it makes afterward does not
pass `--head <branch>`. Both are real, both landed, and `gh` still refuses
because it has no way to know the current branch is the one just pushed.
Two independent one-line fixes converge on the same result; either is
sufficient. Not yet filed as a CLAUDE.md rule or fixed in code — this run
record is the finding, left for a deliberate fix rather than one bolted on
mid-verification-run.

## What this does not establish

- Whether this reproduces identically for `Commit and push` (a separate,
  simpler action) or only for the chained `Commit and create PR` /
  `Commit chain` path — not tested here, since the chain is what failed.
- Whether the Windows-side `gh` (the one the daemon actually shells out to)
  has any config difference from the `gh` used here to diagnose and fix it
  manually (same machine, same WSL distribution, same `gh auth`, so
  unlikely, but not independently confirmed).
- The exact PR title/body the model would have produced. Gone with the
  failed attempt; only the shape (two requests, similar cadence to the
  known-good routedpr-2026-09-12 run) is known.

## Cleanup, verified

Windows GUI instance closed and confirmed gone. llama-server stopped (it
was started for this run). WSL-side `remote-server-daemon` and
`terminal-server` processes killed by PID after confirming their identity
key matched this run's scratch profile. `~/scratch-pockettts` left on disk
(a normal shallow clone, not the maintainer's real working copy of
pocket-tts) — the PR itself is the artifact that matters; the clone is
disposable.
