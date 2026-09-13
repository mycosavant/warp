# Handoff: ACP agents installed from pinned, audited sources, never through npx

Written 2026-09-13. Decision:
`decisions/2026-09-13-acp-agents-are-not-launched-through-a-package-manager.md`.
The maintainer, verbatim:

> i don't like npx at all, and really don't even like package managers in the
> first place. i do strongly feel that we should not npx acp agents. Can we
> vendor them? or have forks that we pin and audit before bumping? i don't mind
> the workload there at all. NPM is pretty much notorious for vulns now, and
> it's only going to get worse as threat actors w/ ai-models get increasingly
> more sophisticated.

Can start any time; it touches docs, launch scripts and one panel path.
Claims tagged RAN, READ, TOLD or ASSUMED.

## Where npx is today

- **Every recommended launch line uses it**:
  `npx -y @agentclientprotocol/claude-agent-acp@0.73.0`, in `CLAUDE.md`,
  `.fork/docs/manual.md`, `.fork/docs/composer.md` and
  `.fork/tools/warpdev.ps1` (READ, grep 2026-09-13; `environment.md` mentions
  pinning but has no launch line). The version is pinned; its
  dependencies are not: `claude-agent-acp` 0.73.0 declares `zod: ^4.0.0`, a
  range (READ, `package.json` in the npx cache).
- **Warp's code does not run npx itself.** `app/src/ai/acp_agent/mod.rs:1131`
  lists launchers (`npx`, `bunx`, `pnpx`, `uvx`, `dlx`, `pipx`) only to name
  the real agent on a permission card (READ).
- MCP servers are commonly launched through npx too. Out of scope here; file
  it as the next supply-chain item.

## What each agent is made of, and what "audit" can mean for it

| agent | source | what can be audited |
|---|---|---|
| `claude-agent-acp` | Apache-2.0 TypeScript (READ, `package.json`) | the adapter's own source and its JS dependencies |
| its `@anthropic-ai/claude-agent-sdk` 0.3.257 | **proprietary**: *"© Anthropic PBC. All rights reserved"* (READ, `LICENSE.md`), shipping a platform binary package (`claude-agent-sdk-linux-x64`, ~200 MB, READ) | **not the source.** It can be pinned by sha256 and fetched once, not forked or committed to this repo. Its audit is behavioural: the `HANDOFF-VENDORCALLS.md` census re-run on every bump |
| `codex-acp` (Zed) | survey: license, language, dependency tree (ASSUMED Rust) | if Rust, `cargo vendor` and a source build |
| `opencode` | survey (ASSUMED MIT, TypeScript on Bun) | source and its dependencies |

**Say this plainly in the docs:** for the agent this fork recommends, the part
that talks to the model and runs the shell is a closed binary. Pinning it
stops a swapped package; it cannot show what the binary does beyond what a
census observes.

## The shape to build

1. **A lock in the repo**, e.g. `.fork/agents/lock.toml`: for each agent, the
   exact version of every package in its tree (no ranges), each tarball's or
   binary's sha256 and URL, and for open-source parts the source commit.
2. **Forks the maintainer owns for the open-source parts**: `mycosavant/claude-agent-acp`,
   and the others if the survey supports it, each pinned by commit and built
   from source. A bump is: read the upstream diff, diff the resolved dependency
   tree, re-run the VENDORCALLS census, then move the pin. Record each bump in a
   run record.
3. **A fork-owned installer**, not a package manager at run time: fetch the
   locked artifacts once, verify every sha256, install with lifecycle scripts
   disabled (`--ignore-scripts` if npm is used offline at all) into a
   fork-owned prefix (`~/.local/share/warp-fork/agents/<name>/<version>` and the
   Windows equivalent under `%LOCALAPPDATA%`), refuse on any mismatch. Pin the
   Node or Bun runtime too; it is part of the tree.
4. **Launch lines use absolute paths** into that prefix. Update every doc and
   `warpdev.ps1`.
5. **Warp says when an agent is launched through a package manager.** The
   launcher list already exists (`mod.rs:1131`). A one-line note when
   `WARP_FORK_ACP_COMMAND` starts with one of them, saying it fetches code at
   launch and pointing at the installer, is honest for strangers. Do not refuse
   it: that decides for the user.

## Order

1. Survey the three agents (licenses, trees, install scripts, what they fetch
   at install), RAN rather than read where possible. One page in
   `.fork/docs/agents-supply-chain.md`.
2. Build the lock and installer for `claude-agent-acp` alone, measure a clean
   install with the network cut after the fetch step, and run one panel turn
   from the installed path on Windows and in WSL.
3. Move the docs and launcher, then the panel note.
4. The other two agents.

Commits `fork: <subject> (SUPPLYCHAIN)`.
