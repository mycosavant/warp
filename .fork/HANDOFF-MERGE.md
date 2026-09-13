# Handoff: the drift check, then the upstream merge

Written 2026-09-13 by the session that ranked the open backlog after voice
closed. **The maintainer approved the merge and the drift-check correction the
same evening.** This is the first of three handoffs written together; the
others are `HANDOFF-HYGIENE.md` (docs, runs beside this one) and
`HANDOFF-CORRECTNESS.md` / `HANDOFF-BUILDS.md` (start after this merge lands,
because the merge may move the code they touch). Retire this file to
`archive/` when its last task is done.

## Read these first

1. `CLAUDE.md`: *Method: run it* (the `git diff A...B` base trap and the
   fourteenth stale-doc row, the validator a merge broke), and *Working rules*
   (merging upstream: watch the overlap; `--workspace --all-targets`; diff
   failure membership, not counts).
2. `.fork/tickets/T10-staying-current.md` and the last merge's run record, if
   any is cited there. The 2026-09-04 merge is `db8e43786`; read its body.
3. `.fork/docs/manual.md`, *Watching upstream drift, without merging it*.

## State at writing

| | |
|---|---|
| `dev` | `9d878a1ec`, clean, pushed |
| local `upstream/master` | `3959ea721`, fetched 2026-09-13 17:37 by this session |
| merge base | `5a6ded1e8` (the 2026-09-04 tip), computed |
| unmerged upstream | **63 commits**, committer dates 09-04 to 09-12 |
| files both sides changed | **44** (39 at the 09-04 merge, of which 4 conflicted) |
| Linux release binary | `v0.fork.7d8e21b76`, stale |
| Windows release binary | `v0.fork.ac5f6b25f`, the maintainer rebuilt it 2026-09-13 |

Recompute every number above before using it. They are as of the fetch.

---

## Task 1: the drift check reported a false zero

**What was measured.** `drift-check.sh` has run once, 2026-09-10 18:32, by a
session running the cron line by hand (`7a5a3028e`). It reported
`0 upstream commits`. The `upstream/master` reflog has **no entry that day**
(09-04, then 09-13), while by committer date at least 38 upstream commits had
landed on master by the end of 09-10. So its fetch did not update the ref, and
the report was computed against a six-day-old ref with nothing on the page to
say so.

**What is not established: why.** The script's fetch sends stderr to
`/dev/null` and prints its own warning on failure, and the cron line appends
`2>&1` to the log. The log holds no warning. So either the fetch exited 0
without fetching, or the run that produced that row was not the line as
written. The likeliest suspect is that the 09-10 run happened inside a Claude
Code sandbox with network restricted, but that is a guess. Reproduce before you
explain it.

**The fix is to make the fetch's outcome part of the report, not to guess the
cause.**

- After fetching, compare the local ref to `git ls-remote <remote>
  refs/heads/<branch>`. If they differ, say so in the report as the first
  line, and write a status column to `drift.tsv` (keep the file readable by
  the existing row; add the column at the end).
- Stop discarding the fetch's stderr. Put it in the report.
- A report computed against refs that could not be confirmed current must not
  print a bare `0`. Print that the count is unconfirmed.
- Calibrate by breaking it: run with the remote URL pointed at an unreachable
  host (use `WARP_FORK_UPSTREAM` or a scratch clone, never by editing the
  real remote) and watch the report say the count is unconfirmed.

**Cron.** This session's permission layer refuses `crontab`, so do not
conclude anything from not being able to see it (friction a21 is exactly that
mistake). The line was installed on 2026-09-10 and is `0 9 * * 1`, so its
**first real run is Monday 2026-09-14 09:00**. The maintainer offered to run
commands on their side: ask them to `tail ~/.local/state/warp-fork/drift.log`
after that, and compare against `git ls-remote`. A `/loop` is not a substitute,
because it lives only as long as a session.

Commit: `fork: the drift check says when its count could not be confirmed (T10)`.

## Task 2: the validator door, before the merge

The 2026-09-04 merge broke local Custom Inference endpoints through a validator
that 64 tests never met. Before merging again, confirm both of upstream's doors
are pinned for a loopback URL:

- **direct:** `crates/ai/src/api_keys_tests.rs:301` asserts a local URL is
  accepted. Read it; check which hosts it covers (`127.0.0.1`, `localhost`,
  `[::1]`, a LAN address).
- **settings-file load:** `api_keys.rs:206` filters endpoints through the
  validator on load. Look for a test that loads a `settings.toml`-shaped
  endpoint with a loopback URL and asserts it survives. The ranking agent
  reported none; it was wrong about the first door, so check.

Add only the missing one, calibrated by breaking the validator's loopback arm.
If both exist, write nothing and say so in the merge record. After the merge
these tests are the first gate to read.

## Task 3: the merge

Approved by the maintainer on 2026-09-13. The rules:

1. `git fetch upstream`, then compute: `mb=$(git merge-base upstream/master
   dev)`. Never paste a base.
2. List the overlap before merging: `comm -12 <(git diff --name-only $mb dev |
   sort) <(git diff --name-only $mb upstream/master | sort)`. Save it in the
   run record. For each file in it that is a fork seam (`fork.rs`, anything
   under `egress_policy`, `http_client`, `websocket`, `local_control`,
   `acp_agent`, `local_agent`, `api_keys.rs`, the settings definitions, the
   remote-server proto), read upstream's side of the diff before resolving.
3. `git merge upstream/master` on `dev`, a merge commit as before.
4. **Resolve at the conflict with the judgment the drift script refuses to
   automate.** The 09-04 merge had one conflict settled by noticing an import
   was dead in the *merged* body, and one break in no conflict at all.
5. **Look for the fork claims that depend on upstream not doing something**,
   since a clean merge can still break them. At minimum:
   - new outbound request paths in `http_client`, `websocket`, or a new
     dialler dependency (the websocket test that pins "no other crate takes a
     dialler" will catch the last one);
   - new `FeatureFlag`s or channel-list entries that turn on anything
     account- or server-backed (`fork::FORCE_DISABLED` is where to answer);
   - settings that moved definitions (the 09-07 lesson: the reader looked in
     the pre-merge vector);
   - new `warpctrl` actions changing the catalog count (update both pins, never
     loosen);
   - telemetry or autoupdate touching `app_version()`.

### Gates, in this order

- `cargo check --workspace --all-targets`
- the Task 2 validator tests, the egress policy tests, both catalog pins,
  `local_sync` byte-stability tests
- `-p warp --lib` **twice**, membership diffed against
  `.fork/runs/lib-baseline-2026-09-12/`
- `./script/format` then `git status` (revert what you did not mean to touch)
- `.fork/tools/claude-md-budget.sh`, `node --check app/src/local_control/console.js`
- `script/presubmit`

### After the gates

- Rebuild **one side at a time**, never both at once (host memory): Linux
  release via `.fork/tools/build.sh`, then sync `/mnt/c/dev/warp` and ask the
  maintainer to run `C:\dev\build.ps1 -Release`, or run it yourself if they
  say so. Verify by the version sidecar and `--version`, not the build script's
  word.
- **Refresh the WSL remote-server daemon** afterwards (the symlink points into
  `target/release`); `warpctrl session inspect` in a WSL pane should answer
  `host`.
- Live smoke on Windows with a scratch profile (`WARP_DATA_PROFILE`, debug
  builds only; release uses the maintainer's profile, so ask first): an ACP
  panel turn with `claude-agent-acp@0.73.0` and `WARP_FORK_ACP_MODE=default`,
  the read-aloud palette entry, and `warpctrl instance list`. Stop with
  `warpctrl window close`.
- Record in `.fork/runs/merge-<date>/`: the overlap list, each conflict and
  how it was decided, gate results, what was checked in step 5 and what was
  not. Update T10.
- Push `dev` when the gates are green.

## Not in this handoff

The CORRECTNESS and BUILDS work, and any product change a merged upstream
commit suggests. File those as friction or tickets; do not fold them into the
merge commit.
