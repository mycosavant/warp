#!/usr/bin/env bash
# The WSL half of the Windows egress run: every socket held inside the
# distribution by anything Warp started there -- the remote-development daemon
# with its terminal-server and proxy (WARP_FORK_WSL_AUTO_CONNECT), the ACP agent
# (`claude-agent-acp` under node), and a language server. Same loop as the
# Linux measurement in `.fork/tickets/open-questions.md`; `state all` includes
# SYN-SENT, so an attempted connection appears even if it is refused.
#
#   .fork/tools/egress-poll-wsl.sh <out.tsv> <stopfile> [interval_s]
#
# The agent's own API traffic is expected here and is the control: a poller
# that never shows `api.anthropic.com` during a turn was not looking.
out=$1; stop=$2; interval=${3:-0.2}
pattern='warp-oss|remote-server|terminal-server|claude|node|npx|rust-analyzer'
samples=0
: > "$out.raw"
echo "# started $(date -u +%Y-%m-%dT%H:%M:%S.%3NZ) interval ${interval}s pattern $pattern" > "$out"
while [ ! -f "$stop" ]; do
  samples=$((samples+1))
  ts=$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)
  { ss -Htnp state all 2>/dev/null | awk -v OFS='\t' '{print "tcp",$0}';
    ss -Hunp 2>/dev/null        | awk -v OFS='\t' '{print "udp",$0}'; } |
    grep -E "$pattern" | awk -v ts="$ts" -v n="$samples" -v OFS='\t' '{print ts,$0,n}' >> "$out.raw"
  if (( samples % 150 == 0 )); then
    echo "# heartbeat $ts sample $samples tcp_all $(ss -Htn state all 2>/dev/null | wc -l)" >> "$out"
  fi
  sleep "$interval"
done
echo "# stopped $(date -u +%Y-%m-%dT%H:%M:%S.%3NZ) samples $samples" >> "$out"
printf '# first_seen\tproto\tstate\tlocal\tremote\tusers\tsample\n' >> "$out"
# Distinct tuples in first-seen order; the timestamp and sample are those of the first sighting.
awk -F'\t' '{ k=$2"|"$3"|"$4"|"$5"|"$6; if (!(k in s)) { s[k]=1; print } }' "$out.raw" >> "$out"
