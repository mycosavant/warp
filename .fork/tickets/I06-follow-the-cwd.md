> Idea I6, split out of `.fork/IDEAS.md` on 2026-09-05. History; read the section you came for.

# I6 — Follow the CWD

> *"Setting for tools pane & file viewer to follow terminal CWD."*

**Selected.** Small, and it is friction every day.

The plumbing exists: `app/src/pane_group/working_directories.rs`,
`app/src/workspace/view/startup_directory.rs`, `ActiveFileModel` on
`PaneGroup`, and `AgentConversationDisplayData.working_directory`. The terminal
already knows its CWD (it has to, for the prompt and for `cd` tracking).

The work is a setting plus a subscription: when the tracked pane's CWD changes
and the setting is on, re-root the project explorer / file viewer.

**Which pane, though — and my first answer was wrong.** I originally wrote
"follow the focused pane, or it will thrash while you glance around a split."
That is still thrash; it just needs a slower glance. From the 2026-08-21
answers:

> *"We don't want each active pane to steal the file explorer."*

Exactly right. Follow the **main pane** — see [I13](I13-main-pane-in-group.md),
which this is now coupled to. A pane you named is stable; a pane that merely has
focus is not. So I13 is a prerequisite rather than a separate feature, and
together they are still small: one `Option<PaneId>`, one setting, one
subscription.

Remaining decisions, both minor:

* **Debounce.** A `cd` inside a shell loop should not re-index a tree eighty
  times. Measure the project explorer's existing re-root cost before choosing a
  delay.
* **Default off**, at least at first. A file tree that moves on its own is
  disorienting until you have asked for it.

---

