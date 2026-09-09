#!/usr/bin/env bash
# One `acp probe` turn against the local model, with a socket census around it.
#   optout-probe.sh <label> <outdir> [extra env assignments...]
set -u
label=$1; out=$2; shift 2
mkdir -p "$out"
stop="$out/$label.stop"; rm -f "$stop"
tsv="$out/$label-sockets.tsv"

# Poller: every socket of anything matching the pattern, with the pid, so the
# driving `claude` session can be told from the agent it spawns.
( : > "$tsv"
  while [ ! -f "$stop" ]; do
    ts=$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)
    ss -Htnp state all 2>/dev/null |
      grep -E 'claude|node|npx' |
      awk -v ts="$ts" -v OFS='\t' '{print ts,$0}' >> "$tsv"
    sleep 0.2
  done ) &
poller=$!

echo "# poller $poller started $(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)" | tee "$out/$label-timeline.txt"
sleep 1

env "$@" \
  ANTHROPIC_BASE_URL=http://127.0.0.1:8080 \
  ANTHROPIC_API_KEY=local \
  ANTHROPIC_MODEL=gemma-4-12b \
  /home/effatha/git/warp/target/release/warp-oss --warpctrl acp probe \
    --command 'npx -y @agentclientprotocol/claude-agent-acp@0.73.0' \
    --prompt 'In one sentence, what is 2+2?' \
    --cwd /home/effatha/git/warp \
    --output-format ndjson > "$out/$label-probe.ndjson" 2> "$out/$label-probe.err"
echo "# probe exited $? $(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)" | tee -a "$out/$label-timeline.txt"

# Positive control: the poller must be able to see a non-loopback connection.
node -e 'require("https").get("https://example.com",r=>{r.resume();r.on("end",()=>process.exit(0))}).on("error",()=>process.exit(0))'
echo "# control done $(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)" | tee -a "$out/$label-timeline.txt"
sleep 2
touch "$stop"; wait $poller 2>/dev/null
echo "# poller stopped, $(wc -l < "$tsv") samples" | tee -a "$out/$label-timeline.txt"
