#!/usr/bin/env bash
# rc.sh -- the console reached over the tailnet, 2026-09-08.
#
# Runs from WSL against the Windows release build. The phone's half is a
# person: the script prints the pairing QR in a pane on the monitor and then
# waits for the record to show a prompt that came from the phone. Copy the
# shape of .fork/runs/remote-control-2026-09-05/rc2a.sh.
set -u
EXE=/mnt/c/dev/warp/target/release/warp-oss.exe
CURL=/mnt/c/Windows/System32/curl.exe
LAUNCHER='\\wsl.localhost\Ubuntu\home\effatha\git\warp\.fork\tools\warpdev.ps1'
TS='C:\Program Files\Tailscale\tailscale.exe'
STATE=/mnt/c/Users/onemind/AppData/Local/warp
CTL() { "$EXE" --warpctrl "$@" --output-format json 2>&1; }
OUT=${1:?outdir}; LOG="$OUT/driver.log"
log() { echo "[$(date -u +%H:%M:%S)] $*" | tee -a "$LOG"; }
json() { python3 -c "import sys,json;d=json.load(sys.stdin);print($1)"; }

log "binary: $(ls -la --time-style=+%F_%T "$EXE" | awk '{print $6}') version $(cat "${EXE%.exe}.version" 2>/dev/null || echo none); tree $(git -C /mnt/c/dev/warp log --oneline -1)"
log "tailnet: $(powershell.exe -NoProfile -Command "& '$TS' ip -4" | tr -d '\r')"
if CTL instance list | grep -q '"instance_id"'; then log "an instance is already running; stop it first (window close)"; exit 2; fi

log "launch: product profile + console, bind tailnet"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$LAUNCHER" -Console > "$OUT/launch.txt" 2>&1 &
for i in $(seq 1 40); do sleep 3; CTL instance list | grep -q '"instance_id"' && break; done
CTL instance list > "$OUT/list.json"; log "instance: $(json 'len(d.get("instances",[]))' < "$OUT/list.json") record(s)"
grep -E 'warpdev: (binary|profile)|CONTROL_BIND|predates' "$OUT/launch.txt" | tee -a "$LOG"

log "the origin the fork publishes for a phone"
CTL pair show > "$OUT/pair-show-watch.json"
URL=$(json 'd["url"]' < "$OUT/pair-show-watch.json"); ORIGIN=${URL#https://}; ORIGIN=${ORIGIN%%/*}
log "  url origin: $ORIGIN (expect 100.82.213.46:41234)"

log "reachability from the Windows side on the tailnet address (curl.exe is Schannel: -k for the private CA)"
"$CURL" -k -sS -o /dev/null -m 8 -w '  https / -> %{http_code} %{ssl_verify_result} tls=%{tls_version}\n' "https://$ORIGIN/" 2>&1 | tee -a "$LOG"
"$CURL" -sS -m 8 -o "$OUT/ca.crt" "http://$ORIGIN/ca.crt" && log "  ca.crt sha256 $(sha256sum "$OUT/ca.crt" | cut -c1-16) vs state $(sha256sum "$STATE/WarpOss/data/fork/console-ca.crt" | cut -c1-16)"

log "cd the pane, one text-only conversation"
CTL input submit 'cd /home/effatha/git/warp' >/dev/null; sleep 4
CTL agent prompt 'Say the single word ready and nothing else.' > "$OUT/prompt-c.json"
C=$(json 'd["conversation_id"]' < "$OUT/prompt-c.json"); echo "$C" > "$OUT/conversation-c.txt"; log "conversation C $C"
for i in $(seq 1 40); do sleep 3; CTL agent list | python3 -c "import sys,json; d=json.load(sys.stdin); c=[c for c in d['conversations'] if c['conversation_id']=='$C']; sys.exit(0 if c and not c[0]['is_busy'] else 1)" && break; done

log "the QR on the monitor: the pane runs pair show for C"
CTL input submit "$EXE --warpctrl pair show --conversation $C" >/dev/null
log "  scan it with the phone on cellular, then send a prompt from the page"

log "waiting up to 15 min for the record to show the phone"
EV=$(ls "$STATE"/WarpOss*/data/fork/events/"$C".jsonl 2>/dev/null | head -1)
for i in $(seq 1 90); do
  sleep 10
  EV=${EV:-$(ls "$STATE"/WarpOss*/data/fork/events/"$C".jsonl 2>/dev/null | head -1)}
  EV=${EV:-$(grep -l "$C" "$STATE"/WarpOss*/data/fork/events/*.jsonl 2>/dev/null | head -1)}
  [ -n "$EV" ] && grep -q 'paired_device' "$EV" && break
done
[ -n "$EV" ] && cp "$EV" "$OUT/events-c.jsonl"
log "  phone rows: $(grep -c paired_device "$OUT/events-c.jsonl" 2>/dev/null || echo 0)"
grep paired_device "$OUT/events-c.jsonl" 2>/dev/null | python3 -c "
import sys,json
for l in sys.stdin:
    e=json.loads(l); print('   ', e.get('ts','')[11:19], e.get('event'), e.get('via') or e.get('answered_by') or '')" | tee -a "$LOG"
CTL agent read "$C" > "$OUT/read-c.json"
log "done; the block state and the phone's screenshots are the person's"
