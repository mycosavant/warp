# Handoff: the composer overhaul

**Paste this whole file as the first message of a new session.** It is written to
be self-contained: a fresh agent should be able to start from it without reading
this conversation.

---

## What you are doing

Making this fork's agent composer top-of-class. It is currently failing that, and
the failure is measured rather than felt.

**Read `.fork/COMPOSER.md` first — it is the ticket.** Everything below is
context for working on it, not a substitute for it.

## Read these, in this order

1. **`CLAUDE.md`** — the fork's working method. Long; navigate by heading. The
   part that matters most for this task is the opening section on *running* the
   thing rather than reading it, and the repeated finding that this codebase's
   commonest defect is a doc that outlived its code.
2. **`.fork/COMPOSER.md`** — the ticket. Measurement, architectural root, all
   three transports, constraints.
3. **`.fork/GOAL.md`** — the standing horizon. It outranks board ordering while
   it exists, and it **freezes permission posture**, which bounds this work.
4. **`.fork/TASKS.md`**, section `## T20` — the run that produced the
   measurement. Read T20.4's as-built entry; it records the wrong answer being
   given first and why.

## The one-paragraph version

During a turn with approvals, Warp's own chrome outweighs the agent's words
**9.4 : 1** — 2558 characters to 271 in a measured four-step turn. The agent's
narration never reached the screen at all; it is in the transcript and was pushed
off the top by Warp's asking note plus the approval card. Nothing is dropped.
Almost nothing is shown. The root cause is that `Translator::note`
(`app/src/ai/acp_agent/translate.rs:864`) emits Warp's own words as
`Message::AgentOutput`, the same message type as the agent's prose — so the
renderer literally cannot tell them apart.

## Start here, in this order

1. **Study the reference apps and write the comparison into `.fork/COMPOSER.md`.**
   T3 Code, Cursor, VS Code + the GitHub Copilot chat extension, opencode v2
   desktop (its animations specifically). **None of this has been done.** Do not
   skip it and start moving pixels; the maintainer named these deliberately.
2. **Give Warp its own message kind.** The enabling change. Nothing good is
   possible before Warp's voice is separable at the renderer.
3. **Abbreviate the asking note after the first ask per conversation.** ~500
   characters back per ask, no permission change, largest measured effect for the
   smallest diff.
4. Then tool rows, turn shape, layered approvals.

Re-measure the 9.4:1 ratio after each step. It is the number to move.

## How to actually see what you are doing

A composer change is invisible to `cargo test`. Two instruments, both proven:

**Screenshot the running app.** On the Windows build:
```
powershell.exe -NoProfile -File 'C:\dev\shot.ps1' -Process warp-oss -Out 'C:\dev\shots\x.png'
```
It captures one window via `PrintWindow(PW_RENDERFULLCONTENT)` without raising or
focusing it. Omit `-Process` and it silently grabs the whole virtual screen,
which is unreadable. Then `Read` the PNG.

**Capture the wire.** What the agent actually sent, as newline-delimited JSON:
```
warp-oss.exe --warpctrl acp probe \
  --command 'wsl.exe -d Ubuntu -- npx -y @agentclientprotocol/claude-agent-acp@0.73.0' \
  --cwd '/home/effatha/git/warp' --mode default --output-format ndjson \
  --prompt '...'
```
Line 1 is the agent's `initialize` reply and carries
`agentInfo: {name, title, version}`. **Read it.** A finding was published as
"probed at 0.70.0" that had run 0.73.0, with the version printed on screen and
taken from memory anyway.

**Compare against the transcript**, which is Warp's own complete record:
`WARP_FORK_TRANSCRIPT=on` writes `.warp/transcripts/<conversation>.md` under the
pane's directory. The panel-vs-transcript diff is what produced the 9.4:1 number.

## Building and running

Everything here is measured, and each line has cost someone a session.

- **Build:** `cargo build --features gui,warp_control_cli`. Without
  `warp_control_cli` there is no `--warpctrl` and no instruments at all.
- **Cap the jobs:** `CARGO_BUILD_JOBS=8`. An uncapped release build took the WSL
  VM down; a single `rustc` on the `warp` crate holds 8–13 GB.
- **Windows is a separate checkout** at `C:\dev\warp` with its own git. `git -C
  /mnt/c/dev/warp log --oneline -1` against your own HEAD **before every Windows
  run** — a build there reports success and changes nothing if the tree did not
  move. Sync with `powershell.exe -NoProfile -Command "git -C C:\dev\warp fetch
  origin dev; git -C C:\dev\warp merge --ff-only FETCH_HEAD"`.
- **Launch instrumented:** `powershell.exe -NoProfile -File C:\dev\warpdev.ps1
  -On -Launch`. It refuses if a Warp is already running, which is deliberate.
- **Stop it with `warpctrl window close`**, never `taskkill`. Killing the parent
  leaves the crash-recovery sibling, which becomes a full Warp and publishes its
  own discovery record — measured.
- **Linux/WSLg:** `env -u WAYLAND_DISPLAY LIBGL_ALWAYS_SOFTWARE=1
  ./target/release/warp-oss`.

## The traps that will cost you a day

- **A doc saying "measured" is measured *as of its date*.** Re-run before
  building on it. This fork has burned nights on claims that were true when
  written.
- **Pin the agent version.** `npx -y …claude-agent-acp` unpinned resolves to
  whatever is newest, and 0.70.0 and 0.73.0 differ in ways that changed a
  conclusion. It is pinned to `@0.73.0` now; keep it pinned and log what
  answered.
- **Do not emit tool calls as `Action` / `ToolCall` messages.** On these paths
  the agent has *already run* the tool; an `Action` is an instruction and Warp's
  action model will run it again. This hazard is written down in
  `acp_agent/translate.rs`'s module docs and has been built against anyway three
  separate times. The prose is the record on these paths, deliberately.
- **`Exchange.tools` is always empty and `get_action_result` returns `None`** on
  both fork paths. That is by design, not a gap. Any design whose success
  criterion is "`get_action_result` returns `Some`" is the trap above wearing a
  different hat.
- **Warp's words must stay out of the transcript.** `transcript::strip_chrome`
  exists because an agent grepping its own history read Warp's asides as its own
  words. A new message channel must preserve that — and will make it easier.
- **A surface must never claim more than the code does.** Thirteen instances
  tracked. The live example: the approval card listed three options and drew two
  buttons with nothing saying the third could never be selected.

## The boundary

**Permission posture is frozen** (`.fork/GOAL.md`). This work is presentation.

- Abbreviating the asking note: presentation. In scope.
- Collapsing, restyling, layering disclosure: presentation. In scope.
- Adding an "allow always" control, changing what a *yes* buys, changing which
  options are selectable: **out of scope.** That is I18, and it is the
  maintainer's decision, not a design consequence.

If a change you want requires crossing that line, stop and say so rather than
building it.

## What the maintainer has said about this

Quoted rather than paraphrased, because it is the actual brief:

> this fork WILL have a top of class composer. currently we are failing that,
> and not by a little.

> there are a lot of apps to reference, t3 code has a nice composer, cursor,
> vscode and the github chat extension is pretty good, opencode v2 desktop is
> nice. love their animations. there are many. we need to dig into those.

And on the surrounding permission argument, which you will trip over and should
not reopen:

> Decision fatigue is real and it's more than simple "friction" it is inhibition
> to productivity.

> it makes no sense that arbitrary execution commands would be allowed but I'm
> having to "approve once" for each file read, cat, grep, find, etc.

That incoherence is real and is recorded in `.fork/IDEAS.md` under I18. **It is
not this ticket.** Do not solve it here.

## One correction to carry forward

`opencode` is **not** a tool the maintainer uses and never has been — it exists
on this machine only because this fork's testing introduced it. Much of
`CLAUDE.md`'s permission reasoning is built on `opencode.json` and is therefore a
worked example of a shape rather than evidence about real exposure. The agent
that matters is `claude-agent-acp`, and the config that governs it is Claude
Code's own `~/.claude/settings.json`, which is not in this repository.

## Working rules

- Commits: `fork: <lowercase subject> (COMPOSER)`. The body explains **what was
  found**, including when it contradicts something previously recorded.
- Corrections belong in the commit that makes them *and* in the doc that was
  wrong.
- Calibrate every new test by making it fail. A test that cannot fail is not
  evidence, and this fork has shipped several.
- When something has only been read, say so.
