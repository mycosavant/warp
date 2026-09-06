# Viewer phase 3, measured: the record of a conversation on a paired device

2026-09-05, Windows debug build `v0.fork.9ed773dc3`, the maintainer's own
profile, launched `warpdev.ps1 -Console -Exe C:\dev\warp\target\debug\warp-oss.exe`
(product profile, so `claude-agent-acp@0.73.0` inside WSL in mode `auto`,
plus `WARP_FORK_EVENT_LOG=on` and the wide bind at `192.168.254.3:41234`).
Driven by `phase3.sh`; `phase3-rest.sh` is the same without the launch, for
an instance already up.

## What was done

Paired the way a phone does: `warpctrl pair show`, the code read off the
URL's fragment, `POST /v1/pair` with it as the bearer, then one credential
per action from `POST /v1/pair/credential`. `pair.json` says what the device
got:

```
app.ping agent.list events.subscribe agent.approvals agent.deny agent.cancel agent.trace
```

A panel turn was started with `warpctrl agent prompt` (two tool calls, Read
and Bash), and the paired device asked for the record twice: eight seconds
in with no cursors, and after the turn with the first reply's line counts as
cursors. Then a scratch-profile Brave on the Windows side opened a fresh
pairing URL, and `click.ps1` tapped the newest conversation row.

## What was found

**The whole record reached the device, and the harness's file was found
inside the distribution by the instance.** `trace-2.json`'s header:

| field | value |
|---|---|
| `harness_file` | `\\wsl$\ubuntu\home\effatha\.claude\projects\-home-effatha-git-warp\cd581153-….jsonl` |
| `harness_versions` | `2.1.257` |
| `warp_lines` / `harness_lines` | 9 / 24 (17 bookkeeping) |
| `joined_calls` | 2 |
| `clock_offset_ms` | +6348 |

No `--harness-dir`, no `WARP_FORK_HARNESS_DIR`: the handler asked the
session for the native spelling of `/home/effatha/.claude/projects`, with
`effatha` taken off the session directory, and the file was there. The
clock skew between the Windows log and the distribution's file is 6.3 s on
this run against 5.5 s on phase 0's.

**The tail cursor is exact.** Trace #1, eight seconds in, held Warp's four
frame lines (`session_start`, `session_agent`, `session_mode`,
`session_model`) and no tool line yet, so no join key and the note said so.
Trace #2 with `warp_after: 4` returned the five Warp lines after them and
all 24 harness lines (`harness_after` 0), thirteen rows after folding:
`trace-2-summary.txt`. Nothing repeated, nothing missing; the join key was
read off the whole file, not the tail, which is why the tail poll still
found the harness's file.

**The page draws it** (`console-conversation.png`): the prompt, the agent's
name and version, the mode line, both calls as one row each with `done`
badges, the full Read input and the Bash command, both files' clocks
(`warp 21:37:23 → 21:37:24 · harness 21:37:29 → 21:37:30`, the skew made
visible), the results, Warp's `stop`, the agent's two text messages, and
the usage footer. `console-home.png` is the list it was opened from; the
rows carry the `›` that says they open, drawn because the device's action
list held `agent.trace`.

**What the skew does to the reading order, visible in the screenshot.**
Warp's `stop` at 21:37:27 is drawn before the agent's "I'll read the first
line…" at 21:37:29 and its answer at 21:37:32, because the harness's clock
runs six seconds ahead and the page orders by time as the CLI does. The
calls are anchored by their ids and read right; the free text around them
reads six seconds late. Disclosed in the note line and not corrected, for
the reason the observability page gives.

## Two things the run cost, both driver-side

- **The launch never returned.** `warpdev.ps1 … > launch.txt 2>&1` from the
  driver blocks until Warp exits, because Warp inherits the redirect; the
  phase 0 driver had the same shape with `tee` and this file's own docs
  already record it. Detached now, with the discovery record as the
  readiness signal.
- **`curl` from WSL cannot reach the Windows wide listener at the LAN
  address**: `connection refused` from `192.168.254.3` to `192.168.254.3`,
  while `Invoke-WebRequest` from Windows answered 200. Mirrored networking
  shares the address, not the socket. `curl.exe` from `System32`, which
  runs on the Windows stack, is what the driver uses. A phone on the LAN is
  the Windows-stack case.

## Left open

- `warpctrl agent trace --live` on this binary refused the reply with
  `missing field warp_after`: the header skips the cursor fields when they
  are zero and the CLI's reader did not default them. Fixed in source after
  this run (`#[serde(default)]`), not yet run against a rebuilt binary.
- The harness's `prompt` row is dropped on the page when Warp's copy has the
  same text, as the text renderer does; the JSON keeps both.

## Files

`driver.log`, `launch.txt`, `instance.json`, `pair-show.json` (the spent
code), `pair.json`, `trace-1.json`, `trace-2.json`, `trace-2-summary.txt`,
`events.jsonl` and `harness-session.jsonl` (both records, unedited),
`console-home.png`, `console-conversation.png`, `console-url.txt`.
