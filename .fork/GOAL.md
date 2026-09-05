# The horizon: live in it

**Set 2026-09-04, replacing the horizon set 2026-09-01** (*"the consent path,
seen on the wire"*). Delete this file when it is met or abandoned. It is a
horizon, not doctrine. **Read it first; it outranks the tickets' ordering while
it stands.**

---

## The verdict on the last horizon

**Not met, and retired rather than failed.** Its refusal criterion was met in
run 1: a `reject_once` on the wire, the request recorded verbatim, the agent
surviving it. Three of its four unknowns were answered. The fourth, compaction
cadence under ordinary work, was unmeasurable in run 2 because the transcript
wrote to a filesystem the agent could not read (T20.1, since fixed), and run 2
itself answered the eight-hours caveat the previous horizon carried: **no**.
Forty-four permission requests in fifty minutes in `default` mode, terminated
by the maintainer.

Two things then made the remaining unknown not a product question. The Windows
launcher now starts the agent in its own shipped permission mode, where Warp is
not asked and the instruments are off, because that is the build a person lives
in. And Claude Code's own session file already records every compaction as a
`compact_boundary` line with the summary beside it, so the cadence is on disk
for anyone who wants it (`.fork/docs/observability.md`).

The maintainer's words, 2026-09-04: going round in circles over nuts and bolts
around the permission gates on the ACP path. Set aside. Permission posture stays
frozen and is not measured further under this horizon.

## Destination

> **Live in the Windows build, in WSL panes, with the fork's agent in the
> panel, for a working week. The friction log is the backlog.**

Met when all three hold:

- **Seven working days**, not necessarily consecutive, launched through
  `warpdev.ps1`'s product profile (the default; `-Instrumented` is the rig and
  does not count toward this).
- **A friction log**, `.fork/runs/run-live-2026-09/friction.md`, with one dated line
  per thing that stopped a turn or annoyed enough to notice. An empty day is a
  line saying so.
- **The board landed or dropped**: each of the five items below done, or
  struck with a reason in the commit that strikes it.

## The board

Carried as a designed page, with status chips: **`.fork/board.html`**, a
self-contained static file (open it in any browser). Local-first since
2026-09-05 by the maintainer's rule: no third-party fonts, scripts or APIs, and
a claude.ai copy only when asked for. The copy published 2026-09-04 at
<https://claude.ai/code/artifact/4dae4336-8d7d-448f-b3d0-69ae93f570ca> is the
same board as of that date and is not updated.

| | item | state 2026-09-04 |
|---|---|---|
| 1 | Merge upstream (T10) | **done**, `b175101a2`; drift check in `.fork/tools/drift-check.sh`, cron line not yet installed |
| 2 | Windows launcher: product by default, rig behind `-Instrumented` | **done**, this commit |
| 3 | Measure egress on Windows, against the merged binary | **done** 2026-09-05, `.fork/runs/egress-windows-2026-09-05/`: every warp-oss socket loopback, T2.5 voice measured, T19 WebSocket gate written down |
| 4 | Composer cancel path (`.fork/docs/composer.md` item 7, the `^C` at the prompt, the missing `stop`) | **done** 2026-09-05, `2ff77def8`; the `^C` was the person's command dying, and the buffered sentence was the whole answer |
| 5 | Hide the two cloud chips in the agent footer (T19) | open |
| 6 | The viewer: one trace from the harness's record and Warp's (`.fork/docs/observability.md`) | phase 0 **done** 2026-09-05, `7bb323cbc`, `.fork/runs/viewer-phase0-2026-09-05/`; phases 1 and 2 **done** the same day: `warpctrl agent trace`, text by default, `--html FILE` for the page |
| 7 | WSL parity: connect automatically, then diffs, then language servers (`.fork/docs/wsl.md`) | **done** 2026-09-05: `ea61116e1`, `099b26ea5`, `6d07c1e1a`–`f6f59cfe8` |

## What changes about frictions

The last horizon said fix nothing mid-run, because the run was a measurement.
This one is the product. **Fix what stops you, the same day. Log what merely
annoys, and let the count rank it.** A friction that recurs three times
outranks a new idea with none.

## What would make this fail, stated so it can

- **Measuring instead of living.** An instrumented launch is for a question you
  can write in one sentence before starting it. If the sentence is not written,
  launch the product.
- **Fixing by recency.** The log's newest line is not its most important; the
  count is.
- **Counting a day the panel was not used.** A day launched through the product
  profile with no agent turn in it is not a working day for this purpose.
- **Building item 6 before phase 0.** `.fork/docs/observability.md` names one defect and one
  measurement that come before any rendering.

## Not this horizon, and why

- **Permission-path measurements, I18, `.fork/docs/classifier.md`.** Set aside by the
  maintainer. Posture frozen.
- **T14.14's model picker and I22's OpenRouter provider.** Under ACP the agent
  owns model and provider; a second picker in Warp duplicates it.
- **I1's inbox.** No friction log has asked for it.
- **The clean-build test of the `-j 8` cap.** Worth running once; not part of
  living in the build.

## Decisions on record

- **ACP stays the transport, and `local_agent` stays beside it.** Asked
  2026-09-04 whether ACP was the right call once the permission half is idle by
  default. It is, for the reason that survives the freeze: it is the only
  versioned interface a non-Claude agent offers, and the thesis names local
  models, which no Claude-only path reaches. `local_agent` is kept as the path
  with no `npx` in front of it. Neither is removed. What stops is measuring the
  permission half of ACP.

## Standing constraints

No push, no PR, no upstream merge without an explicit say-so (the 2026-09-04
merge had it). **Permission posture stays frozen.** `CARGO_BUILD_JOBS=8`, and the
cap is **unresolved, not lifted**: the 2026-09-04 measurement was a warm build in
which the app crate compiled alone for six of seven minutes at 15.1 GB, and the
many-large-crates case that took the guest down has not been run. Never build on
both sides of the VM at once. Leave no Warp or agent processes running.
