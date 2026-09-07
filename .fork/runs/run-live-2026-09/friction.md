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

## Notes

**2026-09-05, #1.** Found within minutes of the first product-profile session.
The pane was probably not routed (nothing connects the server automatically,
and `session inspect` was not run), so this is the 9p column of the state table
in `.fork/docs/wsl.md`: detection walks the repo from Windows and never
finishes, the panel has no repository, and the fallback text blames WSL. The
maintainer restated the requirement the same day: WSL is a remote server, and
nothing user-facing goes through 9p. Diffs, the editor's language servers and
the clipboard are the surfaces named.
