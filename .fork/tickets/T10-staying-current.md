> Ticket T10, split out of `.fork/TASKS.md` on 2026-09-05. History; read the section you came for.

## T10 — Staying current  ← ONGOING

> Decided 2026-08-24: **this stays a soft fork.** The rename is deferred, and
> the reason is measurable rather than aesthetic — renaming every `warp-oss`,
> `WARP_*`, `WarpOss` and `warpctrl` symbol converts ground the fork currently
> *shares* with upstream into permanent conflict. Until then, upstream merges
> are cheap and the only thing that makes them expensive is letting them lapse.
>
> `.fork/archive/CONSOLIDATION.md` §1 records the corrected divergence figures and the
> measurement error that produced the wrong ones.

- [x] **T10.1** Merge `upstream/master` through 2026-08-24 (`6696954c6`).

### T10.1 — as built

**The overlap is the number to watch, not the file count.** The fork touches
204 files; the 110 upstream commits touched 465; they overlap in **39**, and
only **4** of those conflicted. Both figures are cheap to compute, and the first
is the early warning:

```bash
MB=$(git merge-base upstream/master dev)
comm -12 <(git diff --name-only $MB...dev | sort) \
         <(git diff --name-only $MB..upstream/master | sort) | wc -l
```

**The four conflicts, and what each taught.**

* `input_tests.rs` — a fork test and three upstream tests at the same offset.
  Kept both. Purely positional; no judgement needed.
* `user_workspaces/mod.rs` — upstream `8cbb01d45` split the file into a module.
  Took upstream's side and re-applied both account-gate bypasses at their new
  home, where `is_custom_inference_enabled` is now `is_byo_endpoint_enabled`.
  **A rename inside a moved file is the shape most likely to lose a fork edit
  silently**, because the conflict shows the fork's code against *nothing*.
* `warp_agent_page.rs` — upstream's APP-5559 refactor turned a widget `render`
  method into a free function. Took upstream's shape, re-applied
  `fork::is_anonymous_for_ui`.
* `pwsh.ps1` — upstream's lint pass rewrote `(Get-Location).Path` to
  `$PWD.Path` on the two lines T6.2 had replaced with `Warp-Get-Location`.
  **Upstream's new spelling carries the identical UNC bug T6.2 fixed** — both
  are provider-qualified. Kept the fork's. Upstream's three other new
  `$PWD.Path` sites were deliberately left: a cache key, a `Get-Item
  -LiteralPath`, and a `Set-Location` argument, none of which are handed to
  Warp, which is the only place the qualifier hurts.

**The dangerous half was the part no conflict marked.** Git merged three files
cleanly that could not compile, because upstream added code constructing a type
the fork had widened:

* `warp_tui/src/orchestration_model.rs` and `transcript_view.rs` — new upstream
  files with exhaustive matches over `BlocklistAIHistoryEvent`, which T8.3
  extended with `ConversationSettledChanged`.
* `persistence/src/model_tests.rs` — six exhaustive `AgentConversationData`
  literals with no `settled` field.

**That last one was already red before the merge.** T8.3 added a required field
and never compiled `crates/persistence`'s tests; the merge only moved the line
numbers. Same failure mode as the `warpctrl` catalog count pinned in two crates
where only the fast one gets run.

**So the gate after any merge — or any fork change to a shared type — is
`cargo check --workspace --all-targets`.** Not the binary build, which passes
happily: every one of these three lives in test or TUI code the `warp-oss`
target never compiles.

- [x] **T10.3** Merge `upstream/master` through 2026-09-14 (`e3464f102`), as
  `2d7ffdc07`. Record: `.fork/runs/merge-2026-09-14/`.

### T10.3 — as built

72 commits, 47 overlapping files, **zero conflicts, and two compile breaks
anyway** (`0e6d4323c`): a new exhaustive match missing the fork's
`ConversationSettledChanged`, and a `TeamScope` argument added to a function
the fork's `agent.spawn` calls. Same shape as T10.1's dangerous half, so the
gate above held. A third failure was not code: ignored build output kept a
crate directory upstream deleted, and the `crates/*` glob refused the
workspace. Before merging, the drift check learned to say when its count was
unconfirmed (`1409ce080`), and two pins went in so the next merge stops at the
settings-file loopback door or at a new unchecked `reqwest` client
(`9750c9bd0`).

---

