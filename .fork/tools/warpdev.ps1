<#
.SYNOPSIS
  Launch the Windows fork build with its instruments on, without ever leaving
  them on.

.DESCRIPTION
  The fork's four environment variables turn a Warp launch into a *measured*
  one. Setting them at User or Machine scope would be the obvious way to do it
  and is the wrong one: a variable set in October is still set in December, and
  `WARP_FORK_ACP_COMMAND` silently replaces the agent transport for every
  session after it. That is a corrupted measurement that looks like a working
  day.

  So nothing here is ever persisted to the environment. The variables are set
  with `$env:` inside this process only, which means they reach the Warp it
  starts and die with this script. What *is* persisted is a single word in a
  state file, and every run of this script prints it.

  The limitation, stated rather than discovered: this governs launches made
  through this script. A Warp started from Explorer, a shortcut, or a bare
  `warp-oss.exe` inherits none of it and is not instrumented, however this
  toggle is set. `-Status` will tell you what a launch *would* do; it cannot
  tell you what a running instance was launched with.

.EXAMPLE
  warpdev.ps1              # report state; enable if off, offer to disable if on
.EXAMPLE
  warpdev.ps1 -Launch      # launch Warp applying the current state
.EXAMPLE
  warpdev.ps1 -On -Launch  # enable and launch in one go
#>
[CmdletBinding()]
param(
    [switch]$On,
    [switch]$Off,
    [switch]$Status,
    [switch]$Launch,
    [string]$Exe = 'C:\dev\warp\target\debug\warp-oss.exe'
)

$ErrorActionPreference = 'Stop'

# Beside the user's profile rather than in the repo: a repo file would be
# untracked noise in `git status` on every run, and this is a fact about this
# machine, not about the fork.
$StateFile = Join-Path $HOME '.warpdev'

# The instruments, and why each is here. Kept as data so `-Status` can print
# exactly what a launch would set — the variable, the value, and the reason.
$Instruments = @(
    @{ Name = 'WARP_FORK_ACP_COMMAND'
       Value = 'npx -y @agentclientprotocol/claude-agent-acp'
       Why = 'the agent panel answers from this agent instead of upstream' }
    @{ Name = 'WARP_FORK_ACP_MODE'
       Value = 'default'
       Why = 'makes the agent ask; without it its own classifier answers and Warp is never in the loop' }
    @{ Name = 'WARP_FORK_EVENT_LOG'
       Value = 'on'
       Why = 'one JSONL per conversation - tool calls, permission asks, what was decided' }
    @{ Name = 'WARP_FORK_TRANSCRIPT'
       Value = 'on'
       Why = 'the conversation on disk, owner-only, under the pane directory' }
)

function Get-State {
    if (Test-Path $StateFile) {
        $raw = (Get-Content $StateFile -Raw).Trim().ToLower()
        # Anything unrecognised reads as off. A state file that has been edited
        # by hand into something meaningless must not silently mean "measured".
        return ($raw -eq 'on')
    }
    return $false
}

function Set-State([bool]$Enabled) {
    Set-Content -Path $StateFile -Value $(if ($Enabled) { 'on' } else { 'off' }) -NoNewline
}

function Show-State([bool]$Enabled) {
    if ($Enabled) {
        Write-Host "warpdev: ON  - a launch from this script is instrumented" -ForegroundColor Green
        foreach ($i in $Instruments) {
            Write-Host ("  {0,-24} = {1}" -f $i.Name, $i.Value) -ForegroundColor DarkGray
        }
    } else {
        Write-Host "warpdev: OFF - a launch from this script is stock upstream behaviour" -ForegroundColor Yellow
    }
    Write-Host "  state file: $StateFile" -ForegroundColor DarkGray
    Write-Host "  note: a Warp started any other way is NOT instrumented, either way." -ForegroundColor DarkGray
}

$enabled = Get-State

if ($On)  { $enabled = $true;  Set-State $true;  Show-State $enabled }
elseif ($Off) { $enabled = $false; Set-State $false; Show-State $enabled }
elseif ($Status) { Show-State $enabled }
elseif (-not $Launch) {
    # The bare form: report, then do the thing that is not already true.
    Show-State $enabled
    if ($enabled) {
        $answer = Read-Host "`nDisable instrumentation? [y/N]"
        if ($answer -match '^(y|yes)$') { Set-State $false; $enabled = $false; Write-Host ""; Show-State $enabled }
        else { Write-Host "left on." -ForegroundColor DarkGray }
    } else {
        Set-State $true; $enabled = $true; Write-Host ""; Show-State $enabled
    }
}

if (-not $Launch) { return }

if (-not (Test-Path $Exe)) {
    Write-Host "warpdev: no binary at $Exe" -ForegroundColor Red
    Write-Host "  build it with C:\dev\build.ps1, and check that checkout is current:" -ForegroundColor DarkGray
    Write-Host "  git -C C:\dev\warp log --oneline -1" -ForegroundColor DarkGray
    exit 1
}

# The commit check, because the Windows build is a second checkout that nothing
# syncs. A build there reports success and changes nothing when it is behind,
# which is indistinguishable from a build that had nothing to do.
try {
    $head = (git -C 'C:\dev\warp' log --oneline -1 2>$null)
    if ($head) { Write-Host "warpdev: building tree at $head" -ForegroundColor DarkGray }
} catch { }

if ($enabled) {
    foreach ($i in $Instruments) { Set-Item -Path "env:$($i.Name)" -Value $i.Value }
    Write-Host "warpdev: launching INSTRUMENTED" -ForegroundColor Green
} else {
    # Cleared rather than assumed absent: this process may have inherited them.
    foreach ($i in $Instruments) { Remove-Item -Path "env:$($i.Name)" -ErrorAction SilentlyContinue }
    Write-Host "warpdev: launching plain (no instruments)" -ForegroundColor Yellow
}

# `-NoNewWindow` is load-bearing and not cosmetic. `warp-oss.exe` is a
# console-subsystem binary; without this it gets its own console, `stdout` is a
# tty, and `warp_logging` writes no logfile at all - while a log still appears,
# because the crash-recovery sibling has no console and its file is moved into
# place when the parent dies. A log beginning "Parent has crashed" is that one,
# and the interesting half was never written.
Start-Process -FilePath $Exe -WorkingDirectory (Split-Path (Split-Path (Split-Path $Exe))) -NoNewWindow

# Read back rather than assume: a launch that fails silently is the failure this
# whole script exists to make visible.
$deadline = (Get-Date).AddSeconds(45)
while ((Get-Date) -lt $deadline) {
    Start-Sleep -Seconds 2
    $found = & $Exe --warpctrl instance list 2>$null | Select-String 'inst_'
    if ($found) {
        Write-Host "warpdev: up - $found" -ForegroundColor Green
        exit 0
    }
}
Write-Host "warpdev: no discovery record after 45s." -ForegroundColor Red
Write-Host "  The window may still be on first-run onboarding, which has no workspace." -ForegroundColor DarkGray
exit 1
