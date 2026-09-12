# Build, test and memory, as measured

*Current as of 2026-09-11. The **index** is the rule list in `CLAUDE.md`'s
"Working rules" section. This page is the **account** — the runs, the wrong
numbers, and what corrected them.*

*Split out of `CLAUDE.md` on 2026-09-12 with no sentence changed. This was the
single largest contiguous block in the file — 28,077 characters, one coherent
topic (build verification, test flakiness, and the multi-day memory-cap saga)
sitting inside a 95,118-character "Working rules" section that was 62% of the
whole file. `.fork/tools/claude-md-budget.sh` is what made the file's size a
number instead of an impression; this page is where the number went.*

---

**What the version sidecar cost, and why it is not an environment variable.**
Moved here from `CLAUDE.md` 2026-09-12; the rule stays there. Stamping
`GIT_RELEASE_TAG` compiles the tag into `warp_core` through `option_env!`, and
cargo rebuilds every dependent of a crate it rebuilds without checking whether
the output changed — 55 crates invalidated by one changed tag. Measured on WSL
2026-09-05, release profile: the commit that introduced the sidecar (and so
changed `warp_core`) recompiled 53 crates in 6m57s, the last cascade.

| build after | crates compiled | wall time |
|---|---|---|
| a `warp_core` change (`e8fb118ee`) | 53 | 6m57s |
| a docs-only commit (`c56fc22de`) | **0** | **0.56s** |

The second row is the whole argument: `--version` answered the new sha from a
binary whose timestamp had not moved, because nothing it was built from had
changed. Under the stamp it would have been 55 crates again.

**A live run measures the binary, not your source — check the timestamp.**
Measured 2026-08-30 and it cost a rebuild plus a wrong conclusion: a release
build was started, then a fix was written while it compiled, and the run that
followed exercised the *pre-fix* binary. The feature looked broken, the unit
test for it passed, and the gap between those two facts is exactly the shape of
a real bug — so the next twenty minutes went into the wrong place. `date -r
target/release/warp-oss` against the newest file you touched settles it in one
second. This is the read-back rule (above) applied to a build: **after any
mutation, confirm the mutation before believing the next reading**, and a
compile is a mutation with a long latency and no completion signal of its own.

**…and on Windows that timestamp check does not work, because the build is a
different checkout.** Found 2026-09-02. `C:\dev\warp` is its own clone whose
`origin` is the WSL repo, and `build.ps1` does `Set-Location C:\dev\warp` then
`cargo build` with **no sync step**. So after six commits in the WSL tree the
build ran, printed `Finished dev profile in 34.12s`, exited 0 — and produced
nothing, correctly, because *that* tree had not changed. The binary's timestamp
was 18 hours old and cargo was right.

The recorded remedy above fails here: `date -r` against "the newest file you
touched" compares a binary in one tree to a source file in another, so it reports
a stale binary every time and means nothing. **Check the commit, not the clock**
— `git -C /mnt/c/dev/warp log --oneline -1` against your own HEAD, before every
Windows run. **To sync, the command depends on which side's `git` you are
holding, and this line got it wrong until 2026-09-04 when both forms failed in
one sitting.** The `gh` remote is GitHub and only carries what has been pushed,
which needs a say-so, so it is usually behind and answers *"Already up to
date"* against the wrong source. The `origin` remote is the UNC path
`\\wsl.localhost\Ubuntu\…`, which Windows `git` resolves and WSL `git` cannot.
So from WSL, fetch by the Linux path; from PowerShell, fetch `origin`:

```bash
git -C /mnt/c/dev/warp fetch /home/effatha/git/warp dev && git -C /mnt/c/dev/warp merge --ff-only FETCH_HEAD   # from WSL
git -C C:\dev\warp fetch origin dev; git -C C:\dev\warp merge --ff-only FETCH_HEAD                          # from PowerShell
```

The launcher's own hint uses the PowerShell form because that is where it runs.

**A symlink in that checkout needs two things and Developer Mode is only one.**
`core.symlinks = false` is written into the clone's config by git at **clone**
time and is sticky, so the toggle fixes nothing already on disk. Set it true,
then re-materialise: `git reset HEAD <path>` (checkout restores from the
**index**, so after a `git rm --cached` it says *"did not match any file(s)
known to git"* while HEAD holds mode `120000`), delete the plain file, then
`git checkout -- <path>`. **Test the capability with `git`, not PowerShell** —
`New-Item -ItemType SymbolicLink` refuses under Developer Mode because
PowerShell 5.1 omits `SYMBOLIC_LINK_FLAG_ALLOW_UNPRIVILEGED_CREATE` and Git for
Windows passes it, so the stand-in fails where the real client works. Account:
`.fork/docs/manual.md`, *The skills symlink*.

`.fork/docs/manual.md` documents the clone and never says to update it, which is how
a two-tree setup reads as one tree for months. Worth stating in general: a build
that reports success and changes nothing is indistinguishable from a build that
had nothing to do, and only one of those means your code ran.

**And never `pgrep -f` a pattern your own command line contains.** From the same
session: `until ! pgrep -f "release/warp-oss"; do sleep 2; done` never exits,
because the `bash -c` running the loop has that string in its own argv and
matches itself. It waited 34 minutes for itself to die and never ran the build
it was guarding. Match on something narrower (`pgrep -f "release/warp-oss$"`),
or check for the thing you actually care about — the discovery record, a port,
a file.

**Diff test-failure membership, not counts.** Measure a same-session baseline on
a stashed tree and compare *which* tests failed. There is a known pre-existing
failure set (`gh`-dependent git tests, flaky secret-redaction globals, terminal
view) whose members vary run to run — a count that matches can still hide a
regression, and a count that differs by one is usually the flaky set.

**How wide that variation actually is, measured across six runs of `-p warp
--lib` on 2026-09-04:** 19, 20, 21, 21, 23 and 28 failures, against a union of at
least 26 distinct names (26 across the five runs that were saved to a file; the
28-failure run was read off the terminal and lost, which is its own small
lesson). So the count carries almost no information — a nine-failure swing is
the normal weather here, and a single number will either alarm or reassure at
random. The baseline has to be a *union of at least two runs*, or one flaky pass
in a single baseline run promotes an old failure to a fresh "regression". That
happened on this merge: two names showed as regressions against a two-run
baseline and were neither.

**And the family behind the swing has a name: shared login state.** The cluster
that moves is `ai::mcp::file_based_manager` (up to nine at once), and its
assertion is *"post-login activation should start TUI Warp-global servers"* —
the same global the already-listed `auth_completion_waits_for_cloud_initial_load_before_migrating`
and `test_byo_api_key_disabled_for_anonymous_firebase_user` contend over. All of
it passes at `--test-threads=1` and 3/3 in isolation. So when a whole module
fails together and then passes alone, look for a global the tests are logging in
and out of, not for a bug in that module. The two auth tests above fail
*serially* too, which is what separates the genuinely broken from the merely
ordered.

**And the same trap has a third crate in it: `-p warp_cli`.** Measured
2026-08-31, by walking into it. The empty-approvals sentence was edited in
`crates/warp_cli/src/local_control/commands.rs`; `-p local_control` and `-p warp
--lib` were run and both passed; the assertion on that exact string lives in
`crates/warp_cli/src/local_control_tests.rs` and sat **red for several hours**,
shipped in a commit whose body described the fix. Written by someone who had read
the T8.6 warning below the same day. The string is now a `pub(crate)` constant
asserted by reference rather than copied, which is the fix that survives the next
person. **`cargo check --workspace --all-targets` does not catch this** — a
stale `assert_eq!` compiles perfectly.

**Adding a `warpctrl` action? Run `-p warp --lib` too, not just `-p
local_control`.** The catalog count is pinned in *two* places: the fast one is
`catalog_has_exactly_<count>_retained_actions` in
`crates/local_control/src/protocol_tests.rs` — the number is part of the name, so
grep `fn catalog_has_exactly` rather than pasting this — and its twin is
`capabilities_advertises_the_complete_catalog` in
`app/src/local_control/mod_tests.rs`.

**`PAIRABLE_ACTIONS` is pinned by two tests, and this paragraph said it was
pinned by none.** Corrected 2026-09-01 by calibration, after the wrong version
had stood here for a day and was one step from funding a panel task to build a
test that already existed. `a_paired_device_gets_the_read_surface_and_the_safe_half_of_answering`
asserts the **whole list** against a literal slice — membership, not a count, so
it is strictly stronger than the catalog pin above — and widening the list also
reddens `saying_yes_does_not_travel_by_default_and_saying_no_does`, which holds
the consent asymmetry. Both verified by making them fail: adding
`ActionKind::AgentApprove` to the list fails exactly those two and nothing else.

What *did* go stale for two days was the **count in this file**, and no test can
pin prose — which is the whole reason this file keeps telling you to read a
number off the test. The observation was right and the mechanism invented under
it was not, the same shape as the discovery-record retraction above. T8.6 updated the first, left the second
red, and shipped — because `cargo test -p local_control` takes a second and the
app crate does not. `crates/warp_cli` holds two more guardrails: an
exhaustive `match` over the CLI enum and a list requiring every action to have a
parseable example.

**Widening a shared type — or merging upstream — is gated by `cargo check
--workspace --all-targets`, not by the binary build.** Same failure mode as
above, one level up. When the fork adds a variant to an enum or a field to a
struct that upstream also constructs, the compiler finds every site *it
compiles* — and `--bin warp-oss` compiles neither test code nor `warp_tui`.
T10.1's merge landed three such breaks that git had merged perfectly cleanly:
two new upstream TUI files matching exhaustively over `BlocklistAIHistoryEvent`
(T8.3's `ConversationSettledChanged`), and six `AgentConversationData` literals
in `crates/persistence` missing T8.3's `settled`. The persistence one **was
already red before the merge** — T8.3 shipped a required field without ever
compiling that crate's tests. A clean `cargo build` proves nothing here.

**Measured 2026-09-04, and the headline number below is wrong by 2x: a single
`rustc` on the `warp` crate holds 16.6 GB RSS, not 8.1.** Sampled every 10 s
through an uncapped release build: peak **17,028 MB in one process**, and at
that moment **exactly one `rustc` was running** — the app crate is the tail of
the graph and compiles alone. 19.8 GB stayed available, swap moved 250 MB.
This supersedes both the 8.1 GB figure and the unverified 13.7 GB one, and it
names the crate, which neither of those did.

**That makes the cap *more* defensible, not less, and it is the opposite of what
this measurement was expected to show.** At 16.6 GB per large crate against
~39 GB of guest memory, **two** concurrent large crates is the ceiling and three
is the crash — so the mechanism recorded below is right and its arithmetic was
optimistic by half. `-j 8` is safe not because eight jobs fit but because seven
of the eight are small.

**What this run does NOT establish is whether uncapping is safe, and the reason
is the trap this file already names twice.** The build compiled **4 crates** —
it was incremental against a Sep 1 tree — so `-j` was never the binding
constraint, the sampled maximum concurrency was 7, and the run says nothing
about the many-crates-in-parallel case that actually took the VM down. The
command ran correctly and answered a question nobody asked. **A clean build is
the test**, it costs 30+ minutes, and it carries the real risk; it has not been
run. Until it is, treat the cap as unresolved rather than lifted, and treat
"am I building on both sides at once?" as the question that matters more than
either number.

**Run 2026-09-11, and the cap is lifted: `-j` never stood between this build and
the wall, at any width.** Clean `target/release`, uncapped `-j 32`, 1058 crates,
7m16s. The entire parallel front summed to **9,229 MB across 21 concurrent
compilers** — less than the `warp` crate compiling **alone** (12,706 MB exact,
`/usr/bin/time -v`). `MemAvailable` bottomed at **25,871 MB** of ~39 GB, and it
did so while exactly **one** compiler was running. Same shape the `-j 8` clean
build found, now confirmed at four times the width.
`.fork/runs/uncapped-2026-09-11/`.

**The extrapolation this file carried was wrong, and how it was wrong is the
transferable part.** *"Eight jobs averaged ~470 MB each, so thirty-two is
~15 GB"* averaged the eight crates that happened to be resident when a sampler
ticked. Uncapped, 31 compilers summed to 4,893 MB — **~158 MB each**. A wider
front recruits *smaller* crates, because the big ones are the graph's tail and
compile alone whatever `-j` says. So a per-job average measured at one width
does not scale to another, and the paragraph above that reasons from one is the
shape to distrust.

**One cost, measured, and it runs the other way: uncapping raises the
single-crate peak.** 11,342 MB at `-j 8` against **12,706 MB** at `-j 32`, same
instrument, **+12%** — the jobserver bounds `rustc`'s *internal* codegen-unit
parallelism, so a wider `-j` lets one `rustc` run more of its 64 units at once.
`-j` does reach the app crate; it reaches it in the direction opposite to the
one the cap was chosen for.

**What survives untouched is the hazard that actually took the guest down**: an
uncapped WSL build *concurrent with a Windows one*. That pressure is one level
up, on the host's 64 GB, and the run above was deliberately the opposite — a
clean host with nothing else on it. **"Never build on both sides at once" is now
the only rule**, and `CARGO_BUILD_JOBS=8` is the knob for the cases where it
cannot be honoured: a remote session where a dead VM costs the link, or a desk
with something else memory-hungry running. `.fork/tools/build.sh` honours an
environment cap and is otherwise uncapped; `build.ps1` stays capped, because
this measurement was taken inside the guest and does not transfer.

**Sampled again on the post-merge release build (2026-09-04, every 10 s across
7m15s), and it confirms the mechanism while still not being the test.** Peak
15.1 GB in one `rustc`, on the `warp` crate; `MemAvailable` never fell below
**22.0 GB** of 39.2; swap moved 245 MB, which is nothing. The shape is the
finding: **the app crate compiled alone for 35 of the 42 samples**, roughly six
of the seven minutes. The `-j 8` phase at the front was eight genuinely small
crates, the largest 1.3 GB.

So *"`-j 8` is safe not because eight jobs fit but because seven of the eight
are small"* now has numbers under it, and a sharper version: for most of a build
of this shape **`-j` is not doing anything at all**, because there is only one
job to cap. The cap earns its keep in the parallel front half, which is exactly
the half this run made no demands of.

**It is still not the clean-build test, for the same reason as last time.** The
`target/release` directory was warm, so this measured the tail of the graph, not
many large crates at once. Two instrument notes worth copying. The crate count
was almost published as *"2 crates compiled"* — an artifact of piping the build
through `tail -30` and then grepping the truncated log, which is the
measuring-the-wrong-quantity error one layer down, in the *capture* rather than
the command. And the sampler recorded the **maximum** single `rustc` RSS when
the quantity that kills the VM is the **sum**; `MemAvailable` is what actually
answered the question, and it was in the sampler by luck rather than design.

**The instrument behind every number in this section counted `rust-analyzer` as
a compiler, found 2026-09-09.** `.fork/tools/memsample.sh` selected processes
with `ps -C rustc`, which on this procps is **not** an exact match — it also
selects `rust-analyzer`, and one over this workspace holds **15-16.5 GB**. Every
sample taken with an editor open added that to the totals and reported it as the
heaviest single compiler. The fix is `ps -p $(pgrep -x rustc)`.

The tell sat in the output for twenty minutes and was read past: rust-analyzer
has no `--crate-name`, so its crate column reads `?`. **A row whose `max_crate`
is `?` is not describing a compiler** — which is also why the rows above
survive, since their peak sample names the `warp` crate and a rust-analyzer
cannot.

**Re-measured the same evening on a clean build with the corrected sampler, and
the shape is the finding.** The `-j 8` parallel front — seven concurrent
compilers — peaked at a **summed 3,287 MB**, heaviest single 911 MB. The `warp`
crate then compiled **alone**, climbing 1.5 GB → **14,975 MB** over four minutes
and still rising when the build was stopped, so that is a floor and not a peak.

**The closest the machine came to the wall was while exactly one compiler was
running**: `MemAvailable` bottomed at **8,198 MB** during the single-crate
phase, against ~19,300 MB throughout the parallel one. So `-j 8` is not what
stands between this build and the edge, and no value of `-j` would be — the
ceiling is one crate that compiles by itself. What is still unmeasured is an
*uncapped* front half: eight jobs averaged ~470 MB each here, and thirty-two of
them together is the question the cap was actually chosen for.
`.fork/runs/pricefetch-2026-09-09/memsample-fixed.tsv`.

**And every number above is a 10-second sampler's, so every one of them is
low.** Measured 2026-09-09, ten builds, `.fork/runs/profile-2026-09-09/`.
Adjacent ticks near the app crate's peak swing 1.7 GB, so where the sampler
lands decides the answer: two readings of *the same build* gave 15,587 MB and
14,978 MB. **The exact peak is 15,809 MB**, and it comes from `/usr/bin/time
-v`, which reports the heaviest single descendant's RSS out of
`getrusage(RUSAGE_CHILDREN)` with no sampling window at all — calibrated three
levels deep against a program allocating a known 700 MB. Two identical
baselines under it agreed to **6 MB** and produced byte-identical binaries. Use
it for anything comparing one build to another; `memsample.sh` is still what
gives the *sum* across compilers and the `MemAvailable` trace, and it takes
`MEMSAMPLE_INTERVAL` now.

**The app crate is bounded since 2026-09-09, and it took two lines.**
`[profile.release.package.warp]` in the root `Cargo.toml` sets `debug = 0` and
`codegen-units = 64`: **15,809 MB → 11,342 MB (−28.3%)**, 378 s → 305 s, and a
binary 125 MB smaller. `opt-level = 2` (−567 MB) and `split-debuginfo =
"unpacked"` (−359 MB, and it moved 45 KB of a 777 MB binary) were measured and
refused. The argument for all four is in `Cargo.toml` beside the block, at
length, because the next person to read a two-line profile override will
otherwise assume it was guessed.

Three things from it worth having outside that comment. **`debug = 0` on a
*package* costs less than it reads** — `readelf` on the result shows full line
tables for every dependency crate and **zero entries for `app/src`**, so a
backtrace loses file/line in the app crate and keeps it everywhere else.
**`codegen-units = 64`'s runtime cost is unmeasured**, and it is the first line
to remove if a regression ever appears in the app crate; `debug = 0` alone
still holds 2,302 MB of the 4,467. And **a per-package profile override cannot
be delivered by an environment variable, silently**:
`CARGO_PROFILE_RELEASE_PACKAGE_warp_DEBUG=0` is ignored with no error and rustc
still gets `-C debuginfo=1`, while the whole-profile `CARGO_PROFILE_RELEASE_DEBUG=0`
works and reaches every crate in the graph, so toggling it costs a full rebuild
instead of one crate.

**None of which is the largest lever available.** The same crate had 22,795 MB
of headroom with no `rust-analyzer` resident and 8,198 MB with one. Killing an
editor's language server is worth 14.6 GB — three times the whole profile
sweep — so kill it before measuring anything, and report any profile saving
against that or it will look better than it is.

**The specific hazard to keep in view is not the VM's size, it is the host's.**
Windows-side tests and builds draw on the same 64 GB, so an uncapped WSL build
concurrent with a Windows build is the exact scenario that took the guest down —
and it is the one case the extra guest headroom does not help with, because the
pressure is one level up. Treat "am I building on both sides at once?" as the
question, not "how much has WSL got?".

**And that question has an answer you can run, which is the half this paragraph
was missing until 2026-09-09.** A clean build was started that day on the
strength of `free -m` *inside the guest* — 22 GB available, which reads as a
green light — while the host was at **60 GB of 64**. The maintainer said so and
that is how it was caught. The guest structurally cannot see the hazard; that
is what makes it one. From inside WSL:

```bash
powershell.exe -NoProfile -Command "$os = Get-CimInstance Win32_OperatingSystem;
  ($os.TotalVisibleMemorySize - $os.FreePhysicalMemory)/1MB"
```

Measured the same evening, after the build and the language server were killed:
host 63.8 GB total, 43.6 used, `vmmemWSL` still holding **19.5 GB** it had not
returned, and `llama-server` holding **10.2 GB** by design and permanently.
Two consequences. WSL gives memory back slowly, so the host number lags what
the guest freed, and a build started on a fresh `free -m` can be starting into
an occupied host. And **the guest's own residents have to go first** — a 15 GB
rust-analyzer beside a build is not merely pressure, it is a confound that
makes the measurement meaningless even when the build finishes.

**Cap the release build: `CARGO_BUILD_JOBS=8 cargo build --release …`.**
*(Superseded 2026-09-11 — see the resolution above. The account below is why the
cap existed and is kept because the crash it describes was real.)*
Measured 2026-08-29 on WSL: an uncapped release build **took the whole VM down**
— the guest came back at `up 1 min` with an empty `dmesg`, which is the
signature of the VM dying rather than Linux OOM-killing a process. A single
`rustc` compiling the `warp` crate holds **~8.1 GB RSS**, and cargo defaults to
one job per core (32 here), so several 8 GB-class crates reach codegen together
and exhaust the VM's 31 GiB. At `-j 8` the same build finished with 19 GiB still
free. `[profile.release]`'s own comment in `Cargo.toml` records this hazard from
the CI side — *"OOM-killing release builds"* — so it is one failure with two
faces, and the guest-side face is worse because it takes the session with it.
The host has 64 GB and had **no `.wslconfig` at all**, so the VM's 32 GB was
WSL2's default half-of-host rather than a chosen value; one now exists with more
headroom and real swap. The `autoMemoryReclaim` line in it sat under `[wsl2]`, where WSL refuses it, from the day it was written until 2026-09-07, so that half was never in force; it is under `[experimental]` now, and the swap file is on X: (`.fork/runs/wsl-move-2026-09-07/`).

**And read the two numbers above as pre-fix.** The 32 GB and the `-j 8` that was
landed on after repeated OOM crashes both describe the VM *before* the
`.wslconfig` existed. Today `memory=40GB` with `swap=16GB`, so the cap is running
with headroom it was not chosen against. A single `rustc` was sampled at
**13.7 GB RSS** on 2026-08-30 with swap at 12.8/16 GB — but **which crate that
was compiling was not verified**, so it is not a like-for-like replacement for
the 8.1 GB figure and is recorded only as evidence that 8 GB is a floor rather
than a ceiling. Re-measure properly before re-tuning the cap.

**And the prefix defeats the person's own allow rule — measured 2026-09-03.**
Claude Code matches `Bash(cargo:*)` against `cargo …` and not against
`CARGO_BUILD_JOBS=8 cargo …`; its docs say an allow rule *"won't match past an
assignment of any other variable"*. Probed at `claude-agent-acp` 0.73.0 in
session mode `default`, the bare command raised 0 permission requests and the
prefixed one raised 1. **Seven of run 2's 44 asks were exactly this** — cargo
commands the maintainer had already allowed, asked about again because this
file told the agent to type the prefix. The rule form that survives it is
`Bash(CARGO_BUILD_JOBS=8 cargo:*)`, measured 0 requests in this repo, and the
docs are silent on it; the other remedy is `jobs = 8` under `[build]` in a
cargo config, which retires the prefix and this instruction with it. Neither is
a Warp change. Account in `.fork/runs/classifier/README.md`.

**`cargo clean --release` breaks the WSL remote-development server, and it
surfaces as a network error.** Measured 2026-09-09.
`~/.warp-dev/remote-server/warp-oss` is a **symlink into
`target/release/warp-oss`** — the manual's staging recipe, because on the Oss
channel there is nothing to install from. Clean the target and the symlink
dangles, so every WSL pane's auto-connect fails at spawn with *"Response channel
closed before receiving a reply"*, which Warp's banner renders as **"Failed to
start SSH extension"** and which reads like a broken tunnel. File browsing and
code review in WSL panes fall back to 9p until a release build exists again.

Two things follow. **Before `cargo clean --release`, know that you are also
uninstalling the daemon** — and a warm `target/release` is what makes an
app-crate re-measurement cost four minutes instead of half an hour, so cleaning
is expensive twice over. And **when that banner appears, read the log rather
than the network**: `grep -i 'remote server' <log>` names the real failure on
the line above.

**Never share `CARGO_TARGET_DIR` between two checkouts of this workspace.**
Measured 2026-08-24: running a baseline in a `git worktree` with the main tree's
target directory (to save disk) left artifacts that did not match either tree,
and the damage was *silent* — the next build failed with
`no variant or associated item named CtrlCCancelsThirdPartyHarness found` and
`no field 'inviteLink' on the GraphQL type 'Team'`, both pointing at source that
was correct on disk. Worse, it invalidates verification done before it: a
`cargo check --workspace --all-targets` that passed only proved the *cache* was
consistent. Give the worktree its own target directory and accept the disk, or
measure the baseline by stashing in place.

**A cargo *feature* enabled by one dependency changes how another crate
behaves, and nothing in the diff shows it.** Found 2026-08-31 and it is the
fork breaking the fork: `agent-client-protocol`, added for T14.5, enables
`serde_json/preserve_order`. Cargo unifies features across the whole build, so
`serde_json::Map` became an insertion-ordered `IndexMap` **everywhere** —
including `drive/local_sync/format.rs`, which relied on it being a sorted
`BTreeMap` to emit stable bytes. A git-backed sync silently started producing a
different byte stream for identical content: spurious diffs and avoidable merge
conflicts, in the one feature whose whole job is to be diffable. **No line of
`local_sync` changed.**

The tripwire existed and fired: `json_payload_keys_are_sorted_not_insertion_ordered`
says in its own comment *"pinned because the workspace enabling `preserve_order`
would silently make every file's byte-stability depend on hash iteration"*. Three
tests had been red for an unknown period, and nobody had run `-p warp --lib
local_sync` after adding ACP. **The guard worked; the habit around it did not** —
and `cargo build`, `cargo check --workspace --all-targets` and every gate in this
file are all silent, because a feature flip is not a compile error.

Fixed by sorting explicitly at the seam rather than by fighting the feature: the
ACP crate needs it, unification means it cannot be turned off for one crate
anyway, and **a module that must produce stable bytes should not depend on a
global default to get them.** The general rule: if your output's byte-stability
comes from a dependency's default, pin it locally or it is one `cargo add` away
from changing.

**And the fork already knew this.** `tool_digest.rs`'s `canonical_json` sorts
explicitly, with a doc naming the hazard exactly — *"if any crate in the graph
ever turns it on, every stored digest silently stops matching and every server
reads as a rug-pull"*. Same hazard, two modules: one defended in code, one
defended only by a test. The feature got turned on and the defended one was fine.
**Swept afterwards and there is no third**: `approvals::digest_of` and `graph`'s
fingerprint both hash field-by-field over strings, so neither can be reordered.

**A build script that reads a file it does not `rerun-if-changed` is a merge
trap.** `crates/graphql/build.rs` registers a schema from
`../warp_graphql_schema/api/schema.graphql` and watched only itself, so an
upstream merge that changed the queries *and* the schema together left a stale
registration in `OUT_DIR` and failed with "no field X on type Y" against a schema
that has the field. Fixed in both graphql build scripts (T11.1). If you add one,
declare every input.

