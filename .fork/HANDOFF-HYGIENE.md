# Handoff: the boards that outlived their work

Written 2026-09-13, approved by the maintainer the same evening. **Can run
beside `HANDOFF-MERGE.md`**: Part A touches only `.fork/`. Part B touches code,
so start it only after the merge lands. Retire this file to `archive/` when
both parts are done.

## Why this is worth a session

The ranking pass that produced these handoffs read every board, handoff and
ticket and found about thirty claims marked open that commits had closed, plus
seven handoffs whose lists were empty. `CLAUDE.md` names a doc that outlived
its code as this fork's commonest defect; here it is the planning layer, which
is what the next session reads first.

**The inventory below came from three agents reading docs, and one of them was
wrong on a point checked afterwards** (it said the endpoint validator had no
direct test; `crates/ai/src/api_keys_tests.rs:301` is one). So **verify each
sha before striking a claim**: `git show --stat <sha>` and read enough of the
body to know it closes the thing named.

## Read these first

1. `CLAUDE.md`, *How to write in this file*: retract in place, leave the
   wrong claim visible, date both halves, never rewrite a run record.
2. `.fork/README.md`, the map you will be updating.

---

## Part A: docs only

### A1. Archive the finished handoffs

Move to `.fork/archive/` (a `git mv`, not a rewrite), after confirming each
one's list is empty:

| file | closed by |
|---|---|
| `HANDOFF-FOCUS.md` | `79e6ffe3f`, `2eb7804b4`, `dcdc80494`, `8ee329a29`, `bb0af075d`, `41e77d9ef`, `94d8b32bb`, `3d006bac1` |
| `HANDOFF-GHTESTS.md` | tasks 1-4; its "Windows release stale" is superseded |
| `HANDOFF-PROFILE.md` | `c25c8fbc2`; threads closed in `a7b6803c8`, `eaf389d4e`, `79b3d2deb` |
| `HANDOFF-AUTH.md` | marked spent; leftovers `2a18a2dc8`, `7a5a3028e`, `aeebfd4fa`, `242592160` |
| `HANDOFF-MOBILE.md` | steps 1-6; the device rows live on in `docs/remote-control.md` |
| `HANDOFF-SPECSFETCH.md` | `87d4d2049` |
| `HANDOFF-LOCALMODEL.md` | done 2026-09-09 |
| `HANDOFF-DECIDE.md` | all seven questions ruled (`f2e1558c6`, `eaf389d4e`, `2a18a2dc8`, `a7b6803c8`, `aeebfd4fa`; auth parked) |
| `HANDOFF-NEXT.md` | points at the retired `GOAL.md`; its items 4-6 are done (`60ac539bc`, `9b58f8eda`, voice) |

Items still open inside any of them are carried by `docs/` pages or by the
2026-09-13 handoffs; if you find one that is carried nowhere, add it to
`.fork/tickets/T15-loose-ends.md` rather than leaving it in the archive.

### A2. Correct the boards in place

Each correction struck, dated, with the sha that falsified it.

- `board.html`: every chip reads open and all seven items are done (merge
  `b175101a2`, cancel `2ff77def8`, viewer `7bb323cbc`/`9ed773dc3`, WSL
  `ea61116e1`/`099b26ea5`/`6d07c1e1a`; egress on Windows and the cloud chip
  per their run records). "cron not yet installed" is wrong (`7a5a3028e`); the
  parked T14.14 picker was built (`952d80a08`).
- `next.html`: item 2's "still live in the two PR-content call sites" (`35a29d6d6`,
  `a5e50790a`); the housekeeping strip; the read-aloud item's "unbuilt on this
  side" (`68cc117c2`, `ac5f6b25f`, `9d878a1ec`). Then decide whether its
  "done when" is met. If it is, mark the page retired at its head, the way
  `GOAL.md` was.
- `roadmap.html`: the voice row under "Declined or not open" (now built), and
  replace the "What's actually left" framing with a pointer to the 2026-09-13
  handoffs. Keep the struck history.
- `mobile.html`: item 1 says no real phone was used (`313280f3d`,
  `62ad2f2c6`); the notification row (measured 2026-09-07); the CA-installed
  row (reach run 09-08/09); the QR clipping (`STACK_BELOW_WIDTH`,
  `5a954bf7e`).
- `reach.html`: the port-22 rule for `100.64.0.0/10` is added
  (`.fork/runs/reach-2026-09-08/README.md:27`); Private DNS is fixed.
- `docs/remote-control.md`, *Unverified*: the phone-off-the-LAN run is no
  longer owed (`313280f3d`).

### A3. Ticket boxes

- `T13`: labelled ACTIVE with all boxes ticked.
- `T16` (1): the `#[cfg(windows)]` gitignore test exists (`2d4dcf69b`); the
  connect-after-`cd` hazard (`4c4c21798`).
- `T18`: `did_change_watched_files` (`2a18a2dc8`), `acp probe --cwd` (T20.5).
- `T21.5`: routed commit message and PR paths (`036bcccac`, `848427cfc`,
  `35a29d6d6`, `a5e50790a`).
- `T15`: the Firefox-on-Android row (`docs/remote-control.md:452`); world 1 in
  the event log (`210b029a1`).
- `open-questions.md:8`: Developer Mode (`aeebfd4fa`).
- `friction.md`: a24 open note (`35a29d6d6`); a10 (`serve.ps1` default is
  already `-Ctx 98304`).

### A4. The map

Update `.fork/README.md`: the handoff paragraphs (the four 2026-09-13 handoffs
live, the rest archived), the ticket table's state column for T04 (every box
ticked) and T13, and the four 2026-09-13 decisions under `decisions/`. Run
`.fork/tools/claude-md-budget.sh` if you touch `CLAUDE.md` at all.

Commit per area, e.g. `fork: archive nine finished handoffs (DOCS)`,
`fork: the boards catch up with what shipped, each line struck with its sha (DOCS)`.

---

## Part B: small code and tooling, after the merge

- **T21.2, two model-display fixes.** The settings page draws an empty bar for
  an unknown spec (`specs.rs:49-56` documents the clamp); draw `?` the way the
  inline card does. The *Full Terminal Use* tab is empty because nothing fills
  `cli_agent`; the ticket's recommendation is one line in `picker.rs`. Test
  the first; photograph both on Windows with `shot.ps1 -Process warp-oss`.
- **The Windows scratch-profile recipe** (friction a3): `manual.md` never
  mentions `WARP_DATA_PROFILE`. Say that it works in debug builds only, where
  the settings file lands (`%LOCALAPPDATA%\warp\WarpOss-<name>\config\`), and
  the ACP-agent cwd trap when the profile's pane is PowerShell. The voice
  memory file and `docs/voice.md` have a worked example.
- **A waiter** (friction a12): `.fork/tools/wait-for.sh` that waits on a pid or
  a file, with a timeout, and never on a `pgrep -f` pattern (a wrapping
  `bash -c` matches its own argv).

Commits under `(T21)` and `(DOCS)`.
