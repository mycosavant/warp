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
