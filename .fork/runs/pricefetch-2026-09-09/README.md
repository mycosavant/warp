# The specs card's fetch, 2026-09-09

`.fork/next.html` item 4, run from `.fork/HANDOFF-SPECSFETCH.md`. The handoff
also carried three decisions for the maintainer; all three were answered before
any code, and **the first one produced a measurement that killed the
hypothesis this session brought to it.**

---

## What the person supplied

| | |
|---|---|
| decision 1, the agent's calls to `api.anthropic.com` during a local turn | *measure the two documented opt-outs first* |
| decision 2, `serve.ps1`'s context default | *yes, set it to 98304* |
| decision 3, the clean-build test of `-j 8` | *yes, no Windows build is running* |
| raised in the same message | `~/dev/kode-rs` as a possible third harness — recon only, no build |

They also read the handoff's own framing back at it and were right twice; both
corrections are in this file.

---

## 1. Decision 2, done first because it was one line

`C:\dev\llama\serve.ps1` now defaults to `-Ctx 98304` instead of `12288`, with
the reason in its own `.DESCRIPTION` rather than only in a run README: Claude
Code's floor is ~20,900 tokens, this repository adds ~42,000, and at the old
default a panel turn was refused with an error that reads as the model being
too small. The running `llama-server` was already at 96k, so nothing was
restarted.

Untracked machine config, so this run is the only record that it changed.

---

## 2. Decision 1: the hypothesis was documented, plausible, and wrong

Last night's run set `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`, still saw
four TLS connections to `api.anthropic.com` per turn, and concluded *"Claude
Code classifies whatever this is as essential."*

Anthropic's own data-usage page says something narrower. That variable covers
metrics, error reports, `/feedback`, session surveys and feature-flag
evaluation — and **two things are exempt from it by name**, each with its own
switch:

| exempt from `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC` | its own switch |
|---|---|
| the WebFetch domain safety check (sends the hostname to `api.anthropic.com`) | `skipWebFetchPreflight: true` in settings |
| official plugin marketplace auto-install | `CLAUDE_CODE_DISABLE_OFFICIAL_MARKETPLACE_AUTOINSTALL` |

The marketplace one fit the measured shape well enough to be worth an hour: a
fresh agent process per turn, 9 KB up against 171 KB down, opened *before* the
model — the shape of a manifest download at startup, not an upload of anything.

**It is not that.** Two probe runs, minutes apart, same prompt, same local
model:

| run | environment | the agent's own pid |
|---|---|---|
| baseline | `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` | `160.79.104.10:443` at `23:27:07.083`, llama-server at `23:27:09.099` |
| opt-out | the same **plus** `CLAUDE_CODE_DISABLE_OFFICIAL_MARKETPLACE_AUTOINSTALL=1` | `160.79.104.10:443` at `23:27:57.361`, llama-server at `23:27:57.579` |

Same host, same order, connection still opened before the model. The
WebFetch preflight was not exercised — neither turn used WebFetch — and its
switch lives in a settings file belonging to the maintainer's own Claude Code,
so it was deliberately not touched. **So the remaining exemption is untested
and the tested one is excluded.**

### Two things this run got for free

**The phone-home is not Warp's, and now that is shown rather than argued.**
This measurement used `warpctrl acp probe` inside the distribution — **no Warp
process at all**, no GUI, no panel, no `wsl.exe` hop from Windows. Same agent,
same env, same connection. Last night's run established that Warp made zero
non-loopback connections; this one establishes that removing Warp entirely
changes nothing about the agent's. It also made the whole measurement cheap
enough to run twice in four minutes without restarting the maintainer's live
instance.

**And it is much cheaper than a panel run**, which is worth keeping: the
question "what does the agent dial" never needed the GUI in the loop.

### The instrument's limitation, stated because it matters for the next reader

**This poller never caught its own positive control.** A `node` process
connecting to `example.com` produced no row, in three separate attempts,
including one holding the socket open for five seconds — while `ss -tnp` on
the same machine happily attributed every `claude` socket to its pid. Under
WSL2 mirrored networking the guest's view of its own sockets is partial, and
this was not chased further.

So **this census is trustworthy for what it saw and worthless for what it did
not.** Both findings here are presences, which is the direction that survives a
blind instrument: the connection was there with the opt-out set. Any claim of
the form "and nothing else connected" must come from last night's run, whose
control did fire, not from this one.

### What is still the maintainer's

The hypothesis is dead, so decision 1 returns to the three options the handoff
listed — **accept and document**, **contain**, or **disclose** — with one
option removed and the evidence sharper. Nothing was built. There is no
decision file yet, deliberately: recording "we tried a thing and it did not
work" as a decision would put a measurement where a choice belongs.

---

## 3. Item 4, as built

`WARP_FORK_MODEL_PRICES=fetch`, off by default, refused by name.

| | where |
|---|---|
| the switch and its argument | `app/src/fork.rs`, `model_prices_fetch` |
| the catalogue, the cache, the one request | `app/src/ai/acp_agent/prices.rs` |
| `slug` on a row, and the price lookup | `app/src/ai/acp_agent/specs.rs` |
| the four slugs and the local model's row | `app/src/ai/acp_agent/specs.default.toml` |
| a `GET` that carries no Warp headers | `crates/http_client/src/lib.rs` |

**The mapping table stays, which was the ticket's finding and has not
changed.** A row names its catalogue `slug`; the fetch fills that row's
numbers. What the fetch adds is the case a table cannot serve: an agent whose
model ids *are* catalogue slugs is priced with no row at all, which is every
OpenRouter-backed agent and the 365 rows `opencode` hands over.

### Falsifier 1: the fetch runs with the variable unset

Does not, and is pinned twice. `Plan::of` is a pure function returning `Off`
whenever the switch is off, asserted across every cache state including the two
that *want* a fetch. Beside it, `the_module_has_one_door_and_the_plan_is_in_front_of_it`
asserts the module builds exactly one HTTP client and that it sits after the
`Plan::Fetch` arm — because the first test pins the decision and only the
second pins that there is nowhere else to make a request from. That is the
`eventsource` bypass's exact shape, which stood for months under a comment
saying it could not happen.

**Calibrated by failing, and it failed for an instructive reason.** The first
version counted `http_client::Client::` across the whole file, found two, and
tripped over the module doc that *names the method it is checking*. A guard
that counts prose is a guard the next paragraph loosens; it strips doc lines
now.

### Falsifier 2: an OpenRouter slug that does not map to the agent's id

**Fires, was expected to, and is the reason the design is what it is.** Claude
Code offers `fable`, `opus`, `sonnet`, `haiku`, `default` and `opus[1m]`.
OpenRouter offers `anthropic/claude-opus-5`. No rule derives one from the
other, which is why the table names the slug and the fetch never touches the
mapping. A `slug` the catalogue does not carry falls back to the row's own
numbers, so switching the fetch on can add a price and cannot take one away.

### The live catalogue, read the same day

430 rows, 58 vendors, no key, 692 KB. `openrouter-models-2026-09-09.json` in
this directory is the exact body the tests were written against.

**The calibration nobody asked for and everybody should want:** the fetched
prices for all four hand-written rows match the table *exactly* — Fable 10/50,
Opus 5/25, Sonnet 2/10, Haiku 1/5 — two days after a person typed them off the
vendor's page. The fetch and the human agree.

Three things in the live data changed the code:

- **Five rows quote `-1`**, all of them `openrouter/*` auto-routers, where the
  price depends on which model the router picks. Twenty-one other rows quote a
  genuine `0`. Both sit in the same list, so a parse failure that fell back to
  zero would announce that an auto-router is free. Negatives are dropped;
  `0` is kept and draws an empty bar.
- **Every `anthropic/*` slug has a `:batch` twin at half price**, one colon
  away and adjacent in the list. The lookup is exact-match only; a prefix rule
  would draw every Anthropic cost bar at half width and look entirely
  plausible.
- **Prices are decimal strings in USD per token.** Converted once, on the way
  in.

### A disclosure found by reading, which changed the design

`http_client::Client::get` attaches Warp's client id, app version and four
fields describing the operating system — down to the Linux kernel version — to
**every** request. `include_warp_http_headers` returns `true` unconditionally
on every non-wasm target; only the wasm branch asks whether the destination is
Warp's. In this fork the app version is `v0.fork.<sha>`, which names the commit
the binary was built from.

So the ordinary way to make this request would have handed `openrouter.ai` a
fingerprint of the machine and the build, in a fork whose first sentence is
that nothing leaves unasked, and **nothing in the diff of the new module would
have shown it.** `get_without_warp_headers` is the fix: one function beside
`get`, still through `execute_inner`, so the egress policy sees it. It is not a
way out of `Client`.

This is the "read what the cheap path does" rule from `CLAUDE.md` paying out
again — the same shape as `get_relevant_files` falling back to an outline
search that posted every symbol name.

### And a correction the maintainer asked for

`specs.rs` described the fetch as *"the fork's first outbound request to a
party that is not the person's own agent"*. Read plainly, that claims the
machine makes no third-party requests, which is false — the agent's process
makes many, all week, and `CLAUDE.md` says so in as many words. The sentence
meant something narrower: **the first host Warp's own HTTP client dials by
Warp's own choice.** Both the module doc and the ticket now say that.

---

## 4. The leftover from item 3

`gemma-4-12b` has a row now, `class = "Local"`, `input`/`output` zero. Item 3's
own text promised it and never wrote it. Zero is the true price and it draws an
empty cost bar rather than a `?`, which is the honest reason the bar is empty.

The ranking is 0.25 intelligence, 0.7 speed, deliberately not flattering: the
model picked a tool, ran it and read the output back correctly on a *one-step*
task, and nothing measured says that holds for a long multi-step turn.

---

## 5. kode-rs: recon only, no build

Raised by the maintainer in the same message. `~/dev/kode-rs` (`/mnt/c` has no
copy), HEAD `7c0cbc5`, **2026-08-10 — a month stale**. Ten crates.

**Its ACP side is real and it is on the main branch**, not only in a worktree:
`crates/kode-engine/src/acp.rs`, 1,176 lines, plus a 210-line protocol test and
a design doc. Hand-rolled v1 subset, no `agent-client-protocol` dependency,
exposed as `kode-engine acp`. It handles `initialize`, `session/new`,
`session/prompt`, `session/cancel`, and sends `session/update` and
**`session/request_permission`** — the last being the one Warp's consent
surface actually needs.

**What it does not have decides how much of this fork it would light up.** Its
design doc's own out-of-scope list is explicit: no `session/load`, no
`authenticate`, no `fs/*`, no MCP-over-ACP, and no mode or config-option
updates. Against the fork's seams that means:

| fork surface | against kode-rs today |
|---|---|
| permission requests reaching the panel and a paired phone | **works** — `session/request_permission` is implemented |
| `WARP_FORK_ACP_MODE` | nothing to set; unset is the correct configuration and refuses nothing |
| the model chip (T14.14) | **dead** — no `configOptions`, no `session/set_config_option` |
| the Model Specs card | draws `?`, which is what it is for |
| resume across turns | no `session/load`; unmeasured what Warp does with that |

It also already has `kode-models/openrouter_catalog.rs` and `pricing.rs` — it
solved item 4's problem on its own side, independently.

**Not built, not run, and deliberately so**: it is a month stale, it is 58 GB
on disk mostly target output, and "another point of infra we own" is a board
decision rather than a line item in a price-fetch run. The honest summary is
that the two missing methods are exactly the two the fork's newest work depends
on, and both are small additions to a file that already speaks the protocol.

---

## 6. Gates

| | |
|---|---|
| `cargo check --workspace --all-targets` | see `gates.txt` |
| `cargo test -p warp --lib acp_agent:: fork_tests` | see `gates.txt` |
| `./script/format` | clean; one drive-by into `crates/remote_server/src/manager_tests.rs` reverted |
| the clean-build test of `-j 8` (item 7) | `memsample.tsv`, its own section below |

---

## Files

| file | what |
|---|---|
| `openrouter-models-2026-09-09.json` | the live catalogue, 430 rows, the body the tests are written against |
| `phonehome-baseline-sockets.tsv` | the census with `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` |
| `phonehome-optout-sockets.tsv` | the same plus the marketplace opt-out — the falsification |
| `phonehome-*-timeline.txt` | poller start, probe exit, control, sample counts |
| `phonehome-probe-head.txt` | the agent's own `initialize` reply, version read off the wire |
| `optout-probe.sh` | the probe-plus-census script, kept because it needs no Warp |
| `gates.txt` | the test and check output |
