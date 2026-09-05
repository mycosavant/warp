> Idea I8, split out of `.fork/IDEAS.md` on 2026-09-05. History; read the section you came for.

# I8 — The visor

> *"'Quake' visor for lead agent (also Wave Terminal, recently added feature)."*

**Selected, and it is first, because both halves already exist and have never
been introduced to each other.**

## Warp already has quake mode

`GlobalHotkeyMode` (`app/src/settings/mod.rs:280`):

```rust
pub enum GlobalHotkeyMode {
    Disabled,
    /// "Quake mode" shows a dedicated window with special properties
    QuakeMode,
    /// "Activation hotkey" shows/hides all of the normal windows
    ActivationHotkey,
}
```

`toggle_quake_mode_window` (`app/src/root_view.rs:1479`) creates a window with
`WindowStyle::Pin`, exact bounds from `quake_mode_settings`, and background blur.
There is screen-change repositioning, hidden-state handling, and a global
action. It is a finished feature.

## And it is not macOS-only — but the platform question has a real answer

The doc comment says "*thanks to it using an AppKit NSPanel*", which reads
macOS-only. It is not:

* The setting declares `supported_platforms: SupportedPlatforms::DESKTOP`
  (`app/src/terminal/keys_settings.rs:17`), and `DESKTOP` is commented in
  `crates/settings/src/lib.rs:146` as *"Mac, Linux, and Windows"*.
* `toggle_quake_mode_window` has **no `cfg` gate** at all.
* The winit backend handles the window style:
  `WindowStyle::Pin => WindowLevel::AlwaysOnTop`
  (`crates/warpui/src/windowing/winit/window.rs:660`), with a
  tiling-window-manager special case at `:1392` and `:1445`.
* Someone has clearly debugged this on Linux already. From
  `crates/warpui/src/windowing/winit/delegate/global_hotkey.rs:78`:

  > Trigger when the hotkey is released, _not_ pressed. This is due to an X11
  > quirk where focus is transferred out of Warp windows after a global hotkey
  > is pressed. **This breaks our quake mode logic.** However, focus is restored
  > when the hotkey is released.

  You do not write that comment without having had quake mode working on X11.

## The display-server question, and a wrong answer I had to retract

The global hotkey on Linux is **X11-only**:

```rust
GlobalHotKeyManager::new().expect("x11 implementation never actually fails")
```

Wayland compositors do not grant global key grabs to clients, so if Warp were a
Wayland-native client on WSLg, the hotkey could not fire. I checked the running
release build and concluded exactly that — and it was wrong, because I had
launched it plainly rather than the way this repo's own README says to.

The two runs, same binary, back to back:

| launched as | `/memfd:wayland-cursor-rs` mapped? | backend |
|---|---|---|
| `./target/release/warp-oss` | yes | Wayland |
| `env -u WAYLAND_DISPLAY LIBGL_ALWAYS_SOFTWARE=1 ./target/release/warp-oss` | **no** | X11 |

The documented WSLg invocation (`README.md`, "Running under WSL2 (WSLg)")
unsets `WAYLAND_DISPLAY` precisely so winit takes the X11 path. Under that
recipe — the one you would actually use — **Warp is an X11 client, on the same
display server the global-hotkey implementation targets.**

So the Wayland objection does not apply to the way this build is run, and the
X11 quirk the delegate's comment describes is the arrangement it was written
for. Quake mode on WSLg is plausible after all.

## It works. On Linux, under WSLg. Verified 2026-08-22

The one thing that mattered was whether the hotkey *fires*, and I could not
press it — WSLg takes synthetic clicks but not synthetic keystrokes. The user
pressed it.

**Settings → Features → Global hotkey → "Dedicated hotkey window", bound to
`ctrl-shift-Q`. It opens.** Confirmed three ways after the fact:

* `warpctrl window list` reports **two** windows where there was one.
* The X11 geometry is unmistakably quake: `(-32,-32) 1451x324` — full width,
  324px tall, anchored to the top edge, against the main window's
  `(2013,630) 1246x802`.
* A screenshot of that window id shows a complete, correctly rendered Warp
  workspace with its own tab list and its own terminal session.

So the doc comment's "thanks to it using an AppKit NSPanel" is misleading about
platform support, the `WindowStyle::Pin → WindowLevel::AlwaysOnTop` winit path
is real, and the X11 global grab works under XWayland when Warp is launched
with the `env -u WAYLAND_DISPLAY` recipe.

**This is a one-day feature, not a two-week one.**

## What is left

The quake window opens a *terminal*. The visor should open an *agent*.

`PanesLayout` (`app/src/pane_group/mod.rs:869`) already has the variant:

```rust
pub enum PanesLayout {
    SingleTerminal(Box<NewTerminalOptions>),
    Snapshot(Box<PaneNodeSnapshot>),
    Template(PaneTemplateType),
    AmbientAgent,
}
```

`toggle_quake_mode_window` (`root_view.rs:1479`) builds its window with
`add_window`; the layout choice is made there. So the work is a setting plus a
match arm — plausibly no new app surface at all, which is the `graph.rs`
standard this board is graded against.

> **Retracted 2026-08-22, on building it. `AmbientAgent` is the wrong
> variant** — it is the *Cloud Agent setup tab*
> (`initial_ambient_agent_pane` → `create_cloud_mode_terminal` →
> `enter_ambient_agent_setup`). "Ambient" is upstream's word for the **cloud**
> agent. Pointing the visor at it would have wired the hotkey to the
> account-gated path. The conclusion — "a setting, no new app surface" — was
> right; the mechanism named twice on this page was not. See T8.1 in
> `.fork/tickets/` for what it actually took.

Still worth checking on **Windows**, where `global_hotkey` uses Win32
`RegisterHotKey`, but that is now a portability check rather than the question
the feature hangs on.

## The feature, assuming it passes

The quake window opens a terminal. `warpctrl tab create --tab-type agent`
already exists and works (T1, 100 actions). The visor is: **make the quake
window's initial layout an agent tab instead of a terminal**, as a setting.

That is a `PanesLayout` choice at `add_window` time —
`PanesLayout::{SingleTerminal, Snapshot, Template, AmbientAgent}`
(`app/src/pane_group/mod.rs:869`). Note `AmbientAgent` is already a variant.
There may be nothing to build but a setting and a match arm.

> **Also retracted — see above.** What it took instead: the quake window is
> already built from `NewWorkspaceSource::Empty`, whose path
> (`configure_empty_workspace` → `add_new_session_tab_with_default_mode`)
> *already* enters agent view when the global default session mode is `Agent`.
> The only missing piece was making the visor's mode independent of that
> setting. One predicate, one call, no new layout variant. **Built 2026-08-22
> — T8.1.**

## Why this one is worth doing first

It is the endgame of T5 made reachable. The fork's thesis is your agent, on your
keys, with no account — and a lead agent you can summon over whatever you are
doing with one key is what makes that thesis a daily habit rather than a
capability. It is also the smallest item on this page, which is not a
coincidence: it is small precisely because six months of other work already
built both halves.

---

