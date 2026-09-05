# WSL, as a remote server

**As of 2026-09-05, night, third pass.** This page is the current state of one surface.
It is rewritten in place when the state changes; the history is in `git log`
and in the tickets it cites (T6, T16, T17, T18, T20.1). Where a row says
*measured*, there is a date and a ticket; where it says *unmeasured* or *read*,
nobody has run it yet.

## The requirement

The maintainer's frame, stated on 2026-09-01 and restated 2026-09-05 after the
diff panel refused in a WSL pane:

> A WSL pane is a remote session, the way Zed and VS Code treat it. The
> distribution is the server. File reads and writes, diffs, search and language
> servers run inside it, over Warp's remote-development server. Nothing
> user-facing reads the distribution's files through 9p (`\\wsl$\…`). The
> Windows side owns the window, the terminal, the input and the clipboard.

Parity means: what works in a native Linux Warp on a Linux repo, and in a
native Windows Warp on a `C:\` repo, works in a WSL pane on a repo inside the
distribution. The editor, its language servers, the diff panel, the tree, search
and the clipboard are the surfaces that test it.

Three tickets have relitigated pieces of this (T6, T16, T18). Each fixed one
surface and recorded the frame in its own words. This page exists so the frame
is written once, and the table below says how far it has been carried.

## State

"Routed" means a remote-development server is attached to the pane's
distribution (`warpctrl session inspect` reports `{"where": "host"}`). Since
`ea61116e1` a fresh pane routes itself when its shell bootstraps; "not routed"
is what a pane gets when that connect failed, when
`WARP_FORK_WSL_AUTO_CONNECT=off`, or under `WARP_FORK_POLICY=0`.

| surface | routed | not routed | how established |
|---|---|---|---|
| terminal, blocks, exit codes | native | native | the pty is the distribution's shell either way (T6.1) |
| file tree | **inside the distribution** | 9p walk: minutes on a small repo, never finishes on this one | measured 2026-09-02, T16 phase 1 |
| open a file in the editor | **inside the distribution** (`OpenBuffer`) | 9p, works | measured 2026-09-02, T16 phase 2 |
| global search | **inside the distribution** (`ripgrep_search`) | 9p, ~9 s where `C:` takes 0.1 s | measured 2026-09-02, T16 phase 2 |
| git branch and dirty chip | inside the distribution | through `wsl.exe` | measured, T6 and T16 |
| diff panel | **inside the distribution** (`RemoteDiffStateModel` over `GetDiffState`). Opened on this repo: header names the distribution, a clean tree says "No open changes", two changes made outside the pane appeared with their hunks within 8 s | 9p, and on this repo it **works**: panel 3 s after the `cd`, the full index (6505 files) 27 s. The "never finishes" this row carried was not reproduced. The no-repository fallback, reached only in a directory that is not one, names the 9p read since `099b26ea5` | measured 2026-09-05, both columns, below |
| **editor language servers** | **inside the distribution** (`6d07c1e1a` and three fixes after it). The server is the distribution's own `rust-analyzer`, spawned through `wsl.exe`; a routed buffer opens with it, hover shows the real signature, go-to-definition into another crate opens a second routed buffer | **inside the distribution too**, same server, same key: the workspace is `\\wsl$\<distro>\...` either way, so the two panes share one server. `WARP_FORK_WSL_LSP=0` puts the Windows-side server back | measured 2026-09-05, below |
| agent panel | the agent starts inside the distribution | same | measured 2026-09-02, T18 |
| the agent's own LSP tool | inside the distribution, the agent's own server | same | measured 2026-09-02, T17 |
| clipboard | Warp's, Windows-native. Select-and-copy never touches the distribution; OSC 52 from a program in the pane goes through Warp's handler | same | read 2026-09-05; **the failing gesture has not been named** |
| **connecting the server** | **automatic when the shell bootstraps** (`ea61116e1`). The palette's *Connect Warp Remote Server to this pane's WSL distribution* and `warpctrl remote wsl connect` remain, for re-attaching or for another distribution | only when the connect failed or the switch is off | measured 2026-09-05, twice, below |

Until this date two of the three bold rows were the same fact: everything that
is routed only routed after someone connected, and the product profile
connected nothing. See "Connecting, as built" below for what the first run of
the automatic connect found, which was not the thing it was built to find.

## The diff panel, routed and not

**Routed, it works, and nothing had to be built.** Measured 2026-09-05 on the
debug build `1a42ecdb8`, scratch profile, this repository. Both panes
restored at launch routed before the first prompt; `cd` into the repo and
`warpctrl surface code-review open`, and four seconds later the panel's
header read `WSL: Ubuntu:/home/effatha/git/warp` over "No open changes" for a
clean tree. Two edits made from a WSL shell outside Warp -- a line appended to
a tracked file and a new untracked file -- appeared in the panel within eight
seconds with their hunks and a "93 unmodified lines" fold, with no command
typed in the pane: the daemon's watcher saw them. The tab's own chip went to
`+3 -0` in the same interval. The remote diff stack was written for SSH and
had never been opened against a WSL host; it needed no change. The
screenshots are `C:\dev\shots\diff-routed-{2,3}.png`.

**Not routed, on this repo, it also works -- which this page said it did
not.** Measured 2026-09-05 on `099b26ea5`, same profile, launched with
`WARP_FORK_WSL_AUTO_CONNECT=0` (zero `attaching a remote server` lines in the
log, `session inspect` → `local`). `cd` into the repo at 17:18:46; the panel
reported `loading` in the same second, had the branch list at 17:18:49, and
drew both changes over `\\wsl$\ubuntu\home\effatha\git\warp` with the same
hunks as the routed run. `repo_metadata::local_model` finished the full walk
at 17:19:13: **6505 files in 27 s**, about 4 ms a file, not the 20 ms an
entry T16 measured for the tree. So the friction #1 screenshot -- the fallback
sitting on this repo in the product instance -- was not reproduced here, and
its cause is not established; the sentence this page carried, that
"repository detection never finishes" on this repo, was an inference from
T16's tree measurement and is retracted. What the unrouted column costs on
this repo is a 27 s walk and the Windows-side LSP hint at the foot of the
panel (*"Language support is not currently enabled. Enable rust-analyzer"*),
which is the 9p editor path the next section describes.

**The message, when it is reached, used to name the wrong cause and now
names the real one.** What decides whether a diff shows is whether a
repository has been **detected** for the pane's directory. Detection publishes
`RepositoriesChanged`; the right panel picks a path from it and builds a
code review view for that path (`app/src/workspace/view/right_panel.rs`).
A `Remote` path builds `RemoteDiffStateModel` over the server; a `Local` one
builds `LocalDiffStateModel` on this machine
(`app/src/code_review/diff_state/mod.rs:454-476`).

When nothing has been detected the view draws a fallback, and until
`099b26ea5` three panels chose it from `session.is_wsl()` alone: the diff
panel (`code_review_view::session_env`), the right panel's own copy of that
match, and the project explorer through the enablement `workspace/view.rs`
computes. A WSL session's type is `Local` (`session/filesystem.rs` says why),
so `is_remote` was never true for one, and a routed pane looked exactly like
an unrouted one to all three. In an unrouted pane on this repo the sequence
was: Windows-side detection starts walking the repo over 9p at about 20 ms an
entry (T16), the panel has no repository for the duration, and the text
blames WSL.

Now `CodingPanelEnablementState::from_session_env_with_wsl_routing` takes one
more fact, `session_filesystem(..).host().is_some()`, and a routed WSL pane is
`RemoteSession { has_remote_server: true }`: the explorer shows its loading
state until the daemon's metadata lands, search runs through the daemon, and
the diff panel with no repository says "Diffs only work for git
repositories" -- the daemon ran detection, nothing arrived, so the directory
is not one. That last text also replaces "Diffs only work for local
workspaces" for an SSH session with a server, which was wrong for the same
reason. An unrouted WSL pane keeps the `UnsupportedSession` arm, because its
9p path still works on a small repo and slow is not broken; its text is now
*"This WSL pane has no Warp server attached, so its repository is read from
Windows over 9p and can take minutes to detect. Connect one from the command
palette."* The explorer's says the same of its listing. The rule is pure and
has four tests, one of which walks every input combination to show upstream's
answer is unchanged whenever nothing is routed.

Measured on the Windows debug build the same evening, all three arms, in
`C:\dev\shots\diff-*.png`: routed in the repo, the diffs (`diff-routed-4`);
routed in `/tmp`, *"Diffs only work for git repositories"* with no button
(`diff-routed-norepo`); unrouted in `/tmp`, the WSL text
(`diff-unrouted-norepo`). The last one drew as a single line past the panel's
edge on its first run and took the icon and title with it -- upstream's
messages were each one short line and a plain `Text` never had to wrap --
fixed in `6c9f68bf5` with the explorer's `FormattedTextElement`-in-a-
`Shrinkable` shape and re-measured (`diff-unrouted-norepo-wrapped`).

Check first, before theorising: `warpctrl session inspect` on the pane. If it
says `local`, the pane was never connected and what you are looking at is the
9p column.

## Language servers, as built

**Measured before designed**, with a stdio LSP client driven from the Windows
side (`.fork/runs/lsp-routed-2026-09-05/`, four probes and a live run). The
cheap shape from the ranking below worked on its first run: a Windows process
spawned `wsl.exe -d Ubuntu -- rust-analyzer` with a `\\wsl.localhost\...`
working directory, sent `file:///home/...` URIs, and got a definition back in
3.41 s, the same as the native Linux control. The Windows `rust-analyzer.exe`
over the redirector took 25 s on the same three-symbol crate. On this
repository the distro-spawned server answered from cold in 61 s.

So the open question this page carried, whether a Linux `rust-analyzer`
accepts the URIs a Windows client sends, was the wrong question: it accepts
Linux ones. What it cannot accept is what `crates/lsp` sends, because
`url::Url::from_file_path` refuses a Linux path on Windows outright
(`Path::is_absolute` is false without a drive or UNC prefix). The whole gap
was path spelling, and the protocol never came into it.

**The shape.** Every key in Warp stays a Windows path. A workspace inside a
distribution is `\\wsl$\<distro>\...`, the spelling
`canonicalize_wsl_unc_path` folds every other one to and the one an unrouted
pane already enables a server under, so a routed pane and an unrouted pane on
the same repository share one server. What changed:

- `crates/lsp`: `LspServerConfig::with_wsl_distro` makes the spawn
  `wsl.exe -d <distro> --shell-type login -- <binary>` with the UNC root as
  cwd (`wsl.exe` maps it to the Linux directory, measured), skips the
  data-dir install (a Windows executable), and names the distribution in
  the not-installed error. `UriMapper` is the one seam every URI goes
  through: `Local` is upstream's pair of functions; `WslDistro` spells
  `\\wsl$\ubuntu\home\x.rs` as `file:///home/x.rs` and folds every
  `file:///...` the server answers with back to the canonical spelling.
  `--shell-type login` is load-bearing: the default PATH had nothing under
  `/home`.
- `code::routed_lsp`: `WslHosts` records which distribution a host is, from
  the `Sessions` subscription on `SessionConnected`, the one place the host
  id and `wsl_distro_name()` are both in hand. `lsp_path_for` gives a routed
  buffer its `\\wsl$` path; `location_for_lsp_path` turns a path the server
  handed back into that host's remote buffer when a client for it is
  connected; `repo_root_for_lsp_path` adds the remote root to the enablement
  lookup, because the daemon registered the repository as remote.
- The editor's LSP-facing reads of `file_path()` go through `lsp_path()`; the
  buffer model runs the same document lifecycle for a routed buffer; the
  footer, the find-references card and the shutdown manager's "is anyone
  using this server" scan follow the LSP path too.

**Measured live, Windows debug build `00ce16d01`, scratch profile, this
repository, routed pane.** Opening `app/build.rs` from the tree drew the
footer with *Enable rust-analyzer* (`lsp-routed-footer-enable.png`); enabling
it registered a server for `\\wsl$\ubuntu\home\effatha\git\warp` and
spawned pid 18348 on Windows, which was a `rust-analyzer` inside the
distribution with cwd `/home/effatha/git/warp` and the `wsl.exe` relay as its
parent; the server log shows the routed buffer's `didOpen` under the UNC
path. Hovering `app_target_dir` drew the card with `pub fn
app_target_dir(profile: &str) -> Result<...>` and its doc line
(`lsp-routed-hover.png`). *Go to definition* from the context menu resolved
to `crates/warp_util/src/path.rs`, which the view mapped to the host's remote
buffer and opened as a second routed tab, through the daemon
(`lsp-routed-goto-definition.png`). A definition into the toolchain's
`std/src/macros.rs` mapped the same way, outside the workspace.

Three fixes the live runs paid for, each a `file_path()` gate that read
"local" where it meant "has a server": the editor's footer was only added
for a `Local` location (`7100a8f94`); the shutdown manager stopped the
freshly started server as unused ten seconds later because its scan asked
each editor for `file_path()` (`5afc14d02`); and the definition path was
silent about where it dropped, so it logs now (`00ce16d01`).

**Two things this measured that are not the fork's.** The editor's cmd-click
modifier is the Super key on winit builds (`cmd: state.super_key()`), so
go-to-definition on Windows is Win+click, not Ctrl+click; the context menu
has the same item. And the Windows-side comparison carried a confound: its
`cargo metadata` failed on an argument its cargo did not know, so part of
the 25 s is the Windows toolchain and not only 9p.

Zed's shape, a daemon-owned server behind a new message family in
`remote_server.proto`, was not built. Nothing here needed the protocol.

## Clipboard

What is known. Select-and-copy and paste are Warp's own, on the Windows
clipboard; the pane's shell is not involved. OSC 52 from a program inside the
pane (nvim, tmux) reaches `ansi_handler.rs:1166`, is decoded, and is written to
the system clipboard at `app/src/terminal/view.rs:12071` if
`terminal.osc52_clipboard_access` allows it; a blocked write shows a banner.
`wl-copy`, `xclip` and `clip.exe` run inside the distribution and never pass
through Warp; the first two need WSLg's display, the third is Windows interop.

What is not known is which of those failed. **Name the gesture** (copied from
where, pasted to where, with what) and this row gets a mechanism.

## Connecting, as built

**The arm.** `ModelEventDispatcher::complete_bootstrapped_session`
(`app/src/terminal/model_events.rs`) is where SSH sessions get their server
notified, and it is now where a WSL session gets one attached. After the
session is registered, `wsl_auto_connect_target` decides from four facts --
the dispatcher's SSH support (headless has no manager), the `SshRemoteServer`
flag (which gates both transports, I16), `fork::wsl_auto_connect_enabled()`,
and whether the session is an SSH wrapper (then it is the SSH arm's) -- and
returns the session's own distribution from `SessionInfo::wsl_name()`. That
goes to `start_wsl_remote_server`, the same call the palette and `warpctrl`
make. The decision is pure and has four unit tests; the environment parser has
one.

Unlike the SSH arm it never holds the bootstrap back. The shell initialises as
it always did; `connect_session` spawns check, spawn and handshake onto the
background executor and the outcome arrives as a manager event. A failed
connect is therefore a `WARN` line and a not-routed pane, and `SessionConnected`
re-running repository detection (T16) means it does not matter whether the
shell has `cd`-ed before the server comes up.

The switch is `WARP_FORK_WSL_AUTO_CONNECT`, on unless `0`/`off`/`false`, and
off under `WARP_FORK_POLICY=0`. There is no install step: the palette path
never had one either, and on the Oss channel there is nothing to install from
(`remote_server_binary()` documents the binary as "deployed/managed locally";
the manual's recipe is a symlink). A distribution without the staged binary
fails at spawn, in the log.

**Run 1, 2026-09-05 11:39, debug build `ea61116e1` under
`WARP_DATA_PROFILE=wslauto`** (a second instance beside the product one;
`WARP_DATA_PROFILE` is honoured by debug builds only, gives its own
`config/` and `data/`, and on Windows still shares the registry preference
store, which is why the scratch profile skipped onboarding without being
seeded). The arm fired:

```
15:39:31Z  Shell is bootstrapped with session_id SessionId(5444020844095028547)
15:39:31Z  WSL session SessionId(...) bootstrapped in Ubuntu; attaching a remote server
15:39:32Z  Proxy: no daemon running, will start one
15:39:34Z  Proxy: daemon socket ready after 1.826770122s
15:39:34Z  Remote server version mismatch, removing stale binary:
           client=Some("v0.fork.ea61116e1") server="v0.fork.14a9a5384"
15:39:34Z  Removing stale remote server binary in WSL distro Ubuntu:
           rm -f ~/.warp-dev/remote-server/warp-oss
15:39:34Z  Remote server connection failed: ... reconnect to reinstall
```

Three seconds from bootstrap to a handshake, including a cold daemon start,
and then a refusal. **Two findings, and the second is the one that matters.**

1. *The version check is live now.* I16 recorded it as a rough edge to check
   before anything shipped, when both sides reported `None`. Version stamping
   (2026-09-04, `build.sh`/`build.ps1`) made it real: the Windows client and
   the Linux daemon staged in the distribution are two builds of the same
   fork, a commit apart, and will be most of the time. Note also *"no daemon
   running"*: the product instance that had been up since 09:48 had never
   started one, which is the not-routed column measured from the other side.

2. *The repair deleted the operator's symlink.* Upstream's remedy for a
   mismatch is `rm -f <binary>` so the reconnect reinstalls the pinned
   artifact. On Oss the path is unversioned and what sits there is the symlink
   the manual says to place by hand. After run 1, `~/.warp-dev/remote-server/`
   had no `warp-oss` in it, and every later connect from any instance --
   including the product one -- would have failed at spawn with nothing in
   the panel to say why. Restored by hand. So the second commit,
   `1a42ecdb8`, makes `version_is_compatible` take the channel and answer
   true on Oss, with the differing tags logged one line earlier so the
   mismatch is still visible. What that gives up is protocol drift between
   the two sides going unnoticed; the two builds here are usually within a
   day of each other, and a real drift shows up as decode errors in the same
   log.

**Run 2, same day, debug build `1a42ecdb8`, same profile.** Routed, twice:

```
15:47:16Z  Shell is bootstrapped with session_id SessionId(7311933633460623612)
15:47:16Z  WSL session SessionId(...) bootstrapped in Ubuntu; attaching a remote server
15:47:17Z  Proxy: connecting to daemon socket at ~/.warp-dev/remote-server/d9b8ac72/server-bd6b5408.sock
15:47:17Z  Remote server version differs from the client: client=Some("v0.fork.1a42ecdb8") server="v0.fork.14a9a5384"
15:47:17Z  Remote server connected: session=SessionId(7311933633460623612) host=4880ca5e-...
```

`warpctrl session inspect` on that pane: `{"where": "host", "host_id":
"4880ca5e-2c1f-4b5a-8c39-ee84d472c03a"}`. A second tab opened with `warpctrl
tab create` bootstrapped at 15:47:43 and was connected in the same second,
also `host` by `session inspect --pane`. The symlink was still in place
afterwards and one daemon was running. So the cost of a fresh pane is about
one second with the daemon warm and about three with it cold, and nothing the
shell does waits on either.

Two instrument notes from the run. `session inspect` without `--pane` answers
`ambiguous_target` once a second tab exists, because every tab has an active
pane; name the pane. And the log carried four `CloudObjects::Listener`
lines -- a websocket to Warp's cloud-objects service attempted at launch and
failing on missing credentials. That is the WebSocket board item 3 says the
deny-list cannot see, observed on Windows in the product's own log rather
than argued; it is recorded in `.fork/runs/wsl-auto-connect-2026-09-05/` and
belongs to that item.

Two things to keep in view after this. The daemon outlives Warp: it is the
process at `ps aux | grep remote-server-daemon`, spawned from the symlink's
target at the moment the first proxy found none running, and a newer Linux
build is not what serves a pane until that process is restarted. And the
debug binary's `WARP_DATA_PROFILE` is the only way measured so far to run a
second instance on Windows without restoring the product instance's panes
into it; the release binary ignores the variable.

## What to build, ranked

1. **Connect automatically.** Built, above. What remains of it is a switch in
   the panel for the not-routed case -- today the only sign a connect failed
   is the log and `session inspect` -- and that waits for the friction log to
   ask.

2. **Open the diff panel on a routed pane and fix what is found.** Done,
   `099b26ea5`, above. Nothing was found on the routed side; what was fixed
   was the fallback, and it turned out to be three sites reading one bool,
   not two.

3. **Language servers for routed buffers. Done 2026-09-05**, in the cheap
   shape: the distribution's own server through `wsl.exe`, paths spelled at
   one seam. The manager was never handed a `Remote` root; it was handed the
   same `\\wsl$` root the unrouted pane uses. Zed's shape stays unbuilt.
   Section above, and `.fork/runs/lsp-routed-2026-09-05/`.

4. **Clipboard**, once the gesture is named.

## Where the code is

| what | where |
|---|---|
| the one answer to "where are this session's files" | `app/src/terminal/model/session/filesystem.rs` (`classify`, `session_filesystem`, `native_path`) |
| the WSL transport and the shared connect | `app/src/remote_server/wsl_transport.rs`, `crates/remote_server/src/wsl.rs` |
| the automatic connect and its gate | `app/src/terminal/model_events.rs` (`wsl_auto_connect_target`, `complete_bootstrapped_session`), `app/src/fork.rs` (`wsl_auto_connect_enabled`) |
| the version rule that deleted the symlink, and its Oss case | `crates/remote_server/src/manager.rs` (`version_is_compatible`) |
| the palette action | `app/src/terminal/view/init.rs:1213`, handled at `terminal/view.rs:27799` |
| `warpctrl remote wsl connect` / `list` | `app/src/local_control/handlers/remote_wsl.rs` |
| the panels' enablement, with the routed-WSL arm | `app/src/coding_panel_enablement_state.rs` (`from_session_env_with_wsl_routing`), computed in `app/src/workspace/view.rs` (grep `wsl_routed`) and `code_review_view::session_env` |
| the diff panel's fallback and its remote stack | `code_review_view::render_no_repo_for_enablement`, `code_review/diff_state/{mod,remote}.rs`, `app/src/remote_server/diff_state_tracker.rs` |
| the editor's LSP attach | `app/src/code/local_code_editor.rs` (`lsp_path`, `try_connect_lsp_server`), `code/global_buffer_model.rs` (`sync_remote_buffer_with_lsp`) |
| a routed buffer's LSP path, and the host-to-distribution record | `app/src/code/routed_lsp.rs` (`WslHosts`, `lsp_path_for`, `location_for_lsp_path`, `repo_root_for_lsp_path`), `app/src/fork.rs` (`wsl_lsp_in_distro_enabled`) |
| the server inside the distribution, and the URI seam | `crates/lsp/src/config.rs` (`UriMapper`, `with_wsl_distro`), `crates/lsp/src/command_builder.rs` (`wsl_argv`) |
| the protocol | `crates/remote_server/proto/remote_server.proto`, `diff_state.proto` |
| OSC 52 | `crates/warp_terminal/src/model/grid/ansi_handler.rs:1166`, `app/src/terminal/view.rs:12071` |
| the guard on new `session_type()` readers | `app/src/terminal/model/session/filesystem_tests.rs:367` |

## Open, as of this date

- What the friction #1 screenshot was actually showing. The unrouted panel
  works on this repo on `099b26ea5` (27 s walk); the product instance that
  day sat on the fallback, and nothing here says why. Candidates, all
  unmeasured: the outline/indexing walk running concurrently, or a
  detection that had failed rather than one still running.
- Global search's `UnsupportedSession` arm: read again 2026-09-05, it is not
  a hard block. `blocker()` returns `NoSearchableRoots` only when the pane
  gave no directory, and its own doc says the state is misnamed for search.
  A routed pane now reaches the `RemoteSession` arm instead, which blocks
  nothing. Reconciled by reading; T16's routed-search measurement stands.
- The default of `terminal.osc52_clipboard_access`.
- Go-to-definition into a routed buffer that was not yet open lands at line
  1: the tab for `path.rs` opened at the top rather than at line 326. The
  `cursor_at` runs before the daemon's content arrives. Not chased; the
  local path may have the same race or may not.
- The footer's *Install* button for a distribution root refuses with a
  toast naming the distribution rather than installing there. Deliberate,
  because what Warp downloads is a Windows executable; a server has to be
  installed inside the distribution by hand.
