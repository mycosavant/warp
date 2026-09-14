# A local model answers the panel, 2026-09-09

> **Superseded in part 2026-09-13 by `.fork/runs/vendorcalls-2026-09-13/`.**
> *"The documented remedy does not work"* is wrong. This run's pane cwd,
> `/home/effatha/git/warp`, had a gitignored `.claude/settings.local.json`
> that set `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC` to `""` until the
> maintainer removed the line on 2026-09-13 (READ that day; mtime then
> 2026-09-03, so most likely present on 09-09), and a settings
> `env` entry overrides the variable in the launch command. With nothing
> overriding it, the flag removes every startup request. What the connections
> carried is now measured there. The body below is unchanged.

`.fork/next.html` item 3, run from a phone-driven session against
`.fork/HANDOFF-LOCALMODEL.md`. **No fork code was written or changed.** The
Windows checkout was three commits behind and all three are docs, so the binary
under test is current for this question.

**The headline: three of the four criteria hold, and the fourth fires the
falsifier the board wrote for it.** The answer is local. The agent's process is
not.

---

## What was run

| | |
|---|---|
| runtime | `llama-server` b10844, `gemma-4-12b-it-UD-Q4_K_XL.gguf`, `127.0.0.1:8080` |
| agent | `claude-agent-acp` 0.73.0, read off the wire from `agentInfo`, not from memory |
| how the model is chosen | `ANTHROPIC_BASE_URL=http://127.0.0.1:8080`, `ANTHROPIC_MODEL=gemma-4-12b` inside `WARP_FORK_ACP_COMMAND` |
| Warp | product profile + `-Console`, `warpdev.ps1`, agent overridden with `-Agent` |
| pane | routed into WSL (`session inspect` → `{"where": "host"}`), cwd `/home/effatha/git/warp` |
| turns | two: one text-only, one that had to run a shell command |

## Done when all four hold

| | criterion | result |
|---|---|---|
| 1 | a turn in the panel is answered by a model running on this machine | **yes** |
| 2 | the event log's `session_model` names that model | **yes** — ``current `gemma-4-12b` `` |
| 3 | the agent's own harness transcript agrees | **yes** — `"model":"gemma-4-12b"` |
| 4 | a socket census during the turn shows loopback and nothing else | **no.** Warp: loopback only. The agent: four TLS connections to `api.anthropic.com` |

`warpctrl agent trace`, the fork's own join of both records, is the single
cleanest artifact and it carries criteria 1 to 3 in eight lines:

```
17:39:47.497 P  In one sentence, what is 2+2?
17:39:49.066 W  agent: @agentclientprotocol/claude-agent-acp 0.73.0 "Claude Agent"
17:39:49.804 W  model: `model` "Model", current `gemma-4-12b`
17:39:51.384 A  2+2 is 4.
17:39:51.384 H  usage: 1 assistant messages, 5 input, 8 output, 62790 cache read
17:39:51.589 W  stop
```

---

## The falsifier fired, and it is the agent's socket, not Warp's

The board's first falsifier: *"A socket to a provider during a local turn. Then
the turn is not local, and the census says where it went. Record it; do not
paper over it."*

Two pollers ran for the whole 317 seconds, each with a positive control, because
a census with no control is not a measurement:

| side | what it saw |
|---|---|
| Windows, every `warp-oss.exe` socket | **three listeners, no outbound connection at all** — `127.0.0.1:64959` (warpctrl), `127.0.0.1:9282` (upstream's server), `100.82.213.46:41234` (the console's tailnet listener, which `-Console` asks for) |
| Windows control | `curl.exe` → `104.20.23.154:443`, caught |
| WSL, the agent's own process | `127.0.0.1:8080` **and** `160.79.104.10:443` on **both** turns |
| WSL control | `node` → `172.66.147.243:443` (example.com), caught |

`160.79.104.10` is `api.anthropic.com`, resolved forward from the name rather
than guessed from the range.

**The connection is opened before the model is.** Turn 1: Anthropic at
`17:39:49.876`, llama-server at `17:39:50.328`. Turn 2 the same order.

### One confound worth naming, because it nearly became the finding

The WSL poller's pattern is `warp-oss|remote-server|terminal-server|claude|node|
npx|rust-analyzer`, and **the session driving this run is itself a `claude`
process talking to `api.anthropic.com`.** The first read of the census showed
`claude → api.anthropic.com` and looked like a result. It was pid 208502, the
driver, running since before the census started. Only splitting by **pid** and
first-seen timestamp separates the driver from the two agent processes (322226,
326427) that appear within two seconds of each `agent prompt`. A census that
watches by process *name* on a machine where the driver shares that name will
hand you your own traffic as evidence.

### What crossed it: bounded by volume, since it cannot be read

TLS, so the content is not available. The byte counters are, and they settle the
question the criterion actually cares about:

| destination | bytes sent | bytes received |
|---|---|---|
| `127.0.0.1:8080`, llama-server | **254,858** | 1,583 |
| `api.anthropic.com`, four connections | 9,286 | 171,451 |

**The prompt went local.** A 62,795-token prompt is about 250 KB, and 254,858
bytes went to loopback. The Anthropic side is inverted — 9 KB up, 171 KB down —
which is the shape of a fetch, not an upload of a conversation. What those four
requests carried is **not established here**; only that they are too small to be
the prompt and that something sizeable came back.

### The documented remedy does not work

`CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` was set and the run repeated. The
connection to `api.anthropic.com` **still appears**, at
`17:43:54.704`, ahead of llama-server at `17:43:55.156`. Claude Code classifies
whatever this is as essential. Evidence in `wsl-sockets-nonessential-off.tsv`.

### Why this is not a defect in the fork, and is worth recording anyway

`CLAUDE.md` already says it in one sentence: *"`egress.rs` is **Warp's** HTTP
client, not the agent's — so the agent's own API channel is an exfil path the
deny-list does not cover."* This run turns that sentence into a measurement.
Warp made **zero** non-loopback connections while answering from a local model,
which is the fork's own claim holding exactly where the fork can enforce it. The
agent runs beside Warp, dials its own sockets, and no enforcement point in this
repository is on that path — not `http_client`, not `crates/websocket`, not the
new `crates/egress_policy`.

So "a local model answers the panel" is true of the *answer* and not yet true of
the *machine*. Closing the gap is not a `warpctrl` change; it is a firewall rule
or a network namespace around the agent, and it is a decision, not a fix.

---

## The second falsifier did not fire: the model drives the tools

*"The turn answers and the tool calls do not"* was the board's other worry.
Turn 2 asked for a shell command and got it:

```
Ran echo LOCALMODEL-TOOL-OK
```console
LOCALMODEL-TOOL-OK
```
It printed `LOCALMODEL-TOOL-OK`.
```

Gemma 4 12B picked the tool, ran it, read the output back and quoted it
correctly. On this evidence a 12B is enough to drive the agent's tools for a
one-step task. Nothing here says it holds for a long multi-step turn.

---

## What actually blocked this, and it was not the model

**The first attempt failed on context, twice, and the cause is this
repository.** Two clean numbers from the same probe run at two working
directories:

| cwd | request size |
|---|---|
| an empty directory | **20,860 tokens** — Claude Code's own floor: system prompt, tool definitions, skills listing |
| `/home/effatha/git/warp` | **62,960 tokens** |

So this repo's `CLAUDE.md` and the skill it `@`-includes add **about 42,000
tokens to every request**. `.fork/docs/model-economy.md` estimated `CLAUDE.md`
at ~29,000 tokens on the day before this run; the measured whole-project
contribution is larger, and it is the difference between a local model that can
answer here and one that cannot.

`serve.ps1` defaults to `-Ctx 12288`, which refuses both. The fix was
**`-Ctx 98304`**, and the GPU allows it for a reason worth writing down:

| context | VRAM used, of 12,227 MiB on an RTX 5070 |
|---|---|
| 12,288 | 10,306 |
| 32,768 | 10,638 |
| 98,304 | 11,686 |

**+20k tokens of context cost 332 MiB.** That is Gemma's sliding-window
attention — most layers keep only a window, so KV cache here is roughly 16 MiB
per 1k tokens instead of the hundreds of MiB a full-attention 12B would need.
96k fits with 541 MiB to spare on a 12 GB card, and that is the whole reason
this run was possible at all.

Speed at that size: prompt processing **~2,500 tok/s**, generation ~62 tok/s. A
cold 63k-token turn is about 26 seconds of prefill; llama.cpp's prefix cache
made every later turn near-instant (`n_tokens = 62802`, 5 tokens actually
evaluated).

**`serve.ps1` was not edited.** `-Ctx` was passed per launch, so a restart
returns to 12288 and the panel stops working with no obvious cause. Changing
that default is the maintainer's call and is the one thing this run recommends.

---

## Found on the way

**A model id on the wire is a request, not a fact about what answered.**
llama-server ignores the `model` field and serves whatever is loaded: asked for
`opus[1m]`, it replied `"model":"gemma-4-12b"`. So `session_model` naming a
model is weaker evidence than it looks — it records what Warp *asked for*. The
load-bearing evidence is the agent's echoed `model_usage` and llama-server's own
log, and this run has both.

**The chip list is persisted and it poisons the next launch.** The handoff
warned that `acp-models.json` is one list not keyed by agent. Measured: it
arrived holding `default_id: opus[1m]`, so it was moved aside before the run —
and the run *rewrote* it as `default_id: gemma-4-12b` with Claude's models and
the local one in the same list. Left alone, the maintainer's next ordinary
launch would have asked the real Anthropic API for `gemma-4-12b`. **Restored to
`opus[1m]` at the end of this run**; both versions are kept here.

**`agent trace` cannot find a WSL agent's transcript on the Windows build.** It
looked in `C:\Users\onemind\.claude\projects` and stopped. The agent ran inside
the distribution, so the file is at
`\\wsl.localhost\Ubuntu\home\effatha\.claude\projects`. `--harness-dir` fixes
it and the error message says so, clearly, which is why this is a note and not a
ticket. `WARP_FORK_HARNESS_DIR` is the standing form of the same answer.

**`acp probe --cwd` earns its documentation.** Run from a WSL directory the
Windows binary passes a `\\?\UNC\...` cwd and `claude-agent-acp` refuses it as
not absolute. `--cwd /home/effatha/git/warp` passes a POSIX path through
unverified, which is exactly what its help text promises.

---

## What is left running

**One Warp instance**, `inst_55f31fb93cae41e6b59a3d01e3cddde7`, launched on the
**product profile with `-Console`** — the maintainer's ordinary agent, not the
local model, because a remote working day wants a capable one. The two
conversations from this run are in its history.

**`llama-server` is still up** on `127.0.0.1:8080` at `-Ctx 98304`, because the
four small AI features wired to it on 2026-09-07 need it. It holds 11.7 GB of
VRAM; `Get-Process llama-server | Stop-Process` frees it.

To get the local model back in the panel:

```bash
warpctrl window close
ps1 -File 'C:\dev\llama\serve.ps1' -Ctx 98304
ps1 -File '\\wsl.localhost\Ubuntu\home\effatha\git\warp\.fork\tools\warpdev.ps1' -Console \
  -Agent 'wsl.exe -d Ubuntu -- env ANTHROPIC_BASE_URL=http://127.0.0.1:8080 ANTHROPIC_API_KEY=local ANTHROPIC_MODEL=gemma-4-12b npx -y @agentclientprotocol/claude-agent-acp@0.73.0'
```

Then put `acp-models.json` back to `default_id: opus[1m]` afterwards, or the
next ordinary launch asks Anthropic for a model it does not have.

---

## Files

| file | what |
|---|---|
| `sockets-summary.txt` | the distilled census, both sides, with the controls |
| `windows-sockets.tsv` | raw Windows poller output, 941 samples over 317 s |
| `wsl-sockets.tsv` | raw WSL poller output across both panel turns |
| `wsl-sockets-nonessential-off.tsv` | the same with `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` |
| `socket-bytes.txt` | per-socket `ss -ti` byte counters, the volume argument |
| `agent-trace-turn1.txt` | `warpctrl agent trace`, Warp's log joined with the agent's |
| `event-log-turn1.jsonl`, `event-log-turn2.jsonl` | the fork's event log for each turn |
| `panel-before.png` | the panel before the run, chip reading `auto (cost-efficient)` |
| `panel-turn1.png` | the answer, chip reading **`gemma-4-12b`** |
| `acp-probe.ndjson` | the pre-flight probe that found the context wall |
| `acp-probe-96k.ndjson` | the same probe answering once context was raised |
| `v1-messages-probe.json` | `/v1/messages` in the Anthropic shape, re-measured today |
| `acp-models-before.json`, `acp-models-after.json` | the chip list either side of the run |
| `llama-server.log` | the runtime's own log, including the `n_tokens = 62802` turns |
| `timeline.txt` | UTC timestamps for every phase, for correlating the census |
| `launch.txt`, `serve-launch.txt` | what the two launchers printed |
