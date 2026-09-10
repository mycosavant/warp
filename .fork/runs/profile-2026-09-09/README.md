# Bounding the `warp` crate, 2026-09-09

Run from `.fork/HANDOFF-PROFILE.md`, answering the maintainer's question:
*"Is there a way to bound the compilation for just the one, most demanding
crate?"*

**Yes. `[profile.release.package.warp]` with `debug = 0` and
`codegen-units = 64` takes the peak from 15,809 MB to 11,342 MB — 28.3% off —
and the build from 378 s to 305 s.** Applied, with the argument beside it in
`Cargo.toml`.

Ten builds, about an hour, all with the dependency graph warm so each one is a
single app-crate compile.

---

## 1. The first thing measured was the instrument, and it was wrong

The handoff said the baseline was 15,587 MB and *"do not re-measure it"*. A
control was run anyway, same source, same day, and read **14,978 MB** — 609 MB
lower. Both runs took 372–373 s and had the same shape, so the build had not
changed; the sampler had landed differently.

The trace says why. Adjacent 10-second ticks near the peak:

```
11,123 → 10,775 → 11,023 → 12,130 → 10,458 → 6,572
```

Swings of 1.7 GB between samples. **A 10-second sampler under-reads this peak
by roughly the size of the effects the run exists to measure**, which would
have made every small lever uninterpretable and the two large ones look
imprecise.

### The fix is exact rather than finer

`/usr/bin/time -v` reports `Maximum resident set size` from
`getrusage(RUSAGE_CHILDREN)`, which is the peak RSS of the **heaviest single
descendant process** — no sampling window, so no miss. The kernel propagates it
transitively, which was not assumed: calibrated against a program allocating a
known 700 MB, run one, two and three levels deep, reporting 718,180 /
718,180 / 717,924 KB.

`memsample.sh` still runs beside it at a 2-second interval, because a
per-process maximum cannot give the **sum** across compilers or the
`MemAvailable` trace. It now takes `MEMSAMPLE_INTERVAL`.

### And then the noise floor, which is what makes the small results readable

Two identical baseline builds:

| | exact peak | build | binary |
|---|---|---|---|
| `baseline` | 15,809 MB | 378 s | 777,693,216 B |
| `baseline2` | 15,803 MB | 374 s | 777,693,216 B |

**6 MB apart, 0.04%, and byte-identical binaries.** So every number below is
outside the noise, including the two that were nearly dismissed as inside it.

**The true baseline is 15,809 MB.** The recorded 15,587 and 14,975 were the
same build under a coarser instrument. `CLAUDE.md`'s older 8.1 GB and the
unverified 13.7 GB are superseded.

---

## 2. Results

All against `baseline`. One app-crate compile each; `crates` was 1 for every
row, and `peak_crate` was `warp` for every row — never `?`.

| variant | exact peak | Δ | Δ% | build | binary |
|---|---|---|---|---|---|
| `baseline` | 15,809 MB | — | — | 378 s | 777.7 MB |
| `baseline2` | 15,803 MB | −6 | −0.04% | 374 s | 777.7 MB |
| `split-debuginfo = "unpacked"` | 15,450 MB | −359 | −2.3% | 388 s | 777.6 MB |
| `opt-level = 2` | 15,242 MB | −567 | −3.6% | 387 s | 766.6 MB |
| `codegen-units = 32` | 13,588 MB | −2,221 | −14.0% | 375 s | 784.7 MB |
| `debug = 0` | 13,507 MB | −2,302 | −14.6% | 343 s | 644.7 MB |
| `codegen-units = 64` | 12,876 MB | −2,933 | −18.6% | 342 s | 792.5 MB |
| `codegen-units = 256` | 12,610 MB | −3,199 | −20.2% | 365 s | 811.1 MB |
| `debug = 0` + `cgu = 32` | 11,760 MB | −4,049 | −25.6% | 308 s | 648.3 MB |
| **`debug = 0` + `cgu = 64`** | **11,342 MB** | **−4,467** | **−28.3%** | **305 s** | **652.4 MB** |

Lowest `MemAvailable` moved with it: 22,601 MB at baseline, **27,325 MB** on
the recommended pair. 4.7 GB more headroom on the phase that was already the
closest this machine gets to the wall.

### The two mechanisms are different, which is why they stack

Perfect addition would be −4,523; the pair delivers −4,467, or 89% of it. DWARF
size and LLVM module size are separate costs, so buying both is worth it.

### The codegen-units curve has a knee at 64

| units | saving | binary |
|---|---|---|
| 16 (default) | — | 777.7 MB |
| 32 | −2,221 MB | 784.7 MB |
| 64 | −2,933 MB | 792.5 MB |
| 256 | −3,199 MB | 811.1 MB |

Savings flatten while the binary grows monotonically. 64 buys 712 MB over 32
for 8 MB of binary; 256 buys 266 MB more for another 19 MB. 64 is the buy.

---

## 3. The two levers that were rejected, and why the numbers are still worth having

**`opt-level = 2`: −567 MB, and it costs runtime in the app crate.** Real —
outside the 6 MB floor — but it is 12% of what the recommended pair delivers in
exchange for the one cost nobody wants. Refused.

**`split-debuginfo = "unpacked"`: −359 MB, and the binary moved 45 KB.** If the
DWARF had genuinely moved out of the link the binary would have collapsed, not
shrunk by 0.006%. The debug sections are all still in it. So on this target the
key is close to inert, the 359 MB is real but unexplained, and **the mechanism
was not established** — recorded that way rather than given a story. The
current `"packed"` is a macOS setting and stays one.

---

## 4. What `debug = 0` actually costs, measured rather than asserted

`[profile.release]`'s own comment justifies keeping line tables to *"symbolicate
panics and Sentry stack traces"*. **This fork force-disables Sentry**
(`FORCE_DISABLED` in `app/src/fork.rs`), so half that justification does not
apply here. The other half is local panic backtraces, and the question is how
much of one is lost.

A **package** override reaches only this crate, so the answer is checkable.
`readelf --debug-dump=rawline` on the resulting binary, counting source paths
in the line tables:

```
1295  crates/core_arch/src
 527  crates/warpui_core/src
 198  crates/core_simd/src
 118  crates/ai/src
 113  crates/warp_core/src
 100  crates/warp_completer/src
  99  crates/warp_terminal/src
  89  crates/warpui/src
   0  app/src
```

**Every dependency keeps full line tables; `app/src` has none.** So a backtrace
loses file/line for frames in the app crate and keeps it everywhere else. That
is a narrower cost than "no panic backtraces", and it buys 2,302 MB and 133 MB
of binary.

---

## 5. What is unmeasured, said plainly

**`codegen-units = 64` costs runtime in the app crate and that was not
measured.** Fewer cross-unit inlining opportunities is the mechanism; the size
of the effect is unknown.

The argument that it is cheap is that the hot code is not in this crate, and it
is upstream's own argument: `[profile.dev.package]` raises `opt-level` for
`warp_terminal`, `ttf-parser`, `strsim`, `memchr` and a dozen others, and
**never for `warp`**, which is wiring. Every one of those crates keeps
`codegen-units = 16`, because the override reaches one package.

That is an argument, not a measurement. **If a runtime regression ever appears
in the app crate, the `codegen-units` line is the first thing to remove** —
dropping it alone still leaves 2,302 MB of the 4,467.

---

## 6. It cannot be delivered by an environment variable, and the failure is silent

Worth knowing, because the obvious way to avoid editing a shared upstream file
is to put the setting in `build.sh` instead. Calibrated on a scratch crate:

| | rustc got | cargo said |
|---|---|---|
| nothing | `-C debuginfo=1` | `[optimized + debuginfo]` |
| `CARGO_PROFILE_RELEASE_PACKAGE_pkgprof_DEBUG=0` | `-C debuginfo=1` | `[optimized + debuginfo]` |
| `CARGO_PROFILE_RELEASE_DEBUG=0` | *(flag absent)* | `[optimized]` |

**The per-package form is ignored with no error and no warning.** The
whole-profile form works and is the wrong tool: it reaches every crate in the
graph and so invalidates all of it, meaning a toggle costs a full rebuild
rather than one crate.

So the lever has to live in a tracked TOML file, and `Cargo.toml` is the
consistent home — `[profile.dev.package]` is already there.

---

## 7. The largest lever is still not in the profile

From the previous run and unchanged by this one: the same crate bottomed out at
`MemAvailable` **22,795 MB** with no `rust-analyzer` resident and **8,198 MB**
with one. **14.6 GB, free, from killing an editor's language server** —
three times what the whole profile sweep bought.

Every build in this run was taken with no `rust-analyzer` running, checked
before each round. Report any profile saving against that or it will look more
impressive than it is.

---

## 8. Method

```bash
echo '<toml block>' | .fork/tools/profile-variant.sh <name> .fork/runs/profile-2026-09-09
```

Splices the block into `Cargo.toml` between two markers, touches
`app/src/lib.rs`, runs `CARGO_BUILD_JOBS=8 /usr/bin/time -v cargo build --bin
warp-oss --release --features gui,warp_control_cli`, and samples beside it at
2 s. It waits on the **cargo pid**, never on `pgrep -f` — a waiter whose own
command line contains the pattern matches itself and never fires.

Between rounds, the host's free memory was read from
`Get-CimInstance Win32_OperatingSystem` and the sweep set to stop below 8 GB.
It never fired; the low point was 13.4 GB free. `free -m` inside the guest read
26–38 GB available throughout and would have been useless as a guard.

Nothing was cleaned. `target/release` held 1,124 rlibs at the start and the
same at the end, which is what made ten builds cost an hour instead of five.

## Files

| file | what |
|---|---|
| `summary.tsv` | one row per variant: exact peak, sampled peak, build time, binary size |
| `<variant>.tsv` | the 2 s sampler trace for that variant |
| `<variant>.log` | cargo output plus the `/usr/bin/time -v` block |
| `baseline-10s.tsv`, `baseline-10s.log`, `summary-10s-sampler.tsv` | the discarded first baseline, kept because it is the evidence for §1 |
