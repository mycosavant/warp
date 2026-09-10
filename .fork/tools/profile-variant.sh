#!/usr/bin/env bash
#
# One variant of the `[profile.release.package.warp]` sweep.
# Written for .fork/runs/profile-2026-09-09; kept because the shape is reusable.
#
#   echo '<toml block>' | .fork/tools/profile-variant.sh <name> <outdir>
#
# The block is spliced into Cargo.toml between two markers, so a variant is one
# heredoc and never a hand edit. **The markers are not in Cargo.toml** -- the
# 2026-09-09 sweep removed them when it applied its answer, so before reusing
# this, put these two lines back where the variant block should go:
#
#     # >>> profile-sweep
#     # <<< profile-sweep
#
# The script asserts they are there and stops if they are not, which is the
# behaviour you want: a splice that silently no-ops gives you a clean build,
# no error, and a variant identical to the baseline.
#
# TWO instruments, because the first one is not good enough on its own:
#
#   /usr/bin/time -v   the EXACT peak RSS of the heaviest single descendant
#                      process, from getrusage(RUSAGE_CHILDREN), which the
#                      kernel propagates transitively (calibrated three levels
#                      deep against a known 700 MB allocation). No sampling
#                      window, so no miss. This is the number to compare.
#
#   memsample.sh @2s   the SUM across compilers and the MemAvailable trace,
#                      which a per-process maximum cannot give. At the default
#                      10s it under-read this build's peak by 609 MB -- larger
#                      than some of the levers being measured.
#
# Waits on the cargo PID, never on `pgrep -f` -- a waiter whose own command
# line contains the pattern matches itself and never fires (CLAUDE.md,
# 2026-08-30, walked into three times since).
set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
name="$1"; out="$2"
block=$(cat)

python3 - "$block" <<'PY'
import re, sys
block = sys.argv[1]
src = open('Cargo.toml').read()
new = re.sub(r'(# >>> profile-sweep\n).*?(# <<< profile-sweep\n)',
             lambda m: m.group(1) + (block + '\n' if block.strip() else '') + m.group(2),
             src, flags=re.S)
assert '# >>> profile-sweep' in src, 'markers not found'
open('Cargo.toml','w').write(new)
PY

touch app/src/lib.rs
start=$(date +%s)
CARGO_BUILD_JOBS=8 /usr/bin/time -v cargo build --bin warp-oss --release \
  --features gui,warp_control_cli > "$out/$name.log" 2>&1 &
cargo_pid=$!
# The sampler's loop exits the moment no cargo/rustc is visible, so it must not
# start before cargo has exec'd.
until ps -p "$cargo_pid" >/dev/null 2>&1 && pgrep -x cargo >/dev/null; do sleep 0.2; done
MEMSAMPLE_INTERVAL=2 .fork/tools/memsample.sh "$out/$name.tsv" &
sampler=$!
wait "$cargo_pid"; rc=$?
end=$(date +%s)
wait "$sampler" 2>/dev/null

exact=$(awk -F': ' '/Maximum resident set size/{printf "%d", $2/1024}' "$out/$name.log")
size=$(stat -c%s target/release/warp-oss 2>/dev/null || echo 0)
crates=$(grep -c 'Compiling' "$out/$name.log")
sampled=$(awk 'NR>1{if($4>m){m=$4;c=$5}}END{print m"\t"c}' "$out/$name.tsv")
lowavail=$(awk 'NR>1{if(n==0||$6<a){a=$6};n++}END{print a}' "$out/$name.tsv")
printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
  "$name" "$rc" "$((end-start))" "${exact:-0}" "$sampled" "$lowavail" "$size" "$crates" \
  >> "$out/summary.tsv"
echo "done $name rc=$rc $((end-start))s exact_peak=${exact:-?}MB sampled=$sampled avail_low=$lowavail crates=$crates bin=$size"
