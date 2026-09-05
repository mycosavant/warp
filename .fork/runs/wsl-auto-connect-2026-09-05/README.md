# WSL auto-connect, first runs

**2026-09-05.** Board item 7, step 1: a WSL pane attaches Warp's
remote-development server to its own distribution when the shell bootstraps.
Both runs are the Windows **debug** binary under `WARP_DATA_PROFILE=wslauto`,
a second instance beside the product one (which was up, from `target\release`
at `14a9a5384`, and could not be rebuilt without closing it). The account of
both runs is in `.fork/docs/wsl.md`, "Connecting, as built"; the files here
are the log lines they rest on, filtered with

```
grep -n "bootstrapped\|remote_server\|remote server\|Remote server\|wsl_transport\|model_events\|CloudObjects" warp-oss.log
```

| file | build | outcome |
|---|---|---|
| `run1-ea61116e1.log` | the arm alone | fired 3 s after launch, cold daemon started, handshake refused on a version mismatch, **staged symlink deleted** by upstream's repair |
| `run2-1a42ecdb8.log` | plus the Oss version rule | routed in ~1 s; second tab routed in the same second; symlink intact |
| (not kept; the log is recreated per launch) | `1a42ecdb8`, evening | both restored panes routed at launch; diff panel opened routed on this repo for the first time, works; `C:\dev\shots\diff-routed-{2,3}.png` |
| (not kept) | `099b26ea5` | routed in the repo and in `/tmp`: diffs, then "Diffs only work for git repositories"; `diff-routed-4.png`, `diff-routed-norepo.png` |
| `run5-099b26ea5-autoconnect-off.log` | `099b26ea5`, `WARP_FORK_WSL_AUTO_CONNECT=0` | zero connect attempts; **unrouted diff panel works on this repo**, 3 s to the panel and 27 s to the 6505-file walk (the `repo_metadata::local_model` line; this file is filtered with the pattern above plus `code_review|Code Review|repo_metadata|attaching`); in `/tmp` the WSL text, unwrapped -- `diff-unrouted.png`, `diff-unrouted-norepo.png` |

One line in run 2 is not about WSL: `CloudObjects::Listener: Attempting to
start websocket connection`, then `failed to connect ... missing
authentication`. Board item 3 names that WebSocket as the path the egress
deny-list cannot see. It fails on the account gate today; it is still
attempted.

Scratch profile recipe, for the next second instance on Windows: copy
`%LOCALAPPDATA%\warp\WarpOss\config\settings.toml` to
`%LOCALAPPDATA%\warp\WarpOss-<profile>\config\`, drop the
`[warp_drive.local_sync]` section so the scratch instance does not sync into
the product's mirror repository, set `default_session_mode = "terminal"`, and
launch the **debug** binary with `WARP_DATA_PROFILE=<profile>` set
(`Start-Process ... -NoNewWindow`). Onboarding needs no seeding: on Windows the
preference store is the registry under `Software\Warp.dev\WarpOss`, keyed by
app name and shared across data profiles. Every `warpctrl` call then needs
`--instance`.
