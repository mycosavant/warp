# The routed commit message, measured

*2026-09-12.* The live check that `036bcccac` shipped without. It asked one
question: **does the commit dialog in a routed WSL pane now fill in?**

**Yes.** And the answer is not a screenshot of a plausible sentence — the
message names a function that did not exist anywhere before this run.

## What was driven

The recommended configuration, and the same rig as
`.fork/runs/localmodel-2026-09-07/` rather than a new one: Windows GUI, WSL
pane auto-routed at shell bootstrap, `llama-server` b10844 with Gemma 4 12B at
`127.0.0.1:8080`, and the `localai` scratch profile, which declares that server
as a key-less Custom Inference endpoint. The **debug** binary, because
`WARP_DATA_PROFILE` is honoured by debug builds only and a release build would
have run on the maintainer's own profile.

Both halves at the same commit, which matters because they are separate
binaries on separate filesystems:

| | binary | version |
|---|---|---|
| GUI | `C:\dev\warp\target\debug\warp-oss.exe` | `v0.fork.036bcccac` |
| daemon | `~/.warp-dev/remote-server/warp-oss` → Linux `target/release` | `v0.fork.036bcccac` |

Built one side at a time, Linux first (4m31s) and Windows second (4m42s),
because the one surviving build rule is never both at once.

## The probe, and why it is shaped like that

A generated commit message is easy to believe and hard to check: a model that
received nothing could still emit something that reads correctly for a
repository called `scratch-localai`. So the diff was made **undateable from
anything but itself** — the 2026-09-07 working state was committed away, and
one function added:

```rust
/// Probe for the routed commit-message path: this function did not exist
/// before 2026-09-12, so a generated message that names it cannot have come
/// from anywhere but this diff.
fn routed_commit_probe(bitrate: u32) -> u32 {
    bitrate.saturating_mul(7).saturating_sub(13)
}
```

Same discipline as the LSP check in `CLAUDE.md`: make the right answer one a
fabrication cannot reach.

## What happened

| step | routed | evidence |
|---|---|---|
| pane in `~/scratch-localai` | `session inspect` → `"where": "host"` | all six sessions `host`; the driven pane inspected by id |
| code review panel | header *"WSL: Ubuntu:/home/effatha/scratch-localai: main"*, the diff rendered | `routed-2-code-review.png` |
| **Commit clicked** | **the dialog filled in** | `routed-4-dialog.png` |
| model requests | **3 → 4** | one request at the moment the dialog opened |

The message:

> **Add routed_commit_probe function**
>
> Introduce a new function to probe the routed commit-message path, including a
> timestamped doc comment for provenance tracking.

It names the function and describes the doc comment. Neither existed before
this run, so the diff reached the model, which means it reached the GUI, which
means the daemon returned it.

Per-request, from `server-timings.log`: **296 prompt tokens, 32 out, 710 ms** —
the same shape the *unrouted* path produced on 2026-09-07 (296 in, 31 out,
0.6 s), which is the corroboration worth having: the routed path now sends what
the working path always sent.

And the failure that defined the defect is gone. `Failed to autogenerate commit
message` and `No AI endpoint is configured` each appear **0 times** in
`warp-oss.log`. That zero is readable only because the positive evidence above
proves the path ran — an unattempted generation would leave the same zero.

## The control, which is the half that could have been missed

`036bcccac` refactored `git_actions::generate_commit_message` into two
functions so the daemon and the client could each run half. That refactor is on
the **local** path too, so the fix could have repaired the routed case and
broken the case that already worked.

Re-run with `WARP_FORK_WSL_AUTO_CONNECT=0`, everything else identical: all
seven sessions `"where": "local"`, the panel header changes to the 9p UNC path
`\\wsl$\ubuntu\home\effatha\scratch-localai`, and **the same message arrives**
(`unrouted-2-dialog.png`), requests 4 → 6. Nothing regressed.

## What this did not establish

- **One model, one diff, one repository.** A seven-line diff in a two-file
  repo. Nothing here says anything about a large diff meeting the daemon's
  truncation budget, which `get_diff_for_commit_message` applies on the daemon
  side and the client now never recomputes.
- **The PR-content paths were not driven, and they are still broken the same
  way.** `GitCreatePrRequest.autogenerate_content` and
  `GitCommitChainRequest.autogenerate_pr_content` still generate on the daemon
  with the daemon's client. They fall back to `gh pr create --fill`, so they
  produce a plausible PR rather than an empty box — which is the likeliest
  reason nobody filed them.
- **Version skew was not exercised.** Both halves were the same commit on
  purpose. The claim that an older daemon degrades to upstream behaviour rests
  on prost skipping an unknown field, which is read, not measured.
- **The debug binary is not the binary anyone lives in.** `WARP_DATA_PROFILE`
  forced that choice, and it is the same choice the 2026-09-07 run made.

## Files

| | |
|---|---|
| `run.sh` | the driver: launch, routed check, panel, commit click, capture |
| `driver.log` | every step with timestamps, including the `ambiguous_target` below |
| `routed-1-pane.png` … `routed-4-dialog.png` | the routed run |
| `unrouted-1-code-review.png`, `unrouted-2-dialog.png` | the control |
| `warp-oss.log` | the GUI log, carrying zero of the 2026-09-07 error |
| `server-timings.log` | the model's own per-request accounting |

## One instrument note

`warpctrl session inspect` with no argument answered `ambiguous_target`: this
profile restored six tabs and **every one of them reports `is_active: true`**,
so resolution by active session has nothing to disambiguate on. `session list`
answers fine and `session inspect --session <id>` answers fine. Worth knowing
before reading a bare `inspect` failure as a routing problem — it is a
targeting problem, and the routing underneath it was correct all along.
