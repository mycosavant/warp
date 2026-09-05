> Idea I1, split out of `.fork/IDEAS.md` on 2026-09-05. History; read the section you came for.

# I1 — The inbox

> *"Instead of vertical tabs/panes, I want an option to render it like an inbox
> of threads. t3code being one I find nice — they also have a feature to set
> threads as 'settled', which is similar to archive, but the session isn't
> hidden completely, it goes to the bottom of the panel and can be re-activated
> any time. Very much like an email or messaging app."*

## What this is, technically

Two separable things, and they should be separated because one is nearly free
and the other is a week.

**(a) A second rendering of the left panel** that groups threads by state and
recency instead of listing tabs by position. **(b) A per-thread `settled` bit**
that moves a thread to a bottom section without deleting it.

## What already exists

More than you would guess.

`ToolPanelView` (`app/src/workspace/view/left_panel.rs:165`) already has four
modes — `ProjectExplorer`, `GlobalSearch`, `WarpDrive`, **`ConversationListView`**.
The fourth one is the inbox, unfinished:

* `app/src/workspace/view/conversation_list/` — 2,198 lines across a view, an
  item renderer, and a view-model.
* `ConversationListViewModel` (`view_model.rs`) holds a **flat, fuzzy-searched
  list of conversation ids**. No grouping, no sections, no sort control. It
  caches ids only and re-reads each row at render time — which is the right
  shape for adding sections to, because sections are a function of the row data,
  not of the cache.
* `AgentConversationEntry` (`app/src/ai/agent_conversations_model/entry.rs:74`)
  already carries everything an inbox row wants: `title`, `initial_query`,
  `created_at`, `last_updated`, `status`, `working_directory`, `artifacts`,
  `run_time`, `request_usage`.

So the data model for an inbox is **done**. What is missing is that the list is
a list.

## `settled` is a one-field change, and there is a precedent for it

`AgentConversationData` (`crates/persistence/src/model.rs:1166`) is stored as a
**single serialized-JSON column** (`agent_conversations.conversation_data`,
`app/src/persistence/agent.rs:20`). It already ends with:

```rust
/// Whether the user has pinned this child agent in the orchestration
/// pill bar. Orchestrator conversations always serialize as `false`.
#[serde(default, skip_serializing_if = "is_false")]
pub pinned: bool,
```

`settled: bool` is that field again. **No SQL migration** — the column is a
blob, `#[serde(default)]` handles old rows, and `skip_serializing_if` keeps it
out of rows that do not use it. Older builds reading a newer row ignore the
field. This is as cheap as persistence gets.

## The trap, and it is a real one

`MAX_PERSISTED_CONVERSATION_COUNT = 200` (`app/src/persistence/agent.rs:41`),
enforced on every upsert by `select_conversations_to_evict`, which drops whole
conversation *trees* oldest-first.

**An archive that evicts is not an archive.** Settling a thread is a promise
that it will be there later; the eviction policy currently makes that promise
false at conversation 201, silently, with no UI anywhere that says so. Anyone
who builds the `settled` section without touching eviction has built a feature
that quietly loses your work.

Two ways out, and the cheap one is probably right:

* **Exempt settled conversations from eviction**, and count only unsettled ones
  against the cap. One predicate in `select_conversations_to_evict`. The risk is
  unbounded growth, which is a real risk but a slow and visible one.
* **Raise the cap and make it a setting.** Simpler still, and dishonest —
  it moves the cliff rather than removing it.

I would do the first and put the number of settled threads somewhere visible.

## The smallest version that is still the idea

1. `settled: bool` on `AgentConversationData`, mirroring `pinned`.
2. Exempt settled rows from eviction.
3. `ConversationListViewModel` returns **sections** rather than a flat `Vec`:
   *Active* (has a live stream), *Recent* (by `last_updated`), *Settled*
   (bottom, collapsed by default). The row renderer in `item.rs` is unchanged.
4. Settle / unsettle from the row's context menu. No new keybinding until you
   have used it for a week and know what it should be.

Explicitly **not** in the first version: a new panel, a new pane type, a second
sidebar implementation, or anything that makes "inbox" and "vertical tabs"
different code paths. It is a sort mode on a list that already renders.

> **Built 2026-08-23 — T8.3.** All four items, and the page was right about the
> shape: `settled` really was `pinned` again with no migration, and the inbox
> really was a sort mode on a list that already renders. Better than expected
> on one point — `ConversationSection` already existed with `Active`/`Past`,
> already collapsible, so *Settled* is a third variant rather than new
> machinery.
>
> Two things this page could not have known, both found by running:
>
> **The eviction trap has a second half.** Exempting settled rows was correct
> and sufficient for the cap. But `agent_conversations` carries an AFTER UPDATE
> trigger that stamps `last_modified_at` on any write leaving it alone — so the
> obvious implementation made every settled thread look freshly used, and since
> eviction *orders* by that column, unsettling one would have let it evict
> genuinely newer work. Caught by reading the inbox, where every settled row
> said "2 min ago".
>
> **Settling has to work on threads that are not loaded**, and the `pinned`
> precedent does not: `set_conversation_pinned` warns and gives up when the
> conversation is not in memory. Fine for a pill bar full of on-screen
> children; useless for an inbox, where the rows worth settling are the ones
> nobody opened this session. Needed its own persistence path.
>
> Also added `agent settle`, which is how any of it was checked without a
> mouse. The keybinding is still deliberately absent, per item 4.

## Settled: beside, not instead of

Asked and answered 2026-08-21 — **beside**. So the inbox is the fourth
tool-panel view, which is where the code already is, and the tab bar is
untouched. That removes the only large unknown in this entry: tabs remain how
panes are addressed everywhere (`warpctrl tab list`, launch configs,
`cross_window_tab_drag`), and none of that has to move.

Worth saying out loud, because it is an argument *for* the inbox rather than a
consolation: a thread and a tab are not the same object today. A conversation
can exist with no tab open. The tab bar structurally cannot show you that; the
inbox can. Beside is not a compromise — the two are showing different things.

---

