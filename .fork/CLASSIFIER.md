# A local permission classifier

**Filed 2026-09-03, at the maintainer's request, as its own file.** It is a
posture question with an implementation behind it, and both halves need arguing
before either is built.

**Status: measured 2026-09-03, and the answer is no model.** The evaluation set
is built (`.fork/classifier/`), the rule was tried, and the falsifier at the
bottom of this file fired. The section directly below is the result; the rest
of the file is the argument as it stood before the measurement, kept because the
posture half of it is still the maintainer's to settle.

---

## Measured 2026-09-03 — a rule handles a third, and the third was already built

Full account in `.fork/classifier/README.md`; the corpus, the labels, the rule
and eighteen probes are beside it. The shape of it:

- **The corpus has one class.** Of run 2's 44 asks the person answered **41
  yes**, 2 went unanswered to an accidental ctrl+c, and the 1 no was the person
  ending the run. A classifier fitted to the person's decisions learns *yes*;
  the only informative labels are hand-written ones, and those are an
  *envelope*, not a model's target.
- **Counted from the full inputs, the split is 28 host / 16 project** — the
  30/9/7 in the next section overlaps and sums to 46. The 16 are 7 edits inside
  the session cwd, 7 `cargo` commands, and 2 `git` calls to GitHub.
- **A containment rule auto-answers 14 of 44 (32%) with zero unsafe answers**,
  and both halves of it exist today: the seven `cargo` asks are the person's own
  `Bash(cargo:*)` rule defeated by the `CARGO_BUILD_JOBS=8` prefix `CLAUDE.md`
  mandates — measured 0 requests without the prefix, 1 with it, 0 with a rule
  that names the prefix — and the seven edits are what
  `WARP_FORK_ACP_MODE=acceptEdits` answers, measured scoped to the cwd.
- **The containment check itself is Claude Code's, already running.** An
  allowed verb on a path outside the cwd asks; `cd` out of it asks. Measured,
  then found in its docs. Warp building the same rule would duplicate the engine
  it is talking to.
- **The 28 host asks are the task.** The agent was syncing, building,
  screenshotting and driving the Windows Warp through `powershell.exe`. Seventeen
  are read-only by hand label; the signal separating them from the eleven that
  write, type into the user's pane, or launch a process sits inside a quoted
  PowerShell string the agent requoted twice. The one call that did harm was
  among them and the person approved it. That is the falsifier below, met.

**What this leaves the maintainer:** two configuration lines that remove a third
of the asks with no Warp change, the posture question below unchanged, and one
transport observation — the 17 read-only host calls are `warpctrl` read verbs
Warp could vouch for from its own catalog if the WSL side could reach the
Windows instance without `powershell.exe`. That is a build and not this ticket.

---

## The corpus is contaminated as evidence about *decisions* — read this first

**Added 2026-09-03 after the maintainer pushed back, and it is the most important
sentence in this file.**

The evaluation set below is built from one run's 44 asks, and the conclusion drawn
from it was that *every real decision was yes*, therefore a model trained on those
decisions learns only "yes". That inference does not hold, because **the person
answering those 44 asks was answering through a surface that was not showing them
enough to decide on.**

Measured, on the same run:

- The approval card rendered the agent's own one-line `description` — *"Compare
  local HEAD to Windows checkout HEAD"* — inside an escaped JSON blob beside a
  multi-line command. **29 of 29 parseable asks carried a filled `description`**
  and none of them was legible as one.
- `acts on`, the field that says what a call touches, read **"not stated by the
  agent" for all 36 execute asks**, because ACP sends no `locations` for a shell
  call.
- And separately, Warp's own chrome outweighed the agent's words **9.4 : 1**
  during turns with approvals, so the narration explaining *why* each call was
  being made never reached the screen (`.fork/COMPOSER.md`).

A stream of yeses from a person shown a JSON dump, an empty "acts on", and none
of the agent's reasoning is **not** evidence that those calls were safe. It is
evidence of what this file already calls the failure mode: rubber-stamping. Using
it as ground truth would train, or validate, against exactly that.

**So the corpus is sound for one purpose and unsound for another.** It remains a
good record of *what was asked* — tool kinds, commands, paths, the 30/9/7 split —
and that is what the rule measurement uses, which is why the rule finding stands.
It is not usable as labelled ground truth for *what should have been answered*,
and no model should be trained or evaluated against it.

**What a sound corpus needs:** asks collected after the card shows the
description and after the composer stops burying the agent's prose — both now
partly addressed, neither verified in a real session — and labels applied
deliberately, by a person reading each call, rather than inferred from what was
clicked under time pressure.

**And one run is one run.** The 30/9/7 split describes a session spent driving a
Windows host from inside WSL. A session spent writing Rust in one repository will
have a different shape, and the rule's 14-of-44 hit rate is a fact about that
afternoon rather than about the tool. Collect several before deciding anything
irreversible.

## The problem, measured rather than felt

Run 2 raised **44 permission requests in 50 minutes across 5 prompts** — one
every 69 seconds, roughly 420 for an eight-hour day. That is what ended the run.

Counted afterwards from the run's own event log, by what each ask touched:

| | count |
|---|---|
| the Windows host — `/mnt/c/dev/warp`, `powershell.exe`, `warp-oss.exe` | **30** |
| `cargo` builds and tests | 9 |
| inside the session directory and nothing else | **7** |

By tool kind: 36 `execute`, 7 `edit`, 1 `read`.

**And the configuration those asks arrived under is incoherent**, which is the
maintainer's finding and the reason this ticket exists rather than an
allow-list one. `~/.claude/settings.json` on this machine allows `python:*`,
`python3:*`, `node:*`, `cargo:*` and `make:*` — arbitrary code execution — while
a file *read* raises an approve-once prompt. The 30-rule deny list beside it
matches command prefixes (`sudo:*`, `dd:*`, `mkfs:*`), so
`python -c "import subprocess; ..."` matches none of them.

That is not a security posture. It is two unrelated defaults meeting, and it
produces the worst of both: **decision fatigue on the harmless half, and
rubber-stamping on the dangerous half.** A person who has answered *yes* forty
times is not reading the forty-first.

---

## The argument the fork has to have with itself

`GOAL.md` and `CLAUDE.md` are written against exactly this: *"a model classifier
deciding permissions — the `auto` mode `claude-agent-acp` ships with — is **not**
consent, and the fork discloses it rather than adopting it."*

**That sentence is doing two jobs and only one of them survives inspection.**

The real objection T14.18 measured was never *"a model decided"*. It was that
**Warp was not in the loop and could not say what happened**: a panel session
produced zero permission requests because the agent's own classifier answered
first, and this repo's own docs warn that a zero there means *Warp was not
asked*, never that nothing needed asking. The decision happened somewhere Warp
could not see, in a policy Warp cannot read, with no record.

A classifier that **Warp runs, locally, and logs** is a categorically different
object. It is not the vendor's cloud classifier operating out of view; it is a
decision the fork makes, on the user's machine, that the fork can explain
afterwards. On the fork's own thesis — nothing leaves the machine, the user's own
credentials, disclosure over choosing for people — that is arguably *more*
on-thesis than either alternative currently available:

- delegating to `auto`, where Warp is blind; or
- 420 prompts a day, where the human is a rubber stamp and the audit trail
  records consent that did not happen.

**And auto is the mainstream configuration, not an aberration.** It is Claude
Code's shipped default as a product; the settings file carrying
`defaultMode: auto` is one Claude Code wrote. The fork has been treating the
near-universal setup as a thing to be corrected, which is a posture decision
nobody took deliberately. `"a model decided"` is not self-evidently worse than
`"a person clicked yes forty-four times without reading"`. **That argument is
open. This ticket does not settle it — it is the thing to settle first.**

---

## What already exists underneath it

**`crates/input_classifier` is a working local inference pipeline**, and the
important thing about it is the machinery, not the model.

| piece | where |
|---|---|
| three `bert-tiny` ONNX models + tokenizer, embedded in the binary | `models/onnx/`, via `rust_embed` (`onnx/mod.rs:23-28`) |
| two interchangeable runtimes | `onnx_candle` and `onnx_ort` features (`onnx/mod.rs:1-4`) |
| a panic guard with automatic fallback | `HasPanicked` → `HeuristicClassifier` (`onnx/mod.rs:157-161`) |
| a record of **which path decided** | `InputClassifierDecisionSource`, six variants (`lib.rs:17-32`) |

It classifies **Shell vs AI** — whether the input box holds a command or a
prompt. **It is not a risk classifier and cannot be repurposed as one.** What it
proves is that this fork can ship an embedded model, run it locally with no
network, fall back safely when it fails, and record why an answer came out the
way it did. That is the whole hard part of the infrastructure, already paid for.

**One trap, and it is the kind `CLAUDE.md` warns about explicitly.** No
`nld_classifier_*` feature is in `app`'s `default` list — verify with
`cfg!(feature = "nld_classifier_v3")` in a test rather than by reading the TOML,
because a feature in `default` can enable others through its own dependency list.
This is a `#[cfg]` that **removes code**, so `fork::FORCE_ENABLED` cannot reach
it: a runtime preference resurrects a flag slot, it cannot conjure code the
linker dropped.

**And the consent machinery is already shaped for this:**

- `acp_permission::choose(request, decision) -> Choice` is the single seam every
  answer goes through, and `is_selectable` now asks it rather than restating it.
- `digest_of` binds an answer to the request that was shown.
- **`event_log::Entry::answered_by` already exists** and names the surface that
  answered (`control_plane` or `panel`). A classifier is one more value in that
  field, and the disclosure hook is therefore already built.

---

## The design, and its one non-negotiable shape

**Asymmetry.** This fork already builds consent asymmetrically —
`agent.deny` needs no switch because saying no can only ever make less happen,
while `agent.approve` needs `WARP_FORK_REMOTE_APPROVE`. The classifier must
inherit that shape:

- **Escalating is always allowed.** A classifier turning an auto-approve into an
  ask needs no permission and no confidence threshold. That direction cannot
  hurt.
- **Auto-approving is allowed only inside an envelope the person declared** —
  and the envelope is the user's, not the model's. The classifier chooses within
  it; it never widens it.
- **It must never silently deny.** A silent no wastes a whole turn and, measured,
  some agents do not survive a refusal at all. A classifier that would say no
  should ask instead.

**What it must never do**, from this fork's own record: emit an answer Warp
cannot explain afterwards. If the log cannot say *why* a call was auto-approved,
the feature has reproduced `auto` with extra steps.

### Open design questions, none of them settled

1. **What is it classifying?** Not "is this dangerous" in the abstract. Candidates:
   *does this call stay inside the session directory*, *is this read-only*, *does
   this reach the network*, *does this touch credentials*. The 30/9/7 split above
   suggests **"does this leave the project"** is the highest-value single axis,
   and it may not need a model at all.
2. **Does it need a model?** Genuinely open, and the honest answer may be no. A
   rule over `kind` + `locations` covers `edit` and `read` exactly, and
   `execute`'s command sits in agent-specific `raw_input` where a rule is
   fragile — which is where a classifier earns its keep, and also where it is
   most likely to be wrong.
3. **Where does it sit?** Inside `acp_permission::choose` as a pre-step, or
   beside it as a separate stage that produces a *recommendation* the existing
   path consumes. The second keeps `choose` the single seam.
4. **How is it calibrated?** This is the part that makes or breaks it. A
   classifier with no held-out evaluation is a vibe. There is a real corpus
   available — the event logs already record every ask with its
   `tool_input_preview` and what was decided — so the first buildable artefact is
   an **evaluation set**, not a model.
5. **What does the person see?** The fork's answer to everything adjacent has
   been disclosure: say what happened, in whose words, with what authority.
   Minimum: every auto-answer logged with `answered_by: classifier` and its
   reason, and a per-turn note in the panel. Note that this collides with
   `.fork/COMPOSER.md` — Warp's chrome already outweighs the agent 9.4:1, so
   *more* disclosure text is not free.

---

## The smallest thing that is still the idea

In order, and each is worth doing even if the next never happens:

1. **Build the evaluation set from the event logs that already exist.** Every
   `permission_request` line carries the tool kind, the input preview, the cwd
   and the decision. Label them. This answers question 1 with data instead of
   argument, and it is the only step that cannot be skipped.
2. **Try the rule first.** `kind` + `locations` for `edit`/`read`, and a
   path-containment test for `execute` where the command is parseable. Measure
   what fraction of the 44 it would have handled. If it is most of them, the
   model is unnecessary and this ticket ends happily.
3. **Only then a model**, and only for the residue — with the evaluation set from
   step 1 as the gate, and the `input_classifier` machinery as the template
   rather than the implementation.
4. **Disclosure last and non-optional**: `answered_by: classifier`, the reason
   recorded, and the panel told once per turn rather than once per call.

---

## The falsifier

If the evaluation set shows the asks are dominated by ad-hoc `execute` calls that
no rule and no small model can separate into safe and unsafe with high
confidence, then **this whole ticket is the wrong answer**, and the honest choice
is between `default` and a knowingly-adopted, disclosed `auto` — which is the
maintainer's decision and not a thing to build.

## What this is not

Not I18. That entry is about a *persistent grant* — a decision the person makes
once and Warp remembers. This is about a decision Warp makes each time, on the
person's behalf, inside a boundary they set. They interact, and route 3 of I18 is
close enough that the two should be scoped together, but they are different
objects and conflating them is how this gets built twice.

Not a permission-posture change by itself. Building the evaluation set and the
rule measurement changes nothing anyone can approve. Turning any of it on does,
and that is frozen and the maintainer's.
