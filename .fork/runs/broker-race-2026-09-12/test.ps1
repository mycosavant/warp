# Runs the Windows-only credential broker test in the app crate.
# Env setup copied from C:\dev\build.ps1; see that file for why each line exists.
param([string]$Filter = 'local_control::tests::credential_broker')
$ErrorActionPreference = 'Continue'
Set-Location C:\dev\warp
$env:CARGO_BUILD_JOBS = '8'
Remove-Item Env:GIT_RELEASE_TAG -ErrorAction SilentlyContinue
$env:PROTOC = "$env:LOCALAPPDATA\Microsoft\WinGet\Packages\Google.Protobuf_Microsoft.Winget.Source_8wekyb3d8bbwe\bin\protoc.exe"
$env:PATH = "C:\Program Files\CMake\bin;" + (Split-Path $env:PROTOC) + ";$env:PATH"
$env:LIBCLANG_PATH = 'C:\Program Files\LLVM\bin'
Write-Output "=== cargo test start $(Get-Date -Format HH:mm:ss) ==="
cargo test -p warp --lib --features gui,warp_control_cli -- $Filter 2>&1 | ForEach-Object { "$_" }
Write-Output "=== cargo exit $LASTEXITCODE at $(Get-Date -Format HH:mm:ss) ==="
