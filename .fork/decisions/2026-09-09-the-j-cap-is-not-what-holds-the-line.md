> A decision on record, 2026-09-09. Binding until a later decision names it.

- **`CARGO_BUILD_JOBS=8` is not what stands between a release build and the
  wall, and no value of `-j` is.** The ceiling is a single crate compiling by
  itself. Measured on a corrected sampler, clean build, this date:

  | phase | held |
  |---|---|
  | the `-j 8` parallel front, 7 concurrent compilers | **summed 3,287 MB**, heaviest single 911 MB |
  | the `warp` crate, **alone** | 1.5 GB → **14,975 MB** over four minutes, still climbing when the build was stopped — **read this as 15,809 MB**, see below |
  | lowest `MemAvailable` | **8,198 MB — during the single-crate phase**; ~19,300 MB throughout the parallel one |

  The closest the machine came to the edge was while **exactly one** compiler
  was running. Capping jobs cannot touch that.

  **The two figures in that table are a 10-second sampler's, and both are
  low.** Re-measured later the same day with `/usr/bin/time -v`, which has no
  sampling window: the app crate's true peak is **15,809 MB** and the lowest
  `MemAvailable` on a build with no `rust-analyzer` resident is 22,601 MB. The
  conclusion is unchanged and slightly stronger — the single crate is bigger
  than the sampler said, and the parallel front is not.

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
lever that reaches only it. **Measured and applied the same day**; the four
candidates are no longer candidates.

**`debug = 0` and `codegen-units = 64` take the app crate from 15,809 MB to
11,342 MB — 28.3% — and the build from 378 s to 305 s, for a binary 125 MB
smaller.** Lowest `MemAvailable` moved 22,601 → 27,325 MB. Ten builds,
`.fork/runs/profile-2026-09-09/`.

| lever | Δ peak | verdict |
|---|---|---|
| `debug = 0` | −2,302 MB | **taken** |
| `codegen-units = 64` | −2,933 MB | **taken**; the curve's knee (32 → −2,221, 256 → −3,199, binary grows throughout) |
| both together | **−4,467 MB** | 89% of additive, so the two mechanisms are genuinely different |
| `opt-level = 2` | −567 MB | refused — 12% of the pair's saving for the one cost nobody wants |
| `split-debuginfo = "unpacked"` | −359 MB | refused — moved 45 KB of a 777 MB binary, so close to inert here, and the mechanism was not established |

**Three things the run turned up that outlive the numbers.**

**The instrument was wrong first, again.** The handoff's 15,587 MB baseline and
the clean run's 14,975 MB are the *same build* read by a 10-second sampler that
lands where it lands — adjacent ticks near the peak swing 1.7 GB. The exact
figure comes from `/usr/bin/time -v`, which reports the peak RSS of the
heaviest single descendant with no sampling window, calibrated three levels
deep against a known 700 MB allocation. Two identical baselines then agreed to
**6 MB** and produced byte-identical binaries, which is what makes the small
levers readable at all.

**`debug = 0` costs less than it reads, and that is checkable rather than
arguable.** A *package* override reaches one crate: `readelf` on the result
shows full line tables for every dependency and **zero entries for `app/src`**.
A backtrace loses file/line in the app crate and keeps it everywhere else.

**It cannot be an environment variable, and the failure is silent.**
`CARGO_PROFILE_RELEASE_PACKAGE_warp_DEBUG=0` is ignored with no error — rustc
still gets `-C debuginfo=1`. Only the whole-profile form works, and that
reaches every crate and invalidates the graph. So the setting lives in
`Cargo.toml`, beside the `[profile.dev.package]` block that was already there.

**What is unmeasured**: `codegen-units = 64` costs runtime in the app crate by
an unknown amount. The argument that it is small is upstream's own — its
`[profile.dev.package]` raises `opt-level` for `warp_terminal` and a dozen
others and never for `warp` — and an argument is not a measurement. Remove that
line first if a runtime regression ever appears there; `debug = 0` alone still
holds 2,302 MB of the 4,467.

**And the profile is still not the largest lever available.** The same crate
had 22,795 MB of headroom with no `rust-analyzer` resident and 8,198 MB with
one. Killing an editor's language server is worth 14.6 GB — three times the
whole sweep.

Evidence: `.fork/runs/profile-2026-09-09/` (ten builds, exact peaks) and
`.fork/runs/pricefetch-2026-09-09/memsample-fixed.tsv` (31 samples through both
phases, and the source of the two low figures corrected above). Related: [[the sampler counted rust-analyzer as a
compiler]] — `ps -C rustc` is not an exact match on this procps, and every
number this script produced before this date with an editor open was inflated
by 15-16.5 GB.

---

## Closed 2026-09-11: the cap is lifted, and this file's title was right

The one thing left open here — an uncapped front half — was run at the desk with
the maintainer's authorisation, on a clean `target/release` and a clean host.
`.fork/runs/uncapped-2026-09-11/`.

| | uncapped `-j 32`, clean, 1058 crates |
|---|---|
| wall time | 7m16s |
| peak **summed**, parallel front | **9,229 MB** across 21 compilers |
| peak **single** `rustc` | 12,706 MB exact, the `warp` crate, **alone** |
| `MemAvailable` floor | **25,871 MB** of ~39 GB, with **one** compiler running |

**The whole front, at four times the width, summed to less than one app crate.**
That is this file's title stated as a number rather than an inference.

**The extrapolation that kept this open was wrong in a way worth naming.**
*"Eight jobs averaged ~470 MB each, so thirty-two is ~15 GB"* took the average of
the eight crates that happened to be resident at a sampler tick. Uncapped, 31
compilers summed to 4,893 MB — ~158 MB each. A wider front recruits *smaller*
crates, because the large ones are the graph's tail and compile alone whatever
`-j` says. **A per-job average measured at one width does not scale to another**,
and reasoning from one is the move to distrust.

**One cost, and it points the other way.** Uncapping raises the *single-crate*
peak: 11,342 MB at `-j 8` against 12,706 MB at `-j 32`, same instrument, +12%.
The jobserver bounds `rustc`'s internal codegen-unit parallelism, so a wider `-j`
lets one `rustc` run more of its 64 units at once. So `-j` does reach the app
crate — in the direction opposite to the one the cap was chosen for, and by
1.4 GB against 25.9 GB of headroom. Worth knowing before anyone raises
`codegen-units` again.

**What is untouched, and is now the only rule**: an uncapped WSL build
*concurrent with a Windows one*. That pressure is on the host's 64 GB, one level
above anything the guest can see, and this run was deliberately its opposite.
`.fork/tools/build.sh` is uncapped by default and honours a `CARGO_BUILD_JOBS`
already in the environment, which is the knob for the case that rule cannot
cover — a remote session where a dead VM costs the link. `build.ps1` stays
capped, because this measurement was taken inside the guest.
