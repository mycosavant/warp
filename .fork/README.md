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

## Driving Warp from an agent (`warpctrl`)

Upstream ships a complete local control plane and gates it off in public
builds. The fork opens it (see `.fork/TASKS.md` T1) and ports it to Windows.
It is the orchestration surface: an external agent can drive windows, tabs,
panes, sessions and the input buffer of a running instance.

There is no separate binary — the app binary enters control mode via a hidden
flag, and everything after `--warpctrl` is parsed as `warpctrl`:

```powershell
.\target\debug\warp-oss.exe --warpctrl instance list
.\target\debug\warp-oss.exe --warpctrl app ping
```

For day-to-day use, wrap it:

```powershell
function warpctrl { & 'C:\dev\warp\target\debug\warp-oss.exe' --warpctrl @args }
```

### What it can do

85 actions, all implemented. `warpctrl action list` emits the full catalog as
JSON with `parameter_spec`, `result_spec` and `target_scope` per action, so
tool definitions can be generated from it rather than hardcoded.

```
app       ping version active focus
window    list inspect create focus close
tab       list inspect create activate move close rename reset_name color.*
pane      list inspect split focus navigate resize maximize unmaximize close rename
session   list inspect activate next previous reopen_closed
input     insert replace submit
surface   settings.open command_palette.open ai_assistant.toggle warp_drive.* ... (20)
setting   list get set toggle
theme     list get set dark.set light.set system.set
appearance get zoom.* font_size.*
keybinding list get
file      open
```

`input insert` and `input replace` stage text without running it; **`input
submit` runs it** — that one is a fork addition, because without it an agent
can type but never execute. All three reject newlines and control characters,
so one call runs exactly one command and nothing can be smuggled in behind it.
`submit` returns an error rather than a false acknowledgement when the target
pane is busy.

Mutations need a focused window with a workspace. `app focus` first if
`window list` reports `is_active: false`.

### Enablement

Two gates, both opened by fork policy, both still overridable:

* `FeatureFlag::WarpControlCli` — forced on in `app/src/fork.rs`.
* Settings → Scripting — defaults to Enabled via
  `settings::local_control::effective_default_mode`. Stored in secure
  storage, so an explicit choice there still wins.

Set `WARP_FORK_POLICY=0` to get stock upstream behaviour (both off).

### Security model

Not a remote surface. Three boundaries, all local:

1. A discovery record in `%LOCALAPPDATA%\warp\local-control` (Windows) or
   `$XDG_RUNTIME_DIR/warp/local-control` (Unix), owner-only, containing
   routing metadata and **never a token**.
2. An owner-authenticated credential broker — a 0600 Unix socket checked
   against the kernel-reported peer UID, or on Windows a named pipe with a
   protected DACL whose client is impersonated and compared by token user SID.
   It mints short-lived, instance-bound, single-action bearer grants held only
   in process memory.
3. A loopback HTTP endpoint that rejects browser `Origin`, requires an exact
   `Host`, and validates the grant's existence, expiry, instance and scope.

The broker authenticates the **OS account, not the application**: anything
already running as you is inside the boundary. Enabling local control grants
nothing to another user, and nothing to the network.

Verify the Windows ACL is owner-only with:

```powershell
icacls "$env:LOCALAPPDATA\warp\local-control"
# expect exactly: <domain>\<user>:(F)  -- no SYSTEM, no Administrators
```

### Driving it from Claude Code (MCP)

`warpctrl mcp` serves the whole catalog to an MCP client over stdio. Register
it once:

```bash
claude mcp add warp -- C:\\dev\\warp\\target\\debug\\warp-oss.exe --warpctrl mcp
```

Every implemented action becomes one tool, named `warp_<action>` with dots
replaced by underscores — `tab.create` becomes `warp_tab_create`. Tools are
generated from the catalog, so adding an action publishes a tool with no
further work.

The typical sequence an agent follows:

```
warp_instance_list      -> confirm an instance is reachable
warp_app_focus          -> mutations need a focused window with a workspace
warp_tab_create         -> optional, gives the agent its own tab
warp_input_submit       -> run a command
```

`warp_input_submit` returns `executed: true` when the command ran immediately,
or `queued: true` when the pane's shell is still starting — a freshly created
tab is the common case. A queued command runs as soon as the pane is ready, so
wait before reading its output rather than resubmitting.

Failures come back as tool results with `isError` rather than transport
errors, carrying the local-control error code so the cause is actionable:
`missing_target` means focus a window first, `local_control_disabled` means
Scripting is off.

Note the server talks JSON-RPC on stdout — run it only via an MCP client, not
interactively. Diagnostics go to stderr.

### Platform status

Working on Linux/macOS (upstream) and Windows (fork port). Under WSL2 the
process runs and read actions work, but the window never composites, so it has
no workspace and mutations fail with `missing_target`. Use the Windows build.

## Voice input, transcribed on this machine

Upstream sends your voice to `api.warp.dev`. The provider setting
(`Wispr` | `OpenAI`) picks *Warp's* upstream vendor, not where inference runs,
so neither value keeps audio local — "Provider: OpenAI" is not an escape hatch.

Under fork policy `LocalTranscriber` replaces that path entirely. It is
**fail-closed**: when it is installed it is the only transcriber, and a
misconfiguration is an error, never a quiet fall back to the server. If it were
a fallback, the failure it hid would be exactly the one that matters.

### Setting it up

Everything lives under `agents.voice.local_transcription` in `settings.toml`
(`%LOCALAPPDATA%\warp\WarpOss\config\settings.toml` on Windows). The defaults
already point at a stock `whisper-server`, so with one running you need no
configuration at all.

    [agents.voice.local_transcription]
    backend  = "http"                                # or "command"
    endpoint = "http://127.0.0.1:8080/inference"     # whisper.cpp's default
    model    = ""                                    # required by OpenAI-shaped servers
    command  = ""                                    # backend = "command" only
    command_args = "--model {model} --language {language} --no-timestamps --file {audio}"

**You already have a whisper server.** OpenWhispr ships whisper.cpp's
`whisper-server` and a base model:

    C:\Users\<you>\AppData\Local\Programs\OpenWhispr\resources\bin\whisper-server-win32-x64.exe
    C:\Users\<you>\.cache\openwhispr\whisper-models\ggml-base.bin

Run it standalone and Warp will use it:

    whisper-server-win32-x64.exe -m "%USERPROFILE%\.cache\openwhispr\whisper-models\ggml-base.bin" --port 8080

### Which endpoint, and why the whole URL is a setting

whisper.cpp and the OpenAI-compatible servers agree on the request
(`multipart/form-data`, field `file`) and on the reply (`{"text": ...}`). They
disagree only on the route — measured, not assumed:

    POST /inference                -> {"text":" List the files in this directory.\n"}
    POST /v1/audio/transcriptions  -> 404 File Not Found

So point `endpoint` at `/inference` for whisper.cpp and
`/v1/audio/transcriptions` for speaches, faster-whisper-server or LocalAI —
those also need `model` set, which whisper.cpp ignores.

### The `command` backend

For a transcriber with no server. The recording is written to a `0600`
temporary file, the binary runs, and **stdout is the transcript** — which is
why whisper-cli needs `--no-timestamps`; without it the transcript goes to a
file and Warp sees silence (the error says so).

Arguments are split on whitespace *before* `{audio}`, `{model}` and
`{language}` are substituted, so a value containing spaces stays one argument.
`{language}` becomes `auto` when no language is set, rather than dropping the
argument and leaving a dangling `--language`.

### Verifying it yourself

There is an `#[ignore]`d end-to-end test that drives the real HTTP path:

    WARP_VOICE_TEST_ENDPOINT=http://127.0.0.1:8080/inference \
    WARP_VOICE_TEST_WAV=/mnt/c/dev/speech16k.wav \
      cargo test -p warp --lib transcribes_a_real_recording -- --ignored --nocapture

`speech16k.wav` is a 16 kHz mono sample generated with Windows SAPI — the same
format `voice_input` produces. Warp resamples to 16 kHz mono for the same
reason whisper wants it.

What this does *not* prove is the microphone-to-transcript path end to end;
that needs someone to speak into it. Worth doing once with a proxy running to
confirm nothing reaches `api.warp.dev`.

## The four small AI features, without Warp in the middle

Next Command, Prompt Suggestions, Shared Block Title Generation and Commit & PR
Generation are each one `POST` to `api.warp.dev/ai/*` — a JSON body, a JSON
reply, no streaming, no session state. Warp's server is a bearer-authenticated
proxy in front of a model, which is why these four can be re-pointed without
touching the agent.

Under fork policy they go to a model you control instead. Like voice, this is
**fail-closed**: even unconfigured they never reach `api.warp.dev`. The reason
is the payloads. Between them these four carry terminal output plus the command
that produced it, your working directory and recent shell history, and an
entire working-tree diff. The account gates are already bypassed, so without
this you could flip a toggle and quietly resume shipping all of that upstream.

Unconfigured, you get an error naming the setting to fill in.

### Setting it up

**No key or URL goes in `settings.toml`** — that file is plaintext. Both come
from Warp's own Custom Inference storage, which uses the OS keychain and already
has an editor:

> Settings → Warp Agent → Custom Inference → add an endpoint

Give it a URL, a key and at least one model. That is the whole setup. A pasted
Anthropic, OpenAI or OpenRouter key on the same page works too, with no endpoint
at all.

`settings.toml` only chooses among what is stored there:

    [agents.local_ai]
    endpoint = ""      # Custom Inference endpoint name; empty = use the first
    model    = ""      # empty = the endpoint's first model

    [agents.local_ai.models]   # per-feature overrides; empty = agents.local_ai.model
    next_command       = ""    # fires on nearly every prompt — go small and fast
    prompt_suggestions = ""
    block_title        = ""
    code_review        = ""    # reads a whole diff — a bigger model pays off here

Resolution order, most explicit first: the endpoint named above → the first
configured endpoint → an Anthropic key → an OpenAI key → an OpenRouter key. A
*named* endpoint that does not exist is an error rather than a fall-through to
some other provider, because falling through would send the payload somewhere
you did not pick.

Google is not in that chain on purpose: the Gemini API is not OpenAI-shaped at
its documented endpoint, so a Google key needs an explicit Custom Inference
entry pointing at a compatibility route — better than a guess made here that
fails at request time.

### Local, meaning issued from this machine

Not necessarily inferred on it. The same path serves a llama.cpp server on
loopback and `api.anthropic.com` with your own key. What both have in common —
and the whole point — is that Warp is not in the middle. For a fully on-device
setup, point a Custom Inference endpoint at whatever you already run:

    http://127.0.0.1:11434/v1/chat/completions    # Ollama
    http://127.0.0.1:8080/v1/chat/completions     # llama.cpp / LM Studio

Protocol comes from the endpoint's schema dropdown — OpenAI Chat Completions,
OpenAI Responses, or Anthropic Messages. All three are implemented.

### Trying it

    # in a repo with uncommitted changes
    Commit & PR generation  ->  a commit message from your endpoint

A wrong model name comes back as the provider's own 404, which names the model;
set `agents.local_ai.model` to fix it. An endpoint that is not listening comes
back naming the URL it could not reach.

This part has *not* been verified against a real provider — there was no key or
local LLM available to test with. The request shape is asserted field by field
against a stub server, but a stub agrees with whatever it is told. One real
request is worth more than that whole test file.

## Warp Drive without an account

Warp Drive is backed by a real local SQLite store (`crates/cloud_object_persistence`,
diesel + bundled sqlite3). Warp's server is a **sync layer on top** of it, not
the storage. So the fork does not have to replace anything — the store already
works, and with no account nothing is ever sent from it.

What was missing is that upstream treats that store as a *cache*. Several
things wait for the server to confirm the cache before using it, and with no
account they wait forever.

### What changed

Four things, all conditional on fork policy and reversible with
`WARP_FORK_POLICY=0`:

* **The drive is writable.** Creating any object needs an `Owner`, which
  upstream derives from the signed-in user and leaves as `None` when there
  isn't one. Objects created without an account are owned by a fixed local
  identity instead.
* **Nothing waits for a sync that is not coming.** Upstream marks the initial
  load complete only after a successful server fetch. 24 places await that
  before doing their work — the Warp Drive spinner, `warp mcp list`, execution
  profiles, environments. With no account the local store *is* the load, so
  they proceed against it.
* **Logging out no longer deletes the store.** Upstream removes the sqlite
  database on logout, which is safe when its contents are copies of
  server-owned objects and is data loss once they are the originals.
* **No false offline banner.** "You are offline. Some files will be read only."
  is about the network, not the account. With no account nothing becomes read
  only when the network drops, so the banner is suppressed.

### It does not sync, and that is enforced

With no account nothing is sent — but that was already true upstream, as an
accident of ordering rather than a promise. The sync queue only starts draining
after a *successful* server fetch, so items simply piled up unsent.

Locally owned objects are now refused by the sync queue outright, and filtered
out of the queue that is rebuilt at startup. Without both, the first time you
added an account those objects would have been pushed to Warp's server under a
user id it has never heard of.

Sign in and normal syncing resumes for everything owned by that account.
Objects created while account-free stay local — they are owned by an identity
the server does not know, which is the point.

### Where your objects live

On Windows, confirmed by inspection:

    %LOCALAPPDATA%\warp\WarpOss\data\warp.sqlite

The directory is resolved at runtime rather than fixed —
`persistence::sqlite::database_file_path_for_scope` picks a per-scope path, so
the GUI, the TUI and the remote-server daemon never share a database, and a
secure container directory is used where the platform has one. On another
platform or channel, read it off `app_database_file_path` rather than assuming
the path above.

Note the store is `WAL`-mode: `warp.sqlite-wal` holds recent writes and can be
much larger than the database itself. Copy all three files (`.sqlite`, `-wal`,
`-shm`) if you want to inspect it, or the objects written this session will
appear to be missing.

Objects show a laptop icon reading **"Saved locally"** rather than a sync
spinner. That is upstream's own indicator for "changed locally, queue not
draining" — under local-first it is simply always the accurate one.

The local owner is the constant `"local"` on every machine, deliberately, so a
store stays in your Personal space after it moves between machines. A
per-machine identity would file a copied store under "Shared with me". This is
what makes T4.4 (git-backed sync) possible without rewriting ownership.

### Verified in a running GUI

Confirmed on the Windows build, 2026-08-18: Warp Drive renders its contents
rather than a perpetual spinner, a workflow created with no account is still
there after a restart, it sits under **PERSONAL** with the "Saved locally"
icon, and its alias survives.

That run was worth doing. It caught a bug every unit test had passed over:
locally-created objects were coming back under **"Shared with me"**. T4.2
taught `personal_drive` to *write* the local sentinel as owner but left
`owner_to_space` *reading* `AuthStateProvider::user_id()` directly. Signed in
those agree; account-free they do not, because `user_id()` is `None` while the
sentinel is not. The tests covered the writing side and the reading side, each
correctly — the defect lived in the *agreement between them*, which is not
somewhere a unit test naturally looks. Both sides now resolve through
`personal_drive`.

## Your drive as a git repository

The store has a portable on-disk form: a whole Warp Drive materializes into a
directory you can keep in your own repository, and reads back with its object
graph intact — identities, folder hierarchy and all.

    <root>/
      deploy-1f0e3a2b.json           a workflow
      field-notes-8c4d1e07.md        a notebook: front matter + markdown
      scripts-3b7a9f21/              a folder is a directory
        .warp-folder.json
        test-5e2c8d40.json

**You drive git, not Warp.** Warp reads and writes the directory; you run
`git commit`, `git pull`, and resolve conflicts with the tools you already
have. That is the whole reason this is not a sync engine: a merge over a graph
of objects with identities is precisely the machinery this fork exists to
remove, and git already does the job on text.

**SQLite stays authoritative; the tree is a mirror.** One source of truth plus
a projection needs only a rule for which side wins, and the rule is that
nothing happens implicitly.

Two properties are worth knowing because they constrain everything else:

- **An unchanged object produces unchanged bytes.** Otherwise `git status` is
  permanently dirty and the repository is useless as a sync target. This is why
  the format does not carry `folders.is_open` — expanding a folder in the
  sidebar would dirty the repo — nor the local SQLite row ids, nor any sync
  bookkeeping.
- **An export never touches a file it did not write.** Your README, your notes,
  your `.git` are safe: a file is deleted only after being read and recognised
  as one Warp wrote, a directory only once it is empty, and dot-directories are
  never entered.

Trashed objects are exported, with their timestamp — emptying the trash is your
decision, and an export that pre-empted it would take the undo away.

### Running an export

Set the destination in `settings.toml` — there is no GUI control for it yet:

    [warp_drive.local_sync]
    path = "C:\\dev\\my-warp-drive"    # absolute; empty or absent = disabled

Then drive it from `warpctrl`:

    warpctrl drive status    # where it would go, and what would go there
    warpctrl drive export    # write it

`status` writes nothing, not even the directory, and reports an unset path
rather than erroring — it is the command you run to find out *why* an export
will not run. Both are MCP tools too (`warp_drive_sync_status`,
`warp_drive_sync_export`), so an agent can run them.

**The path is settings-only on purpose.** `warpctrl setting set` is gated by an
allowlist and this key is deliberately not on it, so an agent can ask for an
export but cannot choose the directory that gets pruned. Empty, relative,
filesystem-root and not-a-directory destinations are all refused before
anything is read or written.

Verified on Windows against a real repository: `git init` in the mirror, a
README, a `notes.json` and a `my-notes/todo.md` alongside the exported
workflow, then commit and export twice more — `removed_files: 0`,
`git status --porcelain` empty, `.git` intact.

**Known gap: workflow aliases do not travel.** An alias is not a drive object —
`WorkflowAliases` is a settings group holding `alias` plus the `workflow_id` it
points at. So a workflow arrives on another machine without its alias. The
format already preserves the id the alias refers to, so this is fixable; see
`.fork/TASKS.md` T4.4g.

Not yet wired: nothing applies an imported tree back into the store, so this is
currently a one-way mirror. See `.fork/TASKS.md` T4.4f.

## Driving the Windows build from WSL

Written down 2026-08-18 after the original working session was lost to a
cleared context. The capability had been rebuilt from scratch twice by then;
the scripts referenced here exist so it does not have to be a third time.

The GUI only works on Windows (WSLg never composites a window, so the Linux
build has no workspace and every mutating `warpctrl` action fails with
`missing_target`). But an agent running in WSL can drive that Windows build
end to end, because WSL interop makes `powershell.exe` an ordinary executable:

```bash
powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'C:\dev\shot.ps1' -Out 'C:\dev\shots\x.png'
```

Anything under `/mnt/c` is visible to both sides, so scripts, screenshots and
proof files pass between them as plain files. No SSH, no agent, no daemon.

### The three scripts

| Script | What it does |
|---|---|
| `C:\dev\build.ps1` | Builds `warp-oss.exe` with the env that winget's PATH changes never reach. |
| `C:\dev\shot.ps1`  | Screenshots a window (or the whole virtual screen) to PNG. |
| `C:\dev\mcp_win*.ps1` | Drives a running instance over MCP, batched. |

**`$ErrorActionPreference` must be `Continue` in any script that runs cargo.**
Cargo writes its progress (`Compiling foo v1.2.3`) to stderr, and under `Stop`
PowerShell promotes the first such line to a terminating `NativeCommandError`
and aborts about two seconds in — *while still exiting 0*. It looks exactly
like a successful incremental build that had nothing to do. Check
`$LASTEXITCODE` and the binary's timestamp, not the exit status of the script.
This cost a full cycle before it was spotted.

`shot.ps1` takes `-Process warp-oss` to capture just Warp's window after
raising it, `-Scale` to shrink the output, and `-DelaySeconds` to wait for the
UI to settle. It calls `SetProcessDPIAware` first — without that, the window
rect and the captured pixels disagree on a scaled display and the grab lands
in the wrong place. It falls back to the whole virtual screen when the process
has no `MainWindowHandle` yet, which is the normal state during early startup.

A full-screen grab at this display's 2560x1440 is around 4 MB; `-Scale 0.5`
brings it to roughly 800 KB, which is still legible for checking whether a
panel rendered.

### Driving it without an MCP client

`warpctrl mcp` speaks newline-delimited JSON-RPC on stdio, so a batch of calls
is just a file piped into it. This needs no MCP client at all and is the
easiest way to script a verification:

```powershell
$msgs = @(
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}'
  '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"warp_app_focus","arguments":{}}}'
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"warp_tab_create","arguments":{}}}'
  '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"warp_input_submit","arguments":{"text":"..."}}}'
)
$msgs | Out-File C:\dev\mcp_in.jsonl -Encoding ascii
Get-Content C:\dev\mcp_in.jsonl | & .\target\debug\warp-oss.exe --warpctrl mcp 2>$null
```

Write the messages as `-Encoding ascii`. PowerShell's default adds a BOM, and
the first JSON-RPC line then fails to parse.

Single actions do not need the MCP framing at all — `--warpctrl input submit
'...'` is enough, which is what `C:\dev\proof.ps1` does.

### Proving a command actually ran

The pattern used throughout T1: submit a command that writes a file, then look
for the file. It is the only check that cannot be satisfied by an
acknowledgement that lies.

```powershell
.\target\debug\warp-oss.exe --warpctrl input submit 'Set-Content -Path C:\dev\proof.txt -Value RAN'
Start-Sleep -Seconds 6
Test-Path C:\dev\proof.txt
```

Sleep before checking. `input.submit` returns `queued: true` rather than
`executed: true` when the pane's shell is still starting, and a freshly created
tab is the common case — the command runs a moment later.

### Shut it down with CloseMainWindow, not Kill

```powershell
(Get-Process warp-oss).CloseMainWindow()
```

`Kill()` skips the cleanup that removes the local-control discovery record, so
the next `instance list` reports `ambiguous_instance` against an instance that
no longer exists. Warp also spawns a crash-recovery sibling that re-binds
`127.0.0.1:9282` and respawns a terminal server when the parent dies, so a
killed process leaves the port held and the next launch fails to bind. This is
also the most likely cause of the `.recovery` log-file mystery recorded under
T2.
