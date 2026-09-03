# Handoff: the local permission classifier

---

## What you are doing

Deciding whether this fork should run a **local** permission classifier, and if
so, building the thing that makes that decision answerable with data.

**Read `.fork/CLASSIFIER.md` first — it is the ticket.** Everything below is
working context.

**The first deliverable is an evaluation set, not a model.** If you find yourself
choosing an architecture before you have labelled data, you have skipped the only
step that cannot be skipped.

## Read these, in this order

1. **`.fork/CLASSIFIER.md`** — the ticket. The posture argument, what already
   exists, the asymmetry the design must have, and the falsifier.
2. **`CLAUDE.md`** — the fork's method. Long; navigate by heading. The sections
   that bear directly on this: *"Method: run it"*, *"Look for the gate first"*,
   and everything about `WARP_FORK_EVENT_LOG`.
3. **`.fork/GOAL.md`** — the standing horizon, and it **freezes permission
   posture**. Read the boundary section below before you build anything.
4. **`.fork/IDEAS.md`**, the `I18` entry — the adjacent ticket. Route 3 there is
   close enough to this that they should be scoped together, and they are
   different objects.

## The one-paragraph version

A working session raised **44 permission requests in 50 minutes**, which ended
the run. Counted from its own event log: **30 reached the Windows host, 9 were
cargo, 7 were inside the session directory**. Meanwhile the configuration those
asks arrived under allows `python:*`, `node:*` and `cargo:*` — arbitrary code
execution — while a file *read* prompts. That is not a posture; it is two
unrelated defaults meeting, and it produces fatigue on the harmless half and
rubber-stamping on the dangerous half. The fork's standing objection to letting a
model decide turns out to be an objection to **Warp not being in the loop**,
which a local, logged classifier does not share.

## Start here

1. **Build the evaluation set from event logs that already exist.** Every
   `permission_request` line carries `tool_name`, `tool_input_preview`, `cwd`,
   `project`, and the matching `permission_replied` carries the decision. Label
   them. Logs live at:
   - Linux: `~/.local/state/warp-oss/events/*.jsonl`
   - Windows: `%LOCALAPPDATA%\warp\WarpOss\data\fork\events\*.jsonl`
     (from WSL: `/mnt/c/Users/<user>/AppData/Local/warp/WarpOss/data/fork/events/`)

   One file per **conversation**, appended as events arrive. **Read them in
   timestamp order, not filename order** — a reader inferring causality from
   `cat *.jsonl` will be wrong.

   The run-2 log is the richest single sample:
   `27357def-2174-47ff-b260-b8ce3918dea6.jsonl`, 228 lines, 44 asks.

2. **Try the rule before the model.** `kind` + `locations` covers `edit` and
   `read` exactly. For `execute` the command sits in agent-specific `raw_input`,
   which is where a rule gets fragile and where a classifier would earn its
   keep — and where it is most likely to be wrong. Measure what fraction of the
   44 a containment rule would have handled. **If it is most of them, say so and
   stop.** That is a successful outcome, not a failed one.

3. **Only then consider a model**, gated by the evaluation set from step 1.

## What already exists — look before building

`crates/input_classifier` is a working local inference pipeline:

- three `bert-tiny` ONNX models plus a tokenizer, **embedded in the binary** via
  `rust_embed` (`src/onnx/mod.rs:23-28`)
- two interchangeable runtimes behind features: `onnx_candle`, `onnx_ort`
- a panic guard that falls back to a heuristic classifier
  (`src/onnx/mod.rs:157-161`)
- `InputClassifierDecisionSource` — six variants recording **which path decided**

It classifies **Shell vs AI** and **cannot be repurposed as a risk classifier**.
What it gives you is proof that this fork can ship an embedded model, run it with
no network, fail safe, and record why — the hard part of the infrastructure,
already paid for. Treat it as the template, not the implementation.

**Trap:** no `nld_classifier_*` feature is in `app`'s `default` list. Verify with
`cfg!(feature = "nld_classifier_v3")` **in a test**, never by reading
`Cargo.toml` — a feature in `default` can enable others through its own
dependency list, and this fork has already been wrong that way once. It is a
`#[cfg]` that *removes code*, so `fork::FORCE_ENABLED` cannot reach it.

## The seams you will touch

| what | where |
|---|---|
| the single place every permission answer is decided | `crates/warp_cli/src/local_control/acp_permission.rs` — `choose(request, decision) -> Choice` |
| whether an option is even selectable | same file, `is_selectable` — it **asks** `choose` rather than restating it, deliberately; keep that |
| binding an answer to the request that was shown | `app/src/local_control/handlers/approvals.rs`, `digest_of` |
| **the disclosure hook, already built** | `event_log::Entry::answered_by` — names the surface that answered (`control_plane`, `panel`). A classifier is one more value there |
| where an ask is parked and rendered | `app/src/ai/acp_agent/registry.rs`, `app/src/ai/blocklist/inline_action/acp_approval.rs` |

## The design shape that is not negotiable

This fork builds consent **asymmetrically**: `agent.deny` needs no switch because
saying no can only ever make less happen, while `agent.approve` needs
`WARP_FORK_REMOTE_APPROVE`. Inherit that:

- **Escalating is free.** Turning an auto-approve into an ask needs no
  permission and no confidence threshold.
- **Auto-approving happens only inside an envelope the person declared.** The
  classifier chooses within it and never widens it.
- **Never silently deny.** A silent no wastes a whole turn, and measured, some
  agents produce no further output at all after a refusal. A classifier that
  would say no should ask instead.
- **Never emit an answer Warp cannot explain afterwards.** If the log cannot say
  *why*, the feature has reproduced `auto` with extra steps, which is the thing
  this exists to improve on.

## The boundary

**Permission posture is frozen** (`.fork/GOAL.md`), and it is the maintainer's to
lift.

- Building the evaluation set: changes nothing. In scope.
- Measuring what a rule would have handled: changes nothing. In scope.
- Writing the classifier behind a flag that is off: in scope, and say so.
- **Turning any of it on, by default or otherwise: out of scope.** Bring the
  measurement and let the maintainer decide.

## Instruments

- **`WARP_FORK_EVENT_LOG=on`** — one JSONL per conversation. Your corpus.
- **`warpctrl acp probe --output-format ndjson`** — the raw wire, including the
  permission request with its options and `_meta`. Line 1 carries
  `agentInfo: {name, title, version}`; **read it**, because a finding here was
  once published against the wrong version with the right version on screen.
- **`warpctrl agent approvals --output-format json`** — what is parked right now.
  Use JSON for anything a script decides on.
- **`shot.ps1 -Process warp-oss`** — screenshot the running window without
  raising it, for anything about what a person sees.

Pin the agent: `npx -y @agentclientprotocol/claude-agent-acp@0.73.0`. Unpinned it
resolves to whatever is newest, and 0.70.0 and 0.73.0 differ in ways that already
changed one conclusion.

## Two corrections to carry, so you do not inherit them wrong

- **`opencode` is not a tool anyone here uses.** It exists on this machine only
  because this fork's testing introduced it. `CLAUDE.md` builds a long permission
  argument on `opencode.json`; that is a worked example of a *shape*, not
  evidence about real exposure. The agent that matters is `claude-agent-acp`.
- **`auto` is Claude Code's shipped product default**, not an aberration. The
  settings file carrying `defaultMode: auto` is one Claude Code wrote. The fork
  has been treating the near-universal configuration as a thing to be corrected,
  and nobody decided that deliberately.

## Working rules

- Commits: `fork: <lowercase subject> (CLASSIFIER)`. The body says **what was
  found**, including when it contradicts something previously recorded.
- Corrections go in the commit that makes them *and* in the doc that was wrong.
- Calibrate every test by making it fail.
- When something has only been read, say so. A claim marked *measured* is
  measured **as of its date** — re-run it before building on it.
