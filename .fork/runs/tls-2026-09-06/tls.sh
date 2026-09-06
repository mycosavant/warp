#!/bin/bash
# Step 2 of .fork/HANDOFF-MOBILE.md, measured: the wide listener speaks TLS
# with a fork-minted authority, the same port hands the authority out in the
# clear and nothing else, and the 09-05 flow (pair, credential, prompt) runs
# over https://. The phone's side is curl.exe; the desk's side is shot.ps1;
# Brave in a scratch profile stands in for a phone that has not installed the
# authority (--ignore-certificate-errors) for the page screenshots only. The
# measurement that counts, a phone with the authority installed and no warning
# page, needs a person with a phone; README.md says what that person does.
set -u
EXE=/mnt/c/dev/warp/target/release/warp-oss.exe
CURL=/mnt/c/Windows/System32/curl.exe
BRAVE='C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe'
LOG_WIN=/mnt/c/Users/onemind/AppData/Local/warp/WarpOss/data/logs/warp-oss.log   # WarpOss, not Warp: the first two passes read a stale file and logged "0 listener lines"
BIND=${BIND:-192.168.254.3:41234}
CTL() { "$EXE" --warpctrl "$@" --output-format json 2>&1; }
OUT=${1:?outdir}; LOG="$OUT/driver.log"
log() { echo "[$(date -u +%H:%M:%S)] $*" | tee -a "$LOG"; }
uuid() { cat /proc/sys/kernel/random/uuid; }
SHOT() { powershell.exe -NoProfile -File 'C:\dev\shot.ps1' -Process "${2:-warp-oss}" -Out 'C:\dev\shots\'"$1"'.png' 2>&1 | tail -1; cp "/mnt/c/dev/shots/$1.png" "$OUT/$1.png"; }
# curl.exe is Schannel, and Schannel refuses a chain whose leaf names no CRL
# distribution point with CERT_TRUST_REVOCATION_STATUS_UNKNOWN (measured on
# the first run of this driver). The authority is private and has no CRL, so
# revocation is switched off for the driver; phones do not check revocation
# against a user-installed root, and OpenSSL-built curls do not either.
TLS_CURL() { "$CURL" --ssl-no-revoke --cacert "$OUT/ca.crt" "$@"; }
wait_idle() { for i in $(seq 1 40); do sleep 3; CTL agent list | python3 -c "import sys,json; d=json.load(sys.stdin); c=[c for c in d['conversations'] if c['conversation_id']=='$1']; sys.exit(0 if c and not c[0]['is_busy'] else 1)" && return; done; log "  (still busy after 120 s)"; }
say() { python3 -c "import sys,json;d=json.load(sys.stdin);r=d.get('response',d);e=r.get('error') or d.get('error');print('  ', 'REFUSED' if e else 'OK', (e or {}).get('code',''), '-', (e or {}).get('message','') or json.dumps(r.get('data',''))[:100])"; }

log "binary $(cat /mnt/c/dev/warp/target/release/warp-oss.version) ($(ls -la --time-style=+%F\ %H:%M /mnt/c/dev/warp/target/release/warp-oss.exe | awk '{print $6, $7}'))"
log "instances before: $(CTL instance list | python3 -c "import sys,json;print(len(json.load(sys.stdin).get('instances',[])))")"
LOG_MARK=$(wc -l < "$LOG_WIN" 2>/dev/null || echo 0)
log "launch: warpdev.ps1 -Console -Bind $BIND, detached"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'C:\dev\warp\.fork\tools\warpdev.ps1' -Console -Bind "$BIND" > "$OUT/launch.txt" 2>&1 &
for i in $(seq 1 30); do sleep 2; CTL instance list | grep -q '"instance_id"' && break; done
log "instance: $(CTL instance list | python3 -c "import sys,json;d=json.load(sys.stdin);print(len(d.get('instances',[])),'record(s)')")"
sleep 3
tail -n +"$((LOG_MARK + 1))" "$LOG_WIN" | grep -i "local-control\|authority\|wide" > "$OUT/log-listener.txt"
log "log: $(grep -c . "$OUT/log-listener.txt") listener lines; $(grep -o 'wide listener started[^"]*' "$OUT/log-listener.txt" | head -1); $(grep -o 'authority minted[^;]*' "$OUT/log-listener.txt" | head -1)"

log "--- the clear half: http://$BIND answers the certificate and an install page, nothing else"
"$CURL" -s -m 5 -D "$OUT/ca-headers.txt" -o "$OUT/ca.crt" "http://$BIND/ca.crt"; log "  /ca.crt: $(head -1 "$OUT/ca-headers.txt" | tr -d '\r'); $(grep -i '^content-type' "$OUT/ca-headers.txt" | tr -d '\r'); $(grep -c 'BEGIN CERTIFICATE' "$OUT/ca.crt") certificate(s)"
openssl x509 -in "$OUT/ca.crt" -noout -subject -dates -ext basicConstraints 2>&1 | tr '\n' ' ' | sed 's/^/  authority: /' | tee -a "$LOG"; echo
for path in /v1/state / /console.js; do
  "$CURL" -s -m 5 -D "$OUT/plain-headers.txt" -o "$OUT/plain-body.txt" "http://$BIND$path"
  log "  GET $path in the clear: $(head -1 "$OUT/plain-headers.txt" | tr -d '\r'); $(grep -q 'get the certificate' "$OUT/plain-body.txt" && echo 'the install page' || echo 'SOMETHING ELSE'); $(grep -c 'warp' "$OUT/plain-body.txt") lines mention warp"
done
cp "$OUT/plain-body.txt" "$OUT/install-page.html"

log "--- the TLS half: https://$BIND with the authority trusted, and without"
TLS_CURL -s -m 5 -D "$OUT/tls-headers.txt" -o "$OUT/console-over-tls.html" -w '%{http_code} verify=%{ssl_verify_result}\n' "https://$BIND/" > "$OUT/tls-status.txt" 2>&1; log "  GET / over TLS, --cacert: $(cat "$OUT/tls-status.txt" | tr -d '\r') $(head -c 15 "$OUT/console-over-tls.html")"
TLS_CURL -s -m 5 -o /dev/null -w '%{http_code}\n' "https://$BIND/ca.crt" | sed 's/^/  GET \/ca.crt over TLS: /' | tee -a "$LOG"
"$CURL" -s -m 5 -o /dev/null -w '%{http_code}\n' "https://$BIND/" > "$OUT/tls-untrusted.txt" 2>&1; log "  GET / over TLS, no --cacert: exit $? ($(cat "$OUT/tls-untrusted.txt" | tr -d '\r')) -- the warning page a fresh phone sees"
"$CURL" -sv -m 5 --cacert "$OUT/ca.crt" -o /dev/null "https://$BIND/" > "$OUT/schannel-revocation.txt" 2>&1; log "  curl.exe --cacert without --ssl-no-revoke: $(grep -o 'CertGetCertificateChain trust error [A-Z_]*' "$OUT/schannel-revocation.txt")"
# The leaf, read off the wire by the Windows side (WSL cannot reach the wide
# listener at all, exit 7, so openssl here would measure nothing).
powershell.exe -NoProfile -Command "\$c=New-Object Net.Sockets.TcpClient('${BIND%%:*}',${BIND##*:}); \$s=New-Object Net.Security.SslStream(\$c.GetStream(),\$false,{\$true}); \$s.AuthenticateAsClient('${BIND%%:*}'); \$x=New-Object Security.Cryptography.X509Certificates.X509Certificate2(\$s.RemoteCertificate); Write-Output ('protocol ' + \$s.SslProtocol + ' cipher ' + \$s.NegotiatedCipherSuite); Write-Output ('subject ' + \$x.Subject + ' issuer ' + \$x.Issuer + ' notAfter ' + \$x.NotAfter.ToString('u')); \$x.Extensions | ForEach-Object { Write-Output (\$_.Oid.FriendlyName + ': ' + \$_.Format(\$false)) }; \$s.Close()" > "$OUT/leaf.txt" 2>&1; log "  leaf: $(tr -d '\r' < "$OUT/leaf.txt" | grep -i 'protocol\|subject\|Alternative\|Enhanced' | tr '\n' ' ')"

log "--- the 09-05 flow, over https://"
CTL tab create > "$OUT/tab.json"; sleep 2
CTL input submit 'cd /home/effatha/git/warp' >/dev/null; sleep 4
CTL agent prompt 'Say the single word ready and nothing else.' > "$OUT/prompt-c.json"
C=$(python3 -c "import json;print(json.load(open('$OUT/prompt-c.json'))['conversation_id'])")
log "conversation C $C"; wait_idle "$C"; echo "$C" > "$OUT/conversation-c.txt"
CTL pair show --conversation "$C" > "$OUT/pair-show.json"
URL=$(python3 -c "import json;print(json.load(open('$OUT/pair-show.json'))['url'])")
CAURL=$(python3 -c "import json;print(json.load(open('$OUT/pair-show.json')).get('ca_url'))")
log "pair show: url scheme ${URL%%:*}://, ca_url $CAURL"
"$EXE" --warpctrl pair show --conversation "$C" > "$OUT/pair-show-pretty.txt" 2>&1; log "  pretty: $(grep -i 'first time' "$OUT/pair-show-pretty.txt")"
ORIGIN=${URL#https://}; ORIGIN=${ORIGIN%%/*}; CODE=${URL##*#}
TLS_CURL -s -m 5 -X POST -H "authorization: Bearer $CODE" "https://$ORIGIN/v1/pair" > "$OUT/pair.json"
log "redeem over TLS: $(python3 -c "import json;d=json.load(open('$OUT/pair.json'));print('device' if d.get('device_token') else d, '| expires_at:', d.get('expires_at','ABSENT (no clock)'), '| confined:', d.get('conversation_id'))")"
DEV=$(python3 -c "import json;print(json.load(open('$OUT/pair.json'))['device_token'])")
cred() { TLS_CURL -s -m 5 -X POST -H "authorization: Bearer $DEV" -H 'content-type: application/json' -d "{\"protocol_version\":1,\"request_id\":\"$(uuid)\",\"action\":\"$1\"}" "https://$ORIGIN/v1/pair/credential"; }
call() { TLS_CURL -s -m 20 -X POST -H "authorization: Bearer $1" -H 'content-type: application/json' -H "origin: $2" -d "{\"protocol_version\":1,\"request_id\":\"$(uuid)\",\"action\":{\"kind\":\"agent.prompt\",\"params\":{\"prompt\":\"$3\",\"conversation_id\":\"$C\"}}}" "https://$ORIGIN/v1/control"; }
TOK=$(cred agent.prompt | tee "$OUT/cred-prompt.json" | python3 -c "import sys,json;print(json.load(sys.stdin)['bearer_token'])")
log "prompt C with Origin: https://$ORIGIN (the page's own origin)"
call "$TOK" "https://$ORIGIN" "Reply with the single word over-tls and nothing else." | tee "$OUT/prompt-tls.json" | say | tee -a "$LOG"
wait_idle "$C"
log "prompt C with Origin: http://$ORIGIN (a scheme this listener never served; must be refused)"
call "$TOK" "http://$ORIGIN" "Reply with the single word wrong-scheme and nothing else." | tee "$OUT/prompt-wrong-scheme.json" | say | tee -a "$LOG"
sleep 2
CTL agent read "$C" > "$OUT/read-c.json"
log "C has $(python3 -c "import json;print(json.load(open('$OUT/read-c.json'))['exchange_count'])") exchanges (2 expected: ready, over-tls)"
TLS_CURL -s -m 5 -o /dev/null -w '%{http_code}\n' "https://$ORIGIN/v1/state" -H "authorization: Bearer $TOK" | sed 's/^/  GET \/v1\/state over TLS with the prompt credential (wrong scope, expect 403): /' | tee -a "$LOG"

log "--- the block in the pane: the chip clicked, then a narrow pane"
# `slash run remote-control` answers "not available in this build" (first run):
# the registry entry is upstream's GuiOnly command and the fork's chip is the
# door. The chip sat at (905,516) in this window on the probe screenshot.
SHOT tls-footer
powershell.exe -NoProfile -File 'C:\dev\click.ps1' -Process warp-oss -X 905 -Y 516 2>&1 | tail -1 | tee -a "$LOG"
sleep 3; SHOT tls-after-click
# The block is in the terminal pane behind the panel; Escape returns to it.
powershell.exe -NoProfile -File 'C:\dev\keys.ps1' -Process warp-oss -Key Escape 2>&1 | tail -1
sleep 2; SHOT tls-block
CTL pane split --direction right > "$OUT/split1.json"; sleep 1; CTL pane split --direction right > "$OUT/split2.json"; sleep 2
log "  two splits: $(python3 -c "import json;print([json.load(open('$OUT/split%d.json'%i)).get('pane',{}).get('id') for i in (1,2)])")"
SHOT tls-block-narrow
CTL pane list > "$OUT/panes.json" 2>&1

log "--- Brave, scratch profile, standing in for a phone that has NOT installed the authority"
powershell.exe -NoProfile -Command "Start-Process -FilePath '$BRAVE' -ArgumentList '--user-data-dir=C:\dev\brave-scratch-tls','--no-first-run','--window-size=430,900','--window-position=40,40','http://$BIND/'" 2>&1 | tail -1
sleep 6; SHOT brave-install-page brave
powershell.exe -NoProfile -Command "Start-Process -FilePath '$BRAVE' -ArgumentList '--user-data-dir=C:\dev\brave-scratch-tls','https://$BIND/'" 2>&1 | tail -1
sleep 5; SHOT brave-warning-page brave
log "  and the same page with the certificate check switched off, which is what a phone with the authority installed gets, as far as a screenshot can say"
powershell.exe -NoProfile -Command "Get-Process brave -ErrorAction SilentlyContinue | Stop-Process -Force" 2>/dev/null; sleep 2
powershell.exe -NoProfile -Command "Start-Process -FilePath '$BRAVE' -ArgumentList '--user-data-dir=C:\dev\brave-scratch-tls2','--no-first-run','--ignore-certificate-errors','--window-size=430,900','--window-position=40,40','https://$BIND/'" 2>&1 | tail -1
sleep 6; SHOT brave-pairing-page-secure brave
powershell.exe -NoProfile -Command "Get-Process brave -ErrorAction SilentlyContinue | Stop-Process -Force" 2>/dev/null

log "--- close"
CTL window close > "$OUT/close.json"; sleep 8
log "after close: $(CTL instance list | python3 -c "import sys,json;d=json.load(sys.stdin);print(len(d.get('instances',[])),'record(s)')")"
powershell.exe -NoProfile -Command "netstat -ano | findstr ${BIND##*:}" > "$OUT/netstat-after.txt" 2>&1; log "netstat on the port after close: $(grep -c LISTENING "$OUT/netstat-after.txt") LISTENING"
log "done"
