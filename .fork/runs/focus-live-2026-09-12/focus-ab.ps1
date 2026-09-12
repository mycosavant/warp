# One half of the HANDOFF-FOCUS Task 2 A/B: launch one Warp under a scratch
# profile, ask the three questions from a background process, close it.
param(
    [Parameter(Mandatory)] [string]$Exe,
    [string]$Profile = 'focusab'
)
$ErrorActionPreference = 'Continue'

function Ctl([string[]]$argv) {
    # A broker handshake failure is not a targeting answer; retry it, and say so.
    for ($try = 1; $try -le 3; $try++) {
        $out = & $Exe --warpctrl @argv --output-format json 2>&1 | Out-String
        Write-Output ("--- warpctrl {0} (exit {1}, try {2})" -f ($argv -join ' '), $LASTEXITCODE, $try)
        Write-Output $out.Trim()
        if ($out -notmatch 'unauthorized_local_client') { return }
        Start-Sleep -Seconds 1
    }
}

Write-Output "exe: $Exe"
Write-Output ("version: " + (Get-Content "$Exe.version" -ErrorAction SilentlyContinue))
if (-not (Test-Path ([IO.Path]::ChangeExtension($Exe, 'version')))) {
    Write-Output ("sidecar: " + (Get-Content ([IO.Path]::Combine((Split-Path $Exe), 'warp-oss.version')) -ErrorAction SilentlyContinue))
}

$pre = & $Exe --warpctrl instance list --output-format json 2>$null | ConvertFrom-Json
if ($pre -and $pre.instances -and @($pre.instances).Count -gt 0) {
    Write-Output "REFUSING: a Warp instance is already up"
    $pre.instances | ForEach-Object { Write-Output "  $($_.instance_id) pid $($_.pid)" }
    exit 2
}

Get-ChildItem env: | Where-Object Name -like 'WARP_FORK_*' | ForEach-Object { Remove-Item "env:$($_.Name)" }
$env:WARP_DATA_PROFILE = $Profile
$launchedAt = Get-Date
Start-Process -FilePath $Exe -WorkingDirectory 'C:\dev\warp' -NoNewWindow

$up = $false
$deadline = (Get-Date).AddSeconds(90)
while ((Get-Date) -lt $deadline) {
    Start-Sleep -Seconds 2
    $l = & $Exe --warpctrl instance list --output-format json 2>$null | ConvertFrom-Json
    if ($l -and $l.instances -and @($l.instances).Count -gt 0) { $up = $true; break }
}
Write-Output ("up: {0} after {1:N0}s" -f $up, ((Get-Date) - $launchedAt).TotalSeconds)
if (-not $up) { exit 1 }
Start-Sleep -Seconds 5

# app active first: it is the check that Warp is not frontmost. A launch
# sometimes takes foreground, which makes the pre-fix binary pass and the run
# worthless. If it did, start another application so it takes foreground --
# the handoff's "click another application", without the user's cursor.
# (Minimizing via ShowWindow was tried first and left app.active naming the
# window, so it is not used.) Recorded as the mode; the helper is ended by pid.
$active = & $Exe --warpctrl app active --output-format json 2>$null | Out-String
$mode = 'unfocused at launch'
$helper = $null
if ($active -match '"window_id"') {
    $helper = Start-Process powershell.exe -PassThru -ArgumentList @(
        '-NoProfile', '-Command',
        'Add-Type -AssemblyName System.Windows.Forms; $f = New-Object Windows.Forms.Form; $f.Text = ''focus-ab helper''; $f.Width = 320; $f.Height = 120; [void]$f.ShowDialog()'
    )
    Start-Sleep -Seconds 4
    $mode = "focused at launch; helper window pid $($helper.Id) started"
}
Write-Output "focus mode: $mode"
Ctl @('app', 'active')
Ctl @('window', 'list')
Ctl @('tab', 'reset-name')
Ctl @('session', 'inspect')
Ctl @('app', 'active')

if ($helper) { Stop-Process -Id $helper.Id -ErrorAction SilentlyContinue }
Ctl @('window', 'close')
Start-Sleep -Seconds 5
$post = & $Exe --warpctrl instance list --output-format json 2>$null | ConvertFrom-Json
Write-Output ("instances after close: {0}" -f @($post.instances | Where-Object { $_ }).Count)
