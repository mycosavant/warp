# Friction log: living in the build

**Horizon: `.fork/GOAL.md`, set 2026-09-04.** One dated line per thing that
stopped a turn or annoyed enough to notice. An empty day is a line saying so.
The count ranks; the newest line is not the most important.

Launched through `warpdev.ps1`'s product profile unless a line says otherwise.

| date | # | what | stopped work? | where it went |
|---|---|---|---|---|
| 2026-09-05 | 1 | Opened the diff panel in a WSL pane on this repo: *"Cannot detect diffs for this folder. Diffs don't currently work in WSL."* | yes | `.fork/docs/wsl.md`; board item 7. Step 1 (connect automatically) built the same day, `ea61116e1` + `1a42ecdb8`; the diff panel itself is still unmeasured routed |

## Notes

**2026-09-05, #1.** Found within minutes of the first product-profile session.
The pane was probably not routed (nothing connects the server automatically,
and `session inspect` was not run), so this is the 9p column of the state table
in `.fork/docs/wsl.md`: detection walks the repo from Windows and never
finishes, the panel has no repository, and the fallback text blames WSL. The
maintainer restated the requirement the same day: WSL is a remote server, and
nothing user-facing goes through 9p. Diffs, the editor's language servers and
the clipboard are the surfaces named.
