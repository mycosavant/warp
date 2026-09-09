# Which model, how much effort, and what this fork pays for

*Written 2026-09-09 from Anthropic's [cost and performance
guidance](https://claude.com/blog/reducing-cost-and-improving-performance-with-claude-platform),
applied to this repository rather than summarised. The general advice is in the
post; what is here is the part that changes what you type.*

---

## The one number that matters here

```
CLAUDE.md              21,941 words   ~29,000 tokens
.fork/docs/manual.md   37,301 words   ~50,000 tokens
```

**`CLAUDE.md` is read into every session in this repo.** It is the single
largest fixed cost the project has, and it is also the reason sessions get
things right, so the answer is not to cut it — it is to *cache* it.

**Measured on the wire 2026-09-09, and the whole-project figure is larger than
the estimate above.** The same agent, the same prompt, two working directories:
an empty one produced a **20,860-token** request and `/home/effatha/git/warp`
produced **62,960**. So the project adds about **42,000 tokens to every
request** — `CLAUDE.md`, the skill it `@`-includes, and the project skill
listing together — on top of a 20,860-token floor that is Claude Code's own
system prompt and tool definitions and belongs to no repository.

That number stopped being an accounting curiosity the same day. A local
12B on a 12 GB card has to be given **96k of context** to answer a single
question in this repo, and at `serve.ps1`'s 12,288 default it cannot answer at
all (`.fork/runs/localmodel-panel-2026-09-09/`). For a cloud model the prefix
is a bill; for a local one it is a wall.

Two consequences that are actionable today:

- **A cached prefix must be byte-exact.** Editing `CLAUDE.md` invalidates that
  cache for every session started afterwards. That is fine once a day and
  expensive five times a day. **Batch edits to it** — collect corrections during
  a session, land them in one commit at the end, rather than amending it three
  times mid-run.
- **Long sessions want the 1-hour cache TTL.** A cache miss costs 1.25x normal
  input at the 5-minute TTL and 2x at the 1-hour one, and cache *reads* are a
  fraction of full input. For a session that spans a build (7 minutes) or a live
  run (30+), the 1-hour TTL pays for itself on the first resume.

The corollary for `manual.md` at 50k tokens: **never read it whole.** It says so
itself ("navigate by heading"). Grep it, or read the surface page instead. The
same goes for `next.html` and `board.html` — strip and slice, do not paste.

---

## Model by stage of work

The post's central finding is that **a stronger model at low effort beats a
weaker model at high effort**, and that the top of the effort range is where
the money goes for the least return — on some benchmarks the last step to max
costs 46% more for about half a point.

That maps cleanly onto the kinds of work this repo generates:

| stage | model | effort | why |
|---|---|---|---|
| **Measurement runs** — a recipe exists, the falsifiers are written, the work is executing and reading output | Sonnet 5 | low–medium | The expensive thinking already happened when the board item was written. This is execution against a spec. |
| **Design decisions** — "is this premise even true", scope calls, architecture seams, an expensive-to-reverse choice | Fable 5.1 / Opus 5 | high | Where being wrong costs a day of building. The cert-vs-APK question, the egress layering, `PAIRABLE_ACTIONS` additions. |
| **Adversarial review** — a diff before it is trusted, a security-shaped claim | Fable 5.1 / Opus 5 | medium–high | The cost of a miss is asymmetric. |
| **Doc sweeps** — "name anything whose doc comment claims something the code does not do" | Sonnet 5 | medium | Mechanical pattern-matching over many files. This is what found twelve stale docs in one day. |
| **Bulk mechanical edits** — renames, formatting, log triage | Haiku 4.5 | low | No judgment involved. |

**The discipline that saves the most money here is one the fork already has.**
`next.html` writes a *definition of done* and *falsifiers* before any code. That
converts a design problem into an execution problem — and an execution problem
runs correctly on a cheaper model at lower effort. The board's rigour is a cost
optimisation that nobody framed as one.

**Escalate on a surprise, not in advance.** Run the measurement on Sonnet. If a
falsifier fires and the cause is not obvious in one step, that is the moment to
bring a stronger model to the interpretation. Starting every run at the top of
the range pays for judgment that most runs never need.

---

## The prompt anti-patterns, and the uncomfortable one

The post lists instruction patterns that make frontier models worse and more
expensive: verification rituals, emphasis boosters ("be maximally thorough"),
manual scratchpad scaffolding, contradictory rules, and stale few-shot examples.
An audit that removed them cut cost 14.6% and *raised* accuracy 5.3%.

**This file's neighbour is one long verification ritual, and most of it earns
its place.** The distinction is worth stating precisely, because the wrong cut
would be expensive:

- **A generic booster is waste.** "Double-check your work", "be thorough",
  "consider all possibilities" produce duplicated tokens and extra tool calls
  and improve nothing.
- **A specific, earned rule is not a booster — it is domain knowledge.** *"Read
  the count off the test, never off prose"* and *"compute the merge base, never
  paste it"* are not exhortations to try harder. They are facts about how this
  codebase has actually burned people, and each one names the instrument. They
  pay for themselves the first time they fire.

**Where `CLAUDE.md` genuinely does carry cost** is the third category the post
names: *stale examples that teach outdated reasoning*. The file already records
fourteen instances of documentation outliving its code, several of them its own.
Every corrected-then-retained narrative is tokens spent teaching a session about
a world that no longer exists — and the file's own closing paragraph makes the
same complaint from a readability angle: *"every paragraph opens with a bolded
thesis… nothing is allowed to be a quiet supporting detail"*, which buries the
lookup-shaped facts people actually come for.

**So the audit worth running on this repo is not "remove the verification".** It
is: *for each retraction narrative, is the story still load-bearing, or has the
correction settled long enough that one sentence would do?* That is a real
piece of work, it is Sonnet-shaped, and it would shrink the per-session prefix
without losing a single rule.

---

## Batch, for the sweeps

The post reports ~58% lower cost for unattended batch workloads. Two recurring
jobs here are exactly that shape, and neither is interactive:

- **The stale-doc audit** — one question asked of every file in a directory.
- **The drift check** — `.fork/tools/drift-check.sh` against upstream.

Neither needs a person watching. Both are candidates for the Batch API if either
starts running regularly rather than occasionally.

---

## Context hygiene this repo already gets right

Worth naming so nobody "improves" it:

- **Run records are pointed at, not pasted.** `.fork/runs/<name>/README.md`
  cited by path costs nothing until something actually needs it.
- **Docs are split by surface.** Reading `wsl.md` instead of all of `manual.md`
  is a 10x difference in what enters context.
- **Findings live in commit bodies.** `git log` is a real source here, retrievable
  on demand instead of resident.

And one thing to keep doing deliberately: **append, do not edit.** New context
belongs at the end of a conversation, not spliced into the stable prefix, or the
cache is thrown away for a one-line change.

---

## The short version

1. Batch `CLAUDE.md` edits into one commit per session.
2. Never read `manual.md` whole; grep it or read the surface page.
3. Measurement runs on Sonnet 5 at low–medium effort. Escalate only when a
   falsifier fires and the cause is not obvious.
4. Design and adversarial review on Fable 5.1 or Opus 5.
5. Writing the falsifier before the run is what makes the cheap model correct.
