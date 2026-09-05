> Idea I5, split out of `.fork/IDEAS.md` on 2026-09-05. History; read the section you came for.

# I5 — Recent files and tabs

> *"Recent files/tabs (à la ctrl+E / ctrl+O in VSCode/Zed) — [open, history]."*

Two lists: what is open now, and what was open recently. Warp has the pieces —
a command palette (`app/src/command_palette.rs`), global search
(`ToolPanelView::GlobalSearch`), `app/src/undo_close/` (so closed-tab history is
already tracked somewhere), and `ActiveFileModel` on the pane group.

The interesting question is whether this should be a new surface at all, or a
**source inside the command palette**. A palette that already exists, already
has fuzzy matching and already has a keybinding is a cheaper home than a new
modal — and it is how Zed does it too. That framing probably makes this small.

Not selected, but it is the strongest of the unselected UI items and would be a
good one to promote once I3 lands.

---

