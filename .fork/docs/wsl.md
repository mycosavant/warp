# WSL, as a remote server

**As of 2026-09-05.** This page is the current state of one surface. It is
rewritten in place when the state changes; the history is in `git log` and in
the tickets it cites (T6, T16, T17, T18, T20.1). Where a row says *measured*,
there is a date and a ticket; where it says *unmeasured* or *read*, nobody has
run it yet.

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
distribution (`warpctrl session inspect` reports `{"where": "host"}`). "Not
routed" is a fresh pane, which is what the product profile launches today.

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
| **connecting the server** | **manual.** The command palette's *Connect Warp Remote Server to this pane's WSL distribution*, or `warpctrl remote wsl connect` | this is the default | read; IDEAS I16's correction records why |

Two of the three bold rows are the same fact. Everything that is routed only
routes after someone connects, and the product profile connects nothing. A
person opening Warp, a WSL pane and the diff panel gets the not-routed column,
which is the 9p column, which is the column the frame forbids.

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

## What to build, ranked

1. **Connect automatically.** The gap under every other row. SSH sessions
   attach a server at `InitShell` time; the attach is keyed on
   `IsSSHWrapperSession::Yes`, whose payload is a ControlMaster socket path a
   WSL session cannot have, which is why the WSL arm was never written (IDEAS
   I16, correction). `Session::wsl_name()` already carries the distribution, and
   `start_wsl_remote_server` (`app/src/remote_server/wsl_transport.rs:373`) is
   already shared by the palette action and `warpctrl`, so the arm is a call to
   it from the place SSH makes its call. Default on in the fork, off under
   `WARP_FORK_POLICY=0`. A failed connect must leave the pane working
   not-routed and `session inspect` saying so. The ordering fix from T16
   (`SessionConnected` re-runs detection) means it does not matter whether the
   shell has already `cd`-ed when the server comes up. First connect installs
   the daemon; measure how long a fresh pane waits.

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
| the palette action | `app/src/terminal/view/init.rs:1213`, handled at `terminal/view.rs:27799` |
| `warpctrl remote wsl connect` / `list` | `app/src/local_control/handlers/remote_wsl.rs` |
| the diff panel's fallback and its remote stack | `app/src/code_review/code_review_view.rs:2870-2935`, `code_review/diff_state/{mod,remote}.rs`, `app/src/remote_server/diff_state_tracker.rs` |
| the editor's LSP attach | `app/src/code/local_code_editor.rs:940`, `code/global_buffer_model.rs:1400` |
| the protocol | `crates/remote_server/proto/remote_server.proto`, `diff_state.proto` |
| OSC 52 | `crates/warp_terminal/src/model/grid/ansi_handler.rs:1166`, `app/src/terminal/view.rs:12071` |
| the guard on new `session_type()` readers | `app/src/terminal/model/session/filesystem_tests.rs:367` |

## Open, as of this date

- Whether the pane in the 2026-09-05 screenshot was routed. `session inspect`
  answers it; it was not run.
- `RemoteDiffStateModel` against a WSL host: never opened.
- Global search's `UnsupportedSession` arm is a hard block
  (`workspace/view/global_search/view.rs:366`), the left panel sets that state
  for every WSL pane, and T16 measured routed search working. Those three
  facts have not been reconciled.
- The default of `terminal.osc52_clipboard_access`.
- For the cheap LSP shape: whether a Linux `rust-analyzer` accepts the URIs a
  Windows-side client sends, and what `crates/lsp` does with a root it cannot
  `canonicalize`.
