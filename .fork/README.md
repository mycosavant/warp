# Personal Warp fork — operating manual

Fork of `warpdotdev/warp` (dual-licensed AGPL-3.0-only / MIT). Goal: a Warp
client with no telemetry, no account requirement, and agents driven by my own
Claude subscription, API keys, and local models.

Licensing note: AGPL obligations attach on **distribution**, not personal use.
If this fork is ever published as a binary, source must ship with it.

## Branch topology

| Branch          | Role                                                             |
|-----------------|------------------------------------------------------------------|
| `master`        | Pristine mirror of `upstream/master`. Never commit here.          |
| `sync/upstream` | Scratch branch where upstream merges are resolved and evaluated.  |
| `dev`           | Integration branch for this fork. All fork work lands here.       |

Remotes: `origin` = `mycosavant/warp` (my fork), `upstream` = `warpdotdev/warp`.

### Sync workflow

```bash
git fetch upstream
git checkout master && git merge --ff-only upstream/master   # master stays pristine
git checkout sync/upstream && git reset --hard master        # scratch = new upstream
git checkout dev && git merge sync/upstream                  # resolve here, not on master
```

Because every fork-authored file lives in paths upstream does not use
(`.fork/`, and new `warp_fork_*` crates), merges should only conflict where a
fork change deliberately edits an upstream file. Keep those edits minimal —
that is the entire point of the kill-switch design in `SPEC.md`.

## Repo hygiene — resolved 2026-08-17

The checkout had been written by a **Windows git** through the `\\wsl$` share
while being read by WSL's git. That produced three simultaneous corruptions,
which together showed up as 5,894 spuriously modified files:

1. **CRLF line endings** on 5,894 files (`core.autocrlf` unset on the Linux side)
2. **`.claude/skills` symlink** materialized as a 17-byte regular file
   containing `../.agents/skills` — which silently hid all ~30 of Warp's repo
   skills from Claude Code
3. **64 exec bits dropped** (`statusline.sh`, CI entrypoints, build scripts)

Fixed by pinning repo-local config and doing a pristine re-checkout. The config
is pinned **repo-locally** (`.git/config`), so it now holds regardless of which
git binary touches this working tree:

```
core.autocrlf=false  core.eol=lf  core.symlinks=true  core.filemode=true
```

### The `.claude/skills` symlink is *not* a Windows↔WSL bridge

Worth stating plainly, because it's easy to assume otherwise: `.claude/skills`
is a **relative, repo-internal** symlink (`-> ../.agents/skills`) committed in
Warp's own git tree as mode `120000`. It exists so Claude Code finds Warp's
in-repo skills at the path it expects. It has nothing to do with bridging
Windows and WSL. It was simply collateral damage from the Windows checkout.

### Warp's shell bootstrap (the `source ~/.bashrc` garbling)

Two separate pieces, often confused:

1. **Persistent**, in `~/.bashrc:224-225`, added by Warp — "Auto-Warpify":
   ```bash
   [[ "$-" == *i* ]] && printf 'P$f{"hook": "SourcedRcFileForWarp", ...}'
   ```
   It emits a DCS escape announcing to Warp that an rc file was sourced. The
   escape bytes are non-printable, which is why it looks mangled when echoed.

2. **Runtime**, *not* in any file — the large
   `[ -z $WARP_BOOTSTRAPPED ] && eval '...'` blob. Warp **injects this into the
   PTY** in response to hook (1). It sets `WARP_SESSION_ID`, sends an
   `InitShell` hook as hex-encoded JSON over OSC `9278`, and runs
   `command -p stty raw`.

So sourcing `.bashrc` under a non-Warp foreground process (e.g. Claude Code)
fires hook (1), Warp injects (2), and because nothing is there to *consume* the
escape sequences they get echoed literally. The `stty raw` is what causes the
stair-stepped output — raw mode disables newline translation. Recover with
`stty sane`.

Note `WARP_USING_WINDOWS_CON_PTY=true` in that blob: this is **Windows Warp
driving a WSL2 shell over ConPTY** — direct evidence of the existing
Windows↔WSL integration, and the natural starting point for improving it.

This session-hook channel is also telemetry-adjacent and is in scope for the
Phase 1 kill switch.

### The Warp Claude Code plugin

Lives at `~/git/warp-claude-plugin` — already forked
(`origin` = `mycosavant/claude-code-warp`, `upstream` = `warpdotdev/claude-code-warp`).
Marketplace `claude-code-warp` ships **two** plugins:

- **`warp`** (v2.2.0) — native Warp notifications when Claude finishes or needs
  input. Pure shell hooks (`on-stop`, `on-notification`,
  `on-permission-request`, `on-prompt-submit`, `on-post-tool-use`,
  `on-session-start`) that emit terminal escape sequences. **Keep and
  customize** — no account required, genuinely useful.
- **`oz-harness-support`** (v1.1.2) — binds Claude Code to Warp's **Oz cloud
  agent** infrastructure (parent-message delivery, `oz-*` skills). **Out of
  scope / to be replaced** by local orchestration, since Oz is the paid cloud
  service this fork is moving away from.

It is installed on the **Windows-side** Claude Code, not in this WSL
environment (`~/.claude/plugins/installed_plugins.json` has no warp entry).
That asymmetry is itself an argument for the WSL-integration work.

## Building on Windows (the working GUI path)

**Status: verified.** `warp-oss.exe` builds in ~8 minutes and renders a real
native window. This is the recommended way to actually *use* the fork; the
WSL2 build compiles and runs but its window never reaches the desktop (below).

Checkout lives at `C:\dev\warp`, built from the WSL repo so nothing has to be
pushed to GitHub.

### Prerequisites

Already present here: Visual Studio, Rust 1.92.0 MSVC (rustup auto-syncs to
`rust-toolchain.toml`), Git. Installed via winget:

```powershell
winget install -e --id Google.Protobuf   # protoc, required by prost-build
winget install -e --id Kitware.CMake
winget install -e --id LLVM.LLVM         # libclang, for bindgen
```

Warp's own `script/windows/bootstrap.ps1` installs these plus VS 2022 Build
Tools and InnoSetup; the three above are enough if VS and Rust already exist.

**winget's PATH changes do not reach an already-running shell**, and a
WSL-spawned `powershell.exe` inherits a stale Windows PATH. Set them
explicitly for the build.

### Cloning — three traps, all hit on the first attempt

1. **`git -c` vs `git clone -c`.** `git -c core.autocrlf=false clone` applies
   the setting to that invocation only; it is *not* written to the new repo.
   The result was 6,281 CRLF-modified files — the same mess as the original
   WSL checkout. Use `git clone -c ...`, which does persist, or set the config
   and renormalize afterwards.
2. **`core.symlinks` must be `false` on Windows** unless Developer Mode is on.
   With it `true`, checkout of `.claude/skills` fails and takes the whole index
   with it: `fatal: Could not reset index file to revision 'HEAD'`, leaving
   `git ls-files` empty. Setting it `false` fixed it immediately.
   Consequence: `.claude/skills` is a plain file on Windows, so Warp's in-repo
   skills do **not** resolve for Claude Code there. Enable Developer Mode if
   you want them.
3. **LFS objects are not in the WSL repo.** The WSL clone has the real
   binaries in its *worktree* but an empty LFS object store, so cloning from it
   fails with `smudge filter lfs failed`. Point LFS at GitHub instead:
   `git config lfs.url https://github.com/warpdotdev/warp.git/info/lfs`.

Working sequence:

```powershell
git clone -c core.autocrlf=false -c core.eol=lf -c core.symlinks=false `
  --branch dev \\wsl.localhost\Ubuntu\home\effatha\git\warp C:\dev\warp
cd C:\dev\warp
git config lfs.url https://github.com/warpdotdev/warp.git/info/lfs
git lfs pull
git reset --hard HEAD          # expect: 0 modified files afterwards
```

### Build and run

```powershell
$env:PROTOC = "$env:LOCALAPPDATA\Microsoft\WinGet\Packages\Google.Protobuf_Microsoft.Winget.Source_8wekyb3d8bbwe\bin\protoc.exe"
$env:PATH = "C:\Program Files\CMake\bin;" + (Split-Path $env:PROTOC) + ";$env:PATH"
$env:LIBCLANG_PATH = 'C:\Program Files\LLVM\bin'
cargo build --bin warp-oss --features gui
.\target\debug\warp-oss.exe
```

~8 GB of build artifacts. Verified: fork markers present in the binary, zero
Sentry symbols, real `MainWindowHandle`, onboarding renders.

## Running under WSL2 (WSLg)

**Use this:**

```bash
env -u WAYLAND_DISPLAY LIBGL_ALWAYS_SOFTWARE=1 ./target/debug/warp-oss
```

Verified working — renders the full UI correctly.

Two independent WSLg problems, each with its own symptom:

**1. Wayland presents nothing.** With `WAYLAND_DISPLAY` set, winit picks the
Wayland backend (`Running app with windowing system: Wayland`). The window is
genuinely created — the log shows `window resized` and
`active window changed: Some(WindowId(0))` — but never paints, so it appears in
alt-tab and the taskbar as a blank grey rectangle, with focus thrashing
between `Some(WindowId(0))` and `None`. Likely cause: `sctk_adwaita`
client-side decorations failing, visible as
`XDG Settings Portal did not return response in time`.

Unsetting `WAYLAND_DISPLAY` routes through Xwayland instead
(`windowing system: X11`) and the window renders. Under X11 you can confirm it
directly, which is impossible for a Wayland client:

```bash
xwininfo -root -tree | grep -i warp
# 0x600003 "Warp": ("dev.warp.WarpOss" ...)  1182x738+32+32  +2142+327
```

**2. GPU passthrough fails.** Without `LIBGL_ALWAYS_SOFTWARE=1` the log fills
with `MESA: error: ZINK: failed to choose pdev` and
`libEGL: failed to create dri2 screen`. With it, the log is clean and wgpu
selects `Vulkan Cpu (llvmpipe)` — software rendering, but correct.

### Stale instances hold port 9282

Warp binds `127.0.0.1:9282` and spawns a crash-recovery sibling
(`--crash-recovery-mechanism=force-dedicated-gpu`). **Killing the main PID is
not enough** — the recovery child re-binds the port and respawns a terminal
server, and the next launch fails with
`Failed to bind local HTTP server on 127.0.0.1:9282: Address already in use`.

Kill the whole family before relaunching:

```bash
pkill -f 'debug/warp-oss'; sleep 2; ss -tlnp | grep 9282   # expect no output
```

### Harmless startup noise in this environment

`org.freedesktop.secrets was not provided` (no keyring), `portal.Settings`
missing (no XDG desktop portal), and the clipboard falling back from
`ext-data-control` to X11. None affect the fork.

## Local telemetry (OpenTelemetry)

Upstream already ships an OTLP/HTTP exporter but locks it to cloud-agent runs:
it demands a `WARP_CLOUD_AGENT_OTLP_TOKEN` dispatch credential, and its span
filter drops everything not tagged `tags.cloud_agent`. The fork removes both
obstacles **for loopback endpoints only**.

Start any OTLP collector, e.g. Jaeger:

```bash
docker run --rm -p 4318:4318 -p 16686:16686 jaegertracing/all-in-one:latest
```

Then run Warp pointed at it:

```bash
WARP_CLOUD_AGENT_OTLP_ENDPOINT=http://localhost:4318 \
OTEL_SERVICE_NAME=warp-fork \
  ./target/debug/warp
```

Traces appear at <http://localhost:16686>. Use `RUST_LOG` to widen beyond the
`INFO` default — upstream picked `INFO` because only marked spans were
exported, so `RUST_LOG=warp=debug` is now considerably more expensive.

Agent and harness spans come for free: `ai/agent_sdk/setup_observability.rs`
already emits `setup_environment_resolution`, `..._repo_clone`,
`..._setup_commands`, `..._codebase_indexing` and `..._skill_loading`, and
`tracing-opentelemetry` bridges them into OTLP.

**Safety properties, both covered by tests in `app/src/tracing/native_tests.rs`:**

- Authentication is dropped *only* when the endpoint host is loopback
  (`localhost`, `127.0.0.1`, `::1`). Lookalikes such as `localhost.evil.com`
  and `127.0.0.1.evil.com` are correctly treated as remote.
- A malformed endpoint is treated as non-loopback, so it falls back to the
  authenticated path rather than silently exporting without a credential.
- Plain `http` remains rejected for non-loopback hosts, so the local-export
  affordance cannot leak traces unencrypted to a remote collector.

Export stays **opt-in and off by default**: with `WARP_CLOUD_AGENT_OTLP_ENDPOINT`
unset, `init` installs a no-op subscriber and nothing is collected or sent.
The Phase 1 egress deny-list deliberately does not block loopback, so the
collector is unaffected.

### git-lfs — resolved

`git-lfs` (3.4.1) is now installed. Its four hooks in `.git/hooks/`
(`post-commit`, `post-checkout`, `post-merge`, `pre-push`) had **also** lost
their exec bits to the Windows corruption — and because `.git/hooks/` is not
tracked by git, the re-checkout could not repair them. Fixed with `chmod +x`.

The 7 LFS binaries (4 Windows `.pdb`, 3 `bert_tiny_*.onnx`, ~124 MB) then still
showed as modified purely because the index predated the LFS filters. No
download was needed — the content was already correct (`git lfs status` reported
matching OIDs, e.g. `LFS: 28217b2 -> File: 28217b2`). Cleared with:

```bash
git add --renormalize .
```

**Working tree is now completely clean: 5,894 → 0 modified files.**

A backup of the LFS content remains at `~/.warp-lfs-backup`; it can be deleted
once you're confident, but it costs nothing to keep.
