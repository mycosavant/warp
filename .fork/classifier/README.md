# The permission-classifier evaluation set, and what it decided

**Built 2026-09-03 from logs that already existed. Nothing here changes what
Warp does; permission posture stays frozen and every number below is an input
to the maintainer's decision, not a decision.**

**Verdict, corrected 2026-09-03 the same evening — the first version of this
file said "do not build the model", and that does not stand.** What stands: a
containment rule handles a third of the asks exactly, and every part of that
rule already exists — in Claude Code's own permission engine and in the
maintainer's own allow list — so the third is recoverable with two configuration
lines and no code. What was retracted, by the maintainer's own re-measurement
(`57f0e866a`) and then by re-reading this directory's own data: the argument
that the other two thirds could not be separated. That argument was built on the
person's 41 yeses and on the command text, and both were the wrong evidence.
The yeses were given through a card that showed a JSON blob and an empty *acts
on*; and every one of the 36 shell asks carried a one-sentence description of
intent that the first version of this file never read, sitting one key over
from the command it analysed. Details under *The 28 that remain*. **The model
question is open again, with a concrete input to evaluate; nothing here decides
it.**

## What is in this directory

| file | what |
|---|---|
| `eval-set.jsonl` | 69 tool calls, one per line: every call in run 2 (59 — 44 asked, 15 not) plus the five control-plane probe conversations. Each carries the **full** tool input, the session cwd, whether Warp was asked, what the person decided and from which surface, hand labels, and what three rule envelopes would have answered. |
| `build_eval_set.py` | rebuilds it. The hand labels and the rule are in the file, so the rule is reviewable as code and the labels as data. Run it; the first thing it prints is a calibration. |
| `probes/probes.json` | eighteen `warpctrl acp probe` runs reduced to agent version, opening mode, and whether the agent asked; the prompts are in `index.md`. Every claim marked *measured* below is one of these rows. |
| `probes/summarise.py` | produces that file from the probe's ndjson. |

## Where the corpus came from, and why the event log alone was not enough

Warp's event log records every `permission_request` with a `tool_input_preview`
— **truncated at 320 characters**, which cut 14 of run 2's 44 commands — and
records the calls that never asked as a `tool_start` with an empty preview. The
agent's own session file (`~/.claude/projects/…/<linked_session_id>.jsonl`) has
the whole input for all 59, and the two join on the `toolu_…` id that Warp
already writes as `call_id`. All 59 matched; none were missing from either side.

Read the event log in timestamp order, not filename order, as `CLAUDE.md` says.

## Calibration first

The rule is scored three ways, and the first way is not a proposal — it is a
check. **E0** is Claude Code's own behaviour as this script understands it, and
it has to reproduce the log: an ask where Claude Code asked, none where it did
not. The first draft disagreed on 7 of 59 calls. Every disagreement was a rule
Claude Code applies that nobody here had written down, and each was then
measured with a probe rather than guessed:

| what the draft missed | probe | result |
|---|---|---|
| an allowed verb on a path **outside the cwd** asks | `ls /mnt/c/dev \| head -3`, both verbs on the allow list | **1 request** |
| `cd` out of the cwd asks | `cd /mnt/c/dev && ls \| head -3` | **1 request** |
| a relative path inside does not | `find .fork -maxdepth 1 -iname "*.md" \| head -3` | 0 |
| newline-joined commands split like `&&` | `ls .fork \| head -3⏎echo "---"` | 0 |
| a built-in read-only set exists | `git remote -v`, no rule matches it | 0 |

With those three rules in, **E0 reproduces the log on 59 of 59 calls.** So the
matcher is trustworthy for what follows, and the first finding is already on
the table: **the cwd-containment rule `CLASSIFIER.md` proposed is Claude Code's,
and it is already running.** Claude Code's documentation names the same rules —
compound commands are matched per segment across `&&`, `||`, `;`, `|`, `|&`,
`&` and newlines, and *"paths outside that scope … still prompt"* — which was
read after the probes, not before.

## The 44 asks, labelled by hand

Two axes: where the call acts, and the strongest thing it does.

| scope | effect | asks |
|---|---|---|
| **host** — `/mnt/c`, `C:\`, `powershell.exe` | read | 17 |
| host | write | 6 |
| host | build | 2 |
| host | input — `warpctrl input submit`, typing into the user's pane | 2 |
| host | **launch** — `warpdev.ps1 -Launch`, which started a second Warp (run-2 finding 5) | 1 |
| **project** | write — `Edit` on files under the session cwd | 7 |
| project | build — `cargo test`, prefixed `CARGO_BUILD_JOBS=8` | 7 |
| project | net — `git fetch` / `git ls-remote` to GitHub | 2 |

**28 host, 16 project.** `CLASSIFIER.md`'s original 30/9/7 overlapped (it sums
to 46); this is counted from the full inputs and the rows are disjoint.

Three of the 44 are artefacts of the run rather than of the agent: two edits
went `unanswered` because the person pressed ctrl+c meaning *copy* and the
panel read it as *cancel*, and each was then asked again. That is a composer
finding (`.fork/COMPOSER.md`), and it is two of the 44.

## What the person decided — and why the column says nothing

| decision | count |
|---|---|
| allowed | **41** |
| unanswered (the ctrl+c) | 2 |
| denied | 1 — the last ask, the person ending the run |

Every real decision was yes. **The first version of this file read that as the
rubber-stamp `CLASSIFIER.md` describes, measured. That was the wrong
mechanism under a correct observation**, the shape `CLAUDE.md` warns about most.
Measured by the maintainer's other session against the same run (`57f0e866a`,
`2c914dc98`): the approval card rendered `raw_input.to_string()` — the agent's
one-line description sat inside an escaped JSON blob beside a multi-line
command — *acts on* read *"not stated by the agent"* on all 36 shell asks
because ACP sends no `locations` for a shell call, and Warp's own chrome
outweighed the agent's words 9.4 : 1, so none of the narration explaining a call
reached the screen. A yes given under those conditions is not the person's
judgement about the call; it is the person's judgement that the card could not
be read and the work should continue.

So the conclusion survives with a different and stronger reason: the decision
column cannot calibrate anything, not because the person was careless but
because the person was blind. The corpus stays sound for **what was asked** —
the kinds, the inputs, the split — which is what the rule measurement below
uses. It is not usable as ground truth for what *should* have been answered,
and the card has since been fixed (presentation only; no permission changed).

## What a rule handles

| envelope | auto-answers | of which unsafe or outside the project |
|---|---|---|
| **E1** — the allow rules the person already wrote, with the env-var prefix stripped | **7 of 44** (16%): the seven `cargo` asks | 0 |
| **E2** — E1, plus edits inside the session cwd | **14 of 44** (32%): those seven plus the seven edits | 0 |

Residue under E2: **30** — the 28 host asks and the 2 network ones. The rule is
correct to leave every one of them: the agent was reaching another machine's
filesystem, launching processes on it, and typing into the user's shell.

**And both halves of E2 exist today, each measured:**

- **The seven `cargo` asks are the person's own `Bash(cargo:*)` rule, defeated
  by the prefix `CLAUDE.md` tells the agent to type.** Probed at
  `claude-agent-acp` 0.73.0 in session mode `default`: `cargo --version` raised
  **0** requests, `cargo --version 2>&1 | tail -1` raised 0, and
  `CARGO_BUILD_JOBS=8 cargo --version` raised **1**. Claude Code's docs say an
  allow rule *"won't match past an assignment of any other variable"*. The rule
  form that names the prefix works and the docs are silent on it:
  `Bash(CARGO_BUILD_JOBS=8 cargo:*)` in this repo's `.claude/settings.local.json`
  took the same command to **0** requests. The alternative is to retire the
  prefix by putting `jobs = 8` under `[build]` in a cargo config, which also
  retires the instruction in `CLAUDE.md`. Either is one line; neither is a
  permission-posture change in Warp.
- **The seven edit asks are what `WARP_FORK_ACP_MODE=acceptEdits` answers, and
  it is scoped.** Probed: in `acceptEdits`, a `Write` inside the session cwd
  raised 0 requests and the file appeared; a `Write` to a directory outside it
  raised 1 and nothing was written. Warp already discloses the mode in the
  panel every turn and re-sends it every turn. That is the *"allow all edits
  this session"* affordance I18 wants, with the disclosure built and nothing new
  to trust — and it is the maintainer's to switch on, not this ticket's.

**A trap met on the way, so nobody re-measures the wrong thing.** Six probes
put an allow rule in a fresh scratch directory's `.claude/settings.json`, with
and without `git init`, and the rule never took effect — which read as
*"project settings do not load through ACP"*. The same rule in this repo's
`.claude/settings.local.json` took effect at once. The mechanism was not
established (workspace trust is the candidate); what is established is that a
project-level rule is a lever in a directory Claude Code already knows and not
in one it has never seen, and `settingSources` in `dist/` could not have told
you that.

## The 28 that remain — and the field this file did not read

Nineteen are `powershell.exe …` invocations and nine name a `/mnt/c` path
directly. By hand, 17 are read-only — mostly `warpctrl` read verbs (`instance
list`, `pane read`, `session inspect`, `--help`) — and 11 are not: syncing and
building the Windows checkout, screenshots written to `C:\dev`, two `input
submit` calls that typed a command into the user's pane, and the `-Launch`.

**The first version of this section argued that the discriminating signal sat
inside a quoted PowerShell string the agent requoted twice, that a prefix rule
could not reach it and a model would be guessing, and that the person had
approved the harmful launch — therefore the falsifier was met. Retracted.** The
person approved the launch blind (previous section). And the signal was never
only in the command: **every one of the 36 shell asks carried a one-sentence
`description`** — *"Enable instrumentation and launch the Windows Warp build"*,
*"Submit a shell-identifying command to the active pane"*, *"Compare local HEAD
to Windows checkout HEAD"* — which this directory's own `eval-set.jsonl` had
held from the start under `input.description`. Scored now against the hand
labels, by nothing cleverer than the description's first verb
(`intent_reads_as_read_only` in the builder):

| the description's first verb | hand label | asks |
|---|---|---|
| reads as an action (sync, launch, submit, take, copy, rebuild, run…) | write / input / launch / build | **16** |
| reads as read-only (check, compare, list, inspect, read, locate…) | read | **16** |
| reads as read-only | net — `git fetch` / `ls-remote` | 2 |
| reads as read-only | build — `cargo test` filtered to a listing | 2 |
| no description | read — the one `find … ls /mnt/c` call | 1 |

**Zero of the sixteen writes, inputs, launches and builds read as read-only.**
The four over-inclusions are a fetch and a test listing, which is what a read
verb honestly covers. So the sentence the agent sends on every ask separates
the 28 as well as the hand labels do, and it was the one thing on the card a
person could not find.

Two things follow, and neither is a verdict:

- **Ergonomically, the measured problem was the card, not the count.** Forty-four
  asks with a legible one-line intent and a readable command is what Claude
  Code's own `default` mode shows, and it is the ordinary experience of the
  harnesses the maintainer compares this to. Whether 44 asks in 50 minutes is
  disqualifying *with the fixed card* is unmeasured, and it is what run 3
  measures before anything here is built.
- **The description is agent-authored.** It is not a boundary against a hostile
  agent, and neither is anything else on this path — `acp_permission.rs` says
  what it defends against is honest agents. For an honest agent, a rule over the
  description plus Claude Code's containment check is a candidate for *ordering
  and highlighting* on the card, and a candidate input for a model if one is
  ever wanted. Whether it is fit to auto-answer inside an envelope is the
  posture question, unchanged and the maintainer's.

**Still true, and still not this ticket:** the 17 read-only host calls are
`warpctrl` read verbs Warp could vouch for from its own catalog, if the WSL side
could reach the Windows instance without `powershell.exe`. That is a transport
build.

**One run is one run.** This split describes an afternoon driving a Windows host
from inside WSL. A session writing Rust in one repository will look different,
and the rule numbers above are numbers about this afternoon.

## What this changes in the standing argument

- **"Does it need a model?"** — Unanswered. The rule handles a third and the
  third is already built; the first version of this file said the rest could not
  be separated, and that rested on evidence contaminated by the card and on
  never reading the description field. The next input is a run with the fixed
  card.
- **"What is it classifying?"** — Containment is already classified by Claude
  Code. What remains unclassified is *effect*, and the agent already states it
  in a sentence on every shell ask; how far that sentence can be trusted is the
  question.
- **"How is it calibrated?"** — Not against this run's decisions, which were
  made blind. Against hand labels, or against decisions from a run where the
  person could see.
- **The posture argument in `CLASSIFIER.md` stands unchanged** and is the
  maintainer's. What this corpus adds is that a widened `default` costs two
  lines, and that the asks it does not remove were asks about another machine.

## Re-running

```
python3 .fork/classifier/build_eval_set.py            # rebuild and print the tables
python3 .fork/classifier/probes/summarise.py probe-*.ndjson > probes/probes.json
```

The builder defaults to the Windows event directory and this machine's Claude
Code project directory; both are flags. A new ask in run 2's conversation with
no entry in `LABELS` stops the build rather than defaulting, because an
unlabelled row in an evaluation set is the thing the set exists to prevent.
