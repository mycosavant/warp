# Version skew: an old daemon against a new GUI, driven live

Closes `.fork/HANDOFF-GHTESTS.md` Task 4's *"Version skew is unexercised"* —
the claim that an older remote-server daemon degrades to upstream behaviour
rested on "prost skips an unknown field," read, never measured.

## Setup

- **Old daemon**: `60ac539bc4d4b338f16178524f0bd86d45a166e4`, the direct parent
  of `036bcccac` (the commit that added `return_diff_only`/field 4 to
  `GitGenerateCommitMessageRequest` and `diff`/field 3 to the response oneof).
  Confirmed by reading the proto at both commits: a clean field append, no
  renumbering — the "prost skips it" premise is architecturally sound, not a
  coin flip.
- Built in an isolated `git worktree` (own `target/`, never touches the main
  tree's build or `CARGO_TARGET_DIR`), stamped with a version sidecar
  (`v0.fork.60ac539bc`) so the client's log names it precisely instead of
  `<unknown>`.
- **New GUI**: the Windows debug binary already on disk, `v0.fork.7d8e21b76`
  — three commits behind current HEAD but well after `036bcccac`, so it
  already sends `return_diff_only`. (Incidentally: running `session inspect`
  against this GUI hit the exact `ambiguous_target` bug fixed earlier this
  session in `03bf98fd0` — this binary predates that fix. Worked around with
  an explicit `--tab` id; independent confirmation the bug and fix are real.)
- The only lever for pointing a WSL pane at a specific daemon build is the
  symlink `~/.warp-dev/remote-server/warp-oss` — no env override exists.
  Confirmed clean before starting: no daemon/proxy running, symlink pointing
  at the main tree's binary. Symlink swapped to the old worktree binary,
  swapped back immediately after, old daemon's specific PIDs killed by PID
  (not by name), state re-verified byte-for-byte against the pre-test
  `readlink`. Full sequence in this session's transcript; nothing left
  pointing at the old binary or running from it afterward.
- `localai` scratch profile reused (`WARP_DATA_PROFILE=localai`, **debug**
  GUI binary — `WARP_DATA_PROFILE` is honoured by debug builds only; a
  release GUI would have silently run on the maintainer's real profile).
  `~/scratch-localai` reused with a fresh dateable marker
  (`version_skew_probe_20260912_200901`), not the existing PR-drive markers.

## The drive

Routed pane confirmed (`session inspect --tab 2852` → `"where": "host"`),
`git status --short` run inside it, code review panel opened, Commit dialog
opened on the staged change.

## Result — matches every prediction

- **Commit dialog**: *"Failed to autogenerate commit message: No AI endpoint
  is configured..."* — the identical message routed panes showed before
  `036bcccac` existed. The old daemon decoded the request fine (field 4
  silently skipped), never saw `return_diff_only`, fell back to generating
  the message itself, and failed for the mundane reason that the WSL distro
  has no AI endpoint installed.
- **Dialog resolved, did not hang** — screenshot confirms an idle dialog
  (placeholder text, both buttons live), not a stuck spinner.
- **Log**: `[WARN] [remote_server::manager] Remote server version differs
  from the client: ... client=Some("v0.fork.7d8e21b76")
  server="v0.fork.60ac539bc"` — fired exactly as predicted, informational
  only. Traced separately: `version_is_compatible` is hardcoded `true` for
  `Channel::Oss` (no pinned artifact to reinstall there), so this WARN never
  escalates to a teardown or a UI banner on this channel — it's purely a log
  line, independent of the specific field-compatibility behaviour being
  tested.
- No decode error, no panic, no `HostDisconnected`, no daemon exit. The
  connection carried the `git status` and code-review-panel traffic
  immediately before and after the mismatched request with no issue.

## What this does not establish

- Only the commit-message path was driven. The PR-content path
  (`GitCreatePrRequest`/`GitCommitChainRequest`) has the identical shape of
  fix and the identical `Message`/`Diff`/`Err`-independent client handling,
  but was not separately exercised against an old daemon.
- Only one skew distance (one commit before the field was added) was tested.
  An arbitrarily old daemon missing several since-added fields was not tried,
  though the mechanism (unknown fields are just skipped) doesn't predict a
  qualitative difference from testing more of them.
- The observed "correctly degrades" outcome depends on the client tolerating
  `Err` gracefully, which was true before this test (routed panes have always
  shown this exact message pre-fix) — this run confirms the *skew* case
  reaches that same well-trodden path, not that the path itself is new.

## Cleanup, verified

Windows instance closed and confirmed gone (`instance list` → `[]`). Symlink
restored, confirmed by `readlink -f` matching the pre-test value exactly. Old
daemon's two PIDs killed by number; `ps aux` confirms no process referencing
the worktree path remains. Worktree removed (`git worktree remove --force`)
now its one job is done. No Windows-side `warp-oss`/`llama-server` processes
remain.
