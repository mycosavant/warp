# Live check for WARP_FORK_WINDOW_BOUNDS. Three launches of one debug Warp under
# one scratch profile, so launches 2 and 3 restore what the one before saved:
#   1. variable set      -> the window should open at the pinned rect.
#      Then the window is moved elsewhere (SetWindowPos, no focus taken), so
#      the position saved at close is NOT the pinned one.
#   2. variable set      -> a restored window should still open at the pinned rect.
#   3. variable unset    -> control: the restored window opens where launch 2
#      left it, which is the pinned rect, so this launch then moves it and
#      the check is only that nothing forces it back.
# Rects are printed in physical pixels (the script is DPI-aware) with the
# monitor's scale, so logical 100,100 at 150% reads as 150,150.
param(
    [string]$Exe = 'C:\dev\warp\target\debug\warp-oss.exe',
    [string]$Profile = 'brokerrace',
    [string]$Bounds = '1400x900+100+100'
)
$ErrorActionPreference = 'Continue'

Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Place {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr a, int x, int y, int cx, int cy, uint f);
}
"@
[void][Place]::SetProcessDPIAware()

function Launch([string]$label, [bool]$withVar, [bool]$moveAfter) {
    Get-ChildItem env: | Where-Object Name -like 'WARP_FORK_*' | ForEach-Object { Remove-Item "env:$($_.Name)" }
    $env:WARP_DATA_PROFILE = $Profile
    if ($withVar) { $env:WARP_FORK_WINDOW_BOUNDS = $Bounds }
    $proc = Start-Process -FilePath $Exe -WorkingDirectory 'C:\dev\warp' -NoNewWindow -PassThru
    Remove-Item env:WARP_FORK_WINDOW_BOUNDS -ErrorAction SilentlyContinue

    $h = [IntPtr]::Zero
    $deadline = (Get-Date).AddSeconds(90)
    while ((Get-Date) -lt $deadline -and $h -eq [IntPtr]::Zero) {
        Start-Sleep -Seconds 2
        $proc.Refresh()
        $h = $proc.MainWindowHandle
    }
    if ($h -eq [IntPtr]::Zero) { Write-Output "$label : no window for pid $($proc.Id)"; return }
    Start-Sleep -Seconds 6

    $r = New-Object Place+RECT
    [void][Place]::GetWindowRect($h, [ref]$r)
    $dpi = [Place]::GetDpiForWindow($h)
    $screen = [System.Windows.Forms.Screen]::FromHandle($h)
    Write-Output ("{0} : var={1} pid={2} rect=({3},{4}) {5}x{6} scale={7}% monitor={8} primary={9} monitorBounds={10}" -f `
        $label, $withVar, $proc.Id, $r.L, $r.T, ($r.R - $r.L), ($r.B - $r.T), [int]($dpi * 100 / 96), `
        $screen.DeviceName, $screen.Primary, $screen.Bounds)

    if ($moveAfter) {
        $ok = [Place]::SetWindowPos($h, [IntPtr]::Zero, 600, 400, 1000, 700, 0x0014)
        Start-Sleep -Seconds 2
        [void][Place]::GetWindowRect($h, [ref]$r)
        Write-Output ("{0} : moved ok={1} -> rect=({2},{3}) {4}x{5}" -f $label, $ok, $r.L, $r.T, ($r.R - $r.L), ($r.B - $r.T))
    }

    $out = & $Exe --warpctrl window close --output-format json 2>&1 | Out-String
    if ($out -notmatch '"ok":\s*true') { Write-Output "$label : close answered: $($out.Trim())" }
    $deadline = (Get-Date).AddSeconds(30)
    while ((Get-Date) -lt $deadline -and -not $proc.HasExited) { Start-Sleep -Seconds 1 }
    Write-Output ("{0} : exited={1}" -f $label, $proc.HasExited)
    Start-Sleep -Seconds 3
}

Write-Output "exe: $Exe"
Write-Output ("sidecar: " + (Get-Content ([IO.Path]::ChangeExtension($Exe, 'version')) -ErrorAction SilentlyContinue))
Write-Output "bounds: $Bounds"
$pre = & $Exe --warpctrl instance list --output-format json 2>$null | ConvertFrom-Json
if ($pre -and $pre.instances -and @($pre.instances).Count -gt 0) { Write-Output "REFUSING: a Warp instance is already up"; exit 2 }

Launch 'launch 1 (fresh or restored, var set)' $true $true
Launch 'launch 2 (restored after a move, var set)' $true $true
Launch 'launch 3 (restored after a move, var unset)' $false $false
