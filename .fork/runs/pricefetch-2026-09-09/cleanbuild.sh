#!/usr/bin/env bash
cd /home/effatha/git/warp
out=.fork/runs/pricefetch-2026-09-09
.fork/tools/memsample.sh "$PWD/$out/memsample.tsv" &
sampler=$!
echo "start $(date -u +%Y-%m-%dT%H:%M:%SZ)" > $out/cleanbuild.txt
CARGO_BUILD_JOBS=8 cargo build --release --features gui,warp_control_cli >> $out/cleanbuild.txt 2>&1
echo "exit $? $(date -u +%Y-%m-%dT%H:%M:%SZ)" >> $out/cleanbuild.txt
wait $sampler
