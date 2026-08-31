# Friction log — clean run, 2026-08-31

Conditions, chosen from what the earlier run measured rather than by preference:

- **`claude-agent-acp` 0.70.0 with `WARP_FORK_ACP_MODE=default`.** Not `opencode`.
  Two reasons, both measured today: `opencode` abandons a turn after a refusal
  and `claude-agent-acp` does not; and in `default` mode Warp is actually in the
  consent loop rather than the agent's own classifier answering first.
- **Every question scoped to named files**, with an explicit boundary. Six audits
  earlier today: scoped ones cost 0-1 asks each, the one unscoped one cost 16 and
  a lost turn.

Success condition per GOAL.md: nothing here stops a turn.

| turn | asked | asks | stopped? | note |
|---|---|---|---|---|
| 1 | audit `local_sync/mod.rs` (scope too tight — module decls only) | **0** | no | 38s. Said so plainly instead of fabricating. My boundary error, not the fork's. |
| 2 | audit `local_sync/apply.rs` + tests | **0** | no | 120s. Two doc findings; running that crate's tests then found a real regression. |

| 3 | `format.rs` byte stability | **0** | no | 135s. Found the alias round-trip asymmetry. |
| 4 | `wsl_transport.rs` + `wsl.rs` | **0** | no | 84s. Found `SpawnFailed` unreachable on the path it was written for. |
| 5 | `mode.rs` session modes | **0** | no | 82s. Found the module header describing a discarded first cut. |
| 6 | `handlers/events.rs` | **0** | no | 38s. Nothing wrong; my boundary too tight for two of three questions. |
| 7 | **adversarial check of this morning's own fix** | **0** | no | 154s. Found the fix's recursion untested. |

| 8 | `handlers/approvals.rs` | **0** | no | 106s. Module doc asserts approval "is a keystroke"; `answer_acp` is not one. |
| 9 | `event_log/warp_agent.rs` | **0** | no | 120s. Preview doc says "two things"; three arms. |
| 10 | `acp_agent/translate.rs` | **0** | no | 73s. "Permission requests never reach this file" — T14.17 staled it that morning. |
| 11 | `crates/local_control/protocol.rs` | **0** | no | 194s. **Nothing wrong.** Every `skip_serializing_if` enumerated. |
| 12 | `fork.rs` | **0** | no | 199s. "Default off, unlike every other predicate" — three others are too. |
| 13 | `local_control/console.rs` | **0** | no | 106s. CSP comment contradicts itself about `img-src`. |
| 14 | `acp_agent/registry.rs` | **0** | no | 134s. A doc comment on the wrong function; one missing blank line. |
| 15 | `graph.rs` resume guard | **0** | no | 78s. Fingerprint doc oversells completeness against a comment a page later. |

## Totals

- **turns: 15**
- **permission requests: 0**
- **turns stopped: 0**
- mode `default` confirmed in `agent list`; `permissions_denied` absent, correctly

## What it produced

**The biggest finding of the day, and it came from running tests rather than from
the audit itself.** `agent-client-protocol` — this fork's own T14.5 dependency —
enables `serde_json/preserve_order`. Cargo unifies features across the build, so
`serde_json::Map` became insertion-ordered *everywhere*, including
`drive/local_sync/format.rs`, which relied on it being a sorted `BTreeMap`.

The fork's git-backed Warp Drive was emitting a different byte stream for
identical content: spurious diffs in the one feature whose whole job is to be
diffable. **No line of `local_sync` changed.** Three tests had been red for an
unknown period, and the one named
`json_payload_keys_are_sorted_not_insertion_ordered` says in its own comment that
it exists for exactly this. The guard worked; nobody ran it.

## Friction

- **Turn 1's boundary was too tight** and cost a turn's usefulness — `mod.rs` is
  four `pub mod` lines. The scoping rule needs a second half: *name the files that
  contain the implementation.* The agent handled it correctly and said the
  question could not be answered in scope. Annoying, not stopping.
- **Nothing else.** Two turns, zero asks, zero stops, with Warp in the consent
  loop the whole time (`mode: default`, not `auto`).


## Turns 3-7

Seven turns, **zero permission requests, zero stops**, Warp in the consent loop
throughout (`mode: default`, confirmed in `agent list`). Five defects found, all
fixed, all pinned:

| turn | finding |
|---|---|
| 3 | `header()` wrote aliases for any object type; `from_parts` drops them for non-workflows — so the two were not inverses. Not live, gated anyway. |
| 4 | `SpawnFailed`'s doc says it is what you see without WSL. Most callers go through `output()` and got `IoError`, including `detect_platform` — the *first* thing anyone without WSL hits. |
| 5 | `mode.rs`'s header and CLAUDE.md both said an unadvertised mode id is "reported, not sent". It **refuses the turn**. Both were describing a first cut the file's own `Decision::Refuse` doc records as wrong. |
| 6 | Nothing wrong. |
| 7 | This morning's `sorted_keys` fix claims "at every depth" and no test proved past depth 1. Calibrated: strip the recursion and the *old* test passes while the new one fails. |

### The one friction, and it is mine

**Twice the boundary was too tight** — turn 1 named `local_sync/mod.rs` (four
`pub mod` lines) and turn 6 named `handlers/events.rs` (which returns a URL; the
stream lives elsewhere). Both times the agent said the question could not be
answered in scope rather than fabricating. Cost: a turn's usefulness, never a
turn.

So the scoping rule needs its second half: **name the files that contain the
implementation** — and if you do not know which those are, that is a locating
question first, asked separately.

### What the day's pattern turned out to be

Five of the day's findings were the same defect class, and none of them was a bug:

> **a doc that outlived its code** — the code was corrected, the prose above it
> was not, so the comment preserves a design that was considered and rejected.

Two were in files whose *other* comments argue the correction at length. All five
came from one question: *"name anything whose doc comment claims something the
code below it does not do."* That question is now in CLAUDE.md with the table.


## Turns 8-15

Eight more scoped audits. **Zero permission requests across all fifteen turns,
zero stops, Warp in the consent loop throughout** (`mode: default`,
`permissions_denied` absent on every conversation).

Seven more defects, all the same class, all fixed. One audit (`protocol.rs`)
found nothing wrong after enumerating every `skip_serializing_if` in the file
against whether absence is distinguishable from a default — which is what makes
the other seven worth believing.

## The day's actual finding

Twelve stale docs in one day, every one from a single question:

> *name anything whose doc comment claims something the code below it does not do*

**None of them is a careless comment.** Each was written carefully, was true when
written, and was falsified by a later change to the code beside it. Four are
*internally* inconsistent — the file contradicts itself and both halves are signed
work. One was wrong purely by **position**: a missing blank line had attached
`waiting()`'s doc to the next function, documenting that function as doing the
opposite of what it does and leaving `waiting()` undocumented.

**One of the twelve was mine, from that same morning.** T14.17 added two functions
to `translate.rs` that take a `ParkedRequest`; the header above them says
permission requests "never reach this file". Same person, hours apart, neither
noticing. Adding a function is exactly when the paragraph above it stops being
true and exactly when nobody re-reads it.

**Nothing in the toolchain sees any of this.** `cargo check --workspace
--all-targets`, `cargo test`, `./script/format`, `check_no_inline_test_modules` —
all pass with all twelve in place.
