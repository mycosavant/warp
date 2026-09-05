> Ticket T6, split out of `.fork/TASKS.md` on 2026-09-05. History; read the section you came for.

## T6 — WSL integration

User-stated high-priority feature-add. File explorer and remaining features
seamless across Windows and WSL2.

- [x] **T6.1** Scope what "seamless" means concretely; enumerate broken surfaces
- [x] **T6.2** Path translation (`\\wsl.localhost\...` ↔ `/mnt/c/...`). Reframed
      by T6.1: translation already existed. What was missing was *one spelling*
      — see "T6.2 — as built" below.
- [x] **T6.3** File explorer across the boundary — see "T6.3 — as built" below.
- [x] **T6.4** Decided: **run the Linux build when your code is in WSL, keep the
      Windows build for code on `C:`.** Both sides are now measured rather than
      argued. See "T6.4 — decided" below.
- [x] **T6.5** `warpctrl` can talk to the agent. Four actions — `agent.list`,
      `agent.prompt`, `slash.list`, `slash.run` — see "T6.5 — as built" below.
      The original framing (find a keystroke injection that carries modifiers)
      was abandoned for the better half of the same task: give the app an action
      rather than teach the harness to fake a keyboard.
- [x] **T6.6** Orchestration: one agent running several. Four more actions —
      `agent.read`, `agent.spawn`, `agent.cancel`, `agent.reveal` — plus the
      two guardrails and the local-agent tool mapping without which the first
      of them would have been decorative. See "T6.6 — as built" below.
- [x] **T6.7** Local summarization, so `/compact` works without an account.
      Found by T6.5: `/compact` submits correctly and then fails with
      `missing authentication credentials`, because summarization is an
      `AIAgentInput::SummarizeConversation` and `local_agent::handles` takes
      only `UserQuery`. Answered by running Claude's *own* `/compact` against
      the session it already holds — see "T6.7 — as built" below.

### T6.1 — as built

Scoped by running it, on Windows, with a live `bash` session in Ubuntu-WSL2,
following T1.7's method. The whole enumeration is empirical: every line below
is a thing that was watched happening, not a thing read in the source and
assumed.

**Setup.** Settings → Features → "Default shell for new sessions" → `Ubuntu`,
which is `session.new_session_shell_override` in `settings.toml`. Warp reads
the distribution list straight out of `HKCU\...\Lxss` (`terminal/wsl/model.rs`),
filtering `docker-desktop` and `rancher-desktop`; Ubuntu was offered and
launched first try. Restored sessions keep the shell they were saved with, so
the setting only shows up on a *new* tab. It was set back to Default afterwards.

**The surprise: most of it already works.** Upstream has been through this
area recently — `d46473504` routes Warp's internal `git` through `wsl.exe` for
UNC working directories, `136f451dc` matches `wsl$`/`wsl.localhost` hosts
case-insensitively, `aa873b543` removes canonicalizations that froze the UI on
WSL tabs. This is not a greenfield.

Verified working, WSL session, `cd ~/git/warp`:

| Surface | Result |
|:--|:--|
| Warpify (blocks, timing, exit codes) | works |
| cwd chip | `~/git/warp` — shell-native, correct |
| git branch + dirty count | `dev`, `± 0` — real, via `wsl.exe` |
| Code review panel | works, real diffs |
| Opening a WSL file in the editor | works (`warpctrl file open '\\wsl.localhost\...'` rendered the markdown) |

**Broken, in order of how much it costs the user.**

**(a) The project explorer looks empty for a WSL-native directory — and it is
not empty, it is unindexed, and it says nothing about that.** The root appears,
named correctly, expanded, with no children. No spinner, no message, no error.

The controlled experiment, same session, same window, one `cd` apart:

    cd ~/git/warp        -> root "warp", zero children
    cd /mnt/c/dev/warp   -> full tree, ~46 entries, instant

So it is not "WSL sessions"; it is the path. `/mnt/c/...` converts to `C:\...`
by `convert_wsl_to_windows_host_path` and everything downstream is ordinary.

**Correction, and it matters.** The first reading of this — "empty tree, a hard
failure" — was wrong, and the thing that showed it was wrong was leaving the
window open. On a later launch the same root filled in completely, about a
minute after the tab was activated, with nothing changed but time. So (a) is
the same fault as (d): the walk is running, over 9p, and until it lands the
panel is a root with no children and no indication that anything is happening.

Why it renders as an empty tree rather than a spinner is exact, in
`file_tree/view.rs`: the loading and "doesn't work in WSL" states are both
inside `if self.displayed_directories.is_empty()`. A root *had* arrived — from
the pane group's own working directories, which do not wait for an index — so
that branch was never taken and `render_file_tree` drew a root with nothing
under it. A directory that is being indexed is indistinguishable from a
directory that is empty.

That is the whole user-visible bug, and it is a small fix independent of
everything else: an unloaded root should say so.

**(b) Global search refuses outright, on the shell rather than the path.**

    Global search unavailable
    Global search doesn't currently work in Git Bash or WSL.

Same session, `cd /mnt/c/dev/warp` — an ordinary Windows directory, whose file
tree renders perfectly a panel away — and global search *still* refuses. The
gate asks what shell you launched, not what directory you are in. One line:

    app/src/workspace/view.rs
        let is_unsupported_session = is_wsl_session;

feeding `CodingPanelEnablementState::UnsupportedSession`. Three panels read it,
but only global search treats it as a hard block: the file tree
(`file_tree/view.rs`) and code review (`code_review_view.rs`) consult it only
when they have nothing to show, as a fallback *message*. That is why (a) shows
an empty tree rather than the "doesn't currently work in WSL" text — the tree
had a root, so it never reached the message.

**(c) Three spellings of one directory, in one window, at one time.**

    ~/git/warp                                          (cwd chip)
    \\WSL$\Ubuntu\home\effatha\git\warp                 (agent pane)
    \\?\UNC\WSL$\Ubuntu\home\effatha\git\warp           (code review header)

The third is a verbatim path leaking into the UI. It is also the fingerprint of
the underlying problem, and worth stating precisely, because it is not obvious
and it is the thing T6.2 has to be built on:

> `dunce::simplified` strips the `\\?\` prefix only for `VerbatimDisk`. Every
> other prefix is left alone. So `dunce::canonicalize` — which Warp uses as its
> normal-form function, in `StandardizedPath::from_local_canonicalized` and in
> `normalize_cwd` — turns `C:\dev\warp` into `C:\dev\warp` and turns
> `\\WSL$\Ubuntu\...` into `\\?\UNC\WSL$\Ubuntu\...`.

Measured on this machine with `CreateFileW` + `GetFinalPathNameByHandleW`,
which is exactly what Rust's `canonicalize` calls:

    C:\dev\warp                        -> \\?\C:\dev\warp        (dunce strips)
    \\wsl$\Ubuntu\home\...\warp        -> \\?\UNC\wsl$\...
    \\WSL$\Ubuntu\home\...\warp        -> \\?\UNC\WSL$\...
    \\wsl.localhost\Ubuntu\home\...    -> \\?\UNC\wsl.localhost\...

Two things follow, and both bite. Canonicalization is not idempotent-as-
identity for WSL paths: it is a pass-through with a prefix bolted on. And it
does not normalise case or host — `WSL$`, `wsl$` and `wsl.localhost` all name
the same directory and canonicalize to three different strings, where a drive
path canonicalizes to its real on-disk case. Any two code paths that reach the
same WSL directory by different spellings hold different map keys, for ever.
`parse_wsl_unc_path` compares hosts case-insensitively; a `HashMap<PathBuf, _>`
does not.

**(d) The first index of a WSL repo from Windows takes minutes, and can take
longer than anyone will wait.** The clean isolating experiment — a *PowerShell*
session (not WSL) whose cwd is `\\wsl$\Ubuntu\home\effatha\git\warp` — proves
the file tree is willing to index a WSL UNC path: it goes straight into a
proper loading skeleton, so this is not a refusal. It was still a skeleton
**ten minutes later**, and the git chip had disappeared meanwhile. The same
repo on the Windows disk: instant.

The 9p redirector is why, and it is measurable. Same 2247-file tree, three ways
in:

    inside WSL (native ext4)                        26 ms
    Windows disk (C:\dev\warp\crates)              101 ms
    Windows -> WSL over 9p (\\wsl$\...)           1323 ms

**13× the Windows disk, 50× native.** And Warp indexes ignored files too — the
tree renders them in italics — so the walk does not stop at `.gitignore`. The
WSL checkout is 209,644 files, of which 197,136 are under `target/` (76 GB).
`MAX_FILES_PER_REPO` is 200,000. At the measured 9p rate that budget alone is
two minutes of `stat` before anything else.

**(e) The local agent could not start at all in a WSL session — this fork's own
bug.** Found by running (d)'s sibling experiment, an agent pane in the WSL
session:

    Request failed with error: Other(Could not start `claude`. The local agent
    needs the Claude Code CLI on PATH (https://claude.com/claude-code).
    Caused by:
        The directory name is invalid. (os error 267))

`os error 267` is `ERROR_DIRECTORY`. T5's `Turn::from_request` took
`session_context.current_working_directory()` — which is deliberately
*shell*-native, so `/home/effatha/git/warp` — and handed it to `current_dir` on
a Windows process. Two faults in one message: the failure, and a first sentence
confidently blaming something else while the real cause sat in the `Caused by:`
line underneath.

Fixed in `efa59bf81`. Not by converting the path — that would start the process
and move the cost onto the 9p numbers above, and an agent is a file-reading
workload. Claude now runs *inside* the distribution, `wsl.exe --distribution X
--cd <linux path> --exec /bin/sh -lc 'exec claude "$@"' claude …`, which is the
same treatment `warp_util::git` already gives `git`, arrived at for the same
reason and with the same login-shell caveat. Four tests, on a pure function, so
the decision is assertable without a Windows host or a distribution.

**`@`-mentions work in WSL. The one report against them was the directory, not
the boundary.**

Reported from the keyboard: the `@` picker opens in both the terminal and the
agent input in a WSL session; the terminal offers files, and the agent panel
seemed to offer only folders.

It offers both. `AIContextMenu::get_categories_for_mode` picks between two
categories that share the label "Files and folders":

    if is_active_dir_in_git_repo { RepoFiles } else { CurrentFolderFiles }

— and that line is *identical* in the agent branch and the terminal branch, so
the two inputs cannot disagree about a directory. They were looking at
different ones. The terminal pane was in `~/git/warp`, a repository, so it got
`RepoFiles`: the whole recursive index. The agent pane was in `~/git`, which is
not a repository, so it got `CurrentFolderFiles`:
`std::fs::read_dir` of that one directory, which on this machine holds **39
directories and exactly one file**. Nothing filtered the files out; there was
one, and zero-state sorts reverse-alphabetically (`data_source.rs`,
`file_data_source_for_pwd`), so `Clipboard Text.txt` sorts to the bottom of a
list of folders.

Worth knowing rather than discovering: with a non-empty query, files are
deliberately ranked *above* directories — `match_result.score += 100` for
`!is_directory`. So `@` alone in a folder-heavy directory looks folder-only,
and `@` plus two characters does not.

The distinction that is actually load-bearing, and is invisible in the UI
because both categories carry the same label: `RepoFiles` is the recursive
repository index, `CurrentFolderFiles` is one non-recursive `read_dir`. Inside
a repo you can mention `app/src/fork.rs`; outside one you can only mention what
is in front of you.

**Also confirmed from the keyboard, and previously only assumed here:**
ctrl-clicking a file link in agent output opens the system file manager
(Directory Opus) on the WSL path. That is the `is_network_resource` carve-out
earning its keep — it excludes WSL UNC hosts precisely so `is_path_valid` does
not reject WSL file links, and there is an upstream test saying so. It is no
longer an assumption.

**Still not reached, and named rather than quietly skipped:** drag-and-drop
across the boundary.

**What this means for T6.2 and T6.3.** The work is not "add path translation" —
translation already exists and is used in a dozen places. It is, cheapest and
most valuable first:

1. ~~**Say that a root is loading.**~~ Done — see "T6.3 — as built" below.
2. ~~**Move the global-search gate from the shell to the path.**~~ Done — see
   "T6.3 — as built" below. The answer turned out to be simpler than "translate
   the path": search never needed the shell in the first place.
3. ~~**One spelling.**~~ Done — see "T6.2 — as built" below. `dunce::canonicalize`
   indeed could not be that function.
4. ~~**Do not present a verbatim `\\?\UNC\…` path to a human.**~~ Done, by the
   same function: keying a directory one way and displaying it another is what
   put three spellings on screen in the first place.

Note what is *not* on this list: making the index fast. Nothing in Warp can
make 9p cheap, which is why T6.4 is the more consequential decision.

**What this means for T6.4.** The decision was framed as "two working options,
and the Linux build costs llvmpipe". T6.1 puts a number on the other side of
that trade. A Warp *inside* WSL reads the files ~50× faster than a Warp on
Windows reaching in, and skips (a) through (e) entirely — there is no boundary
to be seamless across, so none of the five bugs can exist. Software rendering
is a cost paid once per frame on a machine with cores to spare; 9p is a cost
paid per file, by every index, search, diff and agent read. **T6.1's finding is
that the WSLg build is the stronger option, not the fallback** — and that the
Windows build's WSL support is worth fixing for the case where the files really
are on `C:`.

#### Verified on Windows, 2026-08-19

`efa59bf81`, Ubuntu-WSL2, warp-oss debug build. (a) and (b) reproduced by
screenshot; (c) read off three visible panels at once; (d) timed with
`Get-ChildItem -Recurse` on both sides and `find` inside the distribution; (e)
reproduced in the agent pane and fixed.

The (e) fix, verified after rebuild, in a restored WSL session at `~/git/warp`:

    /agent Run pwd and reply with just the directory path, nothing else.
           Bash
           /home/effatha/git/warp

Claude ran inside Ubuntu, in the session's own directory, and said so. Before
the fix the same prompt returned `os error 267`.

The same screenshot is also where (a)'s correction came from: the project
explorer, empty a minute earlier, was by then showing the whole WSL tree. Which
is the T5.5 lesson again — the verification screenshot is worth reading for
what it happens to contain, not only for the thing it was taken to prove.

Warp's own log was no help: the second `warp-oss.exe` of a pair takes
`warp-oss.log.recovery` when the first holds `warp-oss.log`, and that file
stayed zero bytes for the whole session. Everything above came from the UI and
from probes run beside it.

### T6.3 — as built

Item 1 of the list above: **an unread root is not an empty root.**

The panel already had a loading state and simply never reached it. Its guard is
`total_item_count() == 0`, and an unread root is not zero items — it
contributes its own header. So a root that had not been indexed yet drew
exactly what a folder with nothing in it draws: the name, expanded, no
children.

Measured rather than assumed, in the real view against the real model, with a
repository held in `IndexedRepoState::Pending`:

    total_item_count = 1
    items            = ["…/repo"]
    root entry loaded = Some(true)
    expanded          = true

That `loaded = true` was the lie the whole bug rested on.
`FileTreeEntry::new_for_directory` hardcodes it. That suits its other caller —
`remote_model.rs` fills the entry in on the next line — and suits none of the
four placeholders in `file_tree/view.rs`, each of which exists *because* the
contents have not arrived. They now build an unloaded entry, and the panel asks
whether any root has been read.

Any, not every. With a repository open in one pane and a slow root in another,
the tree that already has contents keeps showing them — and that mixed case is
the ordinary one here, where a `C:` root indexes instantly and a `~/` root does
not.

A read that fails is still a read: `IndexedRepoState::Failed` keeps the
loaded-and-empty entry so a root that cannot be indexed does not spin forever.
Only `None` — the model has not been asked yet, or the registration will be
retried — counts as pending.

#### Verified 2026-08-19

`dbe8c310b`. Three tests in `view_tests.rs`, run against the real
`RepoMetadataModel` and `DetectedRepositories`, not mocks:

| test | what it holds |
| --- | --- |
| `a_root_that_is_still_indexing_reads_as_loading_not_as_empty` | the fix. Fails without it — confirmed by reverting `create_unloaded_entry` to the old behaviour and re-running |
| `a_directory_that_is_genuinely_empty_does_not_spin_forever` | the regression the `loaded` flag exists to prevent |
| `a_root_with_contents_keeps_showing_them_while_a_sibling_loads` | why the predicate is "any", not "every" |

48 file-tree tests pass. `cargo clippy -p warp --lib` clean. Builds and links
on Windows.

**Not verified on screen.** The remaining link is `render_file_tree`'s two-line
branch into `render_loading_state` — read, not watched. Reproducing it live
needs a cold WSL repository in a WSL session, and the attempt ran aground: the
running window had cached metadata for every root it was pointed at, `warpctrl
tab activate` does not take an index, and the `cd` landed in an agent pane's
steering input instead of a terminal. Recorded here rather than papered over,
per T1.7. Next hands-on session should catch it by launching with the shell
override set to Ubuntu and `cd`-ing into a repository Warp has never indexed
(`~/git/lapce`, 12,372 files, is the one to use).

One thing that attempt did surface, unasked: steering a *resumed* local-agent
turn fails with `No deferred tool marker found in the resumed session`. That is
the already-recorded `--input-format stream-json` gap wearing a different face.

#### Item 2 — global search was refusing the shell, not the path

The list said "move the gate from the shell to the path". The gate turned out
not to need a path at all.

Global search is in-process ripgrep over `search_roots`
(`GlobalSearch::run_warp_ripgrep_cli` → `warp_ripgrep::search::search_streaming`)
— filesystem I/O and nothing else, with no shell anywhere in it. And
`left_panel.rs` hands global search and the project explorer **the same**
`active_directories`, so the two panels cannot disagree about what is there.
Which is exactly what T6.1 watched happen: the tree rendering a directory
perfectly while the search panel refused it, in one window.

So the refusal was wrong twice. A WSL session in `/mnt/c/...` is looking at a
Windows directory and searches at full speed. And a WSL session in `~/...`
searches correctly too — measured from Windows, same query, same repo:

| root | time | matches |
| --- | ---: | ---: |
| `C:\dev\warp` | 0.12 s | 40 |
| `\\wsl.localhost\…\git\warp` | 9.52 s | 40 |
| `\\wsl.localhost\…\git\lapce` (12,372 files) | 17.16 s | 54 |

Identical results, ~39× the wall clock, no benefit from a warm cache on the 9p
side. Results stream in batches, so slow and correct beats a wall.

The decision moved out of `render` into `blocker`, a pure function over the
enablement state and whether any root arrived — testable without a window.
`UnsupportedSession` now blocks search only when there is no directory at all,
and the message says that. The old one — "Global search doesn't currently work
in Git Bash or WSL" — was wrong about WSL and had never been true of Git Bash
either: the state is set from `Session::is_wsl`, and nothing else.

Scoped to global search on purpose. The file tree and code review read the same
`UnsupportedSession`, and the shared seam at `workspace/view.rs:17484` is still

    let is_unsupported_session = is_wsl_session;

Moving that moves three panels at once and needs its own evidence.

#### Verified on Windows, 2026-08-19

`68ec5d437`, debug build, restored WSL session. The session is genuinely WSL and
genuinely in a Linux-native directory:

    uname -sr; pwd
    Linux 6.18.33.2-microsoft-standard-WSL2
    /home/effatha/git

With `warpctrl surface global-search open`, the panel renders its ordinary zero
state — "Search in files across your current directories" — where T6.1 recorded
"Global search unavailable / doesn't currently work in Git Bash or WSL" for this
same session.

**Not verified: a query typed into the live box.** `warpctrl` has no action for
entering a search query (`surface.global_search.open` is the only search action
in the catalogue), the panel's query editor was occluded, and raising the window
would mean stealing focus from the user's own desktop. What the query *would*
do is the ripgrep table above, measured directly against the same roots.

Five tests over `blocker`, including that remote sessions are unchanged.

### T6.2 — as built

Items 3 and 4 of T6.1's list, which turned out to be one function: a directory
that is *keyed* one way and *displayed* another is how three spellings ended up
on screen at once.

#### Item 3 — canonicalization normalizes a WSL path to nothing at all

Canonicalizing is supposed to give one name to one directory. Measured with
`CreateFileW` + `GetFinalPathNameByHandleW`, which is exactly what Rust's
`canonicalize` calls — one directory, every spelling Windows accepts:

| input | `GetFinalPathNameByHandleW` | opens |
|:--|:--|:--|
| `\\wsl$\Ubuntu\home\…\warp` | `\\?\UNC\wsl$\Ubuntu\…` | yes |
| `\\WSL$\Ubuntu\home\…\warp` | `\\?\UNC\WSL$\Ubuntu\…` | yes |
| `\\wsl$\ubuntu\home\…\warp` | `\\?\UNC\wsl$\ubuntu\…` | yes |
| `\\wsl$\UBUNTU\home\…\warp` | `\\?\UNC\wsl$\UBUNTU\…` | yes |
| `\\wsl.localhost\Ubuntu\…` | `\\?\UNC\wsl.localhost\Ubuntu\…` | yes |
| `\\wsl.localhost\ubuntu\…` | `\\?\UNC\wsl.localhost\ubuntu\…` | yes |
| `\\WSL.LOCALHOST\Ubuntu\…` | `\\?\UNC\WSL.LOCALHOST\Ubuntu\…` | yes |
| `\\wsl$\Ubuntu\home\…\WARP` | — | **no**, `ERROR_FILE_NOT_FOUND` |
| `C:\dev\warp` | `\\?\C:\dev\warp` | yes |

Seven spellings of one directory, seven distinct "canonical" strings. A drive
path normalizes to its real on-disk case; a WSL path normalizes to nothing —
it is a pass-through with a prefix bolted on.

That last-but-one row is the boundary, and it is why this cannot be a blanket
`to_lowercase`: **the Linux path components are case-sensitive.** `…\git\WARP`
does not exist where `…\git\warp` does. The host and the distribution are
case-insensitive; everything after them is not.

`canonicalize_wsl_unc_path` (`warp_util::path`) is the part `dunce` cannot do,
because only Warp knows the host is the local WSL redirector rather than a
machine on the network. Host and distribution fold to lower case, the Linux
path is left exactly as given. Wired at both normal forms
(`StandardizedPath::from_local_canonicalized`, `normalize_cwd`) *and* at the
producer (`convert_wsl_to_windows_host_path`, which emitted `\\WSL$\` before),
so a path that reaches a map without passing through canonicalization is still
the same key.

Folding the distribution is safe past the filesystem too: `wsl.exe
--distribution ubuntu` and `--distribution UBUNTU` both start Ubuntu, so
`git.rs`, which parses the distribution back out of the path, is unaffected.
It already compared distributions with `eq_ignore_ascii_case`; this only makes
the map keys agree with a decision the tree had already taken.

#### Item 4 — the verbatim spelling is not a path you can hand back

Same function at the display seam (`user_friendly_path`, which every path Warp
shows already goes through), on purpose rather than a second one.

Not only cosmetic. `\\?\UNC\…` is what `dunce::canonicalize` returns, leaked —
nobody typed it, and it does not generally work if copied back out:

    cmd /c dir "\\wsl$\Ubuntu\home\effatha\git"        -> OK
    cmd /c dir "\\?\UNC\wsl$\Ubuntu\home\effatha\git"  -> "UNC paths are not supported"

PowerShell accepts both. So the fold is the difference between a string the
user can paste somewhere and one that fails when they do.

#### A third spelling, found by running it: PowerShell reports a *location*

Not on T6.1's list, because T6.1 never put a PowerShell session in a WSL
directory long enough to read its window title. `(Get-Location).Path` is
provider-qualified, and for a UNC path the qualifier is part of the string:

    C:\dev\warp        -> C:\dev\warp
    \\wsl$\Ubuntu\home -> Microsoft.PowerShell.Core\FileSystem::\\wsl$\Ubuntu\home

Warp took it literally. The most direct evidence is the OS window title, which
is set from the same string:

    MainWindowTitle: Microsoft.PowerShell.Core\FileSystem::\\WSL.LOCALHOST\Ubuntu\home\effatha\.clau…

In the chip log the only three working directories ever recorded were
`C:\Users\onemind`, `C:\dev\warp`, and the qualified WSL one — drive paths come
back bare, every UNC path carries the prefix.

Fixed in the bootstrap (`app/assets/bundled/bootstrap/pwsh.ps1`) with
`$PWD.ProviderPath`, falling back to the qualified form on the non-filesystem
drives (`Env:`, `Function:`) where `ProviderPath` is empty — Warp cannot
canonicalize either, and a literal `Env:\` is a better thing to hand it than an
empty string. Two call sites, both of which pass the string on: the `pwd` in
the precmd message and the window title.

Deliberately not changed, having checked rather than assumed: the node-version
cache key, which only compares the string with itself, and the inner runspace's
`Set-Location`, which round-trips through PowerShell's own location parser.
`Get-Item -LiteralPath` accepts the qualified form and returns a clean
`FullName`, so the node chip's directory walk was never affected.

#### Verified on Windows, 2026-08-20

`746bbc1ab`, debug build. The isolating experiment is two panes of one tab in
**one** directory reached by **two** spellings — `\\WSL.LOCALHOST\Ubuntu\…` in
the left pane, `\\wsl$\Ubuntu\…` in the right — with the project explorer open.

| | project explorer |
|:--|:--|
| without the fold | **`t6repo` twice**, each with its own `.git` and `a.txt` |
| with the fold | `t6repo` once |

That is the symptom T6.1 predicted from the source ("any two code paths that
reach the same WSL directory by different spellings hold different map keys"),
watched happening and then watched stopping.

The counterfactual build is worth describing because the confound is real:
reverting the whole commit would also revert the PowerShell fix, and *then*
neither pane produces a root at all, which proves nothing about spelling. So
the counterfactual build kept the new `pwsh.ps1` — read from disk at runtime in
a debug build, since `rust-embed`'s `debug-embed` is enabled only for wasm — and
put back only the four Rust files. Two spellings were the only variable.

Item 4 in the same window, in one line of the block header. The command typed
was `Set-Location '\\WSL.LOCALHOST\Ubuntu\home\effatha\…'`; what Warp renders
above the block is

    \\wsl$\ubuntu\home\effatha\.claude\jobs\9f032504\tmp\t6repo git:(main)

— host folded, distribution folded, `.claude`/`9f032504`/`t6repo` untouched, no
provider prefix, no `\\?\UNC\`. Before the fix the same element read
`Microsoft.PowerShell.Core\FileSystem::\\WSL.LOCALHOST\Ubuntu\home\…`.

Incidentally confirmed, having been broken in the same session: the git branch
chip and the diff-stats chip. `ShellGitBranch` went 11 executions / 11 failures,
all `phase: value`, `status: failure`, empty stdout, `exit_code: <none>`; after
the fix, `status: success` / `* main` in the same directory, and the panel shows
`⎇ main` and `1 ● +1 −1`. **The mechanism is inferred, not traced:** the absent
exit code is the shape of a process that never spawned, which fits Warp handing
the provider-qualified string to a child process as its working directory, but
the chip execution path was not read.

**Not verified: a WSL *session*.** Everything above is a PowerShell session
sitting on a WSL UNC path, which is what exercises `normalize_cwd`. The WSL-
session route runs through `convert_wsl_to_windows_host_path` instead — covered
by tests on Windows, not on screen. Reaching it needs the Settings shell
override (T6.1's recipe), which is not in `warpctrl`'s allowlisted settings.

Ten new tests; 107 pass in `warp_util` on Windows against 103 on Linux, the
difference being the `#[cfg(windows)]` ones. Three existing expectations moved
to the canonical spelling, which is the point of changing the producer.

#### And the last surface T6.1 named: drag-and-drop

Dragging a file out of Explorer's `\\wsl$\Ubuntu\…` view into a WSL session
inserted `//wsl$/Ubuntu/home/…`, because `convert_windows_path_to_wsl` knows
only about drive letters and swaps separators for everything else. Linux
collapses the leading `//`, so the shell looks for `/wsl$/Ubuntu/…`:

    $ ls '//wsl$/Ubuntu/home/effatha/git/warp/.fork/docs/manual.md'
    ls: cannot access ...: No such file or directory
    $ ls /home/effatha/git/warp/.fork/docs/manual.md
    /home/effatha/git/warp/.fork/docs/manual.md

The answer was already in `parse_wsl_unc_path`. What it needed was the
session's distribution, which `Session::windows_path_converter` could not
supply because it returned a bare `fn` pointer with nowhere to put one; it now
returns a boxed closure. A path in another distribution falls through to the
generic conversion, there being no path from inside one distribution to
another's filesystem.

Both drop seams covered — the terminal and the rich input — and the
session-level test was confirmed to fail without the fix. **Not verified on
screen, and it cannot be from here:** winit takes file drops through OLE
`IDropTarget`, which needs a real drag over the window rather than a message
that can be posted to it.

### T6.4 — decided

**Run the Linux build when your code is in WSL. Keep the Windows build for
code on `C:`.** One thing is untested rather than working: the local agent in
the Linux build — see the end of this section for what that does and does not
mean.

T6.1 argued this from the 9p numbers. What was missing was the other half —
whether the Linux build is actually usable at *current* code, since T1.11
verified it five commits and two sessions ago. Checked by running it at
`342867ee6`, under WSLg with the documented `env -u WAYLAND_DISPLAY
LIBGL_ALWAYS_SOFTWARE=1`:

| | |
|:--|:--|
| `window list` | `has_workspace: true` straight from launch |
| UI | renders fully; no grey rectangle, no onboarding wall (the flag persisted) |
| Project explorer | the whole `~/git/warp` tree, ignored dirs in italics |
| git chip | `⎇ dev  1 ● +98 −8`, native, no `wsl.exe` wrapper |
| Global search | **`blocker` typed into the live box → 173 results in 41 files** |

That search result closes the gap T6.3 recorded and could not close: "a query
typed into the live box". It could not be done on Windows, where `warpctrl` has
no action for it and raising the window would steal the user's focus. Under
WSLg it is reachable, because XTEST posts to a specific window: `warp-xin.py
click 345 113`, then one `key` per character. `_` needs a modifier and came out
as `-` on the first attempt, which is its own small proof that the box is live —
`wsl-unc` returned "No results found" and `blocker` returned 173.

And the number that decides it. Same repository — `~/git/warp`, 209,644 files —
indexed from a `cd`:

| Build | Time to a populated project explorer |
|:--|:--|
| Windows, over 9p (T6.1(d)) | still a skeleton at **10 minutes** |
| Linux, native ext4 | **populated at the first capture, 10 s** |

10 s is an upper bound, not a measurement: the first screenshot was taken at
t+10 s and the tree was already there. Two caveats stated rather than buried —
the page cache was warm from building in that tree, and the poll interval is
coarse. Neither dents a ratio of at least 60×, and T6.1's controlled tree walk
(26 ms native / 101 ms Windows disk / 1323 ms over 9p, same 2247 files) is the
clean version of the same comparison.

What the Linux build costs, all of it named:

- **Software rendering.** Measured in T1.11, not assumed: 0% CPU at idle, ~280%
  of one core while painting 50,000 lines of scrollback, back to zero within
  two seconds.
- **X11, not Wayland.** With `WAYLAND_DISPLAY` set the window is created and
  never paints. Unsetting it routes through Xwayland and works.
- **A separate profile.** Settings, themes and the Drive store do not carry
  over from the Windows install; T1.11 saw the Linux profile report 0 objects.
  This is the real switching cost, and it is a one-off.

**The local agent in the Linux build: still untested, and the first answer here
was wrong.**

What was written first — and committed — was that the agent "does not work in
the Linux profile", on the evidence that every route to it did nothing:
`warpctrl input submit` ran the prompt in bash; typing it into a
`tab create --type agent` composer and pressing Return ran it in bash
(`Command 'what' not found, did you mean: chat / phat / jhat / wham`);
`/agent …` was not intercepted; `surface agent-management open` returned `ok`
and rendered nothing. The hypothesis attached to it was that the Linux profile
has its own store and had never been through this fork's agent setup.

**The control disproves it.** Running the identical sequence on the *Windows*
build — where the local agent is known to work — produced the identical
failure, PowerShell's version of it: `what: The term 'what' is not recognized
as a name of a cmdlet`. Nothing about the Linux profile was being observed. The
sequence was simply wrong.

The route is `input replace` to put the prompt in the composer **without**
running it, then **`ctrl`+`shift`+`Return`** — the chord the agent tab's own
header advertises and which had been read past four times. On Windows that
works, and the fork's local agent answers:

    /agent what is 6 times 7 answer with only the number
           42

On Linux it still could not be sent, for a reason that says nothing about the
build: XTEST delivers plain keys to Warp — `Return` submits, and every
character of the search query above arrived — but a *modified* key does not
register with Warp's keybinding matcher, with 0.03 s or 0.2 s modifier holds.
So the agent is **untested in the Linux build**, and there is no evidence
either way. The blocker is the injection tool.

What is established, by inspection rather than by running: the Linux build
takes the *simplest* agent path. `spawn_for` runs plain `claude` with the
working directory when there is no distribution to cross, which is exactly the
Linux case; the whole `wsl.exe --distribution … --cd …` wrapper T6.1(e) had to
add exists only because Warp-on-Windows is outside the distribution. `claude`
2.1.234 is on `PATH` at `~/.local/bin/claude`, and `WARP_FORK_LOCAL_AGENT=1`
was set on the launch. Every ingredient is present and none of it was watched
working.

The generalisable bit, and the reason this is written up rather than quietly
fixed: **a failure observed only in the new environment is not evidence about
the new environment until it has been tried in the old one.** Four symptoms all
pointed at "the Linux profile", and all four were the harness.

And a correction to something this file has implied twice, now that the Linux
side has been tried properly:

> **`warpctrl` can open any surface but can only type into the terminal.**
> `input insert|replace|submit` all reach the terminal input. There is no
> action that enters a global-search query and none that sends a prompt to the
> agent composer — `action list` has `surface.conversation_list.open` and
> `surface.agent_management.open` and nothing that writes. On Windows that is
> the end of it. Under WSLg it is not: XTEST posts to a chosen window without
> touching focus, so `warp-xin.py` gets past it, which is how the search query
> above was finally typed. The wall is `warpctrl`, not the app.

**Why this is not "the Windows build is a failure".** Software rendering is a
cost paid once per frame on a machine with cores to spare. 9p is a cost paid
per file, by every index, search, diff and agent read. The Windows build is
the faster one whenever the files are on `C:` — 101 ms against 1323 ms, the
same comparison pointing the other way — and T6.2 and T6.3 were worth doing
for exactly that case. What changed is which one is the fallback.

### T6.5 — as built

`warpctrl` could open every surface and type into exactly one of them, so an
agent driving it could start a shell command and nothing else. Four actions,
all on seams that already existed:

| | |
|:--|:--|
| `agent.list` | live conversations: id, title, status, `is_busy` |
| `agent.prompt` | a prompt to a new conversation or an existing one |
| `slash.list` | the registry, with `is_orchestration`, `is_available`, `submits_prompt` |
| `slash.run` | any slash command, argument and all |

**`agent.prompt` addresses a conversation, not a pane.** That is the unit work
is handed to: the pane can be split, moved between tabs, or closed underneath
it. It returns the conversation id, which the keyboard path throws away — an
orchestrator that started three agents has no other way to tell them apart.

**`slash.run` is one action for the whole registry** because
`Input::execute_slash_command` is one function for the whole registry.

#### The allowlist

The user's call, taken as a question rather than assumed: orchestration
commands run freely, everything else needs `force`. `/exit` and `/logout` sit
in the same registry as `/compact`, and an agent should not end its own session
by mistyping a command name.

An allowlist rather than a deny-list, because the registry is upstream's and
grows: a command added tomorrow is excluded by default. A test asserts that
property rather than trusting it — it walks every command the running build has
and fails if one is admitted that is not on the list.

29 of 63 commands are admitted in this build. Off the list on purpose:
`/clear` (discards a conversation, no undo), `/auto-approve` (an agent widening
its own permissions is a decision for a person), and the account and appearance
verbs.

#### Four things found by running it

**1. `input.submit '/agent …'` was never going to work,** and neither was any
amount of cleverness about it. `input.submit` runs its text as a command.

**2. The registry stores names with the slash** — `StaticCommand::name` is
`"/compact"`, not `"compact"`. The first version stripped the slash from the
caller's input only, so every lookup missed.

**3. Which commands exist is a property of the build, not the source.**
`/compact` is behind `SummarizationConversationCommand`; `/queue`,
`/fork-from`, `/rewind`, `/profile`, `/host`, `/harness`, `/environment` behind
their own flags. In a unit-test process none of them are registered at all,
which is why the allowlist is tested against `SlashCommandKind` rather than
against the registry. It is also why `slash.list` exists.

**4. A staged prompt is not a sent prompt.** The first `agent.prompt` created
the conversation, returned its id, reported `in_progress` — and left the text
sitting in the composer. Every JSON field said success; the screenshot did not.
`try_enter_agent_view` asks the *origin* whether to submit or stage, and
`AgentViewEntryOrigin::Input` answers "only if already in agent view", which
from `warpctrl` is never. `AgentViewEntryOrigin::Cli` is `Always`, and is the
case this already had a name for.

#### And `/compact`, which is the one that does not work

`slash.run compact` reported `handled: false`. Twice, for two different
reasons, and the second is the interesting one.

First it was availability — a property of the *pane*, not the build. That is
now visible (`slash.list` reports `is_available` against the pane you target)
and `slash.run` refuses up front instead of executing into a `false`. Three
failure modes, distinguishable, which is what an orchestrator can act on:

    not in this build   invalid_params            -> slash.list has the list
    not in this pane    target_state_conflict     -> target another pane
    not allowlisted     insufficient_permissions  -> re-run with force

Then it was still `false`, and upstream's own comment says why:

> Some slash commands (e.g. /plan, /compact) return false to indicate the full
> text should be sent as a regular AI query — fall through in that case.

`/compact` is not an action. It is a prompt whose *prefix* is the instruction.
`slash.run` now falls through the same way, reconstructing `/compact
<argument>` into the conversation the pane is showing.

And then it failed for real:

    I'm sorry, I couldn't complete that request.
    Request failed with error: Other(missing authentication credentials)

Summarization is `AIAgentInput::SummarizeConversation`, and
`local_agent::handles` takes only `AIAgentInput::UserQuery` — deliberately, per
T5: "silently answering them from a local model would be worse than not
answering". So the request goes upstream, and this fork has no account. **The
plumbing is right and the feature is absent.** Tracked as T6.7.

#### Verified on Windows, 2026-08-20

`fa6f25092`, debug build, every claim from the CLI with no keyboard involved.

    warpctrl agent prompt 'Reply with exactly one word: warpctrl-ok'
      -> conversation_id 84ee4216…, created: true
      -> on screen: /agent Reply with exactly one word: warpctrl-ok
                    warpctrl-ok

    warpctrl agent prompt 'Now reply with exactly: second-turn-ok' --conversation 84ee4216…
      -> created: false, and the same conversation now has two turns

    warpctrl agent list      in_progress -> success, title taken from the prompt
    warpctrl slash run logout
      -> insufficient_permissions: refused: `logout` is not an orchestration
         command. Re-run with force if you meant it.
    warpctrl slash run copy-debugging-id --force
      -> handled: true, and the toast appears
    warpctrl slash run not-a-real-command
      -> invalid_params, naming `slash.list`

Handoff, which is the point of all of it:

| Target | Result |
|:--|:--|
| New tab | `tab.create` then `agent.prompt` — works |
| New pane | `pane.split` then `agent.prompt` — works; agent right, terminal left |
| New window | `window.create` then `agent.prompt` — same composition, untested |
| Background | not attempted; see T6.6 |

Ten tests. `warp_util` unaffected; `local_control` 40, `warp_cli` 244.

### T6.6 — scope

**One agent running several, from `warpctrl`.** T6.5 makes an agent
addressable; this makes a fleet of them manageable. The finding that shapes it:
**almost all of the machinery already exists**, because `/orchestrate` needs
it. What is missing is the `warpctrl` surface onto it.

**Background agents are real and already have a name.**
`HiddenPaneReason::ChildAgent`, in `pane_group/tree.rs`:

> Pane is a child agent spawned by an orchestrator. It stays hidden until the
> user explicitly reveals it from the status card.

That is exactly the "create the surface but set the pane to hidden" idea, built
and in use. `create_hidden_child_agent_conversation` takes a
`HiddenChildAgentConversationRequest` — parent pane, name, parent conversation,
harness, env vars, task context — and returns the conversation. The reveal side
exists too: `Event::RevealChildAgent`, `OpenChildAgentInNewTab`,
`OpenChildAgentInNewPane`.

So the work is wrapping, not inventing:

1. **`agent.spawn`** — a child conversation under a parent, with a name and a
   prompt. `--background` uses the hidden pane; `--pane`, `--tab`, `--window`
   place it visibly. All four targets the user asked for, and three of them are
   already reachable by composing T6.5 with `pane.split` / `tab.create` /
   `window.create` — `agent.spawn` is the one that makes the fourth possible
   and the other three atomic. Carries the guardrails below: `--allow-tools`
   and the depth cap.
2. **`agent.reveal <conversation> [--pane|--tab]`** — the toggle. Three events
   already exist for it.
3. **`agent.cancel <conversation>`** — stop a turn. An orchestrator that cannot
   stop a runaway child is not in charge of it.
4. **`agent.read <conversation>`** — the transcript, or the last message. The
   gap that makes the rest hard to use: `agent.list` says *that* a conversation
   finished, never *what it said*. Handing work back needs the answer.
5. **`agent.list` should report the pane and tab.** Left `None` in T6.5 — the
   fields are in the protocol and unpopulated — because it needs the terminal
   surface id mapped through the pane group. Cheap, and it is what makes
   "reveal the one that is blocked" possible.

#### Guardrails: what a child agent is allowed to do

Decided with the user, 2026-08-20, and the reason both are needed is that
**there are two different spawn paths and one lever does not cover both.**

**1. A tool allowlist per child.** The seam exists and is already load-bearing:
`RequestInput::with_supported_tools(Vec<ToolType>)` sets
`supported_tools_override`, and `generate_multi_agent_output` uses it *instead
of* `get_supported_tools`. There is exactly one caller today — passive
suggestions, which are read-only — so the mechanism is proven and has room for
a second consumer.

The vocabulary is `ToolType` in `task.proto`, 34 entries. The safety-relevant
ones are the obvious ones: `RUN_SHELL_COMMAND`, `APPLY_FILE_DIFFS`,
`EDIT_DOCUMENTS`, `CREATE_DOCUMENTS` are the write half; `READ_FILES`, `GREP`,
`FILE_GLOB`, `SEARCH_CODEBASE` are the read half. So "read-only child" is
expressible exactly, which is the case the user named.

**2. A spawn-depth cap, configurable.** Because the tool list only governs what
the *model* may do. `warpctrl` itself is a second spawn path, and a lead agent
that can run `warpctrl agent spawn` can run it in a loop regardless of its own
tool list. The cap is the backstop for the path the allowlist cannot see.

Note which of these is the stronger control, because it is not the obvious one:
**`SUBAGENT` and `RUN_AGENTS` are entries in the tool list.** Withholding them
from a child forbids further fan-out at the point the request is built, which
is a harder guarantee than a counter someone has to remember to increment. The
depth cap is the belt; the tool list is the braces, and the braces are load
bearing.

**The trap, and it would have shipped a guardrail that does nothing.** In this
fork the local agent intercepts *before* the tool list is read:

    generate_multi_agent_output(...)
        if local_agent_enabled() && local_agent::handles(&params) {
            return local_agent::generate(...)      // <- returns here
        }
        let supported_tools = params.supported_tools_override.take()...

So a tool allowlist set on the Warp side is silently ignored for every request
the local agent answers — which, in this fork, is every plain user query. The
child would be told it is read-only and would have a shell.

The fix is available rather than theoretical: `claude` takes `--allowedTools`,
`--disallowedTools`, `--tools` and `--permission-mode`, so the local agent can
honour the same restriction once the vocabulary is mapped. The safety-relevant
correspondences are clean — `RUN_SHELL_COMMAND`↔`Bash`, `READ_FILES`↔`Read`,
`APPLY_FILE_DIFFS`↔`Edit`/`Write`, `GREP`↔`Grep`, `FILE_GLOB`↔`Glob`,
`SUBAGENT`↔`Task` — which is what matters, since a guardrail only needs to be
exact about the things it forbids.

**These two ship together or not at all.** A tool allowlist that the local
agent ignores is worse than no allowlist, because it reads as a guarantee.

**And it is a guardrail, not a sandbox.** It stops the model *calling* a tool.
It does not stop a long-running shell command that a tool already started, and
it is not a boundary against a determined prompt injection — the child is still
a process on this machine with the user's credentials. Worth saying plainly in
the docs when this ships, because "read-only agent" invites the stronger
reading.

**What is *not* in scope**: making `/compact` work, which is T6.7 and about
where summarization runs, not about `warpctrl`. And dependency chains between
handoffs, which is T7 and is not a `warpctrl` feature at all.

### T6.6 — as built

Four actions, two guardrails, and one mapping that had to ship with them.
`warpctrl` now has 96 actions; `agent` has six.

| | |
|:--|:--|
| `agent.read` | the transcript, or the last N exchanges |
| `agent.spawn` | a child agent in a hidden pane |
| `agent.cancel` | stop a turn |
| `agent.reveal` | put a hidden child on screen |

and `agent.list` now fills in `pane_id`, `tab_id` and `is_hidden`, which T6.5
left as `None`.

#### `agent.read` was the piece everything else needed

`agent.list` reports *that* a conversation finished and never *what it
produced*, so before this an orchestrator could dispatch work, watch it
complete, and have no way to collect the result. That is why T7.1 was blocked
on T6.6 rather than on T6.5: a graph without this can sequence work but cannot
hand anything along it, which is half the point.

Built on `AIConversation::root_task_exchanges` and the two formatters the
copy-to-clipboard path already uses, so it reports the text a person would get
from the overflow menu.

Input and output are **separate fields**, not one `USER:`/`AGENT:` transcript.
The caller is a program; the thing it wants is the last `output`, and making it
parse a formatted transcript to find that would repeat the mistake
`input.submit` makes with `/agent`.

Tool results are **off by default**, and `included_tool_results` reports what
actually happened rather than echoing the request — they need the action model
of the surface that owns the conversation, and that surface can be closed while
the conversation survives.

#### Hidden panes: two questions, two answers

`pane.list` reports `visible_pane_ids` and is right to — a hidden pane is not
addressable as a pane. But "which pane holds this conversation" is a different
question, and for a background child the answer is a real pane that happens to
be hidden. So `agent.list` walks `pane_ids()` and reports visibility as a
field. Verified: with six conversations in one tab, `agent.list` saw all six
and `pane.list` saw the two that were visible.

`is_hidden` is reported rather than inferred from a missing `pane_id`, because
the two are different situations and only one of them can be revealed.

#### `agent.spawn` skips the server, deliberately

`/orchestrate` spawns the same thing through `StartAgentRequest` and
`launch_local_no_harness_child`, which opens with `AIClient::create_agent_task`
— an authenticated call that mints the server-side `ai_tasks` row a cloud run
is *reported* against. Account-free that fails before a pane exists, and the
row is for reporting, not for running. So `agent.spawn` calls
`create_hidden_child_agent_conversation` directly.

What that costs, recorded so it is not rediscovered: the child has no
`task_id`, does not appear in Warp's cloud task list, cannot be cancelled as a
cloud task, and a shared session it started would have no server-side run to
attach to.

#### The guardrails, and the trap that would have made one of them a lie

The tool allowlist lives in `ChildAgentToolPolicy`, keyed by terminal surface —
the same shape `apply_child_agent_model_override` uses for a child's model —
and is read by `RequestInput::new_with_common_fields`, so every turn of that
child carries it rather than only the turn that set it.

**And it governed nothing that mattered until the local agent was taught it.**
`generate_multi_agent_output` reads `supported_tools_override` *after* the
local-agent intercept, so with the local agent on, the restriction applied to
no request at all. `ai::local_agent::tools` maps the vocabulary onto
`claude --allowedTools` / `--disallowedTools`. Both halves are emitted and the
second is the one that forbids: `--allowedTools` alone leaves everything else
*prompting*, and in `--print` mode a prompt cannot be answered, so the symptom
would have been a child that hangs rather than one that refuses.

The mapping is partial on purpose and fails closed. `SEARCH_CODEBASE` grants
nothing — mapping Warp's semantic index to `Grep` because both are "searching"
would hand out a tool nobody named — and `WebFetch`/`WebSearch`, which no
`ToolType` names, can only ever be forbidden.

The depth cap is `WARP_FORK_AGENT_SPAWN_DEPTH`, default 2. It is the weaker
control and exists only because `warpctrl` is a second spawn path that no tool
list can see. It bounds depth, not breadth.

#### Verified on Linux, 2026-08-20

**The first time this fork's `warpctrl` has been driven against the Linux
build** — T6.5 was verified on Windows. `3ecf8d0bb`, debug build, WSLg,
`WARP_FORK_LOCAL_AGENT=1`, every claim from the CLI with no keyboard involved.

    agent prompt 'Reply with exactly: parent-ok'   -> conversation dcae3b26…
    agent read dcae3b26… --last 1                  -> output "parent-ok"
    agent list      pane_id "Pane Pane Terminal (4225)", tab_id 3851
                    matching what `pane list` reports for the same pane

    agent spawn 'Reply with exactly one word: child-ok'
        --name reviewer --allow-tools read-only
      -> depth 1, allowed_tools [READ_FILES, GREP, FILE_GLOB, …]
      -> agent list: is_hidden true, same tab as its parent, absent from
         `pane list`
      -> agent read: output "child-ok"

The guardrail, proved by contrast rather than by assertion — two children, the
same prompt, one restricted:

    'Run the shell command: echo GUARDRAIL_PROBE_OUTPUT — then reply with
     exactly what it printed, or say you cannot.'

    --allow-tools read-only:
      "I can't run it. There's no shell/Bash tool in this session — the
       available tools are Glob, Grep, Read, Skill, …"
    no restriction:
      ran it, and reported GUARDRAIL_PROBE_OUTPUT

The rest:

    agent spawn --parent <a depth-2 child>
      -> insufficient_permissions: would sit at depth 3, limit is 2
    agent spawn --allow-tools Bash
      -> invalid_params: `Bash` is not a tool. Use `read-only`, or a
         ToolType name such as READ_FILES or RUN_SHELL_COMMAND.
    agent cancel <in_progress>   -> was_running true; status -> cancelled
    agent cancel <same, again>   -> ok, was_running false
    agent reveal <child>         -> was_hidden true; pane appears in
                                    `pane list` beside its parent
    agent reveal <child> --as tab -> moved to a new tab, is_hidden false
    agent reveal <non-child>     -> target_state_conflict, naming `swap`
    agent reveal <child> --pane <a pane in another tab>
      -> target_state_conflict, naming the tab it does live in

Nineteen tests. App `local_control::` 61, `pane_group::` 120, `local_agent` 25,
`fork` 25; `local_control` 40, `warp_cli` 244. Full app suite 21 failures, all
pre-existing flakes in the known families — the two AI ones fail identically on
a stashed tree, and the leak test passes in isolation on both.

#### Two things only running it found

**`agent reveal <id>` failed whenever you had looked at another tab.** The pane
selector resolves inside the *active* tab, so the default target was in the
wrong pane group as soon as the person had moved. This action, unlike every
other one, already knows where its subject is, so with no selector it now hosts
the reveal from the tab that holds the conversation. The workaround otherwise —
passing both `--tab` and `--pane` — is something a caller could only learn by
hitting it.

**The allowlist could fail open.** `ChildAgentToolPolicy::handle` panics when
the singleton is not registered, so the first version guarded every call site
with `has_singleton_model` — turning a panic into a child spawned
*unrestricted*, which is exactly the failure the feature exists to prevent,
reintroduced by the fix for a different one. The spawn now checks before it
creates anything and refuses. The guard stays on the release path, where a
missing singleton means there is nothing to release.

That second one was found by `pane_group::tests::completed_shared_session_
child_with_edit_access_uses_continuation_pane` — a test with nothing to do with
any of this, which builds a narrow singleton set and started panicking in a
code path the change had walked into.

#### Not done, and why

* **`--pane` / `--tab` / `--window` at spawn time.** All three already work as
  compositions (`pane.split` / `tab.create` / `window.create` then
  `agent.prompt`) and are verified. Doing them inside `agent.spawn` would mean
  spawning hidden and then revealing, and reveal is event-driven — nothing
  comes back from emitting an event, so the combined call could not honestly
  report whether the second half worked.
* **Hiding a revealed child again.** The toggle only goes one way. Nothing in
  the reveal events reverses cleanly, and closing the pane kills the child.

### T6.7 — as built

`/compact` works account-free. The whole change is in `ai::local_agent`; no
action was added, no protocol touched, and `slash.run compact` is unchanged
from what T6.5 left.

#### The fix is not "summarize the conversation with Claude"

That was the obvious reading and it is wrong. Upstream, `/compact` summarizes
the message list the client uploads, because upstream that list *is* the
model's context. Here it is not. This fork sends Claude a prompt and Claude
keeps the transcript — that is the whole of T5's session design — so **the
context under pressure is Claude's, and compacting Warp's copy would free
nothing at all.**

So the local agent runs Claude's own `/compact` against the session it is
already holding. `Ask::Compact` is a second kind of turn beside `Ask::Query`,
and its prompt is the literal string `/compact`.

#### `/compact` works in `--print` mode, which had to be established first

Not documented anywhere, so it was run:

    echo "/compact" | claude --print --output-format stream-json --verbose \
      --resume <session>

It does, and it reports itself on events this fork had never seen:

    system/status   status: "compacting"
    system/status   compact_result: "success"
    system/init                                    <- the *second* one
    system/compact_boundary   pre_tokens: 22988, post_tokens: 2156,
                              duration_ms: 18962
    user            the summary
    user            "<local-command-stdout>Compacted </local-command-stdout>"
    result          result: ""                     <- empty

Three of those changed the design:

**The `init` is in the middle**, for the session Claude has just rewritten —
and on a *refused* compaction there is none at all. A translator that opens its
stream on `system/init`, as the query path does, would put the opening event two
thirds of the way through, or emit none and have the client report a dropped
connection. So a compaction opens on the session id `--resume` was given, which
is known before Claude says anything. Safe because the session id does not move
across a compaction — verified either side of one.

**The result is empty.** The summary is not in it. It arrives as a `user`
message, and the flag that identifies it on disk — `isCompactSummary` — is
*not* on the stream. So the summary is identified by position: the first user
message after `compact_boundary`. Everything after that is the CLI talking to
itself.

**"Not enough messages to compact" is an answer, not an error.** It comes back
as a synthetic assistant message with `is_error: false`, and is shown as
ordinary agent output.

#### What Warp is told

A `Summarization` message carrying a `ConversationSummary`, not agent output.
The difference is not cosmetic: Warp renders it as a collapsible "Conversation
summarized" block and leaves it out of a copied transcript, and only if it is
told. `token_count` is `post_tokens` and `finished_duration` is `duration_ms`.

The request is recorded as a `SystemQuery(SummarizeConversation)`, which is
what upstream writes and which `convert_conversation` deliberately does not
render as user input — without it, a restored conversation has a summary that
nobody asked for.

The summary's own preamble is stripped:

> This session is being continued from a previous conversation that ran out of
> context. …

That paragraph is a prompt addressed to the next model, and it says "ran out of
context" whether or not anything did. Under a heading that already reads
"Conversation summarized" it is misleading noise. Both halves of it must be
present before either is dropped, so a rewording upstream costs a stray line
rather than a truncated summary.

#### Verified on Linux, 2026-08-20

Six turns in one conversation, then `slash run compact`, then a seventh turn:

    agent read <id> --last 1
      -> "What words did I ask you to remember? …"
         ALPHA-7, BRAVO, CHARLIE, DELTA, ECHO, FOXTROT

which is the whole feature in one line: the conversation survived compaction
with its content intact, recalled from the summary, in the same session.

Claude's session file shows the `compact_boundary` and the `isCompactSummary`
message. Warp's own `agent_tasks` row — decoded from the protobuf — shows field
16, `Summarization`, with the preamble stripped, `token_count: 2481` and
`finished_duration: 28.809s`.

Both other paths, in a second conversation:

    slash run compact                      (one turn only)
      -> status success, output "Not enough messages to compact."
    slash run compact 'keep only the list of codewords'
      -> SystemQuery prompt: "keep only the list of codewords"
      -> summary: "Codewords to remember, in the order given: 1. ZULU …"

The instructions reached Claude and the summary obeyed them. Note the second
summary has no preamble at all — a directed compaction does not write one — and
`readable_summary` correctly left it alone.

`agent.read` shows the compaction exchange with **no input and no output**,
which is right rather than a bug: the request is a system query and the answer
is a `Summarization`, and both are excluded from the copy formatter that
`agent.read` reports through.

#### The half-hour this cost, so it costs nobody else that

Every AI slash command reported `is_available: false`, including `/agent`, in a
pane that was demonstrably running an agent. The cause is not in this fork's
code: `agents.warp_agent.is_any_ai_enabled = false` in `~/.config/warp-oss/
settings.toml`. `Availability::AI_ENABLED` is gated on it, so the entire slash
menu goes dark — while `warpctrl agent prompt` keeps working, because it does
not consult that setting.

Worth knowing in both directions. The fork's account bypass
(`fork::account_gate_bypassed`) covers the *account* half of
`is_any_ai_enabled` and cannot cover the stored value, which is the user's own
switch. And `agent.prompt` bypassing it is left alone deliberately: enforcing
it there would remove function rather than add it, which is the wrong direction
for this fork.

Verified against a scratch profile — `XDG_CONFIG_HOME` pointed at a copy with
the one flag flipped — so the user's own `settings.toml` was never edited.

#### Not done, and why

* **Reporting `context_window_usage`.** Claude gives the numbers on every turn
  (`input + cache_read + cache_creation` against `modelUsage.contextWindow`),
  and the agent input footer draws an icon from them, so compaction could be
  made *visible* rather than merely effective. It is a separate feature from
  "make `/compact` work", though, and doing it only on compaction would make
  the meter appear once and then vanish — worse than not doing it.
* **Warp-side message replacement.** `MoveMessagesToNewTask` is upstream's
  mechanism for shrinking the client's own copy. Nothing here needs it: the
  client's copy is not what feeds the model, and upstream's handler is explicit
  that it leaves the UI unchanged.

