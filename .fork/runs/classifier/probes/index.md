# Probes, 2026-09-03

All against `npx -y @agentclientprotocol/claude-agent-acp@0.73.0` (version read
from the agent's `initialize` reply, recorded per row in `probes.json`), with
`WARP_FORK_ACP_MODE`-equivalent `--mode default` unless stated, never
`--approve`. Every session opened in `auto` and was moved by Warp. "Asked" is
`permission_requests_received` on the one call each prompt produced.

| probe | cwd | extra settings | command | asked |
|---|---|---|---|---|
| a | scratch | — | `cargo --version` | 0 |
| b | scratch | — | `CARGO_BUILD_JOBS=8 cargo --version` | **1** |
| c | scratch | — | `cargo --version 2>&1 \| tail -1` | 0 |
| d | this repo | — | `git remote -v` (no rule matches it) | 0 |
| e | this repo | — | `find .fork -maxdepth 1 -iname "*.md" \| head -3` | 0 |
| f | this repo | — | `ls .fork \| head -3` ⏎ `echo "---"` | 0 |
| g | this repo | — | `ls /mnt/c/dev \| head -3` | **1** |
| h | this repo | — | `cd /mnt/c/dev && ls \| head -3` | **1** |
| i | scratch, `.claude/settings.json` | `Bash(CARGO_BUILD_JOBS=8 cargo:*)` | `CARGO_BUILD_JOBS=8 cargo --version` | 1 — *see n* |
| i2 | same, after `git init` | same | same | 1 — *see n* |
| j | scratch, `--mode acceptEdits` | — | `Write` inside cwd | 0, file written |
| k | scratch, `--mode acceptEdits` | — | `Write` outside cwd | **1**, nothing written |
| l | scratch, `.claude/settings.json` | `Bash(rustup:*)` | `rustup --version` | 1 — *see n* |
| l2 | same, after `git init` | same | same | 1 — *see n* |
| m | scratch, `.claude/settings.json` | `Bash(CARGO_BUILD_JOBS=8 cargo --version)` | `CARGO_BUILD_JOBS=8 cargo --version` | 1 — *see n* |
| m2 | same, after `git init` | same | same | 1 — *see n* |
| **n** | this repo, `.claude/settings.local.json` (temporary, restored byte-identical) | `Bash(rustup:*)` | `rustup --version` | **0** — so project-level rules load here and did not in a fresh directory |
| **o** | this repo, same file, temporary | `Bash(CARGO_BUILD_JOBS=8 cargo:*)` | `CARGO_BUILD_JOBS=8 cargo --version` | **0** |

Rows i, i2, l, l2, m, m2 measured a directory Claude Code had never seen, and
say nothing about the rules they carried; n and o are the same questions asked
where the rule actually loads. The scratch directories were under this
session's scratchpad and are gone.
