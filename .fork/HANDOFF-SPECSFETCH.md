# Handoff: the specs card's fetch, and three decisions from the night before

> **Run to completion 2026-09-09. Do not run it again** —
> `.fork/runs/pricefetch-2026-09-09/` is the account, `87d4d2049` the commit.
> All three decisions were answered by the maintainer before any code. Item 4
> is built and item 7 was run.
>
> **Two things in this file are wrong and the run README says why.** Decision
> 1's *"`CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` does not remove them"* is
> true but its implied conclusion is not: two things are documented as exempt
> from that variable, and one of them was tested and excluded, so the phrasing
> *"Claude Code classifies whatever this is as essential"* was a guess wearing
> a measurement's clothes. And *"the fork's first outbound request to a party
> that is not the person's own agent"* reads as a claim about the machine and
> is false — it is the first host **Warp's own HTTP client** dials by Warp's
> own choice.

**Written 2026-09-09, after `.fork/runs/localmodel-panel-2026-09-09/`. Paste-target:
start a new session and say *"read `.fork/HANDOFF-SPECSFETCH.md` and run it to
completion"*.**

Read `CLAUDE.md` first, as always. This file is the run, not the method.

---

## Where things stand

`.fork/next.html` items 1, 2 and 3 are done. Item 3 was measured last night and
**three of its four criteria held**; the fourth fired the falsifier the board
wrote for it, and that result is the reason half this file is decisions rather
than work.

| item | state |
|---|---|
| 1 · the picker against a non-Anthropic list | done 2026-09-07 |
| 2 · a model runtime reachable from both sides | done 2026-09-07 |
| 3 · a local model answers the panel | **done 2026-09-09**, one falsifier fired |
| **4 · the specs card's fetch** | **open — this run** |
| 5 · `authenticate`, disclosed now and sent later | open |
| 6 · housekeeping that has recurred | between builds |
| 7 · the clean-build test of `-j 8` | needs a say-so |

---

## Put these three to the maintainer in your first message

Each is one line to answer, none is yours to decide, and the first one outranks
everything else on this page.

### 1. The agent phones home during a fully local turn. Close it, or accept it?

Measured last night, both turns, reproducible. Warp made **zero** non-loopback
connections for the whole run. The *agent's* process opened **four TLS
connections to `api.anthropic.com`** on every turn, before it opened the one to
the local model, and `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` does not
remove them. Bounded by volume, since TLS cannot be read: 254,858 bytes to
loopback against 9,286 up and 171,451 down to Anthropic — so the prompt went
local and something else did not.

This is the fork's first sentence, so it is not a footnote. Three ways to go,
and **none of them is a `warpctrl` change**:

| | what it is | cost |
|---|---|---|
| **accept and document** | the agent is the person's own choice; the fork's claim is about Warp | free, and the claim gets narrower and more honest |
| **contain** | a firewall rule or a network namespace around the agent process | ops, not code; breaks the agent if it turns out to need that channel |
| **disclose** | Warp says the agent it started holds a connection to a host that is not the model — T14.18's pattern for session modes, applied to sockets | real design work, and **it is not obviously small**: Warp spawns `wsl.exe`, so the agent's real process is inside the distribution and enumerating its sockets is platform-specific |

**Do not build any of these on your own judgment.** Record the answer in
`.fork/decisions/` whichever way it goes.

### 2. `serve.ps1` defaults to 12,288 tokens of context. Change it?

At that default the local model **cannot answer a single question in this
repository**, and the error reads as the model being too small when it is not:
this repo adds ~42,000 tokens to every request on top of Claude Code's own
20,860-token floor. `-Ctx 98304` works and fits on the 12 GB card because
Gemma's sliding-window attention costs ~16 MiB per 1k tokens.

It was passed per launch last night and **not** written into
`C:\dev\llama\serve.ps1`, which is untracked machine config. So a llama-server
restart silently breaks the panel again. One-line change, the maintainer's file.

### 3. Item 7 needs "no Windows build is running"

The clean-build test of `-j 8` carries the risk it measures and draws on the
host's 64 GB from the same pool as a Windows build. It only runs on an explicit
say-so. Ask; if the answer is no, leave it.

---

## The run: item 4, the specs card's fetch

### What already exists — do not rebuild it

| | where |
|---|---|
| the card, the header, the relative-cost bar | `app/src/ai/acp_agent/specs.rs` |
| the compiled-in table, four hand-written rows | `app/src/ai/acp_agent/specs.default.toml` |
| the user's override, read on top, no rebuild | `<state dir>/fork/acp-model-specs.toml` |
| the design and the table-vs-fetch argument | `.fork/tickets/T21-the-agents-models.md`, "The design, as written before the decision" |
| the id-matching rules (`opus[1m]` → `opus`, `default` by description) | the doc comment at the top of `specs.default.toml` |

`specs.rs` already names the fetch as the next step and says why it was
deferred. Read that module's header before writing anything; it is the spec.

### What to build

**`WARP_FORK_MODEL_PRICES=fetch`**, and nothing without the variable:

- **Opt-in, off by default**, refused by name unless set. Same parser shape as
  `WARP_FORK_REMOTE_APPROVE`, not `WARP_FORK_CONTROL_BIND`'s — a typo here is
  simply not consent, and costs nothing but stale numbers.
- **One request per launch at most.** OpenRouter's `/api/v1/models` needs no key
  and answers ~356 rows across vendors, which also covers `opencode`.
- **Cached under `fork::state_dir()`**, with the cache's **age in days printed
  on the card**. A number with no date is the stale-doc defect drawn as a
  picture, which is the thing this whole card exists to avoid.
- **The mapping table stays.** The fetch fills numbers into the existing
  id→row mapping; it does not replace it. That is the ticket's finding and it
  has not changed.

### Falsifiers, from the board — if either fires, that is the finding

- **The fetch runs with the variable unset, on any path.** One test pins that
  the request builder is never constructed without it.
- **An OpenRouter slug that does not map to the agent's id.** The mapping table
  is still the design; the fetch only fills its numbers.

### The irony worth naming in the commit body

You are building the fork's **first outbound request to a party that is not the
person's own agent**, the day after measuring an agent making outbound requests
nobody asked for. That is an argument for the opt-in being real rather than
decorative, and for the test above being the one that must fail before you
trust it. `crates/egress_policy` is a **deny-list**, so `openrouter.ai` passes
by default — the deny-list is not the layer that decides this, the variable is.

---

## Leftovers from last night, both small

- **The local model has no row in the specs table**, so its card draws `?`.
  Item 3's own text said *"the specs table gets a row with `output = 0`, so the
  cost bar is empty for the one honest reason"* — that row was never added. It
  is a good first commit here and it exercises the same file the fetch touches.
- **`agent trace` cannot find a WSL agent's transcript on the Windows build.**
  It looks in `C:\Users\<user>\.claude\projects` and stops; the agent ran inside
  the distribution. `--harness-dir '\\wsl.localhost\Ubuntu\home\effatha\.claude\projects'`
  works and the error message says so clearly, which is why this is a note and
  not a ticket. `WARP_FORK_HARNESS_DIR` is the standing form.

---

## Traps this run will walk into

- **`cargo check --workspace --all-targets` is the gate**, not the binary build.
  A new field or variant that upstream also constructs breaks test code and
  `warp_tui`, and neither is compiled by `--bin warp-oss`.
- **Run `-p warp --lib` too, not just the fast crate.** `cargo test -p
  local_control` takes a second and the app crate does not; that asymmetry has
  shipped a red test at least twice.
- **Diff test-failure *membership*, not counts.** Six runs of `-p warp --lib`
  on 2026-09-04 gave 19, 20, 21, 21, 23 and 28 failures against a union of 26
  names. Baseline from **two** runs or a flaky pass promotes an old failure to
  a fresh regression.
- **A live run measures the binary, not your source, and on Windows the
  timestamp check does not work** — `C:\dev\warp` is a separate checkout that
  nothing syncs. `git -C /mnt/c/dev/warp log --oneline -1` against your HEAD.
  From WSL, sync by the Linux path:
  `git -C /mnt/c/dev/warp fetch /home/effatha/git/warp dev && git -C /mnt/c/dev/warp merge --ff-only FETCH_HEAD`.
- **`acp-models.json` is one list, not keyed by agent.** Last night's run left
  `default_id: gemma-4-12b` in it, which would have made the next ordinary
  launch ask Anthropic for a model it does not have. It was restored to
  `opus[1m]`. If you launch with `-Agent`, check it afterwards.
- **A model id on the wire is a request, not a fact.** llama-server ignores the
  `model` field and serves whatever is loaded — asked for `opus[1m]` it replied
  `gemma-4-12b`. If you measure anything about model selection, the agent's
  echoed `model_usage` is the load-bearing half, not what Warp asked for.
- **If you census sockets, split by pid.** The WSL poller watches by process
  *name* and the session driving the run is itself a `claude` process talking
  to `api.anthropic.com`. Last night's first read handed back the driver's own
  traffic as evidence. And a census with no positive control is not a
  measurement.

## Standing constraints, from `.fork/GOAL.md`

- No push, no PR, no upstream merge without explicit say-so.
- Permission posture is **frozen**. Do not measure it further.
- `CARGO_BUILD_JOBS=8`. Never build on both sides of the VM at once.
- **Leave no Warp or agent processes running** — except that the maintainer is
  remote and wants a working instance left up. Leave exactly one, and say which.

## What is running right now

- **One Warp instance**, product profile with `-Console`, on the tailnet bind.
- **`llama-server`** on `127.0.0.1:8080` at `-Ctx 98304`, holding 11.7 GB of
  VRAM. The four small AI features use it. `Get-Process llama-server |
  Stop-Process` frees the card if you need it.

## When it is done

Write `.fork/runs/pricefetch-2026-09-<dd>/` in the shape of the existing run
READMEs: what was done in order, what the person supplied, what it means, and a
file table. Mark item 4 on `.fork/next.html`. Add a friction line to
`.fork/runs/run-live-2026-09/friction.md` for anything that stopped or annoyed —
an empty day is a line saying so, and the horizon needs seven days logged.
Record the answer to decision 1 in `.fork/decisions/` whichever way it goes.
Commit as `fork: <subject> (T21)`.
