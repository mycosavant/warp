> Ticket T13, split out of `.fork/TASKS.md` on 2026-09-05. History; read the section you came for.

## T13 — The run gate (ratified Tier 1, items 4 and 5)  ← ~~ACTIVE~~ done

> **Labelled ACTIVE until 2026-09-14 with all three boxes ticked.** Checked that
> day against `crates/warp_cli/src/local_control/graph.rs`, which has `review`,
> `rejected` verdicts and `--resume`. The kode-engine extraction §12 names as a
> dependency is carried in `T15-loose-ends.md`, not here.

> `.fork/archive/CONSOLIDATION.md` §4.1 orders these fourth and fifth, and they are the other
> half of the maintainer's sentence: T12 delivers *"check in on"*, T13 delivers
> *"keep things moving semi-autonomously"*. They land as validators over
> `crates/warp_cli/src/local_control/graph.rs`'s TOML — §5's *"migrate toward
> the file, not the schema"* — because that file is already the fork's best
> expression of the smallest-thing rule and it added zero app surface.
>
> **Constraint carried from §12: write these fresh, do not copy Tusk's.**
> Migrating a *concept* is free; migrating Tusk source into an AGPL tree before
> the §10-step-1 extraction is the one-way door that step exists to hold open.

- [x] **T13.1** `ZB-PLAN` — the sealed-subgraph guard, as `warpctrl graph check`
      over an existing plan file. **The guard had nothing to guard, and finding
      that was the ticket**: a plan that re-runs from scratch every time can
      never reuse evidence, so no edit to it can invalidate any. What was
      missing was Tusk's own §7 promotion trigger, and here that trigger is
      `--resume`. Built as record → resume → guard.
- [x] **T13.2** `ZB-CONTRACT` — per-assertion verdicts. A node declares what
      must hold after it, and the runner records a verdict *per assertion*
      rather than one pass/fail per node. This is the same detector T11.1 built
      for events, applied to work instead of to activity. **An assertion is a
      command, not a sentence** — the statement and the evidence are the same
      string — and a node whose assertion fails is `rejected`, a fifth state.
      Tusk's two open decides are both answered by the file: the contract lives
      on the node, and nothing produces a verdict except the command itself.
- [x] **T13.3** `ZB-REVIEW` — an independent completion-review gate: the agent
      that checks is not the agent that did the work. **There was no reviewer to
      build** — a review is a *node shape*, not a node kind, and Tusk's whole
      no-transcript overlay collapses into `agent.spawn`, which has no other
      mode. What was built is the fence: `review = true`, three refusals in
      `validate`, and the recipe in the schema. The tension with T13.2 resolves
      by demoting the verdict — **a model's answer may narrow acceptance, never
      widen it.**

