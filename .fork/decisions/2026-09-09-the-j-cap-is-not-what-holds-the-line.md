> A decision on record, 2026-09-09. Binding until a later decision names it.

- **`CARGO_BUILD_JOBS=8` is not what stands between a release build and the
  wall, and no value of `-j` is.** The ceiling is a single crate compiling by
  itself. Measured on a corrected sampler, clean build, this date:

  | phase | held |
  |---|---|
  | the `-j 8` parallel front, 7 concurrent compilers | **summed 3,287 MB**, heaviest single 911 MB |
  | the `warp` crate, **alone** | 1.5 GB → **14,975 MB** over four minutes, still climbing when the build was stopped |
  | lowest `MemAvailable` | **8,198 MB — during the single-crate phase**; ~19,300 MB throughout the parallel one |

  The closest the machine came to the edge was while **exactly one** compiler
  was running. Capping jobs cannot touch that.

**This is the third time the `-j` question has been argued, so here is what is
settled and what is not, separately.**

**Settled.** The parallel front of this build is cheap — eight jobs averaging
~470 MB. `-j 8` is not buying safety there, and the app crate's ~15 GB is real
and independently confirmed (`CLAUDE.md`'s 15-17 GB figure survives; the doubt
raised against it on this date was withdrawn within the hour, because a
rust-analyzer cannot supply a crate name and that sample said `warp`).

**Not settled, and it is the only part left.** An **uncapped** front half. Eight
jobs averaged ~470 MB here; what thirty-two of them do together is the question
the cap was actually chosen for in 2026-08-29, and it has still never been run.
Until it is, keep the cap — not because the measurement supports it, but
because nothing has replaced the crash that motivated it.

**Keep `-j 8` and stop re-deriving why.** It costs nothing on a build whose
front half is small, and the one scenario that could justify removing it is
unmeasured. The next person to reopen this should be running the uncapped
build, not re-reading the numbers.

**What to attack instead, if the goal is a build that fits.** The target is the
`warp` crate's own 15 GB, and `[profile.release.package.warp]` is the stable
lever that reaches only it — `debug = 0`, `opt-level`, `codegen-units`,
`split-debuginfo`. None is measured yet. `.fork/HANDOFF-PROFILE.md` is that run
and `.fork/next.html` item 7b is the board entry.

Evidence: `.fork/runs/pricefetch-2026-09-09/memsample-fixed.tsv`, 31 samples
through both phases. Related: [[the sampler counted rust-analyzer as a
compiler]] — `ps -C rustc` is not an exact match on this procps, and every
number this script produced before this date with an editor open was inflated
by 15-16.5 GB.
