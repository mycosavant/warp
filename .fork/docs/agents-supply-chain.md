# ACP agents: where their code comes from

As of 2026-09-14. Decision: `decisions/2026-09-13-acp-agents-are-not-launched-through-a-package-manager.md`.
Work: `HANDOFF-SUPPLYCHAIN.md`. Measurement: `runs/supplychain-2026-09-14/`.
Claims are tagged RAN, READ, TOLD or ASSUMED.

| agent | state |
|---|---|
| `claude-agent-acp` 0.73.0 | **locked and installed without a package manager**, measured on Linux and through the Windows build |
| `codex-acp` 0.16.0 | surveyed, not locked |
| `opencode` 1.18.25 | surveyed, not locked |

**For the agent this fork recommends, the part that talks to the model and
runs the shell is a closed binary.** `claude-agent-acp` is an Apache-2.0
adapter over Anthropic's agent SDK, and the SDK ships a 215 MB `claude`
executable under *"© Anthropic PBC. All rights reserved"* (READ, its
`LICENSE.md`). The lock stops a swapped file **at install**; nothing re-checks
the installed tree afterwards. It cannot show what the file does either; only a census of its traffic can, which is why every bump
re-runs `runs/vendorcalls-2026-09-13/instruments/`.

## Installing

```bash
python3 .fork/tools/agents.py fetch claude-agent-acp     # network: download, check each sha256
python3 .fork/tools/agents.py install claude-agent-acp   # no network: check again, extract
```

It needs Python 3.12 or newer (`tomllib`, and `tarfile`'s extraction filter)
and refuses to run on anything older. Then name the launcher it prints:

```bash
# Linux, or .fork/launch.sh, which does this and refuses to start without it
WARP_FORK_ACP_COMMAND="$HOME/.local/share/warp-fork/agents/claude-agent-acp/0.73.0/bin/claude-agent-acp"
```

```powershell
# Windows build, agent inside WSL; warpdev.ps1 asks the lock for the path
$env:WARP_FORK_ACP_COMMAND = 'wsl.exe -d Ubuntu -- /home/<you>/.local/share/warp-fork/agents/claude-agent-acp/0.73.0/bin/claude-agent-acp'
```

On Windows, in a WSL pane, the bare `'$HOME/…'` form works too: Warp wraps a
command that does not start with `wsl.exe` in `wsl.exe --exec /bin/sh -lc`
(READ, `acp_agent::agent_argv`), and `sh` expands `$HOME` inside the
distribution (RAN, attempt 4 of the run). The explicit `wsl.exe` form also
starts the agent inside WSL from a PowerShell pane, where the Windows cwd
makes it refuse the session anyway (RAN, attempt 1).

What `install` does (READ, `.fork/tools/agents.py`):

- Checks every cached file against the lock and refuses on any mismatch.
- Extracts the tarballs itself, with Python's `tarfile` `data` filter. No
  package manager runs, so no lifecycle script can. None of the 105 locked
  packages has one anyway (RAN, at lock time).
- Takes only `bin/node` and `LICENSE` from the Node runtime, so the installed
  tree has no `npm` or `npx` (RAN, `find`).
- Builds in `<version>.partial`, renames it into place, and makes the tree
  read-only. An existing install is refused without `--force`. A failed
  install leaves no `.partial` behind (RAN, tamper control), and with
  `--force` the old install is removed only after the new one has verified
  (RAN, the run's `force-bad-cache.txt`).
- Writes `bin/<name>`, a two-line `sh` script that `exec`s the pinned `node`
  on the adapter's entry point by absolute path. No `PATH` lookup, so the
  nvm trap in `docs/wsl.md` does not apply.

Measured (RAN, the run record): install with the network cut in 8.45 s; one
flipped byte refused; the installed `node_modules` byte-identical to the tree
the 2026-09-13 census measured; a headless probe, a Linux panel turn and a
Windows-build panel turn all answering.

## What each agent is made of

### `claude-agent-acp` 0.73.0

- **The adapter's source rebuilds to the published package.** Tag `v0.73.0` is
  `ea7076c0bc32`. `npm ci --ignore-scripts` on the tag's own lockfile, then
  `tsc`, produces a `dist/` identical to the npm tarball's apart from
  `dist/tests`, which the package's `files` excludes (RAN, `diff -r`). A fork
  built from source can therefore reproduce what npm ships.
- **It declares three dependencies, and one is a range**:
  `@agentclientprotocol/sdk` 1.4.0, `@anthropic-ai/claude-agent-sdk` 0.3.257,
  `zod` `^4.0.0` (READ, `package.json`).
- **npm resolves those three to 112 packages. 100 are peer dependencies** of
  the SDK (`@anthropic-ai/sdk`, `@modelcontextprotocol/sdk` and what they pull
  in, `express` among them), and 8 are the SDK's per-platform binaries (RAN,
  the census tree's `package-lock.json`). Nobody has measured whether the
  adapter needs the peers at run time. If it does not, the tree to audit is
  about a dozen packages.
- **The range already moved.** A fresh resolve on 2026-09-14 differs from the
  census tree in four packages: `@anthropic-ai/sdk` 0.123.0→0.125.0, `hono`
  4.13.5→4.13.7, `jose` 6.2.10→6.2.12, `zod` 4.5.4→4.6.5 (RAN). The lock pins
  the census tree, because that is the tree that was measured.
- Licences of the 105 packages locked for linux-x64: MIT 90, ISC 7, Apache-2.0
  2, BSD-3-Clause 2, BSD-2-Clause 1, Unlicense 1, and Anthropic's two, which
  say `SEE LICENSE IN README.md` (the SDK) and `SEE LICENSE IN LICENSE.md`
  (the binary) (RAN, from each tarball's `package.json`).
- **npm's lockfile drops `libc`**, so a platform filter that reads it keeps
  the musl `claude` binary next to the glibc one. The tarball's own
  `package.json` still declares `libc`, and `agents.py lock` reads it from
  there (RAN: the first lock kept both).

### The Claude agent SDK inside it

Proprietary; not forked and not committed here. Pinned by sha256 and fetched
once. The linux-x64 `claude` binary is 215,469,464 bytes, sha256
`9a64bda9d8722a1f…` (RAN). Its audit is behavioural: the vendor-calls census.

### `codex-acp` 0.16.0 (Zed)

- Apache-2.0, Rust 2024 edition (READ, `LICENSE` and `Cargo.toml` at tag
  `v0.16.0`, `bb590500e864`). The handoff's ASSUMED "Rust" holds.
- The npm package is a JS launcher that picks one of six optional
  per-platform packages. It has no install script (READ). The linux-x64
  binary is 232,937,920 bytes, dynamically linked against glibc (RAN, `file`).
- Source tree: `Cargo.lock` has 1,049 packages. 74 come from git, not
  crates.io: 70 from `openai/codex` at tag `rust-v0.137.0`, 2 from
  `openai-oss-forks` (tungstenite), 2 from `helix-editor/nucleo` (RAN, grep
  counts). `cargo vendor` and a source build have not been run.
- A grep of `codex-acp/src` for self-update or package-manager calls found
  nothing (RAN). The `openai/codex` crates it depends on were not searched.

### `opencode` 1.18.25

- MIT, TypeScript on Bun (`packageManager: bun@1.3.14`) (READ, tag `v1.18.25`,
  `cb7d8b2f5e44`). The monorepo's `bun.lock` has about 3,200 entries, which
  includes desktop and console packages the CLI does not ship (RAN, a line
  count; not a CLI-only tree).
- **The npm package runs a postinstall script**, `postinstall.mjs`. It copies
  the binary from one of twelve optional platform packages, and if that
  package is missing it runs `npm install` itself into a temp directory
  (READ).
- **At run time it installs and updates code on its own** (READ):
  - It installs language servers with `npm`, `gem` and
    `dotnet tool install` (`lsp/server.ts`; `OPENCODE_DISABLE_LSP_DOWNLOAD`
    exists).
  - It upgrades itself to a new patch release through whichever package
    manager installed it, unless `autoupdate: false` or
    `OPENCODE_DISABLE_AUTOUPDATE` is set (`cli/upgrade.ts`). Minor and major
    releases only notify.
  - It fetches a model catalogue unless `OPENCODE_DISABLE_MODELS_FETCH=1`
    (RAN, `runs/vendorcalls-2026-09-13/`).

  A pinned opencode therefore needs all three off in its launch, or the pinned
  binary replaces itself.

## A bump

1. Read the adapter's upstream diff between the two tags.
2. Resolve the new tree once in a scratch directory
   (`npm install --package-lock-only --ignore-scripts --save-exact <pkg>@<v>`)
   and diff its versions against the current lock.
3. Rebuild the adapter from the new tag and diff `dist/` against the tarball.
4. `agents.py lock claude-agent-acp --package … --package-lock … --source …
   --commit … --entry … --node … --node-keyring …` and review the lock's diff.
   `--node-keyring` is `gpg-only-active-keys/pubring.kbx` from
   `github.com/nodejs/release-keys`. `lock` refuses a `SHASUMS256.txt` whose
   signature does not verify.
5. Install, then re-run the vendor-calls census against the installed tree.
6. Commit the lock and a run record together.

## Not established

- **No native Windows install.** Windows launch lines use the WSL install.
  A native install under `C:\Users\First Last\…` would need quoting: a direct
  launch splits the command with `shell_words::split`, so a quoted path with
  a space parses (READ, `agent-client-protocol` 2.0.0, `acp_agent.rs:873`),
  while the panel's agent label splits on whitespace (READ, `agent_name`) and
  would show a fragment. Neither is tested.
- **Nothing re-verifies an installed tree.** It is read-only but owned by the
  user, and every agent, MCP server and shell here runs as that user, so a
  `chmod u+w` and a copy replaces `claude` with no check at launch. A
  `verify` that re-hashes against a manifest written at install, called from
  `launch.sh`, would close it; not built.
- **No `mycosavant/claude-agent-acp` fork exists.** Creating one is an action
  on the maintainer's GitHub account. The lock records upstream's tag commit,
  and the installer uses the npm tarball, which was shown identical to a
  build from that commit rather than built from it.
- **The census has not been re-run on the installed tree.** The adapter and
  `claude` are byte-identical to what it measured; Node is v24.21.0 where it
  ran v24.5.0.
- Node's checksums are trusted through `nodejs/release-keys` as fetched from
  GitHub at `481637f813e9` (RAN, `gpgv`: good signature from Antoine du
  Hamel's key). That trust is first-use trust.
- MCP servers launched through `npx` are out of scope; they are the next
  supply-chain item.
