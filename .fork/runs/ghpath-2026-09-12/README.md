# Setting `PATH` on a child does not choose the child

**2026-09-12.** Closes Task 1 of `.fork/HANDOFF-GHTESTS.md`, and answers the
question that handoff left open about `run_git_command`.

## What was measured

`app/src/util/git.rs::run_gh_command` did:

```rust
let mut cmd = Command::new("gh");
if let Some(path_env) = path_env { cmd.env("PATH", path_env); }
```

Program resolution happens against the **parent** process's `PATH`. Setting
`PATH` in the child's environment changes what the child sees once it is
running; it does not change which file is executed.

Baseline on this machine, at HEAD `11a450c7d`, membership diffed rather than
counted:

```
cargo test -p warp --lib util::git
test result: FAILED. 18 passed; 6 failed
```

Five of the six write a fake `gh` into a temp directory, put that directory at
the front of the `path_env` string, and assert on what the fake answers. The
failures name the *real* `gh`: `gh command failed: no git remotes found`, which
is gh 2.100.0's wording for a repo with no remote — the fake says nothing of
the kind. That is the proof, not the reasoning.

**Why these tests ever passed.** The fakes imitate gh's own error text, and one
of them imitates the wording an older gh used for the same condition. The real
binary was producing an answer close enough to the fake's that four of the five
assertions held. A test that passes for a reason unrelated to its subject, and
fails only when the environment shifts under it — here, a gh upgrade.

## The fix

`warp_util::path::resolve_executable_in_path` already existed: it walks a
`PATH` string, checks the executable bit, and on Windows tries `PATHEXT`
extensions. `app/src/terminal/local_tty/docker_sandbox.rs` already used it for
exactly this purpose. `run_gh_command` now resolves `gh` against `path_env` and
falls back to the bare name, so a caller passing an incomplete `PATH` keeps the
old behaviour and `is_gh_missing_error` still sees the same `ENOENT` text.

Result: 23 passed, 1 failed — and the one is unrelated (below).

## Calibrated by breaking it, twice

Both breaks asserted their own pattern matched before writing, so a
non-matching patch could not print `ok` and mean nothing.

| break | predicted | observed |
|---|---|---|
| the repo-view fake answers `CALIBRATION/CALIBRATION` | only `get_repository_info_reads_gh_repo_view` reddens | exactly that, 23 ok / 1 FAILED |
| all five fakes `exit 77` with an unrecognised error | all five redden, nothing else | exactly that, 19 ok / 5 FAILED |

The narrow break is the one that means something about the fix; the broad one
establishes that each of the five is executing its own fake.

## What this cost upstream, and what it did not

`specs/APP-4188/TECH.md` is the spec that added `path_env`. It is explicit
about the goal:

> On Finder/Dock launches, Warp inherits launchd's minimal `PATH`. `git` itself
> still works (Apple ships `/usr/bin/git`), but hooks `git` invokes — notably
> LFS `pre-push` → `git-lfs` — fail. `gh` also fails outside Homebrew-default
> layouts.

and step 3 removed a hardcoded `/opt/homebrew/bin` prefix on the grounds that
"callers that need `gh` findable now pass a captured interactive PATH." They
did not become findable. The spec's own testing note says why nothing caught
it: *"No unit tests — the plumbing is a thin `Option<&str>` forward and the
real-world condition (launchd minimal `PATH`) isn't reproducible in a test
harness."* The one condition that would have falsified the change was declared
untestable, and the tests written later appeared to cover it.

**The `git` half is fine, and that is not a guess.** `path_env` on
`run_git_command_with_env` exists for *hooks*, which are grandchildren spawned
by `git` and do inherit the child's environment. Measured directly:

```
$ env PATH="$FAKEBIN:$PATH" git commit -q --allow-empty -m one
HOOK FOUND only-here
$ git commit -q --allow-empty -m two
.git/hooks/pre-commit: 2: only-here: not found
```

So the same mechanism has opposite consequences on either side of the same
spec: `gh`'s `path_env` had to resolve the child itself and could not; git's
only has to reach the child's children and does.

## Two places that still have it, unfixed and unmeasured

Found by sweeping `.env("PATH"` across the workspace. Both are upstream.

- **`crates/node_runtime/src/lib.rs:453`** states the mechanism *correctly* —
  "`CreateProcessW` uses the parent process's PATH, not the child's
  `lpEnvironment` PATH" — and fixes only Windows, by going through `cmd.exe /c`.
  The `#[cfg(not(windows))]` branch is `Command::new("node").env("PATH", …)`,
  which has the same defect the comment describes. The comment frames a general
  rule as a Windows quirk.
- **`crates/lsp/src/command_builder.rs:81`** has the identical platform split,
  and its call sites pass bare names (`"gopls"`, `"npm"`,
  `"pyright-langserver"`). APP-4188 cited LSP as the repo's existing idiom for
  this; on Unix the idiom does not resolve against the captured `PATH` either.

**Not established:** that either one fails for a real user. Both work whenever
the binary is also on the inherited `PATH`, which is the common case on Linux.
Neither was changed here — that is a wider upstream change than this task, and
it is the maintainer's call.

## The sixth failure, diagnosed rather than assumed

`detached_tag_display_returns_short_sha` had nothing to do with `gh`. The
maintainer's global git config sets `tag.gpgsign = true`, so the test's
`git tag v1.0` becomes an annotated tag, fails non-interactively with
`fatal: no tag message?`, and the tag is never created. The `git checkout v1.0`
that follows therefore never detaches, and the function correctly reports the
branch it is still on.

```
$ git tag v1.0
fatal: no tag message?
$ git rev-parse --abbrev-ref HEAD
main
```

With `tag.gpgsign false` set locally, the same sequence gives `HEAD` and a
short sha. So the product is right and the test was not hermetic against the
developer's own config. Both `init_repo` helpers now set `commit.gpgsign` and
`tag.gpgsign` to false.

## Not established

- No production run. The `gh` path was not driven through the GUI or the
  routed daemon; the evidence is the test binary executing a fake it could not
  previously reach.
- The macOS launchd case is reasoned from the spec's own description, not
  reproduced — there is no Mac here.
- Windows `PATHEXT` resolution comes from `resolve_executable_in_path`'s
  existing tests, not from a Windows run of `run_gh_command`.
