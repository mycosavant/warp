# The uncapped clean build, run 2026-09-11

Open since 2026-08-29, re-derived three times, never run. Run at the desk with
the maintainer's explicit authorisation, which the standing `CARGO_BUILD_JOBS=8`
constraint required.

## What was run

`cargo clean --release` (42.7 GiB removed), then
`/usr/bin/time -v .fork/tools/build.sh -j 32` — the trailing `-j` overrides the
script's own `export CARGO_BUILD_JOBS=8` on the cargo command line while keeping
the version sidecar. 32 = `nproc`, so this is cargo's uncapped default.

Sampler: `.fork/tools/memsample.sh` at `MEMSAMPLE_INTERVAL=2`, post-dating the
`pgrep -x rustc` fix, so no `rust-analyzer` is counted as a compiler.

Preconditions, all three met and all three read rather than assumed:

- **Host** 24.6 GB of 63.8 at start. It had been at 44.1 twenty minutes earlier,
  held by `vmmemWSL` after the session's own debug builds; `cargo clean` plus
  ~8 minutes of waiting returned it. The guest said 37 GB available throughout,
  which is the reading that cannot see the hazard.
- **No `rust-analyzer`**, no `rustc`, no Warp, no `llama-server`.
- **Nothing building on the Windows side.**

## The result

| | |
|---|---|
| crates compiled | 1058 |
| wall time | **7m16s** (7m44.9s including the script) |
| max concurrent `rustc` | **32** |
| peak **summed** RSS, parallel front | **9,229 MB** at n=21 |
| peak **single** `rustc` | **12,528 MB** sampled, **12,706 MB** exact |
| `MemAvailable` floor | **25,871 MB**, of ~39 GB |
| host peak | 43.6 GB of 63.8 |
| exit | 0, binary stamped `v0.fork.e77c69c88-dirty`, `warpctrl` present |

**The cap was never what held the line.** The entire parallel front — every one
of the 32 jobs — summed to less than the `warp` crate compiling by itself, and
the closest the machine came to the wall was during the single-crate phase,
which `-j` cannot touch. That is the same shape the 2026-09-09 clean build
found at `-j 8`, now confirmed at four times the width.

The old extrapolation was wrong, and it is worth saying how: *"eight jobs
averaged ~470 MB each, so thirty-two is ~15 GB"* took the average of the eight
crates that happened to be resident when a 10-second sampler ticked. Uncapped,
31 concurrent compilers summed to 4,893 MB thirty seconds in — ~158 MB each.
A wider front recruits *smaller* crates, because the big ones are the graph's
tail and compile alone whatever `-j` says.

## The one thing that cuts the other way

**Uncapping raises the single-crate peak.** 11,342 MB at `-j 8` (2026-09-09,
after `[profile.release.package.warp]` landed) against **12,706 MB** at `-j 32`,
same instrument, **+12%**. The jobserver bounds `rustc`'s *internal* codegen-unit
parallelism, so a wider `-j` lets one `rustc` run more of its 64 units at once.

So `-j` does reach the app crate. It reaches it in the direction opposite to the
one the cap was chosen for, and by 1.4 GB against 25.9 GB of headroom.

## What this does not establish

There is **no clean `-j 8` build to compare 7m16s against**, so the speed gain
is unmeasured. The comparison that would settle it costs another clean build.

And it says nothing about the case that actually took the guest down in
2026-08-29: an uncapped WSL build **concurrent with a Windows build**. The
pressure there is one level up, on the host's 64 GB, and this run was
deliberately the opposite — a clean host with nothing else on it. **That hazard
is untouched by this measurement**, and "am I building on both sides at once?"
remains the question that matters more than `-j`.
