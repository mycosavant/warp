# Working in this fork

Personal fork of `warpdotdev/warp`. **Thesis: no telemetry, no account
requirement, agents driven by the user's own Claude subscription, API keys and
local models.** Work happens on `dev`. Fork-specific docs live in `.fork/`.

**Root docs describe upstream.** `AGENTS.md` is upstream's and is still right
about architecture, feature flags, testing and exhaustive matching — read it for
those. Where it disagrees with this file, this file wins, and the one place it
actively misleads is called out under *Formatting* below.

---

## How to write in this file

**This file is the index. Accounts go elsewhere.** A finding lands here as a
*rule*, in as few lines as the rule needs, and its account — the measurements,
the false starts, the run — goes to a `.fork/docs/` page or a `.fork/runs/`
record, cited by path. **Writing the account here is the regression**, and it is
the easy one to make, because the account is what you have just finished
learning. This rule has been in the file since 2026-09-10, at the bottom, where
it was read after the damage; it is here now for the obvious reason.

**The budget is enforced, not remembered.** Writing the rule above and then
not moving the account is exactly what happened the day after it was written
— 2026-09-11 added 49 lines of account with nobody consulting it, and the
convention section's own cost ate most of the trim that followed. A rule that
relies on being read by a session under time pressure is not a fix; a check
that fails the gate already run before every push is. `.fork/tools/claude-md-budget.sh`
runs in `script/presubmit` and checks **two** budgets, because they answer
different questions: a cloud-tier margin under Claude Code's own
150,000-character memory-file warning (currently clear), and a local-tier
number — whether a genuinely local model's fixed floor (its system prompt
plus this file) fits inside a realistic local context window *at all*. This
file currently fails the local tier by 61,362 characters (the script,
2026-09-12; this said "over 100,000", which it never printed), and no amount
of moving accounts elsewhere closes that gap while this stays a rules index
for a fork this size — `.fork/docs/model-economy.md` has the table and the
two env-var levers that exist below the file's own size. Run the script
yourself before you believe you're done: `./.fork/tools/claude-md-budget.sh`.

**Retract in place, and leave the wrong claim visible.** This file's value is
that it records what was believed and what falsified it, so a reader can tell a
claim that survived contact from one nobody has tested. Deleting a wrong
sentence destroys exactly that. The form, used throughout:

- Strike it (`~~…~~`) or write *"this line said X until DATE"*, then give the
  correction and **what falsified it** — a run, a command, a person.
- Date both halves. A measured claim is measured *as of* its date.
- Put the correction in **the commit that makes it and the doc that was wrong**,
  not only in the newer doc that happens to be right.
- A run record in `.fork/runs/` is **never rewritten**; supersede it with a
  pointer at its head and leave its body alone.

**Say what you did not establish.** A finding with no stated limit reads as
stronger than it is, and the next reader spends a day on the gap.

---

## Method: run it

**Scope and verify by running the thing, not by reading it.** This is the rule
the project is built on, and it is stated here because it keeps paying:

- **T1.7** documented the control surface by executing all 88 actions one at a
  time. Three documented facts were wrong, including the count. Only running it
  would have found any of them.
- **T5.6** chased a turn that ended `Cancelled` with "nothing" in the log.
  Reading the `CancellationReason` enum produced a confident, wrong suspect. The
  log had the answer: 205 `SelectText` actions and a `CtrlC` — a person trying
  to copy. Three claims were retracted.
- **2026-08-21** produced two more in one session: reading the running process
  gave the wrong display backend (the launch recipe matters), and remembered
  guidance gave the wrong formatting rule (see below).

When something here has only been read, say so. `.fork/tickets/I00-idea-board.md` marks its
unverified claims at the top of the file; keep that habit.

**And a scope written from one reading is a scope you will pay to correct.**
T16 has now produced three of these in two days. Its first draft said the
Windows build was unusable for WSL work — one measurement, on the worst
directory in the checkout, generalised. Its phase 2 was written as five broken
surfaces and **four of them already worked**, because they route on a
`LocalOrRemotePath` variant that phase 1 had already made `Remote`; only the
ones that asked `session_type()` were broken. Its phase 3 was written as *"49
`to_local_path_lossy` call sites"* and the hazard turned out to be **seven**,
all in the project explorer, because `Repository` models are only ever built by
`DirectoryWatcher` and are therefore local by construction. Each correction took
under an hour of running; each would have cost a day of building.

**The subtler one from the same day: measuring the wrong quantity looks exactly
like measuring.** Project rules for a remote repository were about to be
recorded as broken, on the evidence that no row appeared in the `project_rules`
SQLite table. The table is a *persistence cache*: `apply_project_rules` inserts
remote rules into the live `path_to_rules` and skips only the write. The query
ran correctly and answered a question nobody had asked. Same shape as reading
`/proc/<pid>/cwd` to learn what an *agent* sees, and as `git diff A...B` with an
assumed base — the command works, and it is one step downstream of the mistake.

**And the commonest defect in this fork is not a bug — it is a doc that outlived
its code.** Twelve found in one day (2026-08-31), every one by asking an agent
*"name anything whose doc comment claims something the code below it does not
do"*:

| where | the doc said | the code did |
|---|---|---|
| `egress.rs` | a check in `execute_inner` *"cannot be bypassed"* | `eventsource` bypassed it |
| `acp_permission.rs` | *"only by a surface capable of showing…"* | no surface parameter exists; refuses for all |
| `apply.rs` | a reassigned alias is *"named rather than counted"* | named **and** counted |
| `wsl.rs` | `SpawnFailed` is *"what a caller sees"* without WSL | most callers got `IoError` |
| `mode.rs` **and this file** | an unadvertised mode id is *"reported, not sent"* | it **refuses the turn** |
| `approvals.rs` | *"Approval is a keystroke here, and saying otherwise would be a lie"* | true of the pane path; `answer_acp` sends a typed option id and says so |
| `warp_agent.rs` | the preview holds *"the same two things and nothing else"* | three arms; the third is stdin content |
| `translate.rs` | permission requests *"never reach this file"* | T14.17 hands them here to log — **staled that same morning, by me** |
| `fork.rs` | *"default off, unlike every other predicate in this module"* | three others are off by default too, one arguing its own asymmetry |
| `console.rs` | `img-src` is denied by `default-src 'none'` | T12.3 added it as `'self'`; the same comment block says so four lines later |
| `registry.rs` | `waiting_for` returns *"everything currently waiting"* | it filters to one conversation — **one missing blank line** had glued `waiting()`'s doc onto it, leaving `waiting()` undocumented |
| `graph.rs` | the fingerprint holds *"everything the runner actually uses"* | `compose_prompt` uses the workspace and its own doc says a page later that it is excluded deliberately |
| `warp_features` (upstream) | `LSPAsATool` — *"we expose LSP as a tool to the agent"* | gates `LspRepoWatcher::ensure`/`teardown` only; no agent-facing LSP path exists and `ToolType` has no LSP variant |
| `CLAUDE.md` itself, the TUI bullet | the TUI *"is not a fork surface"*, the account-gate bypass *"simply absent there"* | `crates/warp_tui` depends on the **app** crate, and `e4f52077a` had lifted the gate eleven days earlier — the TUI opens *"Not signed in"* and a fork ACP agent answers |
| `acp_agent/mod.rs` header | *"A second turn is refused"* | true of an agent that cannot resume; T14.7 gave the others a `session/load`, and `cannot_resume`'s doc records that correction **twenty lines below** |
| the same header, same list | *"model selection … falls through untouched"* | T14.14 built the model chip 2026-09-07 — three days before this was read |

**The last two rows arrived together, in one header, in a file somebody was
already editing** (2026-09-10). Neither was noticed until the question below
was asked out loud, which is the argument for asking it: `cannot_resume`'s own
doc had carried the correction to one of them for weeks and nobody looked up.
The ordinals the rest of this passage counts stop at the thirteenth row.

**Thirteen, and the last five are the instructive ones**: none is a careless
comment. Each was written carefully, was true when written, and was falsified by a
later change to the code beneath or beside it. Four of the twelve are *internally*
inconsistent — the file contradicts itself and both halves are signed work — and
one was wrong purely by *position*, a missing blank line having attached it to the
next function. **Nothing in the toolchain can see any of this.** `cargo check`,
`cargo test`, `./script/format` and every gate in this file pass with all twelve
in place, which is why the question has to be asked out loud.

The last one is the shortest fuse in the table: the author of the stale sentence
and the author of the code that staled it were **the same person, hours apart**,
and neither noticed. Adding a function is exactly when the paragraph above it
stops being true, and exactly when nobody re-reads it.

**The thirteenth is the first one found in an *upstream* file, and it is the most
expensive shape: a doc that answers the question you came to ask, wrongly.**
Found 2026-09-02 while auditing whether LSP-backed navigation was already built
and switched off — this file's own "look for the gate first" advice, run
literally. `FeatureFlag::LSPAsATool` is exactly what that search is looking for:
the name says it, the doc says *"When enabled, we expose LSP as a tool to the
agent"*, and it is off in **both** halves (in no channel list; `lsp_as_a_tool`
declared in `app/Cargo.toml` but absent from `default`). A reader stops there and
concludes the feature exists behind a switch.

All three of its call sites are `LspRepoWatcher::ensure`/`teardown`
(`crates/lsp/src/model.rs:305,335,409`) — forwarding repo file-change events as
`workspace/didChangeWatchedFiles`. Nothing about tools, nothing about agents.
Measured the same day: no `warpctrl` action, no built-in MCP server and no
`remote_server` message touches LSP, and `ToolType` — generated from the
`warp-proto-apis` git dependency, 36 variants — has no LSP entry at all. So the
tool is not gated off, it is **absent from the protocol**, and the flag is a
statement of intent sitting on top of a watcher.

Two things follow. **A flag's name and doc are not evidence that a feature
exists** — grep its call sites before believing either, which is the same
discipline this file already demands for the two halves of a gate and for the
count behind a pinned test. And **the fork's stale-doc question is worth asking
of upstream files too**: the first twelve were all fork-authored, which quietly
suggested the defect was ours.

**The fourteenth was falsified by a merge, and its tests stayed green for
twelve days.** Found 2026-09-07. `.fork/docs/manual.md` promised since T3 that
a Custom Inference endpoint could be `http://127.0.0.1:8080` and the four small
AI features would go there. Upstream's 2026-08-26 merge added a validator that
requires https and refuses every loopback and private host, run on the modal's
Save and on every settings-file load, so from that day a local endpoint was
refused at the form and, declared in the file, invalidated every endpoint in it.
T3's 64 tests build the endpoint struct by hand and never meet the validator.
Neither author was wrong when they wrote: upstream's server dials the URL, so
its rule is right there, and the fork dials it from this process, so the manual
was right here. A merge put the two in one binary and nothing in the toolchain
could see it. The shape to remember: **a fork claim that depends on upstream
not doing something is re-tested by every merge, and only a test that goes
through upstream's door can see it fail.** Fixed the same day in
`validate_custom_endpoint_url`, with the argument beside it, and then twice
more that night, because the same merge had moved the endpoint into a
settings definition the fork's reader did not look at: the form refused the
URL, the file dropped the endpoint, the reader looked in the pre-merge
vector, and the tests at each layer were green. The first live run found the
third one in a minute (`.fork/runs/localmodel-2026-09-07/`).

The pattern is always the same: the code was corrected and the prose above it was
not, so the doc preserves a design that was considered and rejected. Two of these
were in files whose *other* comments argue the correction at length. **Ask that
question of any file you are about to trust**, and expect the answer to be about
a paragraph that used to be true.

**And a claim marked *measured* is measured as of its date, not measured now —
re-run it before building on it.** This is the subtler failure, because "measured
2026-08-27" reads like ground truth and is treated as exempt from the doubt
applied to everything else. Twice on 2026-08-29, hours apart: T14.9's *"`agent
read` shows nothing until the turn ends"* was true when written and had stopped
being true, and T14.7's *"the ACP path writes no tool events at all"* had been
fixed by T14.9 without this file being updated. Each was about to fund a night of
building something that already worked, and the second misled an advisor who was
reading carefully and had no way to know. The fork's own docs are the most
dangerous stale input available, precisely because they are the ones written to
be trusted. A measured claim you are about to spend a day on costs ten minutes to
re-run.

**…but running it does not save you if you guessed one of its inputs. Name the
inputs you did not verify.** Measured 2026-08-24: the fork's divergence from
upstream was published as 1168 files and 515 commits when it is 204 and 141 —
wrong by 5x, because the base handed to `git diff <base>...dev` was an upstream
commit `dev` already contained. The command ran perfectly and reported honestly;
the error was one step upstream of it, in a value assumed rather than computed.
`git diff A...B` is *supposed* to protect against a stale base, but when `A` is
an ancestor of `B` the merge-base is `A` itself and the three-dot form silently
degrades to a plain two-dot diff — no warning, no protection. Compute bases
(`$(git merge-base upstream/master dev)`); never paste them. Full account in
`.fork/archive/CONSOLIDATION.md` §1.1.

**…and a number that cannot be true is a reading error, not a fact to
record. It was recorded, and the instruction built on it nearly took the
distro with it.** 2026-09-07: this session read the Lxss `BasePath` for the
Ubuntu distro, saw a C: path, searched X: for a `.vhdx`, found none, and
told the maintainer the image was still on C: and to `wsl --manage --move`
it. Its own note said *"988 GB on a 931 GB drive"* and nobody stopped at a
file larger than the disk it was supposedly on. The path was a **junction**:
Settings → Apps → Move leaves the Store package folder on C: as seven
reparse points into `X:\WpSystem\<SID>\...`, so the registry was true and
the bytes were elsewhere, and the search never looked under `WpSystem`. The
move lifted the vhdx out of app-container EFS; WSL could not attach it
(`E_ACCESSDENIED`, `ERROR_FILE_ENCRYPTED`) and Windows Home has no way to
decrypt. Repaired by another session by moving it back and restoring
`BasePath`; `.fork/runs/wsl-move-2026-09-07/`. Three rules from it. **Never
`wsl --manage --move` this distro.** A negative from a search is only as
good as the directories it entered (`Get-ChildItem -Force` under
`X:\WpSystem`, or `dir /AL` on the package folder, would each have settled
it). And an impossible number is the one thing a session cannot have
measured, so it is the first thing to re-check, not the one to write down.

**GUI gestures are runnable now, so "needs a person" needs an argument.**
`use_computer drag` (T9.1) performs press-move-release against one window and
photographs the frame *before* the release, which is the only moment a drop
preview or a drag ghost exists. It works on Windows too, without taking the
user's cursor, as long as you pass `--pid`/`--window-id` and the window is
foreground (T9.2). Recipe under "Driving a gesture" in `.fork/docs/manual.md`. It
found a real bug on each of its first two runs. What still needs a person is
anything about how something *feels* — latency, smoothness — because no
capture answers that.

**Launch Warp on Windows with `Start-Process … -NoNewWindow`, or it writes no
log at all.** `warp-oss.exe` is a console-subsystem binary; `Start-Process`
gives it its own console, so `stdout_is_a_tty` is true and `warp_logging`'s
`use_logfile` is false. What hides this is that a log still appears — the
crash-recovery sibling has no console and does log, and its file is moved into
`warp-oss.log` when the parent dies. If a log starts with "Parent has crashed;
continuing execution", it is the sibling's and the interesting half was never
written. A person double-clicking the binary is unaffected.

A crash itself is usually Warp's deliberate `Failed to render a frame 3 times
in a row; exiting...`, and the sibling that appears was spawned at *startup*
and parked in `WaitForSingleObject` — its arrival means only that the parent
went away.

## Look for the gate first

The single most repeated finding in this fork: **the feature already exists,
complete and tested, and is switched off.** Grep for the flag before writing
anything.

| | what was already there | where the gate was |
|---|---|---|
| T1 | the whole local control plane (`warpctrl`) | `DOGFOOD_FLAGS` + a per-channel settings default |
| T4 | local Warp Drive sync | an account gate |
| T5 | the entire agent transport | one function — `generate_multi_agent_output` |
| T7 | agent fan-out for a run-scale graph | nothing; the verbs existed, only the *plan* was missing |
| I15 | screenshots, input, recording, window enumeration | `DOGFOOD_FLAGS` + a non-default cargo feature |
| I16 | the whole remote-development server | `RELEASE_FLAGS` behind `cfg!(feature = "release_bundle")` — a *packaging* gate, so it is off in every build you make yourself |
| CLASSIFIER | a cwd-containment rule over an agent's permission requests | **Claude Code's own engine** — an allowed verb on a path outside the cwd already asks, and `cd` out of it already asks. Measured 2026-09-03 before being found in its docs; `.fork/runs/classifier/` |

Gates come in pairs. `crates/warp_features/src/lib.rs` holds `DOGFOOD_FLAGS`
(runtime); `app/Cargo.toml`'s `default` list holds the compile-time half.
Opening one usually means touching both, plus `app/src/fork.rs`.

**Prefer `fork::FORCE_ENABLED` to editing a flag list.** It sets a *user
preference*, and `FeatureFlag::is_enabled` resolves override → user preference →
channel state. So it outranks every **channel list** without touching an upstream
file — which is why I16 needed no edit to `warp_features/src/lib.rs` despite the
flag being `#[cfg(not(windows))]` there.

**But it does not outrank every `#[cfg]`, and this line said it did until
2026-08-30.** Audited from the panel, the claim is true for one kind of `#[cfg]`
and false for another, and the difference decides whether a session is wasted:

- **A `#[cfg]` on a flag-list *entry*** — `DOGFOOD_FLAGS`/`PREVIEW_FLAGS`/
  `RELEASE_FLAGS` membership. The preference wins, because the enum variant
  itself is un-gated and the preference resolves at step 2 of `is_enabled`
  (`crates/warp_features/src/lib.rs:1088`). This is the I16 case.
- **A `#[cfg]` that removes *code*** — at the consumer call site, on a
  cargo-feature-gated module, or on an enum variant absent from this build. A
  runtime preference structurally cannot reach these. `FORCE_ENABLED` resurrects
  a flag slot; it cannot conjure code the linker removed.

The failure mode is the silent one this file exists to warn about: the build is
clean, nothing changes, and there is no error to search for. When a flag looks
gated, check **which** kind you have before reaching for `FORCE_ENABLED`.

**And answering "is this flag on?" from the flag lists gives a confident wrong
answer — measured 2026-09-02, three times in one session.** T16's last three
open items each turned on whether a surface was reachable in this fork. Resolved
against `DOGFOOD_FLAGS`/`PREVIEW_FLAGS`/`RELEASE_FLAGS`, the answer was that
`AIContextMenuCode` and `FileBasedMcp` are in **no list at all** and both
surfaces are dead. That was one edit from being written down. Both are also
entries in `app/src/features.rs`'s `enabled_features()` behind a cargo feature,
and both cargo features are in `default` — so both are **on**, and both losses
the ticket described are real.

**The second half is sharper than the pairing rule above, which this file has
carried since T1 and which is not sufficient.** Checking `default` is *also* not
the test, because a feature in `default` enables others through its own
dependency list:

```toml
remote_codebase_indexing = ["full_source_code_embedding"]
```

`remote_codebase_indexing` is in `default`; `full_source_code_embedding` is not,
and is on anyway. **`cfg!(feature = "…")` in a test is the only honest check** —
an assertion that it was off is what caught this, by failing on its first run
against a TOML reading that had just said otherwise.

That one matters past the ticket: `FullSourceCodeEmbedding` gates the embedding
index, and its only non-mock `StoreClient` is `ServerApi`, whose
`generate_embeddings` sends `Fragment { content: String, .. }` — chunks of the
user's source — to Warp's GraphQL service for OpenAI or Voyage to embed.
`egress.rs` is a deny-list aimed at telemetry vendors and does not cover Warp's
own API. **It is not a leak**: `auto_indexing_enabled` defaults false and the
only writer setting it true is a speedbump banner's `AllowIndexing` arm after
the user ticks "always allow". But that defence is upstream's default, not a
fork policy, in a fork whose thesis is that nothing leaves the machine — so
**Answered 2026-09-02: it does, and closing it turned up a second disclosure
underneath.** `FullSourceCodeEmbedding` is now in `FORCE_DISABLED`, and it is
the first entry there for which the runtime force is the *primary* removal
rather than a backstop — the Sentry entries beside it are belt-and-braces over a
cargo feature that removes the code, this one is the only thing switching off a
feature that is compiled in and on. `the_index_that_uploads_source_is_forced_off_not_merely_absent`
asserts the cargo feature is **on** for exactly that reason, so the evidence for
the entry sits beside the entry and nobody deletes it as redundant.

**Then check what the removal leaves running, because the fallback was the
interesting half.** `SearchCodebase` does not stop working — `get_relevant_files`
falls back to outline search — and that fallback `POST`s `/ai/relevant_files`
with every candidate file's path, its symbol names and the comments written
above each symbol. **Building the outline is local and searching it was not**,
which is a distinction the feature's name actively hides, and it would have
survived the force-disable untouched. The general form: when you switch off the
expensive path, read what the cheap path does, because a fallback is code nobody
chose and everybody inherits.

Closed by `ai::get_relevant_files::local_rank`, and the shape of the fix is the
one this file keeps recommending: **the local ranker already existed.**
`warp_search_core` is the tantivy searcher behind the command palette, already
an `app` dependency, giving BM25, field weights and the same tokenizer the rest
of the app searches with — so the whole replacement is a schema declaration and
one function. What it costs is stated in that module rather than discovered
later: it is lexical, so a query with no shared token returns nothing where a
model would have matched a synonym, and the tokenizer splits `_ - / \ :` but not
camelCase. `the_symbol_map_leaves_by_exactly_one_call_site_and_it_is_guarded`
pins the **count**, not the guard, for the same reason `egress.rs` needed its
second check: a backstop that covers today's call sites is a fact about today.

**And indexing now defaults off, which needed two changes because the gate is a
disjunction.** `should_build_outlines` is `indexing_enabled &&
(codebase_context_enabled || outline_codebase_symbols_for_at_context_menu)`, so
`fork::codebase_indexing_default()` is consulted from *both* settings'
`default_value`; wiring one leaves the walk running through the other, with no
error and a diff that looks finished. The reason is not egress — with the
embedding index gone the remaining work is local tree-sitter parsing — it is
that outline building parses up to 5,000 files of every repository the shell
navigates into, ~100 s over 9p per T6, and upstream asks before the *embedding*
index and never before this one. The settings toggle is the consent and there is
deliberately no second prompt: an affordance that is off until switched on has
already asked.

## Prefer the smallest thing that is still the idea

`crates/warp_cli/src/local_control/graph.rs` is the standard: a run-scale task
graph that added **zero** new app surface — a TOML file and a `while` loop over
verbs that already existed. Reach for a file and a loop before a subsystem.

**One thing to know before running someone else's plan, audited 2026-08-31 and
disclosed in the file's own docs rather than hidden:** the read-only floor is
structural only for a node with `review = true`, where a wide default allowlist
is refused and the fence cannot be widened by naming `read-only` itself — *"a
reviewer that can write can make its own verdict true."* Everywhere else, safety
rests on the plan author naming a restricted allowlist, and *"omit the key
entirely for no restriction"* is the documented default. Assertions are shell on
the runner's commands. So a plan file is code, and it is worth reading before
`graph run` the way any script would be. Nothing wrong was found; this is the
shape of what it is, and this table entry used to sell it without saying so.

---

## The fork's seams

Fork behaviour is deliberately concentrated, so it stays reviewable against
upstream and rebasable.

| file | what it owns |
|---|---|
| `app/src/fork.rs` | **the policy seam.** `is_active()`, `FORCE_ENABLED`/`FORCE_DISABLED` feature flags, and ~a dozen predicates (`local_agent_enabled`, `local_drive_enabled`, `account_gate_bypassed`, …). Start here. |
| `crates/egress_policy/` | the deny-lists — **two of them since 2026-09-04, with a switch each**: telemetry vendors (`WARP_FORK_ALLOW_TELEMETRY_EGRESS`) and Warp's own hosts (`WARP_FORK_ALLOW_WARP_EGRESS`). Kept apart because they make different claims and only the second has a legitimate reason to be lifted — `WARP_FORK_POLICY=0` cannot reach this crate. **A crate of its own since 2026-09-09, and the reason is a cycle rather than taste**: `crates/websocket` had to consult it, and `websocket → http_client` does not compile, because `http_client → warp_core → websocket`. T19 had recorded the objection as one of layering, which is an opinion a later reader can overrule; a cycle is not. So the policy is a leaf with no dependencies and every client sits on top of it. And remember it is a *deny-list*: an unlisted host is an allowed host, so it protects only retroactively and only against the hosts named in it. |
| the three enforcement points | **three, and the count has been wrong twice.** `http_client`'s `Client::execute_inner` covers every verb builder and the oauth2 adapter; `RequestBuilder::eventsource` reaches `execute_inner` **never** and carries its own check; `websocket::WebSocket::connect` is the third. **The count was one** while `egress.rs` claimed a check there *"cannot be bypassed by a call site that forgot"* and `eventsource` had been bypassing it the whole time (found 2026-08-31 by an agent in Warp's own panel, in the file the fork's strongest claim rests on). **It was then two, and `crates/websocket` had been beside it the whole time** — it dials through `async-tungstenite` and has never had an `http_client` dependency, so neither check could see a WebSocket (2026-09-09). Neither miss was a live leak, and both times that is the argument for closing rather than noting: "no call site does this today" is a fact about today. **Adding a way out of `Client` means adding a `redirect_if_blocked` call — grep `self.wrapped` in `lib.rs`, that is the shape of a bypass.** In `websocket`, `imp::connect` is the only door and two tests pin it: one that the door is single and checked, one that **no other crate in the workspace takes a WebSocket dialler as a dependency**. That second one found `crates/graphql` declaring `ws_stream_wasm` directly on the wasm target, unused in its source and not in any binary the fork ships; it is a listed exception with its reason beside it, watched rather than removed. |
| `app/src/ai/local_agent/` | a local implementation of the one agent-transport function, answering from the `claude` CLI. |
| `app/src/ai/acp_agent/` | the same function again, answering from **whatever agent `WARP_FORK_ACP_COMMAND` names**, over the Agent Client Protocol (T14.5). It denies every permission request it receives — but **that is not read-only and must never be described as it**: measured, an agent at its own defaults wrote a file and asked nothing, so Warp denied nothing. T14.8 names that mechanism: `claude-agent-acp` starts in session mode **`auto`**, which it describes itself as *"use a model classifier to approve/deny permission prompts"* — so the thing deciding was a model, and Warp was never in the loop. `session/set_mode` to `default` is what makes it ask. **Re-measured 2026-08-30 at 0.70.0: still `auto` by default, now six modes.** And it is *that agent's* feature, not the protocol's — `modes` is protocol-level and `SessionModeId` is an opaque string, so `opencode` 1.18.25 answers `modes: null` and has no auto-anything to set. Do not generalise a mode id across agents. **And the panel path sends no `set_mode` at all** (T14.18): `acp_agent` sends `session/new` with a cwd and nothing else, so with `claude-agent-acp` a panel session runs in `auto` for its whole life and Warp is asked nothing — measured, 0 permission requests and the file written, against 2 requests when `default` is sent first. The fork's permission model is not too tight there; it is **unreached**. This has stayed hidden because every panel session on the board used `opencode`, which has no modes to be in. **T14.18 answers it by disclosing, not by choosing**: the mode a session starts in is now reported in the panel in the agent's own words, and `WARP_FORK_ACP_MODE` requests one — with no default, because a mode id is opaque and the protocol's own examples are `ask`/`architect`/`code`. Warp says which mode is in force; it never picks one for you. **And the panel's model chip picks this agent's model since 2026-09-07** (T14.14, both halves): the agent's `configOptions` model list becomes the panel's `agent_mode` list, kept in `fork::state_dir()/acp-models.json` between launches because upstream keeps it in memory only, and the pick rides `params.model` and is sent as `session/set_config_option` every turn, since a resumed session reports one model and runs it until told. Measured to Claude Code's own transcript, `.fork/runs/model-2026-09-07/`. `ANTHROPIC_MODEL` in `WARP_FORK_ACP_COMMAND` still decides the agent's default, which is what runs until the chip has a list, so on a profile that has never met the agent that is the first turn. **And a `session/new` refused for a credential says so since 2026-09-10** (T21, `auth.rs`): the agent's own error text is quoted, its own list of ways in is rendered from the wire with id, name and description, and the sentence names `WARP_FORK_ACP_AUTH` as a variable **that does not exist** — naming is not parsing, and Warp neither holds a credential nor sends `authenticate`, because choosing a method id is choosing where a credential comes from. It keys on the *error code*, not on whether the agent advertised methods: measured on the wire, Codex and Gemini both send `-32000` (`ErrorCode::AuthRequired`), while `initialize` is answered before the agent has seen the request — so Codex names all three of its methods whether or not a key is set, and the advertised-list rule would greet the WSL-cwd failure with *"sign in"*. `warpctrl acp probe` cannot see this: it prints an ACP `Error`'s `Display`, which is its `message` alone. `.fork/runs/auth-2026-09-10/`. |
| `app/src/drive/local_sync/` | account-free Warp Drive: snapshot, apply, git-backed sync. |
| `app/src/ai/mcp/tool_digest.rs` | what each MCP server's tools claimed to be, hashed at connect. The tool rug-pull warning rests on this. |
| `app/src/local_control/console.*` | the console (T12) — the fork's **only** browser-reachable surface. Five unauthenticated routes serving five constants (page, script, service worker, manifest, icon), under `default-src 'none'; script-src 'self'`. The worker exists because Chrome on Android has no `Notification` constructor and posts a page's notification only through a registration — measured 2026-09-06 on the emulator (`.fork/tools/phone.sh`), after Brave on the desk had passed the same step the day before; it handles no `fetch` and no `push`, and a test pins that. The script never assigns `innerHTML` and a test pins that — **and since 2026-08-31 a second test pins the sinks that parse no markup at all**: `setAttribute`, `.href`, `.src`, `.style`, `window.open`, `location.assign`. `script-src 'self'` stops an injected `<script>`; it does not stop a `javascript:` href and it does not govern navigation. The guard was narrower than the rule it guards, which is how a rule stops being true without a diff looking wrong. Both tests are calibrated by making them fail, not by watching them pass. Keep it that way, because everything it draws was authored by an agent. **What the CSP does not cover, stated so nobody credits it with more than it does:** `connect-src 'self'` cannot tell the page's own fetch from a hostile one to the same origin, and no directive governs top-level navigation — so the page's safety rests on the `textContent`-only discipline, and the CSP is what stops that discipline's failure from becoming remote code. **After editing it run `node --check app/src/local_control/console.js`** — it is `include_str!`d, so a syntax error compiles fine, passes every Rust test, and breaks the whole page at runtime. And remember the page draws from `PendingApproval`: a control there must be gated on what the *entry* permits, not only on what the device may do (T14.6). |
| `app/src/terminal/model/session/filesystem.rs` | **where a session's files actually live**, as one answer. `SessionType` is a *bootstrap* fact and a WSL session's is `Local` — `determine_session_type` compares hostnames and WSL2 inherits the Windows machine name — while its files are inside the distribution. Every call site that asked `session_type()` about a *file* therefore reached back across the 9p redirector, at roughly 20 ms per directory entry, past a server sitting idle beside those files. `session_filesystem` returns `Local`, `Host(id)` or `Unreachable`; the rule is a pure `classify(session_type, is_wsl, connected_host)` with unit tests, the lookups around it are not. **`Unreachable` is a third state on purpose**: a remote session with no server attached is not local, and a caller that treats it as local reads *this* machine's filesystem for another machine's paths — which succeeds often enough to be worse than failing. Two places keep `session_type()` deliberately and say why: the orchestration gate (about where *commands* run, and a WSL shell is already native Linux) and the completer (which had already solved this upstream in `wsl_guest_listing`, APP-3993 — **upstream independently found that enumerating a WSL directory from Windows is wrong, and asks the guest**). A new reader is caught by `every_file_that_reads_session_type_has_been_classified`, which requires every live `session_type()` read in `app/src` to appear in a list with a reason. |
| `app/src/local_control/`, `crates/local_control/`, `crates/warp_cli/src/local_control/` | the `warpctrl` control plane, 115 actions. The count is pinned by **two** tests in different crates — update both, and never loosen either. **This line said 109 for two phases**: T11.2 took it to 110, T11.4 to 111, T11.5 to 114 and item 6's `agent.trace` to 115, and each updated the pins without updating this table. Read the count off the test, never off prose — and grep for `fn catalog_has_exactly`, because the test's own name embeds the number and so goes stale on exactly the schedule this warning is about. |
| `app/src/remote_server/wsl_transport.rs`, `crates/remote_server/src/wsl.rs` | the second `RemoteTransport`: Warp's remote-development server, in a WSL distro instead of over SSH. |

**A routed pane's daemon has none of this process's configuration**, so
anything `fork.rs` re-points at the user's own model or keychain is dead there
until the *client* does the work. Upstream has the daemon both read the repo
and call the model; the endpoint, model and key live here. Fixed 2026-09-12 by
handing the client the inputs — `return_diff_only` for the commit message,
`return_pr_inputs` and `title`/`body` for the two PR paths, which cost a
second round trip because something runs after the model. **The PR paths hid
five days longer, and that is the rule: the commit dialog went blank and was
filed in a day, while `create_pr` falls back to `gh pr create --fill`, so the
same broken generator kept shipping real PRs titled by git. A fallback is not
a fix — it is a defect that stopped reporting itself.** `.fork/docs/wsl.md`.

**Every request through `http_client::Client::get` tells the destination what
this machine is, whoever the destination is.** Found 2026-09-09 while building
the fork's first deliberate third-party request (T21.4). `include_warp_http_headers`
returns `true` **unconditionally** on every non-wasm target — only the wasm
branch asks whether the destination is Warp's — so `get`, `post` and every
other verb attach the client id, the app version and four fields describing the
operating system down to the Linux kernel version. In this fork the app version
is `v0.fork.<sha>`, which names the commit the binary was built from.

For Warp's own API that is correct and is what the headers are for. Anywhere
else it is a fingerprint, and **nothing in a new module's diff shows it** —
the disclosure is in a function the call site does not mention.
`Client::get_without_warp_headers` is the one door that omits them; it still
runs through `execute_inner`, so the egress policy sees it. Before adding any
request to a host that is not Warp's, ask which of the two you want, and
remember that the ordinary-looking one is the disclosing one.

**The fork no longer talks to Warp's servers at all, and the two things that
were stopping it were both accidents.** Closed 2026-09-04, from a question about
why the About page reads `v#.##.###`.

That string is not a version format. It is the `unwrap_or` fallback at
`about_page.rs:72`, so it means **no version was stamped**: `app_version()` is
`option_env!("GIT_RELEASE_TAG")`, set by release CI and by nothing else, and
`--version` says `<unknown>` while the commit sha appears zero times in the
binary. Local builds were unidentifiable, which with two checkouts had already
cost time twice.

The fix is one environment variable, and setting it would have switched on
autoupdate. Two of the three guards on the self-replace path are
`app_version().is_none()`, so *not knowing its own version* was what stopped the
fork replacing its binary from Warp's servers. `fork.rs` had no autoupdate
predicate and `egress.rs` did not cover `warp.dev`. Now
`FeatureFlag::Autoupdate` is in `FORCE_DISABLED` and `fork::autoupdate_allowed`
guards `check_for_update`, so the two are decoupled and `.fork/tools/build.sh`
and `build.ps1` both name the build `v0.fork.<sha>`.

**They name it in a sidecar file, not in the environment, since 2026-09-05,
and the day in between cost twenty minutes per commit.** Stamping
`GIT_RELEASE_TAG` compiles the tag into `warp_core` through `option_env!`, and
cargo rebuilds every dependent of a crate it rebuilds with no check that the
output changed. Measured: a changed tag invalidated **55 crates** in one
`cargo check` of the app, so every commit had become a near-clean release
build, with a 41 GB peak on the machine the fork lives on. `app_version()`
now falls back to `<binary>.version` beside the resolved executable, written
by both scripts after a successful build. Same About page, same `--version`,
same discovery record, same remote-server handshake, zero compile cost. **Do
not set `GIT_RELEASE_TAG` for a local build**; both scripts clear it in case
the shell inherited one. The live Windows script is `C:\dev\build.ps1`, and
`.fork/tools/build.ps1` is its tracked copy: change one, copy to the other.

The cost it removed is measured in `.fork/docs/build.md`.

**The bigger one was underneath it, and it was live.**
`generate_multi_agent_output` intercepts for the ACP and local agents *only when
one is configured* — `handles()` asks whether the request carries a
`UserQuery` — so a fork build with neither `WARP_FORK_ACP_COMMAND` nor
`WARP_FORK_LOCAL_AGENT` set sent the prompt, the conversation and the attached
context to `app.warp.dev`, with **no warning, no log line and nothing in the
panel**. Upstream's default, working exactly as upstream intends. Two layers
now: `impl.rs` refuses with a message naming both variables, and `warp.dev` is
on a new first-party deny-list in `egress.rs`.

That list is deliberately **separate from the telemetry one, with its own
switch** (`WARP_FORK_ALLOW_WARP_EGRESS`). The two make different claims —
vendors must never receive data; Warp's own hosts are ones the product
legitimately uses and this fork replaced one at a time — and the first-party
half has a real reason to be lifted: `WARP_FORK_POLICY=0` is this file's own
advice for A/B-ing a suspected regression, and it cannot reach `http_client` at
all. One switch would have silently broken that.

Two things this cost, both worth knowing. The seam refusal is `cfg`-ed out of
test builds, because the same condition (`any(test, feature = "test-util")`) is
what reroutes server URLs to a mockito instance — so it has **no runtime test**
and is pinned by source text; the deny-list is the layer with real assertions.
And it broke three upstream autoupdate tests on its first placement, in
`get_next_request`, which owns a request-queue state machine that upstream
drives directly. The guard belongs at `check_for_update`, where the request is
spawned.

**`WARP_FORK_ACP_COMMAND` names an agent and it answers the agent panel.** Naming
the command *is* the switch, there is no second flag, and it outranks
`WARP_FORK_LOCAL_AGENT`. The table at the end of this section is the index of
every variable the fork adds; `.fork/docs/environment.md` carries the account
behind each.

**Which agent to name, measured 2026-08-31 rather than preferred.** This line
used `"opencode acp"` as its only example for months. Both agents were then put
through the same refusal, and neither is unambiguously better — but one *pairing*
is:

| | asks Warp by default? | survives a refusal? |
|---|---|---|
| `opencode acp` | **yes** | **no** — no further output at all, turn over, even when the prompt says what to do instead |
| `claude-agent-acp`, no mode set | **no** — session mode `auto`, its classifier answers first and Warp is never in the loop | n/a |
| `claude-agent-acp` + `WARP_FORK_ACP_MODE=default` | **yes** | **yes** — *"I can't run that — you denied permission. So, 2+2 is 4."* |
| `codex-acp` (Zed's wrapper) + a provider key | **yes** — opens in `read-only`, no variable needed | not measured |

So **the recommended configuration is the third row**, and it is a pairing:
either half of it alone is worse than `opencode`. Warp sends the identical
per-call rejection (`{"outcome": "selected", "optionId": "reject"}`, never
`Cancelled`) in every case, so the difference is entirely the agent's.

**The fourth row is new on 2026-09-11 and is the first agent that asks without
a mode variable.** codex-acp opens in `read-only`, runs on the maintainer's own
OpenRouter key through codex's `model_providers`, sends no `authenticate`, and
across four watched runs reached **openrouter.ai and nothing else**. Config
traps: `wire_api` accepts only `responses` at the pinned codex, and a
`CODEX_HOME` under `/tmp` is refused. `.fork/runs/codex-wire-2026-09-11/`.

**Two rules from it that are not about codex.** *Ask the question of the pinned
tree, not of HEAD* — a wrapper vendors a version, and reading the newer one is
how a config block got published that the binary refuses. And *linked is not
reached*: the telemetry crates are compiled into that binary while their
endpoints are absent, because nothing calls them and `--gc-sections` takes the
unreferenced chain's strings with it. **A dependency graph answers a different
question from a call graph**, and it is the one that reads as alarming.

**On the Windows build, name the agent so it starts *inside* the distribution, or
every turn in a WSL pane fails before it begins.** Measured 2026-09-02, end to
end. A WSL pane's cwd is a Unix path, Warp passes it verbatim in `session/new`,
and the agent process is spawned by the Windows Warp — so `claude-agent-acp`
refuses the session outright:

```
Invalid params: `cwd` does not exist on the machine running the agent:
/home/effatha/scratch-t17/repo
```

**Not caused by routing, and not by T16** — an *unrouted* WSL pane fails
identically with `/home/effatha`, which is the control that settles the
attribution. It is structural to Windows-Warp-plus-WSL-pane, and it is invisible
on the Linux build, where agent and shell share a filesystem and every one of
this file's other ACP measurements was taken.

The remedy is one variable and no code:

```
WARP_FORK_ACP_COMMAND='wsl.exe -d Ubuntu -- npx -y @agentclientprotocol/claude-agent-acp@0.73.0'
```

**Pin the version, and this file did not until 2026-09-03.** Unpinned, `npx -y`
resolves to whatever is newest, and two installs sat in `~/.npm/_npx/` for a
week. It is not a cosmetic difference: **0.70.0 declares what an `allow_always`
option would widen (`_meta.permission.changes`, with a `lifetime` scope) and
0.73.0 declares nothing on any option** — verified by grepping both `dist/`
trees. A finding was published as *"probed live at 0.70.0"* that had actually run
0.73.0, and the two versions give opposite answers to I18's central question.

**And the version was already in hand.** `acp probe`'s first line is the agent's
`initialize` reply, which carries
`agentInfo: {name, title, version: "0.73.0"}` — read from the wire, printed, and
then labelled from memory anyway. The panel path was genuinely blind and now
writes a `session_agent` event log line; the probe never was. Re-measure before
trusting any ACP claim in these docs dated before this line.

Measured with that in place, end to end on a **routed** session
(`session inspect` → `{"where": "host"}`, `cd` first and connect second):
`session/new` accepted, turns `status: success`, `WARP_FORK_ACP_MODE=default`
requested and accepted — the turn opened in `auto` and the agent moved to
`default`, which is the per-turn re-send earning its keep — and the `LSP` tool
returning real rust-analyzer output:

```
WslMarker (Struct) - Line 1
  id (Field) u32 - Line 3
build_wsl_marker (Function) fn(id: u32) -> WslMarker - Line 6
main (Function) fn() - Line 11
```

**Checked the way an LSP answer has to be checked**, because a plausible one and
a real one read identically: the two symbols carrying doc comments report the
*doc* line (items at 2 and 7), the two without report the item line exactly.
Only the documented ones are offset, each by its own doc length — a fabrication
would have to know which symbols have docs, offset only those, and invent the
field type and signature correctly. The alternative — translating the cwd to
`\\wsl.localhost\<distro>\…` for a Windows-side agent — would also work
(Windows rust-analyzer over UNC measured at ~5% overhead against a local copy on
a small crate) but leaves the agent running its shell commands on the wrong side
of the boundary. `session.windows_path_converter()` already does the *opposite*
direction; nothing does this one.

**Two traps when reproducing this.** The default shell here is bash-over-Ubuntu,
so a fresh pane is *already* a WSL session — typing `wsl.exe -d Ubuntu` into it
makes a nested subshell whose block never completes, after which `input submit`
answers `queued: true` forever and `agent prompt` refuses with
`target_state_conflict`. That reads as three separate product defects and is one
test-setup mistake. And the agent panel takes focus on launch: press Escape to
reach the shell.

**A crash from the LSP tool is probably your toolchain, not the wiring.**
`rust-analyzer crashed with exit code 1` here meant the rustup *default*
toolchain lacked the `rust-analyzer` component while the repo-pinned one had it —
the server is spawned from the agent's cwd, so the pin that matters is the one
resolving there, not in the file being asked about.

**And code navigation is a third thing that pairing gets you, which Warp does
not have and should not build.** Asked 2026-09-02 whether the fork should give
its agents go-to-definition and find-references, the answer is that the
recommended agent already has them. Claude Code ships an `LSP` tool
(`operation`/`filePath`/`line`/`character`, 1-based), enabled per language by
installing a `*-lsp` plugin, and `claude-agent-acp` 0.70.0 sets
`settingSources: ["user","project","local"]` so those plugins load on the ACP
path too. Measured with `acp probe` on the same transport the panel uses:
`tool_call` titled `LSP`, `operation: documentSymbol`, a real answer in 8 s,
**zero permission requests**.

Verified rather than taken on trust, and the check is worth copying because it
is the one that separates a real LSP answer from a plausible one: the returned
line numbers matched **no commit in the repository**, which looks like
fabrication until you notice they are each the first line of that symbol's *doc
comment* — which is what `documentSymbol` reports. They matched the live working
tree, including a doc comment written hours after the probe binary was built. A
model cannot recall that.

**`opencode` is the cautionary half.** It has a full LSP client and an
agent-facing `lsp` tool, gated by `OPENCODE_EXPERIMENTAL_LSP_TOOL`. Setting it
opens the gate and, measured, the rust server did not attach — the tool answered
*"No LSP server available for this file type"* and **the model then invented five
symbols with line numbers matching nothing**. So that variable is not a
recommendation: it converts a missing capability into a confident wrong answer,
which is worse.

Nothing to build in Warp for this. `ToolType` comes from `warp-proto-apis` and
has no LSP variant, so a tool Warp *runs* cannot exist without a protocol change;
`acp_agent` ignores the tool list entirely; and `local_agent` consumes it only to
build `--allowedTools`, so a new variant there would need a mapping arm and would
still just be re-granting the agent its own tool. The one thing Warp is uniquely
positioned to answer is LSP for a **routed WSL session**, where the agent runs on
the Windows side and the language server does not — `crates/remote_server` proxies
no LSP at all. That is the only case where building something here would not be
duplicating a working tool.

Measured across two working sessions the same day: with `opencode`, one refusal
cost a whole turn's answer while the conversation still reported
`status: success`. With the pairing above, seven consecutive turns raised zero
refusals and lost nothing. `opencode` remains perfectly usable and is what most of
this file's other measurements were taken against — it is named second now, not
removed.

**The session cwd comes from the pane, and that is where the agent finds its own
config — so the pane's directory decides whether the user's permission rules
load at all.** Measured: the same agent in a directory without its config file
ran a shell command in `$HOME` and sent no permission request; in a directory
with one it asked, and Warp denied. This corrects an earlier claim here that the
config came from wherever Warp was launched.

### The variables, and the one fact each

| variable | default | what it does, and what not to get wrong |
|---|---|---|
| `WARP_FORK_ACP_COMMAND` | unset | names the ACP agent that answers the panel. Naming it *is* the switch, and it outranks `WARP_FORK_LOCAL_AGENT`. Everything above. |
| `WARP_FORK_LOCAL_AGENT` | off | `1`/`on`/`true`: answer conversations from the local `claude` CLI instead of `api.warp.dev`. |
| `WARP_FORK_ACP_MODE` | unset | the session mode to ask the agent for, **in that agent's own id for it** — `default` for `claude-agent-acp`, which is what makes it ask instead of letting its `auto` classifier answer. An id the agent did not advertise **refuses the turn**. Re-sent every turn, deliberately. |
| `WARP_FORK_POLICY` | on | `0`/`off`/`false` runs stock upstream without rebuilding — the way to A/B a suspected fork regression. **Cannot reach `http_client`**, so pair it with the row below; and a policy-off instance publishes no discovery record, so plan the shutdown first. |
| `WARP_FORK_ALLOW_WARP_EGRESS` | off | lifts the **first-party** block only — `warp.dev` and its subdomains. A separate switch from the next row on purpose. |
| `WARP_FORK_ALLOW_TELEMETRY_EGRESS` | off | lifts the telemetry-vendor deny-list. Nothing legitimate sets this. |
| `WARP_FORK_AGENT_SPAWN_DEPTH` | `2` | how deep `warpctrl agent spawn` may nest. Bounds depth, not breadth. |
| `WARP_FORK_QUAKE_VISOR` | **on** | the fork's visor in the hotkey window; off gives upstream's terminal there. |
| `WARP_FORK_WSL_AUTO_CONNECT` | **on** | a WSL pane attaches the remote-development server to its own distribution at shell bootstrap, so files route inside it instead of over 9p. `warpctrl session inspect` says which you have. |
| `WARP_FORK_WSL_LSP` | **on** | a language server for a `\\wsl$\<distro>\...` workspace runs *inside* the distribution. |
| `WARP_FORK_MODEL_PRICES` | off | `fetch`, and nothing else. **The only variable that makes Warp's own HTTP client dial a host of Warp's choosing** — OpenRouter's public model list, once per launch. Neither deny-list would have stopped it. |
| `WARP_FORK_FRAME_LOG` | off | `on`, or a threshold in ms. Slow-frame accounting to the local log — **reach for this before theorising about why something feels slow**. |
| `WARP_FORK_EVENT_LOG` | off | `on`, or a directory. One JSONL file per agent session — **reach for this before theorising about what an agent did**. **Read a zero carefully**: no permission lines means Warp was not in the loop, not that nothing was decided. Read in timestamp order, never filename order. |
| `WARP_FORK_TRANSCRIPT` | off | `on` writes the conversation under the pane's own `.warp/transcripts/`, so an agent can grep back what its compaction discarded. **Holds the user's prompts verbatim**; owner-only since 2026-08-31. Any other value is taken as the directory. |
| `WARP_FORK_HARNESS_DIR` | unset | the agent's `.claude/projects` directory for `agent.trace`, when the instance's own search does not find it. |
| `WARP_FORK_CONTROL_BIND` | loopback | **the only one that reaches off the machine.** One literal IP, optionally with a port — pin the port. A hostname, a wildcard or a typo leaves the wide listener shut and loopback serving. |
| `WARP_FORK_REMOTE_APPROVE` | off | lets a *paired* device run `agent.approve` — say **yes** to an agent's prompt from a phone. Only a literal `1`/`on`/`true`/`yes`; `agent.deny` needs no switch, because saying no can only make less happen. |
| `WARP_FORK_WINDOW_BOUNDS` | unset | `1400x900+100+100`: every normal window opens there, restored ones too. Virtual-screen coordinates, so on Windows a small offset is the primary monitor. |

Tab→pane drag has no variable of its own; `WARP_FORK_POLICY=0` puts the tab's
horizontal-only drag axis back.

**The accounts are in `.fork/docs/environment.md`** — what each default was
chosen against, what was measured, what was retracted, and why
`WARP_FORK_CONTROL_BIND` refuses a typo loudly while `WARP_FORK_REMOTE_APPROVE`
treats one as simply not consent. Read it before changing a parser or adding a
variable.

---

## Working rules

**Build with `--features gui,warp_control_cli`, and stop a running Warp with
`warpctrl window close`** (`CloseMainWindow` on Windows) — never `kill`, which
leaves a crash-recovery sibling holding the ports. `ok: true` from that call
means only that the request was dispatched, not that the window closed; a CLI
agent alive in a pane or a wedged ACP turn each block it silently, with none
of `agent list`'s instruments lighting up, so end the agent or `agent cancel`
first. Two discovery registries exist on one machine
(`$XDG_RUNTIME_DIR/warp/local-control` vs `$HOME/.warp/local-control`), and a
`no_instance` from one while a Warp runs in the other is indistinguishable
from a genuinely absent Warp until you check which directory was searched.
`WARP_FORK_POLICY=0` still needs a shutdown plan — a policy-off instance holds
upstream's `9282` with an empty discovery directory, so `warpctrl` cannot see
it, but you usually don't need the GUI at all: `--warpctrl` runs
`init_feature_flags` before it dispatches, so a headless
`WARP_FORK_POLICY=0 warp-oss --warpctrl instance list` A/Bs any flag with no
window and no port. **`.fork/docs/warpctrl.md` has every wrong version of
"why won't it close" this file cycled through** — the discovery-record
correction that took two attempts, the credential-broker explanation that
stood wrong for a week, and the orphaned-listener trap on Windows.

**Whether a denial or a refusal costs the whole turn is a fact about the
agent you named, not about the fork** — `opencode` goes silent after a `no`
where `claude-agent-acp` answers around it, though Warp sends the identical
per-call rejection to both; and a turn can report `status: success` while a
denial ~90 seconds in left no answer at all, so read the output rather than
the status. The Claude Code plugin (OSC 777 hooks) must already be loaded
when a CLI agent starts or `agent approvals` stays silently empty, and its
permission payload carries no call id and cannot answer — a hook that only
reports is not the same limitation as a fork that cannot listen.
`opencode.json`'s `bash` permission map has two footguns that read backwards
(later keys win; an unmatched command defaults to *allow*, not *ask*) but
does decompose a compound command and check every segment, so `&&` cannot
smuggle a denied command past an allowed one. `external_directory` grants and
bash allows are two separate doors that do not compose — a grant on the
file-reading door does nothing for the same path reached through a shell
call. And the fork's transports (`local_agent`, `acp_agent`) never re-emit a
tool call as an `Action` message, because the agent already ran it — the
transcript prose is the record on these paths, by design, not an omission.
**`.fork/docs/agent-transports.md` has the full account**: the refusal table,
the plugin's TR-EVENTS-B measurement, the bash-map traps and the four
commands (`ls`/`wc` allowed, `find`/`cat`/`grep` refused) that did not get the
same answer, and why steering in a prompt is worth writing but not worth
relying on.

**An agent in the panel works in the *pane's* directory, and that directory
has two sources you did not choose.** `warpctrl input submit 'cd
/home/effatha/git/warp'` before the first prompt, or an ACP agent silently
resolves its own permission config from the wrong place too. A fresh pane
*inherits Warp's own process cwd*; a **restored** pane keeps whatever
directory it had last launch, which is invisible in the launch command and
survives a reboot — this line said "a fresh pane starts in `$HOME`" until
2026-09-01, which was true only when Warp itself had been launched from
`$HOME`. Reading `/proc/<pid>/cwd` measures the shell, not the agent; they
usually agree and are not the same quantity. For a genuinely clean test
profile, point `XDG_CONFIG_HOME`/`XDG_STATE_HOME` at a scratch directory
(never edit the user's own `settings.toml`) and seed
`$XDG_CONFIG_HOME/warp-oss/user_preferences.json` with
`{"prefs": {"HasCompletedOnboarding": "true"}}` — the directory is
`warp-oss` not `warp-terminal`, and the key must sit inside `prefs` or it is
silently discarded. `.fork/docs/agent-transports.md` has the full table and
both traps.

**Verify a build before trusting a run, and know which checkout you are
asking.** On Linux, `date -r target/release/warp-oss` against the newest file
you touched catches a run against a stale pre-fix binary — a compile is a
mutation with a long latency and no completion signal of its own. **On
Windows that check is meaningless**: `C:\dev\warp` is a separate clone with
no sync step, so check the commit (`git -C /mnt/c/dev/warp log --oneline -1`)
and sync by the side whose `git` you are holding — WSL fetches the Linux path,
PowerShell fetches `origin`. `.fork/docs/build.md` has both commands and the
symlink recipe for Developer Mode's Git-for-Windows/PowerShell split.

**Never `pgrep -f` a pattern your own command line contains** — a wrapping
`bash -c` matches its own argv. Match something narrower, or check the thing
you actually care about.

**Setting `PATH` on a child means opposite things on the two platforms.**
Measured 2026-09-12 with a fake shadowing an installed binary and a fake with a
name nothing else has, each run with a no-`PATH` control:

| | name also on the parent's `PATH` | name only on the child's |
|---|---|---|
| **Linux** (glibc 2.39) | the **parent's** runs | the child's runs |
| **Windows** | the **child's** runs | the child's runs |

So on Windows the child's `PATH` decides; on Linux it is only a *fallback*, and
`Command::new("gh").env("PATH", user_path)` there runs the inherited `gh`
whenever there is one. Resolve explicitly — `warp_util::path::resolve_executable_in_path`
— and both platforms agree. **A test that plants a fake on a child `PATH`
exercises the real binary on Linux** whenever that name is installed; five here
did, green for months because the fake imitated the real error. This rule was
written twice wrongly the same day, first as *lookup always uses the parent's*
and then as *a fallback on both platforms*, each from one observation without a
control. `node_runtime`'s comment is wrong the other way — `cmd.exe /c` on
Windows is earned by `.cmd`/`.bat` resolution, not by `PATH`.
`.fork/runs/ghpath-2026-09-12/`.

**Diff test-failure *membership*, not counts.** A 9-failure swing between runs
is normal weather here (`-p warp --lib`'s known flaky set, mostly shared
login-state tests in `ai::mcp::file_based_manager` that pass serially and
alone). Baseline on a *union* of at least two runs, or a flaky pass in a single
baseline run promotes an old failure to a fresh "regression".

**A catalog count or an allowlist is pinned by a test, never by this file.**
`warpctrl`'s action count and `PAIRABLE_ACTIONS`' membership are each asserted
in two places (`fn catalog_has_exactly_*`, `a_paired_device_gets_the_read_surface…`)
— grep the test name, don't paste the number; it goes stale here on exactly
the schedule this warning is about.

**Widening a shared type, or merging upstream, needs `cargo check --workspace
--all-targets`** — `--bin warp-oss` compiles neither test code nor `warp_tui`,
so a clean binary build proves nothing about either.

**Cap a WSL release build only when you cannot avoid building on both sides at
once: `CARGO_BUILD_JOBS=8 cargo build --release …`.** Uncapped, `-j` was
measured clean at full width on 2026-09-11 and never stood between the build
and the VM's wall — the actual ceiling is the `warp` crate compiling alone,
which no `-j` value changes. What still takes the guest down is a WSL build
concurrent with a Windows one, which is pressure on the *host's* memory and
invisible from inside the guest. `[profile.release.package.warp]` bounds the
app crate's own peak (`debug = 0`, `codegen-units = 64`, −28%); killing a
resident `rust-analyzer` first is worth three times that. `.fork/docs/build.md`
has every number and the two wrong extrapolations that preceded this.

**`cargo clean --release` dangles the WSL remote-server symlink** and surfaces
as *"Failed to start SSH extension"*, which reads like a network fault — read
the log (`grep -i 'remote server'`) before the network.

**Never share `CARGO_TARGET_DIR` between two checkouts of this workspace.** A
shared cache can pass `cargo check --workspace --all-targets` while holding
artifacts that match neither tree.

**A cargo *feature* enabled by one dependency can silently change another
crate's behaviour.** `agent-client-protocol` turned on `serde_json/preserve_order`
workspace-wide, which broke `local_sync`'s byte-stable output with no line of
`local_sync` changed and no compile error anywhere. If a module's output must
be byte-stable, sort explicitly at that seam — do not rely on a dependency's
default to stay off.

**A build script must `rerun-if-changed` every file it reads, not just
itself** — `crates/graphql/build.rs` watched only itself and shipped a stale
schema registration across a merge that changed both the queries and the
schema.

**`.fork/docs/build.md` carries the account behind every rule above**: the
measured crash that justified the original cap, the two wrong extrapolations
from it, and the clean uncapped run that retired it. Read it before re-tuning
`CARGO_BUILD_JOBS` or re-opening the `-j` question — the next move is running
the uncapped build on both sides at once, deliberately, not re-reading numbers.

**Merging upstream: watch the overlap, not the divergence.** `git merge-base
upstream/master dev` computed, never pasted (see *Method*), then the file sets
intersected — 39 of the fork's 204 files on 2026-08-24, of which 4 conflicted.
That number is the early warning, and it is what makes a soft fork cheap; the
cost is not paid when divergence is incurred but when a merge is deferred.

**Prose in this repo is exempt from the global `unslop` rule against em
dashes, and from that rule only.** Sampled 2026-09-04: of seven drawn at random
from this file, one was a parenthetical no other punctuation handles and three
were a period or a comma dressed up. So use fewer, rather than banning them —
the survivors are load-bearing, marking a correction as an aside rather than as
a fresh claim, which is most of what this file does.

Every other `unslop` rule applies in full, and the three that bite hardest are
*say the concrete thing*, *shorten or split dense sentences*, and *significance
inflation*. The real cost of this document is not its punctuation: every
paragraph opens with a bolded thesis and asks the reader for a small revelation,
so nothing is allowed to be a quiet supporting detail. That structure makes
retractions stick, which is why it stays — and at this length it buries the
lookup-shaped facts inside the narrative ones, which are the facts people come
here for.

**Formatting: run `./script/format`, and disregard `AGENTS.md` on this point.**
Measured 2026-08-21: `cargo fmt` with the project's config wants to change **11
files, every one of them fork-authored, with no upstream drive-bys** — so the
workspace-wide command is safe and the "never `cargo fmt`" folklore is wrong.
The real hazard is per-file: tests live in sibling files pulled in with
`#[path]` (see `script/check_no_inline_test_modules`), and rustfmt follows those
edges, so `rustfmt some_mod.rs` silently rewrites `some_mod_tests.rs` too.
Whichever you run, check `git status` afterwards and revert files you did not
mean to touch.

**Reaching the console from a phone, and the real remote backstop: both are
written up, not repeated here.** The console needs mirrored WSL networking, a
Windows firewall rule for the bound port, and Warp actually running — three
things, none of them the VPN. It now speaks TLS (an authority Warp mints
itself) and sits on a Tailscale address rather than the LAN since 2026-09-08.
An Android emulator stands in for a phone today (`.fork/tools/phone.sh`). The
paired console reaches seven `warpctrl` actions (eight with
`WARP_FORK_REMOTE_APPROVE`); SSH reaches all 115, because authority follows
credential strength — a QR code is a bearer token shown to a room, an SSH key
is held by one device. `.fork/docs/remote-control.md` has the TLS mechanics,
what the approval digest does and does not defend against, and the action
count's history. `.fork/docs/away-from-desk.md` has the SSH setup recipe and
its four traps (a firewall rule is per-port; `authorized_keys` can silently be
a directory; `BatchMode=yes` swallows a passphrase prompt, not just a
password).

**A git worktree is workflow insulation, not security insulation, and calling it
"insulated space" for an agent is actively misleading.** An agent with shell
access in a worktree runs as the same UID, sees the same `$HOME`, the same
`~/.ssh`, the same discovery records and credential broker, and can `cd`
anywhere. Security insulation is a boundary the *kernel* enforces on the
process; a directory choice enforces nothing. And the worktree habit carries the
measured `CARGO_TARGET_DIR` hazard recorded above, so it costs something real
while delivering none of the benefit it is reached for. Whatever sandbox is
available is a **word until one calibrated test** — a sandboxed command
attempting a write outside the project and a network call, both confirmed to
fail — has been run.

**There is a second binary, `crates/warp_tui`, and it turned out to be a fork
surface after all — this line said otherwise for eleven days, because a grep
over the library answered a question about the library when the question was
about the binary it links into.** `warp_tui` depends on the `app` crate, so
`fork::` and the account gate bypass are in scope in `app/src/tui/mod.rs`
regardless of what `crates/warp_tui/src/` alone shows. Built with
`--features standalone,warp_control_cli`, it now publishes a discovery record
and answers `warpctrl` like any other instance — a *server*, not an answerer,
so consent still stays a typed command in a different process. The telemetry
deny-list is on by default in it too, because that policy lives below
`http_client`, not in the app's own seam. **`.fork/docs/agent-transports.md`
has the full account**, including what is still unverified: an ACP permission
request parked and answered from a shell while the TUI holds the prompt, end
to end, inside one session.

**There is a browser, and it is on the Windows side.** `/mnt/c/Program Files`
holds Firefox, Brave and Zen, and Windows reaches the WSL wide listener — so a
page served by `WARP_FORK_CONTROL_BIND` can be loaded in a real engine and
photographed with `shot.ps1 -Process firefox`. Launch with a scratch profile
(`-profile 'C:\dev\…' -no-remote`, or `--user-data-dir=` for Chromium) so the
user's own session is untouched. **T12 filed "no browser on this machine" three
times as a blocker; it meant the WSL userland and was written as though it meant
the hardware.** Before naming something as needing a person, check that the
blocker is real.

**Screenshotting the Windows build: `shot.ps1 -Process warp-oss`.** It
captures one window via `PrintWindow(hwnd, hdc, PW_RENDERFULLCONTENT)` —
*without* raising or focusing it, and regardless of what is on top. Omit
`-Process` and it silently falls back to a grab of the entire virtual screen,
which on a working desktop is unreadable. `CopyFromScreen` cannot substitute
(it only sees what is displayed), raising the window first cannot substitute
(the foreground lock refuses it from a background process), and the `2` flag is
not optional (GPU-composited windows capture blank without it). Full reasoning
under "The scripts" in `.fork/docs/manual.md` — **this recipe has been lost to a
cleared session twice.**

**Running on WSLg:** `env -u WAYLAND_DISPLAY LIBGL_ALWAYS_SOFTWARE=1
./target/release/warp-oss`. Unsetting `WAYLAND_DISPLAY` puts winit on X11, which
is what screenshots and global hotkeys need. Synthetic clicks land there, and
**so do synthetic keystrokes — but only as far as the keymap.** Measured
2026-08-24, correcting a blanket "keystrokes do not land" that had been used to
block work: `use_computer drag --press 0xff1b` mid-drag produced
`EditorAction::Escape` → `PaneGroupAction::CancelDrag` in the log and visibly
cancelled the drag, while `use_computer text "echo …"` into a focused terminal
input produced nothing at all. So cancel keys and shortcuts are drivable and
typing is not. `Key::Keycode(n)` is an X **keysym** on this backend, not a
keycode — Escape is `0xff1b`.

**Screenshotting on WSLg: capture the window, never the root.** Same failure as
the Windows `CopyFromScreen` case and worth the two lines it costs:

```
DISPLAY=:0 xwininfo -root -children     # the app is the child with a real geometry
DISPLAY=:0 import -window 0x20006d /tmp/shot.png
```

`scrot`/`import -window root` return a **solid black frame** — the surface is
GPU-composited and the root window never held its pixels. `import -window <id>`
gets the real contents without raising or focusing anything. The id changes every
launch, so read it rather than remember it; Warp is the child sized like a window
(`1246x802+1089+596`), among Weston's own 1x1 and 10x10 stubs.

**A WSL pane's files, its language server, and its diff panel are all
answered by `warpctrl session inspect`, and none of the three behave the way
an unrouted intuition expects — full account in `.fork/docs/wsl.md`.** A
session keeps `SessionType::Local` even when routed, so nothing else in any
payload distinguishes a routed pane from an ordinary one; the diff panel and
LSP both work either way and the "never finishes" this file's neighbours once
carried for the unrouted case did not reproduce; and connect-before-`cd`
ordering stopped mattering once `SessionConnected` began re-running repo
detection. `.fork/docs/wsl.md` has the `file_path()` gates, the commit
timings, and the nvm/`.bashrc` PATH trap for an agent installed that way.

**And take the screenshot before believing `warpctrl agent read`.** Measured on
T14.6: a conversation whose panel was displaying a full error paragraph read back
through `agent read` as an exchange with **no output field at all**, because the
error renders as an error block rather than as output. The CLI is the faster
instrument and it is silent about a whole class of state.

---

## Where to read

`.fork/README.md` is the map: one line per file. The shape, laid out
2026-09-05 after the maintainer said the old single-file board served agents
and not them:

- **`.fork/GOAL.md`** — **read this first if it exists.** A dated, deliberately
  temporary horizon: what the fork is being driven toward right now and what
  "done" means as a *run*. It outranks every ticket's ordering while it stands,
  and it is meant to be deleted when met or abandoned. Absent means there is no
  standing horizon and `.fork/tickets/` is the plan.
- **`.fork/docs/`** — one page per surface, current truth, dated *as of*. Read
  the page before touching the surface: `wsl.md` before anything about a WSL
  pane, `composer.md` before anything the panel draws, `observability.md`
  before adding any capture to the event log, `classifier.md` before repeating
  the line that a model deciding permissions is not consent,
  `environment.md` before changing a `WARP_FORK_*` parser or adding a variable,
  `build.md` before re-tuning `CARGO_BUILD_JOBS` or chasing a build-memory
  number, `warpctrl.md` before touching instance lifecycle or the discovery
  registry, `agent-transports.md` before deciding something is a fork
  limitation rather than a fact about the agent you named. `manual.md` is the
  operating manual, still whole; reach for it to *use* something rather than
  change it.
- **This file is the index and those pages are the accounts.** A finding lands
  here as a *rule*, in as few lines as the rule needs, and its account goes to
  the surface page. That convention is new on 2026-09-10 and **a convention
  alone did not hold it**: the file was back over budget a day later, because
  writing the rule is cheap to remember and moving the account is the step
  that gets skipped at the end of a long session. `.fork/tools/claude-md-budget.sh`
  is the fix that isn't more prose — it runs in `script/presubmit`, fails the
  gate this repo already trusts when the file crosses a budget, and names the
  largest section to cut from. `.fork/docs/model-economy.md` has the warning
  threshold's formula and why it is 5% of the active context window.
- **`.fork/tickets/`** — the work, one file per ticket (`T01`–`T20`) and one per
  idea (`I00`–`I22`), split out of the old `TASKS.md` and `IDEAS.md` with no
  sentence changed. **Mostly historic**: read the one you are about to touch,
  and treat its "as built" parts as live. A citation of `.fork/tickets/`
  followed by a ticket number means the file starting with that number.
- **`.fork/decisions/`** — one paragraph per binding decision, dated.
- **`.fork/runs/`** — event logs, transcripts and friction logs; never edited
  after the run, except the live friction log under `GOAL.md`.
- **`.fork/archive/`** — `SPEC.md`, `CONSOLIDATION.md`, the handoffs. Finished;
  kept because the reasoning in them is not written anywhere else.

`git log` is a real source here, not an afterthought: commit bodies carry the
reasoning, including retractions.

## Commits

`fork: <lowercase subject> (Txx)`, where `Txx` is the ticket in `.fork/tickets/`
(or a surface name such as `WSL`, `COMPOSER`, `VIEWER` when the work belongs to a
page rather than a ticket). The body explains *what was found*, not what was
typed — including when it contradicts something previously recorded.
Corrections belong in the commit that makes them, and in the doc that was wrong.
