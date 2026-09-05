> Idea I19, split out of `.fork/IDEAS.md` on 2026-09-05. History; read the section you came for.

# I19 — agent ergonomics: what the thing inside Warp can feel

**Asked 2026-08-30, as a thought dump, and worth organising because the question
underneath it is one this fork has never asked.** Recorded close to the
maintainer's words, then graded.

## The ask, as given

> Interactive setup debugging sessions — put an agent in warp, you drive here
> (or an orchestrator in warp drives, maybe try both?) one acts as the dev, the
> other is the inside agent, both are debuggers/troubleshooters/optimizers.
>
> what can the agent see? what does it need? is it missing anything? what does
> its perception provide us that we dont have access to now? keyword: **AGENT
> ERGONOMICS**.
>
> **Harness Feedback:** pass the steering messages, error facts/types/messages,
> like give it some haptics, let it feel the shape and friction, watch the edges
> and work around them as we can.

## First, the goalpost question, answered honestly

> Can all of this be done from INSIDE Warp? Rather than Claude driving from
> outside with warpctrl, is there a mechanism (orchestrator perhaps)?

**Yes, and it is built.** `warpctrl graph run` is a task graph — "a plan of
agents, in a file, with edges between them", with `schema`, `check` and `run`.
`agent spawn` puts a child in a hidden pane; `agent prompt`, `read`, `cancel`,
`approve`, `deny`, `reveal`, `settle` are the verbs it drives them with. T7
built the fan-out and `graph.rs` is the file this board holds up as the standard
— a run-scale orchestrator that added *zero* new app surface.

**So the goalposts did move, and it is worth naming which way.** The horizon
says *"a working day on the fork, driven from the fork"*. Most of what has
actually happened is Claude outside, driving `warpctrl` in. That is not cheating
— it is how the control plane gets exercised — but it is not the destination
either, and the distinction had gone quiet.

**And there is a real asymmetry to design around, not a bug.** An agent inside a
pane calling `warpctrl` reaches the **whole 114-action catalog**, because the
broker authenticates it by peer UID. A phone reaches five. So "orchestrator
inside Warp" is *more* privileged than the remote path, not less, and any
security argument for it cannot borrow the pairing list's reasoning. That is the
first thing to write down before building.

## What already exists underneath the rest

- **What the agent did**: `WARP_FORK_EVENT_LOG`, one JSONL per session,
  `tool_start`/`tool_complete` with outcomes.
- **What was said**: `WARP_FORK_TRANSCRIPT`, and it already carries the piece
  the agent's own store lacks — *why* a call was refused, and (measured
  2026-08-30) the text of a failure.
- **Live**: `events.subscribe`, `agent read` (streams mid-turn), `agent list`
  with `quiet_for_seconds`/`waiting_for_you`.
- **Steering, one direction**: the transcript pointer already rides every prompt
  as its own content block, and Warp's own asides are marked `[Warp]` and kept
  out of the file. **So a channel for Warp to say something to the agent
  mid-conversation exists and is in use.** That is the mechanism "harness
  feedback" would extend, and it means the idea is a widening rather than a
  build.

## The part that is genuinely new, and genuinely interesting

Everything above is *Warp watching the agent*. The maintainer's question is the
inverse and this fork has never asked it: **what does the agent perceive that
Warp cannot?**

An agent in a pane has a vantage nothing else here has. It sees the rendered
block, the exit code, the timing, the shape of its own friction — the 8 seconds
before a denial killed a turn, the `wc -l` reflex, the moment a compaction ate
its context. Warp measures these from outside and infers. The agent *has* them.

The concrete version: **stop treating the friction log as something a person
writes afterwards, and let the agent emit it as it happens.** Every entry in
this fork's friction record was reconstructed by a human reading logs. An agent
that could say *"that ask cost me the turn"* at the moment it happened would be
a better instrument than any of the ones T14.10 built, because it reports the
cost rather than the symptom.

## Is it plausible?

**The two-agent debugging pair: yes, cheaply, and it is nearly free.** One agent
in a pane and one driving from outside already works — this session did it. The
untested half is *both inside*, one orchestrating via `graph run`. The parts
exist; nobody has run it. That is a measurement, not a build, and it is exactly
the shape this board says to prefer.

**Harness feedback: yes, and narrower than it sounds.** The channel exists (the
`[Warp]` content block). What would ride it is the material Warp already holds
and currently drops: the refusal reason, the error kind, the fact that a
permission ask is what stopped a turn. **Marked unverified:** whether an agent
told *"your last call was refused because it asked to leave the project
directory"* behaves any differently from one that just saw a failure. That is
the whole bet, it is cheap to test, and it should be tested before anything is
built — a steering message that changes no behaviour is noise in a prompt.

**Agent ergonomics as a standing question: yes, and it is the most valuable part
of the dump.** Not a feature — a habit. After any panel session, ask the agent
what it could not see. This fork's method is *run it*; this extends it to *ask
the thing that ran it*.

## The smallest version that is still the idea

1. **Ask.** End a real panel session with: *what did you need and not have?*
   Zero code. The answer scopes everything below it.
2. **Run the pair.** Two agents, one orchestrating through `graph run`, on a
   real ticket. Record where it breaks. The verbs exist; only the plan is
   missing — which is precisely what T7 found.
3. **Widen the aside** to carry one refusal reason, and measure whether the next
   turn goes differently. If it does not, stop.

---

