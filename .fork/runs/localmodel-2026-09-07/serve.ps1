<#
.SYNOPSIS
  Start llama-server for the fork's four small AI features and, later, the
  panel agent. Written 2026-09-07 (T21).

.DESCRIPTION
  One model, all layers on the GPU, thinking switched off at the template so a
  64-token answer is the answer and not the first 64 tokens of a monologue
  (measured: with thinking on, Gemma 4 12B returned an empty `content` and a
  full `reasoning_content` on every Next Command-shaped request).

  Listens on 127.0.0.1:8080. Under mirrored networking WSL reaches this
  address too, so the Windows Warp, a WSL pane and an agent started inside the
  distribution all see the same server (measured both directions 2026-09-07).

  -Model    a .gguf under X:\models (moved off the nearly full C: on
            2026-09-07; the runtime itself stays under C:\dev\llama); default
            Gemma 4 12B UD-Q4_K_XL
  -Alias    the id the server answers to in the `model` field
  -Ctx      context tokens, total across slots (kv_unified)
  -Think    keep the model's thinking on (default off)
#>
param(
    [string]$Model = 'X:\models\gemma-4-12b-it-UD-Q4_K_XL.gguf',
    [string]$Alias = 'gemma-4-12b',
    [int]$Ctx = 12288,
    [int]$Port = 8080,
    [switch]$Think,
    [string[]]$Extra = @()
)
$ErrorActionPreference = 'Stop'
$exe = 'C:\dev\llama\b10844\llama-server.exe'
$args = @('-m', $Model, '--alias', $Alias, '--host', '127.0.0.1', '--port', "$Port",
          '-ngl', '99', '-c', "$Ctx", '-fa', 'on', '--jinja')
if (-not $Think) {
    # The JSON goes through the environment, not the argument list: a quoted
    # brace survives Start-Process's re-quoting only by accident, and the first
    # attempt reached the server as `{e` and was refused.
    $env:LLAMA_ARG_CHAT_TEMPLATE_KWARGS = '{"enable_thinking":false}'
} else {
    Remove-Item Env:LLAMA_ARG_CHAT_TEMPLATE_KWARGS -ErrorAction SilentlyContinue
}
$args += $Extra
Get-Process llama-server -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500
Start-Process -FilePath $exe -ArgumentList $args `
    -RedirectStandardError 'C:\dev\llama\server.err' `
    -RedirectStandardOutput 'C:\dev\llama\server.out' -WindowStyle Hidden
Write-Host "llama-server: $Alias on 127.0.0.1:$Port, ctx $Ctx, thinking $(if ($Think) {'on'} else {'off'})"
