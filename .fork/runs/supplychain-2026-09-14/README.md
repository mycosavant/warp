# claude-agent-acp from a lock, installed offline, one panel turn each side, 2026-09-14

Run from `.fork/HANDOFF-SUPPLYCHAIN.md` steps 1 and 2. The survey and the
design are in `.fork/docs/agents-supply-chain.md`; this is the measurement.
Claims are tagged RAN, READ, TOLD or ASSUMED.

## What was built

- `.fork/agents/claude-agent-acp.toml`: 105 npm packages and the Node v24.21.0
  runtime for linux-x64, each with URL and sha256, npm's sha512 kept beside it.
  Written by `.fork/tools/agents.py lock` from the `package-lock.json` of the
  npx cache the 2026-09-13 census ran against
  (`~/.npm/_npx/6130df88bef2cca3`), so the locked tree is the measured tree.
- `.fork/tools/agents.py`: `lock`, `fetch`, `install`, `path`.

## Results

1. **The lock reproduces the census tree.** Installed `node_modules` against
   the npx cache, `diff -rq`: no file differs; the cache has two extra entries,
   npm's `.bin` and `.package-lock.json`. The `claude` binary is
   `9a64bda9d8722a1f…` in both (RAN, `linux/install.txt`).
2. **Install works with no network.** `unshare -rn`, control: `curl` inside it
   could not resolve `registry.npmjs.org` and only `lo` exists, down. Install
   8.45 s wall, 382 MB, tree read-only, no `npm` or `npx` anywhere in it (RAN,
   `linux/install.txt`).
3. **A tampered blob is refused.** One flipped byte in the cached `zod`
   tarball: refused naming both hashes, nothing left in the agent directory.
   The same copy with the byte restored installs (RAN, `linux/tamper.txt`).
   The first tamper attempt copied the cache one directory off and tested
   nothing; its failed install also left a `.partial` directory, which the
   tool now removes. Both are fixed and the file holds the rerun only.
4. **Headless probe.** `warpctrl acp probe --command <installed launcher>`:
   `agentInfo` 0.73.0, answer "2 + 2 is 4.", `end_turn`, on the maintainer's
   subscription (RAN, `linux/probe-wsl.ndjson`). The wrapping shell printed
   start 09:39:18Z and end 09:39:20Z; those times are not in the file.
5. **Linux panel turn.** `target/release/warp-oss` `v0.fork.7d8e21b76`, scratch
   `XDG_CONFIG_HOME`/`XDG_STATE_HOME`/`XDG_DATA_HOME`, `WARP_FORK_ACP_COMMAND`
   set to the launcher. `session_agent` names the launcher path and
   `@agentclientprotocol/claude-agent-acp 0.73.0`; `agent read` gives
   `status: success` and "2+2 is 4." (RAN, `linux/`, screenshot included).
6. **Windows panel turn, agent inside WSL.** `C:\dev\warp\target\debug\warp-oss.exe`
   at `ac5f6b25f` (the checkout's HEAD; built two minutes after that commit,
   no version sidecar), `WARP_DATA_PROFILE=supplychain`,
   `WARP_FORK_ACP_COMMAND='wsl.exe -d Ubuntu -- <launcher>'`. Attempt 3: a WSL
   pane at `/tmp/sc-run/ws`, `session_agent` 0.73.0 from `wsl.exe`, mode and
   model reported, `status: success` (RAN, `windows/attempt3/`).

## The two Windows attempts that failed, and why neither is about the install

- **Attempt 1:** a fresh `WARP_DATA_PROFILE` opens `pwsh`, not bash in Ubuntu,
  so `cd /tmp/sc-run/ws` did nothing and Warp sent `cwd: C:\Users\onemind`.
  The agent answered `initialize` as 0.73.0 and refused `session/new`:
  *"`cwd` must be an absolute path"* (RAN). The driver's first version also
  passed an `--instance` flag `warpctrl` does not have; `drive2.txt` is the
  prompt sent by hand after that.
- **Attempt 2:** `settings.toml` given the maintainer's
  `[session.new_session_shell_override] wsl = "Ubuntu"`, and the pane was still
  `pwsh`, because the profile's `warp.sqlite` restored attempt 1's pane
  (`open_from_restored` in the log) (RAN). Deleting the scratch profile's
  `warp.sqlite*` gave attempt 3 a WSL pane.

So a Windows scratch profile needs both: the shell override and no restored
session.

## Attempt 4: the bare `$HOME` form on Windows

`WARP_FORK_ACP_COMMAND='$HOME/.local/share/warp-fork/agents/claude-agent-acp/0.73.0/bin/claude-agent-acp'`,
no `wsl.exe`, same profile, WSL pane restored from attempt 3. Warp wrapped it
itself (`acp_agent::agent_argv`, `wsl.exe --exec /bin/sh -lc`), `sh` expanded
`$HOME` inside Ubuntu, `session_agent` named the command verbatim, and the
turn ended `status: success` (RAN, `windows/attempt4-autowrap-home/`). The
manual's Windows line uses this form.

## After review, same day

A reviewer agent found that `install --force` removed the old install before
verifying the new one, and that the probe and panel turns above had run
against a tree installed by a pre-fix `agents.py`. Fixed in the tool, then
(RAN, `linux/force-bad-cache.txt`): `--force` against a cache with one
flipped byte refuses and the old install still answers `initialize`; the real
install was redone with the committed tool, with the network cut, and
`diff -rq` against the census tree is still clean apart from npm's two
bookkeeping entries. A probe of the reinstalled agent ended `end_turn`. The
panel turns were not repeated.

## Not established

- The census was not re-run against the installed tree. The adapter and the
  `claude` binary are byte-identical to what it measured; Node is v24.21.0
  where the census ran v24.5.0 (ASSUMED not to change what `claude` sends,
  since `claude` is a separate Bun-compiled binary).
- One turn each, no tool use, no permission request.
- Left running after the Linux window closed: the `remote-server-daemon` and
  its `terminal-server` that this launch's WSL auto-connect started (pids
  629913/629915, started 09:41:21Z, the launch second). Not killed.
- Scratch left on disk, outside the repo: `/tmp/sc-run`, `/tmp/sc-src`,
  `/tmp/sc-node`, `/tmp/sc-resolve`, and the Windows profile
  `%LOCALAPPDATA%\warp\WarpOss-supplychain`. The installed agent under
  `~/.local/share/warp-fork/agents/` and the blob cache under
  `~/.cache/warp-fork/agents/` are kept on purpose, since the launch lines now
  point there.
