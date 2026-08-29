# The 24-hour goal: make the fork self-hosting

**Set 2026-08-29 for the weekend. Target was Monday morning 2026-09-01.**

## Met, 2026-08-29, about three hours in

> In **Warp's own agent panel**, in `/home/effatha/git/warp`, hold a
> **multi-turn** conversation that makes a real change to the fork, asks
> permission for it, is answered from the panel or from `warpctrl`, and whose
> **next turn remembers what it did**.

Commit **`cddacfbc7`** was written that way: three turns, seven permission
requests, every one answered from `warpctrl`, a real correction to
`.fork/README.md`, and a commit the agent composed after finding the house
style on its own. The full account is T14.7's as-built in `TASKS.md`, which is
where it now lives permanently; this file is the horizon and the horizon has
been reached.

**What it cost, in the order the work actually went:** a measurement that
overturned the plan (both cells of the two-path table were wrong and the real
blocker — the pane starting in `$HOME` — was in neither), `session/load` for
ACP, two defects found by trying to break it, and one small config file that
turned out to matter more than any of the code.

## What is left, and it is small

**There is no button.** The panel shows the question and then tells you to type
`warpctrl agent approve <id> --digest <d>`. That is answerable *from* Warp —
it is a terminal — but it is not pressing a control in the pane that is asking.
The console already has the button and reaches a phone; the panel does not.

That is the next thing, and it is smaller than anything in T14.7. After it, the
honest test of "using the fork to build the fork on an ongoing basis" is not a
feature at all — it is a second and third session driven this way, finding what
only use finds.

**Delete this file** when that button lands, or replace it with the next
horizon. It is not doctrine and it should not outlive being useful.
