> Idea I3, split out of `.fork/IDEAS.md` on 2026-09-05. History; read the section you came for.

# I3 — The panes are flexible and illegible

> *"Dragging tabs/blocks/panes to split/merge. Panes in a tab re-flow and are
> responsive. Dragging a tab from the tabs panel will only allow one to create a
> new window. Like browsers and Zed/VSCode, dragging a tab over another tab
> should split with that tab. With one tab active, dragging another tab into the
> pane view should split/reflow responsively. Also a right-click option to split
> tabs from the tab itself rather than the pane inside the tab. The current
> shipped tabs/panes are pretty flexible, but not that intuitive."*

**This diagnosis is exactly right, and I can now say precisely why.**

## What exists — quite a lot

`app/src/pane_group/pane/view/header/mod.rs:853` implements quadrant-based
drop splitting, with an ASCII diagram in the doc comment:

```
+--------+
|\ up   /|
| \    / |
| L \/ R |
| /    \ |
|/ down \|
+--------+
```

`calculate_pane_move_direction` normalizes the drag against the target pane's
centre and picks `Direction::{Up,Down,Left,Right}`, with a `DRAG_SPLIT_THRESHOLD`
so it is not twitchy. `PaneDragDropLocation` (`app/src/pane_group/mod.rs:885`)
already models the three destinations: `TabBar(..)`, `PaneGroup(PaneId)`,
`Other`. `PaneNode::move_pane(id, target, direction)` (`tree.rs:260`) does the
tree surgery. `cross_window_tab_drag.rs` handles dragging between windows.

So: **the split-by-direction mechanism is finished.** So is reflow, so is
cross-window detach.

## Why it feels unintuitive — two specific reasons

**1. The drag source is the pane header, not the tab.** Look at the handler:
`PaneHeaderAction::PaneHeaderDragged`. Everything above hangs off a
`PaneHeader`. The tab — the large, obvious, always-visible handle that every
other application trained you to grab — is not a source for pane-group drops.
That is your "dragging a tab over another tab should split with that tab",
and it is a missing entry point on a finished mechanism.

**2. There is no drop indicator, because the split happens *during* the drag.**
`MovePaneWithinPaneGroup` is emitted from `PaneHeaderDragged`, not from
`PaneHeaderDropped`. The layout reflows live, under the cursor, before you
commit. That is technically impressive and it is the reason it feels
unpredictable: there is no *preview* distinct from the *result*, so you cannot
tell what will happen — you can only watch what is already happening and try to
undo it by moving further.

Every application you named does the opposite: a translucent rectangle showing
where it will land, and nothing moves until you let go.

## The smallest version that is still the idea

1. **Make the tab a drag source into the pane group.** The `PaneGroup(PaneId)`
   drop path exists; give the tab the same drag payload the pane header emits.
2. **Split preview from commit.** On drag, emit a *highlight* — the target pane
   id plus the direction, drawn as an overlay rectangle over the half that will
   be taken. Move the tree on `Dropped`, not on `Dragged`. This is the change
   that makes the existing feature usable, and it is smaller than it sounds
   because `calculate_pane_move_direction` already returns exactly what the
   overlay needs to draw.
3. **Right-click on a tab → Split Left / Right / Up / Down.** The tree op and
   the direction enum both exist; this is a context-menu entry calling
   `move_pane`. Cheapest item on this page and it makes the whole capability
   discoverable without a drag at all.

Deliberately not in v1: changing the tab-out-to-new-window behaviour (it already
works via `PaneDraggedOutsideTabBarOrPaneGroup` + `DetachType`), or merge
semantics, which nobody has defined yet.

> **Built 2026-08-22 — T8.2.** All three items, and the diagnosis on this page
> held up exactly. One correction worth carrying: item 1 was blocked by a gate
> this page did not spot — the tab's `Draggable` is pinned to
> `DragAxis::HorizontalOnly` unless `FeatureFlag::DragTabsToWindows` is on, and
> that flag is `RELEASE_FLAGS` + `cfg!(any(macos, windows))`. On Linux a tab
> could not leave the tab bar at all, so "give the tab the same drag payload"
> was necessary but not sufficient. Item 2 was as small as claimed —
> `calculate_pane_move_direction` did return exactly what the overlay needed.
>
> **Amended after dragging one, 2026-08-22.** The "deliberately not in v1"
> paragraph above is wrong on its own terms: tab-out-to-new-window does **not**
> "already work". `PaneDraggedOutsideTabBarOrPaneGroup` exists, but the branch
> that acts on it is behind `DragTabsToWindows`, and that flag is **measured
> off** in this fork's builds (`FORKDBG DragTabsToWindows=false
> is_release_bundle=false`) — `RELEASE_FLAGS` only applies to a release bundle.
> So the gesture has never worked here, in either tab layout. Making it work is
> a one-line `FORCE_ENABLED` entry, not a rewrite.
>
> A first pass at this note claimed the T8.2 axis relax had *broken* the
> gesture. It had not: `tab.rs` is the horizontal tab bar and the user runs
> vertical tabs, whose `vertical_tabs.rs` was never touched. Recorded because
> the mistake is the same one this page keeps making — reasoning about a gate
> from one of its two call sites. See `.fork/tickets/` T8.2 "REVISIT SOON",
> which also collects the missing drag-cancel key and a Windows-only lag report.
>
> **Closed 2026-08-23.** The one-line `FORCE_ENABLED` entry above was right,
> and measured true in the same build that measured false the day before —
> `DragTabsToWindows=true is_release_bundle=false`. It also made the
> hand-rolled axis relax redundant, which is the shape this fork keeps finding:
> the flag was always the gate, and touching anything downstream of it was
> working around a switch rather than throwing it.
>
> The drag-cancel key was the interesting one, because **this page and the task
> board both named a seam that cannot work.** Keystrokes are matched along the
> responder chain before the element tree sees the event, innermost view first,
> so no ancestor view and no `Draggable` ever gets a look at Escape. And the
> agent-view pop that made the missing cancel look like a bug happens in
> `TerminalView` *before* the event the workspace would have handled. What was
> actually missing was smaller and lower down — nothing in the app could answer
> "is a drag happening?", because a `DraggableState` belongs to the view that
> renders it. `warpui_core`'s `drag::in_flight` is that answer.
>
> One gate this page still has not spotted: `vertical_tabs.rs:3206` pins the
> *tab group* draggable to one axis **unconditionally**, no flag. Tabs detach
> now; groups do not.

## Related, and cheap once the above lands

**Drag to re-order and resize** — dividers already drag (`dragged_border:
Option<DraggedBorder>` on `PaneGroup`); tab re-order already computes a hover
index (`calculate_tab_focus_hover_index`). Verify by running before scoping
anything; this may be a bug report rather than a feature.

---

