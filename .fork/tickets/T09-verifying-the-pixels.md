> Ticket T9, split out of `.fork/TASKS.md` on 2026-09-05. History; read the section you came for.

## T9 — Verifying the pixels  ← DONE (2026-08-24)

> Every GUI claim in this file was verified by a person clicking, by SQLite, or
> by a log. That is the fork's one standing exception to *run it*, and T8 spent
> it three times: "needs a person on Windows" is written into T8.2 twice and
> into the closing report once.
>
> T9 removes the exception. It is `.fork/tickets/` I15 (computer use), scoped down to
> the half this fork actually wants.

- [x] **T9.1** A drag an agent can perform. (`.fork/tickets/` I15)
- [x] **T9.2** The same on Windows, without taking the user's mouse.
- [x] **T9.3** The ghost a cross-window drag leaves behind. It was T8.2's
      tab-to-pane drop answering a release that belonged to the cross-window
      drag; fixed and A/B'd.
- [x] **T9.4** Three things a person found in the release build: a dead zone
      that was both too big and measured from the wrong point, a cancelled drag
      that left a pane looking empty, and a modifier that was never wired.
      Two fixed, the third answered — and a fourth found underneath it.

### T9.1 — as built

**`use_computer drag`, and nothing else.** `crates/computer_use` already had
screenshots, clicks, typing, window enumeration and an XInput2 MPX agent seat.
The one gesture missing from its CLI was the one every blocked item needed. The
verb is built out of `Action::{MouseDown, MouseMove, Wait, MouseUp}` — variants
that already existed — so the library, the crate's dependencies and the app
surface are all unchanged.

#### The scope correction: the flag gates the wrong half

`.fork/tickets/` I15 says opening computer use means `fork::FORCE_ENABLED` **and**
the `local_computer_use` cargo feature, "T1.1 and T1.2 again". For the thing
this fork wants, **neither is needed**, and the entry was scoped from the
wrong end.

`FeatureFlag::LocalComputerUse` gates whether **Warp's own agent** is offered
computer-use tools (`app/src/ai/agent/api.rs:361`, one term of a four-way
`&&`). But this fork's agent is Claude Code driving the `claude` binary, and it
reaches the computer through its own Bash tool. It does not need Warp to hand
it a `use_computer` action.

`use_computer` is a separate binary that checks no feature flag at all. It was
built with `cargo build -p computer_use --bin use_computer` and driven against
a running Warp with no flag touched, no preference seeded and no cargo feature
added. The gate was never in the way of the half that matters.

Whether to open the in-app tool as well is a separate question and is not
answered here.

#### Three things the crate had already decided correctly

* **Window-target coordinates are window-local pixels**, translated to root by
  `windows::window_local_to_root`. So a drag is expressed in the coordinates a
  screenshot of that window shows you, which is the only sane arrangement and
  not the one the flag names suggest.
* **The agent seat is actor-scoped, not call-scoped** — a private XInput2
  master pointer/keyboard pair, with a comment saying "a drag that spans
  batches keeps its button held". That is what makes `--screenshot` possible:
  the drag runs as two `perform_actions` calls on one actor, the capture
  happens between them, and the button is still down for it.
* **None of it touches the real cursor.** The MPX seat has its own pointer.
  The user's mouse does not move and their focus does not change.

#### The frame that only exists mid-drag

`perform_actions` takes its screenshot at the end of a batch, so a naive drag
photographs the *result*. The result was already reachable — `warpctrl pane
list` answers it better than a PNG does. What was not reachable is the drop
preview, the floating tab ghost and the detach chip, none of which survive the
mouse-up.

So `--screenshot` writes its PNG **before** the release, and that is the whole
reason the verb is shaped as two batches instead of one.

#### What it found, immediately

The first drag it performed was supposed to split a pane. It detached a tab
into a new window instead, and **that was correct behaviour for the code as it
stood** — see the T8.2 entry above. T8.2's tab-as-drop-source work went into
`tab.rs`, the horizontal strip, and never reached `vertical_tabs.rs`, which is
what the Linux build renders. One tool, one gesture, one real gap, on the first
run.

#### Verified by running, 2026-08-23 (Linux, X11 under WSLg)

* **Detach works.** Dragging a tab out of the vertical panel and releasing:
  `begin_multi_tab_drag source_wid=0 preview_wid=1 source_tab_index=2` →
  `finalize_preview_as_new_window (CREATES NEW WINDOW)`, and `window list`
  goes from one window to two. **This is the T8.2 item that shipped compiled
  but never performed** — `DragTabsToWindows` in `fork::FORCE_ENABLED` is the
  gate, and it is now confirmed by the gesture rather than by an `eprintln!`.
* **Tab → pane split works**, after the fix above: five `DragTabOverPane` and
  one `DropTabOnPane`, one window, the pane moved into the target tab and the
  source tab closed itself.
* **The drop target is visible.** The mid-drag capture shows the translucent
  accent overlay across exactly the half the drop takes. T8.2 built that and a
  person confirmed it on the horizontal strip; this is the first time it has
  been photographed.

#### One observation that did not survive being tested

A first run left the app firing tab drags on its own for two minutes after the
CLI had exited — eight `StartTabDrag` with nothing driving the pointer, all
originating from windows the detach had created.

The obvious suspect was a button leaked by the agent seat. **It reproduced
zero times.** A controlled run — one drag, then forty seconds during which no
command was issued at all — recorded exactly one `StartTabDrag` and nothing
after it. The cause is not established; the most likely remaining explanation
is the physical mouse over the windows the detach had just put on the desktop.
Recorded because it looked exactly like a tool bug and was not one.

### T9.2 — as built

**`background_supported()` answered `false` on Windows, and its own comment
gave the reason: "The Windows input stack drives the screen / foreground
window."** That is true of `SetCursorPos` + `SendInput`, which is what the
backend was. It is not true of Windows.

`PostMessage` to one `HWND` was already the fork's answer for *clicks* and
*keys* — `click.ps1` and `keys.ps1` have used it for months, and README
already records which of the four obvious variants work. T9.2 is that same
mechanism extended to press-move-release and moved into the crate, so
`use_computer drag --pid --window-id` means the same thing on both platforms.

Three parts, and the first was blocking the other two.

**`use_computer windows` did not work on Windows**, so there was no way to
*obtain* an id to pass to `--window-id`. It answered "only supported on macOS
and Linux (X11)" on the one platform this fork's GUI is used on.

`EnumWindows`, deliberately **not** `Process::MainWindowHandle`. Warp puts
every window in one process and Windows nominates exactly one of them as
"main", so a tab torn out into a second window is invisible to that API — and
the nomination *moves*, which is worse than it being wrong, because the same
call answers a different window before and after a tear-out. That cost one
confusing run before it was noticed.

**Window-targeted input.** Mouse and keyboard as posted messages, with the
held-button state carried in every `wParam`, because a move mid-drag with an
empty `wParam` reads to the receiving application as "the button came up
somewhere I did not see". Two details earned their comments: the wheel
messages carry **screen** coordinates while every other mouse message carries
client ones, and posted `WM_CHAR` never reaches Warp's editor while posted
virtual-key messages do.

**Window-targeted screenshots, via `PrintWindow(hwnd, hdc,
PW_RENDERFULLCONTENT)`** — `shot.ps1`'s recipe, which README says has been lost
to a cleared session twice, now in the crate. `PrintWindow` needs the
`Win32_Storage_Xps` cargo feature, because the Win32 metadata files it with
the printing APIs rather than the windowing ones.

#### Verified by running, 2026-08-23 (Windows release build)

```
cursor before   (998, 480)
drag            750,16 -> 250,16, window-targeted
cursor after    (998, 480)        <- unchanged
result          the "Settings" tab moved from the sixth slot to the second
```

And `use_computer windows` lists both Warp windows plus every other toplevel
on the desktop, with bounds, class and title.

#### Two limits, both measured

* **The target window must be the foreground window.** A/B'd both ways: focus
  window 0 and drag it, the tabs reorder; focus window 1 and repeat the
  identical drag on window 0, nothing moves. A posted *click* on an inactive
  window does work — it selects the tab under it — so this is specific to
  drags. The cursor is still never touched, which is the part that matters,
  but this is not a way to drive a window nobody is looking at.
  `warpctrl window focus` supplies the activation without a mouse.
* **Modifiers are not expressible.** Posted messages do not set the thread's
  key state, so `ctrl-shift-<key>` arrives as a bare `<key>` — the same wall
  `keys.ps1` documents. `Target::Screen` keeps the `SendInput` path for the
  cases that need it.

### T9.3 — as built

**The user's report, 2026-08-23:** "a tab torn out of the strip to create a new
window, then dragged back got hung on the seam of the pane and the strip, in
the ghost state. resizing the window fixed it."

Reproduced on the first attempt with `use_computer drag`. The target window
draws two tab labels on top of each other in one slot and leaves a gap at the
next; `warpctrl tab list` still shows the tab in its own window, so nothing
merged.

#### The first explanation was wrong, and the log had already said so

An earlier version of this section — and the commit that recorded it,
`6dd5378c9` — said "the release never arrives, so the ghost stays". **It
arrives.** What the log actually shows at the moment of release is:

```
tab_drag: on_drag_while_floating -> GhostInTarget target_wid=0 insertion_index=4 caller_wid=2
dispatching typed action: WorkspaceAction::DropTabOnPane { tab_index: 0, ... }
```

`DropTabOnPane`, not `DropTab`. The release was delivered and *answered by the
wrong handler*, which is a different bug with a different fix, and the line was
sitting in the log the whole time under a heading that said the opposite.

#### The cause is T8.2's tab-to-pane drop source

The fork made a tab accept `PaneDropTargetData` so it could be dropped on a
pane. `Draggable` resolves that target by intersecting the drag rect with the
drop targets **of the window dispatching the event**, and picking the smallest.

During a single-tab cross-window drag the source window *is* the floating
preview and is repositioned under the cursor on every frame — so its own pane
is right there, intersecting its own tab's drag rect. The tab's `on_drop`
closure sees `Some(PaneDropTargetData)`, dispatches `DropTabOnPane` and returns
early, so `WorkspaceAction::DropTab` is never dispatched and
`CrossWindowTabDrag::on_drop` is never called. The drag stays live and the
ghost stays drawn.

The two paths are normally mutually exclusive by construction: a tab dragged
straight down onto a pane dispatches `DragTabOverPane` from the first frame, so
`on_tab_drag` never runs and no cross-window drag ever begins. Drag the tab
*out* first and the order inverts — which is exactly the gesture the user
performed, and exactly why it took a person to find.

#### The fix

`fork::tab_pane_drop_target_accepted(app)` — refuse pane drop targets while a
cross-window tab drag is in flight. `data` goes back to `None`, `on_drag` and
`on_drop` fall through to `DragTab`/`DropTab`, and upstream's state machine
runs as it did before any of this was added. Both strips, one predicate.

Pinned by `a_tab_in_flight_between_windows_refuses_pane_drop_targets` in
`workspace/cross_window_tab_drag_tests.rs`, which asserts against a real
`CrossWindowTabDrag` rather than the predicate's arithmetic.

#### Verified by A/B in one binary, 2026-08-23 (Windows release)

A temporary `FORKDBG_T93_OFF` env switch, so the two legs differ only in the
guard — same binary, same script, same machine:

```
off   after tear-out:  windows=2  tabs_in_window0=7
      after drag-back: windows=2  tabs_in_window0=7   NOT MERGED, ghost visible
on    after tear-out:  windows=2  tabs_in_window0=6
      after drag-back: windows=1  tabs_in_window0=7   MERGED, strip clean
```

`shots/t93_off.png` is the doubled label and the gap; `shots/t93_on.png` is the
same strip after the fix. The switch was removed before commit and the shipping
binary rebuilt.

And once the logging problem below was understood, the same gesture was run
against a session that actually logs, which shows the repair line by line:

```
tab_drag: begin_single_tab_drag source_wid=1 (source window IS preview)
tab_drag: on_drag_while_floating -> GhostInTarget target_wid=0 insertion_index=3 caller_wid=1
dispatching typed action: WorkspaceAction::DropTab          <- was DropTabOnPane
tab_drag: on_drop GhostInTarget -> DropResult::DropInto target_wid=0 insertion_index=2
tab_drag: perform_handoff branch=single_tab_source->other target_wid=0 caller_wid=1
tab_drag: execute_handoff_single_tab_to_other target_wid=0 insertion_index=2
tab_drag: finalize branch=InsertedInTarget target_wid=0 insertion_index=2 source_wid=1
tab_drag: finalize_handoff -> CloseSourceWindow transferred_tab_index=0
```

One line is the whole bug and the whole fix: `DropTab` where `DropTabOnPane`
used to be. Everything after it is upstream's state machine, which was never
broken — it was never called.

**The regression risk was the other direction, and it was checked.** The guard
must not switch off tab-to-pane dropping for ordinary drags. Driven after the
fix on both strips: Windows horizontal, a tab dragged onto a pane still splits
it (one window, the target tab goes to two panes); Linux vertical panel, the
same — five `DragTabOverPane` then `DropTabOnPane`, tab `2732` ends with two
panes.

**What was not driven is the cross-window gesture on the vertical panel.** A
cross-window drag repositions the source window under the cursor every frame,
so window-local coordinates drift and the X11 window-targeted path cannot
express it. The predicate and the render wiring are shared with the horizontal
strip and the unit test covers the decision, but nobody has performed *that*
gesture on the Linux build.

#### Two things that were true and stay true

* **Escape does not rescue a stuck cross-window drag.** T8.2's cancel excludes
  them on purpose, and pressing it changed nothing — confirmed before the fix.
  Moot now for this bug, but the exclusion is still there for any other way in.
* **Resizing did not clear it here**, though it did for the user. Their case
  and this one differ in some way still not identified.


### The crash, and what is and is not known

The user crashed the release build while stress-testing cross-window tab
drags. **The crashed process's log is gone**, so the cause is not known.

What was established:

* **Warp's "crash" here is a deliberate exit.** Reproduced one — a window
  resized to an absurd height, which clamps the surface to 65535, fails to
  configure, and trips the existing policy: `Failed to render a frame 3 times
  in a row; exiting...`. The crash-recovery sibling then takes over, which is
  the "child spawned immediately" the user saw. That sibling is spawned at
  *startup* and blocks in `WaitForSingleObject` on the parent, so its
  appearance is not evidence of anything beyond "the parent went away".
* **The crashed parent never wrote a log, and the reason is how we launch it.**
  See the correction below — an earlier version of this bullet blamed a
  best-effort rename in `on_parent_process_crash`, and that was wrong.
* **Not reproduced by dragging.** Six rounds of tear-out-and-drag-back, eight
  windows created in a burst, and twelve create/close cycles, all on the
  release build: no crash. The one lead that remains is Warp's own startup
  warning on this machine — "Newer NVIDIA drivers can crash if multiple
  windows are created if the `Vulkan / OpenGL Present Method` NVIDIA setting is
  set to `Auto` or `Prefer layered on DXGI Swapchain`" — which fires on every
  window creation here, and which the user's stress test was doing a lot of.
  Untested; it points at a driver control panel, not at this code.

**Next time it crashes, copy `warp-oss.log.old.0` before doing anything else.**

#### `Start-Process` silently turns Warp's log file off

**This is the correction, and it explains every missing log above.**

`init_internal` decides with
`use_logfile = !stdout_is_a_tty && !in_ci && !integration_test`. `warp-oss.exe`
is a console-subsystem binary, and **`Start-Process` without `-NoNewWindow`
gives it a console of its own** — so `stdout_is_a_tty` is true, `use_logfile`
is false, and the process writes no log file at all. Not a rename failure, not
a rotation bug: the parent never wrote one.

What made this hard to see is that a log *did* keep appearing. The
crash-recovery sibling is spawned by the parent rather than by a shell, so it
has no console, so it logs — to `warp-oss.log.recovery`. When the parent dies,
`on_parent_process_crash` moves that file into `warp-oss.log`. Every log this
machine produced on 2026-08-23 under `Start-Process` therefore begins:

```
2026-08-23T20:26:08Z [INFO] Parent has crashed; continuing execution.
2026-08-23T20:16:18Z [INFO] Parent has crashed; continuing execution.
2026-08-23T19:34:14Z [INFO] Parent has crashed; continuing execution.
```

…which reads like three crashes' worth of evidence and is in fact three
recovery processes' logs with the interesting half missing.

Proved by changing one flag. `Start-Process … -NoNewWindow`, same binary, same
everything else:

```
warp-oss.log        32455  6:32 PM     <- fresh, rotated the old one to .old.0
first line: 2026-08-23T22:29:02Z [INFO] Using DXC for DirectX shader compilation
```

A normal startup line rather than "Parent has crashed". **So: launch Warp with
`-NoNewWindow` whenever you might want to know what it did.** A person
double-clicking the binary is unaffected — Explorer gives it no console — which
is why this only ever bit the agent.

Consequence for the record above: the user's crash log was never written, so
there is nothing to recover. The T9.3 A/B was measured from window and tab
counts and screenshots because of this; once the launch flag was fixed, the
same gesture was re-run against a logging session and the trace is in T9.3.

### T9.4 — as built

**Three reports from a person using the Windows release, 2026-08-23.** Two were
defects in fork code; the third was a question, and answering it turned up a
fourth defect nobody had run.

#### T9.4a — the dead zone was two problems wearing one coat

**The report:** "the dead space in the center of the panes when dragging to
split is too large… for each of the panes to be split again by dragging, i have
to drag to the relative middle of each of the 4 quads."

Measured rather than guessed, with `use_computer drag --screenshot
--release-at` sweeping the drop point across a pane and photographing each
frame with the button still down. On a 544x644 pane the overlay appeared only
outside a 196x232px box — a third of the pane in each axis, exactly what
`DRAG_SPLIT_THRESHOLD = 0.18` says it should be, since the test is `max(|nx|,
|ny|) < 0.18` against the *half*-extent.

That is upstream's number and it was fine while the split happened immediately
on every drag event: you found the boundary by crossing it. T8.2 made the drop
a preview that only commits on release, and a dead zone you have to hunt for is
a different object from one you fall through — the overlay is simply absent and
nothing says why.

**The second problem is the one the arithmetic hides.** `calculate_pane_move_
direction` took the *drag rect*, and for a pane drag that rect is the 212x40
placeholder chip, not the pointer. `Draggable` keeps the grab point in the same
relative position inside the chip, so the chip's centre sits at a fixed offset
from the pointer **set by where along the header you pressed**. From the old
binary's own log, grabbing a 586px-wide header at x=200 put the reference point
46px to the *right* of the pointer; grabbing near the right end puts it ~60px
to the left. The whole quadrant map slides by up to half a chip depending on
where you happened to grab, which is unlearnable, because it changes every
time.

**The fix is both halves.** `calculate_pane_move_direction(target_pane, at:
Vector2F)` now takes the pointer, threaded from `DraggableState::
dragging_mouse_position()` in the drag closures — the same shape
`vertical_tabs.rs` already used for the group draggable. `DRAG_SPLIT_THRESHOLD`
goes 0.18 → 0.10. The zone still exists, because releasing over the middle of a
pane means "do nothing" and the overlay disappearing is how you learn that
before letting go.

**Verified by running, 2026-08-24 (Linux, X11 under WSLg).** Sweeps at 14px
resolution through the centre of a 544x644 pane, with the exact pane rect and
normalized values read out of a new `log::debug!` (`RUST_LOG=warp::pane_group::
pane::view::header=debug`):

```
vertical    Up   <= 318      none 332..448     Down >= 462     band 144px = 22%
horizontal  Left <= 244      none 258..352     Right >= 366    band 122px = 22%
before                                                         band       = 36%
```

And the part that matters more than the number — **the same sweep from two
different grab points now gives the same seven verdicts**, boundary for
boundary:

```
grab x=200   up up none none none down down
grab x=480   up up none none none down down
```

A committing drop was run afterwards to confirm the preview and the commit
still agree: pointer normalized `(0.0005, 0.264)` → Down, and the layout
became stacked.

#### T9.4b — Escape left the pane blank, and it was T8.2's dim

**The report:** "if you do hit the esc key when dragging a pane/tab, the pane
goes blank. sometimes i can hit / and get it to render, but sometimes it
appears that its just gone."

Nothing is gone. `PaneView::is_being_dragged` paints an opaque `surface_2`
overlay across the pane's contents, and it is cleared by exactly three events —
`PaneDroppedWithinPaneGroup`, `DroppedOnTabBar`, `PaneDroppedOutsideofTabBar
OrPaneGroup`. All three are *drops*. A cancelled drag reaches none of them.

This is T8.2's exposure, not upstream's. Upstream set the flag from the move
event, which only fired at the very end, so a cancel had almost nothing to
undo. T8.2 moved it to `DropPreviewChanged` — deliberately, to keep the pane
dimmed for the whole drag instead of only at the moment it committed — and in
doing so made the flag live for the entire gesture without extending the
cleanup to the gesture's other ending.

`PaneGroup::cancel_drag` could not fix it directly: its pane tree stores
`PaneId`s and its `pane_contents` are `dyn PaneContent`, so it holds no handle
on either `PaneView` or `PaneHeader`. The message travels instead on the
configuration model those two views already share, as
`PaneConfigurationEvent::DragCancelled` — reachable from
`PaneContent::pane_configuration`, and therefore no new trait method on all ten
pane types.

Pinned by `a_cancelled_drag_undims_the_pane_it_was_dragging`, which fails
against the unfixed arm and passes against the fixed one.

**Verified by driving Escape mid-drag**, which needed a new `use_computer drag
--press` (a cancel key is by definition a keystroke that arrives while the
button is still down, which no click-then-type sequence can produce). Two runs
of the same drag, differing only in `--press 0xff1b`:

```
control   dragged pane dimmed (diff 35-45 vs baseline), target pane carries the
          Down overlay (diff 32)
--press   every sampled region 0.0 against the pre-drag baseline
```

and the log says why:

```
PaneHeaderDragged ...
EditorAction::Escape
PaneGroupAction::CancelDrag
WorkspaceAction::CancelDrag
```

#### T9.4c — alt+drag does not merge tabs, and cannot

**The report:** "if i hold alt+drag a tab, i have grown accustomed to that
merging tabs… i'm not sure what this is."

It is not a matter of hitting the right spot. **No drag callback in the app
ever sees a modifier.** `Draggable` destructures `Event::LeftMouseDragged {
position, .. }` and `LeftMouseUp { position, .. }`, dropping the
`ModifiersState` both events carry. The word `modifiers` does not appear in
`draggable.rs`.

The horizontal strip *is* modifier-aware, but only at mouse-down and only for
selection: `tab.rs`'s `on_mouse_down_with_modifiers` gives shift = extend
range, cmd = toggle multi-selection, both behind `FeatureFlag::GroupedTabs`.
Alt is not among them, and none of it survives into the drag.

What merges today is the *pane* drag: drag a pane by its header onto a tab in
the strip and the middle half of that tab merges the pane into it
(`TabBarHoverIndex::OverTab` → `SwitchTabFocusAndMovePane`). Making alt+drag do
it from the tab side is a feature, not a gate — it needs modifiers recorded in
`DraggableState`, a branch in the strip's drag handler, and a merge that can
target a tab other than the active one, which `merge_tab_into_active_tab`
cannot express. Not built.

#### Found on the way: tab → pane cannot fire on the horizontal strip

T8.2 made a tab a drop source for panes, and T9.1 verified it working — **in
the vertical panel**. On the horizontal strip it cannot work at all, and the
reason is three lines apart from the feature.

`tab.rs:2154` activates a tab on **mouse-down**. `vertical_tabs.rs:3312`
activates on **click**, i.e. mouse-up. So on the strip, pressing a tab to drag
it makes it the active tab *before the drag begins* — and
`tab_can_merge_into_active_tab` opens with `index != self.active_tab_index`.
The guard is right: once the dragged tab is active, the panes on screen are its
own, and dropping it on one of them means nothing.

Observed, 2026-08-24: click tab 0, drag tab 1 onto a pane.

```
ActivateTab(0)          <- the click
ActivateTab(1)          <- the mouse-down that starts the drag
StartTabDrag
DragTabOverPane x13
DropTabOnPane
```

Thirteen drag events, a drop, **zero** calls to `calculate_pane_move_direction`
(the new debug line never appears), no preview drawn, nothing merged, tab and
pane counts unchanged.

Not fixed here, because every fix is a decision about tab activation that
belongs to the user: activate on mouse-up like the vertical panel (a real
change to how every tab click feels), or restore the previously active tab when
a press turns into a drag (which changes where a plain reorder leaves you).

#### Correction: synthetic keystrokes *do* land on X11

`CLAUDE.md` says "synthetic clicks land there; synthetic keystrokes still do
not, which is why two tasks are blocked on a person." Too strong, measured
2026-08-24 through the actor's own XInput2 seat:

* **Keymap actions land.** `--press 0xff1b` mid-drag produced
  `EditorAction::Escape` → `PaneGroupAction::CancelDrag` in the log and a
  visibly cancelled drag.
* **Text does not.** `use_computer text "echo hello-from-the-agent"` into a
  focused, accent-bordered terminal input produced nothing at all.

So cancel keys and shortcuts are drivable; typing is not. `Key::Keycode(n)` is
an X **keysym** on this backend, not a keycode — Escape is `0xff1b`.

#### Two tool additions, both small

* `use_computer drag --release-at x,y` — move somewhere inert *after* the
  screenshot and before the release, which turns a drag into a probe. A sweep
  that commits every hit has to rebuild the layout between samples, and the
  layout is where the next sample's coordinates come from.
* `use_computer drag --press <key>` — press and release a key mid-drag, before
  the screenshot.

#### An operational note

The first release build of this pass died in the linker with `ld terminated
with signal 7 [Bus error]`. The disk was **100% full**: `target/` was 134GB, of
which `target/debug/incremental` alone was 66GB. Deleting the incremental cache
(cargo regenerates it) freed it and the build went through in 5m 44s.

### Corrections to T8, from this pass

**A tab *group* cannot be dragged out to a new window, and the axis is not
why.** The T8 closing report named `vertical_tabs.rs:3206` — the group
draggable's unconditional `DragAxis::VerticalOnly` — which reads as though a
flag would open it. It would not. `WorkspaceAction::DropGroup` is
`send_telemetry` plus `ctx.notify()`, and `CrossWindowTabDrag` has no concept
of a tab group at all (its `pane_group` is the split layout *inside* a tab, a
different thing wearing a similar name). Relaxing the axis would produce a
group that leaves the panel and lands nowhere — precisely the bug T8.2 already
hit once and fixed for tabs. This is a feature, not a gate, and it is not
small.

**A cold Windows release build is 19 minutes, not an hour.** `build.ps1`'s own
comment says "a release build from cold is roughly an hour", which is why the
debug profile is its default. Measured 2026-08-23 with no `target\release`
directory present: **18m 48s**, 351,897,600 bytes. Corrected in the script.

---

