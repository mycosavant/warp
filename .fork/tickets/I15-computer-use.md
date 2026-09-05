> Idea I15, split out of `.fork/IDEAS.md` on 2026-09-05. History; read the section you came for.

# I15 — Computer use is already here, and gated

Not from the brain dump. Found 2026-08-21 while chasing what
["instrument the build and monitor the user actions"](I10-browser.md)
would actually cost, and it turns out to cost much less than a browser because
it is written.

## What is in the tree

`crates/computer_use` — a complete screen-control stack:

* An **`Actor`** trait with `Action::{Click, Type, KeyPress, Scroll, …}`,
  `Key`, `MouseButton`, `ScrollDirection`.
* **Screenshots**, whole-screen or `ScreenshotRegion`, plus `thumbnail.rs`.
* A **`Recorder`** with `RecordingConfig`, `RecordingHandle`,
  `post_process_recording`, `finalized_video_duration`,
  `generate_video_thumbnail`. Warp can record a session to video.
* **Window enumeration** — `enumerate_windows() -> Vec<WindowInfo>` and
  `Target::Window { window_id, pid }`, so actions address one window rather
  than the screen.
* Per-platform implementations: `mac/`, `windows/`, and `linux/` with
  **both** `x11/` and `wayland/` (the latter through XDG portals).
* An **XInput2 MPX "agent seat"** on X11 (`linux/x11/seat.rs`) — a private
  master pointer/keyboard pair with its own cursor, so an agent can drive a
  window *without stealing the real cursor or focus*. That is a considered
  answer to the hardest problem in this space and somebody built it properly.
* A **manual CLI**: `cargo build -p computer_use --bin use_computer`, with
  `windows`, `click`, `text`, `keypress`, `screenshot` subcommands.

And it is already an agent tool, not just a library:
`app/src/ai/blocklist/action_model/execute/use_computer.rs`,
`request_computer_use.rs`, `start_recording.rs`.

## The gate, and it is a familiar one

```rust
// crates/warp_features/src/lib.rs — DOGFOOD_FLAGS
FeatureFlag::LocalComputerUse,
FeatureFlag::VideoRecording,
```

`DOGFOOD_FLAGS` is **the same list `WarpControlCli` was in** before T1.1 opened
it. Third time: T1 found the control plane there, T4 found local Drive sync
gated on an account, and here is computer use.

And the flag's name is the point. From
`crates/cloud_object_models/src/ai_execution_profile.rs:475`:

```rust
if is_sandboxed || FeatureFlag::LocalComputerUse.is_enabled() {
```

Without it, computer use is available only when the agent runs **sandboxed in
someone's cloud**. `LocalComputerUse` is the flag that says *run it on this
machine instead* — which is this fork's entire thesis, sitting behind a
dogfood gate.

Two gates, as with T1: the runtime flag above, and the cargo feature
`local_computer_use`, which is **not** in `app/Cargo.toml`'s default list
(unlike `agent_mode_computer_use` and `background_computer_use`, which are). So
opening it means both `fork::FORCE_ENABLED` and the feature list — T1.1 and
T1.2 again, with the same shape.

## What was verified by running, today

Against the release build under WSLg X11:

* **`use_computer windows` lists the Warp toplevel** — id, bounds. `pid`,
  `class` and `title` come back empty, which matches the known Weston
  reparenting quirk rather than being a defect in the crate.
* **Window-targeted screenshots work.** A 1400×693 PNG of the Warp window,
  fully rendered, not the black frame the Wayland path produces. **This alone
  is the whole substrate for I10's "preview a design" and for any agent that
  needs to see what it just changed.**
* **Keystrokes still do not land.** Tried four ways — window-targeted,
  screen-targeted, after a click, and after explicitly setting X input focus.
  Nothing typed appeared.

## One recorded claim corrected

`.fork/tickets/` (T5.4) says of WSLg:

> `XGetInputFocus` returns `None` and `XSetInputFocus` does not stick

**Half of that is wrong.** `XSetInputFocus` on the Warp toplevel *does* stick —
`XGetInputFocus` reports the window back immediately afterwards. And the
default it returns is not `None`; it is `0x438`, which is the **root window**.
"Focus is nowhere" is true in effect, but it is a different fact with a
different fix, and the fix that was assumed impossible turns out to work.

So the wall is narrower than recorded. Pointer motion arrives (hover states
appear under a synthetic cursor). X focus can be set and holds. What still
does not happen is activation and key delivery — which points at winit's own
focus tracking or XWayland's activation model, one layer above X focus, and
**not** at "WSLg cannot do input".

That is worth someone's afternoon, because two blocked items sit behind it:
T2.5's audio egress test and T8.1's quake-mode press. Neither needs a person
if this comes loose.

## Why this matters beyond smoke testing

Agent-assisted smoke testing was the ask, and this covers it. But the same
stack is how an agent verifies GUI work at all — which is the one thing this
fork's method has never been able to do on its own. Every GUI claim in
`.fork/tickets/` was verified by a person clicking, by SQLite, or by logs. A local,
account-free screenshot-and-input channel is the missing half of "verify by
running the thing" for anything with pixels.

Not selected yet — it wants a scope of its own and the keyboard question
answered first. But it is the strongest unselected item on this page and it
came out of a question about browsers.

## Selected, and scoped from the other end — T9.1, 2026-08-23

**Built.** See `.fork/tickets/` T9. One correction to the section above, and it
changes what the idea *is*.

**The gate this entry names is the wrong gate.** "Opening it means both
`fork::FORCE_ENABLED` and the feature list — T1.1 and T1.2 again" was written
from reading. `FeatureFlag::LocalComputerUse` gates whether **Warp's own
agent** is offered computer-use tools. This fork's agent is Claude Code
driving the `claude` binary, and it reaches the computer through its own Bash
tool — so what this fork wants is the **CLI**, and `use_computer` checks no
feature flag at all. It was built and driven against a running Warp with no
flag touched, no preference seeded and no cargo feature added.

The idea was graded as "a gated subsystem to open". It is closer to "a tool
that was already open, missing one verb". The verb was `drag`, and adding it
took no library change — the `Action` variants it needs already existed.

Everything above about the *stack* stands and was confirmed by running it:
window-targeted screenshots, the MPX agent seat, no effect on the real cursor.
The keyboard question is still open and still only matters under WSLg.

---

