<#
.SYNOPSIS
  Launch the Windows fork build. The default is the product; the instruments
  are a switch you pass for one launch.

.DESCRIPTION
  Until 2026-09-04 this script had two states, "on" and "off", and neither was
  the build you would live in. "On" was the measurement rig: the agent forced
  into its `default` permission mode so every ask could be counted, plus the
  event log and the transcript. "Off" was stock upstream, with no fork agent at
  all, so the panel answered from Warp's account-gated cloud path. The daily
  driver, the fork's agent in its own shipped mode with nothing recording, had
  no spelling here at all. Run 2 was fifty minutes in the rig and was ended by
  the ask count; that count was the instrument working, not the product.

  Three profiles now, chosen per launch and never persisted:

    (default)       PRODUCT. `WARP_FORK_ACP_COMMAND` names the agent, started
                    inside the WSL distribution so a pane's Unix cwd resolves.
                    Nothing else is set: the agent runs in its own shipped
                    permission mode (`auto` for claude-agent-acp, where Claude
                    Code's classifier answers the easy asks on this machine, on
                    your subscription). No event log, no transcript. This is
                    thesis-compliant: nothing about where data goes or whose
                    credential pays is changed by it. What it costs is Warp's
                    visibility into permissions, and that is a measurement
                    loss, not a safety loss (T14.18).

    -Instrumented   THE RIG. Product plus `WARP_FORK_ACP_MODE=default`,
                    `WARP_FORK_EVENT_LOG=on`, `WARP_FORK_TRANSCRIPT=on`. The
                    agent asks about everything not on your allow list, and
                    every ask and answer is written down. Use it when you want
                    Warp in the loop and are prepared to answer for it.

    -EventLog       PRODUCT + THE LOG. The product profile plus
                    `WARP_FORK_EVENT_LOG=on` and nothing else: the agent stays
                    in its shipped mode, so Warp is still never asked, and what
                    the log gains is the tool calls and the join key to the
                    agent's own session file (`linked_session_id`). Added for
                    viewer phase 0 (`.fork/docs/observability.md`), whose
                    question is whether that join holds under `auto`; the rig
                    could not answer it because it forces `default`.

    -Stock          UPSTREAM. All four variables cleared. For A/B-ing a
                    suspected fork regression. Plan the shutdown first: with
                    `WARP_FORK_POLICY` untouched this still publishes a
                    discovery record, but the agent panel is upstream's.

  Nothing is ever written to the Windows environment. The variables are set
  with `$env:` inside this process only, reach the Warp it starts, and die with
  this script. The old `~/.warpdev` state file is no longer read: a persisted
  "instrumented" is exactly the corrupted-measurement-that-looks-like-a-working-
  day this header has always warned about, and with a product default there is
  nothing left for a state file to remember.

  The limitation, stated rather than discovered: this governs launches made
  through this script. A Warp started from Explorer, a shortcut, or a bare
  `warp-oss.exe` inherits none of it: no fork agent, no instruments.
  `-Status` says what a launch *would* do; it cannot say what a running instance
  was launched with. For a live one, look for the event-log directory.

.PARAMETER Exe
  The binary to launch. Unset, the release build is preferred and the debug
  build is the fallback; the choice is printed.

.PARAMETER WslRepo
  The WSL checkout this Windows checkout is a clone of, as a UNC path. Used only
  to report how far behind `C:\dev\warp` is. Nothing here syncs it: syncing the
  tree at launch without rebuilding would leave the source newer than the binary
  in the same checkout, which is the mismatch the commit check exists to catch.

.EXAMPLE
  warpdev.ps1                 # launch the product
.EXAMPLE
  warpdev.ps1 -Instrumented   # launch the rig, for one session
.EXAMPLE
  warpdev.ps1 -EventLog       # the product, with the event log and nothing else
.EXAMPLE
  warpdev.ps1 -Status         # print what a launch would set, and the tree state
#>
[CmdletBinding()]
param(
    [switch]$Instrumented,
    [switch]$EventLog,
    [switch]$Stock,
    [switch]$Status,
    [switch]$Force,
    [string]$Exe,
    [string]$WslRepo = '\\wsl.localhost\Ubuntu\home\effatha\git\warp',
    # Accepted so a shell alias written against the old script keeps working.
    # `-On` was the rig and maps to `-Instrumented`; `-Off` was stock upstream
    # and maps to `-Stock`; `-Launch` was the only way to launch and is now the
    # default, so it is a no-op.
    [switch]$Launch,
    [switch]$On,
    [switch]$Off
)

$ErrorActionPreference = 'Stop'
$WinRepo = 'C:\dev\warp'

if ($On)  { Write-Host "warpdev: -On is now -Instrumented" -ForegroundColor DarkGray; $Instrumented = $true }
if ($Off) { Write-Host "warpdev: -Off is now -Stock" -ForegroundColor DarkGray; $Stock = $true }
if ($Instrumented -and $Stock) {
    Write-Host "warpdev: -Instrumented and -Stock exclude each other." -ForegroundColor Red
    exit 2
}
if ($EventLog -and $Stock) {
    Write-Host "warpdev: -EventLog and -Stock exclude each other (stock has no fork log)." -ForegroundColor Red
    exit 2
}
if ($EventLog -and $Instrumented) {
    Write-Host "warpdev: -Instrumented already includes the event log; -EventLog ignored." -ForegroundColor DarkGray
}

$OldStateFile = Join-Path $HOME '.warpdev'
if (Test-Path $OldStateFile) {
    Write-Host "warpdev: ~/.warpdev is no longer read (profiles are per launch); delete it when convenient." -ForegroundColor DarkGray
}

# What each profile sets. Kept as data so `-Status` prints exactly what a launch
# would do: the variable, the value, and the reason.
$Product = @(
    @{ Name = 'WARP_FORK_ACP_COMMAND'
       # **Started inside the distribution, and on this platform that is not
       # optional (found 2026-09-03 while verifying T20.1).** A WSL pane's cwd
       # is a Unix path, Warp passes it verbatim in `session/new`, and the agent
       # is spawned by the *Windows* Warp, so the unwrapped `npx` form refuses
       # the session outright with "`cwd` does not exist on the machine running
       # the agent". Pinned, because `npx -y` with no version resolves to
       # whatever is newest and two installs once sat side by side for a week
       # giving opposite answers to the same question.
       Value = 'wsl.exe -d Ubuntu -- npx -y @agentclientprotocol/claude-agent-acp@0.73.0'
       Why = 'the agent panel answers from this agent, started inside WSL so a pane cwd resolves' }
)
$Instruments = @(
    @{ Name = 'WARP_FORK_ACP_MODE'
       Value = 'default'
       Why = 'makes the agent ask; without it its own classifier answers and Warp is never in the loop' }
    @{ Name = 'WARP_FORK_EVENT_LOG'
       Value = 'on'
       Why = 'one JSONL per conversation: tool calls, permission asks, what was decided' }
    @{ Name = 'WARP_FORK_TRANSCRIPT'
       Value = 'on'
       Why = 'the conversation on disk under the pane directory, for grepping back what compaction dropped' }
)
$AllVars = @($Product + $Instruments | ForEach-Object { $_.Name })

if ($Stock) {
    $ProfileName = 'STOCK (upstream; no fork agent)'
    $ToSet = @()
} elseif ($Instrumented) {
    $ProfileName = 'INSTRUMENTED (the rig)'
    $ToSet = @($Product + $Instruments)
} elseif ($EventLog) {
    $ProfileName = 'PRODUCT + EVENT LOG'
    $ToSet = @($Product + ($Instruments | Where-Object { $_.Name -eq 'WARP_FORK_EVENT_LOG' }))
} else {
    $ProfileName = 'PRODUCT'
    $ToSet = @($Product)
}

function Show-Plan {
    Write-Host "warpdev: profile $ProfileName" -ForegroundColor Green
    foreach ($i in $ToSet) {
        Write-Host ("  {0,-24} = {1}" -f $i.Name, $i.Value) -ForegroundColor DarkGray
        Write-Host ("  {0,-24}   {1}" -f '', $i.Why) -ForegroundColor DarkGray
    }
    $setNames = @($ToSet | ForEach-Object { $_.Name })
    $cleared = @($AllVars | Where-Object { $setNames -notcontains $_ })
    if ($cleared.Count -gt 0) {
        Write-Host ("  cleared: {0}" -f ($cleared -join ', ')) -ForegroundColor DarkGray
    }
    Write-Host "  note: a Warp started any other way gets none of this." -ForegroundColor DarkGray
}

# Resolve the binary: release if it exists, else debug, and say which.
if (-not $Exe) {
    $release = Join-Path $WinRepo 'target\release\warp-oss.exe'
    $debug   = Join-Path $WinRepo 'target\debug\warp-oss.exe'
    if (Test-Path $release) { $Exe = $release }
    elseif (Test-Path $debug) { $Exe = $debug; Write-Host "warpdev: no release build; using debug" -ForegroundColor Yellow }
    else { $Exe = $release }
}

Show-Plan

if (-not (Test-Path $Exe)) {
    Write-Host "warpdev: no binary at $Exe" -ForegroundColor Red
    Write-Host "  build it with C:\dev\build.ps1 -Release, and check that checkout is current:" -ForegroundColor DarkGray
    Write-Host "  git -C $WinRepo log --oneline -1" -ForegroundColor DarkGray
    exit 1
}
Write-Host "warpdev: binary $Exe" -ForegroundColor DarkGray

# Tree checks, all warnings. The Windows build is a second checkout that
# nothing syncs, and a build there reports success and changes nothing when it
# is behind, which is indistinguishable from a build that had nothing to do.
# So: which commit is the tree on, is the binary older than that commit, and
# how far behind the WSL `dev` is the tree.
#
# The binary-vs-commit comparison is honest *inside one checkout*. Comparing a
# binary here to a source file in the WSL tree is not (CLAUDE.md, 2026-09-02),
# which is why the WSL side is compared by commit only.
try {
    $head = (git -C $WinRepo log --oneline -1 2>$null)
    if ($head) { Write-Host "warpdev: tree at $head" -ForegroundColor DarkGray }
    # Scoped to source paths, so a docs-only commit does not cry stale on every
    # launch. The first cut compared against HEAD unscoped and parsed the epoch
    # with `[datetime]'1970-01-01Z'`, which never fired; verified against the
    # 2026-09-04 binary (built 21:58, last source commit 21:13, docs commit
    # 22:08) that this form says "not stale" and the unscoped one would have
    # said "stale".
    $srcEpoch = [int64](git -C $WinRepo log -1 --format=%ct -- app crates Cargo.toml Cargo.lock 2>$null)
    if ($srcEpoch) {
        $epoch0 = [datetime]::new(1970, 1, 1, 0, 0, 0, [System.DateTimeKind]::Utc)
        $binEpoch = [int64][math]::Floor(((Get-Item $Exe).LastWriteTimeUtc - $epoch0).TotalSeconds)
        if ($srcEpoch -gt $binEpoch) {
            Write-Host "warpdev: the binary predates the tree's last source commit; rebuild with C:\dev\build.ps1 -Release" -ForegroundColor Yellow
        }
    }
} catch { }
try {
    $winHead = (git -C $WinRepo rev-parse HEAD 2>$null)
    $wslHead = (git -C $WslRepo rev-parse dev 2>$null)
    if ($winHead -and $wslHead -and $winHead -ne $wslHead) {
        $behind = (git -C $WslRepo rev-list --count "$winHead..dev" 2>$null)
        Write-Host "warpdev: Windows checkout is $behind commit(s) behind WSL dev" -ForegroundColor Yellow
        # `origin` is the UNC path of the WSL repo and resolves from Windows git,
        # which is where this runs. `gh` is GitHub and carries only what has been
        # pushed, so it is usually behind; from WSL git use the Linux path instead.
        Write-Host "  sync:    git -C $WinRepo fetch origin dev; git -C $WinRepo merge --ff-only FETCH_HEAD" -ForegroundColor DarkGray
        Write-Host "  rebuild: C:\dev\build.ps1 -Release   (a launch never syncs for you: tree newer than binary is the mismatch above)" -ForegroundColor DarkGray
    }
} catch { }

if ($Status) { exit 0 }

# **Refuse to launch on top of a Warp that is already up (T20.3).** Measured in
# run 2: an agent answered an approval to "launch the Windows Warp build" while
# one was already running. Warp restores session layout, so the duplicate came up
# with identical panes and tabs and took foreground -- from the user's seat,
# indistinguishable from everything having crashed and restarted. Then it
# compounds, because two instances make every `warpctrl` call without
# `--instance` answer `ambiguous_instance`, including the agent's own. It was
# parked on a request to tell the two discovery records apart when the confusion
# was noticed: working back toward a cause it had created.
#
# The query below is the same one this script already ran *after* launching, to
# confirm the thing came up. Asking it first costs a second and is the whole fix.
#
# **Every record it returns is a live Warp, and that was measured rather than
# assumed.** The first cut of this check filtered the list by pid, on the
# strength of `CLAUDE.md`'s "killing the process leaves a stale discovery
# record". It does not: `crates/local_control/src/discovery.rs` prunes dead-PID
# records on every scan (`is_pid_alive`, two call sites), and killing Warp here
# left `instance list` empty. The pid filter was dead code guarding a condition
# that cannot arise, so it is gone and the doc it came from is corrected.
#
# What *did* accumulate three instances in one session is the opposite case and
# is covered: a CLI agent in a pane blocks `window close`, the close is refused,
# and the instance stays **alive**. Those are exactly the records below.
$existing = $null
try {
    $existing = & $Exe --warpctrl instance list --output-format json 2>$null | ConvertFrom-Json
} catch { }
# Fail *open* on a query that did not answer: refusing to launch because the
# check itself broke would take away the only way to start.
#
# **Written as `@($existing.instances)` first, which is fail-*closed*.** In
# PowerShell `@($null).Count` is **1**, so a query that returned nothing at all
# -- exe missing, `--warpctrl` absent from the build, a non-zero exit swallowed
# by the `try` -- produced one phantom instance and refused the launch, printing
# a blank `pid` line. Exactly the first-build case where the check is least
# entitled to an opinion. Found by review 2026-09-03 and confirmed by running
# `@($null.instances).Count` -> 1 against `@(@{instances=@()}.instances).Count`
# -> 0.
$live = if ($null -ne $existing -and $null -ne $existing.instances) {
    @($existing.instances)
} else {
    @()
}

if ($live.Count -gt 0 -and -not $Force) {
    Write-Host "warpdev: refusing to launch - Warp is already running." -ForegroundColor Red
    foreach ($inst in $live) {
        Write-Host "  $($inst.instance_id)  pid $($inst.pid)  $($inst.channel)" -ForegroundColor DarkGray
    }
    Write-Host "  A second instance restores the same panes and takes foreground, which looks" -ForegroundColor DarkGray
    Write-Host "  exactly like a crash-and-restart; and two instances make every warpctrl call" -ForegroundColor DarkGray
    Write-Host "  without --instance answer ambiguous_instance." -ForegroundColor DarkGray
    Write-Host "  Stop it with:  $Exe --warpctrl window close" -ForegroundColor DarkGray
    # **The bypass is deliberately not advertised here.** The actor in the
    # incident this check exists for was an *agent*, and an agent that is
    # refused reads the escape out of the error text and re-runs with it -- at
    # which point the refusal means nothing. `-Force` stays in the param block
    # for a person who reads the script; it is not offered to whoever tripped
    # the guard.
    exit 2
}
if ($live.Count -gt 0) {
    Write-Host "warpdev: -Force given; launching a second instance alongside $($live.Count) already up" -ForegroundColor Yellow
    Write-Host "  Expect ambiguous_instance from warpctrl calls without --instance." -ForegroundColor DarkGray
}

# Cleared rather than assumed absent: this process may have inherited them, and
# an inherited `WARP_FORK_ACP_MODE=default` would turn a product launch into the
# rig without anything printed saying so.
foreach ($name in $AllVars) { Remove-Item -Path "env:$name" -ErrorAction SilentlyContinue }
foreach ($i in $ToSet) { Set-Item -Path "env:$($i.Name)" -Value $i.Value }
Write-Host "warpdev: launching $ProfileName" -ForegroundColor Green

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
