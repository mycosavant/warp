> Ticket T15, split out of `.fork/TASKS.md` on 2026-09-05. History; read the section you came for.

## T15 — Loose ends carried, not forgotten

- [ ] **An agent echoing Warp's own `set_mode` would be reported as acting on
      its own.** Raised in review of T14.22, reasoned rather than run.
      `translate.rs` feeds *every* `CurrentModeUpdate` to `mode::changed`, which
      says "The agent changed this session's mode on its own" with no filter for
      an update that is merely the echo of the `set_mode` Warp just sent — and
      `mod.rs`'s own comment anticipates that arrival ("a `current_mode_update`
      can arrive as soon as the mode request does").

      The spec permits an agent to notify on any change, including a
      client-initiated one. For such an agent turn 1 would read "the agent
      accepted" immediately followed by "the agent changed this on its own",
      contradicting itself; and on turns 2+, where `RequestQuietly` means Warp
      says nothing, the falsely-attributed note would be the **only** mode note
      shown. That inverts the module's purpose, which is to say truthfully who
      set the policy.

      Not reachable with either measured agent — `claude-agent-acp` sends no
      such echo and `opencode` has no modes — so this is a hazard about an agent
      nobody here has run, which is exactly the population T14.22's other
      review finding also came from. Cheap fix when it fires or as hardening:
      `changed()` compares `now` against the conversation's recorded
      `known.current`, which `acknowledged` set to the requested mode moments
      earlier, and on a match stays silent or attributes it as confirmation.
      One map read that already exists.

- [x] **Re-check `ALLOW_VERIFIED_AGENTS` against a real prompt** (from T11.5).
      **Done, and this box was stale.** `CLAUDE.md` records it under the Claude
      Code plugin: with the plugin loaded, `agent approve` drove a real prompt —
      `keystroke: "enter"`, the request left the queue, the tool ran and the file
      appeared. Recorded there and never ticked here, which is the same
      box-versus-as-built drift T14.22's run found six more of.
      The named unverified input: the permission prompt that proved the path was
      synthesised, so the claim that Return means yes rests on Claude Code's
      documentation rather than on this fork having watched one. The cheapest
      check is answering one real prompt with `warpctrl agent approve`.
- [x] **The discovery record that outlives the process — bisected 2026-09-10,
      and it outlives the *scan*.** A specimen was already on disk:
      `~/.warp/local-control/` held a record and broker socket written
      2026-08-28, naming a pid that had been dead for thirteen days. One
      `env -u XDG_RUNTIME_DIR warpctrl instance list` removed both immediately.
      `discovery_dir()` answers `$XDG_RUNTIME_DIR/warp/local-control` when that
      variable is set and `$HOME/.warp/local-control` when it is not, so **one
      user on one machine has two registries** and the environment a process was
      launched from decides which. The pruner does exactly what `CLAUDE.md` says,
      on every scan, in the directory it was given — and nothing had ever given
      it that one.

      The litter is not the cost. The same split makes a *running* Warp
      registered in one directory invisible to `warpctrl` in the other, which
      showed as `no_instance` for a Warp alive on screen. That message now names
      the directory it searched and the variable that decides it (`242592160`).

      **Two things this did not settle**, kept here rather than assumed away:
      whether it is the cause of T11.5's three shutdowns, which were in a scratch
      directory and were not re-run; and T13.1's guess that the split is whether
      an instance ran agents, which is neither supported nor refuted — two
      launches in one shell share an environment. `.fork/runs/discovery-2026-09-10/`.
- [ ] **`kode-engine` and Tusk's pure cores extracted as MIT/Apache crates**
      (§10 step 1). Not work in this repo, but a *dependency* of T13 and of any
      later migration, and §12 forbids the migration until it is done.

### T12 — the browser pass, and the "no browser on this machine" claim retracted

**Three as-builts below say a real browser has never loaded this page. That was
true of the WSL side and false of the machine, and the difference cost nothing
only because it was caught the same week.** `/mnt/c/Program Files` holds Firefox,
Brave and Zen; Windows reaches the WSL wide listener (`Invoke-WebRequest` →
`200`); and `C:\dev\shot.ps1` has been in the operating manual since T9. So the
item filed three times as *"one session with a phone"* was, for its most
important half, one command away the whole time.

The mistake was a scope word. "No browser on this machine" meant the Linux
userland the agent runs in, and got written as though it described the hardware.
The fork's own rule — *name the inputs you did not verify* — was followed; what
was not done was checking whether the named blocker was real.

**What a real browser proved that `node` could not, 2026-08-27.** Scratch
profiles for both browsers, so nothing touched the maintainer's own session.

| | result |
|---|---|
| Firefox, page load | renders; `<title>` correct; paired from a scanned code; badge `live` |
| Brave (Blink), page load | same, at a 430px viewport — the approval card, both buttons, full-width targets |
| **markup in agent text** | an agent asking to run `` rm -rf build/ && echo <b>not markup</b> `` drew the angle brackets **as text** in both engines |
| the two-tap `Yes` | clicked by hand by the maintainer; agent read `0d` |
| `agent.deny` from the CLI | agent read `1b` |

**The escaping row is the one that mattered.** T12.2's as-built says outright
that its `<img onerror>` payload "looks like a demonstration and is not one",
because a DOM shim with no HTML parser reports any string verbatim. A real Blink
and a real Gecko parser have now received attacker-shaped text through the same
path and rendered it inert. That claim is no longer resting on the test alone.

**And the two-tap arming was confirmed by a person, which is the only way it
could be.** The maintainer clicked `Yes` twice without being told to, describing
it as "twice (per design)" — so the armed state reads as deliberate rather than
as a button that failed the first time. No capture answers that question;
T12.2's as-built said so and was right to.

**One false alarm, recorded because the reasoning was worth more than the
result.** The first Firefox screenshot showed the approval already answered, and
the fake agent had read `0d` after the point where nothing of ours had written to
that PTY. The suspicion — that Warp itself writes a stray `\r`, which for a real
agent would be an *accidental approval* — was serious enough to stop and isolate
rather than wave off. It was the maintainer clicking `Yes`. The escalation was
still correct: an unexplained byte reaching an agent's stdin is exactly the
silent-failure class this phase exists to detect, and "probably nothing" is not
an answer to it.

**What is still unverified, and it is now a short list.** Nothing about desktop
browsers. What remains is *installation on the phone that will actually be
used* — **Firefox on Android**, with DuckDuckGo as the Chromium fallback. The
install rows in `README.md` were originally written around iOS Safari and Android
*Chrome*, neither of which is this maintainer's phone; they are corrected there
and the Firefox row is the one still open. **Closed 2026-09-07, noted
2026-09-14**: the maintainer ran the checklist on their Android phone in
Firefox, and a notification reached the lock screen with the console
backgrounded (`docs/remote-control.md`, *Unverified*).

### T13.3 — as built

**The ticket asks for a reviewer and there was none to build.** Tusk's
`ZB-REVIEW` is a *run mode*: a fresh engine run given the original prompt and
the worktree, with a forced read-only overlay and the prior transcript
deliberately withheld. Every one of those is a thing Tusk had to add, because
its runs inherit context by default.

Here a review is a **node shape, not a node kind**. `agent.spawn` starts a fresh
conversation knowing only its prompt — `parent_conversation_id` writes a link in
the parent/child index and copies nothing (`history_model.rs:583,599`), which is
why an empty prompt is refused outright. `allow_tools = ["read-only"]` already
exists and resolves to ten read tools, mapping on the fork's primary path to
`--allowedTools Read,Grep,Glob --disallowedTools Bash,Write,Edit,…`. An ordering
edge that hands nothing along already exists. **So Tusk's overlay collapses into
the spawn primitive: the no-transcript construction it had to build and test is
the only mode this fork's spawn has.**

**Three for three.** T13.1's sealed-subgraph closure became a filter, T13.2's
coverage invariant became a uniqueness check, and now Tusk's independence
overlay becomes nothing at all. The common cause each time is that this fork
writes the relationship on the thing itself rather than in a side table — and
here the thing written on itself is *context*: there is no side table of history
for a child to be accidentally handed.

**The tension with T13.2, and how it resolves.** T13.2 ruled that asking a
second model whether the first model's claim is true is a claim about a claim,
and named it a non-goal. T13.3 looks like exactly that. It is not, and the
distinguishing variable is *what the judge reads*: a reviewer denied the
transcript has no claim in front of it, so it produces a fresh, **uncorrelated**
claim about the world. The failure T13.2 named is correlated error — a judge
inheriting the builder's frame — and independence severs it.

What does **not** escape is that the reviewer's *verdict* is still model
judgement rather than falsifiable evidence. So it is stripped of authority:

> **A model's answer may narrow acceptance, never widen it.**

The review composes with assertions as AND-only — a detector wired to an exit
code, never an approver. Its unreliability is then asymmetric-safe: a false
"gaps" costs one human read, and a false "complete" leaves you exactly where
"no assertion failed" already left you. **A review can only usefully fail**, and
its gaps — unlike its verdict — are individually falsifiable by looking. That is
the same asymmetry the fork already ratified in T11.5, where `agent.deny` needs
no switch because saying no can only make less happen.

**So what shipped is a fence.** `review = true` on a node, read only by
`validate`, refusing the three edits that silently turn a reviewer into a rubber
stamp — each of which leaves a plan that still runs and a gate that still says
yes:

| refused | because |
|---|---|
| a `pass` edge into a review | a handoff appends the upstream answer to the prompt, so one `pass = "what I did"` hands the reviewer the exact claim it exists not to see |
| a review naming its own `allow_tools` | there is one right answer, and a reviewer that can write can make its own verdict true |
| a review not downstream of every working node | its input is the *working tree*, which is global, so an early review reads a workspace mid-edit and does it differently every time |

`review` joins the fingerprint, because un-marking a reviewer is one word long
and changes what its answer meant.

**One deliberate divergence from the advice taken.** The recommendation was to
refuse an allowlist *wider* than read-only; this refuses **any** `allow_tools` on
a review, and resolves it to `read-only` regardless of `[defaults]`. Offering the
choice is what invites the wrong one, and a plan whose `[defaults]` are wide was
the case a fence over the node's own field would have missed. Pinned by a test
that refuses even `allow_tools = ["read-only"]` itself.

**Gap handling composes with T13.1 with no glue, and this is the good part.** A
rejected review is not sealed, so the fix pattern is *append a node*: add the
fix downstream, add it to the review's `needs`, `--resume`. Sealed work is
reused, the fix runs, the review re-runs. That is Tusk's "gaps spill as proposed
tasks" achieved as a workflow instead of an object model — the supersede/patch
door T13.1 kept shut stays shut, nothing auto-edits the plan, and the person is
the gap-materialiser, which is Tusk's own posture too.

**Rejected:** an assertion whose command spawns an agent (wrong on scope — an
assertion gets one node's output, a review needs the plan's intent; on budget —
`ASSERT_TIMEOUT` is 120s and "an assertion is a check, not the work"; and on
authority — it would let a model verdict borrow command-grade authority). A
`warpctrl graph review` subcommand (a second runner for a one-node graph). A
plan-level `[plan] intent` field (restating intent in the review's prompt is the
same authoring work, and needs no new field). A `git` baseline in the run record
(the completion question is "does the tree satisfy the requirement", which the
tree answers alone; a baseline distinguishes "the agent did it" from "it was
already so", and for a completion gate those are the same verdict).

**Verified by running, 2026-08-27**, and the first run failed in the way that
mattered.

| | result |
|---|---|
| all three fence refusals | each refused by name, no Warp needed |
| the schema's own review node | still parses, validates, and lands last in `waves` |
| run 1 — worker claims "I migrated every file", `src/b.rs` still calls `old_api()` | **reviewer read the wrong tree** — see below |
| run 2, after the fix | reviewer named `/tmp/t133/work/src/b.rs` exactly, gate failed, node `rejected` |
| gap closed by hand, `--resume` | `fix` reused, review re-ran, `NO GAPS FOUND`, `done` — 16 s |

**The finding, and it is the reason to run things.** `agent.spawn` takes no
working directory (`AgentSpawnParams` is four fields), so a spawned child starts
in the **pane's** cwd, which has nothing to do with the directory `graph run`
was invoked from — where the assertions run. The first live review, launched
with Warp started from the repo and the plan run from `/tmp/t133/work`, read the
repo and said so: *"There is no `./src` directory in the working tree
(`/home/effatha/git/warp`)"*. **Its gate failed — for the wrong reason.** A
reviewer whose entire input is the workspace had been pointed at a different
one, and every check still went green-then-red in a way that looked like
success. `compose_prompt` now appends one line to a review node naming the
absolute workspace, and the re-run found the planted gap. Nothing but running it
would have caught this: the fence, the tests and the schema were all correct.

Not in the fingerprint, deliberately — the fingerprint is about *plan* edits and
a directory is environment, the same reason `assert = ["cargo check"]` has one
fingerprint wherever it runs.

**Named unverified inputs.** The **residual independence leak is real and is not
fixable by `validate`**: the reviewer has read tools and `plan.toml.run.json`
holds every node's answer verbatim, so it *can* read the claims if it goes
looking. Independence here is structural at spawn and only instructed at
runtime — the same residual Tusk carries, whose worktrees hold the agent's own
notes. The mitigation is the prompt line telling it not to, plus keeping the
record out of the reviewed tree; neither is enforcement. Also unverified: the
sentinel is a **protocol, not a truth check** — `grep -qx 'NO GAPS FOUND'` is
format-fragile, though it fails safe, since a mangled sentinel is a false
rejection costing one read. And nothing here has run on Windows.

**A correction this task turned up, unrelated to it.** `CLAUDE.md` and
`.fork/docs/manual.md` both said the catalog holds **109 actions**. The pins say
**114**: T11.2 added `events.subscribe`, T11.4 added `control.pair`, and T11.5
added `agent.approvals`, `agent.approve` and `agent.deny` — each updating both
count tests and neither doc. Corrected in both, and the README now separates the
catalog size from the 109 that were actually run in the enumerated live-build
campaign, because bumping that number would have silently extended a
verification claim to five actions the sweep never touched.

### T13.2 — as built

**Tusk filed this as `decide → build` with two gating questions, and the file
answers both.** *"Where does the contract attach — a task field, a Harness
Profile section, or a new `acceptance` config domain?"* and *"who produces
verdicts — the ZB-REVIEW reviewer voting per-assertion, or a separate run?"*
Those are hard because Tusk has a config surface, a database and a UI to place
it in. Here there is a TOML file, so the contract goes on the node, and §5's
*"migrate toward the file, not the schema"* did the deciding.

**The second question got the answer the ticket did not offer, and it is the
whole design.** Both of Tusk's options produce a verdict by asking a model. This
fork's answer is that **an assertion is a command**:

```toml
assert = [
  { id = "compiles",   run = "cargo check --quiet" },
  { id = "no-old-api", run = "! grep -rq old_api src/" },
]
```

The reasoning is one line long. An acceptance contract exists to be
*falsifiable*, so **the statement and the evidence are the same string.** A node
that reports "the tests pass" is making a claim, and asking a second model
whether the first model's claim is true is a claim about a claim — the exact
shape of the failure this fork was started over. `cargo check --quiet` cannot be
talked around. A model-judged assertion is therefore not a smaller version of
this, it is the degraded one, and it is a stated non-goal rather than a
deferral.

**Two spellings and one concept**, taking the shape `needs` already has:
`assert = ["cargo check --quiet"]` names itself, and the `{ id, run }` form
exists for when the command is too long to read as a label.

**The coverage invariant collapses, and this is the second time.** Zenith's
*"exactly one active owner per assertion"* has two real failure modes — an
assertion nobody owns, and one two tasks both own — because its contract lives
beside the plan and tasks *claim* entries from it. Here the assertion is written
inside the node that owns it, so it has exactly one owner by construction and
neither failure mode is expressible. What survives is that two assertions on one
node must not share an id, or a verdict could not say which one it is about, and
`validate` refuses that. T13.1 deleted the sealed-subgraph closure the same way.
**Twice now, writing a relationship on the thing itself rather than in a side
table has turned an invariant into a type.** That is worth watching for in
T13.3.

**A fifth `NodeState`, argued rather than assumed.** `Rejected` is not a second
kind of `Failed`, for exactly the reason the enum already gives for `Skipped`
not being one: a reader acts differently. *Failed* is the agent erroring and is
usually worth running again. *Rejected* is the agent finishing and an assertion
disagreeing — running it again unchanged produces the same thing, and what needs
editing is the prompt or the gate. It keeps the node's `output`, because the
claim is what you debug from, and that was the argument that settled it: the
alternative shape lost the answer at the moment it became interesting.

It composes with T13.1 without a line of glue. `Rejected` is settled, so it is
recorded; it is not `Done`, so it is not sealed, so `--resume` runs it again and
editing its assertion is not a violation — **which is exactly the workflow**:
the gate says no, you fix the gate or the prompt, you resume. Verified live.

**And the assertions are in the fingerprint**, which is the other half. Loosening
a gate on a node that already passed is the single most invalidating edit
anybody can make to a plan, and it was the one edit T13.1's guard would otherwise
have been blind to.

**Three threads per assertion, and that is not belt-and-braces.** A child that
never reads stdin blocks *us* once the pipe fills; a child that writes more than
a pipeful blocks *itself* while we poll for its exit. Either is a hang, a real
agent answer is bigger than a pipe buffer, and a `cargo check` on a broken tree
emits far more than one. Both are pinned by tests that pass a megabyte through.
`ASSERT_TIMEOUT` is fixed at 120s and deliberately not a knob — an assertion is a
*check*, not the work, and a plan needing longer has put the work in the wrong
place.

**Verified by running, 2026-08-27.** Release build, WSLg, scratch XDG,
`WARP_FORK_LOCAL_AGENT=1`, a three-node plan against a real `claude`.

| | result |
|---|---|
| `graph check` on a plan asserting the same id twice | refused by name, no Warp needed |
| the schema, which now contains assertions | still parses and validates as a plan |
| run 1 | `hello` **done** (2 gates ok), `strict` **rejected**, `after` **skipped** |
| the record | per-assertion verdicts under both nodes, exit code and the failing gate's first line of stderr |
| a passing verdict in the file | no `detail` key at all — an empty one is omitted |
| `graph check` after the rejection | `1 sealed` — only `hello`; work that did not hold up is not evidence |
| editing the rejected node's gate | **no violation** — that is the fix, not a reach-back |
| `--resume` after that edit | `hello` reused, `strict` re-ran and passed, `after` finally ran (8 s) |
| loosening `hello`'s gate to `true` | **refused** — the assertion is in the fingerprint |
| `--output-format json` | verdicts on the live event *and* in the summary; exit 1 |

**One thing running found that no test would have.** The per-assertion lines are
printed as they happen — minutes earlier, interleaved with every other node — and
the block after `---` is the part anyone actually reads. In the first live run
that block said `strict: rejected — an assertion says otherwise, and it said: ok`,
which names the *agent's answer* and not the gate: the reader learns only that
something disagreed, which is the fact they already had. The summary now prints
each failing verdict under its node, and the state line no longer quotes the
output. Rebuilt and re-run to confirm.

**Named unverified inputs.** **Windows is the big one**: `shell()` picks
`cmd /C` there and no `cmd` has ever run one of these. The command-running tests
are `#[cfg(unix)]` on purpose — pinning `cmd` spellings from a machine that
cannot execute them would assert a guess, which is the failure mode
`.fork/tickets/` marks its own claims for. Also unverified: any assertion that
is slow enough to meet `ASSERT_TIMEOUT` (the live gates were `grep` and `exit
4`), and the `code: None` path, which only a killed or unstartable command
reaches — that one is held by a unit test.

**The obvious next thing, deliberately not built.** Assertions are commands, so
re-running them costs nothing, and a `graph check --verify` that re-ran every
sealed node's gates would catch a pass that has since gone stale. It is not here
because `reusable` carries verdicts rather than re-deriving them, on the
principle that **the record is a record of the run** — a verdict is part of what
happened, not a live probe, the same as the node's answer. The command that asks
the world again is `graph run` with no `--resume`. If that principle turns out
to be wrong, `--verify` is where it gets fixed, and it is a small addition.

**A fifth sighting for T15's leaked discovery record, and the hypothesis held.**
T13.1 noticed that of two cleanly-closed instances only one leaked, and guessed
the difference was whether the instance had run agents. This session ran the
same shape and got the same split: the onboarding-only launch cleaned up, the
launch that spawned agents left both `inst_….json` and `….broker.sock` behind
with no process alive. Two for two is still not a cause, but it is now a
bisection with a direction.

### T13.1 — as built

**The gate check came back empty for once, and the ticket was still mostly
wrong.** `seal|subgraph|verdict|precondition` across
`crates/warp_cli/src/local_control/` returns nothing, so unusually for this
board the answer to "is it already built" was no. What the scoping found
instead was one level up: **the fork had the plan substrate and nothing for the
guard to guard.**

Tusk's design note refuses to ship this in exactly that state — *"With no
persistent DAG, every function in §4 would operate on an empty or one-node graph
— the guard would guard nothing. Per the honest-knob rule, we record the design
and stop rather than ship inert machinery."* Its §6 precondition is a persistent
plan with `depends_on` edges, and this fork has had one since T7.1. But its
§7 trigger is subtler than the precondition, and reading `graph.rs` is what
surfaced it: `run` seeds every node `Pending` (`graph.rs:413`), prints, and
exits. **Nothing is written down, so a second run re-runs the whole plan, so an
edit between runs invalidates nothing.** The seal in Tusk is load-bearing
because the run *continues* from it; here there was nothing to continue from.

So T13.1 is Tusk's §7 trigger, then Tusk's §2.1 guard:

| | what it is |
|---|---|
| **record** | `graph run` writes `plan.toml.run.json` — every settled node, plus a SHA-256 of the node *as it ran*. `--record` moves it, `--no-record` suppresses it. |
| **resume** | `--resume` seeds finished nodes from that record instead of spawning them. Failed and skipped nodes run again. |
| **guard** | `graph check` picks the record up and refuses a plan the record no longer fits. |

**The sealed subgraph collapses here, and saying why is the interesting part.**
Tusk has two node kinds — a *gate* clears while the work upstream of it sits in
any state — so its seal has to be the transitive upstream closure of every
cleared gate. This fork has one kind, and `ready` refuses to start a node until
every edge is `Done`, so **a finished node's ancestors are finished by
construction** and the closure is the set itself. `sealed()` is therefore a
filter, not a walk, and it says so in its own doc comment. The closure still
exists — it moved into `violations`, where the plan may have *grown* an ancestor
since the record was written, which is the one case the collapse does not cover.

**Two rules, and a third that turned out to be unnecessary.** Both are stated
relative to what a resume would reuse, which is what makes them checkable:

1. **edited** — a finished node's own definition changed, so the answer on file
   was produced by a different prompt, allowlist, name or set of edges;
2. **reached back** — a finished node now waits on something that never ran, so
   a resume would run the new node and then skip the one meant to consume it.

The third rule anyone would write — *a finished node was deleted* — is not
there, because deleting one rewrites the `needs` of everything downstream and
that is rule 1 on those nodes. A test pins the claim rather than the comment
asserting it.

**One thing the tests got wrong before the code did.** The first expectations
had a node inserted upstream producing both an `edited` and a `reached back`
entry for the same node. That is accurate and it is one edit counted twice: an
un-run node among a node's *own* `needs` can only have arrived by an edit, so a
direct reach-back always implies a fingerprint change. Suppressed, with the
reasoning inline — the reach-backs worth printing are the ones on nodes nobody
touched, which is exactly how the guard earns its keep.

**The advice the guard is really giving:** *edit the failure, not the evidence.*
A failed or skipped node is not sealed, is yours to rewrite freely, and is the
whole reason you came back to the plan. That is a test name, not a slogan.

**Verified by running, 2026-08-27.** Release build, WSLg, scratch
`XDG_CONFIG_HOME`/`XDG_STATE_HOME`/`XDG_RUNTIME_DIR`, `WARP_FORK_LOCAL_AGENT=1`,
a two-node plan against a real `claude`.

| | result |
|---|---|
| `graph check` with no record | unchanged output, exit 0 — the pre-T13.1 behaviour is intact |
| `--against` a file that is not there | refused; a missing *sibling* is not an error |
| a record claiming to be something else, and a version 99 record | both refused by name |
| run 1, `--timeout 1` | `hello` failed, `after` skipped; record written holding both |
| `check` against that record | `0 sealed` — a failure is not evidence |
| run 2, `--resume` | nothing to reuse, both nodes ran, both `done` |
| run 3, `--resume` | `hello: reused`, `after: reused`, **5 ms, zero new conversations** |
| edit `hello`'s prompt, `check` | refused, naming `after` as having been handed its answer |
| the same edit, `run --resume` | refused before contacting Warp at all |
| the same edit, `run` with no `--resume` | **ran, ungated** — a run that reuses nothing can invalidate nothing |
| `--no-record` | the earlier record's fingerprint untouched |
| insert `lint` in front of both | `hello` edited, **`after` reached back through `hello` to `lint`** |
| append `report` after both, `--resume` | two reused, one spawned, 4 s |

**A gap the first run found, which no unit test would have.** `agent.spawn`
needs an existing conversation to parent to, so `graph run` against a pane whose
agent has never been prompted fails *every node* with *"the targeted pane has no
agent conversation to parent a child to"*. The plan is fine; the pane is not.
`--parent` or one `agent.prompt` first. Not new in T13.1 — T7.1 has always
worked this way — but it is the first thing a person hits and it was written
down nowhere.

**Named unverified inputs.** The guard has never been run against a plan large
enough for the *nearest-ancestor* reporting to matter — the live plan was three
nodes, and the chain case is held by a unit test only. And nothing here has been
run on Windows; the record is written with `std::fs::write` and a path built by
appending to an `OsString`, so a UNC or extension-less plan path is read, not
run.

**And a fourth sighting for T15's open item, with one new detail.** After a clean
`warpctrl window close` and no process alive, `inst_….json` and `….broker.sock`
were still in the scratch `XDG_RUNTIME_DIR`. New: **two instances were closed in
this session and only one leaked** — the first, which never got past onboarding,
cleaned up; the second, which ran agents, did not. That is a difference to bisect
against, not a cause; nothing in T13.1 touches discovery.

### T12.3 — as built

**Half the ticket was already done, and the other half turned out to be two
things the ticket did not mention.** *"`control.pair`'s QR encodes the page URL
rather than a bare token"* landed in T12.1, because a QR pointing at a `POST`-only
route was not a client. What was left was the manifest and the icon — and
scoping that surfaced two problems that make the difference between an icon that
works and an icon that is a decoration.

**1. The wide listener bound port 0, so a home-screen icon died on every
restart.** An installed app is a saved URL. The wide listener took an ephemeral
port for the same reason the loopback one does — *"the address is the part a
person chose, and the port is this instance's to pick"* — which is right until
the thing being saved is the address. `WARP_FORK_CONTROL_BIND` now takes an
optional port: `192.168.1.5:41234`, `[fd00::1]:41234`.

**This reverses a case `a_bind_wider_than_loopback_has_to_be_named_exactly`
pinned as refused.** `192.168.1.5:8080` sat in that test's ambiguous list, which
was correct while the only ambiguous thing was an address. It is not a widening:
a wildcard address is refused because it is *unanswerable* — nothing can say
which networks it covers, so the `Host` check has nothing to check against — and
a port is one number typed on purpose, compared like any other part of the
authority. Verified live: `0.0.0.0:8080` is still refused, logs
*"must name one address, not a wildcard"*, opens no wide listener and leaves
nothing on 8080.

**One case no parser can catch, asserted rather than papered over.**
`fd00::1:8080` without brackets is a *valid IPv6 address* — `fd00:0:0:0:0:0:1:8080`
— so a person who meant "fd00::1 port 8080" has typed something else that is
real. This was found by writing it into the refusal list and watching the test
fail. It still fails closed: the machine does not hold that address, the bind
fails, and loopback keeps serving. Brackets are the disambiguation and that is
what they are for.

**2. `sessionStorage` and "installable" are incompatible by definition.** T12.1
chose `sessionStorage` for the device token — per tab, so a bearer for a control
plane never touches the disk of a phone that may not be only yours — and said it
should change only for a *measured* reason rather than a guessed one. This is
that reason, and it is structural rather than a matter of taste: **a home-screen
launch is a new browsing context every cold start**, so `sessionStorage` is empty
by definition and an installed app would demand a fresh QR scan on every single
launch.

Now `localStorage`, bounded by the twelve hours the server already gives the
token, cleared on a 401, and endable from the device with `unpair` in the header
— which had to come with it, because a token that outlives the tab needs a way to
be ended other than waiting. Measured with a disk-backed store, which is the only
way to simulate a cold launch without a phone: **launch 2, with no code in the
URL and a fresh process, came up `live` and paired.**

**A third thing `unpair` needed, found by running it.** The five-second pollers
outlive an unpair, so without a guard the page replaced a working view with a red
"not paired" error every five seconds — which reads as a fault rather than as
what was just asked for. `refreshApprovals` and `refreshState` now return early
when there is no device. Checked by snapshotting six seconds after the tap.

**What installability actually buys, stated honestly because the ceiling is set
by the transport.** A service worker requires a secure context and `http://` at a
LAN address is not one — so no service worker, no install prompt, no WebAPK, no
offline. **iOS Safari's *Add to Home Screen* is the one path to a standalone
launch here**: it is a manual user action needing neither HTTPS nor a service
worker, and it takes its icon from `apple-touch-icon`, which does not render SVG.
That is why the icon is a PNG and not the smaller, diffable thing it would
otherwise be. Android Chrome gets a shortcut in browser UI.

**And pairing still does not survive a restart, which is fine.** Codes and device
tokens are in memory and die with the process, so after a Warp restart the icon
opens a page saying *"run `warpctrl pair show` … then scan the QR"*. The fixed
port is what makes that possible at all: an app that tells you what to do beats a
URL that refuses to connect.

**Verified by running, 2026-08-27.** Release build, WSLg, scratch XDG,
`WARP_FORK_CONTROL_BIND=172.22.45.116:41234`.

| | result |
|---|---|
| wide listener | bound `172.22.45.116:41234` — the port it was told |
| after `window close` and relaunch | wide still `:41234`; loopback moved `43173` → `46503` |
| the saved icon URL across that restart | `GET /` → `200` |
| `warpctrl pair show` | `http://172.22.45.116:41234/#<code>`, both times |
| `GET /manifest.webmanifest` | `200 application/manifest+json`, `start_url /`, `display standalone`, icon `/icon.png` `512x512` |
| `GET /icon.png` | `200 image/png`, decodes as 512×512 RGBA |
| CSP on every route | now also `img-src 'self'; manifest-src 'self'` — no external host under any directive |
| launch 1 (QR scanned) | `live`, paired, token on disk |
| launch 2 (**no code in URL**, new process) | `live`, paired — the case `sessionStorage` could not serve |
| tap `unpair` | token gone from disk, pairing screen shown, still clean six seconds later |
| `WARP_FORK_CONTROL_BIND=0.0.0.0:8080` | refused, logged, no wide listener, nothing on 8080 |

**Named unverified input, and T12.3 makes it the largest it has been.** Every
claim above about *what a browser does* is read, not run: there is still no
browser on this machine, so `console.js` executed under `node` with a DOM shim.
Specifically unverified — **that iOS Safari installs this manifest and shows this
icon**, that a standalone launch looks right, that the secure-context rule bites
exactly as described on the phone in question, and everything T12.2 already
listed about thumb reach and the arming window. The code does not depend on the
secure-context reasoning being right: the manifest is correct either way, and if
a service worker ever becomes possible the manifest is what would be waiting for
it. **One session with a phone closes T12.1, T12.2 and T12.3's open items at
once**, and it is now the highest-value hour available on this phase.

### T12.2 — as built

**The gate check found the ticket already written, for the third task running.**
T12.2's requirement was *"the page must learn that from the server rather than
assuming it — a button that 403s is worse than a button that is absent."*
`PairedDeviceResult.actions` already exists, and T11.4's doc comment on it reads:
*"Given so a client can present a truthful capability list rather than
discovering the boundary one refusal at a time."* Same sentence, written eight
days earlier. **Nothing was added server-side** — `console.js` already persisted
the whole pairing response, so `device.actions` was in `sessionStorage` before
anything read it.

It also cannot go stale, which is worth writing down because it looks like it
could: `pairable_actions` consults an environment variable, a process cannot
change its own, and a restart drops the in-memory pairing map and forces a fresh
scan. So the list is fixed for an instance's life.

**`Yes` takes two taps, and that is a decision rather than a default.** The first
tap arms the button, which says so, and disarms itself after four seconds. `No`
stays one tap. It is the same asymmetry T11.5 argued for the pairing allowlist —
saying no can only ever make less happen — applied one layer up, where the
failure mode is a pocket rather than an attacker. The cost is one extra tap on
the only action that can make something happen.

**Three bugs, all found by running it, and the third is the one worth reading.**

**1. "nothing is waiting on you" printed above a request that was.**
`renderApprovals` set its note only on the empty branch, so the message from the
last empty refresh survived into a render with one approval. The single sentence
this page must never get wrong, wrong.

**2. An answer's error was wiped by the refresh that followed it.** Answering
and listing shared one note element, and every answer ends in a refresh, so the
reason an answer was refused appeared for roughly one heartbeat. They now have
separate lines with separate lifetimes, because they fail differently and are
cleared by different things.

**3. Half the server's errors were unreadable, including the one that
matters.** With the first two fixed, a deliberately stale answer produced
`HTTP 400` and nothing else. `describeFailure` read `ErrorResponseEnvelope`,
which carries `error` at the top level — but a *typed action* that is accepted
and then fails answers with a `ResponseEnvelope`, which nests it under
`response`. Everything the console can say about an approval comes back in the
nested shape, so the client understood exactly the errors that did not matter
and swallowed exactly the ones that did. **The bug was inherited from T12.1 and
survived that task's verification**, because T12.1 never provoked a typed-action
failure — its only errors were auth and routing, which are the shape it did
understand.

The pattern across all three: each was visible only in a state the happy path
does not reach — a non-empty list after an empty one, an error after a success,
a refusal after a grant. `both_of_the_servers_error_shapes_are_read` and
`an_answer_carries_the_digest_of_what_was_shown` pin the two that fail silently.

**Verified by running, 2026-08-26.** Release build, WSLg, scratch XDG, wide bind,
a fake CLI agent that emits a real `permission_request` over OSC 777 and then
blocks on one raw byte of its PTY — so the byte it reads is the proof.

| | result |
|---|---|
| `WARP_FORK_REMOTE_APPROVE` unset — pairing advertises | `app.ping agent.list events.subscribe agent.approvals agent.deny` |
| …and the page draws | `No` only, plus the line naming the variable |
| tap `No` | buttons disable, agent reads **`1b`** (Escape), list returns to 0 |
| `WARP_FORK_REMOTE_APPROVE=1` — pairing advertises | the same five plus `agent.approve` |
| …and the page draws | `Yes` and `No` |
| **one** tap on `Yes` | button reads *"tap again to allow"*, **nothing sent**, agent still blocked |
| four seconds later | button disarmed itself back to `Yes` |
| two taps on `Yes` | agent reads **`0d`** (Return), list returns to 0 |
| answer a request that moved | `HTTP 400: nothing is waiting on pane …; ` `agent.approvals` ` reports the requests that exist right now` — on its own line, surviving the refresh |
| scratch state directory after all of it | no `Bearer`, no `device_token`, no `bearer_token` |
| shutdown | `warpctrl window close`, no surviving process |

**Named unverified input, unchanged from T12.1 and now larger.** There is still
no browser on this machine; `console.js` ran under `node` with a DOM shim, which
now also stands in for `addEventListener` and `disabled`. So the buttons were
*invoked*, not *tapped*: nothing here proves a 2.75rem touch target is reachable
with a thumb, that the armed state is legible at arm's length, or that a real
browser fires these handlers in this order. **The arming behaviour in particular
is a claim about human timing that no capture can check.** One session with a
phone would settle all of it, and it is the same open item T12.1 filed.

### T12.1 — as built

**The gate check came back "no", which is unusual for this board.** `Html(`,
`ServeDir`, `include_str!(*.html)` and `text/html` across `app/src`,
`crates/local_control` and `crates/http_server` return one hit, and it is a MIME
table in `artifact_download.rs`. Nothing in this fork or upstream's control plane
serves a page. `axum` was already a workspace dependency of `app`, so the route
cost nothing.

**And T11.4 had already written the ticket.** The doc comment on
`validate_endpoint_headers` said, in the commit that shipped the wide listener:
*"When a page does exist, the allowlist belongs in the same commit as the page,
with the exact origin it serves from."* So did `pair_url`'s, about the fragment
being a convention until a page enforced it. Both are discharged here. Finding
the ticket already written by the previous task is the cheapest form of the
gate check and it is worth doing deliberately.

**Two findings, both from running it.**

**1. The QR was never scannable.** `control.pair` built its URL from `PAIR_PATH`,
so a phone following it arrived at `POST /v1/pair` with a `GET` and got `405`.
Nobody had noticed because nobody had scanned one — every T11.4 verification
drove the routes with `curl`, which never follows the URL it was handed. Fixed
by pointing the QR at `CONSOLE_PATH`; the code still ends up POSTed to
`/v1/pair`, by the page, from the fragment. **This is the second time in two
tasks that "the backend works" and "a person can use it" came apart**, and both
times the gap was invisible to the tool used to verify.

**2. The first origin rule was `Origin ∈ expected_hosts`, and probing it live
showed why it should be `Origin == Host`.** With two listeners bound, the wide
one accepted `Origin: http://127.0.0.1:<loopback port>` — an address this server
had bound, so it passed the list. Nothing could exploit it: both origins are
ours, no `Access-Control-Allow-Origin` is ever sent so neither can read a reply,
and a JSON `POST` would need a preflight this server does not answer. But the
rule was then "an origin we serve" when the property wanted is "the origin that
served this page". Comparing to `Host` — already checked for membership one line
earlier — is stricter, shorter, and needs no list. The unit test carries the case
that found it.

**What the origin change actually is, stated so it is not later "fixed" into
something weaker or something wider.** No route sends any CORS response header.
A cross-origin page therefore cannot read a response no matter what this check
decides. The only thing that changed is that a **same-origin** request stopped
being collateral damage — browsers send `Origin` on same-origin `POST` too, so
before this commit the console's own `fetch` to `/v1/control` would have been
refused by the server that served it.

**Deliberately not built.** A feature gate. The page is a constant with no
secret, reachable only by whoever can already reach the listener, and it does
nothing at all without a credential. A `WARP_FORK_CONSOLE` variable would be
ceremony around a static string, and one more thing to be off when someone needs
it. What is disclosed by serving it is that this machine runs the fork — and a
port that answered `403` to everything disclosed that too.

**Verified by running, 2026-08-26.** Release build, WSLg, scratch
`XDG_CONFIG_HOME`, `WARP_FORK_CONTROL_BIND=172.22.45.116`,
`WARP_FORK_EVENT_LOG=/tmp/t121/events`.

| | result |
|---|---|
| `GET /` and `GET /console.js`, both listeners | `200`, correct content types, full policy on both |
| every security header | CSP, `X-Frame-Options: DENY`, `nosniff`, `no-referrer`, `no-store` |
| `warpctrl pair show` | URL is now `http://172.22.45.116:45319/#<code>` |
| unpaired boot | *"run `warpctrl pair show` … then scan the QR"* — and the wording was **wrong first**, saying `warpctrl control pair`, which is not a command |
| paired boot | code redeemed, device token stored, credentials minted for `agent.list` and `events.subscribe`, badge `live` |
| `/v1/state` | rendered `0` with the CLI-agent note |
| `/v1/events` | four live OSC 777 events rendered newest-first with timestamps |
| origin: none / same / other listener / `evil.example` / `https:` / `null` / other port / suffix | `401` (reached auth) / `401` / **`403`** / `403` / `403` / `403` / `403` / `403` |
| shutdown | `warpctrl window close`, no surviving process |

**How the script was run, and what that does not prove.** There is no browser on
this machine, so `console.js` was executed by `node` against the live server with
a ~90-line DOM shim (`/tmp/t121/harness.js`, not committed) providing the dozen
DOM calls the file actually makes. That exercises the real file — pairing,
credential minting, state rendering, SSE frame parsing, reconnect — against a
real Warp.

**It does not prove the escaping.** One event carried
`<img src=x onerror=alert(1)>` as its summary and the harness reported it
verbatim, which looks like a demonstration and is not one: the shim has no HTML
parser, so it could not have rendered that string any other way. The escaping
rests on `textContent` semantics, on `the_script_never_assigns_markup`, and on
`script-src 'self'` — not on that run. **Named as the unverified input: no phone
browser, and no browser at all, has loaded this page.** The cheapest check is to
open it on a phone once.

Also unproven for the same reason: that a `<meta viewport>` layout reads well on
a small screen, and that `sessionStorage` per-tab is tolerable rather than
annoying in daily use. Both want a person, not a capture.

**A method note, and it is about this session rather than about the code.** The
scratch-profile launch cost a restart because `HasCompletedOnboarding` was
written to `$XDG_CONFIG_HOME/warp-terminal/user_preferences.json`, which nothing
reads — the directory is `warp-oss`. The symptom was the documented one:
`has_workspace: false`, empty `pane list`, `tab.create requires a workspace`.

**The correction that nearly got written here was that T11.5 recorded the wrong
path. It did not.** T11.5's as-built names `$XDG_CONFIG_HOME/warp-oss/…`
correctly, and T11.1c's notes add the other half — the file is `{"prefs": {…}}`,
so a flat key is silently discarded. Both were right; the path used this session
came from memory instead of from the file. Writing that up as a doc bug would
have turned two true lines into false ones, which `.fork/archive/CONSOLIDATION.md` §11 names
as worse than leaving an error alone, because the next reader has no reason to
doubt a fresh line. **Recorded as what it is: the recipe has now cost three
sessions a restart while being correctly written down each time**, which is an
argument for it living somewhere a cold start reads — it is now in `CLAUDE.md`.

### T11.1 — the gate check (2026-08-24)

Read, not run, except where noted. **Warp already has a structured, versioned
CLI-agent event protocol. What it does not have is anywhere to keep it.**

**There are two event worlds, and only one of them survives a restart.**

| | world 1 — Warp's own agent | world 2 — third-party CLI agents |
|---|---|---|
| type | `BlocklistAIHistoryEvent`, **26** variants | `CLIAgentEventType`, 10 variants |
| transport | in-process `ctx.emit` | **OSC 777** on the PTY, sentinel-gated (OSC 9 fallback for Codex) |
| wire type | — | `warp_core::cli_agent_protocol::CLIAgentNotification` (`Serialize + Deserialize`, `skip_serializing_none`) |
| versioning | Rust enum | `v` field + `VERSIONED_PARSERS` array; `current_protocol_version()` is derived from its length, and the PTY exports `WARP_CLI_AGENT_PROTOCOL_VERSION` so plugins negotiate |
| unknown values | `#[serde(other)]` on `Harness` | `CLIAgentEventType::Unknown(String)`, and an unsupported `v` is a `report_error!` rather than a panic |
| **persisted?** | **yes** — SQLite, via `AgentConversationData` (the blob T8.3 added `settled` to) | **no** — `CLIAgentSessionsModel` is three in-memory `HashMap`s |

World 2 is the one that matters for driving other people's agents, and it is
**ephemeral**: an event is parsed from the PTY, updates a `HashMap`, paints the
UI, and is gone. Nothing survives a restart, nothing can be replayed, and
nothing can be handed to a second client that connected later — which is exactly
what an SSE subscriber is.

**The taxonomy is better than the one `TR-EVENTS` proposed building**, and in
the place that matters most here: `PermissionRequest` and `PermissionReplied`
are first-class event types. The kode-rs incident this phase exists to prevent —
a swarm running without permissions and nothing surfacing it — is a *recorded
event* in this protocol. It just is not recorded anywhere durable.

**And the projection already has a reference implementation.**
`crates/warp_tui/src/cli_agent_osc_event_publisher.rs` maps world 1 →
world 2's wire format: Warp's own headless TUI publishes `BlocklistAIHistoryEvent`
as OSC 777 notifications, so a mapping from the rich internal enum onto the
serializable one is already written and shipping.

**So T11.1 is a projection, not a taxonomy.** The smallest thing that is still
the idea: subscribe to events that are already emitted and append them to a
per-run file, reusing `CLIAgentNotification` as the line format.

**Two gaps in the wire type for use as a log**, both additive and both
backward-compatible under `skip_serializing_none`:

* **no timestamp** — an append-only log whose entries cannot be placed in time is
  most of the way to useless.
* **no per-event or per-call id** — this is `TR-EVENTS-B` exactly. `session_id`
  exists; a stable id per tool call does not, so a `tool_complete` cannot be tied
  to the `permission_request` that preceded it.

**Also checked, and recorded so it is not re-checked:**

* **`crates/warp_web_event_bus` is not a candidate surface.** It is
  `#![cfg(target_family = "wasm")]`, five variants, and emits to a host
  JavaScript app whose type lives in `warpdotdev/warp-server`. The wasm build is
  a client of Warp's server, not an observer of a local session.
* **`WB-SLEEP` was wrongly recorded as absent in `.fork/archive/CONSOLIDATION.md` §4.1.**
  `crates/prevent_sleep` exists with mac/windows/noop backends and **is** wired —
  but only in `crates/http_client`, guarding HTTP requests and streams. So the
  mechanism is present and the run-scoped use is not. The earlier "not found"
  was a malformed grep (`-i` glued to the pattern), which is the same class of
  error as §1.1: the command ran and reported honestly about the wrong question.

### T11.1 — as built

`app/src/event_log.rs`, ~200 lines, plus one call at the single choke point.
Off unless `WARP_FORK_EVENT_LOG` asks: `on` writes under `fork::state_dir()`,
anything else is taken as the directory, so a run can be pointed at a scratch
path. Policy in `fork::event_log_dir`, mechanism in the module, per the same
split as `WARP_FORK_FRAME_LOG`.

**The hook is `CLIAgentSessionsModel::update_from_event`, and it records
*before* the early return, not after.** That function drops any event arriving
for a terminal it has no session for. Logging after the drop would produce a
file containing only what succeeded, which cannot show the one that did not —
so the record carries `applied: false` instead and the event survives.

**One flat JSON object per line.** Not an envelope wrapping the event: every
reader of this file is a filter, and `jq 'select(.event=="permission_request")'`
should not have to know which fields live a level down. A test asserts no field
nests.

**Warp's clock, not the agent's.** The protocol carries no timestamp and the
agent's clock is not ours to trust; `ts` answers "when did Warp know". `seq` is
process-global rather than per-file, so ordering can be reconstructed *across*
concurrent sessions and a gap in one file is visibly an event that went to
another.

**Verified by running** (Linux/X11, 2026-08-24), by emitting the real OSC 777
form a plugin uses — `ESC]777;notify;warp://cli-agent;{json}BEL` — from a shell
inside a Warp pane, through Warp's own parser:

```
{"ts":"2026-08-25T02:24:10.367Z","seq":0,"v":1,"agent":"claude",
 "event":"permission_request","source":"rich_plugin","session_id":"probe-1",
 "cwd":"/tmp","tool_name":"Bash","tool_input_preview":"rm -rf /","applied":true}
```

Three properties were driven rather than asserted:

* **A plugin cannot choose where Warp writes.** `session_id` becomes a
  filename, and the agent supplies it. `"../../../../../../tmp/pwned"` produced
  `___________tmp_pwned.jsonl` *inside* the log directory; nothing appeared at
  `/tmp/pwned`. The unit test for this **failed first**: the sanitizer stopped
  traversal (no separator survives) but left `..` in the name, which the test
  called a traversal segment. The test's intent was right and the code was
  weaker than it, so the code changed.
* **An event from a newer plugin reads as itself.** `subagent_spawned`, a type
  this build has never heard of, is recorded under its own name rather than as
  "unknown", because `CLIAgentEventType::Unknown` round-trips its string.
* **`seq` shows the gap.** The three surviving lines are `seq` 0, 2, 3 — number
  1 is the traversal event, in the other file.

**Two bugs found on the way, both upstream, both fixed:**

* **`crates/graphql/build.rs` and `crates/warp_graphql_schema/build.rs` read a
  schema they never declared as an input.** Only `rerun-if-changed=build.rs`,
  so an upstream merge that changes the queries *and* `schema.graphql` together
  leaves a stale registration in `OUT_DIR`, and every query in the crate is
  validated against the old schema. It fails as "no field `inviteLink` on the
  GraphQL type `Team`", pointing at Rust that is correct, about a schema on disk
  that already has the field. Verified fixed by touching the schema and watching
  both crates rebuild, which before the fix they did not.
* Not a code bug but worth the same weight: **sharing `CARGO_TARGET_DIR`
  between two checkouts silently corrupts it.** T10.1's baseline was measured in
  a worktree pointed at the main target directory to save disk, and the artifacts
  left behind matched neither tree. It also retroactively weakens the
  verification that preceded it — a `cargo check --workspace --all-targets` that
  passes against a poisoned cache has proved nothing. Recorded in `CLAUDE.md`.

**Not done here, and deliberately:** world 1 (`BlocklistAIHistoryEvent`, Warp's
own agent) is not yet projected into the log. The mapping exists as a reference
in `crates/warp_tui/src/cli_agent_osc_event_publisher.rs`; wiring it is T11.1b.
**Done 2026-08-25 as T11.1b, `210b029a1`** — the as-built directly below
(noted 2026-09-14).
Also still open from the gate check: **no per-call id**, so a `tool_complete`
cannot be tied to the `permission_request` before it (`TR-EVENTS-B`). That one
needs a protocol version bump, because the id has to come from the plugin.

**Filed as decides, not builds:** ACP as the adapter contract
(`.fork/archive/CONSOLIDATION.md` §4.1 — the fork as client, kode-rs as agent), and `WB-SLEEP`
(inhibit system sleep for the duration of a run), the one `WB-` ticket this
substrate does not already answer.

### T11.1b — as built

`app/src/event_log/warp_agent.rs`, plus one call in `BlocklistAIController::new`
and two small accessors on `BlocklistAIActionModel`. `event_log.rs` became
`event_log/mod.rs`; the record shape gained a caller-supplied `Entry` so both
worlds go through one writer and produce one vocabulary.

**Two fields changed meaning, and both are deliberate.**

* **`v` is now optional, and absent means "did not come off a wire".** World 1
  has no protocol version; stamping `1` on it would claim a compatibility
  guarantee that does not exist. Verified in the live log: the world-2 line
  carries `"v":1` and the world-1 lines carry no `v` at all.
* **`call_id` is new, and it is half of `TR-EVENTS-B` delivered for free.**
  Warp's own agent has always had a stable per-action id (`AIAgentActionId`), so
  `permission_request` → `tool_start` → `tool_complete` join on it — and a
  `tool_start` sharing no id with any ask *is* an action that ran unasked. The
  hosted-agent half still needs the protocol bump, because there the id has to
  come from the plugin.

**`tool_start` is added to the vocabulary.** Not in the wire protocol, and
needed: without it an action that begins and never returns leaves no trace, and
"started and never finished" is the shape of the failure this phase is for.
`CLIAgentEventType::Unknown(String)` already round-trips names Warp does not
know, so the log was always a superset of the protocol.

**Where it departs from the TUI reference, and why.**
`cli_agent_osc_event_publisher.rs` maps the same two worlds already, so it is
the starting point — but it feeds a *notification* and this feeds a *log*, which
want opposite things. It reports only the selected conversation; this reports
every one, because orchestrated children are never selected and are the case the
fork cares most about. It emits `tool_complete` only for `AskUserQuestion`,
because the rest would be noise; noise is what a log is for.

#### Three things running it corrected

**1. A bug found by reading, before the run: `Changed` does not mean changed.**
`AIConversation::update_status_with_error` emits unconditionally, and
`update_conversation_in_progress_status` calls it as *every action starts*. The
first draft mapped `InProgress → InProgress` to `prompt_submit`, so a single busy
turn would have written a long run of lines claiming a person was typing
throughout a turn nobody was watching. Now guarded on `prev != new`, which also
kills a duplicate `stop` for one ending, and pinned by a test.

**2. An assumption in a comment that was simply false.** The draft excluded
`session_start` from the query lookup, with a confident comment that it "fires
before anything has been asked". Running it showed the first turn producing
`session_start` and `stop` and **no `prompt_submit` at all** — because
`AIConversation::new` starts at `InProgress`, so the first request's status write
is the no-op above. The fix was to stop asserting and measure: the query *is*
available at `session_start`. It now carries it, five seconds before the turn
ended rather than after:

```
{"ts":"…:36:14.864Z","seq":0,"agent":"warp","event":"session_start","source":"in_process",
 "session_id":"af35bf30-…","cwd":"/home/effatha","project":"effatha",
 "summary":"Reply with exactly the word: marker-one","applied":true}
{"ts":"…:36:19.853Z","seq":1,"agent":"warp","event":"stop", … same summary … }
```

So turn 1 is marked by `session_start` and every later turn by `prompt_submit`,
both carrying what was asked. A second turn in the same conversation was driven
to confirm the `prompt_submit` path independently.

**3. The claim that the two worlds meet on the primary path was wrong, twice
over.** A doc comment asserted that a local-agent turn's tool detail arrives as
world 2 "into the same file, under the same session". Neither half survives:

* Warp's action model sees nothing, because `translate.rs` turns a `tool_use`
  block into *text* rather than a `ToolCall` — deliberately, since a ToolCall is
  an instruction and Warp would run the command a second time.
* The plugin's OSC 777 does not arrive either. `local_agent` spawns `claude`
  with `Stdio::piped()` and reads its JSON directly, so there is no Warp PTY and
  nothing reaches the terminal parser world 2 hangs off.

**Hence T11.1c, which is a real gap and not a design.** On the fork's primary
agent path the log carries the turn frame and no tools. The stream that has them
is already being parsed — `ContentBlock::ToolUse` in `translate.rs` — but filing
those lines under the *run* needs Warp's `AIConversationId`, and `RequestParams`
carries only Claude's session token. That plumbing is the whole of T11.1c. It
was scoped rather than half-built.

#### Verified by running (Linux/X11)

Driven with `warpctrl agent prompt` against a scratch `XDG_CONFIG_HOME`, with
`WARP_FORK_LOCAL_AGENT=1` so the turn is answered by the `claude` CLI. Both
worlds landed in one directory, on one vocabulary, sharing one process-global
`seq`:

```
in_process   warp   session_start                  Reply with exactly the word: marker-one
in_process   warp   stop                           Reply with exactly the word: marker-one
in_process   warp   prompt_submit                  Now reply with exactly: ping
in_process   warp   stop                           Now reply with exactly: ping
in_process   warp   stop_failure                   (error_type: error)
rich_plugin  claude permission_request  Bash       rm -rf /
```

**The action half was not driven, and is held by unit tests instead.** It is
reached only by Warp's own server-backed agent, which this fork has no account
for — the first attempt errored with `missing authentication credentials`, which
is itself the honest demonstration. The decision table is split into a pure
`action_event` for exactly that reason: a test is the only thing that can hold
it. Said plainly rather than implied.

**Two notes for whoever runs this next.** A scratch `XDG_CONFIG_HOME` means
first-run onboarding, and the window sits on "Welcome to Warp" with
`has_workspace: false` until it is dismissed — `warpctrl` answers
`missing_target` and it looks like a control-plane fault. Seed
`HasCompletedOnboarding` = `"true"` in
`$XDG_CONFIG_HOME/warp-oss/user_preferences.json` instead. And on WSLg a
root-window screenshot is **black**: capture the window id from
`xwininfo -root -children` and use `import -window <id>`.

### T11.1c — as built

`app/src/event_log/local_agent.rs` plus tool-event accumulation in
`translate.rs`, and **one field of plumbing**: `RequestParams` now carries
`conversation_id`. Upstream had it in hand the whole time — `RequestParams::new`
is handed the entire `ConversationData` and keeps only what the *server* needs,
and the server does not need this one, because it knows a conversation by its
token. On the local path that token is Claude's session id, so filing tool
events under it would have put a turn's tools in a different file from its
frame. Three lines, and the reason T11.1b scoped this rather than half-building
it.

**A third `source`, not a third vocabulary.** `local_agent` sits next to
`in_process` and `rich_plugin`; `agent` is `claude` and `event` is `tool_start`
/ `tool_complete` exactly as everywhere else, so the queries written for the
other two work unchanged. `v` is absent, which now means what it always claimed
to mean: this did not cross the OSC 777 wire.

**The translator accumulates, the caller writes.** `translate.rs` opens with a
promise — "no process, no clock, no network" — and calling the log from it would
have cost that. So it pushes a `ToolEvent` per `tool_use` and per `tool_result`
and `run()`'s stream drains them. A `HashMap<call_id, name>` in the translator
lets a `tool_complete` say *what* finished; it is not decoration, because Claude
issues parallel tool calls and the starts interleave with the completions
(measured: `seq` 11, 12, 13, 14 on one turn — two starts, then two completions
in order). A single "last tool" slot would have mis-attributed every one of them.

**`input_preview` is untyped on purpose, and this is the interesting bug that
was avoided rather than fixed.** `input` is whatever a tool's schema declares,
so a typed `command: Option<String>` would fail to deserialize the moment an MCP
server declared `command` as an array — and because `on_line` drops any line it
cannot parse, that failure would have taken **the whole assistant message**, not
just the preview. The answer to the user would have vanished to log a field
nobody asked for. Pinned by
`an_input_of_an_unexpected_shape_costs_the_preview_and_not_the_message`.

Which two keys? `command` and `file_path`, matching `warp_agent`'s rule exactly:
the field is grepped for what *ran*, and widening it to every argument makes
that grep unreliable across sources as well as putting more of a tool's payload
— which is where its secrets are — on disk. `excerpt`, `project_name` and
`MAX_TEXT_LEN` moved up to `event_log/mod.rs` for the same reason: three
adapters truncating at three lengths would defeat the comparison the field
exists for.

#### `parent_call_id`, found by running it

Not planned. The first `Task` turn driven through the finished feature produced
this, which *looks* complete:

```
21 local_agent tool_start    Agent toolu_01K1rS…
22 local_agent tool_start    Read  toolu_01Nhnd…
23 local_agent tool_complete Read  toolu_01Nhnd…
24 local_agent tool_complete Agent toolu_01K1rS…
```

The nesting is there, and it is *entirely an inference from interleaving* — the
subagent's `Read` happens to fall between its parent's two lines. Claude's
stream carries `parent_tool_use_id` on the event and it was being thrown away.

Driving two subagents concurrently showed why the inference is not good enough:

```
1 tool_start    Agent toolu_013dvFLd  parent -
2 tool_start    Agent toolu_0152hYmB  parent -
3 tool_start    Read  toolu_015hja3e  parent toolu_013dvFLd  …/f1.txt
4 tool_complete Read  toolu_015hja3e  parent toolu_013dvFLd
5 tool_start    Read  toolu_011UBWMh  parent toolu_0152hYmB  …/f2.txt
6 tool_complete Read  toolu_011UBWMh  parent toolu_0152hYmB
7 tool_complete Agent toolu_0152hYmB  parent -
8 tool_complete Agent toolu_013dvFLd  parent -
```

Both parents are open across the whole middle, and they finish in the **opposite
order** they started. Position says nothing; the field says everything. Fan-out
is the case this fork most wants to watch, so the field is recorded rather than
inferred — and `local_agent` is the only one of the three sources that can, since
world 1 has no nesting and world 2's protocol has no such field.

**What it deliberately does not claim.** There is no `permission_request` on this
path. Claude in `--print` mode does not report one: a refused tool comes back as
an ordinary `tool_result` with `is_error`, indistinguishable on the wire from a
tool that ran and failed. Both are `tool_complete` with `error_type: "error"`,
which is what the stream said. Driven both ways — a successful `Read`, and a
`Read` of a file that does not exist.

Also measured, and worth writing down because it would otherwise be guessed
wrong: **`is_error` is absent on some successful results and `false` on others,
from the same CLI build.** `#[serde(default)]` there is required, not defensive.
Every fixture in `translate_tests.rs` is a captured line for this reason; the
`tool_use` block also carries a `caller` object that no remembered version of the
shape had.

**Verified by running** (Linux/X11, 2026-08-25/26), with `WARP_FORK_LOCAL_AGENT=1`
against a scratch `XDG_CONFIG_HOME` — and this time a scratch `XDG_RUNTIME_DIR`
too, which the T11.1b recipe omitted, so those test instances published into the
*shared* discovery registry. Four turns: one tool, one failing tool, three
parallel tools in a follow-up turn, and two concurrent subagents. Frame and
tools landed in one file under one conversation id, and the same lines arrived
over T11.2's SSE stream (`warpctrl events tail`) with no extra work, since the
fan-out is downstream of `record`.

Both new assertions were mutation-tested: replacing the `call_id` → name lookup
with `None` fails exactly `a_tool_call_and_its_result_are_recorded_as_a_matched_pair`,
and dropping the parent threading fails exactly
`a_subagents_tools_name_the_call_that_spawned_them`.

**Still open, and now the only gap on this path:** a subagent's *turn* has no
frame of its own — no `session_start`/`stop` per child — because Claude does not
emit one and Warp does not know a child exists. `parent_call_id` gives
containment, not lifetime.

### T11.5 — as built

Three catalog actions (111 → **114**), one new terminal-view seam, one env var,
and two things the ticket got wrong that only running found.

#### The gate check: the seam exists, and it is unreachable

`BlocklistAIActionModel` has exactly the API `GB-APPROVE` describes:
`execute_action(action_id, conversation_id, ctx)` accepts and
`cancel_action_with_id(…, ManuallyCancelled, ctx)` rejects, and
`crates/warp_tui/src/tui_permission_prompt.rs` already drives both from a
yes/no selector. `ConversationStatus::Blocked { blocked_action }` already
exists, `agent.list` already reports it, and T11.1b already projects
`ActionBlockedOnUserConfirmation` into the event log.

**And none of it can happen on this fork.** The agent panel is served by
`ai::local_agent`, whose own module docs say it: *"Claude runs its own tools.
Tool activity is reported to Warp as text, never as a `ToolCall` message — a
ToolCall is an instruction, and Warp's action model would execute a tool Claude
had already run."* No `ToolCall` → no queued action → no confirmation → the
whole path is dead code on the fork's primary agent. Upstream's other producer
of `ToolCall`s is the account-backed server this fork removes.

So the branch was **not** built. Wiring it would have produced the exact
artefact the maintainer lost a month to: a feature that exists, is tested, is
documented, and is never reached. This is recorded rather than silently skipped
because the next person will find `execute_action` and assume it was an
oversight.

#### The finding: `warpctrl` could not see the thing that blocks

A `claude` running in a Warp pane is not an `AIConversation`. It is tracked in
`CLIAgentSessionsModel` — a different map, keyed by terminal view, holding
`status`, `tool_name`, `tool_input_preview`, `summary`, `cwd`. Nothing in
`warpctrl` read it. Measured on a live instance with a blocked agent in a
visible pane:

```
$ warpctrl agent list       → "conversations": []
$ warpctrl agent approvals  → the blocked claude, with the command it wants to run
```

That is the T11 failure mode by construction — an agent blocked on a permission
nobody surfaced — and it survived T11.2 *and* T11.4 because both of them
answered `agent.list`. The live event stream did carry the
`permission_request`, so a phone already watching saw it arrive; a phone that
connected afterwards primed itself from `/v1/state` and saw nothing waiting.

#### The second finding: approval is a keystroke

There is no way to tell a CLI agent "approved". It drew a prompt on its own
terminal and reads its own stdin. So the write half is
`TerminalView::press_key_for_local_control`, writing `\r` for approve and
`\x1b` for deny through `write_user_bytes_to_pty` — chosen over `write_to_pty`
for the check it carries: a block under Warp's own agent control returns
`false`, which is reported as a failure instead of a success nobody can verify.
The result reports `"keystroke": "enter"` / `"escape"` and never claims the
agent acted; confirming that is a second `agent.approvals`.

Consequences, both of which are the shape of the feature rather than caveats:

- **`agent.approve` is refused for agents whose prompt was not watched.** Return
  takes the *highlighted* option, which is a fact about someone else's TUI.
  `ALLOW_VERIFIED_AGENTS` is one entry and the refusal names the agent.
  `agent.deny` is allowed for all of them: Escape's worst case is that nothing
  happens, and the caller can see that nothing happened.
- **Two actions, not one with a `decision` field.** A paired device is granted a
  *list of actions*, so a field would have put yes and no behind one grant. The
  split is what makes the pairing line expressible at all.

#### The bug the digest did not catch until it was run

Every approval carries a SHA-256 over what was displayed, length-prefixed
per field, and both answering actions require it back — so an answer that
arrives after the agent moved on is refused rather than misapplied.

The first live run found the digest hashing a field that goes stale underneath
it. `question_asked` sets `Blocked` **without** calling
`clear_permission_scoped_state`, which only runs on `tool_complete`,
`permission_replied`, `prompt_submit` and `stop`. Observed:

```
after permission_request  → permission | Wants to run Bash: rm -rf build/ | rm -rf build/ | 97b73f56…
after question_asked      → permission | Wants to run Bash: rm -rf build/ | rm -rf build/ | 97b73f56…
```

The agent was asking "which database should I use?" and a remote yes taken from
the first screen would have been **accepted**, because the digest had not moved.
Fixed by reading the summary from `Blocked { message }` — set by whatever caused
the *current* wait — and reporting the retained tool fields only when they agree
with it. After:

```
after question_asked      → question | Which database should I use? | (no tool) | b1d4b8d3…
```

Pinned by `a_question_after_a_permission_does_not_inherit_the_command`, and its
mirror `a_permission_with_no_summary_still_reports_its_command`, because the
check compares `Option`s rather than testing for presence: a permission request
may genuinely carry no summary, and `None == None` has to stay a match.

#### The pairing argument T11.4 asked for, and it came out asymmetric

T11.4's as-built required any widening of `PAIRABLE_ACTIONS` to arrive with an
argument. The write half was expected to be one decision; it is two.

| | pairable | argument |
|---|---|---|
| `agent.approvals` | yes | a read reporting strictly less than `events.subscribe` already streams live — withholding the snapshot while granting the stream only punishes a client that connected late |
| `agent.deny` | yes | monotone: Escape on an agent already waiting, so the most a stolen device token achieves is that something proposed does not happen |
| `agent.approve` | `WARP_FORK_REMOTE_APPROVE` only | a yes to whatever the agent thought of, which through a permission prompt is arbitrary code. The digest binds it to the request it was shown; it does not make that request safe |

The honest position on the third is that it **cannot be made safe by
mechanism — only chosen**, so it is chosen per machine and defaults to no. Its
parser is deliberately the opposite shape to `control_bind_from`: there an
unparseable value must be refused loudly because it would otherwise silently
mean something, here anything that is not an affirmative word is simply not
consent, and the safe side and the default side coincide.

#### `GB-GRANTS`: not built, and the argument against it

"Remembered grants" means Warp pressing Return on the user's behalf when a
matching request arrives. Four reasons that is the wrong thing here, three of
them checkable without running anything:

1. **The mechanism cannot be made to key on the request.**
   `tool_input_preview` takes `command` **or** `file_path` out of the tool
   input and drops everything else (`event/v1.rs`). For `Bash` that is the whole
   command; for `Write` it is the path and *not the contents*. A grant keyed on
   "the same request" would re-approve writing entirely different bytes to the
   same file. Sound for one tool and unsound for the rest is a footgun with a
   reassuring name.
2. **It would race the repaint.** A person pressing Return has seen the prompt.
   An auto-press fires on the OSC notification, and nothing tells Warp whether
   the agent's TUI has drawn its selector yet. A stray `\r` into a prompt that
   is not there goes into the agent's *message* input.
3. **It rebuilds the failure this phase exists to detect.** The stated burn was
   a swarm of agents running without permissions and nothing surfacing it. An
   auto-approver is that by construction; logging every auto-grant is the
   mitigation, and the *value* of a grant is that you stop reading the log.
4. **The gate rule points the other way.** Claude Code already has
   `--permission-mode`, `permissions.allow` rules in its settings, and a "don't
   ask again" option in the prompt itself — all keyed on the real tool input and
   applied by the process that knows what it is about to do. A coarser
   re-implementation one layer up is strictly worse than the thing that exists.

The tempting middle path — send `2` for the prompt's own "yes, and don't ask
again" — fails the same test that refuses `approve` for Gemini: option 2 is
"don't ask again" for `Bash`, "allow all edits this session" for `Write`, and on
a two-option prompt it is *No*. Pressing a digit whose meaning varies by tool is
precisely what this task declined to do everywhere else.

**If a future task wants grants, the place to put them is Claude's own settings,
not Warp's memory** — and the useful `warpctrl` verb would be one that *shows*
what the agent has already been granted, not one that grants.

#### Verified by running it

Linux/X11, scratch `XDG_CONFIG_HOME`/`XDG_STATE_HOME`/`XDG_RUNTIME_DIR`/
`WARP_LOCAL_CONTROL_DISCOVERY_DIR`. The agent was simulated by a script emitting
the same OSC 777 `permission_request` a real plugin does, and reading one raw
byte back — chosen over a real `claude` deliberately: it records the exact byte
that reached the PTY, and it does not touch the user's `~/.claude`.

| | |
|---|---|
| a blocked CLI agent, reported with its command, cwd and session id | yes; `agent list` reported `[]` for the same instance |
| two agents in two panes, attributed and ordered stably | yes — `claude` and `gemini` side by side |
| stale digest | refused, naming the fix |
| unknown pane / nothing waiting | refused, naming `agent.approvals` |
| `deny` | pane read byte `1b` |
| `approve` | pane read byte `0d` |
| `approve` on an unwatched agent (`gemini`) | refused by name; `deny` still worked (`1b`) |
| `question_asked` after `permission_request` | tool fields dropped, digest moved (the bug above) |
| pairing offer, switch off | `app.ping, agent.list, events.subscribe, agent.approvals, agent.deny` |
| paired credential for `agent.approve`, switch off | refused, naming `WARP_FORK_REMOTE_APPROVE` |
| paired credential for `agent.prompt` / `input.submit` | refused |
| `agent.approvals` then `agent.deny` over `172.22.45.116` as a paired device | worked; pane read `1b` |
| `agent.approve` over the LAN with `WARP_FORK_REMOTE_APPROVE=1` | worked; pane read `0d` |
| device token anywhere under the scratch state dir, `warp.sqlite` included | 0 files |
| the only new log line | `local-control wide listener started at 172.22.45.116:42917` — an address, no secret |

**Inputs not verified, named rather than glossed.** The permission prompt was
*synthesised*, not produced by a real `claude`, so what is proven is that Warp's
session model, the approvals surface, the digest and the PTY write all behave —
not that a real Claude Code prompt accepts `\r` as yes. `ALLOW_VERIFIED_AGENTS`
therefore rests on Claude Code's documented prompt (option 1, *Yes*, highlighted;
Escape labelled on the reject option), not on this fork having watched one.
**That is the claim to re-check first**, and the cheapest way is to answer one
real prompt with `warpctrl agent approve` and see whether the tool runs.
`172.22.45.116` is also WSL2's NAT address, not a physical LAN, and the client
was `curl` on the same host.

**Unrelated observation, not bisected:** across three clean `warpctrl window
close` shutdowns the discovery record and broker socket were left behind in the
scratch directory, though no process survived. `CLAUDE.md` says ordinary
shutdown cleans both. Nothing in T11.5 touches discovery, so this is either
pre-existing or environmental; recorded here rather than acted on.

### T11.4 — as built

A second listener, three secrets with different lifetimes, one new catalog
action (110 → **111**), and an allowlist that is the actual security boundary.

**The gate check paid off twice, in opposite directions.** `qrcode` is already an
`app` dependency and `drive::sharing::qr_code` already encodes an arbitrary URL
to a matrix *and* a PNG — built for Warp Drive share links, generic, tested. The
QR half of this ticket needed one word (`mod` → `pub(crate) mod`) and a
half-block renderer. But the *bind* half had no gate at all: `[127, 0, 0, 1], 0`
is a literal in `LocalControlServer::start`, ungated by anything. Worth recording
that the rule has a failure mode — "look for the gate first" found a whole
subsystem in one half of the task and nothing in the other, and the second half
is where the design work was.

**The ticket's four must-haves, and the one it did not list.**

| asked for | shipped |
|---|---|
| fail closed on an ambiguous config | `WARP_FORK_CONTROL_BIND` takes one literal IP; a hostname, a wildcard, or a typo leaves the wide listener shut |
| refuse a wide bind without a strong token | the pairing state only exists when a wide listener does, and a device gets nothing without spending a 32-byte `OsRng` code |
| never log the token | the only `log::` line names the address; the code reaches a person through `warpctrl pair show` and the QR |
| CORS allowlist, never `*` | **deliberately not done** — see below |
| — | **a paired device may reach three actions**, and this is the one that mattered |

**The must-have that was missing is the one the catalog forced.** The four listed
above are all about *reaching* the server. None of them constrains what a client
does once it is in, and `ActionKind` has 111 entries including `input.insert`,
`input.submit`, `agent.prompt`, `agent.spawn`, `slash.run` and
`remote.wsl.connect`. `input.insert` followed by `input.submit` is typing a
command into a terminal and pressing return. A pairing path that could mint a
credential for any implemented action would therefore have satisfied every stated
requirement and still been remote code execution reachable by photographing a
screen — which is *precisely* the vibe-kanban failure the ticket names, arrived at
through the front door instead of the back. So `PAIRABLE_ACTIONS` is
`app.ping`, `agent.list`, `events.subscribe`, checked before `issue_credential`
is even called, and stated as an allowlist because a denylist is a promise to
remember every future catalog entry.

**Refusing the CORS allowlist, on purpose.** The requirement assumes a server
that answers browsers and must choose which. This one answers none: any request
carrying `Origin` is refused outright, which *is* the empty allowlist. Adding an
allowlist now would be a widening with nothing to widen to, since there is no
page — the pairing client is a fetch from something holding a device token, not
a document with an origin. Recorded here and in the renamed function's doc
comment so it is not later "fixed" into the weaker thing.

**Two listeners, not one moved one, and the discovery record is why.**
`InstanceRecord::validate_local_control_authority` requires `endpoint.host ==
"127.0.0.1"` and every client calls it. Moving the listener to a LAN address
would have made the instance invisible to `warpctrl` *on the machine running
it* — including `warpctrl window close`. Keeping loopback also means the wide
address is never written to the filesystem at all, so the check that stops a
record from redirecting a client somewhere else keeps its full strength rather
than being relaxed to accommodate this feature.

**`0.0.0.0` is refused, and the reason is narrower than "wildcards are
dangerous".** A wildcard is *unanswerable*: nothing can say which networks it
joined, and the server cannot name a `Host` for clients to present, so the
header check degrades exactly when it starts mattering. `expected_host: String`
became `expected_hosts: Arc<Vec<String>>` — still exact string membership over a
two-entry, server-chosen list. The obvious way to make two listeners work would
have been to compare ports and ignore the address, and
`the_host_check_accepts_the_addresses_bound_and_no_others` refuses that
explicitly.

**A refusal leaves loopback serving, which is the fail-closed reading and not a
softening of it.** The dangerous thing is the wide listener, so the closed state
is "do not open it". Refusing to start the server outright would take out
`warpctrl window close` — and this fork has already been bitten by exactly that
shape, when a `WARP_FORK_POLICY=0` instance published no discovery record and
held a window and a port that nothing could authenticate to. A mistyped
environment variable must not be able to produce an instance nothing can stop.

**Three secrets rather than one, because of where the QR ends up.** A single
long-lived bearer would have to be *in* the QR, therefore also in the scrollback,
screenshot or photograph the QR appeared in, and stay valid. Split: a pairing
code (2 minutes, single-use, the only thing displayed), a device token (12 hours,
returned once over the connection that spent the code), and ordinary 5-minute
action-scoped credentials minted through the *same* `issue_credential` a local
client uses. `warpctrl pair show` is the one command in the CLI that prints a
secret, and that is now a stated property rather than an accident.

**A bug found while writing the renderer, not by a test.** `QrMatrix::is_dark`
indexes `y * width + x` into a flat `Vec`, so an `x` past the right edge does not
read out of bounds and return `false` — it **wraps into the next row**. The quiet
zone, drawn by asking for coordinates outside the matrix, would have been a strip
of the following row's modules printed where the margin belongs, and no scanner
would have read it. Caught by reading `is_dark` before trusting it;
`the_quiet_zone_is_actually_quiet` is the assertion that now holds it, and it
checks the *right* margin specifically because that is the one that wrapped.

**Light modules are painted, not skipped.** A QR needs its light modules light,
and a terminal background is usually dark — "leave it blank" would have produced
a code that looks fine in a diff and does not scan at all.

**Verified by running** (Linux/X11, 2026-08-26), against a scratch
`XDG_CONFIG_HOME`, `XDG_STATE_HOME`, `XDG_RUNTIME_DIR` *and*
`WARP_LOCAL_CONTROL_DISCOVERY_DIR` — the user's own registry was never written
to, and `ls` confirmed it afterwards.

Three listeners, which is the shape the design predicted:

```
172.22.45.116:34983   the wide listener
127.0.0.1:41513       warpctrl's loopback listener
127.0.0.1:9282        upstream's http_server — unrelated, and still unauthenticated
```

and the discovery record on disk said `{"host": "127.0.0.1", "port": 41513}`.
**The wide address is not in it**, which is what leaves
`validate_local_control_authority` at full strength.

| checked | result |
|---|---|
| redeem the code | device token, 12h, actions `app.ping, agent.list, events.subscribe` |
| redeem the same code twice | `unauthorized_local_client: pairing code is not valid` |
| device token → `agent.list` | credential issued |
| device token → `input.submit` | `insufficient_permissions`, and the message names what *is* allowed |
| `GET /v1/state` over the LAN address | the snapshot |
| `GET /v1/events` over the LAN address | `200`, `content-type: text/event-stream`, `: keepalive` frames |
| an `events.subscribe` credential on `/v1/state` | refused — T11.2's scope split still holds on this path |
| forged `Host: evil.example:34983` | refused |
| `Origin: https://evil.example` | refused |
| `warpctrl agent list` on loopback | unchanged |

**No secret reached any log.** All three live secrets — pairing code, device
token, credential — were grepped for across every file under the scratch state
directory, including `warp-oss.log` and `warp.sqlite`: **zero hits each**. The
one line the wide bind writes is
`[INFO] local-control wide listener started at 172.22.45.116:34983`.

**Both fail-closed cases run, and the important half is what still worked.**

| | listeners bound | log | `warpctrl` |
|---|---|---|---|
| `WARP_FORK_CONTROL_BIND=0.0.0.0` | loopback only | `WARN … must name one address, not a wildcard` | `app ping` **and `window close`** both fine |
| `WARP_FORK_CONTROL_BIND=my-laptop.local` | loopback only | `WARN … must be a literal IP address` | same |

and `warpctrl pair show` answered
`local_control_disabled: this instance has no wide listener … set
WARP_FORK_CONTROL_BIND to the address to listen on and restart` — the error is
the feature's discovery path, so it names the variable rather than just refusing.

**One input not verified, and it is named rather than glossed.** `172.22.45.116`
is WSL2's NAT address on `eth0`, not an address on a physical LAN, and the client
was `curl` on the same host rather than a phone. What that exercises is every
line of the code — a non-loopback bind, the `Host` set, the pairing exchange, the
allowlist — and what it does not exercise is a packet that actually crossed a
network, or a real QR scanned by a real camera. The QR *encoding* is
`drive::sharing::qr_code`, already shipped and used by Warp Drive, so the
untested part is the terminal rendering of it, which was eyeballed and is pinned
by three tests but has not been photographed.

**Not done, and deliberately.** No web page, so nothing consumes the pairing URL
yet except by hand; the fragment convention is written down for when one exists.
No revocation beyond expiry — `warpctrl pair` has `show` and nothing else, so a
lost phone is a 12-hour window, not something you can cut short. Both belong with
the client that would use them.

### T11.3 — as built

`AuthToken` now hand-writes `PartialEq` over `subtle::ConstantTimeEq` instead of
deriving it. `subtle` was already in the lockfile transitively, so this adds an
edge rather than a build. The change is four lines; **the finding is the
entry**.

**What the ticket got wrong, found by grepping for the callers instead of
trusting the name.** T11.3 said the derived `PartialEq` made
`verify_authorization_header` a timing oracle on the request path. That function
is not on the request path — it has **no production callers**, only tests.
Control requests authenticate in `lookup_credential`, which is a `HashMap` lookup
keyed by the secret, and that is not a prefix oracle: SipHash is seeded per
process by `RandomState`, so a near-miss hashes somewhere unrelated. The premise
that made this a *prerequisite* for T11.4 does not hold, and the ticket has been
corrected in place.

**Why it still shipped.** `verify_authorization_header` is public API of
`local_control`, it is named exactly what an auth check is named, and the next
two tasks are a new read route and a bind wider than loopback — the moment
someone reaches for the function that looks like the answer. Cheap now,
load-bearing later.

**A second overclaim, caught while writing the doc comment.** The first draft
said the derived comparison "returns at the first difference". It does not:
`String == String` lowers to `bcmp`, and a `bcmp` over 43 bytes may be a single
vectorised compare with no data-dependent timing at all. The real argument is
narrower — nothing *guarantees* that, it is a property of the libc and codegen
on the day, and the failure would be silent.

**What the test pins, and what it does not.**
`a_token_equals_itself_and_nothing_else` checks that the hand-written comparison
is still equality — first-byte, last-byte, prefix and suffix mismatches all
reject. It does **not** pin constant time, and does not claim to: that needs a
statistical timing measurement, which would be flaky in exactly the way that
gets a test deleted. The likelier regression is a hand-rolled comparison
quietly disagreeing with the derived one it replaced, and that is what is held.

**Not done:** `lookup_credential` itself. A constant-time lookup would mean
iterating every credential and `ct_eq`-ing each. It is not obviously wrong to
want that before T11.4, but the `HashMap` does not leak a prefix today, so it
was left alone rather than changed on speculation.

### T11.2 — as built

Two `GET` routes on the existing `warpctrl` server, one new catalog action, and
a `tokio::sync::broadcast` fan-out in `event_log`. Catalog goes 109 → **110**;
both count pins updated, and the first one caught the omission exactly as it is
supposed to.

| | |
|---|---|
| `GET /v1/state` | the snapshot. Body is verbatim `agent.list`, and it requires an `agent.list` credential, because it *is* `agent.list` — the route exists so a browser need not compose a request envelope, not to expose anything new. |
| `GET /v1/events` | the stream. One SSE `data:` frame per event log line. |
| `events.subscribe` | the new action. Its `POST` form answers *where* the stream is and when the credential dies. |
| `warpctrl events tail` | the reader. |

**The new action is not bureaucracy, and this is the one design argument worth
keeping.** It would have been cheaper to let an `agent.list` credential open the
stream. It would also have been wrong: `agent.list` returns titles and busy
flags, while the stream carries tool names, input previews and working
directories for *every* agent in the instance. Granting the second because
someone asked for the first is precisely the scope-conflation T11.4 exists to
avoid. Both directions are now refused, and that was checked rather than
assumed — see the runs below.

**`is_enabled()` stopped being a constant.** Before this it meant
"`WARP_FORK_EVENT_LOG` named a directory", fixed for the life of the process.
A subscriber is a consumer with no file behind it, so it now means "a file **or**
a live subscriber" and can flip either way at any moment — callers must not cache
it. `seq` moved out of `Sink` and became a process-global static for the same
reason: it is documented as process-global and should not restart or vanish
depending on whether a directory was named. Subscribing is therefore enough to
turn the log on; you do not also have to set the environment variable.

**The stream ends when the grant does.** A credential is good for five minutes,
and a connection authorized once at open would outlive its own authority — the
"localhost, therefore fine" reasoning this phase is trying not to ship. Expiry
is re-checked before every frame and on a 15-second tick, so an idle stream
closes on time rather than at the next event.

**No token ever leaves the process.** `events subscribe` deliberately does not
echo the bearer, which left the stream unreadable by anything — a gap the
design created and the first run exposed. `warpctrl events tail` is the answer:
it fetches the URL, opens the stream and prints lines, keeping the secret
internal. The alternative, a `--print-token` flag, puts a credential in a shell
history, a scrollback and a `ps` listing.

#### Found by running it: a Tokio runtime with no clock

The keepalive is a `tokio::time::interval`, and the local-control runtime is
built `.enable_io()` — **no `.enable_time()`**. That combination does not fail to
compile and does not fail to build the runtime. It panics on the first timer,
*inside the connection task*, where axum swallows it and the client sees a
dropped connection with no status:

```
error: transport_unavailable: failed to open the local-control event stream
details: error sending request for url (http://127.0.0.1:43477/v1/events)
```

Every check up to that point was green: `cargo check --workspace
--all-targets`, 272 `warp_cli` tests, four new handler tests, a clean release
build. The real message was only in Warp's own log — `A Tokio 1.x context was
found, but timers are disabled`. Fixed by enabling the time driver, and worth
recording because the *shape* of it recurs: a runtime feature flag is not a
compile-time contract, and a panic inside a spawned connection task reaches the
client as a transport error rather than as a panic.

Two smaller traps for whoever runs this next. `tokio`'s `sync` and `time`
features had to be added to `app/Cargo.toml`, and the code compiled without them
because another workspace crate enables them and cargo unifies features — so the
app crate's own manifest was wrong while everything built. And the scratch
`XDG_CONFIG_HOME` recipe in the T11.1b notes is subtly incomplete: the
preferences file is `{"prefs": {...}}`, so a flat
`{"HasCompletedOnboarding":"true"}` is silently discarded and the window sits on
onboarding with `has_workspace: false`. Patch the key *inside* `prefs`.

#### Verified by running (Linux/X11)

Live app, scratch config, `WARP_FORK_LOCAL_AGENT=1`. The stream received the
turn as it happened:

```
{"ts":"...T20:54:38.733Z","seq":0,"agent":"warp","event":"session_start","source":"in_process",...}
{"ts":"...T20:54:42.527Z","seq":1,"agent":"warp","event":"stop_failure","source":"in_process",...}
```

**Byte-identical to the file log**, which is the point of broadcasting the
rendered line rather than re-serializing: a subscriber re-broadcasts bytes it
never parsed and cannot drift from the on-disk format.

The authorization boundary, driven with `curl` against credentials obtained
from the broker socket the way any client obtains them:

| request | answer |
|---|---|
| `/v1/state` + `agent.list` credential | **200**, real conversation state |
| `/v1/state`, no credential | 401 `unauthorized_local_client` |
| `/v1/state` + `events.subscribe` credential | 403 `credential for events.subscribe cannot open agent.list` |
| `/v1/events` + `agent.list` credential | 403 `credential for agent.list cannot open events.subscribe` |
| `/v1/state` + forged token | 401 `local-control credential is invalid` |
| `/v1/state` + browser `Origin` | 403 `browser-origin ... not allowed` |

Expiry was waited out rather than reasoned about: a tail opened at `20:56:47`
was still live at `20:57:58` and had closed with `credential expired; re-run to
reconnect` by `21:03:14` — the five-minute grant plus at most one 15-second
tick, and the stream does not outlive its own authority.

**Not verified by running:** the `lagged` frame. It needs a subscriber that
stops reading while 256 events go past, which no natural run produces; it is
held by construction and by the bounded channel's own contract, and that is
stated rather than implied.

---

