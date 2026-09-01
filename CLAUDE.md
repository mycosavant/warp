# Working in this fork

Personal fork of `warpdotdev/warp`. **Thesis: no telemetry, no account
requirement, agents driven by the user's own Claude subscription, API keys and
local models.** Work happens on `dev`. Fork-specific docs live in `.fork/`.

**Root docs describe upstream.** `AGENTS.md` is upstream's and is still right
about architecture, feature flags, testing and exhaustive matching — read it for
those. Where it disagrees with this file, this file wins, and the one place it
actively misleads is called out under *Formatting* below.

---

## Method: run it

**Scope and verify by running the thing, not by reading it.** This is the rule
the project is built on, and it is stated here because it keeps paying:

- **T1.7** documented the control surface by executing all 88 actions one at a
  time. Three documented facts were wrong, including the count. Only running it
  would have found any of them.
- **T5.6** chased a turn that ended `Cancelled` with "nothing" in the log.
  Reading the `CancellationReason` enum produced a confident, wrong suspect. The
  log had the answer: 205 `SelectText` actions and a `CtrlC` — a person trying
  to copy. Three claims were retracted.
- **2026-08-21** produced two more in one session: reading the running process
  gave the wrong display backend (the launch recipe matters), and remembered
  guidance gave the wrong formatting rule (see below).

When something here has only been read, say so. `.fork/IDEAS.md` marks its
unverified claims at the top of the file; keep that habit.

**And the commonest defect in this fork is not a bug — it is a doc that outlived
its code.** Twelve found in one day (2026-08-31), every one by asking an agent
*"name anything whose doc comment claims something the code below it does not
do"*:

| where | the doc said | the code did |
|---|---|---|
| `egress.rs` | a check in `execute_inner` *"cannot be bypassed"* | `eventsource` bypassed it |
| `acp_permission.rs` | *"only by a surface capable of showing…"* | no surface parameter exists; refuses for all |
| `apply.rs` | a reassigned alias is *"named rather than counted"* | named **and** counted |
| `wsl.rs` | `SpawnFailed` is *"what a caller sees"* without WSL | most callers got `IoError` |
| `mode.rs` **and this file** | an unadvertised mode id is *"reported, not sent"* | it **refuses the turn** |
| `approvals.rs` | *"Approval is a keystroke here, and saying otherwise would be a lie"* | true of the pane path; `answer_acp` sends a typed option id and says so |
| `warp_agent.rs` | the preview holds *"the same two things and nothing else"* | three arms; the third is stdin content |
| `translate.rs` | permission requests *"never reach this file"* | T14.17 hands them here to log — **staled that same morning, by me** |
| `fork.rs` | *"default off, unlike every other predicate in this module"* | three others are off by default too, one arguing its own asymmetry |
| `console.rs` | `img-src` is denied by `default-src 'none'` | T12.3 added it as `'self'`; the same comment block says so four lines later |
| `registry.rs` | `waiting_for` returns *"everything currently waiting"* | it filters to one conversation — **one missing blank line** had glued `waiting()`'s doc onto it, leaving `waiting()` undocumented |
| `graph.rs` | the fingerprint holds *"everything the runner actually uses"* | `compose_prompt` uses the workspace and its own doc says a page later that it is excluded deliberately |

**Twelve, and the last four are the instructive ones**: none is a careless
comment. Each was written carefully, was true when written, and was falsified by a
later change to the code beneath or beside it. Four of the twelve are *internally*
inconsistent — the file contradicts itself and both halves are signed work — and
one was wrong purely by *position*, a missing blank line having attached it to the
next function. **Nothing in the toolchain can see any of this.** `cargo check`,
`cargo test`, `./script/format` and every gate in this file pass with all twelve
in place, which is why the question has to be asked out loud.

The last one is the shortest fuse in the table: the author of the stale sentence
and the author of the code that staled it were **the same person, hours apart**,
and neither noticed. Adding a function is exactly when the paragraph above it
stops being true, and exactly when nobody re-reads it.

The pattern is always the same: the code was corrected and the prose above it was
not, so the doc preserves a design that was considered and rejected. Two of these
were in files whose *other* comments argue the correction at length. **Ask that
question of any file you are about to trust**, and expect the answer to be about
a paragraph that used to be true.

**And a claim marked *measured* is measured as of its date, not measured now —
re-run it before building on it.** This is the subtler failure, because "measured
2026-08-27" reads like ground truth and is treated as exempt from the doubt
applied to everything else. Twice on 2026-08-29, hours apart: T14.9's *"`agent
read` shows nothing until the turn ends"* was true when written and had stopped
being true, and T14.7's *"the ACP path writes no tool events at all"* had been
fixed by T14.9 without this file being updated. Each was about to fund a night of
building something that already worked, and the second misled an advisor who was
reading carefully and had no way to know. The fork's own docs are the most
dangerous stale input available, precisely because they are the ones written to
be trusted. A measured claim you are about to spend a day on costs ten minutes to
re-run.

**…but running it does not save you if you guessed one of its inputs. Name the
inputs you did not verify.** Measured 2026-08-24: the fork's divergence from
upstream was published as 1168 files and 515 commits when it is 204 and 141 —
wrong by 5x, because the base handed to `git diff <base>...dev` was an upstream
commit `dev` already contained. The command ran perfectly and reported honestly;
the error was one step upstream of it, in a value assumed rather than computed.
`git diff A...B` is *supposed* to protect against a stale base, but when `A` is
an ancestor of `B` the merge-base is `A` itself and the three-dot form silently
degrades to a plain two-dot diff — no warning, no protection. Compute bases
(`$(git merge-base upstream/master dev)`); never paste them. Full account in
`.fork/CONSOLIDATION.md` §1.1.

**GUI gestures are runnable now, so "needs a person" needs an argument.**
`use_computer drag` (T9.1) performs press-move-release against one window and
photographs the frame *before* the release, which is the only moment a drop
preview or a drag ghost exists. It works on Windows too, without taking the
user's cursor, as long as you pass `--pid`/`--window-id` and the window is
foreground (T9.2). Recipe under "Driving a gesture" in `.fork/README.md`. It
found a real bug on each of its first two runs. What still needs a person is
anything about how something *feels* — latency, smoothness — because no
capture answers that.

**Launch Warp on Windows with `Start-Process … -NoNewWindow`, or it writes no
log at all.** `warp-oss.exe` is a console-subsystem binary; `Start-Process`
gives it its own console, so `stdout_is_a_tty` is true and `warp_logging`'s
`use_logfile` is false. What hides this is that a log still appears — the
crash-recovery sibling has no console and does log, and its file is moved into
`warp-oss.log` when the parent dies. If a log starts with "Parent has crashed;
continuing execution", it is the sibling's and the interesting half was never
written. A person double-clicking the binary is unaffected.

A crash itself is usually Warp's deliberate `Failed to render a frame 3 times
in a row; exiting...`, and the sibling that appears was spawned at *startup*
and parked in `WaitForSingleObject` — its arrival means only that the parent
went away.

## Look for the gate first

The single most repeated finding in this fork: **the feature already exists,
complete and tested, and is switched off.** Grep for the flag before writing
anything.

| | what was already there | where the gate was |
|---|---|---|
| T1 | the whole local control plane (`warpctrl`) | `DOGFOOD_FLAGS` + a per-channel settings default |
| T4 | local Warp Drive sync | an account gate |
| T5 | the entire agent transport | one function — `generate_multi_agent_output` |
| T7 | agent fan-out for a run-scale graph | nothing; the verbs existed, only the *plan* was missing |
| I15 | screenshots, input, recording, window enumeration | `DOGFOOD_FLAGS` + a non-default cargo feature |
| I16 | the whole remote-development server | `RELEASE_FLAGS` behind `cfg!(feature = "release_bundle")` — a *packaging* gate, so it is off in every build you make yourself |

Gates come in pairs. `crates/warp_features/src/lib.rs` holds `DOGFOOD_FLAGS`
(runtime); `app/Cargo.toml`'s `default` list holds the compile-time half.
Opening one usually means touching both, plus `app/src/fork.rs`.

**Prefer `fork::FORCE_ENABLED` to editing a flag list.** It sets a *user
preference*, and `FeatureFlag::is_enabled` resolves override → user preference →
channel state. So it outranks every **channel list** without touching an upstream
file — which is why I16 needed no edit to `warp_features/src/lib.rs` despite the
flag being `#[cfg(not(windows))]` there.

**But it does not outrank every `#[cfg]`, and this line said it did until
2026-08-30.** Audited from the panel, the claim is true for one kind of `#[cfg]`
and false for another, and the difference decides whether a session is wasted:

- **A `#[cfg]` on a flag-list *entry*** — `DOGFOOD_FLAGS`/`PREVIEW_FLAGS`/
  `RELEASE_FLAGS` membership. The preference wins, because the enum variant
  itself is un-gated and the preference resolves at step 2 of `is_enabled`
  (`crates/warp_features/src/lib.rs:1088`). This is the I16 case.
- **A `#[cfg]` that removes *code*** — at the consumer call site, on a
  cargo-feature-gated module, or on an enum variant absent from this build. A
  runtime preference structurally cannot reach these. `FORCE_ENABLED` resurrects
  a flag slot; it cannot conjure code the linker removed.

The failure mode is the silent one this file exists to warn about: the build is
clean, nothing changes, and there is no error to search for. When a flag looks
gated, check **which** kind you have before reaching for `FORCE_ENABLED`.

## Prefer the smallest thing that is still the idea

`crates/warp_cli/src/local_control/graph.rs` is the standard: a run-scale task
graph that added **zero** new app surface — a TOML file and a `while` loop over
verbs that already existed. Reach for a file and a loop before a subsystem.

**One thing to know before running someone else's plan, audited 2026-08-31 and
disclosed in the file's own docs rather than hidden:** the read-only floor is
structural only for a node with `review = true`, where a wide default allowlist
is refused and the fence cannot be widened by naming `read-only` itself — *"a
reviewer that can write can make its own verdict true."* Everywhere else, safety
rests on the plan author naming a restricted allowlist, and *"omit the key
entirely for no restriction"* is the documented default. Assertions are shell on
the runner's commands. So a plan file is code, and it is worth reading before
`graph run` the way any script would be. Nothing wrong was found; this is the
shape of what it is, and this table entry used to sell it without saying so.

---

## The fork's seams

Fork behaviour is deliberately concentrated, so it stays reviewable against
upstream and rebasable.

| file | what it owns |
|---|---|
| `app/src/fork.rs` | **the policy seam.** `is_active()`, `FORCE_ENABLED`/`FORCE_DISABLED` feature flags, and ~a dozen predicates (`local_agent_enabled`, `local_drive_enabled`, `account_gate_bypassed`, …). Start here. |
| `crates/http_client/src/egress.rs` | the telemetry deny-list. The "nothing escapes" claim rests on this — and it is enforced in **two** places, not one. `Client::execute_inner` covers every verb builder and the oauth2 adapter; `RequestBuilder::eventsource` reaches `execute_inner` **never** and carries its own check. That second one exists because this module's docs claimed for a long time that a check in `execute_inner` *"cannot be bypassed by a call site that forgot"*, and `eventsource` had been bypassing it the whole time — found 2026-08-31 by an agent in Warp's own panel, in the file the fork's strongest claim rests on. Not a live leak (every SSE call site targets Warp's own service), which is exactly why it was closed rather than noted: a backstop whose coverage depends on where today's call sites point is a fact about today. **Adding a way out of `Client` means adding a `redirect_if_blocked` call — grep `self.wrapped` in `lib.rs`, that is the shape of a bypass.** And remember it is a *deny-list*: an unlisted host is an allowed host, so it protects only retroactively and only against the vendors named in it. |
| `app/src/ai/local_agent/` | a local implementation of the one agent-transport function, answering from the `claude` CLI. |
| `app/src/ai/acp_agent/` | the same function again, answering from **whatever agent `WARP_FORK_ACP_COMMAND` names**, over the Agent Client Protocol (T14.5). It denies every permission request it receives — but **that is not read-only and must never be described as it**: measured, an agent at its own defaults wrote a file and asked nothing, so Warp denied nothing. T14.8 names that mechanism: `claude-agent-acp` starts in session mode **`auto`**, which it describes itself as *"use a model classifier to approve/deny permission prompts"* — so the thing deciding was a model, and Warp was never in the loop. `session/set_mode` to `default` is what makes it ask. **Re-measured 2026-08-30 at 0.70.0: still `auto` by default, now six modes.** And it is *that agent's* feature, not the protocol's — `modes` is protocol-level and `SessionModeId` is an opaque string, so `opencode` 1.18.25 answers `modes: null` and has no auto-anything to set. Do not generalise a mode id across agents. **And the panel path sends no `set_mode` at all** (T14.18): `acp_agent` sends `session/new` with a cwd and nothing else, so with `claude-agent-acp` a panel session runs in `auto` for its whole life and Warp is asked nothing — measured, 0 permission requests and the file written, against 2 requests when `default` is sent first. The fork's permission model is not too tight there; it is **unreached**. This has stayed hidden because every panel session on the board used `opencode`, which has no modes to be in. **T14.18 answers it by disclosing, not by choosing**: the mode a session starts in is now reported in the panel in the agent's own words, and `WARP_FORK_ACP_MODE` requests one — with no default, because a mode id is opaque and the protocol's own examples are `ask`/`architect`/`code`. Warp says which mode is in force; it never picks one for you. |
| `app/src/drive/local_sync/` | account-free Warp Drive: snapshot, apply, git-backed sync. |
| `app/src/ai/mcp/tool_digest.rs` | what each MCP server's tools claimed to be, hashed at connect. The tool rug-pull warning rests on this. |
| `app/src/local_control/console.*` | the console (T12) — the fork's **only** browser-reachable surface. Four unauthenticated routes serving four constants (page, script, manifest, icon), under `default-src 'none'; script-src 'self'`. The script never assigns `innerHTML` and a test pins that — **and since 2026-08-31 a second test pins the sinks that parse no markup at all**: `setAttribute`, `.href`, `.src`, `.style`, `window.open`, `location.assign`. `script-src 'self'` stops an injected `<script>`; it does not stop a `javascript:` href and it does not govern navigation. The guard was narrower than the rule it guards, which is how a rule stops being true without a diff looking wrong. Both tests are calibrated by making them fail, not by watching them pass. Keep it that way, because everything it draws was authored by an agent. **What the CSP does not cover, stated so nobody credits it with more than it does:** `connect-src 'self'` cannot tell the page's own fetch from a hostile one to the same origin, and no directive governs top-level navigation — so the page's safety rests on the `textContent`-only discipline, and the CSP is what stops that discipline's failure from becoming remote code. **After editing it run `node --check app/src/local_control/console.js`** — it is `include_str!`d, so a syntax error compiles fine, passes every Rust test, and breaks the whole page at runtime. And remember the page draws from `PendingApproval`: a control there must be gated on what the *entry* permits, not only on what the device may do (T14.6). |
| `app/src/local_control/`, `crates/local_control/`, `crates/warp_cli/src/local_control/` | the `warpctrl` control plane, 114 actions. The count is pinned by **two** tests in different crates — update both, and never loosen either. **This line said 109 for two phases**: T11.2 took it to 110, T11.4 to 111 and T11.5 to 114, and each updated the pins without updating this table. Read the count off the test, never off prose — and grep for `fn catalog_has_exactly`, because the test's own name embeds the number and so goes stale on exactly the schedule this warning is about. |
| `app/src/remote_server/wsl_transport.rs`, `crates/remote_server/src/wsl.rs` | the second `RemoteTransport`: Warp's remote-development server, in a WSL distro instead of over SSH. |

Environment variables the fork adds: `WARP_FORK_ACP_COMMAND` (**name an agent and
it answers the agent panel**; naming the command *is* the switch, there is no
second flag, and it outranks `WARP_FORK_LOCAL_AGENT`.

**Which agent to name, measured 2026-08-31 rather than preferred.** This line
used `"opencode acp"` as its only example for months. Both agents were then put
through the same refusal, and neither is unambiguously better — but one *pairing*
is:

| | asks Warp by default? | survives a refusal? |
|---|---|---|
| `opencode acp` | **yes** | **no** — no further output at all, turn over, even when the prompt says what to do instead |
| `claude-agent-acp`, no mode set | **no** — session mode `auto`, its classifier answers first and Warp is never in the loop | n/a |
| `claude-agent-acp` + `WARP_FORK_ACP_MODE=default` | **yes** | **yes** — *"I can't run that — you denied permission. So, 2+2 is 4."* |

So **the recommended configuration is the third row**, and it is a pairing:
either half of it alone is worse than `opencode`. Warp sends the identical
per-call rejection (`{"outcome": "selected", "optionId": "reject"}`, never
`Cancelled`) in every case, so the difference is entirely the agent's.

Measured across two working sessions the same day: with `opencode`, one refusal
cost a whole turn's answer while the conversation still reported
`status: success`. With the pairing above, seven consecutive turns raised zero
refusals and lost nothing. `opencode` remains perfectly usable and is what most of
this file's other measurements were taken against — it is named second now, not
removed. **The
session cwd comes from the pane, and that is where the agent finds its own
config — so the pane's directory decides whether the user's permission rules
load at all.** Measured: the same agent in a directory without its config file
ran a shell command in `$HOME` and sent no permission request; in a directory
with one it asked, and Warp denied. This corrects an earlier claim here that the
config came from wherever Warp was launched), `WARP_FORK_POLICY` (set `0`/`off`/`false`
to run stock upstream behaviour without rebuilding — use this to A/B a suspected
fork regression), `WARP_FORK_ACP_MODE` (**the session mode to ask the ACP agent for, by that agent's own id for it** — `default` for `claude-agent-acp`, which is how you make it ask rather than let its `auto` classifier answer. Unset by default and deliberately so: ids are opaque and vendor-specific, so Warp discloses the mode in force and never chooses one. An id the agent did not advertise **refuses the turn** — it is not sent and the turn does not run. This line said "reported, not sent" until 2026-08-31, and so did `mode.rs`'s own module header; both were describing a first cut that `Decision::Refuse` records as wrong and replaced, because a note scrolls and what it is a note *about* is a session running under a policy nobody chose. The parser shape here is `WARP_FORK_CONTROL_BIND`'s — a typo would otherwise silently mean something — and unlike that one, refusing costs only the turn), `WARP_FORK_LOCAL_AGENT`, `WARP_FORK_AGENT_SPAWN_DEPTH`,
`WARP_FORK_ALLOW_TELEMETRY_EGRESS`, `WARP_FORK_QUAKE_VISOR` (the one that
defaults **on** — set it off to get upstream's terminal in the hotkey window),
`WARP_FORK_FRAME_LOG` (`on`, or a threshold in ms — slow-frame accounting to
the local log; **reach for this before theorising about why something feels
slow**), `WARP_FORK_EVENT_LOG` (`on`, or a directory — one JSONL file per
agent session, appended as events arrive; **reach for this before theorising
about what an agent did**. T14.9 gave the ACP path tool events, so the "no tool
events at all" recorded here from T14.7 is **no longer true**. It was also
briefly true-looking for a worse reason: until T14.15 an ACP turn wrote **two**
files that never named each other — `session_start`/`stop` under the
conversation id, tool events under the agent's session id — so opening the
obvious one showed a session with nothing between its ends. Now everything for a
turn is filed under **Warp's conversation id**, the way `local_agent` always
did, with the agent's own id on each line as `linked_session_id`. One turn, one
file. **T14.17 added `permission_request`/`permission_replied` to that path** —
the `tool_input` that was shown, what was decided
(`allowed`/`denied`/`unanswered`), which surface answered, and on the ask whether
Warp had a *yes* to offer at all. **Read a zero here carefully**: T14.18 measured
a panel session producing zero permission requests because the agent's own
classifier answered first, so no lines means *Warp was not in the loop*, never
that nothing was decided. **`unanswered` is what a cancelled turn leaves**, and
it did not exist until an agent reviewing T14.17 in the panel found that the
value was written by a unit test and unreachable on the real path: the ask is
logged synchronously and the answer from a task cancellation drops mid-`await`,
so the trail kept the question and lost its ending. Measured both ways after the
fix — a cancelled ask writes it, an answered one does not. **And read the file
in timestamp order**: the path files per conversation, so `cat *.jsonl` gives
filename order and a reader inferring causality from line order will be
wrong), `WARP_FORK_TRANSCRIPT` (**owner-only since 2026-08-31, and `0644` before that** —
the file holds the user's prompts verbatim and inherited the umask, as did the
event log's `*.jsonl` with its `tool_input` previews; both now go through
`fork::create_private_dir`/`create_private_file`, which put the mode on the
`open` rather than chmod-ing after it, because the window between the two is
exactly when the first line is written. `discovery.rs` had the right instinct
from the start with `0700`/`0600`; these two never got it. **Verified by running,
which exposed a residual reading would have missed:** a transcript written by a
*pre-fix* build keeps `0644` until that conversation is next written, because the
transcript is rewritten whole per conversation and a dormant one is never
rewritten. An active conversation self-heals on its next turn; the event log
self-heals via `tighten_existing` on reopen. Deliberately **not** swept: a sweep
would chmod files the fork is not otherwise touching, in a directory that follows
the pane's cwd and can therefore be anywhere. `chmod 600` on an old transcript is
the user's call, and this sentence is how they learn it is theirs to make. `on` writes to
**`.warp/transcripts/` under the pane's own directory** — not `state_dir`, because outside the session's directory the
agent's read of the file arrives as `tool: other` and *no* answer exists, so the
tidy location is the unusable one. `.warp/` is upstream's project directory and
is tracked, so `/.warp/transcripts/` is gitignored. Any other value is taken as
the directory, and the caller owns reachability. **Writes the conversation to disk so the agent can grep
back what its own compaction discarded — measured across two real compactions:
the agent answered "I DO NOT HAVE IT" from memory and then found the same detail
in the file, with zero permission requests.** Note *what* it recovers: compaction
is not indiscriminate, and a fact flagged as important survives inside the
summary. What is lost, and what this is for, is the bulky incidental detail a
working session is actually made of**. Off by default, because persisting what
was said is not something a no-telemetry fork should start doing unasked. The
pointer rides every prompt as its own content block, so your text is never
edited; the panel says once that it is happening **on both agent paths, and only
since 2026-08-31**. The writer (`transcript::observe`) always hung off the shared
`BlocklistAIHistoryModel`, which both paths feed, while the pointer and the
announcement were injected only in `acp_agent`. Measured both ways before the
fix: an ACP conversation carried one `[Warp]` line, a `local_agent` one carried
**zero**, and the file was written either way — so that path put the user's
prompts on disk, told nobody, and handed the agent nothing. **The fix's first cut
also measured zero**, and the reason is worth keeping: a note is an
`AddMessagesToTask`, and on this transport the task is created by the agent
stream's own `init` event, so a note queued ahead of the stream names a task that
does not exist and is dropped. Its unit test passed throughout. Ordering against
a stream is not something a unit test on the message can see. And Warp's own
asides are
marked `[Warp]` and kept out of the file so an agent never reads them as its own
words. **What it holds that the agent's own store does not is the reason a call
failed**: measured, `opencode` records a denied command as `status=error` with no
notion that anything refused it, so an agent reading its own history sees a
failure where there was a decision. Warp keeps the refusal),
`WARP_FORK_CONTROL_BIND` (**the only one
that reaches off the machine** — one literal IP address, optionally with a port
(`192.168.1.5:41234`, `[fd00::1]:41234`; pin one if you want the console on a
home screen, because an ephemeral port makes a saved URL dead on the next
launch). A hostname, a wildcard, or a typo leaves the wide listener shut and
loopback serving, because refusing to start would take out `warpctrl window
close`),
`WARP_FORK_REMOTE_APPROVE` (lets a *paired* device run `agent.approve` — say
**yes** to a CLI agent's permission prompt from a phone. Off unless it is
literally `1`/`on`/`true`/`yes`; `agent.deny` needs no switch, because saying no
can only ever make less happen. Note the opposite parser shape to
`WARP_FORK_CONTROL_BIND`: there a typo must be *refused loudly* because it would
otherwise silently mean something, here a typo is simply not consent).
Tab→pane drag has no variable of its own; `WARP_FORK_POLICY=0` puts the tab's
horizontal-only drag axis back.

---

## Working rules

**Build with `--features gui,warp_control_cli`.** `warp_control_cli` is *not* in
`app/Cargo.toml`'s default list, and without it there is no `--warpctrl` — the
control plane this fork exists to open is simply absent from the binary.

**Stop a running Warp with `warpctrl window close`** (`CloseMainWindow` on
Windows). Killing the process leaves a stale discovery record and a
crash-recovery sibling holding the ports, so the next launch fails. Ordinary
shutdown cleans both up.

**…and `ok: true` from `window close` never meant the window closed.** Read
2026-08-30: the handler sends the close with `TerminationMode::Cancellable` —
*"the termination can be interrupted"* — and returns the instant it has asked,
without observing the outcome. So `ok` meant *the request was dispatched*, and
nothing in the payload said so. That is the mistake `approvals.rs` explicitly
refuses one action over, reporting the keystroke it sent rather than
`approved: true` because *"a result claiming `approved: true` would assert an
effect this process cannot observe"*. The result now carries
`close: "requested"`, `cancellable: true` and a `verify` sentence naming
`instance list` as the check. **Deliberately not claimed: why a close would be
refused** — the mechanism has not been established by running it, and one
candidate is ruled out (`CloseSessionConfirmationDialog` covers pane and tab
closes; `OpenDialogSource` has no window arm), so naming a cause would be
invented certainty.

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

**…and a CLI agent running in a pane blocks it too, with none of the wedge's
tells.** Measured 2026-08-30: with `claude` alive in a pane, `window close`
answered `ok: true` and the process stayed up — while `agent list` reported **no
conversations** and `agent approvals` reported **nothing waiting**, because a CLI
agent in a pane is neither. So T14.10's instruments, which exist precisely to
answer "why will it not close", are silent on this case. Three instances
accumulated this way in one session, and stale instances make every later
`warpctrl` call answer `ambiguous_instance` — which a check that greps only for
`"ok"` sails straight past. **End the agent in the pane first**, then close.

**…and cancel a wedged ACP turn first, or it will not close at all.** Measured
T14.10 against an agent built to stall: with a turn in flight that has stopped
answering, `window close` returns `ok: true` and Warp stays up — reproduced on
two separate instances, once after waiting 43 seconds. `agent cancel <id>` and
then `window close` exits in about five. So a wedge is not only a time cost; it
takes away the sanctioned way to stop, which is the one thing `kill` was already
ruled out for. `agent list` now reports `quiet_for_seconds` and `last_activity`
for a turn Warp is driving, which is how you tell there is one to cancel.

**A GUI Warp binds two loopback ports, and only one of them is this fork's.**
Measured 2026-08-24 with `ss -ltnp` against a running instance:

| port | owner |
|---|---|
| `127.0.0.1:9282` | **upstream's** `crates/http_server` — `PORT_BASE` 9277 plus the channel offset, and Oss is +5 |
| ephemeral (e.g. `:34969`) | `warpctrl`, which binds port **0** and publishes whatever it gets in the discovery record |

**This corrects a claim that stood here for two days: `warpctrl` never used
9282.** It also settles the open question below. Upstream's server is started by
`LaunchMode::should_start_local_http_server`, which is `!self.is_headless()` —
no feature flag, no channel gate, nothing fork policy touches. It serves the
routers listed at `app/src/lib.rs:2611` and answers **unauthenticated**: its CORS
layer restricts browsers to `warp.dev` origins and stops nothing else. Do not
put anything sensitive behind it; `warpctrl`'s server is the one with `auth.rs`,
the credential broker and the peer-UID check.

**…and `WARP_FORK_POLICY=0` is still a trap, because the same file tells you to
use that flag to A/B a regression.** Observed 2026-08-22: a policy-off instance
ran with a visible window and held a port, while the discovery directory stayed
**empty** — so `warpctrl window close` answered `no_instance` and there was no
sanctioned way to stop it. The port was upstream's 9282, ungated by policy; the
empty directory was `warpctrl` correctly staying off. Plan the shutdown before a
policy-off run.

**And the explanation this file gave for that stood wrong until 2026-08-31.** It
said *"the discovery record carries the credential, so no record means no client
can authenticate"*. It does not: `InstanceRecord` publishes routing metadata, the
loopback endpoint and **the filename of the credential-broker socket**, and
`discovery.rs`'s own module docs say in as many words that *"discovery records
never contain bearer tokens or reusable credentials"* — the secret is minted at
the broker, per action, and kept process-local. The observed behaviour was right
and the mechanism under it was invented: with no record a client cannot find
*where to ask*, which is a weaker and more interesting fact than not being able
to authenticate. Caught by an agent in Warp's own panel, from a prompt that
asserted the wrong version as its premise — it corrected the question instead of
answering it, which is the argument for stating your premise where the agent can
see it.

**…and often you do not need one.** `--warpctrl` runs `init_feature_flags`
before it dispatches, so `WARP_FORK_POLICY=0 warp-oss --warpctrl instance list`
resolves the whole flag set in a process that opens no window and binds no
port. That is enough to A/B any *flag*, which is most of what policy-off gets
used for. Save the GUI run for A/B-ing behaviour. (Put any probe **after**
`mark_initialized()` — `FeatureFlag::is_enabled` panics before it.)

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
argued.** `tool_update_text` early-returns on anything that is not `Completed`,
so a `Failed` call emits no text of its own — which looked like the one real gap
left. The test was the cheap one: a panel session was asked to `cat` a
nonexistent file. The transcript came back carrying

> `cat: …/definitely-not-a-real-file-xyz.txt: No such file or directory` — the
> file doesn't exist, so `cat` exited non-zero.

So the failure is legible in the prose regardless, and a status marker would add
a greppable token and nothing else. **T14.19's leftover is closed without code.**
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

**An agent in the panel works in the *pane's* directory, and a fresh pane
starts in `$HOME`.** Not in the directory Warp was launched from. Both agent
paths read `session_context.current_working_directory()`, so this is identical
for `local_agent` and `acp_agent`, and the failure is quiet in the worst way:
measured T14.7, a first turn asked to work on this repo answered "not a git
repository", created `/home/effatha/target/` and wrote there, and reported
success. **`warpctrl input submit 'cd /home/effatha/git/warp'` before the first
prompt**, and for an ACP agent this decides more than the files — the agent
resolves its own permission config from there too.

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
recorded in `.fork/TASKS.md` each time, which is why it is here.

**Read back a state-changing step before measuring what follows it.** The
merge-base trap has a second form and it bit on 2026-08-29: a driver script
answered an approval with the digest passed positionally instead of as
`--digest`, so nothing was delivered, the turn parked, and `quiet_for_seconds`
honestly reported 171 seconds of silence — which reads exactly like the wedge
that field exists to detect. As with `git diff A...B` against an assumed base,
the measurement was correct and the input to it was not. **After any mutation,
confirm the mutation before believing the next reading**: after `agent approve`,
check the request has left `agent approvals`. One extra call, and it turns a
three-minute misdiagnosis into a two-second one. Two dearer checks are worth it
before a *surprising* finding goes into a doc: calibrate a new instrument against
a known answer first (`wedged-agent.py` is the pattern — fire on the known
present, stay silent on the known absent), and confirm on a second instrument
when one exists, which is the general form of *take the screenshot before
believing `warpctrl agent read`*.

**A live run measures the binary, not your source — check the timestamp.**
Measured 2026-08-30 and it cost a rebuild plus a wrong conclusion: a release
build was started, then a fix was written while it compiled, and the run that
followed exercised the *pre-fix* binary. The feature looked broken, the unit
test for it passed, and the gap between those two facts is exactly the shape of
a real bug — so the next twenty minutes went into the wrong place. `date -r
target/release/warp-oss` against the newest file you touched settles it in one
second. This is the read-back rule (above) applied to a build: **after any
mutation, confirm the mutation before believing the next reading**, and a
compile is a mutation with a long latency and no completion signal of its own.

**And never `pgrep -f` a pattern your own command line contains.** From the same
session: `until ! pgrep -f "release/warp-oss"; do sleep 2; done` never exits,
because the `bash -c` running the loop has that string in its own argv and
matches itself. It waited 34 minutes for itself to die and never ran the build
it was guarding. Match on something narrower (`pgrep -f "release/warp-oss$"`),
or check for the thing you actually care about — the discovery record, a port,
a file.

**Diff test-failure membership, not counts.** Measure a same-session baseline on
a stashed tree and compare *which* tests failed. There is a known pre-existing
failure set (`gh`-dependent git tests, flaky secret-redaction globals, terminal
view) whose members vary run to run — a count that matches can still hide a
regression, and a count that differs by one is usually the flaky set.

**And the same trap has a third crate in it: `-p warp_cli`.** Measured
2026-08-31, by walking into it. The empty-approvals sentence was edited in
`crates/warp_cli/src/local_control/commands.rs`; `-p local_control` and `-p warp
--lib` were run and both passed; the assertion on that exact string lives in
`crates/warp_cli/src/local_control_tests.rs` and sat **red for several hours**,
shipped in a commit whose body described the fix. Written by someone who had read
the T8.6 warning below the same day. The string is now a `pub(crate)` constant
asserted by reference rather than copied, which is the fix that survives the next
person. **`cargo check --workspace --all-targets` does not catch this** — a
stale `assert_eq!` compiles perfectly.

**Adding a `warpctrl` action? Run `-p warp --lib` too, not just `-p
local_control`.** The catalog count is pinned in *two* places: the fast one is
`catalog_has_exactly_<count>_retained_actions` in
`crates/local_control/src/protocol_tests.rs` — the number is part of the name, so
grep `fn catalog_has_exactly` rather than pasting this — and its twin is
`capabilities_advertises_the_complete_catalog` in
`app/src/local_control/mod_tests.rs`.

**`PAIRABLE_ACTIONS` has no such pin, and that is why its count went stale here
for two days.** The catalog count is wrong loudly, in two crates, the moment it
drifts; the pairable list is wrong only in prose. A list that decides what a
weak credential may reach is the wrong one to leave unpinned — noted 2026-09-01
rather than fixed, because adding the test is a change to the consent surface's
guardrails and belongs in a ticket, not in a doc edit. T8.6 updated the first, left the second
red, and shipped — because `cargo test -p local_control` takes a second and the
app crate does not. `crates/warp_cli` holds two more guardrails: an
exhaustive `match` over the CLI enum and a list requiring every action to have a
parseable example.

**Widening a shared type — or merging upstream — is gated by `cargo check
--workspace --all-targets`, not by the binary build.** Same failure mode as
above, one level up. When the fork adds a variant to an enum or a field to a
struct that upstream also constructs, the compiler finds every site *it
compiles* — and `--bin warp-oss` compiles neither test code nor `warp_tui`.
T10.1's merge landed three such breaks that git had merged perfectly cleanly:
two new upstream TUI files matching exhaustively over `BlocklistAIHistoryEvent`
(T8.3's `ConversationSettledChanged`), and six `AgentConversationData` literals
in `crates/persistence` missing T8.3's `settled`. The persistence one **was
already red before the merge** — T8.3 shipped a required field without ever
compiling that crate's tests. A clean `cargo build` proves nothing here.

**Cap the release build: `CARGO_BUILD_JOBS=8 cargo build --release …`.**
Measured 2026-08-29 on WSL: an uncapped release build **took the whole VM down**
— the guest came back at `up 1 min` with an empty `dmesg`, which is the
signature of the VM dying rather than Linux OOM-killing a process. A single
`rustc` compiling the `warp` crate holds **~8.1 GB RSS**, and cargo defaults to
one job per core (32 here), so several 8 GB-class crates reach codegen together
and exhaust the VM's 31 GiB. At `-j 8` the same build finished with 19 GiB still
free. `[profile.release]`'s own comment in `Cargo.toml` records this hazard from
the CI side — *"OOM-killing release builds"* — so it is one failure with two
faces, and the guest-side face is worse because it takes the session with it.
The host has 64 GB and had **no `.wslconfig` at all**, so the VM's 32 GB was
WSL2's default half-of-host rather than a chosen value; one now exists with more
headroom, real swap and `autoMemoryReclaim`.

**And read the two numbers above as pre-fix.** The 32 GB and the `-j 8` that was
landed on after repeated OOM crashes both describe the VM *before* the
`.wslconfig` existed. Today `memory=40GB` with `swap=16GB`, so the cap is running
with headroom it was not chosen against. A single `rustc` was sampled at
**13.7 GB RSS** on 2026-08-30 with swap at 12.8/16 GB — but **which crate that
was compiling was not verified**, so it is not a like-for-like replacement for
the 8.1 GB figure and is recorded only as evidence that 8 GB is a floor rather
than a ceiling. Re-measure properly before re-tuning the cap.

**Never share `CARGO_TARGET_DIR` between two checkouts of this workspace.**
Measured 2026-08-24: running a baseline in a `git worktree` with the main tree's
target directory (to save disk) left artifacts that did not match either tree,
and the damage was *silent* — the next build failed with
`no variant or associated item named CtrlCCancelsThirdPartyHarness found` and
`no field 'inviteLink' on the GraphQL type 'Team'`, both pointing at source that
was correct on disk. Worse, it invalidates verification done before it: a
`cargo check --workspace --all-targets` that passed only proved the *cache* was
consistent. Give the worktree its own target directory and accept the disk, or
measure the baseline by stashing in place.

**A cargo *feature* enabled by one dependency changes how another crate
behaves, and nothing in the diff shows it.** Found 2026-08-31 and it is the
fork breaking the fork: `agent-client-protocol`, added for T14.5, enables
`serde_json/preserve_order`. Cargo unifies features across the whole build, so
`serde_json::Map` became an insertion-ordered `IndexMap` **everywhere** —
including `drive/local_sync/format.rs`, which relied on it being a sorted
`BTreeMap` to emit stable bytes. A git-backed sync silently started producing a
different byte stream for identical content: spurious diffs and avoidable merge
conflicts, in the one feature whose whole job is to be diffable. **No line of
`local_sync` changed.**

The tripwire existed and fired: `json_payload_keys_are_sorted_not_insertion_ordered`
says in its own comment *"pinned because the workspace enabling `preserve_order`
would silently make every file's byte-stability depend on hash iteration"*. Three
tests had been red for an unknown period, and nobody had run `-p warp --lib
local_sync` after adding ACP. **The guard worked; the habit around it did not** —
and `cargo build`, `cargo check --workspace --all-targets` and every gate in this
file are all silent, because a feature flip is not a compile error.

Fixed by sorting explicitly at the seam rather than by fighting the feature: the
ACP crate needs it, unification means it cannot be turned off for one crate
anyway, and **a module that must produce stable bytes should not depend on a
global default to get them.** The general rule: if your output's byte-stability
comes from a dependency's default, pin it locally or it is one `cargo add` away
from changing.

**And the fork already knew this.** `tool_digest.rs`'s `canonical_json` sorts
explicitly, with a doc naming the hazard exactly — *"if any crate in the graph
ever turns it on, every stored digest silently stops matching and every server
reads as a rug-pull"*. Same hazard, two modules: one defended in code, one
defended only by a test. The feature got turned on and the defended one was fine.
**Swept afterwards and there is no third**: `approvals::digest_of` and `graph`'s
fingerprint both hash field-by-field over strings, so neither can be reordered.

**A build script that reads a file it does not `rerun-if-changed` is a merge
trap.** `crates/graphql/build.rs` registers a schema from
`../warp_graphql_schema/api/schema.graphql` and watched only itself, so an
upstream merge that changed the queries *and* the schema together left a stale
registration in `OUT_DIR` and failed with "no field X on type Y" against a schema
that has the field. Fixed in both graphql build scripts (T11.1). If you add one,
declare every input.

**Merging upstream: watch the overlap, not the divergence.** `git merge-base
upstream/master dev` computed, never pasted (see *Method*), then the file sets
intersected — 39 of the fork's 204 files on 2026-08-24, of which 4 conflicted.
That number is the early warning, and it is what makes a soft fork cheap; the
cost is not paid when divergence is incurred but when a merge is deferred.

**Formatting: run `./script/format`, and disregard `AGENTS.md` on this point.**
Measured 2026-08-21: `cargo fmt` with the project's config wants to change **11
files, every one of them fork-authored, with no upstream drive-bys** — so the
workspace-wide command is safe and the "never `cargo fmt`" folklore is wrong.
The real hazard is per-file: tests live in sibling files pulled in with
`#[path]` (see `script/check_no_inline_test_modules`), and rustfmt follows those
edges, so `rustfmt some_mod.rs` silently rewrites `some_mod_tests.rs` too.
Whichever you run, check `git status` afterwards and revert files you did not
mean to touch.

**Reaching the console from a phone: three things, and the VPN is not one of
them.** Measured 2026-08-30, end to end — a phone on the LAN loaded the console
and paired, *with ProtonVPN connected*. **This retracts the claim in
`67fbe4921`'s body that ProtonVPN blocks LAN traffic.** It does not; the pairing
failures that produced that claim were `no_instance` — Warp not being alive at
the moment the phone tried — which was then watched happening live. The LAN
route being intact was read as evidence about the VPN when it was evidence about
nothing.

What is actually required:

1. **`networkingMode=mirrored`** in `.wslconfig`, which puts the WSL listener on
   the Windows stack and gives it a real LAN address (`192.168.254.3` on `eth0`,
   not a WSL-private `172.x`).
2. **An inbound Windows firewall rule for the port**, and it is needed — verified
   present with `Get-NetFirewallRule -DisplayName '*warp*'`.
3. **Warp actually running.** Backgrounding the launch from a shell that then
   exits does not keep it up, and from the phone that is indistinguishable from a
   network block.

**Pin the port, and not only for the reason recorded above.** The firewall rule
names one port, so an *ephemeral* port is refused outright rather than merely
producing a stale saved URL. `WARP_FORK_CONTROL_BIND=192.168.254.3:41234`.

**Check Warp is alive before diagnosing the network.** `pair show` answering
`no_instance` is the whole diagnosis; a phone that cannot load the page tells you
nothing about why until you know there was something to load.

**What the pairing path does and does not defend — three corrections, audited
2026-08-30.** Each was believed in this session before it was read:

- **The control server is plaintext HTTP.** `pairing.rs:288` builds
  `http://{origin}{path}#{secret}` and `discovery.rs:82` the same; there is no
  TLS anywhere in the control plane. On a home LAN that is an accepted residual.
  It is *not* survivable across the internet, and no amount of digest discipline
  fixes it: the token and every approval payload are in the clear.
- **`tool_digest.rs` contributes nothing to this threat model.** It is TOFU
  pinning of **MCP tool definitions** — it hashes what a server advertised at
  connect, diffs on the next connect, and *warns*; its own docs say
  "deliberately not a block". It never sees an approval, a paired device or a
  network path. The digest that guards a phone's yes is `digest_of` in
  `handlers/approvals.rs`, a different mechanism that merely shares a shape. Do
  not credit one for the other's job.
- **And the approval digest does not defend against an active network
  attacker.** It binds a yes to the request *the server showed*, which stops a
  stale phone answering yesterday's prompt — genuinely valuable. But it is
  computed server-side and echoed back, so a MITM on a plaintext path can show
  the phone a benign summary beside the real digest and the thumb binds
  perfectly to the nasty request. **It binds server-state to server-state; it
  never binds what the human saw.**

The remedy for all three is transport encryption the fork does not have. A
Tailscale address is *one literal IP* and so fits `WARP_FORK_CONTROL_BIND`'s
parser unchanged — binding it is **narrower** than the LAN bind, not wider, and
retires the mirrored-networking firewall rule. Prefer it as a *replacement*
bind, never an addition, and never a port-forward.

**`DEVICE_LIFETIME` is 12 hours** (`pairing.rs:116`), so a phone paired at
breakfast is unpaired by dinner, and re-pairing needs a locally-authenticated
`warpctrl` to mint the code. Plan for that before relying on a paired phone for
a working day.

**A git worktree is workflow insulation, not security insulation, and calling it
"insulated space" for an agent is actively misleading.** An agent with shell
access in a worktree runs as the same UID, sees the same `$HOME`, the same
`~/.ssh`, the same discovery records and credential broker, and can `cd`
anywhere. Security insulation is a boundary the *kernel* enforces on the
process; a directory choice enforces nothing. And the worktree habit carries the
measured `CARGO_TARGET_DIR` hazard recorded above, so it costs something real
while delivering none of the benefit it is reached for. Whatever sandbox is
available is a **word until one calibrated test** — a sandboxed command
attempting a write outside the project and a network call, both confirmed to
fail — has been run.

**There is a second binary, it is upstream's, and fork policy does not reach
it — but the telemetry backstop does.** `crates/warp_tui` builds
`warp-tui-oss`, described as *"Warp Agent CLI"*. Built and run for the first
time 2026-08-30; nothing in this repo's docs had mentioned it.

- **Build it the way `script/run-tui` does**: `--features standalone`, without
  which `bundled_resources_dir()` cannot find the sibling `resources/` and the
  binary cannot locate its skills. 9m18s at `-j 8`.
- **It runs.** Alt-screen, mouse tracking, and then a spinner. **Unverified**
  what the spinner waits on — the plausible answer is a model credential, since
  the binary offers `--set-provider-api-key <openai|anthropic|google|grok>` and
  `--api-key` (`WARP_API_KEY`), but nobody has confirmed it.
- **It is not a fork surface.** `grep -rn "fork::" crates/warp_tui/src/` returns
  **nothing**, and so does a grep for `acp_agent`, `local_agent` and
  `generate_multi_agent_output`. So `WARP_FORK_ACP_COMMAND`, the account-gate
  bypass, `FORCE_ENABLED` and every predicate in `app/src/fork.rs` are simply
  absent there. Do not assume a fork behaviour holds in the TUI because it holds
  in the GUI.
- **The one thing that does carry over is the important one.** The telemetry
  deny-list is in `crates/http_client` — `lib.rs:378`, backed by `egress.rs` —
  and `egress::is_active()` reads *only* `WARP_FORK_ALLOW_TELEMETRY_EGRESS`,
  with no reference to the app's `fork::is_active()`. `warp_tui` takes
  `http_client.workspace = true`. **So the backstop is on by default in any
  binary that links the shared client, including this one.** Putting that policy
  in the HTTP client rather than in the app's seam is why the fork's strongest
  claim survives into a binary the fork never edited — worth remembering the
  next time a policy could go in either place.
- **On-thesis, with a caveat.** `--set-provider-api-key` is the user's own key,
  stored locally, which is the thesis nearly verbatim. But the agent behind it
  is *upstream's*, not the fork's, so a TUI session is a different stack from a
  panel session and none of T14's consent work applies to it.

**The real remote backstop is SSH, not the console — and it is set up as of
2026-08-30.** A phone in Termux runs `ssh warp` and gets a shell, key-only, no
prompt. That reaches **all 114 `warpctrl` actions**, against **six** through a
paired console — seven if `WARP_FORK_REMOTE_APPROVE` is set, which adds
`agent.approve`. This line said "five" until 2026-09-01: T14.21 added
`agent.cancel` and updated the module's own docs without updating this file.
**Read the count off `PAIRABLE_ACTIONS` in `app/src/local_control/pairing.rs`,
never off this sentence** — the same rule this file already states for the 114,
and for the same reason.

**That asymmetry is the principle, not an inconsistency.** `PAIRABLE_ACTIONS` is
narrow because the *credential* is weak — a QR code is a bearer token displayed
to a room and spendable by anyone who photographs the screen inside its two
minutes. An SSH key is a strong credential held by one device. Same person, same
phone, different authority, because authority follows credential strength. Reach
for this whenever "why can't the phone do X" comes up: the answer is almost
always about the credential, not about phones.

The setup, in the order that does not lock you out:

1. Key on the phone (`ssh-keygen -t ed25519` in Termux), its **public** half
   appended to `~/.ssh/authorized_keys` — `700` on `~/.ssh`, `600` on the file.
2. **Open the port before disabling passwords**, not after. The password
   fallback is the safety net that lets you debug a failing key path; removing
   it first means the only way to test is also the way that can strand you.
3. `New-NetFirewallRule … -LocalPort 22 -RemoteAddress 192.168.254.0/24` — scope
   it to the subnet rather than `Any`.
4. Verify the host key fingerprint **out of band** (`ssh-keygen -lf
   /etc/ssh/ssh_host_ed25519_key.pub`) instead of accepting the TOFU prompt
   blind.
5. Only then `/etc/ssh/sshd_config.d/10-keys-only.conf` with
   `PasswordAuthentication no`, keeping one session open while you reload.

**Four traps, and three of them produced a confident wrong diagnosis first:**

- **A firewall rule is per *port*.** The 41234 rule for the console did nothing
  for 22, and a *dropped* packet gives no refusal — just a silent hang that
  reads exactly like a broken key or a hung shell. Check
  `Get-NetFirewallRule … LocalPort -eq <port>` before debugging anything above
  the network.
- **`~/.ssh/authorized_keys` was a *directory*** containing a copied pubkey, so
  sshd had never read it and key auth had never worked. `[ -f authorized_keys ]`
  reports "missing" for a directory, which reads as "not set up" rather than
  "set up wrongly". Look at the target before writing to it — `chmod 600` on a
  directory strips its traversal bit.
- **`BatchMode=yes` blocks the passphrase prompt, not just the password one.**
  Chosen to prove a key rather than a password, it produced a false negative on
  a passphrase-protected key: `debug1: Server accepts key` followed by failure
  means `authorized_keys` is *correct* and the client could not sign.
- **`-tt` is not needed for an interactive login** — ssh allocates a TTY when
  stdin is one. `-t` is for a *remote command* that needs a terminal, which is
  how the TUI would be run over SSH.

**There is a browser, and it is on the Windows side.** `/mnt/c/Program Files`
holds Firefox, Brave and Zen, and Windows reaches the WSL wide listener — so a
page served by `WARP_FORK_CONTROL_BIND` can be loaded in a real engine and
photographed with `shot.ps1 -Process firefox`. Launch with a scratch profile
(`-profile 'C:\dev\…' -no-remote`, or `--user-data-dir=` for Chromium) so the
user's own session is untouched. **T12 filed "no browser on this machine" three
times as a blocker; it meant the WSL userland and was written as though it meant
the hardware.** Before naming something as needing a person, check that the
blocker is real.

**Screenshotting the Windows build: `shot.ps1 -Process warp-oss`.** It
captures one window via `PrintWindow(hwnd, hdc, PW_RENDERFULLCONTENT)` —
*without* raising or focusing it, and regardless of what is on top. Omit
`-Process` and it silently falls back to a grab of the entire virtual screen,
which on a working desktop is unreadable. `CopyFromScreen` cannot substitute
(it only sees what is displayed), raising the window first cannot substitute
(the foreground lock refuses it from a background process), and the `2` flag is
not optional (GPU-composited windows capture blank without it). Full reasoning
under "The scripts" in `.fork/README.md` — **this recipe has been lost to a
cleared session twice.**

**Running on WSLg:** `env -u WAYLAND_DISPLAY LIBGL_ALWAYS_SOFTWARE=1
./target/release/warp-oss`. Unsetting `WAYLAND_DISPLAY` puts winit on X11, which
is what screenshots and global hotkeys need. Synthetic clicks land there, and
**so do synthetic keystrokes — but only as far as the keymap.** Measured
2026-08-24, correcting a blanket "keystrokes do not land" that had been used to
block work: `use_computer drag --press 0xff1b` mid-drag produced
`EditorAction::Escape` → `PaneGroupAction::CancelDrag` in the log and visibly
cancelled the drag, while `use_computer text "echo …"` into a focused terminal
input produced nothing at all. So cancel keys and shortcuts are drivable and
typing is not. `Key::Keycode(n)` is an X **keysym** on this backend, not a
keycode — Escape is `0xff1b`.

**Screenshotting on WSLg: capture the window, never the root.** Same failure as
the Windows `CopyFromScreen` case and worth the two lines it costs:

```
DISPLAY=:0 xwininfo -root -children     # the app is the child with a real geometry
DISPLAY=:0 import -window 0x20006d /tmp/shot.png
```

`scrot`/`import -window root` return a **solid black frame** — the surface is
GPU-composited and the root window never held its pixels. `import -window <id>`
gets the real contents without raising or focusing anything. The id changes every
launch, so read it rather than remember it; Warp is the child sized like a window
(`1246x802+1089+596`), among Weston's own 1x1 and 10x10 stubs.

**And take the screenshot before believing `warpctrl agent read`.** Measured on
T14.6: a conversation whose panel was displaying a full error paragraph read back
through `agent read` as an exchange with **no output field at all**, because the
error renders as an error block rather than as output. The CLI is the faster
instrument and it is silent about a whole class of state.

---

## Where to read

Five files in `.fork/`, in the order a cold start wants them.

- **`GOAL.md`** — **read this first if it exists.** A dated, deliberately
  temporary horizon: what the fork is being driven toward right now and what
  "done" means as a *run*. It outranks the board's ordering while it stands, and
  it is meant to be deleted when met or abandoned. Absent means there is no
  standing horizon and `TASKS.md` is the plan.
- **`README.md`** — the operating manual: how to build and run on each platform,
  the full `warpctrl` surface, Warp Drive, WSL integration, and the gotchas that
  cost hours. Reach for it whenever you need to *use* something rather than
  change it. Large; navigate by heading.
- **`IDEAS.md`** — the idea board in front of the task board. Fifteen entries,
  each with what already exists underneath it and an argument for or against
  building it. Read before scoping any new feature, because roughly half of them
  turned out to be already built.
- **`TASKS.md`** — the board, T1–T8, plus an "as built" record for each item
  saying what was actually found. **Mostly historic**: read a section when you
  are touching that area, and treat the "as built" and "Decisions on record"
  parts as live. T8 is the current phase.
- **`SPEC.md`** — the original de-telemetry/de-account reasoning, Phases 0–4.
  Superseded as a plan by `TASKS.md` from Phase 5 on, but its survey findings —
  the request-path trace and the kill-switch seam analysis — are still the best
  explanation of *why* the fork is shaped this way.

`git log` is a real source here, not an afterthought: commit bodies carry the
reasoning, including retractions.

## Commits

`fork: <lowercase subject> (Txx)`, where `Txx` is the task from `TASKS.md`. The
body explains *what was found*, not what was typed — including when it
contradicts something previously recorded. Corrections belong in the commit that
makes them, and in the doc that was wrong.
