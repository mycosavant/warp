# Poll every socket the Windows Warp holds, for the life of a run.
#
# The Windows half of the egress measurement recorded in
# `.fork/tickets/open-questions.md` ("Nothing escapes: measured, not argued"),
# which until 2026-09-05 was Linux-only. Same shape as the `ss` loop there:
# sample often, keep every distinct (process, protocol, local, remote, state)
# tuple, and write it the first time it is seen. `Get-NetTCPConnection` lists
# every state, `SynSent` included, so a connection that is attempted and refused
# still appears; `Get-NetUDPEndpoint` covers UDP.
#
# What this cannot see, stated so nobody credits it with more: name resolution
# on Windows is done by the Dnscache service in a `svchost`, not by the process
# that asked, so a lookup by Warp never appears against Warp's pid. The proxy
# half of the rig covers HTTP; `Get-DnsClientCache` at the end of a run is the
# weak third instrument for the names.
#
#   powershell.exe -NoProfile -File C:\dev\egress-poll.ps1 -Out C:\dev\egress\sockets.tsv -StopFile C:\dev\egress\stop
#
# Watched by process *name*: every `warp-oss.exe` (the parent and its
# crash-recovery sibling), `curl.exe` as the control that proves the poller can
# see a connection at all, and anything descended from a `warp-oss.exe`.
param(
    [Parameter(Mandatory)][string]$Out,
    [Parameter(Mandatory)][string]$StopFile,
    [int]$IntervalMs = 200,
    [string[]]$Names = @('warp-oss.exe', 'curl.exe')
)

$ErrorActionPreference = 'Continue'
$seen = @{}
$samples = 0
$started = Get-Date
$namesAt = [datetime]::MinValue
$treeAt = [datetime]::MinValue
$watch = @{}   # pid -> name
$nameByPid = @{}

function Stamp { (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ss.fffZ') }

"# started $(Stamp) interval ${IntervalMs}ms names $($Names -join ',')" | Out-File -FilePath $Out -Encoding utf8
"# ts`tpid`tname`tproto`tlocal`tremote`tstate`tsample" | Out-File -FilePath $Out -Encoding utf8 -Append

while (-not (Test-Path $StopFile)) {
    $now = Get-Date

    # Names every second (cheap), the parent/child tree every five (not cheap).
    if (($now - $namesAt).TotalSeconds -ge 1) {
        $nameByPid = @{}
        foreach ($p in Get-Process -ErrorAction SilentlyContinue) { $nameByPid[[int]$p.Id] = "$($p.ProcessName).exe" }
        foreach ($kv in @($nameByPid.GetEnumerator())) {
            if ($Names -contains $kv.Value) { $watch[$kv.Key] = $kv.Value }
        }
        $namesAt = $now
    }
    if (($now - $treeAt).TotalSeconds -ge 5) {
        $procs = Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, Name -ErrorAction SilentlyContinue
        $changed = $true
        while ($changed) {
            $changed = $false
            foreach ($p in $procs) {
                $id = [int]$p.ProcessId
                if (-not $watch.ContainsKey($id) -and $watch.ContainsKey([int]$p.ParentProcessId) -and $watch[[int]$p.ParentProcessId] -like 'warp-oss*') {
                    $watch[$id] = "$($p.Name) <- warp-oss.exe"
                    $changed = $true
                }
            }
        }
        $treeAt = $now
    }

    $samples++
    $tcp = @(Get-NetTCPConnection -ErrorAction SilentlyContinue)
    $udp = @(Get-NetUDPEndpoint -ErrorAction SilentlyContinue)
    foreach ($c in $tcp) {
        $id = [int]$c.OwningProcess
        if (-not $watch.ContainsKey($id)) { continue }
        $key = "$id|tcp|$($c.LocalAddress):$($c.LocalPort)|$($c.RemoteAddress):$($c.RemotePort)|$($c.State)"
        if ($seen.ContainsKey($key)) { continue }
        $seen[$key] = $samples
        "$(Stamp)`t$id`t$($watch[$id])`ttcp`t$($c.LocalAddress):$($c.LocalPort)`t$($c.RemoteAddress):$($c.RemotePort)`t$($c.State)`t$samples" | Out-File -FilePath $Out -Encoding utf8 -Append
    }
    foreach ($u in $udp) {
        $id = [int]$u.OwningProcess
        if (-not $watch.ContainsKey($id)) { continue }
        $key = "$id|udp|$($u.LocalAddress):$($u.LocalPort)"
        if ($seen.ContainsKey($key)) { continue }
        $seen[$key] = $samples
        "$(Stamp)`t$id`t$($watch[$id])`tudp`t$($u.LocalAddress):$($u.LocalPort)`t-`t-`t$samples" | Out-File -FilePath $Out -Encoding utf8 -Append
    }
    # A heartbeat with the machine-wide count, so a silent file is provably a
    # poller that was reading live data and not one that had stopped.
    if ($samples % 150 -eq 0) {
        "# heartbeat $(Stamp) sample $samples tcp_all $($tcp.Count) udp_all $($udp.Count) watched_pids $($watch.Count)" | Out-File -FilePath $Out -Encoding utf8 -Append
    }
    Start-Sleep -Milliseconds $IntervalMs
}
$elapsed = [int]((Get-Date) - $started).TotalSeconds
"# stopped $(Stamp) samples $samples elapsed_s $elapsed distinct_tuples $($seen.Count)" | Out-File -FilePath $Out -Encoding utf8 -Append
