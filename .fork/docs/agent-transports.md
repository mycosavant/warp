# The agent you named: transports, permissions, and what is a fact about it

*Current as of 2026-09-11. The **index** is the rule list in `CLAUDE.md`'s
"Working rules" section. This page is the **account** — every place this
repo mistook the agent's own behaviour for a fact about the fork, and what
corrected it.*

*Split out of `CLAUDE.md` on 2026-09-12 with no sentence changed. This page
holds the recurring theme the rules keep restating in miniature: which
requests get answered, whether a refusal costs a whole turn, whether a config
key opens a door — all of it is a fact about `WARP_FORK_ACP_COMMAND`'s
argument, not about this codebase.*

---

## Refusals and turn behaviour: a fact about the agent, not the fork

**Whether a refusal costs the whole turn is a fact about the agent you named,
not about the fork.** Measured 2026-08-31 with `acp probe`, the same prompt and
the same refusal against two agents:

| agent | what it did after Warp said no |
|---|---|
| `opencode` | **nothing.** No further text, turn over — even when the prompt said *"if you cannot run it, say so and then tell me what 2+2 is."* |
| `claude-agent-acp` 0.70.0, `--mode default` | *"I can't run that command — you denied permission to execute it. So, 2+2 is 4."* |

**Warp sent the identical answer both times** — `{"outcome": "selected",
"optionId": "reject"}`, a per-call rejection and *not*
`RequestPermissionOutcome::Cancelled`. Both agents offer a `reject_once` option,
so `deny`'s fallback to `Cancelled` never fires for either. This retracts a
hypothesis raised the same day, that Warp was cancelling turns on denial: it is
not, and the probe shows the agent recording the call as `status: failed` with
*"The user rejected permission to use this specific tool call."*

So a turn dying after a *no* is agent behaviour with no fork remedy, and it joins
the list of things that turn out to be facts about `WARP_FORK_ACP_COMMAND`'s
argument rather than about this codebase — alongside which requests are
answerable at all, and whether the agent has session modes.

**And that run re-confirmed a dated claim rather than trusting it.**
`claude-agent-acp` at 0.70.0 with no `--mode` ran the command and raised **zero**
permission requests: still `auto` by default, still deciding by classifier with
Warp never in the loop. T14.18's measurement holds.

**A denied call can cost the whole turn while the turn reports `success`.**
Measured 2026-08-31, and it sharpens the head-vs-tail rule recorded below. A
denial landing ~90 seconds in, after substantial work, ended the conversation
with `status: success` and **no answer at all** — 2029 characters of tool trace
and the denial notice, nothing addressing the question. The status is the trap:
`agent list` reports a turn that worked, and the absence is visible only by
reading the output. So do not read `success` as "the question was answered";
read the output, or count the asks that were refused.



The `agent cancel` foreground-command and text-buffer findings that used to
sit here are fully accounted for in `.fork/docs/composer.md`, "Item 7,
measured and built" — `fork::panel_agent_is_external` gates the pane
interrupt correctly now, and text streams into one message instead of being
held back whole. Nothing here duplicates it.

## The Claude Code plugin and TR-EVENTS-B

**The `warp` Claude Code plugin is a fork surface, and it is the one nobody
remembers.** `~/.claude/plugins/cache/claude-code-warp/warp/<version>/` is seven
bash hooks that emit OSC 777 to the TTY; Warp parses them into
`CLIAgentEventType`, a **versioned protocol** negotiated through
`WARP_CLI_AGENT_PROTOCOL_VERSION`, with `PermissionRequest` and
`PermissionReplied` as first-class events. I17 already ruled on the sibling
`oz-harness-support` plugin — refused at the manager by
`fork::cloud_harness_plugin_allowed` — but the local one is welcome and largely
unexamined.

**The plugin must already be loaded when the CLI agent starts, and if it is not
the failure is silent.** Measured 2026-08-30: a `claude` running in a pane raised
a permission prompt in its own TUI and `warpctrl agent approvals` stayed
**empty** — no error, nothing in the log, and it looks exactly like "this fork
cannot see CLI agents". Warp installs the plugin on demand, so a session that was
already running when it landed has no hooks. Reloading plugins in Claude Code and
asking again made the request appear immediately. **Check the plugin is loaded
before concluding anything about the CLI-agent path.**

**And with it loaded, `agent approve` genuinely drives a real Claude Code
prompt** — `keystroke: "enter"`, the request left the queue, the tool ran and the
file appeared. That was `approvals.rs`'s weakest claim and it is now watched
rather than assumed. So the fork's thesis path has working remote consent today:
Claude asks, the plugin reports over OSC 777, Warp surfaces it, a paired device
answers.

**TR-EVENTS-B measured 2026-08-30, and the answer is "absent but recoverable".**
The `PermissionRequest` payload was captured verbatim with a second hook
registered beside the plugin's own (`.fork/tools/dump-hook-stdin.sh`; a hook
event runs every command registered for it, so this needs no edit to the
vendored plugin). Ten keys arrive:

```
cwd  effort  hook_event_name  permission_mode  permission_suggestions
prompt_id  session_id  tool_input  tool_name  transcript_path
```

- **There is no `tool_use_id`.** The claim recorded in three files is correct.
- **But `transcript_path` is on the payload**, pointing at Claude Code's own
  session JSONL, whose assistant messages carry `tool_use` blocks with
  `toolu_…` ids.
- **And the entry is written *before* the hook fires** — measured, transcript
  entries at `22:33:13.86–13.99Z` against a dump at `22:33:15Z`. So the id is
  available at decision time, not only afterwards.
- **The obvious join does not work.** Matching on `tool_input` alone is
  ambiguous: the same command resolved to **two** ids in one session, because
  an agent re-runs commands. The usable key is the *most recent* `tool_use`,
  disambiguated by `tool_name` + input among calls not yet resolved. **Not
  verified**: what happens with parallel tool calls, where one assistant message
  carries several `tool_use` blocks and "most recent" stops being a single
  answer.

**Two fields nobody knew were there, and one of them changes I18.**
`permission_mode` is on the payload, so Warp can *know* the mode a CLI agent is
in rather than infer it. And `permission_suggestions` carries Claude Code's own
proposed rule additions, shaped
`{type: addRules, rules: [...], behavior: allow, destination: session}` — a
first-class, **session-scoped** persistent grant. That is direct evidence for
I18's central claim: the "allow all for this session" affordance is not
something the fork would be inventing, it is something already offered and
currently dropped. `effort: {level: …}` is the third, unexamined.

**Two facts about it that change what the fork can do** (read 2026-08-30, T14.20):
the permission hook is **observational only** — it reports and exits, so Warp is
told and cannot answer — and the payload carries **no call id**, which is
`TR-EVENTS-B` named in three files. Both are plugin-side. So "remote consent only
works on ACP" is true today and is **not** an architectural fact: it is what you
get when the only channel is one-directional. Before concluding that a
CLI-agent limitation is structural, check whether it is simply a hook that does
not answer yet.



## The agent's own config decides what it asks

**The agent driving this fork reads `AGENTS.md`, not this file — unless
`opencode.json` says otherwise.** Measured T14.7 by asking it: in this repo
`opencode` listed `AGENTS.md` alone. So an agent sent to build the fork gets
upstream's rules, which say never to `cargo fmt` (folklore this file corrects
below) and which have never heard of the eight-job cap — the rule whose
violation took the WSL VM down the same morning. `opencode.json` at the repo
root now carries `"instructions": ["CLAUDE.md"]`, and after it the agent quoted
both the capped build command and `warpctrl window close` back correctly. The
same file carries `permission: {edit: "ask", bash: "ask"}`, which is what makes
the fork's consent surface reachable at all: **Warp cannot make an agent ask.**
The agent's own config decides *when* to ask; Warp decides only where the ask
lands and who may answer it. Committing that config is how the repo stops
depending on ambient settings for its own safety.



**A request Warp will not answer is usually the agent asking to leave the
project directory — and that is the agent's setting, not Warp's bug.**
`acp_permission` says yes only to tool kinds whose spec meaning stops at the
call, so `other` is refused. Measured T14.8: `other` is exactly what `opencode`
sends *before* any call that would reach outside the project. `cat
.fork/GOAL.md` is one `execute` and is answerable; `cat ~/.bashrc` is an `other`
followed by an `execute`, and refusing the first means the second never arrives.
It resolves paths, so `../warp/...` back inside is a plain `execute`. The remedy
lives in the agent: opencode calls this permission `external_directory`, and
`"permission": {"external_directory": {"/path/*": "allow"}}` in the project's
`opencode.json` stops the ask being raised at all — verified by running.
`claude-agent-acp` sends the same command as one `execute` and never asks this,
so **which requests are answerable is a fact about the agent you named**. Check
that before suspecting Warp when a session stalls on ordinary work.



**The agent's permission config is `opencode.json` in *this repo*, not a user
file and nothing to do with Claude Code or Warp.** `~/.config/opencode/opencode.jsonc`
exists on this machine and is empty but for its `$schema` line, so every
permission decision comes from the committed project file. (`claude-agent-acp` is
the other story entirely: it reads Claude Code's settings and starts in session
mode `auto`, described above.)



## `opencode.json`'s bash pattern map

**And its `bash` pattern map has a footgun that reads backwards.** Measured
2026-08-29, two rules, neither documented where you would look:

- **Later keys win.** `{"*": "ask", "git status*": "allow"}` allows
  `git status`; reverse the two and everything asks.
- **Unmatched commands default to `allow`, not to `ask`.** So `{"echo*":
  "ask"}` does not mean *"ask about echo"* — it means *"ask about echo and allow
  literally everything else"*. A block that reads as tightening is a wholesale
  opening.

Therefore any object form **must** start with `"*": "ask"` and list the allows
after it. **This repo now runs one**, applied 2026-08-30 with the maintainer's
explicit approval:

```json
"bash": {
  "*": "ask",
  "git status*": "allow", "git diff*": "allow", "git log*": "allow",
  "cargo test*": "allow", "cargo check*": "allow"
}
```

The plain string form (`"bash": "ask"`) has no such hazard, and is what this was.



**But the trailing `*` is not a prefix a compound command can ride, and that was
a real worry worth killing.** The obvious reading of `"git status*": "allow"` is
a glob over the whole command string, which would mean `git status && rm -rf ~`
matches and runs unasked. Measured 2026-08-30, calibrated both ways in one
session:

| command | result |
|---|---|
| `git status --short && echo COMPOUND_RAN_UNASKED` | **asked** — `echo` is not allowed |
| `git status --short && git log --oneline -1` | **ran unasked** — both segments allowed |

So `opencode` **decomposes a compound command and requires every segment to match
independently**. The allowlist cannot be smuggled past with `&&`.

That materially changes what widening it costs. Adding read-only commands does
not open a door for whatever is chained after them, because whatever is chained
after them is matched on its own. The remaining argument against any particular
entry is about that command alone — `ls` and `wc` take arbitrary paths, so an
allow is wider than it reads — and not about composition.



**How to check one, because the obvious check cannot fail.** Confirming that an
allowed command runs unasked proves nothing on its own: a map missing its
`"*": "ask"` lead allows *everything*, so the allow-list appears to work
perfectly while the catch-all is wide open. The test that matters is the one
that must **ask**. Measured against the committed file: `git status --short`
ran with 0 requests, `ls -1 .fork` and `wc -l CLAUDE.md` each raised one.

And pick that firing case so the agent will actually run it. The first attempt
used `echo hello`, and the agent answered without running anything — 0 asks and
0 tool calls, which reads exactly like a passing test and is no evidence at all.
A command whose output the agent needs (`ls`, `wc`) is the reliable shape.



## Grants that widen scope, and where they don't reach

**This repo's `opencode.json` grants `external_directory: {"~/.cargo/**":
"allow"}`, and what that does is narrower than it reads.** It is there because
reading a dependency's source is ordinary work here and it was the measured
stall. **The grant is scope, not action**: measured 2026-08-29 with it in place,
a write to `~/.cargo/…` still raised an `edit` request and a shell command still
raised an `execute` one, both approvable, and neither ran. So it does not let the
agent do anything unasked — it converts requests Warp *cannot* answer into
requests it can. `~` expands, `**` alone is enough without a sibling `*`, and
the file parses with comments if you ever want to annotate it (all three run,
not assumed). Widen it only for somewhere you would also be content to answer
`edit` prompts about, and add `~/.rustup/**` if toolchain sources start stalling
— that one has not bitten yet, so it is not granted.



**And it does not cover the same destination reached through the shell.**
Measured 2026-08-30 in a panel session: the agent wanted the
`agent_client_protocol` crate's source — exactly what the `~/.cargo/**` grant
exists for — and reached for it with `find / … | xargs grep`, a **bash** call,
where the map's `"*": "ask"` lead caught it first. The grant is on the
file-reading door; the shell door to the same place is untouched, and the two
permission surfaces do not compose. T14.8 measured this remedy against an agent
that used file reads, so its effectiveness is a fact about *how the agent
chooses to reach for a file*, not about the path being granted.



**The same collision is the fork's most repeatable friction, and it stops turns.**
Twice in eighteen turns `opencode` ran `wc -l` to size a file before reading it,
and each was denied. When the ask landed at the *tail* of a turn, after the answer
was assembled, it cost nothing. When it landed at the *head* — 8 seconds in,
before any reading — it killed the turn: 871 characters of output and no answer.
Same mechanism, opposite costs, and the timing is not something the asker
controls. `wc`, `ls`, `find` and `cat` are read-only and all ask; `git log` and
`cargo check`, which do far more, do not. The allowlist is drawn around commands
the maintainer named, not around what a command can do.



**Answered 2026-08-30, and the four commands did not get the same answer.**
`ls*` and `wc*` are now allowed; `find*`, `cat*` and `grep*` are deliberately
refused, and the split is the argument rather than a convenience:

- **`find` is disqualified outright.** `-exec` makes `find*: allow` an
  arbitrary-command allow wearing a read-only name.
- **`cat` and `grep` reveal file *contents* at arbitrary paths.** Inside the
  project that adds nothing — the agent's own read tool already reads there
  unasked. The differential is *outside* it, and this is the measured hazard
  recorded above: `external_directory` gates the file-tool door, and the bash
  door to the same place is separate and does not compose. Allowing `cat*`
  reopens through bash exactly the hole that grant closes. Note also that
  `egress.rs` is **Warp's** HTTP client, not the agent's — so the agent's own
  API channel is an exfil path the deny-list does not cover.

  **And the agent dials that channel even when the model is local, which was
  measured twice and cannot be configured away by the obvious means.** During a
  turn answered entirely by a `llama-server` on this machine,
  `claude-agent-acp` opens a TLS connection to `api.anthropic.com` *before* it
  opens the one to the model (2026-09-09, `.fork/runs/localmodel-panel-2026-09-09/`).
  `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` does not remove it. Anthropic's
  docs name exactly two things that variable does not cover — the WebFetch
  domain safety check (`skipWebFetchPreflight`) and official marketplace
  auto-install (`CLAUDE_CODE_DISABLE_OFFICIAL_MARKETPLACE_AUTOINSTALL`) — and
  **the marketplace one was tested and is not it**
  (`.fork/runs/pricefetch-2026-09-09/`). The WebFetch check is untested because
  it fires only when WebFetch is used.

  Two things worth carrying from that. **It reproduces with no Warp process at
  all** — `warpctrl acp probe` inside the distribution shows the same
  connection, so this is a fact about the agent and never about the panel, and
  measuring it needs no GUI, no relaunch and four minutes. And **the fix, if
  one is wanted, is not a `warpctrl` change**: it is a firewall rule or a
  network namespace around the agent, which is a decision rather than a bug.

  **Decided 2026-09-11: accepted and documented. Stop re-measuring it.** The
  maintainer runs Claude Code with the non-essential-traffic flags already set
  and the connection is opened anyway, which is one more control ruled out
  without a further run. Containment stays available as an ops choice — a
  firewall rule or a namespace — and is theirs to make outside this repo.

  **The panel discloses it, from a measurement of what is sent** (maintainer,
  2026-09-13). This page recorded the opposite from 2026-09-11 until then: a
  panel disclosure "considered and refused" as consent UI for something Warp
  cannot stop (`a7b6803c8`, a paraphrase with no quote of the maintainer).
  Someone who points the panel at a local model reasonably expects the turn to
  stay local, and the requests may be made as their own Claude account: the
  subscription token is on disk even with a dummy key. Whether any of these
  calls is account-scoped is unmeasured.
  `HANDOFF-VENDORCALLS.md` measures content, account and any stop first;
  `HANDOFF-BUILDS.md` task 4 then builds the note from it. Decision:
  `.fork/decisions/2026-09-13-a-local-model-turn-discloses-the-agents-own-connection.md`.

  **The general fact it stands for, which is the part to carry:** the fork's
  deny-lists live in `crates/egress_policy` and are consulted by **Warp's** HTTP
  client. An agent Warp spawns has its own network stack and its own
  credentials, and nothing in this repository is between it and the internet.
  Naming an agent in `WARP_FORK_ACP_COMMAND` is trusting it, and the thesis
  covers what *Warp* sends — not what the agent does.
- **`ls` and `wc` leak metadata and counts, not contents.** `wc -l
  ~/.ssh/id_ed25519` discloses that the file exists and is 27 lines. That is a
  real cost, and it is the one worth paying, because these two are precisely the
  measured turn-killers.



**And steering does more here than the allow does.** Both measured incidents
were the agent *sizing a file before reading it* — something its own read tool
does without asking. So, as an instruction to any agent reading this file:

> **Read files with your read tool. Do not size them with `wc` or probe with
> `ls` first, and do not shell out for something a native tool already does.**

That removes the ask at zero security cost, which is smaller than any allow.



**…but measured 2026-08-31, steering put *in the prompt* did not hold, and this
paragraph was reading stronger than the evidence.** An audit task was re-run with
that instruction written into the prompt in the imperative, naming the exact
command to avoid. The agent shelled out on its **first** call anyway, again on
its second, and produced sixteen `rg` asks before the turn was cancelled. So
steering is worth writing and it is not a remedy to rely on: it costs nothing and
it fails silently. What actually stopped the turn was the volume, not any one
refusal.



**And that volume looked like the measured case for I18 until the question was
re-run with a boundary, at which point it was not.** Same audit target, same
three questions, one sentence added — *answer only from those two files; do not
follow callers, do not trace the UI* — and the result was **0 permission
requests, 50 seconds, a complete answer**. Six audits across one day now say the
same thing:

| audit scope | asks |
|---|---|
| named files (`egress.rs`, `console.js`, auth trio, transcript) | 0–1 each |
| *"trace where the warning goes and who sees it"* | **16, turn lost** |
| the same target, scoped to two named files | **0** |



**So the rule, and it costs one sentence:** name the files, and say where the
answer stops. An audit question with a boundary is answerable inside this fork's
permission posture exactly as it stands. One without a boundary sends the agent
across the codebase and then outside the project — the `find /` that got refused
was reaching for a crate's source — and the cost is the whole turn, not an
annoyance.

That materially weakens what had been written here an hour earlier as the case
for I18. The persistent grant may still be worth building, and the argument for
it is no longer this measurement. Recorded that way because a number that
survives one control is worth much less than it looked.



**Check it with the case that must *ask*, not the case that must pass.** A map
missing its `"*": "ask"` lead allows everything, so an allow-list appears to work
perfectly while the catch-all stands wide open — the confirming test cannot fail
and proves nothing. The firing case is `cat ~/.bashrc` through bash, which must
still raise a request. And pick a passing case whose output the agent actually
needs, or it will answer without running anything and 0 asks will look like a
pass.



## Tool calls are never re-emitted as `Action` messages

**The fork's transports emit tool calls as *text*, never as `Action` messages —
and wanting them structured is a trap with a name.** `AIAgentOutputMessageType::
Action` / `api::message::Message::ToolCall` is an **instruction**: Warp's action
model executes it and returns a result. An ACP or `local_agent` agent has
*already run* the tool, so emitting one runs it a second time.
`acp_agent/translate.rs`'s module docs say this, note it was inherited from
`local_agent/translate.rs` "which found it the hard way", and say it is restated
because **T14 produced three separate instances of a hazard being recorded in
prose and then built against anyway.** A fourth was started on 2026-08-30 and
stopped at the advisor.

The consequence, so it is not rediscovered as a bug: `Exchange.tools` in the
transcript is **always empty** on both fork paths, and `get_action_result` is
structurally empty for them — the only writer in the app is
`shared_session.rs:368`, the collaboration path. That is by design, not an
omission.

**So the trap, refused by name: any design whose success criterion is
*"`get_action_result` returns `Some` on the ACP path"*.** Including the
disguised form — registering a *synthetic* finished action through
`apply_finished_action_result` so the record looks populated. It never
dispatches, but it inserts entries into an executor's model and bets nothing
upstream ever walks them as pending work; that bet cannot be settled by reading.
The correct success criterion is **"the transcript prose contains the
outcome"**, because on these paths *the prose is the record* — which is exactly
how refusals are already kept (`transcript_tests.rs`, and verified in real
transcripts on disk).



**And the last word of it is won't-fix, measured 2026-08-30 rather than
argued.** `tool_update_text` (the display path of the day, replaced by the tool
row in `146265e37`) early-returned on anything that is not `Completed`, so a
`Failed` call emitted no text of its own — which looked like the one real gap
left. The test was the cheap one: a panel session was asked to `cat` a
nonexistent file. The transcript came back carrying

> `cat: …/definitely-not-a-real-file-xyz.txt: No such file or directory` — the
> file doesn't exist, so `cat` exited non-zero.

So the failure is legible in the prose regardless, and a status marker would add
a greppable token and nothing else. **T14.19's leftover was closed without code** — and then closed *with* code
anyway, from the other end: `146265e37` gave every call its own row, so a
failure now carries a marker (`Failed to run …`, `Denied: run …`) as well as its
prose. The verdict below still stands as a verdict about the prose; what changed
is that the greppable token turned out to be worth having for a different
reason.
Everything the ticket wanted is already there: tool names in the prose, refusals
in the prose with their reason, and now failures too.



Two instrument notes from that run, both the same lesson this file keeps paying
for. `warpctrl agent approvals` in its default **pretty** format carried the
approval id *only* inside the runnable `agent approve '<id>'` line, never as a
labelled field — so a poll grepping for `approval_id` reported zero while a
request was genuinely parked, and that phantom zero was one inference away from
a security investigation into an auto-approval hole that does not exist.
**Fixed 2026-08-31**: the pretty output now has a labelled `approval_id` line,
and the empty case says *in the payload*, not merely in a comment above it, that
an agent asking nothing is not evidence nothing is running. **The rule stands
regardless — use `--output-format json` for anything a script decides on**; the
fix makes the trap need the documentation rather than depend on it. And
`--instance` is a **per-subcommand** flag, not a global one: `warpctrl pane list
--instance <id>`, never `warpctrl --instance <id> pane list`, which exits with
`unexpected argument`.



## A second binary, and what carries over

**There is a second binary, it is upstream's, and fork policy does not reach
it — but the telemetry backstop does.** `crates/warp_tui` builds
`warp-tui-oss`, described as *"Warp Agent CLI"*. Built and run for the first
time 2026-08-30; nothing in this repo's docs had mentioned it.

- **Build it the way `script/run-tui` does**: `--features standalone`, without
  which `bundled_resources_dir()` cannot find the sibling `resources/` and the
  binary cannot locate its skills. 9m18s at `-j 8`.
- **It runs.** Alt-screen, mouse tracking, and then a spinner. **Measured by
  I20**: the spinner is a device-code OAuth account gate, not a model
  credential — `--set-provider-api-key <openai|anthropic|google|grok>` and
  `--api-key` (`WARP_API_KEY`) are a separate path and do not bypass it.
- **It was not a fork surface, and this bullet said so for eleven days after it
  stopped being true.** The evidence was a grep: `fork::`, `acp_agent`,
  `local_agent` and `generate_multi_agent_output` return **nothing** in
  `crates/warp_tui/src/`. All four greps still return nothing, and the
  conclusion drawn from them was wrong, because **`crates/warp_tui` depends on
  the app crate** — `warp = { workspace = true, features = ["tui"] }` at
  `Cargo.toml:73`. The binary's entry point is `app/src/tui/mod.rs`, where
  `fork::` is in scope and always was. A grep over the library answered a
  question about the library; the question asked was about the binary.

  **I20 lifted the account gate in `e4f52077a`.** `initial_login_phase` consults
  `fork::account_gate_bypassed() && fork_agent_will_answer()` — deliberately
  conjoined, because the gate is load-bearing for Warp's own cloud agent and
  merely cosmetic for the fork's transports, which intercept before `ServerApi`
  is reached. Measured with it in place: the TUI opens on *"Not signed in"*, 22
  skills discovered, **a fork ACP agent answers, and a permission request parks
  correctly.** The consent architecture was intact in a binary the fork had
  never run.

  `warpctrl` was absent from it until 2026-09-11 and is now in, for the cost I20
  predicted: a `LaunchMode::Tui { .. }` arm on the `matches!` at
  `app/src/lib.rs`, and `warp_control_cli = ["warp/warp_control_cli"]` in
  `crates/warp_tui`'s features, forwarding like `voice_input` beside it. Build
  it with `--features standalone,warp_control_cli`. **Measured, not reasoned**:
  the TUI process publishes a discovery record and a broker socket within a
  second of launch, advertises all 115 actions, and answers `agent.approvals`
  and `agent.list` from a `warpctrl` in another shell — which on a phone over
  mosh+tmux is a second pane.

  **What that buys is a *server*, not an answerer, and the distinction is the
  safety argument.** Consent stays a typed command in a different process, so
  I20's type-ahead hazard is not taken on: it belongs to an **in-TUI keypress
  prompt**, where an Enter already sitting in the input buffer when the prompt
  takes focus is a yes nobody gave. Nothing here puts a prompt in that terminal.
  Anything that later does must answer that hazard first, and I20 says to
  measure it rather than reason about it.

  ~~**Still unverified**: the end-to-end park-and-approve *inside* a TUI
  session — a real ACP permission request parked there and answered from
  another shell. The channel is proved; that loop is not.~~ **Verified live
  2026-09-12**: a permission request parked in a real TUI session, answered
  with `warpctrl agent approve` from a separate shell, and the file it was
  asking to write came back off disk with the exact content the turn intended
  — not inferred from a status line. `.fork/runs/tui-approve-2026-09-12/`. Not
  covered: the deny path (same code, not driven live), a paired remote device
  answering, and any agent besides `claude-agent-acp`.

  The old sentence is kept above rather than deleted because its *rule* is still
  right and only its example rotted: do not assume a fork behaviour holds in the
  TUI because it holds in the GUI. Check `app/src/tui/`, not `crates/warp_tui/`.
- **The one thing that does carry over is the important one.** The telemetry
  deny-list is in `crates/egress_policy` (it was `http_client`'s own
  `egress.rs` until 2026-09-09), consulted from `http_client` at `lib.rs:378`,
  and `egress_policy::is_active()` reads *only*
  `WARP_FORK_ALLOW_TELEMETRY_EGRESS`, with no reference to the app's
  `fork::is_active()`. `warp_tui` takes `http_client.workspace = true`. **So the
  backstop is on by default in any binary that links the shared client,
  including this one.** Putting that policy below the HTTP client rather than in
  the app's seam is why the fork's strongest claim survives into a binary the
  fork never edited — worth remembering the next time a policy could go in
  either place.
- **On-thesis, with a caveat.** `--set-provider-api-key` is the user's own key,
  stored locally, which is the thesis nearly verbatim. But the agent behind it
  is *upstream's*, not the fork's, so a TUI session is a different stack from a
  panel session and none of T14's consent work applies to it.



## Where the agent runs: a fresh pane, a restored one, and a clean test profile

**An agent in the panel works in the *pane's* directory** — that half is right,
and it is the half that matters. Both agent paths read
`session_context.current_working_directory()`, so this is identical for
`local_agent` and `acp_agent`, and the failure is quiet in the worst way:
measured T14.7, a first turn asked to work on this repo answered "not a git
repository", created `/home/effatha/target/` and wrote there, and reported
success. **`warpctrl input submit 'cd /home/effatha/git/warp'` before the first
prompt**, and for an ACP agent this decides more than the files — the agent
resolves its own permission config from there too.

**This paragraph also said "a fresh pane starts in `$HOME`. Not in the directory
Warp was launched from", and that is backwards — measured 2026-09-01 across
three launches.** A fresh pane's shell *inherits Warp's own process cwd*, and a
**restored** pane keeps the directory it had in the previous launch, which is
undocumented and was the thing actually being observed:

| Warp's process cwd | pane origin | pane shell | agent's `pwd` |
|---|---|---|---|
| the repo | new tab | the repo | **the repo** |
| `$HOME` | restored from last launch | the repo | **the repo** |
| `$HOME` | scratch profile, nothing to restore | `$HOME` | **`$HOME`** |

Row 2 is what separates the two candidate rules: Warp's cwd and the pane's cwd
disagree, and the agent follows the **pane**. Row 3 is the only one that lands in
`$HOME`, and it does so because Warp was *launched* from `$HOME` — which is what
a desktop launcher or a shell sitting at home does, and so is the ordinary case
the original sentence generalised from.

**The remedy is unchanged and the reason for it is stronger.** `cd` first, not
because there is a `$HOME` default to overcome, but because the pane's directory
now has *two* sources you did not choose — where Warp happened to be launched,
and where that pane was pointing days ago. Session restore is the nastier of the
two: it survives a reboot and it is invisible in the launch command.

Two traps for anyone re-running this. Reading the shell's cwd out of
`/proc/<pid>/cwd` measures the **shell**, and the claim is about what the *agent*
sees — they coincide often enough to look like confirmation and they are not the
same quantity. And any instance launched normally has panes to restore, so a
"fresh pane" is not fresh: only `XDG_CONFIG_HOME`/`XDG_STATE_HOME` pointed at a
scratch directory (with the onboarding key seeded, per the recipe below) gives a
profile with nothing to restore.



**Leave the user's `settings.toml` alone.** For any run that needs different
settings, point `XDG_CONFIG_HOME`/`XDG_STATE_HOME` at a scratch directory —
noting that this relocates every other XDG-config tool too, `gh` included.

**…and a scratch profile means first-run onboarding, which looks exactly like a
broken control plane.** The window sits on "Welcome to Warp", so `window list`
reports `has_workspace: false`, `pane list` is empty, and `tab.create` answers
`missing_target`. Seed it instead:

```
$XDG_CONFIG_HOME/warp-oss/user_preferences.json   →   {"prefs": {"HasCompletedOnboarding": "true"}}
```

Both halves are load-bearing and each has burned a session on its own: the
directory is **`warp-oss`**, not `warp-terminal` — a file in the wrong one is
never read — and the key goes **inside `prefs`**, because a flat
`{"HasCompletedOnboarding":"true"}` is silently discarded. Launch once first if
the file does not exist, then merge the key into what Warp wrote; it has real
content. This recipe has cost three sessions a restart while being correctly
recorded in `.fork/tickets/` T15 each time, which is why it is here.

