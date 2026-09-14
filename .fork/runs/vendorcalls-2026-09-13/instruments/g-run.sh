#!/usr/bin/env bash
# Configuration G wrapper: the redacting proxy in WSL, the probe and census on Windows.
#   g-run.sh <label> <prompt> [extra g.ps1 switches...]
set -u
S=$(cd "$(dirname "$0")" && pwd)
label=$1; prompt=$2; shift 2
out=$S/runs/$label; rm -rf "$out"; mkdir -p "$out"
win() { echo "\\\\wsl.localhost\\Ubuntu$(echo "$1" | sed 's|/|\\|g')"; }
VC_OUT=$out/requests.jsonl VC_BLOCK=registry.npmjs.org \
VC_CANARIES=CANARY-PROMPT-5d19b2,CANARY-CTX-7f3a91,CANARY-FILE-c24e08 \
VC_CREDS=/mnt/c/Users/onemind/.claude/.credentials.json \
  mitmdump --listen-host 127.0.0.1 --listen-port 8081 --set confdir="$S/mitm" \
  --set flow_detail=0 --set connection_strategy=lazy --set upstream_cert=false -s "$S/redact_addon.py" > "$S/mitm-$label.log" 2>&1 &
mp=$!
for i in $(seq 1 50); do ss -Htln "sport = :8081" | grep -q . && break; sleep 0.2; done
: > "$out/requests.jsonl"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$(win "$S/g.ps1")" \
  -Out "$(win "$out")" -Prompt "$prompt" -CaCert "$(win "$S/mitm/mitmproxy-ca-cert.pem")" "$@"
kill $mp; wait $mp 2>/dev/null
echo "done $label -> $out"
