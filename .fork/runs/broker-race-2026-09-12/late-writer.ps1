# Reproduces the credential broker race on demand: connect to the broker pipe,
# wait before writing, and see whether the broker authenticates the client.
#
# The body is `{}`, which is not a valid CredentialRequest. That is deliberate:
# the answer's error code says how far the broker got without needing a real
# action name. `unauthorized_local_client` = refused at the identity check;
# `invalid_request` "failed to decode" = the peer was authenticated and the
# payload was read. Delay 0 is the control: the write usually lands first.
param(
    [Parameter(Mandatory)] [string]$Exe,
    [string]$Profile = 'brokerrace',
    [int[]]$DelaysMs = @(0, 0, 0, 300, 300, 300),
    [switch]$KeepRunning
)
$ErrorActionPreference = 'Continue'

Write-Output "exe: $Exe"
Write-Output ("sidecar: " + (Get-Content ([IO.Path]::ChangeExtension($Exe, 'version')) -ErrorAction SilentlyContinue))

$pre = & $Exe --warpctrl instance list --output-format json 2>$null | ConvertFrom-Json
if ($pre -and $pre.instances -and @($pre.instances).Count -gt 0) {
    Write-Output "REFUSING: a Warp instance is already up"
    exit 2
}

Get-ChildItem env: | Where-Object Name -like 'WARP_FORK_*' | ForEach-Object { Remove-Item "env:$($_.Name)" }
$env:WARP_DATA_PROFILE = $Profile
$proc = Start-Process -FilePath $Exe -WorkingDirectory 'C:\dev\warp' -NoNewWindow -PassThru
Write-Output "launched pid $($proc.Id)"

$dir = Join-Path $env:LOCALAPPDATA 'warp\local-control'
$record = $null
$deadline = (Get-Date).AddSeconds(90)
while ((Get-Date) -lt $deadline -and -not $record) {
    Start-Sleep -Seconds 2
    Get-ChildItem $dir -Filter *.json -ErrorAction SilentlyContinue | ForEach-Object {
        $r = Get-Content $_.FullName -Raw | ConvertFrom-Json
        if ($r.pid -eq $proc.Id -and $r.credential_broker) { $record = $r }
    }
}
if (-not $record) { Write-Output "no discovery record with a broker for pid $($proc.Id)"; exit 1 }
$pipe = "warp-local-control\" + $record.credential_broker.socket_path
Write-Output "pipe: \\.\pipe\$pipe"
Start-Sleep -Seconds 3

function Ask([int]$delayMs) {
    $client = New-Object System.IO.Pipes.NamedPipeClientStream('.', $pipe, [System.IO.Pipes.PipeDirection]::InOut)
    try {
        $client.Connect(5000)
        Start-Sleep -Milliseconds $delayMs
        $body = [Text.Encoding]::UTF8.GetBytes('{}')
        $client.Write([BitConverter]::GetBytes([uint32]$body.Length), 0, 4)
        $client.Write($body, 0, $body.Length)
        $client.Flush()
        $len = New-Object byte[] 4
        $got = 0
        while ($got -lt 4) { $n = $client.Read($len, $got, 4 - $got); if ($n -le 0) { throw "eof in length" }; $got += $n }
        $size = [BitConverter]::ToUInt32($len, 0)
        $buf = New-Object byte[] $size
        $got = 0
        while ($got -lt $size) { $n = $client.Read($buf, $got, $size - $got); if ($n -le 0) { throw "eof in body" }; $got += $n }
        $text = [Text.Encoding]::UTF8.GetString($buf)
        $code = if ($text -match '"code"\s*:\s*"([^"]+)"') { $Matches[1] } else { '?' }
        Write-Output ("delay {0,4} ms -> {1}  {2}" -f $delayMs, $code, $text)
    } catch {
        Write-Output ("delay {0,4} ms -> transport error: {1}" -f $delayMs, $_.Exception.Message)
    } finally {
        $client.Dispose()
    }
}

foreach ($d in $DelaysMs) { Ask $d }

if (-not $KeepRunning) {
    $out = & $Exe --warpctrl window close --output-format json 2>&1 | Out-String
    Write-Output ("window close: " + $out.Trim())
    # `ok: true` above means only that the close was dispatched. A quit warning
    # (running command, shared session, unsaved code) holds it open; the scratch
    # profile's settings.toml turns that warning off.
    $deadline = (Get-Date).AddSeconds(30)
    while ((Get-Date) -lt $deadline -and -not $proc.HasExited) { Start-Sleep -Seconds 1 }
    Write-Output ("process exited: {0}" -f $proc.HasExited)
}
