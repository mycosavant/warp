#!/usr/bin/env bash
# One configuration of the vendor-calls census.
#   run.sh <label> <driver: probe|multi|webfetch> [VAR=value ...]
# Env for the agent is the baseline (A) plus the assignments given. A value of
# UNSET removes a baseline variable.
set -u
S=$(cd "$(dirname "$0")" && pwd)
label=$1; driver=$2; shift 2
out=$S/runs/$label; rm -rf "$out"; mkdir -p "$out"
confdir=$S/mitm
PORT=8081
NODE=/home/effatha/.nvm/versions/node/v24.5.0/bin/node
CACHE=/home/effatha/.npm/_npx/6130df88bef2cca3/node_modules
ACP=$CACHE/@agentclientprotocol/claude-agent-acp/dist/index.js
CLAUDE_BIN=$CACHE/@anthropic-ai/claude-agent-sdk-linux-x64/claude
WARP=/home/effatha/git/warp/target/release/warp-oss
CWD=${VC_CWD:-$S/ws}
PROMPT=${VC_PROMPT:-"CANARY-PROMPT-5d19b2. In one sentence, what is 2+2?"}
now() { date -u +%Y-%m-%dT%H:%M:%S.%3NZ; }
tl() { echo "$1 $(now)${2:+ $2}" >> "$out/timeline.txt"; }

sha256sum "$NODE" "$ACP" "$CLAUDE_BIN" > "$out/sha256.txt"

# Baseline env (configuration A).
declare -A ENVV=(
  [HTTPS_PROXY]=http://127.0.0.1:$PORT [HTTP_PROXY]=http://127.0.0.1:$PORT
  [NO_PROXY]=127.0.0.1,localhost [NODE_EXTRA_CA_CERTS]=$confdir/mitmproxy-ca-cert.pem
  [ANTHROPIC_BASE_URL]=http://127.0.0.1:8080 [ANTHROPIC_API_KEY]=local
  [ANTHROPIC_MODEL]=gemma-4-12b [CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC]=1
  [CLAUDE_CODE_EXECUTABLE]=$CLAUDE_BIN
)
for kv in "$@"; do ENVV[${kv%%=*}]=${kv#*=}; done
envargs=(); : > "$out/env.txt"
for k in $(printf '%s\n' "${!ENVV[@]}" | sort); do
  [ "${ENVV[$k]}" = UNSET ] && { echo "$k (unset)" >> "$out/env.txt"; continue; }
  envargs+=("$k=${ENVV[$k]}"); echo "$k=${ENVV[$k]}" >> "$out/env.txt"
done

# Proxy, redacting addon only, no flow file.
VC_OUT=$out/requests.jsonl VC_CANARIES=CANARY-PROMPT-5d19b2,CANARY-CTX-7f3a91,CANARY-FILE-c24e08 \
VC_CREDS=/home/effatha/.claude/.credentials.json \
  mitmdump --listen-host 127.0.0.1 --listen-port $PORT --set confdir="$confdir" \
  --set flow_detail=0 --set connection_strategy=lazy --set upstream_cert=false -s "$S/redact_addon.py" > "$S/mitm-$label.log" 2>&1 &
mp=$!
for i in $(seq 1 50); do ss -Htln "sport = :$PORT" | grep -q . && break; sleep 0.2; done
: > "$out/requests.jsonl"

# Census.
stop=$out/stop; /home/effatha/git/warp/.fork/tools/egress-poll-wsl.sh "$out/sockets.tsv" "$stop" 0.2 &
poller=$!
sleep 1
tl CENSUS_START

# Control 1a: a request through the proxy must appear in requests.jsonl.
curl -s -o /dev/null --cacert "$confdir/mitmproxy-ca-cert.pem" -x http://127.0.0.1:$PORT https://example.com/ ; tl CONTROL_1A

cmd=${VC_CMD:-"$NODE $ACP"}
tl DRIVER_START "$driver"
case $driver in
  probe|webfetch)
    extra=(); [ $driver = webfetch ] && extra=(--approve)
    ( cd "$CWD" && exec env "${envargs[@]}" "$WARP" --warpctrl acp probe --command "$cmd" \
        --prompt "$PROMPT" --cwd "$CWD" --output-format ndjson "${extra[@]}" ) \
      2> "$out/probe.err" | python3 -u "$S/ts.py" > "$out/probe.ndjson" &
    ;;
  multi)
    ( cd "$CWD" && exec env "${envargs[@]}" python3 -u "$S/multiturn.py" "$cmd" "$CWD" 3 ) \
      2> "$out/probe.err" | python3 -u "$S/ts.py" > "$out/probe.ndjson" &
    ;;
esac
dpid=$!
python3 "$S/proctree.py" $$ "$out/procs.tsv" "$stop" &
treep=$!
wait $dpid
tl DRIVER_END "exit=$?"
sleep 3

# Control 2: a non-loopback connection with no proxy must appear in the census.
env -u HTTPS_PROXY -u HTTP_PROXY "$NODE" -e 'require("https").get("https://example.com",r=>{r.resume();r.on("end",()=>setTimeout(()=>process.exit(0),800))}).on("error",e=>{console.error(e.message);process.exit(0)})'
tl CONTROL_2
sleep 1.5
touch "$stop"; wait $poller $treep 2>/dev/null
kill $mp; wait $mp 2>/dev/null
tl CENSUS_STOP
echo "done $label -> $out"
