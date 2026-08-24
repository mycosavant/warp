# Handoff — Consolidating three projects onto the Warp fork

**Paste this whole file as the opening prompt of a local session.** Written to be
read cold: it assumes `warp/`, `kode-rs/` and `tusk/` on disk, plus the full
local `dev-docs/`, and assumes you have seen none of the conversation behind it.

It supersedes the earlier `HANDOFF_IDENTITY_AND_WORKFLOW.md`. That document asked
"what is kode, and where does Tusk fit." The answer arrived from an unexpected
direction — a fork of `warpdotdev/warp` — and this document folds the old
question into the new frame. §6 is the part that survived unchanged.

Sections 1–3 are **durable reference**: measured findings and licensing analysis
that should not need re-deriving. Sections 4–9 are the work.

---

## 0. Where the maintainer stands

Stated directly, so you do not re-litigate settled things:

- **The Warp fork is the preferred direction.** Not a tentative option — a week
  of work and 515 commits behind it. Treat it as the frame, not a candidate.
- **The output is open-source software**, on principle, not as a fallback.
  Monetization, if any, is cloud agents and infra — the space Warp itself
  monetizes.
- **Attribution matters to this maintainer more than the license requires.** The
  stated position is not wanting to "take, steal, or even borrow a single line of
  code or concept without attribution." Design for that, not for the legal floor.
- **The clean-room rebuild is dropped.** See §2 for why it bought nothing.

---

## 1. Measured state of the fork (2026-08-24, `mycosavant/warp` `dev`)

Verify before citing; every figure has its command.

```bash
cd warp
git diff --stat <upstream-base>...origin/dev | tail -3
git diff --name-status <upstream-base>...origin/dev | awk '{print $1}' | sort | uniq -c
git diff --name-status <upstream-base>...origin/dev | awk '$1=="M"{print $2}' | cut -d/ -f1-2 | sort | uniq -c | sort -rn
```

| | |
|---|---|
| Divergence from upstream | **1168 files, +168,895 / −23,492**, 515 commits |
| Split | **888 modified**, 271 added, 5 deleted |
| Where the modifications land | `app/src` **509** · `warp_tui` 104 · `warpui_core` 38 · `ai` 27 |
| `.fork/` documentation | 10,712 lines across 4 files |

**The finding that matters: this is not a patch series and cannot become one.**
The original plan was to keep fork behavior in `.fork/` and carry patches to stay
syncable with upstream. 888 modified files — 509 of them in `app/src`, the
busiest directory in the repo — says that plan is already over.

That is not a failure. The changes wanted are *behavioral* (no account, no
telemetry, a different agent in Oz's seat), and behavior cannot be bolted on
beside an app. `.fork/` succeeded at what it can do — holding the reasoning — and
the code was always going to live in Warp's files.

**Consequence for this session:** stop planning around upstream syncability.
Cherry-picking specific upstream commits stays possible and worthwhile; merging
upstream wholesale does not. Decide the cadence deliberately (§4).

---

## 2. Licensing — the durable reference

*Not legal advice. This is what the licenses and the project's own FAQ say. The
one question worth paying counsel for is named at the end.*

### 2.1 The actual split

| License | Scope | Rust LOC |
|---|---|---|
| **MIT** | `warpui_core` + `warpui` only | 120,918 |
| **AGPLv3** | everything else — `app/`, `ai/`, `warp_tui`, 76 other crates | 362,408 |

```bash
find crates/warpui crates/warpui_core -name '*.rs' | xargs cat | wc -l
find crates -name '*.rs' | xargs cat | wc -l
```

MIT covers the *UI framework* — the general-purpose GPUI-like layer Warp wants
reused elsewhere. The product is AGPL, and 509 of the fork's modified files are
on the AGPL side. **Treat the fork as AGPLv3 with an MIT subsystem**, not as
"mixed." Nothing about this fork's actual work is MIT.

### 2.2 What Warp's own FAQ establishes

Read `FAQ.md` §Licensing directly; four things are load-bearing.

1. **Forking is the intended case.** *"Can someone fork Warp? Yes — that's what
   AGPL is for. The license prevents fully-proprietary relaunches; open
   derivatives are welcome."*
2. **Obligations attach on distribution or network use, not private use.**
   `.fork/README.md` already states this correctly.
3. **There is a CLA**, scoped to *"redistribute contributions under this
   project's licenses and to address future licensing and compliance needs."*
   That last clause is a relicensing grant.
4. **The server and Oz are not open**, and Warp has not committed to opening
   them.

### 2.3 Can Warp take fork work without a PR?

**Yes — but only under AGPL, and that asymmetry is the strategic fact.**

Published fork modifications are AGPLv3 derivative work. Anyone, Warp included,
gets the AGPL grant: use, modify, redistribute — *provided* they keep it AGPL,
preserve copyright notices, and offer Corresponding Source. No PR needed, no
permission needed.

What the CLA buys them is everything *outside* AGPL. Without a signature:

| | Warp may | Warp may not |
|---|---|---|
| Fork code, unsigned | adopt it into their AGPL client | move it into the proprietary server or Oz |
| | reimplement the *ideas* freely | relicense it in any future license change |
| Contributed via PR (CLA signed) | all of the above | — |

So the bucketing rule: **a genuine bug fix upstream costs nothing worth keeping.
The de-account and de-telemetry architecture is a different category.** Decide
per change, not once.

Note also that ideas, architecture and APIs are not copyrightable regardless —
Warp can reimplement any *concept* from the fork with or without a CLA. The CLA
is about code.

### 2.4 Monetization — AGPL supports the plan, and Warp is the proof

AGPL §13 closes the hosted-derivative loophole: if users interact with **the
modified program** over a network, they are owed its Corresponding Source. It
does not reach a *separate program* the client talks to over a protocol.

That distinction is precisely what Warp runs on — AGPL client, proprietary
server, proprietary Oz. **The intended model is the model being forked.** An
AGPL client fork plus a proprietary cloud-agent and infra service is coherent.

Three caveats, in order of how much they should shape design:

- **Keep the seam a protocol, not a library.** The further the cloud component
  drifts from "separate program over a documented wire format" — shared crates,
  types compiled into both, code the client links — the weaker the separation
  argument. Design the boundary now, while it is cheap.
- **A source-withheld client is the one thing that definitively does not work.**
- **This specific boundary is contested and fact-specific.** It is the question
  worth actual counsel before revenue depends on it. The *fork* question is not —
  that one is settled by their own FAQ.

### 2.5 Why the clean-room rebuild was dropped

Recorded so it does not get re-proposed:

1. **It solves a problem that does not exist here.** Clean room exists to build a
   proprietary replacement for a proprietary original *without a license*. There
   is a license, and it grants copy, modify and distribute explicitly. The only
   thing a clean room buys is escaping copyleft — i.e. going proprietary — which
   contradicts the stated goal.
2. **A solo clean room is not one.** The procedure requires personnel separation:
   one team reads the original and writes a spec stripped of expression, a
   *different* team that has never seen it implements from that spec. One person
   doing both halves provides close to zero protection, since the test is access
   plus substantial similarity and access is documented.
3. **Ideas, architecture and APIs are not copyrightable anyway** — reinforced for
   APIs by *Google v. Oracle*. The concepts were always free.
4. **Cost without benefit.** 483k LOC of Rust, where the expensive parts (GPU
   text rendering, PTY handling, the block model, pane tiling) are exactly where
   re-deriving lands somewhere worse than what already runs.

What was *right* in the instinct is the comprehensive spec. Write it — as the
spec for what is being built on the substrate, not as a laundering step.

### 2.6 Attribution: do this, and it is settled

The maintainer's standard is above the legal floor, so meet the standard:

- Keep every upstream copyright header intact; never rewrite `Copyright (C)
  2020-2026 Denver Technologies, Inc.`
- Keep `LICENSE-AGPL` and `LICENSE-MIT` in the tree, unmodified.
- Say in the README, plainly and early, that this is a fork of
  `warpdotdev/warp`, with a link. Do not imply endorsement or affiliation.
- Where a fork feature is *upstream's design, opened rather than invented*, say
  so in the doc for that feature. `.fork/` already does this well — the "Look for
  the gate first" table in `CLAUDE.md` is the model. Keep that habit; it is both
  honest and the most useful thing a future reader can be told.
- On distribution, ship Corresponding Source (AGPL §13 and §6). A public repo
  at the released commit satisfies this; make the release notes name the commit.

---

## 3. Why the substrate is worth the fork — recorded, because it is easy to forget

The maintainer's assessment after a week, which the code corroborates: Warp
repeatedly contains a complete, tested feature that is **switched off, wired to
one surface only, or documented as impossible when it is not.** `CLAUDE.md`'s
"Look for the gate first" table lists six, including the entire local control
plane, local Warp Drive sync, the whole agent transport (one function),
screenshots/input/recording, and the complete remote-development server behind a
*packaging* gate.

The Oz agent seat in particular **was pluggable** — swapping in Claude required
no architectural carving. That is the single most important input to §4: it means
consolidating onto Warp is largely integration work, not a rebuild.

Doc comments asserting macOS-only or Windows-impossible have repeatedly turned
out to be false. Treat every such comment as an unverified claim (`CLAUDE.md`'s
"Method: run it").

---

## 4. The decision this session ratifies

**The Warp fork is the product.** Tusk's concepts migrate into it; kode-engine
becomes one engine among several that it drives; Tusk-as-a-Tauri-app likely
retires.

### Why this is the right shape

Tusk's structural weakness was named by the maintainer before the fork existed:
with any harness other than kode, *"Tusk is sort of just the GUI layer."* A
cockpit beside the terminal is a context switch, and its value is capped by how
deep it can reach into a harness it does not own. The Warp fork does not have
that problem — it **is** the surface where work happens, and it already solved
the expensive parts.

### What migrates

Cockpit-tier capabilities, most of which are pure logic and therefore cheap to
move:

- worktree-per-task lifecycle
- the plan graph and dependency sequencing, with the hard run gate
- the board and auto-advance
- the completion-review gate
- checkpoint capture/restore (`commit-tree`-based, real git objects)

### What to check before discarding Tusk wholesale

**The web surface.** `serve.rs`'s read-only HTTP+SSE front door, pairing + QR,
the authority split, and the PWA. Remote access to your own session is genuinely
differentiated and Warp may have no equivalent. Determine this by reading, not by
assuming — and note the fork already opened Warp's remote-development server
(`.fork/README.md`, "Warp's remote server, in a WSL distribution"), which may or
may not cover the same need. **These are different things** — one is
remote *development*, the other is remote *observation of a running session*.

### The licensing structure that preserves optionality

Tusk and kode-rs are both `MIT OR Apache-2.0`. Permissive code may flow into an
AGPL work — legal, but a one-way door: the *combined* work is AGPL.

**The door closes on Warp's code, not on yours.** You hold copyright on tusk and
kode-rs, so you can keep licensing your own code any way you like, forever.
Therefore:

> Keep `kode-engine` and Tusk's pure logic cores as standalone MIT/Apache crates.
> Have the AGPL fork **depend** on them. Only the integration is AGPL.

Result: an AGPL product, an engine anyone can embed anywhere, and a permissive
logic layer that could carry a different product in two years. This maps exactly
onto Tusk's existing **"pure core, impure seam"** convention — `plan_graph.rs`,
`board.rs` and the `*.logic.ts` modules were already written to be extractable.

**Do this before the migration, not after.** Moving a crate out of an AGPL tree
later is harder than starting it outside.

---

## 5. The file-as-contract pattern — generalize it deliberately

The maintainer's own principle, and the strongest architectural idea in play:

> *"the simplest version of the thing that's still the thing; not a degraded
> version"*

It is already stated in `warp/CLAUDE.md` as **"Prefer the smallest thing that is
still the idea,"** and `crates/warp_cli/src/local_control/graph.rs` is its best
expression. Read that file's header comment before designing anything here — it
argues the case better than this handoff can:

> *"What is missing is not the mechanism. It is that the sequencing is a decision
> the model makes in the moment rather than a declaration made before the run. …
> The reason it is a* file *is durability. A plan held in the lead agent's context
> degrades exactly as the work gets long enough to need it, and compaction is the
> moment it is most at risk. A file is also diffable, reviewable, and lands in a
> commit next to the work it describes."*

And its design call, which is better than Tusk's:

> *"One edge type, because a dependency* is *an edge that carries a payload:
> `hands-to` is `depends-on` plus 'and here is what to pass'. Two edge types would
> have to be kept consistent, and a graph where B hands to C but C does not depend
> on B is a bug you can draw."*

**Tusk's `plan_graph.rs` has two edge concepts and a database. The fork's has one
edge type and a TOML file.** When the plan graph migrates, migrate toward the
file, not the schema. Tusk's genuinely load-bearing additions — cycle checking,
the sealed-subgraph guard, patch validation — are *validations over* a graph and
port cleanly onto the file representation.

### Where the pattern applies next

Three candidates, already identified:

| Subject | Today | As a file |
|---|---|---|
| Task graph | fork: TOML ✅ · Tusk: SQLite | done in the fork; port Tusk's validations |
| Task deps + handoffs | fork: TOML spec ✅ | already the model |
| Agent telemetry / observability | scattered | an append-only event file per run |

**The test for whether the pattern applies:** does the artifact need to be
durable across a context compaction, reviewable in a PR, and readable by more
than one program? Three yeses means file. A UI's transient view state is not a
file; a plan is.

**The failure mode to avoid:** a file that is a *degraded* version of a database
— one that loses the invariants. Port `plan_graph.rs`'s cycle check and sealed-
subgraph guard *as validators over the file*, with `deny_unknown_fields` (which
`graph.rs` already sets). A file with no schema enforcement is the degraded
version.

---

## 6. GitHub Projects as a proving ground — engage with this seriously

The maintainer's observation: GitHub Projects may be the simplest version of what
Tusk's project-management surface was reaching for, and an MCP integration may be
enough to prove the concept.

**Assessment: an excellent proving ground and a poor final home, and the seam
between those is sharp.**

| Works well on GitHub Projects | Must stay local |
|---|---|
| board views, status columns, custom fields | worktree binding and lifecycle |
| iterations, roadmap view | the synchronous run gate |
| assignment, ownership | checkpoint capture and restore |
| multiplayer, auth, mobile — all free | dependency invariants (cycles, sealed subgraphs) |
| already where the issues live | anything needing sub-second latency or offline |

The line is **"does this need to know about the local filesystem, or block a run
synchronously."** GitHub is a network round trip with no worktree concept and no
DAG validation; it cannot gate a run. It is, however, a board that already exists,
already has apps, and costs nothing to maintain.

**The synthesis worth building, which unifies this with §5:**

> **The file is the contract.** A task graph committed to the repo is read by the
> local runner (to gate runs and bind worktrees) *and* projected into GitHub
> Projects for the board view, by an Action or an MCP sync. Neither system owns
> it. It survives without either. And plan changes go through PR review, which no
> database gives you.

That is strictly better than choosing between them, and it makes the GitHub
integration a *view* rather than a dependency — which means the proving ground
can be abandoned for free if it disappoints.

**Suggested experiment, bounded:** one-way sync (file → Projects) for a single
real project, for two weeks. One way, because bidirectional sync is where this
class of integration reliably dies. If the board is genuinely useful read-only,
*then* consider write-back.

---

## 7. The tier question, restated for three projects

This is what survived from the earlier handoff. The frame still holds; the
occupants changed.

| Tier | Job | Occupant, after §4 |
|---|---|---|
| **Harness** | drive a model against a repo, interactively | **the Warp fork** — and claude-code, codex, aider |
| **Engine** | execute a turn headlessly against an injected worktree; emit structured events | **kode-engine**, and anything implementing the adapter contract |
| **Cockpit** | manage many tasks across projects; worktrees, plans, dependencies, review | **the Warp fork** — absorbing Tusk's concepts |

The fork occupies two tiers. That is not a problem — it is what "wrapping the
terminal" means — but it makes one rule load-bearing:

**Keep the engine seam a contract, even though one engine is yours.** Depth comes
from enriching the adapter contract, with kode-engine simply the first to
implement all of it. The named anti-goal is **capability drift**: every time depth
is added by special-casing kode rather than extending the contract, the
multi-engine story degrades one notch, silently, and no test fails.

Detection: grep the fork for adapter-identity branches. A rule with no detector is
a wish — if you keep this one, give it a lint.

### The three questions to answer explicitly

1. **Is terminal `kode` (the TUI) still a product?** The fork now occupies the
   harness tier. If `kode` stays first-class, say what it is *for* — that answer
   used to be "the terminal-native harness," and the fork now does that better.
   The honest answer may be that `kode-rs` becomes an engine repo plus a
   permissive logic library, and the TUI becomes how you dogfood the engine.
2. **Does Tusk survive as an app?** §4 says probably not, with the web surface as
   the open question.
3. **What is the migration order?** See §10.

---

## 8. The four kode-rs overlap decides — mostly dissolved by §4

Background: `kode-rs/WIRING_INVENTORY_2026-08.md` §D and
`kode-rs/dev-docs/OVERLAP_DECIDES_2026-08.md`. The inventory found much of
kode-rs built, tested and never driven — 7 orphaned `kode-core` modules, 2 crates
nothing depends on, ~120 service types never constructed. It escalated four
"overlaps" as product calls.

The briefs found **three of the four are not overlaps at all**:

| Pair | Finding | Under §4 |
|---|---|---|
| coordinator ↔ swarm | Not an overlap: `coordinator/` is policy, `swarm/` is mechanism; a grep for any policy symbol across `swarm/` returns nothing | Delegation policy is engine-tier. Either wire coordinator as the front-end to TR-EVENTS-D's gates, or delete it — but decide it as *engine* policy |
| teleport ↔ Tusk checkpoints | Not an overlap: `TeleportSnapshot` is session state with no file contents; checkpoints are real git objects | **Resolves to delete.** The fork owns session state; per-task state lives in worktrees |
| plugins ↔ skills | Not rivals — `PluginCapabilities` has a `skills: bool`; a plugin *contains* skills. **Blocked**, not open: `SkillRegistry` is written once and never read, so all 8 bundled skills have never executed | Still blocked. Fix the registry bug first; the fork has its own skills surface (`.agents/skills/`) worth comparing against |
| bridge ↔ serve | Genuine rivalry, and it turns on whether remote *control* is wanted at all — which serve's read-only-by-construction envelope deliberately refuses | Now a **fork** question (§4's web-surface check), not a kode-rs one |

**Binding constraint:** per `kode-rs/CLAUDE.md`, unreached code there is presumed
**unfinished, not abandoned**; removal is a `decide` needing sign-off, never
folded into another change. That rule exists because it was violated once — a
430-line superset with 20 green tests was deleted as a "duplicate" of the weaker
implementation that happened to be wired first, and the deletion was reverted.
**Ask whether an unreached implementation is weaker or stronger than the one it
appears to duplicate.** Reachability answers a different question.

---

## 9. Dev-workflow optimization pass

### The hypothesis, verbatim

> *"I have noticed that Opus 5 is much more literal in direction following, and I
> think that many of the delegation and orchestration policies that existed to
> direct other models, this one seems to be encumbered by."*

Strong structural evidence before you even reach model behavior.

### Measured surface (2026-08-24; verify before citing)

```bash
cd kode-rs && wc -l CLAUDE.md AGENTS.md && cat crates/*/CLAUDE.md crates/*/AGENTS.md | wc -l
cd tusk    && wc -l CLAUDE.md AGENTS.md .claude/rules/*.md
cd warp    && wc -l CLAUDE.md AGENTS.md && ls .agents/skills | wc -l
```

| | lines |
|---|---|
| kode-rs root `CLAUDE.md` + `AGENTS.md` (always loaded) | **1,045** |
| kode-rs all per-crate pairs | **7,780** |
| kode-rs worst-case single task (root + `kode-tui` pair) | **2,804** |
| tusk `CLAUDE.md` + `AGENTS.md` + 6 `.claude/rules/*.md` | **1,392** |
| warp fork `CLAUDE.md` + `AGENTS.md` | **470** |

**The fork is a third the size of either other project's governance and is the
most effective of the three.** That is the finding. It is not that documentation
is bad — `.fork/` is 10,712 lines and clearly earns its place — it is that
*always-loaded directive* surface and *reference* surface are different things,
and the fork is the only one of the three that separates them.

kode-rs's root file says, in caps, that you MUST read the per-crate pair before
any task in that crate. A literal reader obeys literally: a one-line fix in
`kode-tui` costs 2,804 lines of governance first. All eleven pairs differ, so
someone maintains both halves of each.

### Two costs, two different fixes

- **Token cost** — 2,804 lines read before work starts. Fixed by *restructuring*:
  progressive disclosure, an index instead of a dump, merging the pairs.
- **Compliance cost** — a literal reader executes every step it is told to,
  including steps written as encouragement for a weaker model. Fixed by
  *rewriting the directives*, not shortening them. "You MUST read these before ANY
  task" is a compliance-cost line, and shortening the file it points at does not
  touch it.

Most of the value is in the second, and it is harder to see, because the file
looks fine.

### Specific items

**a. `AGENTS.md` vs `CLAUDE.md`.** Eleven divergent pairs in kode-rs, one in
Tusk. Warp uses both plus `.agents/skills/` — and warp's are *skills*, loaded on
demand, which is the pattern that actually works. Is the second file read by a
tool that exists?

**b. `tusk/.claude/rules/orchestration-protocol.md`.** Assigns work by model tier
— Fable 5 lead, Opus implementer, Sonnet worker, Haiku trivia — with a delegation
contract requiring self-contained prompts and disjoint file sets. That is
**fleet-management policy**. With one capable model doing everything it becomes
ceremony: prompts written to be self-contained for a subagent never spawned. Ask
what it buys now.

**c. `tusk/.claude/rules/validation-protocol.md`** — 323 lines, 17 steps, a
~40-row scope table. Probably **load-bearing and worth its cost**; it encodes
specific incidents (the Windows `continue-on-error` trap, the macOS gating tiers,
the two-green-runs admission rule). Do not reflexively cut it. The question is
whether a reader finds the right 3 steps quickly or reads all 323 to be safe.

**d. Warp's skills model as the target shape.** 20 skills under `.agents/skills/`,
loaded on demand by trigger. That is progressive disclosure done properly, and it
is the strongest candidate for what replaces the always-on pairs elsewhere.

**e. Where ceremony has paid — argue both directions.** `review-protocol.md`
check 8 (bidirectional spec drift) catches a real class of problem. The
gatekeeper's "separate **verified** from **asserted**" is excellent and rare.
`CLAUDE.md`'s "run it, don't read it" method has repeatedly caught false beliefs.
**The pass should produce a smaller, sharper set — not merely a smaller set.**

### Deliverable

A workflow decision record naming: which documents stay, are rewritten, merge, or
retire, each with a one-line reason; what replaces the `AGENTS.md`/`CLAUDE.md`
split; whether the warp skills model generalizes to the other repos; whether
`orchestration-protocol.md` survives. Plus a rule for when a directive is worth
writing down at all — proposed test: **a directive earns its place if a reader
who ignored it would produce visibly worse work.** Directives that merely restate
good practice are pure cost, because a literal reader spends effort proving
compliance.

---

## 10. Suggested order

1. **Extract before you migrate.** Move `kode-engine` and Tusk's pure logic cores
   into standalone MIT/Apache crates while they are still outside an AGPL tree
   (§4). Cheapest now, and it is the step that preserves every later option.
2. **Ratify §4 and §7** in a decision record. In the fork it goes in `.fork/`;
   Tusk's house format is `docs/plans/*-decide.md`.
3. **Settle the web-surface question** — read `serve.rs` and the fork's remote
   server, decide whether they are the same need.
4. **Port the plan graph toward the file**, carrying Tusk's validations onto
   `graph.rs`'s representation (§5).
5. **Bounded GitHub Projects experiment** — one-way, one project, two weeks (§6).
6. **Workflow pass** (§9), holding the two costs separate.
7. **Re-derive the four decides** (§8) from the ratified frame; confirm they come
   out consistent.

Steps 1–3 are the session. The rest may want to be their own.

---

## 11. House rules that bind this session

From the repos' own committed instructions. Several exist because they were
violated once.

- **No Claude Code session links** in commit messages, PR titles/bodies, or
  comments. Both kode-rs and Tusk call this a privacy concern and say it
  **overrides** default footer behavior. `Co-Authored-By: Claude` is fine.
- **Warp fork commit format:** `fork: <lowercase subject> (Txx)`, body explaining
  *what was found*, including when it contradicts something recorded earlier.
  Corrections belong in the commit that makes them and in the doc that was wrong.
- **Method: run it.** Warp's `CLAUDE.md` is built on this and records five
  occasions where reading produced a confident wrong answer that running
  corrected. When something has only been read, say so.
- **Unreached code in kode-rs is presumed unfinished, not abandoned** (§8).
- **Verify with a scan, not memory** — and remember the scan is the error-prone
  half. In one doc audit six claims looked wrong and four of them were fine; the
  greps were broken. "Correcting" a true statement into a false one is worse than
  leaving it, because the next reader has no reason to doubt a fresh line.
- **Tusk: pure core, impure seam**, and a new `*.logic.ts` lands with its
  `*.test.ts` sibling in the same commit.
- **Secrets never reach a log, an event, or the wire.** All three repos use
  paired-arm `safe_*!` macros whose `full:` arm a release build erases.
- **Build warp with `--features gui,warp_control_cli`**, and stop a running
  instance with `warpctrl window close`, never a kill.

---

## 12. What NOT to do

- **Do not implement features this session.** Documents and decisions. If you are
  editing a `.rs` file you have drifted.
- **Do not plan around upstream mergeability** (§1). Cherry-picking stays open;
  wholesale merging does not.
- **Do not revive the clean-room rebuild** (§2.5).
- **Do not delete any unreached kode-rs module** (§8).
- **Do not open upstream PRs** without an explicit per-change decision — the CLA
  attaches and is a relicensing grant (§2.3).
- **Do not gut the validation protocol** to make it shorter (§9e).
- **Do not migrate Tusk code into the AGPL tree before step 1** (§10).

---

## 13. Current state

- **warp fork** `mycosavant/warp` `dev` — 515 commits ahead, measurements in §1.
  `.fork/` has README (operating manual), SPEC (de-telemetry reasoning), TASKS
  (board + as-built), IDEAS (holding pen). `CLAUDE.md` is the cold start.
- **kode-rs** `dev` at `83bb71d`, CI green on three jobs (ubuntu fmt+clippy+test,
  kode-engine build+run on macOS and Windows). `workflow_dispatch` exists, so CI
  can be run against a branch on demand. `dev-docs/` is **no longer gitignored**
  (PR #85) — the local backlog can and should be committed now; the ignore rule
  had been silently swallowing every doc written after it landed.
- **tusk** — no equivalent ignore problem; 103 docs tracked.
- **Toolchain gap, both directions.** kode-rs CI is rustc 1.97.1. A container on
  1.95.0 *fails* `cargo clippy --workspace --all-targets -- -D warnings` on
  `collapsible_match` in `kode-core/src/markdown_utils.rs` — untouched since
  `d5ab3b6` — which CI passes, because the lint narrowed. Do not "fix" a lint the
  shipping toolchain does not raise. Check `cargo clippy --version` against the
  runner's first.
- **Also noted:** `kode-rs/crates/kode-tui/tests/snapshots/` has the same ignore
  asymmetry `dev-docs/` had — 6 tracked while the directory is ignored, so any
  new snapshot is invisible. Different subsystem, possibly intentional.
