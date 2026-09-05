# Language servers for routed WSL buffers: the probes

2026-09-05. Board item 7, step 3 (`.fork/docs/wsl.md`). Before touching Warp,
the cheap shape from that page's ranking was measured with a stdio LSP client
(`lspprobe.py`, in this directory): initialize, `didOpen`, `documentSymbol`,
then `definition` polled until the server answers, then `hover`. Every URI
the server returns is printed verbatim, so the path question is answered from
the wire and not from a reading of the `url` crate.

The crate under test is `/home/effatha/scratch-lsp/repo`, three symbols, one
of them called from `main`. The definition asked for is `build_marker` at
line 11, column 12 (0-based).

| probe | client runs on | server | root / file URIs | first definition | log |
|---|---|---|---|---|---|
| 1 | Linux | `rust-analyzer` on the distro's PATH | `file:///home/...` | 3.19 s | `probe1-linux-control.log` |
| 2 | **Windows** (`py.exe`) | `wsl.exe -d Ubuntu -- rust-analyzer`, cwd `\\wsl.localhost\Ubuntu\home\...` | `file:///home/...` | **3.41 s** | `probe2-windows-spawns-wsl-rust-analyzer.log` |
| 3 | Windows | Windows `rust-analyzer.exe` 1.92, cwd the same UNC path | `file://wsl.localhost/Ubuntu/home/...` | 25.27 s | `probe3-windows-native-over-unc.log` |
| 4 | Windows | as probe 2, `--shell-type login`, **this repository** | `file:///home/effatha/git/warp/...` | 60.58 s from cold | `probe4-warp-repo-windows-spawns-wsl.log` |

What each row settles:

- **Probe 2 is the cheap shape working end to end from the side Warp runs
  on.** A Windows process spawned the distribution's own server, gave it a
  UNC working directory, sent Linux `file://` URIs, and got Linux URIs back,
  in the same time as the native Linux control. The open question on the WSL
  page -- whether a Linux `rust-analyzer` accepts the URIs a Windows-side
  client sends -- is answered: it does, when the client sends Linux ones.
  What it does not accept is what `crates/lsp` sends today, because
  `url::Url::from_file_path` refuses a Linux path on Windows outright
  (`Path::is_absolute` is false without a drive or UNC prefix). That is the
  whole gap, and it is a path-spelling gap, not a protocol one.
- **Probe 3 is upstream's shape for an unrouted pane**, and the number
  carries a confound worth stating: 18 of the 25 s were `Roots Scanned`, and
  the server's `cargo metadata` call also failed on an argument its cargo
  did not know, so part of the time is the Windows toolchain and not only the
  redirector. It is still the shape that needs a Windows-installed server
  for a Linux crate, and 185 lines of stderr against 2.
- **Probe 4 is the answer on a real workspace.** Sixty seconds from a cold
  server to a correct definition in this repository, and the hover text
  (`pub(crate) fn path_to_lsp_uri(path: &Path) -> Result<Uri>`) matches the
  source signature exactly, which a fabricated answer would not.

Two facts from the same session that the design rests on, both measured
rather than read:

- `wsl.exe` started from a UNC working directory runs its child at the
  Linux directory that path stands for (`pwd` answered
  `/home/effatha/git/warp`), so `Command::current_dir` needs no translation.
- `wsl.exe --shell-type login` gives the child the user's profile PATH. The
  default PATH on this distribution had nothing under `/home`; the login one
  had three entries there. A server installed under `~/.cargo/bin` is
  invisible without it.
