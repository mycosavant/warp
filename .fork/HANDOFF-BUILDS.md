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
