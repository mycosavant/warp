# Configuration G: the claude-agent-acp probe with the agent on the Windows side.
# Launched from WSL; every file it writes is under the WSL scratch directory
# (passed as -Out), none on C:.
param(
    [Parameter(Mandatory)][string]$Out,        # UNC run directory
    [Parameter(Mandatory)][string]$Prompt,
    [string]$Proxy = 'http://127.0.0.1:8081',
    [string]$CaCert,
    [string]$Cwd = 'C:\Users\onemind',
    [switch]$Approve,
    [switch]$NoFlag
)
$ErrorActionPreference = 'Continue'
function Now { (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ss.fffZ') }
function Tl($k, $extra = '') { Add-Content -Path "$Out\timeline.txt" -Value "$k $(Now) $extra" }

$cache = 'C:/Users/onemind/AppData/Local/npm-cache/_npx/6130df88bef2cca3/node_modules'
$node = 'C:/Program Files/nodejs/node.exe'
$acp = "$cache/@agentclientprotocol/claude-agent-acp/dist/index.js"
$claude = "$cache/@anthropic-ai/claude-agent-sdk-win32-x64/claude.exe"
$warp = 'C:\dev\warp\target\release\warp-oss.exe'

Get-FileHash -Algorithm SHA256 $node, $acp, $claude, $warp | ForEach-Object { "$($_.Hash.ToLower())  $($_.Path)" } |
    Set-Content "$Out\sha256.txt"

$env:HTTPS_PROXY = $Proxy; $env:HTTP_PROXY = $Proxy; $env:NO_PROXY = '127.0.0.1,localhost'
$env:NODE_EXTRA_CA_CERTS = $CaCert
$env:ANTHROPIC_BASE_URL = 'http://127.0.0.1:8080'; $env:ANTHROPIC_API_KEY = 'local'; $env:ANTHROPIC_MODEL = 'gemma-4-12b'
$env:CLAUDE_CODE_EXECUTABLE = $claude
if (-not $NoFlag) { $env:CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC = '1' }
Get-ChildItem env: | Where-Object { $_.Name -match 'PROXY|CA_CERTS|ANTHROPIC_BASE|ANTHROPIC_MODEL|CLAUDE_CODE' } |
    ForEach-Object { "$($_.Name)=$($_.Value)" } | Set-Content "$Out\env.txt"

$stop = "$Out\stop-win"
# -Command, not -File: an array parameter does not survive -File, and -Names
# arriving as one string would watch nothing but the process tree.
$pollCmd = "& '\\wsl.localhost\Ubuntu\home\effatha\git\warp\.fork\tools\egress-poll.ps1' -Out '$Out\win-sockets.tsv' -StopFile '$stop' -Names @('warp-oss.exe','node.exe','claude.exe','curl.exe')"
$poll = Start-Process powershell.exe -PassThru -WindowStyle Hidden -ArgumentList @(
    '-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command', $pollCmd)
Start-Sleep -Seconds 3
Tl CENSUS_START

& curl.exe -s -o NUL --ssl-no-revoke -k -x $Proxy https://example.com/
Tl CONTROL_1A

# Windows PowerShell 5.1 strips embedded quotes when it passes arguments to a
# native program, so a quoted "C:/Program Files/..." arrives split. The 8.3
# short path has no space to quote.
$nodeShort = (New-Object -ComObject Scripting.FileSystemObject).GetFile($node).ShortPath -replace '\\', '/'
$cmd = "$nodeShort $acp"
$probeArgs = @('--warpctrl', 'acp', 'probe', '--command', $cmd, '--prompt', $Prompt, '--cwd', $Cwd, '--output-format', 'ndjson')
if ($Approve) { $probeArgs += '--approve' }
Tl DRIVER_START
& $warp @probeArgs 2> "$Out\probe.err" | ForEach-Object { '{"ts":"' + (Now) + '","line":' + ($_ | ConvertTo-Json -Compress) + '}' } |
    Set-Content "$Out\probe.ndjson"
Tl DRIVER_END "exit=$LASTEXITCODE"
Start-Sleep -Seconds 3

# Control 2: a non-loopback connection with no proxy must appear in the census.
Remove-Item env:HTTPS_PROXY, env:HTTP_PROXY
# Rate-limited so the connection outlives the poller's one-second name refresh;
# an unthrottled curl.exe finished before it and the control never fired.
& curl.exe -s -o NUL --ssl-no-revoke --limit-rate 100 https://example.com/
Tl CONTROL_2
Start-Sleep -Seconds 2
Set-Content -Path $stop -Value stop
$poll.WaitForExit(15000) | Out-Null
Tl CENSUS_STOP
