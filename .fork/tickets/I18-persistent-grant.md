> Idea I18, split out of `.fork/IDEAS.md` on 2026-09-05. History; read the section you came for.

# I18 — a persistent grant should cost a gesture, not a tap

**Asked 2026-08-30, and it should have been filed when it was said.** It was
raised in the same breath as the pairing work, noted as "unfiled", and then not
filed — for a whole session, while three summaries carried it as a loose end.
That is the failure this holding pen exists to prevent, so it is recorded here
as part of the entry.

## The ask, as given

> for "always allow" we might consider having the button be a slider to confirm
> the persistent approval? like the lock/power-off screen on iOS

And, separately but in the same territory: an *allow-all-for-this-task* or
*for-this-session* affordance, which the maintainer wants because answering
every prompt one at a time is the friction that makes a working day expensive.

## What already exists underneath it

- **The console's Yes is already deliberately awkward.** T14.16 built two taps
  inside a four-second window, and the reason is exactly the one behind this
  idea: an approval reached by a single tap on a phone in a pocket is not
  consent. So the fork has already accepted that the *gesture* is part of the
  security argument, and this entry asks how far that goes.
- **The agents already offer the option.** An ACP permission request carries
  `{"optionId":"always","name":"Always allow","kind":"allow_always"}` —
  recorded in `.fork/tickets/` from a real session. So *"always"* is not a thing the
  fork would be inventing; it is a thing the fork is currently **declining to
  surface**. That reframes the work: the question is not "should we build a
  persistent grant", it is "we are already hiding one, on what argument, and is
  that argument right?" **Unverified by running** whether the console suppresses
  it deliberately or simply never renders that option kind.
- `acp_permission` already refuses whole tool kinds (`other`), so there is a
  precedent for the fork declining an option the agent offered.

## The argument against, which deserves to be made first

A slide gesture is **not obviously better than two taps**, and the honest
possibility is that it is theatre. Both are "a deliberate act that a pocket
cannot perform"; sliding is merely a *longer* one. If the real risk is a
misdirected tap, two taps already answers it. If the real risk is a person
approving without reading, no gesture fixes that — a slider makes the ritual
more solemn without making the reader more informed, and solemnity that does not
inform is the exact shape of security theatre.

**So the entry's own bar is: name what a slide defends that two taps does not.**
The candidate answer, and it is a real one: a *persistent* grant is categorically
different from a single approval, and an interface that makes them feel the same
is lying about the stakes. Two taps for "yes, this call" and two taps for "yes,
everything like this, forever" is the interface saying they cost the same. They
do not. Under that framing the slider is not about accident-prevention at all —
it is about **making the two decisions feel different**, which is an argument
from honesty rather than from friction.

## Where it is in tension with the thesis

A session-scoped "allow all" is the consent architecture with the person removed
for a while, which is the thing `GOAL.md`'s charter was written against. The
fork's whole position is that a model classifier deciding permissions — the
`auto` mode `claude-agent-acp` ships with — is *not* consent, and the fork
discloses it rather than adopting it. An allow-all button is that, chosen
knowingly. Which may well be fine, and is certainly the maintainer's call, but
it needs saying out loud rather than arriving as a convenience.

## The smallest version that is still the idea

Guessing, and marked as such:

1. **Surface what the agent already offered**, unchanged in meaning — if the
   request carries `allow_always`, the console can say so, even before it can
   answer it.
2. **Scope the grant to something that ends.** "This task" and "this session"
   both end; "always" does not. The narrowest defensible version is the one with
   an expiry the person can see, and the fork already has that vocabulary — a
   pairing code says *two minutes, single use* on its face.
3. **The gesture is the last question, not the first.** Whether it is a slide or
   two taps matters far less than whether the grant has a scope and an end, and
   building the gesture first would be decorating a decision nobody has made.

## Evidence, measured 2026-08-30 — and it lands on point 2, not on the gesture

The `PermissionRequest` payload a Claude Code hook receives was captured
verbatim (`.fork/tools/dump-hook-stdin.sh`, registered beside the vendored
plugin's own hook — a hook event runs every command registered for it, so this
needed no edit to the plugin). Two of its ten keys bear directly on this entry.

**`permission_suggestions` carries Claude Code's own proposed rule additions**,
shaped:

```
{type: addRules, rules: [...], behavior: allow, destination: session}
```

That is a first-class, **session-scoped** persistent grant, with a `destination`
field naming the scope and a `rules` list naming what it would widen. So this
entry's central claim is now measured rather than argued: *the allow-all
affordance is not something the fork would be inventing — it is something already
offered and currently dropped on the floor.*

**And it answers point 2 in the vendor's own vocabulary.** "Scope the grant to
something that ends" was written as the narrowest defensible version and marked a
guess. `destination: session` is exactly that, shipped, with the scope on the
wire where a surface could render it. The fork would be *adopting* a scope rather
than choosing one.

`permission_mode` is the second key, and it belongs to T14.18's territory rather
than this one: Warp can **know** the mode a CLI agent is in rather than infer it.
`effort: {level: …}` is the third and is unexamined.

## The finding that actually decides the shape: the two paths have opposite gaps

Neither transport can today both *describe* a persistent grant and *answer* one,
and they fail at opposite ends. This is the thing to design around, and it was
invisible until both payloads had been read.

| | can Warp answer it? | can Warp say what it widens? |
|---|---|---|
| **ACP** (`opencode`) | **yes** — `allow_always` is a selectable option id | **no** — measured, it carries no declaration, so the only thing to show is the name |
| **Claude Code** (plugin hook) | **no** — the permission hook is *observational only*: it reports and exits (T14.20) | **yes** — `permission_suggestions` names the rules and the scope |

So `acp_permission`'s standing rule — *an option may only be selected by a
surface capable of showing what that option declares* — is not the fork being
cautious on the ACP path. It is the rule correctly refusing the only path that
can currently act, because that path is the one that cannot describe. And the
path that can describe cannot act.

## Re-measured 2026-09-03 — and the first re-measurement was itself wrong

**What was published first, and is retracted:** that `claude-agent-acp` "does
both, depending on the request" — declaring what an `allow_always` would widen on
some requests and not others — probed "live at 0.70.0". Both halves of that are
wrong, and the error was the one this fork keeps paying for: a version was
assumed rather than computed, exactly like the merge-base in `.fork/archive/CONSOLIDATION.md`
§1.1.

**Measured properly, and it is two versions of the agent.** Two installs sit in
`~/.npm/_npx/`:

| install | version | `allow-with-updates` in `dist/` | `permission_mode` / `policy_rule` change builder |
|---|---|---|---|
| `fca12915ff656968` | 0.70.0 | **0 hits** | **present** |
| `d820eb7d96bc2600` | 0.73.0 | 2 hits | **absent** |

**Both of those cache directories were deleted 2026-09-04, so re-run the grep
against a spec rather than a hash.** The `npx` cache keys on a hash of the
*spec string typed*, not on the version it resolves to, which is why three
directories existed for two versions and why a path like the ones above reads
as a pin and is not one. To reproduce either row:

```
npx -y @agentclientprotocol/claude-agent-acp@0.70.0 --version   # populates the cache
grep -rc "allow-with-updates" "$(dirname "$(readlink -f "$(command -v claude-agent-acp)")")/../dist"
```

Or more simply, name the version in `WARP_FORK_ACP_COMMAND` and let `npx`
fetch it. The finding does not rest on the directories surviving; it rests on
0.70.0 and 0.73.0 remaining published, and the recipe above is what this
paragraph exists to preserve.

The probe captured `optionId: "allow-with-updates"`, a string that does not exist
anywhere in 0.70.0 — so **the probe ran 0.73.0**. And the fixture in
`acp_permission_tests::as_claude_sent_it`, which carries a full
`_meta.permission.changes` declaration with `lifetime: {scope: session}`, was
transcribed from 0.70.0, whose builder for it 0.73.0 no longer has.

So each version is internally consistent: **0.70.0 declares on the option when
there is a change set; 0.73.0 never declares on any option.** Not one agent
behaving two ways.

**Why the version was ambiguous at all, and the thing to fix first.**
`WARP_FORK_ACP_COMMAND` is unpinned everywhere it is written down —
`.fork/tools/warpdev.ps1`, `CLAUDE.md`, `.fork/docs/manual.md` — as `npx -y
@agentclientprotocol/claude-agent-acp`, which resolves to whatever is newest. The
0.73.0 cache entry dates from 2026-09-02, so **every measurement taken through
that command on or after that date was 0.73.0, including run 2 and every T20
verification**, and every "0.70.0" label attached to them is unverified. Warp
also never reads `agentInfo` from `initialize`, which both versions send, so
nothing in the event log or `acp probe` records which agent actually answered.

**Two cheap things, and they are instrumentation rather than posture:** pin the
version in all three places, and log `agentInfo.version` in `session_start` and
in `acp probe`. Until then no ACP measurement in this repo is attributable to a
version, which is a property of the evidence and not of any one finding.

## The routes, and the fourth one is the fork's own precedent

1. **Render the declaration when the option carries one.** **Dead** on 0.73.0:
   there is never one to render. Alive only on 0.70.0, i.e. by pinning backwards.
2. **Make the Claude Code hook answer.** The vendor's schema supports it — the
   `PermissionRequest` hook can return `{behavior: allow|deny, updatedPermissions?}`
   — and `permission_suggestions` already names the rules and `destination:
   session`. But it is a **CLI-agent-in-a-pane** feature, and every ask the
   density complaint is about arrived over **ACP**. It does not touch the
   measured cost. It is remote consent for the other transport, which is a real
   ticket and a different one. Recommending it as "next for I18" conflated the
   two; that was wrong.
3. **Grant it in Warp.** Narrower than it sounds and safer than the entry
   implies. Exactness needs `kind` + `locations`, which `edit`/`read` carry and
   `execute` largely does not — so it can only ever cover the minority of asks.
   Its real advantage is one the entry undersells: Warp stays in the loop per
   call, so every auto-answer can be logged `answered_by: grant:<id>` and the
   disguised-zero hazard (T14.18 — no permission lines meaning *Warp was not
   asked*) never arises. Any **agent-side** grant makes asks vanish from Warp's
   view entirely, which is the thing this fork exists to prevent.
4. **The agent's own config — which this fork has already done once, under the
   freeze, with the maintainer's explicit approval.** **Note on `opencode`,
   corrected 2026-09-03 by the maintainer:** it is not a tool they use and never
   has been — it exists on this machine solely because this fork's testing
   introduced it. So `opencode.json` in this repo is test scaffolding, and the
   long permission argument `CLAUDE.md` builds on it is reasoning about a config
   for an agent nobody runs. It is still a valid worked example of the *shape*;
   it is not evidence about the maintainer's actual exposure. `opencode.json`'s bash
   allowlist is exactly this move for the other agent. For `claude-agent-acp`,
   `settingSources: ["user", "project", "local"]` (verified in both versions'
   `dist/`) means this repo's `.claude/settings.json` — currently `{}` — is the
   same lever. **Measured 2026-09-03: true here, and not everywhere.** A
   `Bash(rustup:*)` rule in this repo's `.claude/settings.local.json` took
   effect at once (0 requests); the same rule in a fresh scratch directory's
   `.claude/settings.json` did not, with or without `git init`, across six
   probes. The mechanism was not established — workspace trust is the
   candidate — so a project-level rule is a lever in a directory Claude Code
   already knows and not in one it has never seen, which reading `dist/`
   could not have found. And the fifth route's measurement is in
   (`.fork/runs/classifier/README.md`): the seven in-project `cargo` asks in run 2
   were `Bash(cargo:*)` defeated by the `CARGO_BUILD_JOBS=8` prefix, and the
   seven edit asks are what `acceptEdits` answers, measured scoped to the cwd. And for edits specifically there is a **zero-code** variant:
   `WARP_FORK_ACP_MODE=acceptEdits`, an existing mode the agent describes itself
   as *"Automatically accept all file edits"*, which Warp already discloses in
   the panel every turn and re-sends on every turn. That *is* "allow all edits
   during this session", with the disclosure already built and nothing new to
   trust.

**And route 4 has an audit attached to it that should happen regardless.**
`~/.claude/settings.json` on this machine carries **87** allow rules, including
`Bash(cat:*)`, `Bash(grep:*)`, `Bash(find:*)`, `Bash(wc:*)` and `Bash(ls:*)`
(read 2026-09-03). Those load into every ACP session through `settingSources`.
So `CLAUDE.md`'s carefully argued refusal to allow `find*`/`cat*`/`grep*` in
`opencode.json` — on the reasoning that `cat` and `grep` disclose file *contents*
at arbitrary paths, and `find -exec` is an arbitrary-command allow wearing a
read-only name — **is already moot for the recommended agent**, via a file nobody
audited and which is not in the repository. That is not an argument for or
against a grant; it is a statement that the fork's permission posture is not
currently what its own documentation describes, and nothing can be concluded
about density until it is.

## The audit of `~/.claude/settings.json`, done 2026-09-03

Read, counted and confirmed by running. **It is not in the repository, it governs
every ACP session, and it is wider than anything this fork's own documents
describe.**

`permissions`: **87 allow**, 30 deny, 0 ask, and `defaultMode: auto`. The allow
list is 71 `Bash`, 12 `Read`, 4 `WebFetch`. **22 of the bash rules are
whole-command wildcards:**

```
cargo  cat  cd  cp  echo  find  go  grep  head  ls  make  mkdir
mv  node  ps  python  python3  tail  touch  uname  wc  which
```

**Confirmed live, not inferred.** `settingSources: ["user", "project", "local"]`
is in both agent versions' `dist/`, and two probes settle it: one running
`wc -l CLAUDE.md` and a file read raised **0** permission requests; one asking
for a file *write* raised **1**. `wc` and `Read` are on the list; `Write` is not.

**Three things follow, in order of how much they matter.**

1. **`python:*`, `python3:*` and `node:*` are arbitrary code execution**, and
   they make the 30-rule deny list mostly decorative. Those rules match command
   prefixes — `sudo:*`, `dd:*`, `mkfs:*`, `ssh:*` — and
   `python -c "import subprocess; subprocess.run([...])"` matches none of them
   while matching the `python` allow. `cargo:*` is the same shape (`cargo run`,
   build scripts), as is `make:*`.
2. **`cat`, `grep` and `find` are allowed here**, which is exactly the trio
   `CLAUDE.md` argues at length for refusing in `opencode.json` — `cat`/`grep`
   disclosing file contents at arbitrary paths, `find -exec` being an
   arbitrary-command allow wearing a read-only name. That reasoning is sound and
   it has been moot for the recommended agent the whole time, through a file
   outside the repo that nobody had read.
3. **`defaultMode: auto` is set at user level, and that is where the `auto` comes
   from.** Settled by reading `dist/permissions/modes.js:18-21`:
   `resolvePermissionMode(undefined)` returns **`"default"`**, so the ACP bridge
   absent that setting opens in *ask*, not in auto. `CLAUDE.md` attributes the
   opening `auto` to *the agent's own default*; for the ACP path that is wrong.

   **But the maintainer's point is the one that matters and it is not the same
   claim.** Auto is Claude Code's shipped default as a *product* — the settings
   file carrying `defaultMode: auto` is one Claude Code wrote — and it is how
   these agents are mainstream-used today. So the fork has been treating the
   near-universal configuration as an aberration to be corrected, which is a
   posture decision nobody made deliberately. The classifier behind it is
   Anthropic's, it is shipped, and "a model decided" is not self-evidently worse
   than "a person clicked yes 44 times without reading". That argument is open,
   and this file has been assuming its conclusion.

**And the incoherence is the real finding, named by the maintainer rather than
by this audit:** arbitrary code execution is allowed (`python:*`, `node:*`,
`cargo:*`) while a file *read* raises an approve-once prompt. Whatever that is,
it is not a security posture — it is two unrelated defaults meeting. A person
paying attention gets decision fatigue on the harmless half and rubber-stamps the
dangerous half, which is worse than either extreme chosen deliberately.

**Nothing here was changed.** It is the maintainer's personal file, it governs
tools far beyond this fork, and the appropriate move is a decision rather than an
edit. But the honest summary is that **the fork's permission posture is not
currently what its own documentation describes**, and no argument about approval
density can be settled until this file is either narrowed or knowingly accepted.

The narrowest useful change, if one is wanted: drop `python:*`, `python3:*`,
`node:*`, `make:*` and `cargo:*` to specific subcommands, and move `cat`, `grep`,
`find` into the project's own `.claude/settings.json` where they can be scoped to
a path.

## And a fifth route, filed separately: `.fork/docs/classifier.md`

A **local** permission classifier — one Warp runs on this machine and logs,
rather than the vendor's cloud one it cannot see. Filed as its own file
2026-09-03 because it is a posture question with an implementation behind it and
both halves need arguing.

It is **not** this entry. I18 is a persistent grant: a decision the person makes
once and Warp remembers. That is a decision Warp makes each time, on the person's
behalf, inside a boundary they set. They interact — route 3 above is close enough
that the two should be scoped together — but conflating them is how this gets
built twice.

## A gate for lifting the freeze, as a procedure rather than a feeling

0. **Instrument first.** Pin `WARP_FORK_ACP_COMMAND` to an exact version in all
   three places, and log `agentInfo.version`. Nothing measured before this is
   attributable.
1. **Audit `~/.claude/settings.json`**, since it silently governs every ACP
   session and is not in the repo.
2. **One representative coding session, event log on.** Classify each ask: what
   `kind`, inside the session directory or not, and could an agent-side rule have
   named it.
3. **Nameable asks → the project's `.claude/settings.json`**, committed, and
   calibrated by the case that must still *ask* — the discipline `CLAUDE.md`
   already prescribes for `opencode.json`, where the confirming test cannot fail
   and proves nothing. This is agent config, the move already approved once, and
   not a Warp posture change.
4. **Edits → `WARP_FORK_ACP_MODE=acceptEdits`** per session if wanted. Zero code;
   Warp discloses the mode it is in every turn already.
5. **Build route 3 only on the residue**, and only if it is still frequent and of
   a kind Warp can state exactly. Requirements if it is built: ends with the
   session, disclosed every turn the way the mode note is, every auto-answer
   logged `answered_by: grant`, and never matches a request with empty
   `locations`.
6. **Route 2 is its own ticket** — CLI-agent remote consent — and is not on this
   entry's path.

**The falsifier for the whole procedure:** if the representative session's asks
turn out to be dominated by ad-hoc `execute` calls inside the session directory
that no rule can name, then none of the above helps, and the honest choice is
between `default` and a disclosed `auto` — which is the maintainer's, and is not
a thing to build.

**Two consequences, and the second is the one to keep.**

1. A persistent grant built on the ACP path today would have to render
   `allow_always` as a bare name, which is the disclosure failure the rule
   exists to prevent. Building the gesture there first would be putting a
   solemn ritual in front of an undisclosed decision — the exact theatre this
   entry's own argument-against warned about, arrived at from the other side.
2. **"Remote consent only works on ACP" is true today and is not
   architectural.** It is what you get when the only channel to a CLI agent is
   one-directional. The Claude Code plugin is a *versioned* protocol negotiated
   through `WARP_CLI_AGENT_PROTOCOL_VERSION`, and the fork already owns the
   vendored copy. A hook that answers is a plugin-side change, not a Warp-side
   impossibility — and it is the same shape as `TR-EVENTS-B`'s missing call id,
   which is also plugin-side and also looked structural until it was read.

**Not proposed, and deliberately.** Nothing above is a recommendation to build.
`.fork/GOAL.md` freezes permission-posture changes, and a persistent grant is the
largest posture change on this board — the consent architecture with the person
removed for a while, which is what the charter was written against. This section
records that the evidence arrived and what it says. The decision is the
maintainer's and has not been made.

---

