# Friction log: the consent path on the wire

**Run started 2026-09-01.** Horizon: `.fork/GOAL.md`.

Agent: `@agentclientprotocol/claude-agent-acp` **0.70.0** (latest published as of
this date, so `CLAUDE.md`'s dated measurements against it are current, not
historic). Resolved from an npx cache — it has never been installed on this
machine, only `npx`-run:
`~/.npm/_npx/fca12915ff656968/node_modules/.bin/claude-agent-acp`.

`WARP_FORK_ACP_MODE=default`. `WARP_FORK_EVENT_LOG` and `WARP_FORK_TRANSCRIPT`
on for the whole run. Binary rebuilt at run start — the previous
`target/release/warp-oss` predated ~20 reviewed source files, and *a live run
measures the binary, not your source*.

---

## Settled before launch, by reading

Recorded here because both change what the run is looking for, and one of them
retires a fear rather than confirming it.

### Unknown #1 is a turn-2 question, not a length question

`mode.rs:164` — *"every turn after the first resumes with `session/load`"*. So a
resume happens on turn 2 of any conversation, not only after a restart. And
`mod.rs:660` sends `SetSessionModeRequest` unconditionally after **both**
`session/new` and `session/load`, with a rejected `set_mode` **refusing the
turn**.

**So the failure the horizon feared cannot happen as written.** A mode that did
not survive a load would not produce a silent revert to `auto` and a deceptive
zero; the mode is re-asserted every turn, and a refusal to enter it stops the
turn loudly. What is genuinely untested is narrower: whether the re-send is
*necessary*, and whether `set_mode` after a load ever fails.

### Unknown #2's sibling is already fixed, and the fix names the untested half

`registry.rs:158` records a cross-conversation collision found by running: two
ACP conversations, JSON-RPC ids per-connection, *both agents opened with `0`*,
so the second `park` evicted the first and **both turns denied instantly while
their panels still said they were waiting for a person.** Fixed by scoping the
key to the turn.

That is the two-*conversation* case. The horizon's unknown is the
two-request-on-**one**-connection case, which the fix does not address and which
remains untested.

---

## The four, as observed

| # | unknown | result |
|---|---|---|
| 1 | mode survives `session/load` | **answered — no, and the re-send is load-bearing** |
| 2 | two requests, one connection | **cannot arise with this agent** |
| 3 | a turn that vanishes | **not observed**, 4/4 turns closed |
| 4 | compaction cadence under ordinary work | **not reached** — weaker than the other three |

### 1. The mode does NOT survive `session/load`. Warp's re-send is what saves it.

**This corrects what I wrote here an hour earlier.** My first entry said the mode
"is in force after a load — answered, yes", on the evidence that turn 2 raised a
permission request. That evidence is real and the conclusion drawn from it was
wrong, because it skipped the mechanism.

The event log settles it. `session_mode` is written once per turn from the
**agent's own reply**, and on all four turns it reads:

```
02:31:16  session_mode   current `auto`; offered `auto` … `default` … `plan` …
02:33:23  session_mode   current `auto`; …
02:34:26  session_mode   current `auto`; …
02:40:16  session_mode   current `auto`; …
```

Turns 2–4 each resumed via `session/load`, each **after** Warp had set the
session to `default` on the turn before — and each load reply reports the session
back in `auto`. `describe_current` reads `state.current_mode_id`, and `mode::log`
is handed `advertised`, which is `loaded.modes` straight off the reply. This is
the agent's word, not Warp's cache.

Meanwhile the agent asked for permission on **every** turn, which is behaviour
`auto` does not have — in `auto` its classifier answers and Warp sees zero
requests (T14.18). So by the time each prompt ran, the session was in `default`.

Both halves together: **the session comes back from a load in `auto`, and
`mod.rs:660`'s unconditional per-turn `set_mode` is what puts it back.**

**What I cannot distinguish from outside**, and it does not change the
conclusion: whether the agent genuinely reverts its state on load, or merely
reports the session's *original* mode in the load reply while still being in
`default`. Telling those apart needs the re-send suppressed for one turn, which
is a code change and so is out of scope mid-run. The falsifier is exactly that:
send `set_mode` only on `session/new`, resume, and see whether the agent still
asks.

**Why this is the run's most consequential finding.** `.fork/GOAL.md` describes
the re-send as happening *"because it is untested"* — the tone of a belt-and-
braces measure someone might later tidy away as redundant. It is not redundant.
On this evidence it is the only thing keeping a resumed session inside the
fork's consent model, and deleting it would produce precisely the disguised
failure the horizon named: every turn after the first silently in `auto`, the
agent's classifier answering, and an event log showing **zero permission
requests** that reads as "nothing needed asking".

That line deserves a comment saying so, and the horizon forbids me writing it
mid-run. It is the first ticket out of this run.

### 2. Not observed, and the mechanism is the agent's

Prompted explicitly to *"issue both tool calls in parallel in a single message"*
for two independent edits to two different files, `claude-agent-acp` **serialized
strictly**. The first request parked for 108 seconds with the turn still
`in_progress`; the second was not raised until the first was answered, and then
parked for 120 seconds alone.

So the two-requests-on-one-connection case **cannot arise with this agent**, and
a blocked dispatch loop could not have shown in this run. This is a *"not
observed"*, not a *"does not exist"*: the serialization is the agent's behaviour,
so the fork's exposure here remains untested rather than absent, and it joins the
list of things that are facts about `WARP_FORK_ACP_COMMAND`'s argument rather
than about this codebase.

One incidental reassurance: a request parked **108 seconds without the turn
timing out**, on both sides.

**Sharpened by the event log.** Multiple requests *per turn* are ordinary — the
ids run `…:0`, `…:1`, `…:2`, `…:3` within a single turn, so the connection
carries several. What never happens is two parked *at once*: turn 3's second
request was raised at `02:36:44.884`, **0.1 s** after the first was answered at
`02:36:44.781`. The agent had it queued and released it the instant the first
resolved. So the fork's dispatch loop was never asked to hold two, and the
id-scoping (`{turn}:{rpc_id}`) is doing real work within a turn as well as
across turns.

### 3. No turn vanished

Four turns, four closes: one `session_start` (turn 1) plus three
`prompt_submit`, against four `stop` events, every one matching its prompt.
`permission_request` and `permission_replied` are balanced 8/8, so no ask was
left `unanswered`. **Not observed, with the instrument running** — which is a
result, and is not the same as the failure being absent.

**"Not observed" is a result here; "not looked for" is not.**

## Refusals on the wire

**Observed, turn 1, recorded verbatim from `agent approvals` rather than from
memory.**

```
approval_id     1343b951-e27c-492a-bf2a-9f80fbab9639:0
tool_name       edit
summary         Edit CLAUDE.md
acts_on         /home/effatha/git/warp/CLAUDE.md
digest          6a2ebe329707b36c223428dccd6ac6aea6b2899d4921c337e0d573d0b7c83dc1
options_offered ["Deny", "Allow Once", "Always Allow"]
```

**`Deny` is first** — the exact ordering `acp_permission.rs` exists because of,
re-confirmed against 0.70.0 rather than trusted from its 2026-08-27 measurement.

Warp's answer on the wire was `keystroke: "reject_once"` — a per-call rejection,
**not** `RequestPermissionOutcome::Cancelled`. The event log records the pair:

```
seq 10  permission_request   call_id toolu_01XDMsxcSQJCLqnv8QYQYhU2  tool_name edit
seq 11  permission_replied   call_id toolu_01XDMsxcSQJCLqnv8QYQYhU2
                             decision "denied"  answered_by "control_plane"
```

**The ACP path carries a `call_id`.** `TR-EVENTS-B` — no call id on the payload —
is a fact about the *Claude Code plugin hook*, and three files record it in terms
general enough to be read as covering both paths. It does not cover this one.

**The agent survived the refusal**, which is the property that made this pairing
the recommendation:

> I see the edit was refused. Let me check whether you'd like a different
> phrasing before I retry, or if you want to hold off entirely.

Re-confirmed rather than trusted. The whole consent loop then closed on turn 2:
ask → park → `agent approve` → **the write landed on disk**.

### 4. Not reached, and that is a weaker result than the other three

Seven turns of real documentation work did not come close to compacting.
Probed directly on turn 7, with reads denied so the answer had to come from
memory rather than from the transcript file:

> No compaction or summarization notice has appeared anywhere in this
> conversation that I can see. I still have your original first message verbatim
> in context, including its exact wording…

— and it then quoted turn 1 back correctly. Warp's own transcript for the
conversation is **18 KB**.

**Say what this is.** #3 is *"not observed with the instrument running"*. #4 is
*"the condition never arose"*, which is weaker, and the distinction matters
because there is no compaction detector to have been running in the first place
— that is the unbuilt half T14.13 identified. The cadence under ordinary work
remains unmeasured, and the retracted *"every five or six turns"* figure stays
retracted rather than being replaced by anything from this run.

Reaching it needs either a much longer session or deliberately bulky reads, and
the second would reproduce exactly the context-hostile payload that made the
original figure worthless.

---

## Instruments, checked against each other

The run confirmed several things by running that were previously held by test or
by argument:

- **The whole consent loop closes.** ask → park → `agent approve` → **the write
  lands on disk**, verified with `git diff` and not with the tool's own
  say-so. 14 requests: 1 denied, 13 approved.
- **`WARP_FORK_TRANSCRIPT`'s hardening works, residual and all.** This run's
  transcript is `600` in a `700` directory, and `keep_dir_out_of_git` wrote its
  `.gitignore` — a fix that shipped with **no test** and is now exercised. The
  documented residual is visible in the same listing: every `644` transcript is
  dated **Aug 30** (pre-fix), every Aug 31 one is `600`. *"A dormant
  conversation is never rewritten"*, observed in the wild.
- **One turn, one file** (T14.15) holds: 74 events for the conversation in a
  single JSONL under Warp's id, each carrying the agent's `linked_session_id`.
- **`permission_request`/`permission_replied` balance 8/8** in the logged
  window, so nothing was left `unanswered`.

## Frictions

*Recorded, not fixed. Frictions that merely annoy are the success condition.*

- **The agent shells out for what its read tool does.** Writing the T14.21
  ticket, it ran `awk 'NR>=9092 && NR<=9092+400' … | grep -n … | head -20` to
  find a section boundary. `CLAUDE.md` steers explicitly against this and the
  2026-08-31 measurement already found that steering does not hold; this is a
  third instance. In `default` mode every one of these is a permission request,
  so the cost is an ask, not a stall.
- **`agent read` returns an empty `prompt` field** on every exchange, so the
  transcript of a conversation read back through the CLI has the answers without
  the questions. The prompts *are* in the event log and in the `.md` transcript,
  so nothing is lost — but the instrument a script would reach for first is the
  one missing half the exchange.
- **`stop` events carry an empty reason.** Four turns, four stops, no reason
  string on any. Fine while turns end normally; it is the field that would
  matter for a turn that vanished.

## Board corrections this run produced

- **`.fork/GOAL.md` says seven stale `- [ ]` boxes. It is six.** T14.14's box is
  correctly unticked — only its seam shipped, which `GOAL.md` itself says
  elsewhere in the same file. Audited box by box from each ticket's own as-built
  text; the six that landed are now ticked.

---

## Finding: the mode note hedges about something Warp already knows

**Not fixed — the horizon forbids fixing mid-run. Recorded for a ticket.**

The panel note that turn 1 displayed, verbatim:

> This session is in the agent's `auto` mode, which the agent describes as “Use a
> model classifier to approve/deny permission prompts”. `WARP_FORK_ACP_MODE` asks
> for the agent's `default` mode … so Warp is requesting it. **Whether the agent
> honours the request is the agent's to answer.**

Both halves of that are wrong by the time a person reads it, and the code makes
them wrong.

**The hedge is answered before it is printed.** `mod.rs` runs in this order:

```
651  if let Some(reason) = decision.refusal() { return Err(...) }   // turn refused
654  if let Some(mode) = decision.mode() {
660      send_request(SetSessionModeRequest::new(...)).await
663      if Err  -> return Err(mode_request_failed(...))            // turn refused
670      mode::acknowledged(&conversation_id, mode)
672  if let Some(note) = decision.note() { emit(note) }
```

The note is emitted at line 672. Every path that reaches it has already had
`set_mode` return `Ok` — a failure returns at 663 and the turn never runs. So
the sentence *"whether the agent honours the request is the agent's to answer"*
**can only ever be displayed in the case where the agent already answered, and
answered yes.** It is structurally never accurate. `mode::acknowledged` on the
line above is the code recording the very fact the note says is unknown.

**And `current` names the mode the session is leaving.** The note is built from
`describe_current(state)`, which is the mode advertised in the `session/new`
reply — *before* Warp's `set_mode`. So it says "this session is in `auto`" about
a session that is in, or is about to be in, `default`. A reader's honest takeaway
is *"in auto, default requested, outcome unknown"*, when the truth is *"in
default, and Warp knows."*

**Why this one matters more than its size.** `mode.rs` exists because a panel
session ran under a policy nobody was told about; its own header calls that
"unreached". The note is the entire remedy — and it currently understates what
Warp knows, in the direction of sounding less certain about the user's
protection than it is entitled to be. It is also the exact defect class this
fork's `CLAUDE.md` opens with: a paragraph that was true when written and was
falsified by the code that grew around it. The ordering at 651–672 is careful,
recent work; the note predates the `refusal()` arm that made its hedge moot.

**Suggested shape, not applied:** emit the note from a state that knows the
outcome — `current` should be the mode now in force, and the hedge should become
a statement that the agent accepted the request. Both facts are in hand at 672.
