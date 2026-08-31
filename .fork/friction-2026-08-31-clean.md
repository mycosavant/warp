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

## Totals

- **turns: 2**
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
