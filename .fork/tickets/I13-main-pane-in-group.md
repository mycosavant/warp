> Idea I13, split out of `.fork/IDEAS.md` on 2026-09-05. History; read the section you came for.

# I13 — Main pane in a group

> *"Set pane as MAIN within a group."*

**Promoted, and it is better than I read it.** I had this as a layout feature —
one pane bigger, the rest arranged around it, master/stack as tiling window
managers have it. Your answer 2026-08-21:

> *"Also the CWD follow. We don't want each active pane to steal the file
> explorer. And also could be useful for orchestration, so `main` could also
> represent the lead agent."*

That is not a layout feature. **`main` is an anchor** — a designated pane that
other things point at instead of chasing focus — and it fixes a real defect in
my own scoping of [I6](I06-follow-the-cwd.md), where I had written "follow the
focused pane" and called the question small. It is not small and "focused" is
the wrong answer: glancing at a split would re-root your file tree. Following a
pane you *named* is stable by construction.

So `main` has at least three consumers, and they are the same one bit:

| consumer | what it reads `main` for |
|---|---|
| CWD follow (I6) | which pane's directory the explorer tracks |
| layout | which pane gets the large flex |
| orchestration | which pane holds the lead agent |

The third is the one that makes this a fork feature rather than a nicety. T6.6
and T7.1 built agent fan-out and a run-scale graph, and both have the same
unspoken question: *which pane is the one I am talking to?* Today that is
implicit. `main` makes it a thing you can name, and therefore a thing
`warpctrl` can name.

## The smallest version that is still the idea

One `Option<PaneId>` on `PaneGroup`, alongside the `pane_history` and
`focus_state` that already live there. Set from the pane's overflow menu.
`None` means today's behaviour, so nothing changes for anyone who never uses
it.

Then add consumers **one at a time**, starting with CWD follow, because a bit
with one consumer is easy to delete if the idea is wrong and a bit with three
is not. Layout second. Orchestration third, at which point it probably wants a
`warpctrl pane main --set/--get` to go with it.

Explicitly not in v1: a new layout algorithm. `PaneTemplateType`
(`app/src/launch_configs/launch_config.rs:95`) is already a recursive,
serializable pane tree and `PanesLayout::Template` already instantiates one, so
if a master/stack layout is wanted later it is expressible without new tree
machinery. But that is the *third* thing `main` does, not the first.

> **Built 2026-08-22 and 2026-08-23 — T8.5.** The bit, the CWD consumer, and
> then the orchestration one. The page was right that this is an *anchor* and
> not a layout feature, and right that "follow the focused pane" was the wrong
> scoping.
>
> **The ordering it proposed was wrong, though.** Layout was to be second and
> orchestration third; layout is now not built at all, deliberately. There is
> no small version of "main gets the large flex": the flex of a pane is already
> owned by the border you dragged and by the layout restored from app state,
> and a third opinion that silently overrules both is a policy argument, not a
> feature. Orchestration had no such competition — the unqualified `warpctrl`
> target was one `or_else` — and it is the consumer that makes the bit worth
> having, so it went second.
>
> Worth keeping from the "three consumers" table: **a bit with one consumer is
> easy to delete and a bit with three is not** was the right instinct, and the
> reason to stop at two is the same instinct. Nobody has asked for the layout
> half yet.

---

