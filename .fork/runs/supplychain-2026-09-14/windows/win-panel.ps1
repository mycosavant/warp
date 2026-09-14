# One panel turn on the Windows debug build, with the agent installed by
# .fork/tools/agents.py inside WSL. Scratch data profile; every file this
# writes is under \\wsl.localhost\Ubuntu\tmp\sc-run, none on C:.
param([string]$Out = '\\wsl.localhost\Ubuntu\tmp\sc-run\win')
$ErrorActionPreference = 'Continue'
New-Item -ItemType Directory -Force -Path $Out | Out-Null
function Now { (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ss.fffZ') }
function Tl($k, $extra = '') { Add-Content -Path "$Out\timeline.txt" -Value "$k $(Now) $extra" }
$Exe = 'C:\dev\warp\target\debug\warp-oss.exe'
function Ctl { param([string[]]$argv) $o = & $Exe --warpctrl @argv --output-format json 2>&1 | Out-String; Add-Content "$Out\warpctrl.txt" "--- warpctrl $($argv -join ' ') (exit $LASTEXITCODE) $(Now)`n$o"; $o }

Get-FileHash -Algorithm SHA256 $Exe | ForEach-Object { "$($_.Hash.ToLower())  $($_.Path)" } | Set-Content "$Out\sha256.txt"
$env:WARP_DATA_PROFILE = 'supplychain'
$env:WARP_FORK_ACP_COMMAND = 'wsl.exe -d Ubuntu -- /home/effatha/.local/share/warp-fork/agents/claude-agent-acp/0.73.0/bin/claude-agent-acp'
$env:WARP_FORK_EVENT_LOG = 'on'
$env:WARP_FORK_WINDOW_BOUNDS = '1200x800+60+60'
"WARP_DATA_PROFILE=$env:WARP_DATA_PROFILE`nWARP_FORK_ACP_COMMAND=$env:WARP_FORK_ACP_COMMAND" | Set-Content "$Out\env.txt"

$pre = (& $Exe --warpctrl instance list --output-format json 2>$null | ConvertFrom-Json).instances.pid
Tl LAUNCH
Start-Process -FilePath $Exe -WorkingDirectory 'C:\dev\warp' -NoNewWindow
$inst = $null
for ($i = 0; $i -lt 60; $i++) {
    Start-Sleep -Seconds 2
    $l = (& $Exe --warpctrl instance list --output-format json 2>$null | ConvertFrom-Json).instances
    $inst = $l | Where-Object { $pre -notcontains $_.pid } | Select-Object -First 1
    if ($inst) { break }
}
if (-not $inst) { Tl NO_INSTANCE; exit 1 }
Tl UP "pid=$($inst.pid)"
Start-Sleep -Seconds 8
Ctl @('input', 'submit', 'cd /tmp/sc-run/ws') | Out-Null
Start-Sleep -Seconds 4
Tl PROMPT
Ctl @('agent', 'prompt', 'In one sentence, what is 2+2? Do not use any tools.') | Out-Null
for ($i = 0; $i -lt 45; $i++) {
    Start-Sleep -Seconds 2
    $list = Ctl @('agent', 'list')
    if ($list -match '"status":\s*"(success|error|cancelled)"') { break }
}
Tl SETTLED
Ctl @('agent', 'list') | Out-Null
