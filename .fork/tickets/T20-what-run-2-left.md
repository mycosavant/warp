> Ticket T20, split out of `.fork/TASKS.md` on 2026-09-05. History; read the section you came for.

## T20 — What run 2 left

**Filed 2026-09-02** from `.fork/runs/run-2026-09-02/friction.md`, after the
maintainer terminated run 2 at 50 minutes. Ordered by what blocks what, not by
size. **Nothing here was fixed during or after the run** — `GOAL.md` names
fixing frictions mid-run as a failure mode, and two of these are consent surface.

### T20.1 — The transcript resolves a Unix cwd on the Windows side ✅ **done 2026-09-03**

`TranscriptLocation::resolve` (`app/src/fork.rs:545-547`) does
`session_cwd.join(".warp").join("transcripts")`, called from
`app/src/ai/transcript.rs:367` with the pane's cwd. On the Windows build with a
WSL pane that cwd is `/home/effatha/git/warp`, so the join produces a
POSIX-rooted path that Windows resolves to `C:\home\effatha\git\warp\…` — and
**creating it succeeds**, so nothing errors and nothing logs.

Measured: `C:\home\` did not exist before the run. Warp created the tree and
wrote 43,014 bytes of the user's prompts into it, plus a second file under
`C:\home\effatha\` from the earlier mis-rooted session. The repository's real
`.warp/transcripts/` received nothing.

**Two consequences, and the second is separable enough to be its own fix.**

- **The feature is inert on this platform**, which is why this blocks run 3. The
  transcript exists so an agent can grep back what compaction discarded; the
  agent lives in `/home/effatha/git/warp` and cannot see a file on `C:`. So
  unknown 4's recovery half was absent for the whole run — **run 2 could not
  have answered unknown 4 given eight hours.** Same class as run 1 measuring on
  the Linux build where T18's bug cannot occur: *a horizon measured where the
  instrument does not work is not measured.*
- **The owner-only mode is void.** `create_private_file` puts `0600` on the
  `open` precisely so the window before a chmod cannot leak the first line
  (2026-08-31). DrvFs carries no Unix mode; the file landed `-rwxrwxrwx`. **The
  fix is correct and the filesystem is not listening**, and nothing in the fork
  can notice, because the call succeeded. Worth asking whether a transcript
  should be *refused* rather than written when its destination cannot hold the
  mode — the fork's own reasoning elsewhere is that a silent weaker guarantee is
  worse than a loud refusal.

**The seam already exists and this should not invent one.** T16 built
`session_filesystem()` — `Local` / `Host(id)` / `Unreachable`
(`app/src/terminal/model/session/filesystem.rs`) — for exactly the question
"where do this session's files actually live", and `Unreachable` is a deliberate
third state for the case a caller must not treat as local. A transcript writer
asking that question is the shape of the fix; `session.windows_path_converter()`
is the other candidate and does the opposite direction today.

**Not decided here**: whether to write through `\\wsl.localhost\<distro>\…`,
write from inside the distribution, or decline and say so. The third is cheapest
and most honest and is probably wrong for a feature whose whole value is being
readable by the agent.

**And a trap for whoever takes it**: this is *not* the bug it first looks like.
The reading at the time was *"the agent isn't on the WSL side"*. The agent **is**,
and T18 is working — `session/new` was accepted (it fails outright under T18's
bug), all 228 event lines carry cwd `/home/effatha/git/warp`, and the agent's own
commands treat Windows as the far side (*"sync Windows checkout from the WSL
origin remote"*). Warp writes the transcript, from the other side of the
boundary. Two processes, two filesystems, one path string. Filed as an
agent-placement bug it would have produced a fix in the wrong file.

**As built (2026-09-03, `76fd267`/`57b2a93`/`7281de9`), verified end to end on
the Windows build.** A WSL pane at `/home/effatha/git/warp`, an agent turn, and
the transcript in the repository's own `.warp/transcripts/` — read back *by the
agent* in the next turn, which recovered the word planted in the first and named
the file it read. `C:\home` was not created. Run 3's recovery half works.

**The ticket named the wrong seam, and the right one already existed.**
`session_filesystem()` answers which *route* file operations should take and
answers `Host` for a WSL session the moment a server attaches — correct for the
file tree, wrong here, because a WSL session's files are reachable from Windows
through `\\wsl$\` either way. Routing on it would have made the transcript's fate
depend on whether anyone had run `remote wsl connect`, which is the same "order
two unrelated commands were typed in" defect T16 phase 3 removed. The fix is
`session::filesystem::native_path`, a *sibling* of `session_filesystem`, built on
upstream's `Session::maybe_convert_to_native_path` — which the completer and the
slash commands have used for this exact question all along, and which neither the
ticket nor the first hour of the fix found.

**The shape of it: two processes, two filesystems, one path string.** The
*pointer* handed to the agent stays in the session's spelling, because that is
where the agent greps. The *write* moves into this process's. Those had been one
argument, and on every platform but Windows-with-a-WSL-pane they are the same
string.

**Three things running found that reading would not.**

1. **A test I wrote was wrong, and only Windows could say so.** The identity case
   asserted a POSIX cwd comes back unchanged; on Windows it comes back `None`,
   because `PathBuf` there cannot hold `/home/…`. That is `native_path` behaving
   as documented — and my calibration contradicted my own doc, on the one
   platform the ticket is about.
2. **The pointer had the host's separators.** With the write fixed, the agent was
   handed `/home/effatha/git/warp\.warp\transcripts\….md`: `PathBuf::join`
   inserts the host's separator and the cwd belongs to the guest. The agent coped
   — it found the file and reported the right path back — which is the agent
   being tolerant, not Warp being right. Closed with `typed_path`'s own
   inference rather than a rule invented here.
3. **The second consequence's mechanism was wrong.** The `-rwxrwxrwx` was DrvFs,
   an artifact of the mis-rooted `C:\` path. Measured 2026-09-03 writing from
   Windows into the distribution and reading back from Linux: through `\\wsl$\` a
   file lands `-rw-r--r--`. The `0600` was never lost to a filesystem — it is
   never *requested*, because `create_private_file`'s mode is `#[cfg(unix)]` and
   the Windows binary does not ask. So fixing the path improved the mode as a
   side effect and still does not reach `0600`.

**Decided, on the ticket's own question:** a transcript is **not** refused when
its destination cannot hold the mode. Refusing on Windows leaves the agent
nothing to grep, which is the loss the feature exists to prevent, in exchange for
a guarantee that on a single-user machine buys little. Disclosed instead, in
`create_private_file`, whose "files inherit the ACL of their parent" sentence is
true on NTFS and does not describe ext4 behind a 9p share.

**Two numbers in this ticket to distrust.** It says 43,014 bytes landed in
`C:\home`; the run log says 41,964 there, and 43,014 is the size of the file in
the *repository*. `C:\home` no longer exists, so the original cannot be
re-measured — recorded because the figure was carried into a commit message
before this was noticed.

**Left for T20.3, found in passing**: `.fork/tools/warpdev.ps1` sets
`WARP_FORK_ACP_COMMAND` to the *unwrapped* `npx … claude-agent-acp`, which
`CLAUDE.md` records as failing outright for a WSL pane on Windows. Every
measurement above used the `wsl.exe -d Ubuntu --` form.

### T20.2 — `options_offered` renders as a menu and means a receipt ✅ **done 2026-09-03**

The approval surface lists *"Yes"*, *"Yes, and don't ask again for similar
commands"*, *"No"*. **The middle one can never be selected**, and nothing on
screen says so.

The code is right and says so plainly. `acp_approval.rs`: *"it may offer the
single-shot yes — and nothing else. The always-variants are not rendered at all,
because a button that sets a session policy would be authorising something never
shown."* `acp_permission::choose`
(`crates/warp_cli/src/local_control/acp_permission.rs:480`) refuses any option
where `changes_policy` is true. `registry.rs:207` documents `options_offered` as
kept *"as data rather than as controls"*.

So this is a **rendering** defect: an audit record of what the agent offered,
drawn where a person reads a menu. It is this fork's most-tracked defect —
a surface claiming more than the code does — moved out of a doc comment and into
the consent surface, where the thing misrepresented is what a *yes* buys.

**Posture question, and it is the maintainer's**: making the surface honest about
what it already does is arguably not a posture change — nothing becomes
permitted that was not. Recorded as a question rather than assumed, because the
freeze exists precisely so this kind of "surely this one is fine" does not
accumulate.

**Maintainer's call, taken 2026-09-03: annotate in place.** The freeze in
`GOAL.md` is on *permission posture* — what is permitted — and this changes only
what is shown. Nothing became selectable that was not.

**As built.** Live on the Windows build, a real `claude-agent-acp` edit request,
both surfaces:

```
offered   Yes, Yes, allow all edits during this session (Warp never selects this), No
          [ Yes, once ]   [ No ]
```

**The record was missing the field that decides this, which is why it was hard
to say.** `options_offered` was a `Vec<String>` — the agent sends its options
*typed* (`allow_once`, `allow_always`, `reject_once`) and Warp kept only the
names, so every surface drawing the list had the wording and no way to tell the
real options from the ones that can never be selected. It is now
`Vec<OfferedOption> { name, warp_can_select }`. The agent's wording is untouched;
the note is Warp's and says so.

**Two lines held deliberately.**

- **`warp_can_select` is outside the digest**, with `can_approve` and
  `approve_refused_because` and for the reason already written there: folding
  Warp's policy in would move a digest without the *agent* having asked anything
  different, and a parked yes would stop fitting for a reason nobody was shown.
  Hashing the name alone also keeps the bytes identical to the `Vec<String>` era,
  so a digest taken before the change still fits.
- **`acp_permission::is_selectable` is written in terms of the same two
  predicates `choose` uses**, never beside them. Two rules that agree today is
  precisely T14.6's console bug, where a listing and an answer path disagreed
  about approvability.
  `an_option_is_shown_as_selectable_exactly_when_choose_would_select_it` asserts
  the two agree over every option of both measured agents' lists.

Calibrated by making each fail: `is_selectable` returning `true` always reddens
three tests and nothing else; folding `warp_can_select` into the digest reddens
exactly the new digest test; annotating *every* option reddens the CLI render
test, which is the calibration that a blanket annotation would otherwise pass.

**Residual, not fixed and not a regression**: option names can contain commas
(*"Yes, allow all edits during this session"*), so the comma-joined list reads as
one item more than it is. That predates this and the parenthetical now helps
delimit it, but a list separator that appears inside the items is still wrong.

**Wire shape changed.** `options_offered` entries are objects, not strings. The
"same binary so no skew is possible" reasoning was **wrong** — skew happens in
this repo's own documented loop, where you rebuild while a GUI is still running.
The reason it is still safe to leave the protocol version alone is different and
better: `render_approvals` falls back to a raw JSON dump when a payload will not
parse, and bumping the version would make `discovery.rs` drop the mismatched
record, so a freshly built `warpctrl` could no longer `window close` the old GUI
— which is worse than a degraded listing. A
lenient "accept a bare string too" shim was **deliberately not** added: it would
deserialize every option to `warp_can_select: false` and annotate all of them,
which is a worse answer than a parse error.

### T20.3 — An agent can relaunch its own host into a duplicate window ✅ **done 2026-09-03**

Approval `e0c15631-…:7` (*"Enable instrumentation and launch the Windows Warp
build"*) was answered at `02:08:35.072Z`; a second `warp-oss` (PID 23088) was
created at `02:08:35`. The agent ran `ggwarpdev launch` against a Warp already
running.

Warp restores session layout, so the duplicate came up with **identical panes and
tabs** and took foreground — from the user's seat, indistinguishable from
everything having crashed and restarted. Then it compounds: two instances make
every `warpctrl` call without `--instance` answer `ambiguous_instance`, including
the agent's own, and it was parked on a request to *"distinguish the two
discovery records"* when the confusion was noticed — working back toward a cause
it had created.

Smallest fix is in `.fork/tools/warpdev.ps1:148`, which already runs
`instance list` — but *after* launching, to confirm the thing came up. Run the
same query *before*, and refuse (or prompt) when one is already alive. That
is a script change and no app surface, which is the shape this board prefers.

**Note for the ticket, not a fix**: the `Parent has crashed; continuing
execution` line at the head of `warp-oss.log.old.0` is a red herring —
`CLAUDE.md` already records that it marks the recovery sibling's log rather than
a crash. It cost time here anyway.

**As built (2026-09-03).** `.fork/tools/warpdev.ps1` runs `instance list`
*before* `Start-Process`, prints what is already up, and exits `2`. `-Force` is
the escape hatch and says what it costs. Verified by running all three branches:
nothing up → launches; one up → refuses with no second process created; `-Force`
→ two Warps, and the next refusal correctly lists both.

**The check's first cut filtered the list by pid and that was dead code.** It was
written on this repo's own instruction that *"killing the process leaves a stale
discovery record"*. It does not — `discovery.rs` prunes dead PIDs on every scan
(`is_pid_alive`, two call sites), and a `taskkill /F` here left `instance list`
empty. `CLAUDE.md` is corrected. What actually piles up instances is the
opposite case and is covered: a CLI agent in a pane blocks `window close`, the
close is refused, and the instance stays **alive**.

**Also fixed in the same file, found while verifying T20.1**: the launcher set
`WARP_FORK_ACP_COMMAND` to the *unwrapped* `npx … claude-agent-acp`, which
`CLAUDE.md` records as refusing the session outright for a WSL pane on Windows
(*"`cwd` does not exist on the machine running the agent"*). A launcher handing
out the form that fails is the one thing a launcher must not do.

**And the refusal used to print `launching INSTRUMENTED` first**, then refuse —
a surface saying something the code does not do, which is the family T20.2 is
about. The check now runs before that line.

**Not done, and it is the ticket's own note**: the `Parent has crashed` line at
the head of `warp-oss.log.old.0` remains a red herring, already recorded here.

### T20.4 — Does the composer drop the agent's prose? ✅ **answered: no — it dilutes it 9:1**

The maintainer's verdict, recorded verbatim in the run log because no instrument
caught it: tool labels (`Terminal`, `Read File`) and Warp's own permission blurbs
render; thinking and most of the agent's prose do not.

**Deliberately not diagnosed.** Whether Warp drops `agent_message_chunk`s or the
agent emits little inside a tool loop is unestablished, and guessing is the
failure this board keeps paying for. The test is to compare the panel against the
transcript for a single turn — which run 2 could not do, **because of T20.1**.
That has landed and is verified end to end, so this is startable: a WSL pane on
the Windows build now writes a readable transcript beside the panel it should be
compared against.

**Answered 2026-09-03 by measuring the same turn three ways**, which is what
T20.1 unblocked. The instrument is `acp probe` — the raw ACP stream, on the same
transport the panel uses — against Warp's own transcript, against a screenshot
of the panel.

**Prose renders. All of it.** One prompt asking the agent to narrate between
steps produced, on the wire, `agent_message_chunk` runs before the first tool
call, between the two tool calls, and after the last. Warp's transcript carried
every sentence. The panel carried every sentence, matching the transcript word
for word. A second, shorter turn was run for the one thing the first could not
show — the panel was scrolled, so the *opening* sentence was above the fold —
and with everything on one screen it reads: prompt, mode note, `[Warp]`
announcement, **"About to count the lines."**, `Terminal`, **"That is the
size."** Nothing is dropped, before or between or after.

**Thinking never arrives, and that is the agent's doing, not Warp's.**
`claude-agent-acp` emitted **zero** `agent_thought_chunk` updates in two probes —
the second explicitly asking it to ultrathink — with `reasoningOutputTokens: 0`
in the stop payload both times. There is nothing on the wire to drop.
`acp_agent/translate.rs:345` maps `AgentThoughtChunk` to `AgentReasoning`
("rendered as thinking, not as output") and would render one if it came.

**Re-run 2026-09-03 under the condition that was missing — asks — and the
maintainer's verdict reproduces exactly, by dilution rather than by dropping.**

The turn: four steps, each with *"one short sentence"* of narration demanded
before it, three of which need a file write and therefore an ask. Screenshot
taken with an ask parked. Then every ask approved and the transcript compared
against what had been on screen.

**The agent's narration exists and was never visible.** *"Creating the first file
with contents A."* is in the transcript and is nowhere in the screenshot — pushed
off the top by Warp's own asking note (~590 characters: how to approve, how to
deny, what a yes covers, what it acts on, which directory the session runs in)
plus the approval card. What fills the visible panel is Warp's prose and two
buttons.

Counted over the whole turn:

| author | characters |
|---|---|
| **Warp** — mode note, asking notes, `Answered:` notes, tool labels | **2558** |
| **the agent** — all four narration sentences plus its closing summary | **271** |

**9.4 : 1. The agent's share of its own turn is 9.6%.** At run 2's 44 asks the
arithmetic is far worse.

**So T20.4's earlier "no" was answering an easier question**, and the honest
answer is: *the composer drops nothing and shows almost none of it.* Nothing is
lost, everything is diluted, and from the seat those are indistinguishable. The
scroll hypothesis recorded here first was the wrong shape — this is not about
where the viewport sits, it is about **what is in the stream to look at**.

**That relocates the fix.** There is no dropped-message bug to hunt. The lever is
the asking note, which is four paragraphs sized for the first request of a
session and paid on every one — already named in *"Not a ticket: approval
density"* below as a separable cost, and now measured. Saying it once per
conversation and abbreviating it thereafter would return roughly 500 characters
per ask to the agent, without changing a single permission.

**Still not established**, and left as read-only: `translate.rs` buffers text and
flushes on the next non-text update, on turn end, or on the failure path, and
`take_until` drops the driver future on cancellation with nothing flushing there
— so a **cancelled** turn plausibly loses its last unflushed sentence. Run 2 was
cancelled. Not run.

**Narrowed after review, 2026-09-03: this was measured at *zero* permission
requests, and the run it disagrees with had forty-four.** The event log for both
panel turns (`6a684429`, `8136081e`) shows `permission_request: 0`. So the ask
path — two Warp-authored notes per ask plus an approval card — was never
exercised, and that is precisely the condition the maintainer was describing.
The honest close is **"no drop in the buffered path"**, not "no".

Two things reading adds, neither run:

- **A cancelled turn drops its pending prose.** `translate.rs` buffers text and
  flushes on the next non-text update, on turn end, or on the failure path;
  `mod.rs`'s `take_until` drops the driver future on cancellation and nothing
  flushes there. Run 2 was terminated by the maintainer.
- **Dilution is a better hypothesis than scroll.** Each ask puts an asking note
  (several hundred characters), an answered note and a card into the stream. At
  44 asks that is ~88 Warp-authored blocks against 42 KB of transcript — Warp's
  own text plausibly the majority of what was on screen. That reconciles the
  verdict without a scroll hypothesis, and it is a *dilution* problem with a
  different fix.

The cheap decisive test, not yet run: one turn in `default` with three or four
asks, screenshot while an ask is parked, and count on-screen characters authored
by Warp against those authored by the agent.

**So the run-2 verdict does not reproduce under the conditions tested, and that
is recorded as a difference in conditions rather than as a correction.** It was a real observation of a real
session, taken at the end of a long tool-heavy run; these are one- and two-tool
turns. The obvious reconciliation — prose is present but scrolls past when
dozens of tool calls follow it, so a glance sees tool labels — is a **hypothesis
and was not established**. What is established is that the composer has no
prose-dropping defect to fix, so anything left here is about density and scroll
position, not about a lost message.

### T20.5 — Finish `acp probe --cwd` ✅ **done 2026-09-03**

Left uncommitted by the panel agent when the run was stopped: 86 lines across
`crates/warp_cli/src/local_control/{acp.rs,acp_tests.rs,mod.rs}`, all additions,
59 of them tests. It is T19's open item — `--cwd` validates the path on the
caller's machine, so the Windows binary cannot probe a Unix cwd — and passes a
POSIX-rooted path through unverified instead of refusing it.

**Not compiled and not run.** Treat as unverified: read it before trusting it,
and calibrate the new tests by making them fail.

**As built (2026-09-03).** The WIP's idea is right — a POSIX-rooted `--cwd` the
probing process cannot resolve is the T18 case, where `--command` starts the
agent inside WSL and the Windows binary has no filesystem to check against — and
its ordering was wrong.

`is_foreign_filesystem_path` was asked *after* `is_dir()`, as a fallback for a
path that failed the check. That reads as belt-and-braces and is not: on Windows
a POSIX-rooted path resolves against the current drive, so if the matching `C:\…`
tree exists then `is_dir()` is **true**, the fallback never runs, and
`canonicalize` returns a directory on the wrong machine. Silently — the same
family as T20.1, in the fix for T20.1's own ticket. And the collision is not
hypothetical: `C:\home\effatha\git\warp` had been sitting on the disk that
morning, put there by Warp.

Asked first now, and pinned by
`a_posix_cwd_is_not_silently_resolved_to_a_matching_windows_tree`, which builds
the colliding tree on purpose. **Calibrated on Windows**, the only platform where
it can fire: with the check back below `is_dir()` it reddens and the other ten
pass. 11/11 with the fix.

The WIP's own tests are kept as written and are good — `cfg`-split so each is
honestly green on the platform it runs on rather than describing the other one.

### Moved out: the composer — see `.fork/docs/composer.md`

T20.4's answer turned out to be about the composer rather than about a bug, and
the work it implies is too large for a board entry. Filed as its own file
2026-09-03 with the 9.4:1 measurement, the architectural root (Warp's notes and
the agent's prose are the same `AgentOutput` message, so the renderer cannot tell
them apart), all three transports, and the constraints that must not break.

### Not a ticket: approval density

44 permission requests in 50 minutes across 5 prompts — one every 69 seconds,
~420 for an eight-hour day — is what ended the run, and it is **not filed as work
here**. It is permission posture, which `GOAL.md` freezes and which is the
maintainer's to decide.

**And the claim originally made here about *what* those asks were is contradicted
by the run's own event log.** It said this was "ordinary scoped work **inside**
the session directory, where no boundary was crossed and a boundary would not
have helped". Counted 2026-09-03 from
`27357def-….jsonl` (44 `permission_request` lines, classified on
`tool_input_preview`):

| what the ask touched | count |
|---|---|
| the **Windows host** — `/mnt/c/dev/warp`, `powershell.exe`, `warp-oss.exe` | **30** |
| `cargo` builds and tests | 9 |
| inside the session directory and nothing else | **7** |

By tool kind: 36 `execute`, 7 `edit`, 1 `read`. So the agent was driving the
Windows side of the machine from inside WSL, and *that is exactly the boundary
crossing which should ask*. The sentence was written from the impression of a
session spent in one repository; the log says otherwise, and it is the same
mistake this board keeps recording — a claim about a measurement, made without
running the measurement.

**Two consequences for I18.** "Yes, allow all edits during this session" would
have removed **7 of 44**, so the persistent-grant framing is aimed at the
minority of the cost. And the 30 host-touching asks are the kind an agent-side
rule can name precisely (see I18 route 4), which is where the cheap relief
actually is.

The per-request text is a second, separable cost: four paragraphs before two
controls, every sentence of it true and written for a reason, **sized for the
first request of a session and paid on every one.**
