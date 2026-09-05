> Ticket T8, split out of `.fork/TASKS.md` on 2026-09-05. History; read the section you came for.

## T8 — The app you actually use  ← DONE (2026-08-23)

Phases 0–7 made the fork *correct*: no telemetry, no account, your agents on
your keys, all of it measured rather than asserted. T8 is the first phase aimed
at making it **pleasant**, and it starts from a release build that exists to be
lived in rather than tested.

The full brain dump, all fourteen ideas, with what already exists under each
and an argument for or against, is in **`.fork/tickets/`**. That file is the
holding pen; this section is only what has been selected out of it.

Selection rule, unchanged from the rest of this board: value per line of code.
The five below were picked because in every case **the mechanism already exists
and is wired to the wrong thing** — which is the same finding that has driven
T1, T4, T5 and T7, and by now should be the prior rather than the surprise.

- [x] **T8.0** Linux release build, for daily use rather than for tests.
      `cargo build --bin warp-oss --features gui,warp_control_cli --release`.
      Verified 2026-08-21 by running: `--warpctrl` dispatches out of the
      release binary, the GUI window opens, discovery registers, and
      `window list` / `tab list` / `action list` all answer. `--release` means
      `debug_assertions` is off, so `UserInput`'s redaction is live and the log
      stops carrying what you typed (see "Your development build's log contains
      what you typed" in `README.md`).

- [x] **T8.1** Quake visor for the lead agent. (`.fork/tickets/` I8)
      Quake mode is a finished, cross-platform feature nobody has pointed at an
      agent: `GlobalHotkeyMode::QuakeMode`, `toggle_quake_mode_window`
      (`root_view.rs:1479`), `WindowStyle::Pin` handled in the winit backend.
      `PanesLayout` already has an `AmbientAgent` variant.
      **The blocking question is answered: quake mode works on Linux under
      WSLg.** Bound to `ctrl-shift-Q` via Settings → Features → Global hotkey →
      "Dedicated hotkey window", it opens — confirmed by `warpctrl window list`
      going from one window to two, by X11 geometry that is unmistakably quake
      (`(-32,-32) 1451x324`, full width and top-anchored), and by a screenshot
      of that window showing a complete workspace. The X11 global grab works
      under XWayland when Warp is launched with the `env -u WAYLAND_DISPLAY`
      recipe, so the doc comment's "thanks to it using an AppKit NSPanel" is
      misleading about platform support.

      What remains is small: the quake window opens a *terminal*, and the visor
      should open an *agent*. `PanesLayout` already has an `AmbientAgent`
      variant and `toggle_quake_mode_window` picks the layout at `add_window`
      time, so this is a setting plus a match arm.

      **As built (2026-08-22).** Smaller than the plan, and the plan named the
      wrong mechanism.

      *Retraction: `PanesLayout::AmbientAgent` is not the variant to reach
      for.* It is the **Cloud Agent setup tab** —
      `initial_ambient_agent_pane` builds a cloud-mode terminal and calls
      `enter_ambient_agent_setup`. "Ambient" is upstream's word for the cloud
      agent, not for a local one, so pointing the visor at it would have wired
      the hotkey to the account-gated path this fork exists to avoid. Reading
      the enum's variant names gave a confident wrong answer; reading the arm
      they resolve to gave the right one.

      What it actually took: the quake window is built from
      `NewWorkspaceSource::Empty`, which lands in `configure_empty_workspace`
      → `add_new_session_tab_with_default_mode` — and *that* already enters
      agent view when the global default session mode is `Agent`. So the
      machinery existed; the only thing missing was making the visor's mode
      independent of the setting that governs every other new tab. One
      predicate and one call, both in `root_view.rs`:

      * `fork::quake_visor_opens_agent()` — env `WARP_FORK_QUAKE_VISOR`,
        **default on**, the only predicate in `fork.rs` that defaults that way.
      * `root_view::open_agent_visor`, called inside `add_window`'s builder
        after `RootView::new` — the one point where the window's first session
        exists and nothing has been painted, so there is no visible flash of a
        terminal becoming an agent.
      * `root_view::visor_opens_agent(ctx)` is the *effective* answer, shared
        by the behaviour and by `window.visor.status` so the two cannot drift.

      **Two things outrank fork policy, in opposite directions**, and both were
      found by running it rather than by reading:

      * *Default mode already `Agent`* → the workspace has entered agent view
        on the way here, and forcing it again starts a **second conversation in
        the same pane**. Guarded; measured as exactly one conversation, not
        two.
      * *No AI enabled* → agent view has nothing behind it, so the visor stays
        a terminal. `default_session_mode()` already collapses to `Terminal`
        in that case, which is what made this one line rather than a special
        case.

      **`opens_agent` reports the effective answer, not the policy** — the
      first version reported `fork::quake_visor_opens_agent()` directly and so
      answered `true` while opening a terminal, because AI was off. Same shape
      of bug as T8.5's `pane_contents` vs `visible_pane_ids`: two notions of
      the same question, one in the report and one in the behaviour. Fixed by
      giving both a single function.

      **Two new `warpctrl` actions, 105 → 107**: `window.visor.toggle` and
      `window.visor.status`. Not decoration — synthetic keystrokes reach no
      X11 client under WSLg (see "it is not Warp, and X11 is exhausted"), so
      without a second entry point this feature could not be exercised here at
      all. `toggle` dispatches the same global action the shortcut does, so it
      works with nothing bound.

      *`toggle` deliberately does not report post-call state*, unlike
      `pane.main.*`. `ModelContext::dispatch_global_action` **queues an
      effect** that runs after the request returns, so any state read beside it
      is the state from before the toggle. Reporting it would be a confident
      wrong answer; `status` is a separate call for that reason.

      **Nothing was needed for the command palette.** Upstream already
      registers "Show Dedicated Hotkey Window" and its hide twin as
      `FixedBinding`s, gated on `QUAKE_MODE_ENABLED_CONTEXT_FLAG`. The gate,
      again, was a setting.

      Verified 2026-08-22 across four fresh launches on WSLg, each checked two
      independent ways — the X11 window title and `warpctrl agent list`. Full
      table under "The visor: a drop-down agent on a hotkey" in `README.md`.
      `state` reads `pending_open` rather than `open` from a script, because
      Warp is not the focused app; that is documented upstream behaviour and
      not a failure.

      **Left undone**: the visor is a *window*, and T8.5's main pane is
      per-tab, so "point the visor at the lead agent" is still only half true —
      it opens an agent, but not one that knows about another window's
      designated pane. That is cross-window agent context and a separate idea.

- [x] **T8.2** Tab → pane drag, with a drop target you can see. (I3)
      Quadrant split-on-drop is *implemented*
      (`pane_group/pane/view/header/mod.rs:853`, with an ASCII diagram) and the
      tree surgery exists (`tree.rs:260`). Two things make it feel unintuitive,
      and both are precise: the drag source is the **pane header** rather than
      the tab, and the split is emitted from `PaneHeaderDragged` rather than
      `PaneHeaderDropped`, so the layout reflows live under the cursor and
      there is no preview distinct from the result. Split preview from commit,
      add the tab as a drag source, and add right-click → Split.

      **As built (2026-08-22).** All three items, plus a control-plane verb so
      the operation can be checked without a mouse. One of the three is
      built-but-unverified and is called out below.

      **1. Split preview from commit — done and seen.** `MovePaneWithinPaneGroup`
      was emitted from `PaneHeaderDragged`; it now fires only from
      `PaneHeaderDropped`. On drag the header records a `PaneDropPreview`
      (target pane + direction) and the pane group draws it as a translucent
      accent overlay across the half the drop will take.

      The overlay needs **no measurement**: a `Flex` with two equal `Expanded`
      children lands exactly on the split the drop produces. That matters
      because `render` is handed an `&AppContext` and `element_position_by_id`
      lives on `ViewContext` — the pixel maths stays in the drag handler, where
      a `ViewContext` exists. `drop_preview` threads through
      `PaneTree::render → PaneNode::render → PaneBranch::render` exactly the
      way `hidden_panes` already did.

      *The dead zone became a real state.* `calculate_pane_move_direction`
      returns `None` within 18% of a pane's centre. That used to mean "emit
      nothing and leave the last move standing", so it was invisible; now it
      clears the preview, so the overlay disappears and releasing there moves
      nothing. Pinned by a table test.

      *The header owns the pending direction*, because `PaneHeaderDropped`
      carries a target but no position — so the direction has to be remembered
      from the last drag event. That memory **is** the preview, which is why
      the two ended up as one field.

      **2. Right-click a tab → "Move into active tab, left/right/above/below"
      — done and run.** Four flat entries rather than a submenu; a submenu
      needs the sidecar machinery "Move to group" uses and four short labels
      cost less than that buys. The section vanishes entirely when the move
      would be ambiguous: a tab cannot merge into itself, and a tab holding
      more than one pane has no single "this tab" to move —
      `remove_pane_for_move` takes one pane, and silently moving the first of
      several is a worse answer than not offering it.

      **`tab.merge` (107 → 108 actions)** is the same operation for scripts,
      and the reason any of this could be checked at all. Verified live: pane
      `3527` moved out of tab `3139` into tab `1818` at index 2 and took focus,
      **and the source tab closed itself** — `remove_pane_for_move` emits
      `Event::Exited` when it takes a group's last pane, the same path a pane
      dragged to the tab bar takes. Both refusals fire with the reason in the
      message.

      **3. The tab as a drag source — built, NOT verified by running.** The
      gate was **an axis**, and it is a compile-time one: upstream pins each
      tab's `Draggable` to `DragAxis::HorizontalOnly` unless
      `FeatureFlag::DragTabsToWindows` is on, and that flag sits in
      `RELEASE_FLAGS` under `cfg!(any(target_os = "macos", target_os =
      "windows"))`. On Linux a tab physically cannot leave the tab bar.

      `fork::tab_pane_drag_enabled()` relaxes **only the axis**, not the flag —
      the same flag gates cross-window tab detach at four other sites, which
      spawns a ghost window nobody has exercised here. Opening the axis is all
      a tab-to-pane drag needs.

      The rest is additive and provably so: upstream sets **no**
      `with_accepted_by_drop_target_fn` on the tab at all, so the `data`
      argument to its `on_drag`/`on_drop` was unconditionally `None` and the
      tab-bar path resolves its drop from cursor geometry. Accepting
      `PaneDropTargetData` makes `data` `Some` for panes and changes nothing
      else.

      **This one needed a person** — a drag is press-move-release, and
      synthetic input does not reach X11 clients under WSLg, so it shipped
      compiled but not performed. **Performed by hand on Windows 2026-08-22;
      it works.** What that run found is below, and it is enough to reopen the
      item.

      > **It no longer needs a person, and the sentence above is why T9
      > exists.** `use_computer drag` (T9.1) performs the gesture, and driving
      > it on 2026-08-23 found that the tab-as-drop-source work reached only
      > `tab.rs` — the horizontal strip — and not `vertical_tabs.rs`, which is
      > what the Linux build renders. Fixed in `93895f796`; both halves of
      > this item are now confirmed by a performed gesture rather than by a
      > compiler.

      **Left undone:** merge semantics for a multi-pane tab, and changing the
      tab-out-to-new-window behaviour. Both were out of scope in `.fork/tickets/` I3
      — and the second one no longer can be, for the reason in the next
      section.

      #### REVISIT SOON — what a mouse found, 2026-08-22

      The gesture works. Four things about it do not, and one of them is a
      consequence of the change itself. Nothing here is fixed yet; this section
      is the brief for the next pass.

      **1. Tab-out-to-new-window does not work — and T8.2 did not break it.**

      > **Retraction, same day.** The first version of this section said the
      > axis relax made the gesture "possible and inert", and called that a
      > regression T8.2 introduced. It was written from reading, it is wrong for
      > the layout the user actually runs, and it is wrong in the commit that
      > recorded it (`f22cc9e3c`). Two facts kill it. **The user runs vertical
      > tabs exclusively** — `vertical_tabs` is in `app/Cargo.toml`'s `default`
      > list, and the sidebar is in every screenshot taken this session. And
      > **`vertical_tabs.rs` was never touched**: `tab.rs` is the *horizontal*
      > tab bar, and no commit in T8 modifies the vertical path. Its axis lock
      > at `vertical_tabs.rs:2554` still reads `DragTabsToWindows` alone,
      > exactly as upstream wrote it.

      **Measured, not read.** A temporary `eprintln!` in `init_feature_flags`,
      built and run:

      ```
      FORKDBG DragTabsToWindows=false is_release_bundle=false
      ```

      The flag has never been on in a build made here, for **two independent
      reasons**. `RELEASE_FLAGS` is only extended when
      `ChannelState::is_release_bundle()`, which is
      `cfg!(feature = "release_bundle")` (`channel/state.rs:84`); and the
      app-side entry at `features.rs:112` sits behind
      `#[cfg(feature = "drag_tabs_to_windows")]`. Neither cargo feature is in
      `default`, and `oss.rs` sets `Channel::Oss` with only `DEBUG_FLAGS`. The
      installed stable Warp *is* a release bundle, which is why the behaviour
      exists there and not here — the comparison being made is fork-build
      against stock, not before-T8.2 against after.

      So the correct statement is the boring one: **tab-out-to-new-window has
      never worked in this fork's builds, in either tab layout, and still does
      not.** A gap against stock, not a regression. Expected behaviour, per the
      user: release outside the window creates a new window at the release point
      carrying the dragged tab/group.

      **The fix is still one line, but it is an enablement rather than a
      repair.** `fork::FORCE_ENABLED` sets a *user preference*, and `is_enabled`
      resolves override → user preference → channel state, so it outranks both
      cfgs — the I16 precedent exactly. Adding `DragTabsToWindows` there lights
      up the detach (`workspace/view.rs:28693`), the ghost chip
      (`view.rs:27806`), the focus call (`pane_group/mod.rs:3303`) **and both
      axis locks at once** — which is the part that matters, because
      `vertical_tabs.rs:2554` is already flag-gated, so forcing the flag fixes
      the layout the user runs without touching that file. It also makes the
      `tab.rs:2280` axis relax redundant, so that should come back out.
      **Unverified**: it exercises `cross_window_tab_drag.rs` (~1,800 lines)
      that nothing in this fork has ever run. Do not assume; run it.

      Note that `FORCE_ENABLED` is a code const applied by
      `apply_feature_preferences`, so this is a rebuild — there is no env var
      that flips it, and `WARP_FORK_POLICY=0` would turn it *off* along with
      everything else fork policy does.

      **What survives from the original claim**, scoped correctly: in the
      *horizontal* tab bar T8.2 did relax the axis without opening the detach,
      so there a tab can be pulled out of the strip and land nowhere. Real, but
      in a layout nobody here uses — and forcing the flag on removes it too.

      **2. Nothing cancels a drag, and Esc is not the exception — it only looks
      like one.** Reported symptom: mid-drag of an *agent* pane, Esc appears to
      cancel, and the pane comes back as an empty terminal. It is not a cancel.
      Esc pops agent view, which is upstream behaviour with upstream tests
      (`escape_pops_nested_cloud_agent_view_with_long_running_command`,
      `escape_does_not_exit_root_cloud_agent_view_...`,
      `escape_does_not_exit_local_agent_view_...`, all in
      `terminal/view_tests.rs`). The path is `input.rs:9454`
      `ctx.emit(Event::Escape)` → `terminal_pane.rs:907` →
      `pane_group::Event::Escape` → `workspace/view.rs:16144`. The drag is not
      consulted anywhere along it, so Esc falls *through* the drag and does its
      normal job to the pane underneath. The conversation is popped, not
      destroyed — **read, not run: confirm it is still in `agent list` before
      repeating this claim.**

      The seam for a real cancel is that same arm, `workspace/view.rs:16144`.
      The workspace owns the drop preview, `DraggableState::cancel_drag` already
      exists (`draggable.rs:89`, and its only callers today are
      `cross_window_tab_drag.rs:1456` and `:1777`), and the arm already runs a
      priority chain — resource center, then feature intro. A first branch that
      clears the preview, cancels the drag and consumes the event buys both
      halves at once: the cancel key that is missing, **and** the suppression of
      the agent-view pop that made its absence look like a bug.

      **3. A modifier key for drags is wanted, and would decide item 1 cleanly.**
      The user is open to gating the perpendicular pull behind a modifier rather
      than making it free. That also resolves the tension in item 1 without
      choosing between "reorder within the strip" and "tear out to a window" —
      the modifier picks.

      **4. Header-drag lag on the Windows build — still unmeasured, and the
      first explanation offered was not good enough.** Reading the path a second
      time only strengthened the wrong half of the argument: both
      `set_drop_preview` implementations (`pane_group/mod.rs`,
      `header/mod.rs:~1017`) return early when the value is unchanged, the
      overlay wrapper is a `.filter()` that wraps at most one pane, and the
      `PaneGroup` arm *replaced* per-drag-event tree surgery with a field write.
      Over a pane, the new path is strictly cheaper than the old one.

      Which means the cause is somewhere this reasoning does not reach, and
      "should be cheaper" is not a measurement.

      **The build was a debug build** — confirmed by the user, who was watching
      its output in a terminal at the time. That is now the leading explanation
      and it costs one `--release` build to confirm or kill; T8.0 exists
      precisely because a debug binary is a different animal. Note the terminal
      output is *not* itself the cost: nothing on the drag path logs per event
      (`draggable.rs` has no logging at all, and the only `log::trace!` under
      `elements/gui/` is in `new_scrollable`). It is the unoptimised layout and
      render, not the writes.

      Next after that, if release still drags badly: **the path that did not
      change.** A pane header dragged *toward the sidebar* is
      `PaneDragDropLocation::TabBar`, which emits `Event::DraggedOverTabBar`
      unconditionally on every drag event and recomputes a hover index each
      time — upstream, untouched. `PaneDragDropLocation::Other` likewise emits
      `PaneDraggedOutsideTabBarOrPaneGroup` every event. Only after those: the
      ghost's composite and the `Stack`+`Flex` overlay rebuild.

      `WARP_FORK_POLICY=0` A/Bs fork behaviour without a rebuild, but it does
      **not** isolate T8.2 — it turns off every fork predicate at once.

      **And there is a blind spot to fix first.** None of this can be measured
      today, because the only frame-cost instrumentation upstream ships is
      `LogExpensiveFramesInSentry` — and fork policy force-disables it
      (`fork.rs:46`) along with the rest of the telemetry flags. The reasoning
      was right (it reports to Sentry) but the consequence was not intended:
      **the fork has blinded itself to its own frame performance**, and the
      first time that mattered is the first time somebody said a gesture felt
      slow.

      The established fork answer to exactly this shape of problem is a local
      replacement rather than a re-enablement — `LocalTranscriber` for voice,
      the local agent for the transport.

      **Built the same day: `WARP_FORK_FRAME_LOG`.** `crates/warpui/src/frame_log.rs`
      holds the accounting and no policy; `fork::slow_frame_threshold` holds
      the policy and no accounting; the hook is four lines in
      `redraw_window`, timing the scene-build-plus-render closure rather than
      the whole function. `on` means 33ms (two frames at 60Hz), a bare number
      is a threshold in ms, and it is off by default.

      Two decisions worth keeping. **It summarises once per second rather than
      logging per frame** — a line per slow frame is its own performance
      problem during exactly the stutter it is describing, and would change
      what it measures. And **an unparseable value takes the default rather
      than switching off**, the opposite call from `WARP_FORK_QUAKE_VISOR`,
      because here the default is *off* and a typo answered with silence is
      indistinguishable from a broken feature.

      Verified by running, three ways: with `on`, five summary lines and a
      `worst 246.2ms` on the WSLg debug build; with the variable unset, zero
      lines; with `WARP_FORK_POLICY=0` and the variable set, zero lines.

      **And then it answered item 4.** Same workload (eight `tab create`s,
      WSLg, software GL), same threshold of 1ms so every rendered frame is
      counted, one build against the other:

      | build | frames/sec | mean frame | worst |
      |---|---|---|---|
      | debug | ~27 | 13.9–20.5ms | 44.9ms |
      | release | ~54 | 6.1–7.9ms | 15.0ms |

      **A debug build cannot hold a 16.7ms frame budget and a release build
      sits well inside it** — 2.4× cheaper per frame, twice the frame rate.
      That is sufficient to explain a drag that feels laggy without any
      contribution from the drop preview, and it retires the suspicion of the
      overlay unless a `--release` build still stutters.

      Absolute numbers here are pessimistic for both (software GL under WSLg);
      the ratio is the finding, not the milliseconds.

      **Answered: it was the vertical panel.** The open question this section
      first carried — horizontal strip or vertical sidebar — is closed. The
      user has never used the horizontal layout, which is what turned item 1
      from a regression report into a retraction. Worth keeping as a habit:
      **`vertical_tabs.rs` and `tab.rs` are two separate implementations of
      "the tab strip"**, and a change to one says nothing about the other. The
      X/Y labels in the report are transposed relative to the code, which is
      why everything above says "along the strip" and "out of the strip".

      #### Answered 2026-08-23 — all four, and one retraction

      **1. Tab-out-to-new-window: enabled, by forcing the flag.**
      `FeatureFlag::DragTabsToWindows` joins `fork::FORCE_ENABLED`, and the
      hand-rolled axis relax in `tab.rs` is deleted as redundant — forcing the
      flag opens `tab.rs`'s axis lock, `vertical_tabs.rs:2554`'s, and the
      detach they both feed, from one line. `fork::tab_pane_drag_enabled` now
      owns only the half that has no upstream flag: the drop-target acceptance
      that lets a tab land on a pane at all.

      **Measured, both ways, in the same build:**

      ```
      FORKDBG DragTabsToWindows=true  is_release_bundle=false   # fork policy on
      FORKDBG DragTabsToWindows=false is_release_bundle=false   # WARP_FORK_POLICY=0
      ```

      That is the exact inverse of 2026-08-22's measurement, in a build that is
      still not a release bundle — which is the I16 claim about user
      preferences outranking `cfg` demonstrated rather than asserted.

      Two technique findings from taking that measurement, both worth keeping:

      * **`--warpctrl` runs `init_feature_flags` too.** So a flag can be A/B'd
        with `warpctrl instance list` in a process that opens no window and
        binds no port — which is the way around the `WARP_FORK_POLICY=0`
        shutdown trap this file warns about, rather than the way into it.
      * **`FeatureFlag::is_enabled` panics before `mark_initialized()`**, so a
        probe placed between `apply_feature_preferences` and that call takes
        the process down. Put it after.

      **And one thing this does not fix**, found while reading the axis locks:
      `vertical_tabs.rs:3206` pins the *tab group* draggable to
      `DragAxis::VerticalOnly` **unconditionally** — no flag. So dragging a
      whole group out of the sidebar to a new window still cannot happen, and
      the flag has nothing to say about it. The report said "tab/group"; this
      answers the tab.

      **2. Escape cancels a drag — and the seam named above was wrong.**

      > **Retraction.** This section said a first branch in
      > `workspace/view.rs`'s `pane_group::Event::Escape` arm would buy both
      > the cancel and the suppression of the agent-view pop. It buys neither.
      > The pop happens in `TerminalView`'s `InputEvent::Escape` handler and
      > *then* emits `Event::Escape`, so the workspace arm runs after the thing
      > it was supposed to prevent. And keystrokes never reach that arm as
      > keystrokes at all: `app.rs:3538` matches bindings along the responder
      > chain **before** the element tree is offered the event, and
      > `dispatch_keystroke` walks that chain innermost-first, so the focused
      > editor claims Escape and an ancestor view never sees it. Written from
      > reading; wrong on both counts.

      What was actually missing is smaller and further down: **nothing in the
      app can answer "is a drag happening?"** A `DraggableState` belongs to the
      view that renders the element, so the code that sees the keystroke is
      nowhere near the code that owns the drag. `warpui_core`'s new
      `elements::gui::drag::in_flight` is that answer — a process-global
      register of drags that have started and not finished, with `any_in_flight`
      and `cancel_all`. Entries are pruned lazily by asking each one whether it
      is still dragging, so a missed deregistration cannot leak a permanent
      "yes".

      The cancel is then four lines at the top of `TerminalInput::editor_escape`
      — the first handler, before any branch — and two typed actions,
      `PaneGroupAction::CancelDrag` and `WorkspaceAction::CancelDrag`, that tell
      the two views which accumulate state on the way to drop it: the half-pane
      overlay, a pane hidden in anticipation of a tab-bar move, the hover index,
      `is_tab_being_dragged`, and the per-tab `detached` flag.

      Three decisions inside it:

      * **Cross-window tab drag is excluded.** It owns a second window, a ghost
        and a handoff protocol, and already calls `cancel_drag` along its own
        paths; stopping its `DraggableState` from outside would abandon the
        rest. The guard uses `has_singleton_model` first, because
        `CrossWindowTabDrag` is *not registered in the app's own test harness*
        and `as_ref` panics rather than answering — on a line that now runs on
        every Escape.
      * **A cancel stops a drag; it does not rewind one.** For a pane header,
        which previews and commits on drop, nothing had happened, so it is a
        true cancel. The tab strip reorders live as you drag — upstream — so
        there the movement so far stands. Pretending otherwise would need an
        undo stack for a gesture.
      * **The header clears its preview on drag *start*.** Without a drop it
        never clears, and `set_drop_preview` is silent when unchanged — so a
        leftover from a cancelled drag would swallow the first identical
        preview of the next one and the overlay would simply not appear.

      Verified through the real handler, not a mock:
      `a_drag_in_flight_takes_the_escape_key` opens a history menu, registers a
      drag, presses Escape and asserts the menu is *untouched* and the drag
      gone — then presses it again with no drag and asserts the menu closes.
      The second half matters as much as the first. Five more tests cover the
      register.

      **3. A modifier key for drags: decided against, for now.**
      The tension it was meant to resolve is gone. With the flag forced on,
      one gesture has three outcomes and geometry already picks between them —
      along the strip reorders, over a pane splits, outside the window
      detaches. Those are disjoint regions of the screen, so a modifier would
      not be disambiguating anything; it would add a key to the two cases that
      work today in order to reach the third. Revisit if the detach fires by
      accident in use, which is the evidence that would change the answer.

      **4. Header-drag lag: answered as far as this machine can answer it.**
      The frame log settled the mechanism — debug ~27fps against release ~54,
      2.4× the cost per frame — and that is sufficient to explain the report
      without the drop preview contributing anything. What was left was
      confirmation on the machine that saw it: a `--release` build on Windows,
      dragged by hand.

      > **Confirmed 2026-08-23, and it was the build.** Release, driven by
      > hand: "much snappier… nice and smooth." The next suspect — the path
      > that did not change, `PaneDragDropLocation::TabBar` recomputing a hover
      > index on every drag event — does not need investigating. The same
      > session confirmed the drop preview itself reads as intended; its colour
      > is the theme accent, and it is T8.2's, not upstream's.

      **Not verified by running, and cannot be here:** the gestures. A drag is
      press-move-release and synthetic input still reaches no X11 client under
      WSLg. The flag is measured, the cancel is tested through the handler that
      would run, and the drag itself has been performed by a person exactly
      once — on Windows, before either change.

      #### Three side findings from the same session

      **`is_busy` was true for a conversation nobody had asked anything.**
      `AIConversation` is constructed `InProgress` (`conversation.rs:420`) and
      only leaves that state when a turn *finishes*, so a freshly opened agent
      tab reports `in_progress` indefinitely — measured still true at t+60s,
      in the visor and in a plain `tab create --type agent` alike. `graph.rs`
      is unaffected (`turn_is_finished` also requires
      `last_exchange_is_complete == Some(true)`, which a never-prompted thread
      never has), but **T8.3's inbox would have put empty threads in the wrong
      bucket**. `agent.list`'s `is_busy` now also requires a non-empty
      conversation; `status` still reports upstream's word.

      **`window.list` now says which window is the hotkey window.**
      `is_hotkey_window`, one field, so "close the window I opened" stops being
      a guess that needs a join against `window.visor.status`.

      **Hide-on-blur for the visor: tried on Linux, put back.**
      `QUAKE_WINDOW_AUTOHIDE_SUPPORTED` is `cfg!(any(macos, windows))` and the
      implementation (`update_quake_mode_state`) is entirely
      platform-independent, so the constant looks over-cautious. It is not.
      Measured on X11/winit: a visor opened by `warpctrl window visor toggle`
      never becomes the key window, so the very next focus event sees a
      different active window and hides it — `Map State: IsUnMapped` within
      four seconds of opening, with no focus change from me. The autohide
      works; what does not survive it is the only entry point this platform
      has. Reverted, with the measurement recorded in the constant's doc
      comment so the next person does not repeat it.

- [x] **T8.3** The thread inbox, and `settled`. (I1)
      `ToolPanelView::ConversationListView` already exists with 2,198 lines
      behind it, and `AgentConversationEntry` already carries every field an
      inbox row wants. `settled: bool` mirrors the existing
      `AgentConversationData.pinned` — a JSON blob column, so **no migration**.
      **The trap:** `MAX_PERSISTED_CONVERSATION_COUNT = 200` with tree-wise
      eviction (`persistence/agent.rs:41`). An archive that evicts is not an
      archive; exempt settled rows or the feature silently loses work at
      conversation 201.

      **Sequencing changed 2026-08-23, by measurement (see `.fork/tickets/` I17).**
      A collector was pointed at a real local-agent turn to find out what an
      inbox row could show. Two things came back. `is_busy` was already known
      to be wrong for empty threads and is fixed. The new one:
      **there is no trajectory to show.** The persisted record holds a tool's
      *name* and nothing else — `translate.rs:139` keeps `name` from Claude's
      `tool_use` and drops `id` and `input` at parse time, and `tool_result` is
      not handled at all. Tool results in `agent read --tools` come from the
      live surface's action model, so they do not survive the pane.

      **Amended twice the same day; this is the version that was run.**
      **No capture work blocks this, for either kind of session.** Claude
      writes a complete JSONL transcript — every tool call with its full
      input, and an `Edit` call's `{old_string, new_string}` *is* the diff —
      and Warp can already name the file two different ways:

      - a *CLI-agent* session (Claude in a pane, with the
        `warp@claude-code-warp` plugin upstream already installs) reports a
        **`transcript_path`** on every OSC 777 event;
      - a *local-agent* conversation stores Claude's session id as
        **`server_conversation_token`**, because `translate.rs:401` puts
        `session_id` into `StreamInit.conversation_id`. Verified: token
        `90115094-…` ↔ `90115094-….jsonl`, holding the `Bash` call whose Warp
        record was the single string `` `Bash` ``.

      So T8.3 needs a *reader*, not a pipeline. The row must degrade when the
      transcript is missing — cleaned up, or a conversation predating the
      token — to whatever Warp's own record holds. `translate.rs` and
      `event/v1.rs` both still drop fields and are both still worth fixing, but
      neither is on the path here. Nothing else about the plan above is
      affected.

      **As built (2026-08-23).** The plan held: `settled` really was one field,
      and the inbox really was a sort mode on a list that already renders.
      `ConversationSection` already existed with `Active` and `Past`, already
      collapsible, already header-rendered — so the inbox is a **third
      variant**, not a second implementation. Four things were bigger than the
      plan said, and one of them was a real defect.

      **1. `settled` on `AgentConversationData`, mirroring `pinned`.** No
      migration, as claimed — verified by reading the column back: settled rows
      carry `"settled":true` and every other row has **no `settled` key at
      all**, because `skip_serializing_if` keeps it out. Old rows parse as
      unsettled and are pinned by a test.

      **2. The trap is closed.** `select_conversations_to_evict` exempts any
      tree containing a settled conversation, and does not count it against
      the cap. Exemption is **tree-wise because eviction is** — trees are
      dropped whole, so a per-row check would take a settled child along with
      its parent. Four tests, the sharpest being a settled row that is *by far*
      the oldest, so every ordering rule votes to drop it.

      **3. A thread can be settled without being loaded, and that is the normal
      case.** `set_conversation_pinned` warns and gives up when a conversation
      is not in memory — fine for a pill bar, useless for an inbox, where the
      rows most worth settling are the ones nobody opened this session. So
      `ModelEvent::UpdateAgentConversationSettled` patches the one field
      without a task snapshot, and both loaded and unloaded threads take the
      same write.

      **4. `agent.settle` (108 → 109 actions)**, which is how any of this was
      checked without a mouse. Verified live: settle a live thread, restart,
      read it back settled; settle a thread that was never opened; `--undo`
      removes the key entirely; settling twice answers `changed: false` rather
      than erroring.

      #### The defect, found by looking rather than reading

      Every settled row in the inbox said **"2 min ago"** — which was when I
      settled them. `agent_conversations` has an AFTER UPDATE trigger,
      `update_last_modified_at_for_agent_conversations`, that stamps
      `CURRENT_TIMESTAMP` on any write leaving the column alone. Writing the
      old value in the same statement does not help: the trigger's guard is
      `NEW.last_modified_at IS OLD.last_modified_at`, which that satisfies.

      **Not cosmetic.** Eviction orders trees by `last_modified_at`, so a
      bumped row outranks genuinely newer conversations — harmless while it
      stays settled and exempt, and **a way to evict live work the moment it is
      unsettled**. Tidying up would have deleted things.

      Fixed by letting the trigger fire and then putting the timestamp back:
      the restoring update differs from the bumped value, so the guard fails
      and it does not re-fire. Pinned by a test in both directions. The same
      finding is why settling no longer routes a loaded conversation through
      `write_updated_conversation_state` — that is a full upsert and would let
      the trigger stamp the row, so settling an *open* thread bumped it while
      settling a closed one did not. Same action, same write, same timestamps.

      #### A second thing reading would not have caught

      Settling from `warpctrl` changed the database and **the inbox did not
      move**. `AgentConversationsModel` explicitly ignores
      `UpdatedConversationMetadata`, so the emit went nowhere. Widening that
      would rebuild the list on every title and token-count update, so
      settling emits its own `ConversationSettledChanged` and only that is
      translated into a rebuild.

      **Verified by running:** SETTLED renders at the bottom, collapsed, with
      PAST correctly emptied as threads moved out; temporarily starting it
      expanded confirmed the rows are *in* the section rather than lost, and
      the temporary change was reverted.

      **Left unverified:** the row context-menu item ("Settle thread" / "Bring
      back to inbox") is built and compiles but has not been clicked —
      synthetic input still reaches no X11 client under WSLg. `agent.settle`
      exercises the same `set_conversation_settled` underneath it.

      **Left undone deliberately:** no keybinding, per `.fork/tickets/` I1 — "not
      until you have used it for a week and know what it should be." And the
      transcript reader from I17 is still a separate piece of work; this ships
      the sections and the bit, not the trajectory.

- [x] **T8.4** Pin what a tool claims to be. (I11)
      Hash each MCP tool's `(name, description, input_schema)` at connect,
      store it, and say so when a digest changes under an existing name. This
      is the tool rug-pull defence: a server can rewrite the prompt the model
      reads, after you approved it, and nothing currently notices. `sha2` is
      already a workspace dependency — **no new dep**, and no blake3. Warn in
      v1; do not auto-block until the noise level is known.

      As built. One new file, `app/src/ai/mcp/tool_digest.rs`, plus a
      twelve-line call site. No new dependency, no schema, no catalog action.

      **Three findings changed the design, all from reading the code the plan
      assumed:**

      1. **Connect is not one of several checkpoints; it is the only one.**
         Nothing in this client handles `notifications/tools/list_changed`, and
         `crates/mcp/src/runtime.rs:324` calls `tools/list` exactly once per
         spawn. The tool list is a snapshot taken at connect and never
         refreshed. So a mid-session rewrite is not detected — and is also not
         *acted on*, because the client keeps using the snapshot it already
         has. That is a much better answer than it first looks, and it is only
         true as long as nobody adds `list_changed` handling without adding a
         digest check beside it.
      2. **The installation id is not an identity.** `parsing.rs:322` and
         `:363` mint a fresh `Uuid::new_v4()` for every file-based server on
         every parse, so a `.mcp.json` server has a different installation id
         at each launch. Keying the store on it would have made every connect a
         first connect — a feature that runs, writes a file, and never once
         reports anything. Confirmed by running: three consecutive launches of
         the same server gave `a74fd341…`, `2592fda1…`, `d42814b5…`. Keyed on
         the server **name** instead, which is the key in the config file.
      3. **`(name, description, input_schema)` is not the whole claim.** It
         also hashes `title`, `output_schema` and `annotations`, each
         separately so a change is attributed rather than merely detected.
         `annotations.readOnlyHint` earned its place on its own: it is a claim
         that a tool is safe, and flipping it is a rug-pull that touches no
         prompt text at all.

      Trust on first use: the first connect of a server records what it
      advertises and says nothing, because there is no prior approval to
      compare against. After that, a redefinition is a `[warn]` in the server's
      MCP log — with the definition it advertises *now*, pretty-printed, since
      the store keeps hashes and the old text is gone — plus one toast however
      many tools changed. A new or removed tool is `[info]` only. Then the
      record is updated, so a change is reported once rather than at every
      launch.

      **Verified by running**, with `script/mcp_probe_server.py` — a
      dependency-free stdio MCP server that re-reads its own tool definition
      from a file at every `tools/list`, so changing that file and reconnecting
      *is* the attack. Eight launches against a scratch `HOME`:

      - first connect → store written, nothing said;
      - description and schema rewritten → `[warn]`, both fields named,
        the new definition in the log;
      - relaunched unchanged → silent;
      - a second tool added → `[info]` in the MCP log, nothing in the app log.

      **Not verified:** the toast. `import`/`xwd` cannot capture a root window
      under WSLg — there is no composited root to read — so it is built,
      compiles, and uses the same `add_ephemeral_toast` call as the MCP PATH
      error beside it, but nobody has seen it.

      **Left undone deliberately:** no auto-block, per the plan. And no
      `warpctrl` action to read or reset the store: the file is JSON in
      `~/.local/state/warp-oss/fork/`, and a verb to reset an approval record
      is a verb an attacker would like.

- [x] **T8.5** A main pane, and the CWD following it. (I13 + I6)
      One `Option<PaneId>` on `PaneGroup`, `None` meaning today's behaviour.
      Then consumers one at a time: CWD follow first, layout second,
      orchestration third. **This supersedes the "follow the focused pane"
      scoping**, which was wrong: a pane that merely has focus makes the file
      tree thrash every time you glance at a split. A pane you *named* is
      stable by construction. `main` is also the natural answer to a question
      T6.6 and T7.1 both leave implicit — which pane is the lead agent.

      Built and verified:

      - [x] `PaneGroup::main_pane` / `set_main_pane`, `Event::MainPaneChanged`,
            and `PaneGroupAction::ToggleMainPane` behind a command-palette
            entry with no default keystroke.
      - [x] First consumer: `cwd_anchor_session_view` replaces
            `active_session_view` at the one call site that decides a tab's
            repository (`workspace/view.rs`,
            `refresh_working_directories_for_pane_group`).
      - [x] `pane.main.get`, `.set` and `.clear` (catalog 102 → 105), so the
            effect is drivable and observable without a screenshot.
      - [x] **Run it**, two panes in different repos with focus deliberately on
            the *other* one. Designating pane 0 moved the anchor:
            `old_focused_repo=…/NeuralAudio new_focused_repo=…/warp`.

      **The overflow menu was dropped from scope, deliberately.** Its action
      type is generic per pane type (`P::PaneHeaderOverflowMenuAction`, built
      per child view by `pane_header_overflow_menu_items`), so one shared entry
      means touching every pane type. The palette entry is the intermediate
      surface, exactly as for T8.6 — real UI when more than one thing needs it.

      Two findings from running it:

      - **`pane_contents` is not "the panes that exist".** It outlives a close
        so a pane can be restored, so a pane gone from `pane list` is still in
        it. Validating the designation against `pane_contents` reported a
        closed pane as still main; `visible_pane_ids` is the right notion.
        Pinned by
        `test_main_pane_designation_does_not_survive_closing_that_pane`.
      - **The code review panel will not visibly follow it**, because its repo
        dropdown is a sticky per-pane-group selection that survives close and
        reopen. Pre-existing and not about the main pane: measured in the same
        session, it does not follow *focus* either. The anchor underneath does
        move — the toolbar diff badge tracks it.

      **Second consumer, 2026-08-23 — orchestration.** An unqualified
      `warpctrl` target now means the main pane when the group has one, and the
      active session only when it does not. One `or_else` in
      `local_control::resolver::input_target_pane_id`.

      This is the consumer `main` was promoted for. T6.6 and T7.1 both built
      agent fan-out and both left the same question implicit — which pane is
      the one you are talking to — and the answer was "whichever has focus",
      which is fine for a person with a mouse and wrong for a script: a graph
      that runs for twenty minutes addressed a pane that moved every time
      somebody clicked. A pane you named does not move.

      **Deliberately not scoped to agent actions.** Making `agent prompt`
      follow `main` while `input submit` followed focus would put two panes in
      play for one script, which is worse than either rule on its own.

      **Verified by running**, focus and main deliberately on different panes,
      each shell carrying a different `WHICH_PANE` so the answer is decisive:

      | main | focus | unqualified `input submit` ran in |
      |---|---|---|
      | none | pane 0 | pane 0 |
      | pane 1 | pane 0 | **pane 1** |
      | none | pane 0 | pane 0 |

      Pinned by `test_an_unqualified_control_target_is_the_main_pane_when_there_is_one`,
      which is also why `local_control::resolver` is now `pub(crate)`.

      **The layout consumer is deliberately not built.** `.fork/tickets/` I13 already
      says a new layout algorithm is out of scope for v1, and nothing found
      since changes that: the only honest version of "main gets the large flex"
      is a policy about `PaneFlex` that competes with the flex the user set by
      dragging a border, and with the one restored from app state. There is no
      small version that is still the idea, only a small version that fights
      two existing sources of truth. `PaneTemplateType` is already a
      serializable pane tree, so a master/stack layout stays expressible later
      without new machinery. Ordering therefore went CWD → orchestration, not
      the CWD → layout → orchestration this entry proposed.

      Still open: making the code review panel honour the anchor, if that turns
      out to be wanted. Its repo dropdown is a sticky per-pane-group selection
      that does not follow focus either, so it is a pre-existing choice rather
      than a gap in `main`.

- [x] **T8.6** WSL as a remote target, the way Zed does it. (I16)
      Promoted off the idea board because it is mostly built. Zed treats WSL as
      a remote host — Windows client, headless server inside the distro — so
      files, language servers and terminals live on the fast side of 9p while
      themes, rendering and keymaps never leave Windows.

      Built and verified:

      - [x] `WslTransport`, all seven `RemoteTransport` methods
            (`app/src/remote_server/wsl_transport.rs`) plus its command layer
            (`crates/remote_server/src/wsl.rs`). **A credential-free
            `Initialize` handshake has completed over `wsl.exe`** — daemon
            spawned, stdio bridged, `InitializeResponse` with a real host id.
      - [x] The account question, settled by running it: nothing on the path
            needs one. `handle_initialize` stores the bearer token and replies
            without validating it, the only credential check in the daemon is
            scoped to remote codebase indexing, and the proto documents
            `user_id` as "Empty when not logged in".
      - [x] The gate: `RELEASE_FLAGS` behind `cfg!(feature = "release_bundle")`,
            so the whole stack is compiled into every self-built binary and
            switched off. Opened via `fork::FORCE_ENABLED`, which outranks the
            `#[cfg(not(windows))]` on the same flag — see the note under "Look
            for the gate first" in `CLAUDE.md`.
      - [x] `remote.wsl.list` and `remote.wsl.connect` (catalog 100 → 102), a
            command-palette entry sharing one `start_wsl_remote_server` helper
            with them, and the pane → session resolver that `connect` needed.
      - [x] Server binary and install path: staged locally at the bare OSS
            path, so `check_binary` short-circuits the CDN fetch this fork
            deny-lists. **The absent install prompt is the success signal.**

      - [x] **Run on Windows, end to end** (2026-08-22). A Windows
            `warp-oss.exe` client, `warpctrl tab create`, then
            `remote wsl connect --tab <id>` → `{"distro": "Ubuntu",
            "distro_from_pane": true}`. Twenty seconds later, inside Ubuntu:
            proxy, `remote-server-daemon` and its `terminal-server` child, all
            sharing identity key `2dea4f26…`, with a state directory of that
            name freshly created. No SSH, no account, no install prompt.

      **The warpify step turned out to be unnecessary.** The runbook said to
      type `wsl` into a pane and accept the subshell prompt. That works, but
      setting *Default shell for new sessions* to the distribution is simpler
      and makes every new tab a WSL session: `SessionInfo::wsl_name()` falls
      back to `ShellLaunchData::WSL { distro }`, so launch data alone satisfies
      `connect`. Confirmed by `"distro_from_pane": true` on a tab created with
      `warpctrl tab create` and nothing else. README step 4 now leads with it.

      Two traps worth carrying forward, both found by running it:

      - **A daemon being present proves nothing on its own.** They outlive the
        GUI that spawned them, so a stale one is indistinguishable from a fresh
        success in `pgrep` output. Check `ps -o etimes`.
      - **The proxy and daemon report different paths for the same binary** —
        `~/.warp-dev/remote-server/warp-oss` is a symlink, and the daemon is
        spawned via `current_exe()`, which resolves it.

      Remaining, and now the only part left: **the ambient path.** `wsl` is
      *already* a warpify subshell command on Windows, so a WSL session gets
      warpified exactly as an `ssh` one does; what it does not get is a remote
      server, because that attach is keyed on `IsSSHWrapperSession::Yes`, whose
      payload is a ControlMaster socket path a WSL session cannot have. A WSL
      arm beside the SSH one is the work, and `Session::wsl_name()` already
      carries the distribution — as the default-shell finding above confirms,
      including for sessions that were never warpified at all.

### Deliberately not selected, and why

Recorded here because the arguments matter more than the verdicts; the full
version of each is in `.fork/tickets/`.

* **Context pruning / relevance scoring** (I9) is the most interesting idea on
  the list and the one most likely to be built wrong. T5.2 already established
  that the client holds the entire transcript and re-sends it every turn
  through one function, so this is a *filter*, not a port of
  `vscode-prompt-tsx`. The caching answer is concrete: pruning invalidates the
  cache from the first pruned message onward, so **prune rarely and in large
  chunks** — sixty small prunes cost sixty uncached turns for the same tokens
  removed. But nobody here has measured what a long conversation actually
  contains, and pruning the wrong thing does not throw, it silently degrades.
  **First task is an inspector, not a pruner.**

* **Integrated browser** (I10) is argued against on this fork's own terms.
  There is no web engine anywhere in the tree, and adding one introduces a
  second network stack outside `crates/http_client/src/egress.rs` — which is
  the thing the "nothing escapes" measurement rests on. The claim would become
  conditional the day a webview lands. The `monitor` third of the idea already
  exists as `network_log_pane.rs`.

* **Per-pane zoom / font size** (I4) needs a count before a scope: the running
  build reports exactly one `appearance.text.font_size` and one
  `appearance.window.zoom_level` for the whole app.

* **Computer use** (I15) is the strongest unselected item and was not in the
  brain dump at all — it turned up while costing the browser question.
  `crates/computer_use` is a complete screenshot / input / window-enumeration /
  video-recording stack with mac, windows, X11 and Wayland implementations, an
  XInput2 MPX "agent seat" that drives a window without stealing the cursor,
  agent tools (`use_computer.rs`, `request_computer_use.rs`,
  `start_recording.rs`) and a manual CLI. `FeatureFlag::LocalComputerUse` is in
  **`DOGFOOD_FLAGS`** — the same list `WarpControlCli` was in before T1.1 — and
  its meaning is exactly this fork's thesis: without it, computer use runs only
  when the agent is sandboxed in someone's cloud. Two gates again, runtime flag
  and cargo feature, the T1.1/T1.2 shape. Wants its own scope.

* **Composer** (I2) and **`view-as`** (I7) are waiting on specifics — the first
  from a week of use, the second until the gripe resurfaces.

### The WSLg input wall is narrower than T5.4 recorded

Tested 2026-08-21 against the release build on X11, using
`cargo build -p computer_use --bin use_computer`. Two findings, one of which
corrects the record.

**Window-targeted screenshots work.** A 1400×693 PNG of the Warp toplevel,
fully rendered — not the black frame the Wayland path gives. `use_computer
windows` finds the toplevel and its bounds; `pid`, `class` and `title` come
back empty, which is the known Weston reparenting quirk rather than a defect.

**Keystrokes still do not land** — tried window-targeted, screen-targeted,
after a click, and after explicitly setting X input focus.

But T5.4 says:

> `XGetInputFocus` returns `None` and `XSetInputFocus` does not stick

and **half of that is wrong**. `XSetInputFocus` on the Warp toplevel *does*
stick; `XGetInputFocus` reports the window back immediately after. And its
default return is not `None`, it is `0x438` — the **root window**. "Focus is
nowhere" is true in effect, but it is a different fact, and the operation
assumed impossible works.

So the wall is: pointer motion arrives (hover states appear under a synthetic
cursor), X focus can be set and holds, and **activation and key delivery still
do not happen** — which points one layer above X focus, at winit's own focus
tracking or XWayland's activation model, rather than at "WSLg cannot do input".

Worth an afternoon, because **two blocked items sit behind it**: T2.5's audio
egress test and T8.1's quake-mode press. Neither needs a person if this comes
loose.

#### Follow-up 2026-08-22: it is not Warp, and X11 is exhausted

The afternoon above was spent, with a control that removes Warp from the
picture entirely — `xev`, a trivial X11 client that prints every event it
receives.

Setup: `xev` running and logging, `XSetInputFocus` pointed at its inner window,
focus confirmed two ways (`XGetInputFocus` reads it back, **and `xev` itself
logs `FocusIn`** — so events demonstrably flow to this client).

Then both synthetic-input mechanisms X11 offers:

| mechanism | server accepted it | `xev` saw it |
|---|---|---|
| **XTEST** (`xtest_fake_input`, what `computer_use` uses) | yes | **no** |
| **XSendEvent** (synthetic event straight to the window queue) | `rc=1` | **no** |

**So synthetic keystrokes reach no X11 client at all under WSLg** — not Warp,
not a 200-line event printer. This was never a winit or Warp problem; it is
XWayland/Weston, one layer below every application.

Two consequences:

* **`xdotool` cannot help**, and neither can `wmctrl`. `xdotool key` is XTEST
  and `xdotool key --window` is XSendEvent — precisely the two rows above. Do
  not install them expecting this to move.
* **X11 is exhausted as an approach.** What remains is either below the display
  server (`ydotool`, which writes to `/dev/uinput` as a kernel-level input
  device, so Weston would see it as real hardware) or beside it (drive the
  **Windows** build with `C:\dev\keys.ps1`, which already works — see
  "Driving the Windows build from WSL" in `README.md`).

The narrower claim from the section above still stands and is worth keeping:
`XSetInputFocus` does stick, and `XGetInputFocus` returns the root window
rather than `None`. Both of T5.4's stated facts are wrong; its conclusion was
right for a reason nobody had identified.

#### Decided: stop trying to fix WSLg input

`ydotool` was installed and does not close the gap either. Ubuntu 24.04's
package ships `/usr/bin/ydotool` and **no `ydotoold`**, and `/dev/uinput` is
`crw------- root root`, so it aborts with `failed to open uinput device`.
Reaching it would mean building the daemon from source and granting uinput
access — for a mechanism that still might not be picked up by Weston.

#### First, a correction: three input paths, and only one is broken

An earlier version of this section said Zed's imperative removes "hotkeys,
cursor, input synthesis and rendering" as problems, which reads as though
keyboard shortcuts are broken under WSLg. **They are not**, and conflating
these three cost a wrong claim:

| path | what it is | under WSLg |
|---|---|---|
| **App keybindings** | a person presses `ctrl-shift-c`; the compositor routes it to the focused window | **works** — always has |
| **Global hotkeys** | a system-wide grab that fires when Warp is *not* focused (`GlobalHotKeyManager`, X11-only) | **untested** — see T8.1 |
| **Synthetic injection** | an agent fabricating key events via XTEST or XSendEvent | **broken**, per the measurements above |

Everything in this file about "keystrokes not working" means the third row.
T5.4 was about driving the GUI *from an agent*, and that framing should be
preserved whenever it is quoted.

The second row is the interesting one and nobody here has ever tested it,
because testing it requires a person to press the key — the exact thing the
third row rules out. Settings → Features → Global hotkey exposes it
(`Disabled` by default, then `Dedicated hotkey window` = quake, or
`Show/hide all windows`).

#### The decision itself

**Borrow Zed's imperative** — do not run the GUI inside the distro. This is not
because shortcuts break; it is because file I/O crosses 9p per file, and
because agent-driven verification of anything with pixels needs synthetic input
that WSLg will not deliver. This fork has confirmed both halves of the second
point: synthetic keys reach nothing under WSLg, and the Windows build's
`C:\dev\keys.ps1` already posts keystrokes to a window without stealing focus.

Consequences, all of them simplifications:

* **T2.5** (audio egress) moves to Windows. The rig is already written down.
* **T8.1** (quake mode) moves to Windows, where `RegisterHotKey` has the fewest
  ways to fail — which was already the recommendation.
* **I16 stops being one feature among fifteen.** It is the architectural answer
  to the whole category: run the client on Windows, keep the code in WSL, and
  the input problem is not solved so much as never encountered.

**Synthetic clicks still work**, and remain useful — two quit-confirmation
dialogs were dismissed with `use_computer click` during the T8 remote-server
run, which is how those sessions were closed without killing the process.

---

