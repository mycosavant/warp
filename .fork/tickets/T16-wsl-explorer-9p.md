> Ticket T16, split out of `.fork/TASKS.md` on 2026-09-05. History; read the section you came for.

## T16 — The WSL explorer walks over 9p, and the fix is already running

**Filed 2026-09-01, from a live run plus a Fable 5.1 review.** This is the
defect the fork was started over — *"no file explorer, no diffs, no agent
feedback"* working on WSL projects — located to file and line for the first
time. Nothing here is built yet.

### What was measured, not argued

A Windows debug build at `7725c7e2b` (pid 16428) was pointed at
`\\wsl$\ubuntu\home\effatha\git\warp` and the project explorer opened.

- Index started: `23:32:51Z`, logged as *"Upgrading pending lazy-loaded path to
  fully indexed directory"*, with the root spelled **non-verbatim**
  `\\wsl$\ubuntu\...`.
- At `23:53Z` — **20 minutes** — 343 s of CPU, 587 MB RSS, and **neither**
  completion line (`Successfully indexed repository`, `local_model.rs:2025`, nor
  the budget warning at `:2019`) had been written. The skeleton never populated.

**The per-entry cost over 9p is ~20 ms, not the ~0.5 ms a plain enumeration
suggests**, because `Entry::build_tree_...` does six filesystem ops per child
(`crates/repo_metadata/src/entry.rs:359-372`, `:589-638`) and one of them,
`dunce::canonicalize` → `GetFinalPathNameByHandle`, costs **13.3 ms** by itself.

| walk shape | 2286 files | per file |
|---|---|---|
| plain `scandir` recursion | 444 ms | 0.19 ms |
| Warp-shaped, six ops per entry | 47,893 ms | **20.9 ms** |

Calibrated against Warp's own code rather than only a model: the outline
indexer has a hard 5,000-entry budget and the log shows it run `23:32:51Z` →
`23:34:32Z` — **5,000 entries in 101 s = 20.2 ms/entry**, agreeing within 5%.

### The bug: `Prefix::UNC` is not `Prefix::VerbatimUNC`

`matches_gitignores` does `path.strip_prefix(gitignore.path())` and returns
`false` on `Err` (`entry.rs:774-789`). The gitignore's root is the spelling of
the directory it was found in, and the fork's own `canonicalize_wsl_unc_path`
**guarantees** that is non-verbatim `\\wsl$\...` (`path.rs:629-644`, wired at
`working_directories.rs:1147`). Every child is `dunce::canonicalize`
(`entry.rs:372`), which returns **verbatim** `\\?\UNC\wsl$\...` and which dunce
does not strip for UNC. `std` compares prefixes by parsed variant, so the strip
fails for every child, `/target` never matches, and 410k files queue until
`MAX_FILES_PER_REPO = 200_000` runs out — **~67 minutes**. `.git` escapes only
because its check is component-based (`entry.rs:602`).

Specific to UNC roots: on `C:\` dunce strips and both sides agree; on Linux
there are no prefixes. **Exactly and only the configuration T6.4 rules on.**

Falsifier, no GUI and no filesystem — a `#[cfg(windows)]` test:
`GitignoreBuilder::new(r"\\wsl$\ubuntu\repo").add_line(None, "/target")`, then
assert `matches_gitignores(Path::new(r"\\?\UNC\wsl$\ubuntu\repo\target"), …)`.
False today; the control with a non-verbatim child is true. It proves the
matcher, **not** that the builder feeds those two spellings — that rests on
`entry.rs:372` plus T6.2's measured dunce output.

### Why the remote server does not save us — and it is running

`warpctrl remote wsl connect` succeeds and the daemon runs **natively inside
WSL** (measured: proxy, `remote-server-daemon`, `terminal-server`, the daemon
idle at 0.9% while Windows burned a quarter core). The server implements the
explorer path — `handle_load_repo_metadata_directory`
(`app/src/remote_server/server_model.rs:2157-2205`) calls the same model on
ext4, where there is no prefix problem.

It is never asked, and the reason is one hostname comparison:

1. `determine_session_type` (`app/src/terminal/model/session.rs:723-744`) →
   `Local` when bootstrap hostname == local hostname and not an SSH wrapper.
   **WSL2 inherits the Windows machine name** (measured: `effatha` both sides).
   The only route to `WarpifiedRemote` is `with_ssh_socket_path` (`:1835-1837`).
2. `set_remote_host_id` no-ops unless already `WarpifiedRemote` (`:1004-1009`).
   So connect published `host=d091df60…` and **the session discarded it**.
3. `location_for_path` (`session/active_session.rs:105-118`) yields `Remote`
   only for `WarpifiedRemote { host_id: Some }`.
4. `set_remote_root_directories` (`view.rs:911-975`) is the only writer of
   `remote_host_id` and is fed only from `Remote` entries, so the tree takes the
   local branch and `load_remote_directory` is **never entered**.

**The absent `no host_id` warning is consistent with this, not evidence against
it** — the warn sits inside a function a local root never reaches. Worth keeping:
a missing log line is only evidence if the line is reachable.

`.fork/tickets/:1499` already sells this exact outcome — *"files, terminals,
language servers and agents all live on the fast side of the 9p boundary"* —
citing the ten-minute skeleton at `:1219`. The wiring was never built.

### What to build, ranked by cost

**Boxes reconciled 2026-09-09.** (2) and (3) were built on 2026-09-02 and
this list went on saying otherwise for a week, while the same file recorded
both below — "Fix (2) measured" and "Phase 1 built and verified end to end".
A ticket that contradicts itself is the fork's commonest defect wearing its
own clothes. **(1) is genuinely still open**: the file says in as many words
that "the seam still has no automated test", so it is left unticked rather
than swept along with the two beside it.

- [x] **(1) `#[cfg(windows)]` unit test** on `matches_gitignores` — minutes,
      isolates the bug, fails today. **Ticked 2026-09-14**: it had existed since
      `2d4dcf69b` (2026-09-01), eight days before the sweep above called it open.
      `a_gitignore_cannot_match_a_child_spelled_with_a_different_unc_prefix` in
      `crates/repo_metadata/src/entry_tests.rs`, run on Windows. It passes rather
      than fails, because it pins the prefix inequality and (2) fixed the bug one
      level up.
- [x] **(2) Skip `canonicalize` for non-symlink children** at `entry.rs:371-373`.
      `read_dir` already returns the on-disk name joined to the parent's
      spelling, so this deletes the 13 ms op **and** makes child spelling equal
      root spelling by construction — one edit, fixes the bug and 65% of the
      cost. ~20 ms/entry → ~7 ms; this repo becomes ~55 s. Helps every UNC root.
      Pair with **(2a)**: canonicalize the root before `gitignores_for_directory`
      (`local_model.rs:1903`, `:1100`).
- [x] **(3) Route at the path seam.** In `location_for_path`, before the `Local`
      arm: if `session.wsl_name().is_some()` and the manager reports a host id,
      return `Remote(host_id, linux_cwd)`. ~15 lines, one file, gated on a
      connected server so nothing changes until `remote wsl connect` succeeds.
      **`SessionType` stays `Local`.** Test: WSL launch data + local hostname +
      stubbed manager → `location_for_path` is `Remote`; fails today.

**Do not reclassify the session**, though it is the "correct" model. Ranked
blast radius, all read: `maybe_convert_to_native_path` (`session.rs:1019`) stops
converting `/home/…` to UNC and every existing consumer gets a Linux path; agent
cwd special-cases `WarpifiedRemote{host_id:None}`
(`ai/agent/api/impl.rs:266,324`) and the fork's agents spawn via `wsl.exe --cd`;
`universal_developer_input.rs:129-145`; command corrections (`session.rs:555`)
and the git chip's `wsl.exe` wrapper; and `WarpifiedRemote{host_id:None}` yields
**no explorer root at all** before connect — a regression for every WSL pane.
The path-seam fix touches none of these.

**Unverified ordering hazard**: today connect landed *after* the `cd`
(`23:32:51Z` index start, `23:33:36Z` connected). Whether `SessionConnected`
re-derives display roots and drops a running local index was not traced. The fix
needs a connect-after-cd test. **The hazard was real and is fixed, noted
2026-09-14:** a pane that `cd`-ed first kept its Windows-side tree for good, and
`SessionConnected` now re-runs repo detection (`4c4c21798`). No connect-after-cd
test was found (`run_repo_detection` appears in no test file under `app/src`),
so that half is still open.

### The sidestep that needs no code

Navigate in WSL (`yazi`/`ranger`/`fd`), open with
`warpctrl file open <unc-path> --line N` — **measured 273 ms**, opens in a
Windows tab with markdown rendering. `file open` is O(1) for the read and does
not run `build_tree`. Two settings keep it that way: **keep the project explorer
closed** (the tree indexes an opened file's root only when active,
`view.rs:1561`), and **turn off codebase-symbol outlining**, whose
`RepoOutlines::index_repo` is a 5,000-entry ~100 s background 9p walk per repo
(`ai/outline/native.rs:121-146`). Not verified: whether opening a file with no
`cd` detects the repo and fires the outline — today it fired on the `cd`.

### Fix (2) measured, and the scope this ticket first claimed was wrong

**Measured 2026-09-02 on a Windows debug build rebuilt with fix (2)**, same
machine, same repository, same launch recipe:

| | before | after |
|---|---|---|
| outcome | budget exhausted, tree never rendered | **indexed and rendered** |
| wall clock | 32m50s | **~19s** |
| files | 200,000 (gave up) | **6,384** |
| `target/` | walked | italic in the tree — correctly ignored |

`Successfully indexed repository: \\wsl$\ubuntu\home\effatha\git\warp with
6384 files`, 19 seconds after the `cd`. Roughly **100x**, and the italic
`target/` is the predicted visual tell that the gitignore prefix now matches.

**And the scope written above is too wide.** This ticket's first draft said the
Windows build was unusable for WSL work. That was one measurement, on the worst
directory in the checkout, generalised. Measured since:

| root | index log lines | result |
|---|---|---|
| `$HOME` — not a git repo | **none** | populated instantly, every dotfile |
| `~/git/warp` — a git repo | `Upgrading...` | the 32m50s walk |

`local_model.rs:1136` builds a non-git root with `max_depth: 1` -- *"Only first
level"* -- and `:1952` builds a git repo with `MAX_TREE_DEPTH`. So ordinary
navigation was always lazy and always fast, and only the eager repo walk was
pathological. The maintainer's report of a month of unremarkable WSL use was
accurate the whole time and was dismissed here twice before being measured.

**This also demotes the routing work.** After fix (2) the Windows build is
usable for WSL repositories, so Phase 1 is no longer a rescue. The argument for
it is correctness: the client reaching across the boundary at all is what let a
32-minute walk hide for months, because nothing ever failed -- it was only ever
slow. Keep it on the board; stop selling it on the number.

**Read is not run, three times in one session.** `snapshot.rs` was quoted by
line and is dead code with no consumer outside its own module. A claim about
what the *agent* sees was tested by reading `/proc/<pid>/cwd`, which is the
*shell*. And the lazy/eager split above was announced as "confirmed" from a
source read before `$HOME` was actually opened. Each was caught by someone
noticing that lived experience did not match the conclusion, which is a control
worth more than another measurement of the same thing.

### Phase 1 built and verified end to end, 2026-09-02

**The file tree for a WSL repository is now served from inside Linux.** Measured
on a Windows debug build against a repo the daemon had never seen:

| | before (this morning) | after phase 1 |
|---|---|---|
| who indexes | Windows client, over 9p | **the WSL daemon** |
| daemon CPU for the load | idle at 0.9% | **110 ms of real work** |
| `local_model` index events | every load | **zero, whole process** |
| `~/git/warp` outcome | 32m50s, budget exhausted, never rendered | rendered |
| `target/` | walked | italic — ignored |

**It needed two edits, and the second only surfaced by running the first.**
Routing `pwd_as_local_or_remote` alone produced
`Repository not found: /home/effatha/scratch-t16/repo` — the tree asked the
server to load a directory for a repository nobody had told it about, because
repo detection asks `session_is_local` *independently* and still answered
`Local`, so `navigate_to_directory` was never sent. Both now go through one
predicate, `TerminalView::wsl_connected_host`.

That is the finding, not the tidy-up. **"Is this session local?" is asked in
several places, a WSL session answers yes to all of them, and its files are
somewhere else.** Any future call site that asks without the predicate
reintroduces exactly this bug, which is the concrete argument for making the
shortcut unrepresentable rather than fixing call sites one at a time.

**Incremental updates work** — chased before recording this, because recording a
broken behaviour as ground truth is worse than not recording it. A file created
in WSL appeared in the tree with no warning and 70 ms of daemon CPU. The
`No remote repository found for incremental update` warning fires only at
navigation time, when the server pushes before the client's remote model has
registered the repository. It is a race in the pre-existing remote path, newly
reachable now that WSL uses it — **not introduced here**, and superseded
immediately by the full load.

### Phases 2 and 3, 2026-09-02 — and half of phase 2 was already done

**Measured before building anything**, because T16's own scope was wrong once
already. A Windows debug build at phase 1, one WSL pane, a fresh fixture the
daemon had never seen:

| surface | routed to Linux? | how it was established |
|---|---|---|
| file tree | **yes** | 150 ms daemon CPU, zero `local_model` lines, `StandardizedPath(Unix` root in the dispatched action |
| buffer / `file open` | **yes** | README.md opened and rendered, header reading `/home/effatha/scratch-t16d/…` |
| global search (`ripgrep_search`) | **yes** | result group labelled **WSL: Ubuntu**, `target/` correctly excluded, 30 ms daemon CPU |
| git status chip | **yes** | `master ± 0`, correct for a clean tree |
| **connect after `cd`** | **no** | the pane kept its Windows-side tree permanently |
| **agent file tools** | **no** | `read_files`, `request_file_edits`, tool selection and `SessionContext` all matched `session_type()` |
| **`location_for_path`** | **no** | slash commands, skills, blocklist output, AI context |
| **`session inspect`** | **no** | nothing outside the app log could say whether routing had happened |

So four of the five things phase 2 was written to fix were already working —
they route on the `LocalOrRemotePath` variant, which phase 1 had already made
`Remote`. What was left was everything that asked `session_type()`.

### One answer, and the reason there has to be one

`session::filesystem` promotes phase 1's private `TerminalView` predicate to a
place everything can reach. It answers *reachability*, not classification:
`SessionType` stays whatever bootstrap decided, because it also drives path
conversion, agent execution context, command corrections and chips.

`SessionFilesystem::Unreachable` is a third state on purpose. A remote session
with no host is not local, and a caller treating it as local reads *this*
machine's filesystem for another machine's paths — which succeeds often enough
to be worse than failing.

The rule is split from its lookups as a pure `classify(session_type, is_wsl,
connected_host)`, so it has tests. **T16 said the seam had none because no
`RemoteTransport` double exists; that was true of the transport and was never
true of the rule.** Seven tests, calibrated by three mutations.

Six call sites moved onto it: `location_for_path`, `SessionContext` (which fans
out to agent tool selection, file edits, codebase search, remote codebase
context, the `@` menu's skill list and `read_skill`), `read_files`,
`request_file_edits`, and phase 1's two.

**Two keep `session_type()` deliberately**, with the reason written where the
next reader will hit it. The orchestration gate is about where *commands* run,
and a WSL session's shell is already native Linux — routing it would turn
subagents off for every WSL pane. And the completer already solved this its own
way in `wsl_guest_listing` (APP-3993): **upstream had independently found that
enumerating a WSL directory from Windows is wrong, and asks the guest.**

### Three things the work turned up that were not on the board

**The ordering bug, which is why phase 1 measured as working.** A pane that
`cd`-ed and *then* connected kept its Windows-side tree for the rest of its
life. Every measurement so far had connected first. `HostDisconnected` already
broadcast `RepoChanged` for the symmetric case; connecting had nothing.
`SessionConnected` now re-runs detection — detection, not just the displayed
path, because registration is what tells the server the repository exists and
phase 1 already measured `Repository not found` from routing one without the
other. Verified on a rebuilt binary: `cd` first, connect second,
`session inspect` flips to `host`, and the tree root comes back
`StandardizedPath(Unix`.

**`SearchCodebase` is withheld from a host session, and that is not a loss.**
Routing means taking the remote feature surface, including
`RemoteCodebaseIndexing`'s gate — so a WSL session stops being offered the tool.
Checked before recording it as a regression: that flag, `FullSourceCodeEmbedding`,
`CodebaseIndexPersistence` and `CrossRepoContext` are **all `DOGFOOD_FLAGS`
only**, and `warp-oss` never takes that list (only `bin/dev.rs` and
`bin/local.rs` do). The index is never built in this fork either way. The tool
was being offered over nothing.

**The instrument that was missing.** `warpctrl session inspect` now reports
`{"where": "local" | "host" | "unreachable"}`. Phase 1's whole point is that a
WSL session keeps `SessionType::Local`, so no field distinguished a routed
session from an ordinary one; confirming the routing meant reading the log for
a connect line and then inferring, *from the absence of `local_model` lines*,
that no walk had happened. This fork has been burned by inference-from-a-missing-
log-line before, and it was a fact the process simply knew.

### The registration race: right about the facts, wrong about their significance

`No remote repository found for incremental update` fired on **every**
navigation and cost one investigation last session.

It is not a race the client can close, and the reason is the interesting part.
The key an update is filed under is the repository *root*, and only the server
computes it: the client asks about a working directory, the server resolves it
to a root, indexes it, starts watching it, and names the root in the snapshot
that follows. **The watcher fires during that indexing**, so the first updates
for a newly navigated repository necessarily carry a key the client has not been
given — and pre-registering is impossible for the same reason, because the
client does not know the key either.

Dropping costs nothing because the snapshot is a complete state, not a delta.
That was load-bearing and resting on a comment, so it is now a test. Downgraded
to `debug` with a message that says it is superseded.

### Phase 3: the board said 49 call sites; the hazard is seven

Phase 3 was written as *"a path that belongs to a host should not hand out a
`&Path` that `std::fs` accepts"*, with 49 `to_local_path_lossy` sites behind it.
Audited rather than assumed, **most of that number is not the hazard**:
`Repository` models are only ever constructed by `DirectoryWatcher`, which
watches the local filesystem, so every site reached through
`Repository::root_dir()` is local by construction — outlines, MCP watchers, the
skills watchers' local fallback, diff state.

The hazard is the project explorer, because that is the surface phase 1 moved.
It calls `std::fs::remove_dir_all`, `std::fs::rename` and
`std::fs::File::create_new` on paths taken straight off tree items, and since
phase 1 a WSL repository's tree items are remote. What stood between them was
**seven separate `if !self.is_remote_item(id)` guards at the dispatch site** —
all seven correct, and one forgotten `if` on a new action away from deleting a
directory on whichever machine Warp is running on, at the same path.

`local_path_for` makes the path and the guard the same expression. The dispatch
guards stay, because they stop the whole action rather than letting half of it
run, and phase 1 already shipped one bug from two places that had to agree.

### What is pinned, and what is still only verified by running

**The seam still has no automated test, and after looking it does not belong in
`crates/integration` either.** `RemoteSessionState::Connected` holds a live
`async_process::Child` and an `Arc<RemoteServerClient>`, so a connected session
cannot be constructed without spawning one; and the harness reaches a remote
server over SSH with the gcloud SDK — no WSL path through it, and no CI host
with a distribution to attach to. A mock would be mocking the thing under test.
So the wiring is verified by running, said plainly rather than carried quietly.

What *is* pinned is the mistake, which has now happened twice — a new call site
asks `session_type()` about a file and gets `Local`:

- `every_file_that_reads_session_type_has_been_classified` walks `app/src` and
  requires every live `session_type()` reader to appear in a list with a reason.
  Thirteen entries. It ignores comments, because three files changed this phase
  mention `session_type()` only to say they no longer use it, and a guard that
  fired on those would be switched off within a week.
- `the_project_explorer_converts_a_path_for_std_fs_in_exactly_seven_places`
  counts rather than pinning lines, because a guard that fails on unrelated
  edits gets deleted.

Both calibrated by making them fail.

### The three open items, answered — and each premise was wrong

All three were closed by resolving a gate, and the first answer was wrong every
time, because **a gate here has two halves and the flag lists are only one of
them**.

#### Skills were never lost, and this ticket had already corrected itself once

The three surfaces recorded as given up by a routed session were `RepoOutlines`,
`file_mcp_watcher` and the skills file watchers, on the grounds that all three
hang off `DetectedGitRepo`, which only `detect_possible_local_git_repo` emits.

The skills watchers do not hang off it. They subscribe to `RepoMetadataModel`
and refresh on `StandingQueryResultsUpdated`; `project_skills` travels over the
remote-server protocol (`repo_metadata_proto.rs`); and
`find_project_skill_files_in_tree` carries a doc saying in as many words that it
is the shared local-and-remote path, with the filesystem scan beside it labelled
a local-only fallback. Upstream has tests for exactly this case —
`find_skill_files_in_tree_returns_remote_skill_paths_for_remote_repos` and
`test_removing_remote_project_repo_deletes_shared_cached_skill_paths`. All 41
tests in that module pass here.

**The instructive part is that the same paragraph already contained the
correction.** The entry naming skills as broken also recorded that project rules
were *not* broken, having been assumed so and then found to reach the client
through standing queries. Skills use the identical mechanism. The correction was
made, written down, and not generalised one line further.

#### The other two losses are real — and nearly got recorded as moot

`RepoOutlines` and `file_mcp_watcher` genuinely never see a routed repository.
The question was whether that costs anything here, and resolving
`FeatureFlag` list membership says it costs nothing: `AIContextMenuCode` and
`FileBasedMcp` are in **no list at all** — not `DOGFOOD_FLAGS`, not
`PREVIEW_FLAGS`, not `RELEASE_FLAGS` — so `is_enabled` resolves both false at
step 3 and both surfaces are dead however the session is routed.

That was one edit away from being written down as the answer. It is wrong.
Both flags are also entries in `app/src/features.rs`'s `enabled_features()`
behind a cargo feature, and `ai_context_menu_code` and `file_based_mcp` are both
in `app/Cargo.toml`'s `default` list. Both are **on** in every build made here.
Pinned by `the_surfaces_routing_costs_are_switched_on_in_this_build`, calibrated
by taking `ai_context_menu_code` out of `default` and watching it fail.

What the `@` menu loses is worth stating precisely, because it is not the
category. `is_active_dir_in_git_repo` asks
`DetectedRepositories::get_root_for_canonical_path`, which answers for a remote
root — phase 1 registers one. So the Code section still appears; its data source
is `RepoOutlines::get_outline`, which has nothing for that path. **The category
is offered and empty**, which is a worse failure than its absence and is
invisible to anything that checks whether the menu rendered.

#### Item 3 is moot: the flag is already on, and something else came with it

`RemoteCodebaseIndexing` was carried as a `FORCE_ENABLED` candidate. It needs no
forcing — `remote_codebase_indexing` is in `default`, so the flag is on, and
`SearchCodebase` **is** offered to a `Host` session at
`api/impl.rs:267`. The earlier note that routing withholds it is wrong.

The reason the flag lists mislead here is sharper than the pairing rule this
repository already records. `app/Cargo.toml` declares

```toml
remote_codebase_indexing = ["full_source_code_embedding"]
```

so a feature that *is* in `default` switches on one that is not. Checking
`default` for `full_source_code_embedding` says absent; `cfg!` says present.
**Membership in `default` is not the test — `cfg!` is**, and the test caught it:
the assertion that it was off failed on first run.

That matters beyond T16. `FullSourceCodeEmbedding` gates the embedding index,
whose only non-mock `StoreClient` is `ServerApi`; `generate_embeddings` sends
`Fragment { content: String, .. }` — chunks of the user's source — to Warp's
GraphQL service to be embedded by OpenAI or Voyage. `egress.rs` is a deny-list
aimed at telemetry vendors and does not cover Warp's own API.

**It is not a leak, and the reason it is not is entirely upstream's.**
`auto_indexing_enabled` defaults false, and the only writer that sets it true is
the `AllowIndexing` arm of a speedbump banner, after the user ticks "always
allow". So indexing is consented, twice. But the defence is a settings default
this fork never chose, in a fork whose thesis is that nothing leaves the
machine, and nothing in `fork.rs` says anything about it.
`indexing_that_uploads_source_stays_behind_a_default_this_fork_did_not_set`
pins the default and was calibrated by flipping it.

**Decided by the maintainer: force-disabled, indexing defaults off, and the
fallback closed too.** Three changes, and the second and third were only visible
because the first was made.

1. `FullSourceCodeEmbedding` joins `FORCE_DISABLED` — the first entry there
   where the runtime force is the *primary* removal rather than a backstop over
   a cargo feature that deletes the code. The test asserts the cargo feature is
   **on**, so the evidence for the entry sits beside it and nobody deletes the
   entry as redundant after checking `default`.

2. **The fallback was the interesting half.** `SearchCodebase` does not stop
   working: `get_relevant_files` falls back to outline search, and *that* `POST`s
   `/ai/relevant_files` with every candidate's path, its symbol names, and the
   comments written above each symbol. Building the outline is local; searching
   it was not, and nothing in the feature's name says so. It would have survived
   the force-disable untouched.

   Closed by `ai::get_relevant_files::local_rank`, and the fix has this fork's
   usual shape — **the local ranker already existed**. `warp_search_core` is the
   tantivy searcher behind the command palette, already an `app` dependency,
   with BM25, field weights and the same tokenizer the rest of the app uses. The
   whole replacement is a schema declaration and one function. Its costs are
   documented in the module: it is lexical, so a query sharing no token with any
   path or symbol returns nothing where a model would have matched a synonym,
   and the tokenizer splits `_ - / \ :` but not camelCase.

   `the_symbol_map_leaves_by_exactly_one_call_site_and_it_is_guarded` pins the
   **count** rather than the guard, for the reason `egress.rs` needed a second
   check: a backstop that covers today's call sites is a fact about today.

3. **Indexing defaults off, which took two changes because the gate is a
   disjunction.** `should_build_outlines` is `indexing_enabled &&
   (codebase_context_enabled || outline_codebase_symbols_for_at_context_menu)`,
   so `fork::codebase_indexing_default()` is consulted from both settings'
   `default_value`. Wiring one leaves the walk running through the other, with
   no error and a diff that looks finished —
   `both_halves_of_the_outline_gate_default_the_same_way` exists for that and
   was calibrated by reverting one half. The reason is not egress: with the
   embedding index gone the remaining work is local. It is that outline building
   parses up to 5,000 files of every repository the shell navigates into
   (~100 s over 9p, per T6), and upstream asks before the embedding index and
   never before this one. The settings toggle is the consent; there is no second
   prompt, because an affordance that is off until switched on has already
   asked.

**What this costs, stated rather than left to be found.** Until indexing is
switched on, the `@` menu's Code section is empty and `are_file_symbols_indexed`
is false in the agent's directory context — the latter changes nothing on this
fork's own agent paths, which never read that context. And once it is on,
`SearchCodebase` ranks lexically rather than semantically.

#### What the outline gap does not cost

On this fork's own agent paths, nothing. `acp_agent` reads `params.input`,
`session_context.current_working_directory()`, `conversation_id` and `tasks`;
it never touches `AIAgentContext`, so `are_file_symbols_indexed` — the boolean
`context_model` computes from `RepoOutlines` — is assembled and never sent.
`get_relevant_files` is a `ServerApi` call and off-thesis for the same reason.
So the loss is confined to the `@` menu, which is a person's surface, not an
agent's.


### Corrections this ticket carries

- **T6.4's verdict is a workaround recorded as a law.** Its direction survives
  on the *local* path (~2.5 min for this repo even with ignore rules fixed), but
  fix (2) gives ~55 s and routing gives ext4 speed. `.fork/docs/manual.md:485-489`
  inherits the wrong mechanism from `TASKS.md:2187-2191`, which derived
  "200,000 stats … two minutes" from a constant and from ignored directories
  rendering in italics — italics that could not have been seen on a tree that
  never rendered, and a price of one op per entry where the builder does six.
- **`app/src/code/file_tree/snapshot.rs` is dead code** — no consumer outside
  its own module. It was read during this investigation as though it were the
  live path, and its genuine laziness was used to argue the doc was wrong. The
  live path is `repo_metadata`, which walks a git repo **eagerly to depth 200**
  for non-ignored content (`local_model.rs:1952-1954`). Reading the right-looking
  file is not the same as reading the one that runs.
- **The in-process cache is real** (`file_tree_store.rs`, dropped at
  `local_model.rs:995` when no pane references the repo), which is why a working
  day feels fine and a cold start does not. Nothing is cached on disk, and 9p
  itself showed no warm-up across three passes.

---

