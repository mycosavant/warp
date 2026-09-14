# Handoff: three things the maintainer approved building

Written 2026-09-13. Each has a decision on record under `.fork/decisions/`
dated the same day; read that first, since it carries the maintainer's words.
**Start after `HANDOFF-MERGE.md` has landed.** The three tasks are independent.
Retire this file when all three are done.

## Read these first

1. `CLAUDE.md`: *Look for the gate first* (the mechanism usually exists),
   *Prefer the smallest thing that is still the idea*, and the note that
   `http_client::Client::get` fingerprints the destination.
2. The three decision files named below.

---

## Task 1: say when the local model endpoint stops answering

Decision: `2026-09-13-warp-watches-the-local-model-endpoint.md`. Friction a18.

**Measure today's behaviour first.** With `llama-server` running on the
Windows side and a Custom Inference endpoint configured, stop the server and
use each of the four small AI features (`.fork/tickets/T03`). Photograph what
each shows. The claim on record is "nothing"; confirm it per feature.

**Then build the smallest version that answers a18:**

- **Event-driven first.** When a small-feature request to the configured
  endpoint fails at connect, say so where the person asked, naming the
  endpoint URL and that it is not answering. No polling needed for this half.
- **An on-demand probe** exposed as a `warpctrl` read (for the phone and for
  scripts): `GET` the endpoint's model list or health route with a short
  timeout. **Use `Client::get_without_warp_headers`**; the plain `get` sends
  client id, app version and OS details. A new action changes the catalog
  count, so update both pins.
- **A live indicator that polls** only if the maintainer asks after using the
  first two. It is a constant loopback request against a server they stop on
  purpose, which is noise.

Where the config lives: `app/src/ai/local_completion/config.rs` (keys in the
OS keychain, endpoint in `settings.toml`). Calibrate every new test by breaking
it.

## Task 2: a graph run keeps the machine awake

Decision: `2026-09-13-graph-run-keeps-the-machine-awake.md`. T15 `WB-SLEEP`.

**What exists:** `crates/prevent_sleep`, `prevent_sleep(reason) -> Guard`,
Windows and macOS backends, a no-op on Linux (`build.rs`: `noop` is
`not(any(macos, windows))`). Used only in `crates/http_client`. `graph run`
lives in `crates/warp_cli/src/local_control/graph.rs`, which does not depend on
it.

**The fact that decides the design:** on this desk `graph run` is usually driven
by the Linux `warpctrl` inside WSL, where a guard is a no-op, and the machine
that sleeps is Windows. So a guard in the CLI process is right only when the CLI
is the Windows one. Options, cheapest first. Pick with the maintainer if it is
not obvious after reading.

1. **Warp holds a guard while any agent turn is in flight** (ACP and local
   agent). This covers graph runs, since every node is a turn, and also covers
   a single long turn started from the phone. ACP turns are not `http_client`
   streams, so nothing guards them today. Smallest change, widest effect.
2. **A lease action:** `warpctrl` asks the Windows Warp to hold a guard for a
   run, renewed by heartbeat and dropped on expiry, so a killed CLI cannot keep
   the machine awake forever. More code, and it changes the catalog count.
3. Guard in the CLI process only. Correct on a Windows CLI and silently does
   nothing from WSL. Do not ship this alone.

**Measure with the request list, not by sleeping.** The maintainer has sleep
disabled. `powercfg /requests` lists live power requests and needs an elevated
prompt, so ask the maintainer to run it during a turn, before and after. Read
the Windows backend first to see which request type it creates.

## Task 3: the TUI's test pass before first-class status

Decision: `2026-09-13-the-tui-is-tested-before-it-is-first-class.md`. I20.

This is a measurement task. **Build no answerer.** The output is a run record
and a recommendation the maintainer decides on.

1. **The type-ahead hazard first** (I20, *Not built, and the hazard to name*).
   The TUI does not answer approvals today; approvals are answered from another
   shell. Measure whether that claim still holds under pressure: in a scratch
   profile, under tmux in WSL and over `ssh`, with `claude-agent-acp@0.73.0`
   and `WARP_FORK_ACP_MODE=default`, prompt for a harmless write to a scratch
   file and send Enter (`tmux send-keys`) timed at and just before the prompt
   appearing. Pass means the write did not happen and the turn is still parked.
   Check the file on disk; a status line is not proof. Repeat with latency
   added (`tc` or a slow link) if SSH is the phone's path.
2. **The daily-use pass.** What a person on a phone over mosh and tmux actually
   does: launch with no account, a turn, the model chip, reading a long reply,
   resize, detach and reattach mid-turn, a denial, `warpctrl agent approve`
   from a second tmux window, and Warp's discovery record surviving a
   reattach. Log each friction as you hit it, in the friction log's shape.
3. **Deny path and a second agent.** `9b58f8eda` covered approve with
   `claude-agent-acp` only. Drive deny, and one turn with `codex-acp`
   (`.fork/runs/codex-wire-2026-09-11/` has its config).

Record in `.fork/runs/tui-firstclass-<date>/`. Close with a short list: what
would have to change before first-class status, ranked.

## Task 4: a local-model turn discloses the agent's own connection

Decision: `2026-09-13-a-local-model-turn-discloses-the-agents-own-connection.md`.
It reverses the disclosure half of the 2026-09-11 ruling (commit `a7b6803c8`),
which had refused a panel notice; "accepted" still stands.

~~**Held until `HANDOFF-VENDORCALLS.md` has run.**~~ **Released 2026-09-13**:
the run is `.fork/runs/vendorcalls-2026-09-13/`. No canary appeared in any
request body, so a notice remains the answer the decision allows.

**What the run gives the note to say**, each fact RAN on `claude-agent-acp`
0.73.0 with Claude Code 2.1.257:

| | measured |
|---|---|
| hosts | `api.anthropic.com` at agent start. WebFetch adds `/api/web/domain_info` per domain. Once, with the flag not in force, a marketplace fetch to `downloads.claude.ai` and plugin clones from `github.com`, 8 s into a long turn |
| as the user's account | yes when signed in: two of the calls carry the subscription OAuth token. They are made without an account too |
| prompt or file content in a body | no, in 105 requests across 38 runs |
| the stop | `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` removes every startup call; `skipWebFetchPreflight: true` removes the WebFetch one |
| what undoes the stop | an `env` entry in a project `.claude/settings.local.json` (measured; other settings files presumed the same). This checkout's own set it to `""` until the maintainer removed the line on 2026-09-13 |

A starting text, to be cut to fit: *"`claude-agent-acp` 0.73.0 contacts
api.anthropic.com when it starts, even with a local model: feature flags, an
account check that sends your Claude sign-in, and the MCP registry. Measured
2026-09-13, no prompt or file content was sent. Setting
`CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` stops it unless a settings file
sets it back."*

**The launch command does not hide the note** (maintainer, 2026-09-13, TOLD).
*"Hidden when the launch command already carries that stop"* would hide it in
exactly the case the 2026-09-09 run was in: the variable in the launch command
and a `settings.local.json` in the cwd undoing it. So the note shows on every
qualifying turn until dismissed, and when the launch command carries the flag
its text says the flag is set and that a settings file can still turn the
calls back on. Warp does not read the agent's settings files to decide.

**What to show.** A note in the panel, the same kind as the mode disclosure
(`WarpNote`, which read-aloud already skips), with **Got it** and an option to
make that acknowledgement the last one. It names the agent and the version
measured, whether the requests are made as the user's account, whether any
body carried prompt or file content, and the stop the run found, if any.
~~It is hidden when the launch command already carries that stop.~~ *Not
hidden by the launch command; see the rule above (2026-09-13).* It does not say
"during this turn": Warp does not see the connection per turn, and the note
describes a measured property of a version.

**When to show it. Decide from what Warp already knows, in this order:**

1. **"Local"**: the panel already knows a local model is answering. T21.4
   gave local models a specs row, and the model chip and the `session_model`
   event log line carry the model. `ANTHROPIC_BASE_URL` pointing at a
   loopback or private host inside `WARP_FORK_ACP_COMMAND` is a second signal.
   Pick what is reliable on both the Windows and WSL agent commands, and test
   both.
2. **"This agent"**: key on the agent's `initialize` reply (`agentInfo.name`
   and `version`). Show it for agents **measured** to do this, which today is
   `claude-agent-acp` alone. For an agent never measured, say nothing rather
   than guess, and file its measurement. *Added 2026-09-13:* `opencode` 1.18.25
   was measured and fetches a 4.6 MB model catalogue from `models.opencode.ai`
   on a cold cache, with no credential and no content; `codex-acp` was measured
   and reaches loopback only. The catalogue download is documented, not
   noted in the panel (maintainer, 2026-09-13, TOLD): `docs/manual.md`'s
   `WARP_FORK_ACP_COMMAND` section names it and its switch.
3. **Dismissal**: store "don't show again" keyed by agent name, **not** by
   version, unless you can argue otherwise. A new version is a new binary but
   the same vendor relationship, and re-showing on every `npx` bump teaches
   people to click through. Say which you chose and why in the commit.

**Then the docs.** In `docs/agent-transports.md` and the manual's local-model
recipe, recommend a harness for people who care. **Measure it first.** Run the
same socket census as the 09-09 panel run (whose positive control fired) with
`opencode acp` and `codex-acp` each pointed at the local `llama-server`. Only
an agent that reaches loopback and nothing else gets recommended. The 09-11
codex run reached only OpenRouter, but that was a remote model, so it does
not answer this question. **Measured 2026-09-13** (`.fork/runs/vendorcalls-2026-09-13/`):
`codex-acp` from `~/git/codex-acp` at `296069e`, `CODEX_HOME` config in that
run's `instruments/codex-config.toml`, reached loopback only; `opencode acp`
reached loopback only with `OPENCODE_DISABLE_MODELS_FETCH=1`, and
`models.opencode.ai` without it. Both answered through `llama-server`.

Verify the note live on Windows in a scratch profile. It appears on a local
turn, not on a remote-model turn, and never again after "don't show again"
across a relaunch. Photograph it.
