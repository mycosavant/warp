# Handoff: a local model answers the panel

**Written 2026-09-09 for a phone-driven session. Paste-target: start a new
session and say *"read `.fork/archive/HANDOFF-LOCALMODEL.md` and run it to
completion"*.**

Read `CLAUDE.md` first, as always. This file is the run, not the method.

---

> **DONE 2026-09-09. Result: `.fork/runs/localmodel-panel-2026-09-09/`.**
> Three of the four criteria held. The fourth fired the falsifier: Warp made
> zero non-loopback connections, and the *agent's* process opened four TLS
> connections to `api.anthropic.com` on every turn. What is still worth reading
> here is the recipe — the agent line, the aliases, the census tools. What is
> stale is the context figure implied by `serve.ps1`'s default: this repository
> needs **`-Ctx 98304`**, not 12288, or the local model cannot answer at all.
> The live handoff is `.fork/archive/HANDOFF-SPECSFETCH.md`.

---

## Why this is runnable from a phone today, when it was not yesterday

Friction line **a7** (2026-09-09) said: *"the running Warp is the surface, so
every board item that needs a relaunch is out of reach from it. Item 3 of
`next.html` is the top open item and needs exactly that."*

**That is now false, measured the same day**
(`.fork/runs/remote-launch-2026-09-09/`). From an SSH session, with the desk
**locked**:

- Warp launches, takes the discrete GPU and publishes its record in 3 seconds.
- `warpctrl` reads *and* writes both work; `input submit` was confirmed by
  screenshot rather than by its own `executed: true`.
- `shot.ps1 -Process warp-oss` returns the full window, not a black frame — so
  the panel can be *looked at* from the phone.

So the one blocker on the top open item is gone, and this run is the first that
takes advantage of it.

---

## The goal

**`.fork/next.html` item 3: a local model answers the agent panel.** This is
the item that tests the fork's first sentence.

### Done when all four hold

1. A turn in the panel is answered by a model running on this machine.
2. The event log's `session_model` line names that model.
3. The agent's own harness transcript agrees.
4. A socket census taken **during the turn** shows loopback and nothing else.

### Falsifiers, from the board — if either fires, that is the finding

- **A socket to a provider during a local turn** (a provider list being
  consulted, a title generated elsewhere). Then the turn is not local, and the
  census says where it went. Record it; do not paper over it.
- **The turn answers and the tool calls do not.** A small model that cannot
  drive the agent's tools is a fact about the model, recorded — not a defect in
  the panel.

---

## What is already on disk

Verified 2026-09-09, so do not go looking:

| | where |
|---|---|
| runtime | `C:\dev\llama\b10844\llama-server.exe` |
| launcher | `C:\dev\llama\serve.ps1` — 127.0.0.1:8080, alias `gemma-4-12b`, thinking **off** |
| model | `X:\models\gemma-4-12b-it-UD-Q4_K_XL.gguf` (7.4 GB) |
| census tool | `.fork/tools/egress-poll.ps1` (method: `.fork/runs/egress-windows-2026-09-05/README.md`) |

**The 22 GB Qwen3.6-35B-A3B is not downloaded and is the maintainer's call.**
Use Gemma 4 12B. It answered a Next Command-shaped request in half a second at
68 tok/s on 2026-09-07.

---

## The path, and why it needs no fork code

`claude-agent-acp` honours `ANTHROPIC_BASE_URL`, and llama-server's
`/v1/messages` answered the Anthropic shape correctly on 2026-09-07. So the
recommended agent keeps every permission measurement this fork has made and
swaps only the model. `WARP_FORK_ACP_COMMAND` names the same agent; the agent
names the runtime.

Under mirrored networking a loopback listener on either side answers the other,
measured both directions — so the agent started *inside* WSL reaches
`http://127.0.0.1:8080` on Windows.

### Aliases assumed (see `.fork/docs/away-from-desk.md`)

```bash
alias ps1='/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe -NoProfile -ExecutionPolicy Bypass'
alias warpctrl='/mnt/c/dev/warp/target/release/warp-oss.exe --warpctrl'
```

### Steps

```bash
# 1. runtime up, and prove it before trusting it
ps1 -File 'C:\dev\llama\serve.ps1'
curl -s http://127.0.0.1:8080/v1/models          # from WSL: mirrored networking

# 2. stop the running Warp cleanly. NEVER kill.
warpctrl window close && warpctrl instance list  # must be empty before relaunching

# 3. relaunch, agent pointed at the local runtime
ps1 -File '\\wsl.localhost\Ubuntu\home\effatha\git\warp\.fork\tools\warpdev.ps1' \
  -Console \
  -Agent 'wsl.exe -d Ubuntu -- env ANTHROPIC_BASE_URL=http://127.0.0.1:8080 npx -y @agentclientprotocol/claude-agent-acp@0.73.0'

# 4. census running BEFORE the turn starts, not after
ps1 -File 'C:\dev\egress-poll.ps1'    # check its params; write output into the run dir

# 5. drive one turn
warpctrl agent list
warpctrl agent prompt '<conversation>' 'In one sentence, what is 2+2?'

# 6. verify — three instruments, not one
warpctrl agent read '<conversation>' --output-format json
ps1 -File 'C:\dev\shot.ps1' -Process warp-oss -Out 'C:\dev\panel.png'   # then look at it
grep -h session_model <event log dir>/*.jsonl
```

**Take the screenshot before believing `agent read`.** An error renders as a
block with no output field, and the CLI is silent about a whole class of state.

**Read the event log in timestamp order**, not filename order.

---

## Traps this run will walk into

- **A live run measures the binary, not your source.** The Windows build is a
  separate checkout that nothing syncs. `git -C /mnt/c/dev/warp log --oneline -1`
  against your own HEAD *before* concluding anything. No fork code should need
  changing here, so if you find yourself rebuilding, stop and ask why.
- **The agent panel takes focus on launch**; press Escape to reach the shell.
- **A fresh pane inherits Warp's cwd, and a restored pane keeps its old one.**
  `warpctrl input submit 'cd /home/effatha/git/warp'` before the first prompt,
  and for an ACP agent that also decides where its permission config loads from.
- **`session_model` restore mixes agents' lists.** `acp-models.json` is one
  list, not keyed by agent; the 2026-09-08 run saw an `opencode/...` id
  requested while a different agent ran. If the id looks wrong, that is the
  known cause, and it is a separate finding from this one.
- **Zero permission requests means Warp was not in the loop**, not that nothing
  needed asking. `claude-agent-acp` starts in mode `auto`.

## Standing constraints, from `.fork/GOAL.md`

- No push, no PR, no upstream merge without explicit say-so.
- Permission posture is **frozen**. Do not measure it further.
- `CARGO_BUILD_JOBS=8`. Never build on both sides of the VM at once.
- **Leave no Warp or agent processes running** — except that the maintainer is
  remote today and wants a working instance left up. Leave exactly one, and say
  which.

## When it is done

Write `.fork/runs/localmodel-panel-2026-09-09/` in the shape of the existing run
READMEs: what was done in order, what the person supplied, what it means, and a
file table. Mark item 3 on `.fork/next.html`. Update friction line **a7** — its
premise was falsified the day it was written, and that correction belongs beside
it. Commit as `fork: <subject> (T21)`.
