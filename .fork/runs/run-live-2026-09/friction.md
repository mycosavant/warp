# Friction log: living in the build

**Horizon: `.fork/GOAL.md`, set 2026-09-04.** One dated line per thing that
stopped a turn or annoyed enough to notice. An empty day is a line saying so.
The count ranks; the newest line is not the most important.

Launched through `warpdev.ps1`'s product profile unless a line says otherwise.

| date | # | what | stopped work? | where it went |
|---|---|---|---|---|
| 2026-09-05 | 1 | Opened the diff panel in a WSL pane on this repo: *"Cannot detect diffs for this folder. Diffs don't currently work in WSL."* | yes | `.fork/docs/wsl.md`; board item 7. Step 1 (connect automatically) built the same day, `ea61116e1` + `1a42ecdb8`; step 2 the same evening, `099b26ea5`: routed, the panel works unchanged; unrouted, the text names the 9p read |

## The agent's log

What the agent working `.fork/next.html` lacked, one line per session, in
the same shape. Started 2026-09-07 as I19's first step, done as a habit.

| date | # | what | stopped work? | where it went |
|---|---|---|---|---|
| 2026-09-07 | a1 | Named `opencode acp` for a launch from the Windows build and every turn failed at `initialize` with *Incoming transport closed*; the panel said nothing about why. The login shell had no nvm on its PATH | yes | the absolute path; `.fork/docs/manual.md`, "Naming an agent that lives under nvm". Open: the panel could show the agent's stderr on a transport that closes at `initialize` |
| 2026-09-07 | a2 | Declared a loopback Custom Inference endpoint the way the manual said and the four small features answered *"no Custom Inference endpoints are configured"*; the form refused the URL too. Three stale points from upstream's 2026-08-26 merge, one under the other | yes, an hour | `7529749a8`, `26c376090`; `T03`, `CLAUDE.md`'s fourteenth. Open: a test that goes through upstream's door, so the next merge cannot do this silently |
| 2026-09-07 | a3 | Two release-build launches on a scratch profile ran on the real profile: `WARP_DATA_PROFILE` is debug-only and a relocated `LOCALAPPDATA` moves the discovery record and nothing else | yes, two builds | the debug binary, as the cancel run had already found; `run.sh` says why. Open: the manual's scratch-profile recipe is Linux-only |
| 2026-09-07 | a4 | The commit dialog in a routed WSL pane stayed blank with the local model configured: the daemon generates the message in a process that has no fork config | no | `next.html`, the runtime item; `wsl.md` diff-panel row. Fix is a protocol addition (daemon returns the diff, GUI generates) |
| 2026-09-07 | a5 | `keys.ps1 -Key Plus -Ctrl -Shift` typed `=` into the input; posted modifier key-downs are not modifier state | no | `warpctrl surface code-review open`, or the footer chip; the manual's script table |

## Notes

**2026-09-05, #1.** Found within minutes of the first product-profile session.
The pane was probably not routed (nothing connects the server automatically,
and `session inspect` was not run), so this is the 9p column of the state table
in `.fork/docs/wsl.md`: detection walks the repo from Windows and never
finishes, the panel has no repository, and the fallback text blames WSL. The
maintainer restated the requirement the same day: WSL is a remote server, and
nothing user-facing goes through 9p. Diffs, the editor's language servers and
the clipboard are the surfaces named.
