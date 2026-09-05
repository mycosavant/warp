#!/usr/bin/env bash
#
# Sample rustc memory through a build. Exits when the last cargo/rustc does.
#
#   .fork/tools/memsample.sh /tmp/build.tsv &
#   CARGO_BUILD_JOBS=8 cargo build --release --features gui,warp_control_cli
#
# One line per 10s:
#   epoch  n_rustc  max_rss_mb  max_rss_crate  mem_avail_mb  swap_used_mb
#
# Read `mem_avail_mb`, not `max_rss_mb`. The quantity that takes the VM down is
# the SUM across every rustc, and this records the largest single one -- useful
# for naming which crate is heavy, useless for headroom. `MemAvailable` is the
# column that answers "how close did we get", and it is here deliberately after
# the first version of this script nearly answered with the wrong one.
#
# Measured with it on 2026-09-04: peak 15.1 GB on the `warp` crate, which
# compiles ALONE for most of a warm build -- so during that stretch `-j` has
# nothing to cap. See CLAUDE.md, "Cap the release build".
out="$1"
echo -e "epoch\tn_rustc\tmax_rss_mb\tmax_crate\tavail_mb\tswap_used_mb" > "$out"
while pgrep -x cargo >/dev/null || pgrep -x rustc >/dev/null; do
  n=$(pgrep -x rustc | wc -l)
  line=$(ps -eo rss=,args= -C rustc 2>/dev/null \
    | awk '{rss=$1; crate="?";
            for(i=1;i<=NF;i++){ if($i=="--crate-name"){crate=$(i+1)} }
            if(rss>m){m=rss; c=crate} } END{printf "%d\t%s", m/1024, (c==""?"-":c)}')
  [[ -z "$line" ]] && line=$'0\t-'
  avail=$(awk '/MemAvailable/{print int($2/1024)}' /proc/meminfo)
  swap=$(awk '/SwapTotal/{t=$2}/SwapFree/{f=$2}END{print int((t-f)/1024)}' /proc/meminfo)
  printf '%s\t%s\t%s\t%s\t%s\n' "$(date +%s)" "$n" "$line" "$avail" "$swap" >> "$out"
  sleep 10
done
