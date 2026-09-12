# The window-focus fix, driven live on Windows

2026-09-12, evening. `HANDOFF-FOCUS.md` Task 2. Windows debug builds under
`WARP_DATA_PROFILE=focusab` (release builds ignore that variable,
`crates/warp_core/src/channel/state.rs:140`, and the maintainer's own profile
was not to be touched). One window, one tab, every `WARP_FORK_*` cleared. The
driver is `focus-ab.ps1`; each `.txt` is one launch, verbatim.

| binary | sidecar |
|---|---|
| post-fix | `C:\dev\warp\target\debug\warp-oss.exe`, `v0.fork.bb0af075d` (has `619c345a6`, `03bf98fd0`, `fc2a9bb46`) |
| pre-fix | `C:\dev\prefix-debug-7d8e21b76\warp-oss.exe`, `v0.fork.7d8e21b76` (has none of the three) |

The driver's own `version:` line prints empty: it builds the sidecar path
wrongly. The versions above were read from the sidecars directly.

## Result

Criterion, from the handoff, stated before the run: post-fix `tab reset-name`
answers `ok` with a window and tab, `session inspect` resolves, and `app active`
still names **no** window.

| run | focus when asked | `tab reset-name` | `session inspect` | `app active` |
|---|---|---|---|---|
| 4, post-fix | helper window frontmost | **`ok`, window `0`, tab `1769`** | resolves | no `window_id` |
| 4, pre-fix | helper window frontmost | **`missing_target`** | resolves | no `window_id` |
| 1, pre-fix | unfocused at launch | **`missing_target`** | broker error, not retried | no `window_id` |

The fix does what it claims on the real backend, and the `app.active`
exception held. `window list` reported `is_active: false` in every valid run,
so the condition under test was in force.

**Pre-fix `session inspect` resolves, which this session predicted it would
not.** The pre-fix binary lacks `619c345a6`, but an argument-less call never
asked for an active window then: `session_inspect_target` did not yet default
the tab, so selection ran over every window, and one window with one tab has
one session. The handoff's expectation was right; the prediction was wrong. It
would fail pre-fix with two tabs (`ambiguous_target`, the `03bf98fd0` bug), not
tested here.

## Taking focus away, and the two runs that do not count

A launch from this PowerShell usually **takes foreground** (runs 2, 3 and both
run-4 launches; run 1 did not). A focused window makes the pre-fix binary pass,
which is exactly the false green the handoff warned about.

- **Run 2** was focused, and the pre-fix `tab reset-name` answered `ok`. Invalid.
- **Run 3** tried `ShowWindow(SW_MINIMIZE)` on the process's main window. It
  raised no error and `app active` still named window `0` afterwards. Whether
  the minimize did not land or winit kept reporting focus was not established.
  Invalid.
- **Run 4** starts a small WinForms window in its own `powershell.exe` when
  `app active` names a window; it takes foreground the same way Warp did, and
  is ended by pid before `window close`. That is the handoff's "click another
  application" without the user's cursor.

What this does not establish: the maintainer's original gesture, clicking the
desktop on a second monitor, was not reproduced; a second application in
front is the same `active_window() == None` condition by a different route.
Not run under WSLg, deliberately. Two-window behaviour live was not run; the
unit test `two_windows_with_no_reported_focus_refuse_as_ambiguous_on_both_paths`
covers it on the test platform.

## Two traps found on the way

**The subcommand is `tab reset-name`.** The handoff's recipe says
`tab reset_name`, which clap rejects; against the pre-fix binary an error there
reads like the bug being tested.

**The Windows credential broker races.** Runs 1 and 2 each had one call fail
with `unauthorized_local_client`: *"Unable to impersonate using a named pipe
until data has been read from that pipe. (0x80070558)"*. Both times it was
`session inspect`, immediately after `tab reset-name`; a retry a second later
succeeded (run 2). `ensure_same_user_peer` (`app/src/local_control/mod.rs:935`)
calls `ImpersonateNamedPipeClient` on a pipe, and Windows refuses that until
the server has read from it. Not fixed and not investigated past reading that
call; the order of read and impersonation at its call site is the next thing to
check. The driver retries on this error and logs each try.
