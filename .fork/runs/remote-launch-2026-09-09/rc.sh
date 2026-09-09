#!/usr/bin/env bash
# Can a GUI Warp be launched from a remote shell, and reached from a phone?
#
# The question the maintainer asked on 2026-09-09: if Warp dies while they are
# at work, can they start it again from the phone, or does that need the desk?
#
# Two phases, because the realistic case is the harder one:
#   A  the Windows session unlocked  (the control -- proves the mechanism)
#   B  the Windows session locked    (the real case -- proves it while away)
#
# "Remote" is simulated with `env -i`: an sshd session into WSL carries no
# WSL_INTEROP and no Windows entries in PATH (measured the same day, on the
# maintainer's own live phone sessions at pts/6 and pts/7). Launching a Windows
# binary by absolute path from that environment is exactly what the phone does.
set -u

OUT=/home/effatha/git/warp/.fork/runs/remote-launch-2026-09-09
LOG="$OUT/driver.log"
PS='/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe'
EXE='/mnt/c/dev/warp/target/release/warp-oss.exe'
LAUNCHER='\\wsl.localhost\Ubuntu\home\effatha\git\warp\.fork\tools\warpdev.ps1'
WINLOGS=/mnt/c/Users/onemind/AppData/Local/warp/WarpOss/data/logs
BIND=100.82.213.46:41234

log() { echo "[$(date -u +%H:%M:%SZ)] $*" | tee -a "$LOG"; }
ps_run() { "$PS" -NoProfile -Command "$1" 2>&1 | tr -d '\r'; }

# The whole point: a stripped environment, launching a Windows binary by path.
remote_ps() {
  env -i HOME=/home/effatha PATH=/usr/bin:/bin:/usr/local/bin \
    "$PS" -NoProfile -ExecutionPolicy Bypass "$@" 2>&1 | tr -d '\r'
}

snapshot() {
  log "--- snapshot: $1 ---"
  ps_run "
    \$p = Get-Process warp-oss -ErrorAction SilentlyContinue
    if (\$p) { \$p | ForEach-Object { 'process pid={0} start={1}' -f \$_.Id, \$_.StartTime.ToString('u') } } else { 'process none' }
    if (Get-Process LogonUI -ErrorAction SilentlyContinue) { 'session LOCKED' } else { 'session UNLOCKED' }
    \$l = Get-NetTCPConnection -State Listen -LocalPort 41234 -ErrorAction SilentlyContinue
    if (\$l) { \$l | ForEach-Object { 'listener {0}:{1} pid={2}' -f \$_.LocalAddress, \$_.LocalPort, \$_.OwningProcess } } else { 'listener none' }
  " | tee -a "$LOG"
  echo "instances: $("$EXE" --warpctrl instance list --output-format json 2>&1 | tr -d '\r' | tr -d '\n ')" | tee -a "$LOG"
}

# TLS handshake plus the leaf, through the Windows stack. WSL cannot reach the
# wide listener (connection refused, measured), so this has to go through
# interop even though the console is served from WSL's own machine.
tls_probe() {
  ps_run "
    try {
      \$c = New-Object System.Net.Sockets.TcpClient
      \$c.Connect('${BIND%:*}', ${BIND##*:})
      \$cb = { param(\$s,\$cert,\$chain,\$err) return \$true }
      \$ssl = New-Object System.Net.Security.SslStream(\$c.GetStream(), \$false, \$cb)
      \$ssl.AuthenticateAsClient('${BIND%:*}')
      \$leaf = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2(\$ssl.RemoteCertificate)
      'tls OK {0} leaf={1} sha256={2}' -f \$ssl.SslProtocol, \$leaf.Subject, \$leaf.GetCertHashString('SHA256').Substring(0,16)
      \$ssl.Close(); \$c.Close()
    } catch { 'tls FAIL ' + \$_.Exception.Message }
  " | tee -a "$LOG"
}

close_warp() {
  log "closing the running instance"
  for i in 1 2 3; do
    "$EXE" --warpctrl window close 2>&1 | tr -d '\r' | tee -a "$LOG"
    sleep 4
    if ! ps_run "if (Get-Process warp-oss -ErrorAction SilentlyContinue) { 'alive' } else { 'gone' }" | grep -q alive; then
      log "closed after attempt $i"; return 0
    fi
    log "still alive after attempt $i (the close is Cancellable; a dialog can refuse it)"
  done
  log "WARNING: did not close after 3 attempts"
  return 1
}

launch_remote() {
  log "launching from a stripped environment (simulated ssh session)"
  # setsid so the launcher outlives this shell, and stdin closed so nothing in
  # the chain waits on a terminal that a remote session may not have.
  setsid env -i HOME=/home/effatha PATH=/usr/bin:/bin:/usr/local/bin \
    "$PS" -NoProfile -ExecutionPolicy Bypass -File "$LAUNCHER" -Console \
    > "$OUT/launch-$1.txt" 2>&1 < /dev/null &
  disown
  for i in $(seq 1 20); do
    sleep 3
    if "$EXE" --warpctrl instance list --output-format json 2>/dev/null | grep -q instance_id; then
      log "instance record present after $((i*3))s"; return 0
    fi
  done
  log "no instance record after 60s"
  return 1
}

render_check() {
  local newest
  newest=$(ls -t "$WINLOGS"/warp-oss.log 2>/dev/null | head -1)
  log "log: ${newest:-<none>}"
  if [ -n "${newest:-}" ]; then
    grep -aiE "failed to render|parent has crashed|panic|GPU|adapter|surface" "$newest" 2>/dev/null | tail -12 | tee -a "$LOG"
  fi
}

case "${1:-}" in
  a)
    log "=== PHASE A: unlocked ==="
    snapshot "before"
    close_warp
    snapshot "after close"
    launch_remote a
    sleep 6
    snapshot "after remote launch"
    tls_probe
    render_check
    ;;
  b)
    log "=== PHASE B: locked ==="
    close_warp
    log "locking the workstation"
    ps_run "rundll32.exe user32.dll,LockWorkStation" >/dev/null
    sleep 8
    snapshot "locked, no warp"
    launch_remote b
    sleep 6
    snapshot "after remote launch while locked"
    tls_probe
    render_check
    ;;
  *) echo "usage: rc.sh a|b"; exit 2 ;;
esac
log "done"
