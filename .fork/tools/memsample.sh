#!/usr/bin/env bash
#
# Sample rustc memory through a build. Exits when the last cargo/rustc does.
#
#   .fork/tools/memsample.sh /tmp/build.tsv &
#   CARGO_BUILD_JOBS=8 cargo build --release --features gui,warp_control_cli
#
# One line per 10s:
#   epoch  n_rustc  sum_rss_mb  max_rss_mb  max_rss_crate  mem_avail_mb  swap_used_mb
#
# Read `sum_rss_mb` against `mem_avail_mb`, never `max_rss_mb` alone. The
# quantity that takes the VM down is the SUM across every rustc; `max_rss_mb`
# names which crate is heavy and says nothing about headroom. `MemAvailable`
# answers "how close did we get" but moves with everything else on the machine,
# so only the sum says whether `-j` was the thing holding the line.
#
# Measured with it on 2026-09-04: peak 15.1 GB on the `warp` crate, which
# compiles ALONE for most of a warm build -- so during that stretch `-j` has
# nothing to cap. See CLAUDE.md, "Cap the release build".
#
# **Read every number this script produced before 2026-09-09 with the bug below
# in mind.** Until that date it selected processes with `ps -C rustc`, which on
# this procps also matches `rust-analyzer` -- a different program, no part of
# the build, and one that holds 16.5 GB open over this workspace. Any sample
# taken while an editor was running counted it as a compiler, inflating both
# `n_rustc` and the RSS columns.
#
# The tell was in the output the whole time and was missed for twenty minutes:
# rust-analyzer has no `--crate-name`, so it reports its crate as `?`. **A row
# whose `max_crate` is `?` is not describing a compiler.**
out="$1"
# Sample interval in seconds. The default 10 under-reads a jagged peak: two
# runs of the same app-crate compile, 372s and 373s, reported 15,587 MB and
# 14,978 MB purely from where the ticks landed (2026-09-09). For anything
# comparing one build against another, set this to 2 and read the exact
# single-process peak from `/usr/bin/time -v` instead.
interval="${MEMSAMPLE_INTERVAL:-10}"
echo -e "epoch\tn_rustc\tsum_rss_mb\tmax_rss_mb\tmax_crate\tavail_mb\tswap_used_mb" > "$out"
while pgrep -x cargo >/dev/null || pgrep -x rustc >/dev/null; do
  n=$(pgrep -x rustc | wc -l)
  # `sum_rss_mb` was added 2026-09-09, for the clean-build test of `-j 8`.
  # This script's own header already said to read `avail_mb` rather than
  # `max_rss_mb` "because the quantity that takes the VM down is the SUM across
  # every rustc, and this records the largest single one". That was a correct
  # warning attached to a missing column: `avail_mb` answers "how close did we
  # get" and cannot say whether `-j` was the thing holding the line, because it
  # moves with the page cache and everything else on the machine. The sum can.
  # `-C rustc` is NOT an exact match on this procps: it also selects
  # **rust-analyzer**, measured 2026-09-09, and a rust-analyzer over this
  # workspace holds 16.5 GB. So the pids come from `pgrep -x rustc`, which is
  # exact, and `ps -p` reads only those. See the header.
  pids=$(pgrep -x rustc | paste -sd, -)
  line=$(ps -o rss=,args= -p "${pids:-0}" 2>/dev/null \
    | awk '{rss=$1; crate="?"; s+=rss;
            for(i=1;i<=NF;i++){ if($i=="--crate-name"){crate=$(i+1)} }
            if(rss>m){m=rss; c=crate} } END{printf "%d\t%d\t%s", s/1024, m/1024, (c==""?"-":c)}')
  [[ -z "$line" ]] && line=$'0\t0\t-'
  avail=$(awk '/MemAvailable/{print int($2/1024)}' /proc/meminfo)
  swap=$(awk '/SwapTotal/{t=$2}/SwapFree/{f=$2}END{print int((t-f)/1024)}' /proc/meminfo)
  printf '%s\t%s\t%s\t%s\t%s\n' "$(date +%s)" "$n" "$line" "$avail" "$swap" >> "$out"
  sleep "$interval"
done
