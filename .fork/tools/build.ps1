# Build the fork's Windows GUI binary.
#
# The env setup is the part that is easy to get wrong: winget's PATH changes do
# not reach an already-running shell, and a WSL-spawned powershell.exe inherits
# a stale Windows PATH, so protoc/cmake/libclang have to be pointed at
# explicitly every time. See .fork/README.md "Building on Windows".
#
# ErrorActionPreference MUST be 'Continue'. cargo writes its progress
# ("Compiling foo v1.2.3") to stderr, and under 'Stop' PowerShell turns the
# first such line into a terminating NativeCommandError and aborts the build
# about two seconds in — while still exiting 0, so it looks like it worked.
# Check $LASTEXITCODE and the binary's timestamp instead.
#
# FEATURES: `warp_control_cli` is not in app/Cargo.toml's default list, and
# without it the binary has no `--warpctrl` at all — the control plane this
# fork exists to open is simply absent. A build missing it looks fine and then
# answers "unexpected argument" to every warpctrl command, which reads like a
# broken feature rather than a missing flag.
#
# PROFILE: debug by default, because target\debug is warm. A cold release build
# is not the hour this comment used to claim: measured 2026-08-23 with no
# target\release present at all, it took 18m 48s. Pass -Release for a build to
# live in:
# `--release` turns debug_assertions off, which is what makes `UserInput`
# redact rather than write what you typed into the log.

param(
    [switch]$Release
)

$ErrorActionPreference = 'Continue'
Set-Location C:\dev\warp

# JOBS: capped, not left to cargo's one-job-per-core. A single rustc on the
# `warp` crate was sampled at 16.6 GB RSS (2026-09-04), and Windows and the
# WSL guest draw on the same 64 GB -- an uncapped build here is the half of
# the pair that took the guest down. Set here rather than typed as a
# `CARGO_BUILD_JOBS=8 cargo ...` prefix, because that prefix also defeats
# Claude Code's own `Bash(cargo:*)` allow rule and re-asks for permission.
$env:CARGO_BUILD_JOBS = '8'

# VERSION: name the commit in a sidecar, not in the environment.
# `app_version()` reads `GIT_RELEASE_TAG` at compile time and, failing that,
# `<binary>.version` beside the executable at startup. This script used to set
# the env var, and that compiled the tag into warp_core: measured 2026-09-05, a
# changed tag invalidates 55 crates, so every commit was a near-clean release
# build (twenty minutes, 41 GB peak). The sidecar is written after a successful
# build, below, and costs nothing at compile time. Never set GIT_RELEASE_TAG
# here again; it is cleared in case the shell inherited one.
#
# Unstamped, About shows the literal `v#.##.###` and `--version` says
# `<unknown>` -- that placeholder means NO version, it is not a format.
# Naming a version is safe only since fork::autoupdate_allowed landed.
$sha = (git -C C:\dev\warp rev-parse --short HEAD).Trim()
if ((git -C C:\dev\warp status --porcelain) -ne $null) { $sha = "$sha-dirty" }
$version = "v0.fork.$sha"
Remove-Item Env:GIT_RELEASE_TAG -ErrorAction SilentlyContinue
Write-Output "=== will name the build $version ==="

$env:PROTOC = "$env:LOCALAPPDATA\Microsoft\WinGet\Packages\Google.Protobuf_Microsoft.Winget.Source_8wekyb3d8bbwe\bin\protoc.exe"
$env:PATH = "C:\Program Files\CMake\bin;" + (Split-Path $env:PROTOC) + ";$env:PATH"
$env:LIBCLANG_PATH = 'C:\Program Files\LLVM\bin'

$features = 'gui,warp_control_cli'
if ($Release) {
    $profileArgs = @('--release')
    $binary = '.\target\release\warp-oss.exe'
    $profileName = 'release'
} else {
    $profileArgs = @()
    $binary = '.\target\debug\warp-oss.exe'
    $profileName = 'debug'
}

Write-Output "=== building warp-oss ($profileName, features: $features) at $(Get-Date -Format HH:mm:ss) ==="
cargo build --bin warp-oss --features $features @profileArgs 2>&1 | ForEach-Object { "$_" } | Select-Object -Last 20
$code = $LASTEXITCODE
Write-Output "=== cargo exit $code at $(Get-Date -Format HH:mm:ss) ==="

if ($code -eq 0 -and (Test-Path $binary)) {
    # After a successful build only, so a failed one leaves binary and sidecar
    # agreeing. The path is `<binary without .exe>.version`, which is what
    # `version_sidecar_path` computes from the running executable.
    $sidecar = [System.IO.Path]::ChangeExtension($binary, 'version')
    Set-Content -Path $sidecar -Value $version -NoNewline -Encoding ascii
    Write-Output "=== sidecar $sidecar = $version ==="
}

if (Test-Path $binary) {
    $f = Get-Item $binary
    Write-Output ("=== binary {0:yyyy-MM-dd HH:mm:ss} {1} bytes ===" -f $f.LastWriteTime, $f.Length)

    # Cheapest possible proof that warp_control_cli actually made it in. A
    # binary built without it treats `--warpctrl` as an unknown argument, and
    # that is otherwise only discovered later, mid-test, looking like a
    # different failure.
    & $binary --warpctrl instance list 2>&1 | Out-Null
    if ($LASTEXITCODE -eq 0) {
        Write-Output "=== warpctrl: present ==="
    } else {
        Write-Output "=== warpctrl: MISSING - check the --features line above ==="
    }
} else {
    Write-Output "=== binary MISSING ==="
}
exit $code
