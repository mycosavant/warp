> Ticket T7, split out of `.fork/TASKS.md` on 2026-09-05. History; read the section you came for.

## T7 — Work that outlives a turn

Raised by the user, 2026-08-20, as three ideas that felt related and were hard
to separate: telling a child agent what it may do, telling child agents what
order to run in and who to hand results to, and planning a release a year out.
The first is a guardrail and belongs in T6.6. The other two are the same shape
at two scales, and **the scales want different homes**. This section is the
argument for which.

### The runtime already exists. The *plan* does not.

Worth establishing first, because it changes what is left to build. Warp can
already do all of this at runtime — these are entries in `ToolType`:

    RUN_AGENTS = 32              spawn a batch of children
    SUBAGENT = 16                spawn one
    SEND_MESSAGE_TO_AGENT = 27   pass a result to a named agent
    WAIT_FOR_EVENTS = 33         block until something arrives

with `AIAgentInput::MessagesReceivedFromAgents` and `EventsFromAgents` on the
receiving side, and `ConversationStatus::WaitingForEvents` as the visible
state. That is a message-passing concurrency substrate, and "B waits for A,
then A hands B its result" is expressible in it today.

So what is missing is not the mechanism. **It is that the sequencing is a
decision the model makes in the moment, rather than a declaration made before
the run.** That is the whole of the user's instinct that this should be "more
deterministic than saying plan-then-spawn-a-reviewer, but not programmatic in
the same way". The difference between the model *choosing* to wait and the
plan *saying* it must is the feature.

### Two edges, or one?

The user described `depends-on` (ordering) and "dictate what and where the
child hands off to" (routing) as separate ideas. They collapse:

> **A dependency is an edge that carries a payload.** `hands-to` is
> `depends-on` plus "and here is what to pass".

One edge type with an optional payload spec covers both, and the collapse is
worth taking because two edge types would have to be kept consistent — a graph
where B `hands-to` C but C does not `depends-on` B is a bug you can draw.

Which makes the unit:

    node: id, prompt, agent/harness/model, tool allowlist, working directory
    edge: from -> to, optional payload ("pass your diff", "pass your summary")

Nodes are the T6.6 spawn parameters, exactly. **`RunAgentsAgentRunConfig`
already carries `name`, `prompt`, `title` and a per-child `model_id`** — so the
node is two fields short (tool allowlist, cwd) of something that exists.

### Where the graph lives, and why not in Warp

Three candidates. The middle one is what happens today, and it is the one that
fails.

**In Warp, as a model.** Rejected. Warp's job is running agents; holding your
project plan is not that. And it dies with the process — the plan would not
survive a restart, which is the one thing a long-horizon plan must do.

**In the lead agent's context.** This is the status quo, and it is why the
question gets asked. The plan lives in the context window, so it degrades
exactly as the work gets long enough to need it. **This is the real reason
`/compact` matters** (T6.7): compaction is the moment the plan is most at risk,
and no amount of careful summarization makes a context window a durable store.

**In a file, with a small runner.** Recommended. The plan is a document in the
repository; a runner — an external process, or the lead agent in a loop — polls
`agent.list`, fires `agent.spawn` when a node's dependencies are satisfied, and
reads results with `agent.read`. Everything it needs is T6.5 plus T6.6.

The argument for the file, beyond durability: it is diffable, reviewable, and
lands in a commit next to the work it describes. And **the fork is already
doing this by hand** — `.fork/tickets/` is a dependency graph in prose, with
T6.1 blocking T6.2 and T6.5 opening T6.6 and T6.7. Making that machine-readable
is the entire feature, and the fact that it was written by hand first is
evidence the shape is right.

Note what this buys that is easy to miss: **the plan surviving compaction is
the actual requirement**, not the scheduling. Scheduling is a `while` loop over
statuses. Durability is the hard part, and a file solves it completely.

### And the year-long version: don't build it

The user's EOY26 example — many workstreams, some parallel, some blocking — is
the same graph and a different system, and the tell is the failure mode:

| | Run-scale | Project-scale |
|:--|:--|:--|
| Horizon | minutes to hours | weeks to months |
| Lives in | one session | many, and outlives all of them |
| A node fails and | you retry it in 30 seconds | someone renegotiates a date |
| Readers | one lead agent | people, in a meeting |

Those want different storage, different UI, and different notions of "done".
Merging them produces something that is a bad scheduler *and* a bad issue
tracker.

**Recommendation: the project tracker owns *what and when*; the orchestrator
owns *how, right now*.** GitHub Issues, Linear or a milestone file already do
the first, adequately and — the part a home-grown one cannot match — somewhere
people already look. The join is one sentence of the lead agent's job:

> Given this milestone, emit a run-scale graph.

That keeps the run-scale graph small enough to be worth trusting, and keeps the
project-scale record somewhere a human can argue with it.

- [x] **T7.1** A run-scale task graph: schema, and a runner over T6.5/T6.6.
      Unblocked by T6.6, which supplies `agent.spawn`, `agent.read` and
      `agent.cancel` — without `agent.read` a graph can sequence work but not
      hand anything along it, which is half the point. Shipped as
      `warpctrl graph`, adding no actions — see "T7.1 — as built" below.
- [x] **T7.2** Read a milestone from an issue tracker and emit a T7.1 graph.
      Deliberately last, and deliberately thin: the moment this grows a
      scheduler of its own it has become the thing this section argues against.
      It stayed thin — one command, `graph schema` — because running it showed
      that the interesting half cannot be parsed. See "T7.2 — as built" below.

### T7.1 — as built

`warpctrl graph check <plan.toml>` and `warpctrl graph run <plan.toml>`.

**The catalog is 96 actions before and after.** That is the headline. T6.6
built the verbs; a graph is a composition of them, so the runner is a `while`
loop in the CLI over `agent.spawn` and `agent.read` and the app gained nothing.
A feature that adds no surface is one that cannot break the surface.

#### The format

```toml
[defaults]
allow_tools = ["read-only"]

[[node]]
id = "survey"
prompt = "List every file under src/ that still calls the old API."

[[node]]
id = "fix"
prompt = "Migrate those files to the new API."
allow_tools = ["read-only", "APPLY_FILE_DIFFS"]
needs = [{ node = "survey", pass = "the list of files" }]
```

A node is the `agent.spawn` parameters — prompt, name, tool allowlist — and
nothing else. `[defaults]` exists so a plan whose every node is read-only says
so once: a plan that repeats the restriction on every line is a plan where one
line will eventually be missing it.

**One edge type**, as the T7 argument concluded. `needs = ["survey"]` is
ordering; `needs = [{ node = "survey", pass = "…" }]` is ordering *and* a
handoff. The `pass` phrase is a label rather than a filter — the whole of the
upstream answer is appended either way, under `--- From \`survey\` (the list of
files):`, because a wall of text under no heading is the kind of context an
agent quietly ignores.

Edges are written on the node that waits, because that is the direction a
reader asks the question in: standing at `fix`, what does it need?

#### What is refused before anything spawns

Both of these are otherwise discovered halfway through a run, with children
already running and a partial result to reason about.

* **A cycle**, with its members named. At runtime a cycle is invisible — the
  scheduler stops finding work, which reads exactly like a hang. Found by
  Kahn's algorithm, where what *cannot* be drained is the answer.
* **A misspelled field**, via `deny_unknown_fields`. This matters more here
  than in most places, because the fields are the guardrails:
  `allow_tool = ["read-only"]` silently accepted is a node that runs with no
  restriction at all, discovered by reading what it did.

Also refused: an unknown node in a `needs`, a duplicate id, a node that needs
itself, an empty plan.

`graph check` needs no running Warp and prints the plan in waves:

    4 nodes, 3 in sequence
      1. colours
      2. count, shout
      3. report

Waves rather than a flat list because the parallelism is the interesting part —
a plan whose every node is in its own wave will run one agent at a time, which
is usually not what its author drew.

#### Failed is not skipped

A run that reports six failures when one node failed and five were waiting on
it has buried the only fact worth acting on. So `Skipped { blocked_by }` is its
own state, and it names the *nearest* blocker; the chain back to the root cause
is readable from the other entries.

A failed node stops its own dependents and nothing else. Branches with nothing
wrong with them run to completion, and the process exits non-zero at the end.

#### The race that would have made the whole thing silently useless

A conversation polled in the instant after `agent.spawn` is not busy *yet*. Read
as "finished", it hands the next node an empty string, and the graph runs to
completion having done nothing — with every node reporting success. The test is
therefore `!is_busy && last_exchange.is_complete == Some(true)`: an exchange
that exists and has a finish time cannot be a turn that has not started.

#### Verified on Linux, 2026-08-20

A diamond, run against a lead conversation with `--parent`:

    colours: done — Crimson, teal, amber
    count:   done — 3                              <- these two started
    shout:   done — CRIMSON, TEAL, AMBER              together
    report:  done — COUNT=3 LIST=CRIMSON, TEAL, AMBER

`report` joining two handoffs into one line is the proof that both edges
delivered. `agent.list` afterwards showed all four as hidden children of the
lead.

Then the part that matters more, with a node given `allow_tools = ["Bash"]` —
which is Claude's name for it, not Warp's, so `agent.spawn` refuses:

    bad:       failed — `Bash` is not a tool. Use `read-only`, or a ToolType
                        name such as READ_FILES or RUN_SHELL_COMMAND.
    after-bad: skipped — `bad` did not finish
    unrelated: done — still-here
    exit=1

#### Decisions worth recording

* **`--max-parallel`, default 4.** Every node is a real agent with a real model
  behind it. The graph decides what *may* run together; this decides how much
  of that actually does.
* **No timeout by default.** A node can legitimately sit for a long time —
  `blocked` means it is waiting for a person to approve something — and killing
  it throws the work away to no purpose. `--timeout` is there for unattended
  runs, and the progress output carries the conversation's own status so a
  stuck node reads as `blocked` rather than as nothing happening.
* **No retries.** A node fails once and stays failed. Retrying an agent turn is
  not idempotent — it may already have written files — so it is the plan
  author's call, not the runner's.
* **`agent.read --last 1`.** A node is one prompt and its answer. Reading the
  whole transcript would hand the next node the handoff it already received,
  wrapped in its own reply.

### T7.2 — as built

One command, `warpctrl graph schema`, and a verified loop. The reason it is
one command rather than a `graph from-issues` parser is the finding below,
which is the whole of this task.

#### A real milestone contains no edges. None.

This was scoped by reading actual trackers rather than by imagining one.
`mycosavant/warp` has issues disabled — the GitHub default for a fork — so the
tracker read was upstream's, which is real and public. Two milestones, 21
issues, checked for every convention a parser could key on:

| Convention | Hits |
|:--|:--|
| `depends on #N` / `blocked by #N` / `requires #N` | 0 |
| Task-list issue references (`- [ ] #N`) | 0 |
| Sub-issue / dependency links | 0 |

The only task-list checkboxes that matched at all were `- [x] Yes`, the "have
you searched for existing issues?" box in the bug-report template.

And the bodies are not task specifications. They are user-submitted prose in an
issue template — `### Discord username (optional)`, `### Describe the bug`,
`### To Reproduce`. A milestone named `SSH V2` is seventeen unrelated SSH bug
reports; `Emacs` is four unrelated Emacs ones.

**So a mechanical generator would emit N nodes and zero edges, every time.**
It would be deterministic and it would be confidently wrong about the only
part that matters, because the ordering information is not in the tracker to
extract. That vindicates the T7 argument exactly: the tracker owns *what*, and
it genuinely does not own *when* in any machine-readable form. Deciding what
depends on what means reading prose and exercising judgment, which is an
agent's job and not a parser's.

Hence no `graph from-issues`. Fetching is already one `gh` command; wrapping it
would add a dependency on a tracker's shape in exchange for the boring half.

#### What was actually missing

That the agent had no way to learn the plan format except a human pasting
documentation into its prompt — which is the human this was supposed to
remove. `warpctrl graph schema` prints the format as an annotated plan that is
itself valid and runnable, asserted by a test rather than claimed in a comment.

It leads with the thing a generating agent gets wrong: a child does not inherit
the transcript of whatever wrote the plan, so every prompt has to stand alone.
That mistake is invisible until the run produces confidently context-free
answers.

#### The loop, end to end, on a real milestone

One sentence of the lead agent's job, exactly as the T7 argument predicted, with
no format pasted into it — the agent was told to run `warpctrl graph schema`
itself:

> Read the `Emacs` milestone of `warpdotdev/warp`. Run `warpctrl graph schema`
> to learn the plan format. Emit a task graph that triages that milestone: one
> node per issue that summarises the bug and names the area of a terminal
> emulator it touches, and a final node that proposes an order to fix them in.
> Write it to `triage.toml`, validate it with `warpctrl graph check`.

One turn, four tool calls — `gh`, `graph schema`, `Write`, `graph check` — and
`OK`. The plan it wrote:

    5 nodes, 2 in sequence
      1. issue_1860, issue_2064, issue_402, issue_450
      2. order

with `allow_tools = ["read-only"]` inherited from the schema's default, each
issue's full text embedded in its own prompt (the standalone rule, followed),
a fixed reply format so the join could read them, and four labelled handoffs
into `order`.

Then `graph run`, which ran the four triages in parallel and joined them:

    1. #1860 — M-backspace sends M-b "ackspace" — highest severity, contained
       to the Option-as-Meta encoding path.
    2. #2064 — Command key combo not going to Emacs — same keyboard layer, so
       the person is already in the key-dispatch code.
    3. #402 — split pane doesn't draw properly — equally severe but sits in
       the grid/renderer, a separate area.
    4. #450 — ctrl+z shown as an error — cosmetic, isolated, natural cleanup.

    RATIONALE: … if two people are available, #402 should run in parallel with
    the #1860/#2064 pair, since the code is disjoint.

Worth noticing what the edges in that plan *are*. The milestone had none, and
the agent did not invent dependencies between the bugs — it drew edges between
the units of **work it proposed to do**: analyse each, then read the analyses.
The tracker supplied the nodes; the agent supplied the shape. That is the
division the T7 argument asked for, arrived at without being told.

And the last line of the rationale is a graph: the join node reasoned about
what could run in parallel, which is the plan it would emit next.

#### Two things running it turned up

* **`XDG_CONFIG_HOME` broke `gh`.** The scratch-profile trick from T6.7 —
  used to get `is_any_ai_enabled = true` without editing the real
  `settings.toml` — also relocates every other XDG-config tool, `gh` included,
  so the agent found itself unauthenticated. A rig defect, not a product one,
  and it did not need the rig at all: `agent.prompt` and `graph` never consult
  `is_any_ai_enabled`, so only T6.7's slash-command path needed the override.
* **One turn came back `cancelled`, and it is not explained.** The first
  attempt — the one with `gh` broken — ended `ConversationStatus::Cancelled`
  after four tool calls, with nothing in the log. Nothing cancelled it from
  this side. `Cancelled` comes from a real `StreamCancellation`, and the only
  `CancellationReason` that could plausibly fire unprompted is
  `UserCommandExecuted`: the pane's shell is live while the agent streams. That
  is a candidate, **not a diagnosis** — it did not recur, and it was not
  reproduced. Recorded as T5.5 rather than guessed at.

---

