# WSL, as a remote server

**As of 2026-09-05, evening.** This page is the current state of one surface.
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
| **diff panel** | **unmeasured.** The remote diff stack exists and was built for SSH; nobody has opened it on a WSL host | 9p. Works on a small repo (T6.1). On this repo the panel sits on *"Diffs don't currently work in WSL"* because repository detection never finishes | read 2026-09-05, see below |
| **editor language servers** | **absent by construction.** A routed buffer is `Remote`, the editor's LSP path takes local paths only, and the protocol has no LSP messages | the server is spawned **on Windows** against a `\\wsl$` path: needs Windows-installed servers, reads the crate over 9p | read 2026-09-05, see below |
| agent panel | the agent starts inside the distribution | same | measured 2026-09-02, T18 |
| the agent's own LSP tool | inside the distribution, the agent's own server | same | measured 2026-09-02, T17 |
| clipboard | Warp's, Windows-native. Select-and-copy never touches the distribution; OSC 52 from a program in the pane goes through Warp's handler | same | read 2026-09-05; **the failing gesture has not been named** |
| **connecting the server** | **automatic when the shell bootstraps** (`ea61116e1`). The palette's *Connect Warp Remote Server to this pane's WSL distribution* and `warpctrl remote wsl connect` remain, for re-attaching or for another distribution | only when the connect failed or the switch is off | measured 2026-09-05, twice, below |

Until this date two of the three bold rows were the same fact: everything that
is routed only routed after someone connected, and the product profile
connected nothing. See "Connecting, as built" below for what the first run of
the automatic connect found, which was not the thing it was built to find.

## Why the diff panel says what it says

The message is a fallback, and it names the wrong cause.

What decides whether a diff shows is whether a repository has been **detected**
for the pane's directory. Detection publishes `RepositoriesChanged`; the right
panel picks a path from it and builds a code review view for that path
(`app/src/workspace/view/right_panel.rs:581`). A `Remote` path builds
`RemoteDiffStateModel` over the server; a `Local` one builds
`LocalDiffStateModel` on this machine (`app/src/code_review/diff_state/mod.rs:454-476`).

When nothing has been detected, the view has no repo and renders
`render_no_repo_for_env`. That reads `session.is_wsl()` and, if true, prints
`WSL_TEXT` (`app/src/code_review/code_review_view.rs:2884` and `:270`). The
left panel does the same at `app/src/workspace/view.rs:17695`. Neither asks
whether a server is attached, and neither can reach the `RemoteSession` arm:
`is_remote` comes from `Session::is_local`, which reads `session_type()`, and a
WSL session's type is `Local` (`app/src/terminal/model/session.rs:1389`,
`session/filesystem.rs` explains why).

So in a not-routed pane on a large repository the sequence is: Windows-side
detection starts walking the repo over 9p at about 20 ms per entry (T16's
measurement), the panel has no repo for the duration, and the fallback text
blames WSL. The honest text would be *"still looking for a repository"*, and on
this repo it would stay up for an hour.

In a routed pane the repo should arrive as `Remote` and the panel should build
the remote model. **That has not been run on a WSL host.** T16's phase-2 table
measured the tree, the buffer, search and the git chip; the code review panel is
not in it. The daemon side exists (`app/src/remote_server/diff_state_tracker.rs`,
`crates/remote_server/proto/diff_state.proto`, `GetDiffState` at
`remote_server.proto:67`), so the expectation is that it works or fails small.

Check first, before theorising: `warpctrl session inspect` on the pane. If it
says `local`, the pane was never connected and the screenshot is the 9p column.

## Why the editor has no language server in a routed pane

`LocalCodeEditor::try_connect_lsp_server` starts from `self.file_path()`, which
is `file_location().to_local_path()` (`app/src/code/local_code_editor.rs:945`,
`:1890`). A routed buffer's location is `Remote`, so that is `None` and the
function returns before asking the LSP manager anything. The same shape holds in
`GlobalBufferModel`: `open_or_sync_document_with_lsp` takes a `&Path`, and the
LSP-on-workspace-open path filters to `LocalOrRemotePath::Local`
(`global_buffer_model.rs:1400`, `:1526`). A `Remote` buffer gets syntax
highlighting from its extension (`local_code_editor.rs:1341`) and nothing else.

Under that, the remote-development protocol carries buffers, files, ripgrep,
repo metadata, the codebase index and diff state. It has **no LSP messages** at
all (every `message` in `remote_server.proto`, read 2026-09-05). This is the gap
`CLAUDE.md` already names as the one place building something in Warp would not
duplicate a working tool.

In a not-routed pane the buffer is `Local` at `\\wsl$\…`, so `crates/lsp` spawns
the server on Windows. That needs a Windows `rust-analyzer`, and the server reads
the crate over 9p. It works, at the cost the frame refuses to pay.

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

2. **Open the diff panel on a routed pane and fix what is found.** Then replace
   the two `is_wsl()` gates with `session_filesystem` so the fallback says the
   true cause: `Host` → loading, `Unreachable` → remote without a server,
   `Local` → not a repository. If `code_review_view.rs` starts reading
   `session_type()`, `every_file_that_reads_session_type_has_been_classified`
   will ask for a reason; give it one.

3. **Language servers for routed buffers.** Two shapes, and the cheaper one
   should be measured before the expensive one is designed.

   *Cheap:* spawn the server from Windows as
   `wsl.exe -d <distro> -- rust-analyzer`. The transport is stdio, which is what
   `LspServerModel` already speaks, and the server runs inside the distribution
   against its own filesystem. What changes is paths: the client sends `file://`
   URIs for a `Remote` buffer and gets Linux paths back. `native_path` does one
   direction and `session.windows_path_converter()` the other. Unknown: whether
   `crates/lsp`'s manager can be handed a `Remote` root at all; today every
   entry point is a `&Path`.

   *Zed's shape:* the server runs on the remote, the daemon owns it, and the
   protocol proxies requests. A new message family in `remote_server.proto`, a
   daemon-side spawn reusing `crates/lsp/src/command_builder.rs` on Linux, and
   a client `LspServerModel` over the transport instead of stdio. Correct, and
   the largest item on this page by an order of magnitude.

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
| the diff panel's fallback and its remote stack | `app/src/code_review/code_review_view.rs:2870-2935`, `code_review/diff_state/{mod,remote}.rs`, `app/src/remote_server/diff_state_tracker.rs` |
| the editor's LSP attach | `app/src/code/local_code_editor.rs:940`, `code/global_buffer_model.rs:1400` |
| the protocol | `crates/remote_server/proto/remote_server.proto`, `diff_state.proto` |
| OSC 52 | `crates/warp_terminal/src/model/grid/ansi_handler.rs:1166`, `app/src/terminal/view.rs:12071` |
| the guard on new `session_type()` readers | `app/src/terminal/model/session/filesystem_tests.rs:367` |

## Open, as of this date

- Whether the pane in the 2026-09-05 screenshot was routed. Run 1's *"no
  daemon running"* says the product instance had never started a daemon, so
  no. Moot for panes opened in a build at or after `ea61116e1`.
- `RemoteDiffStateModel` against a WSL host: never opened.
- Global search's `UnsupportedSession` arm is a hard block
  (`workspace/view/global_search/view.rs:366`), the left panel sets that state
  for every WSL pane, and T16 measured routed search working. Those three
  facts have not been reconciled.
- The default of `terminal.osc52_clipboard_access`.
- For the cheap LSP shape: whether a Linux `rust-analyzer` accepts the URIs a
  Windows-side client sends, and what `crates/lsp` does with a root it cannot
  `canonicalize`.
