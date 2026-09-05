> Ticket T18, split out of `.fork/TASKS.md` on 2026-09-05. History; read the section you came for.

## T18 — Every turn in a WSL pane died before it began

**Found 2026-09-02 while running T17's end-to-end check**, which never reached
LSP because the session would not open. On the Windows build, `session/new` was
refused:

```
Invalid params: `cwd` does not exist on the machine running the agent:
/home/effatha/scratch-t17/repo
```

A WSL session's shell reports a Linux cwd; Warp passes it verbatim; the agent
process was started by Warp, on Windows. **Attributed by control** rather than
assumed: an *unrouted* WSL pane fails identically with `/home/effatha`, so this
is neither routing nor T16. It is invisible on the Linux build, where agent and
shell share a filesystem — which is where every other ACP measurement in this
fork was taken.

### As built

The fix already existed one module over. `local_agent::spawn_for` hit the same
bug in T6.1 and solved it by starting `claude` *inside* the distribution;
`acp_agent` never got the same treatment. `agent_argv` now wraps the configured
command in `wsl.exe --distribution <distro> --cd <dir> --exec /bin/sh -lc <cmd>`,
mirroring `spawn_for` down to the login shell and its reason. A command already
aimed at the distribution by hand is left alone, so the manual workaround
documented the same morning keeps working instead of being wrapped twice.

Rewriting to `\\wsl$\<distro>\…` is refused on `spawn_for`'s own measurements —
~13× the same tree on the Windows disk, ~50× from inside the distribution — and
because it *succeeds*, which makes it quietly slow rather than loudly wrong.

`spawn_failure_or` explains the failure as a backstop for whatever the wrap does
not cover. Both rules take `on_windows` as a parameter rather than reading
`cfg!` inside, so they are pure functions with tests that run on every platform —
the same shape as `session::filesystem::classify`, and because a
`#[cfg(windows)]` test is one nobody here can calibrate by making it fail.

**Verified live on a fresh binary with the bare command configured**: `status:
success`, working directory reported as the pane's own, and the agent's LSP tool
returning rust-analyzer's real document symbols. Checked the way an LSP answer
has to be — only the documented symbol is reported at its doc-comment line, the
three undocumented ones at their item lines.

### What this cost to find, recorded because it will happen again

Three of the four blockers hit on the way were the operator's, not the product's:

- **A fresh pane is already Ubuntu** when that is the default shell. Typing
  `wsl.exe -d Ubuntu` into it makes a nested subshell whose block never
  completes; `input submit` then answers `queued: true` forever and `agent
  prompt` refuses with `target_state_conflict`. That reads as three defects and
  is one mistake.
- **The Windows build is a second checkout** (`C:\dev\warp`) that nothing syncs.
  A build printed *"Finished in 34.12s"*, exited 0 and produced nothing, because
  that tree was 18 hours behind. `CLAUDE.md`'s existing timestamp remedy cannot
  catch it — it compares a binary in one tree to source in another.
- **An LSP crash with exit code 1** was the rustup *default* toolchain lacking
  the `rust-analyzer` component. The server is spawned from the agent's cwd, so
  the pin that matters resolves there, not in the file being asked about.

### Still open

- [ ] **`did_change_watched_files` is advertised and never sent** (T17 above).
      Editor correctness, unrelated to agents. Two candidate fixes, both the
      maintainer's call.
- [ ] **`warpctrl acp probe --cwd` validates the path locally**, so it cannot
      probe a Unix cwd from the Windows binary. Small, and it made the probe
      useless for exactly the case being investigated.

---

