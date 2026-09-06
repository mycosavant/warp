# Remote control, measured: one conversation handed to a phone

2026-09-05, Windows debug build `v0.fork.9e496b800`, the maintainer's own
profile, `warpdev.ps1 -Console -Bind 192.168.254.3:41235` (product profile
plus the wide bind; the agent in `auto`). Driven by `rc.sh` from WSL with
`curl.exe` for the phone's side; the chip by `click.ps1`; the console in a
scratch-profile Brave.

## What was done

Two conversations, A and B, each one text-only turn. `warpctrl pair show
--conversation B` minted a control code; a device spent it the way a phone
does and then, as that device: minted a credential for `agent.prompt`,
listed conversations, tried to prompt A, tried to prompt with no
conversation, tried to trace A, prompted B, traced B. Then Brave opened a
second control code for B, typed a prompt into the box and sent it. Then the
chip in Warp's footer was clicked twice: *Stop sharing*, and
`/remote-control` again.

## What was found

**The scope and the confinement work as designed** (`driver.log`):

| as the device paired for B | answer |
|---|---|
| what the device may do | the watch list plus `agent.prompt`, `agent.approve`; `confined: B` |
| the `agent.prompt` credential's grant | `conversation: B` |
| `agent.list` | sees B only |
| prompt A | refused: *paired for conversation B only and may not prompt conversation A* |
| prompt with no conversation | refused: *may not start a new conversation* |
| trace A | refused: *may not read conversation A* |
| prompt B | ran, `created: false`, in B's own pane (`agent read` reports exchanges 2, same pane) |
| a plain `pair show` afterwards | still the watch list, not confined |

**The console lands a confined device on its conversation** with no back
button and the prompt box drawn (`console-confined.png`), and a prompt sent
from the box reached the conversation (`console-prompted.png`; the doubled
letters are `type.ps1`, which posts both the key and the char and Brave
honours both, unlike Warp; the prompt itself is what the page sent).

**The chip** read *Stop sharing* as soon as B had a live control code
(`rc-before.png`), and after the second click the pane held the block with
the QR, the link, the expiry sentence and what the phone may do, with the
link in the clipboard (`rc-after-start.png`, `clipboard.txt`).

**`agent trace --live` works on the rebuilt binary** (`trace-live.txt`), the
phase 3 leftover.

## Two defects, both found by this run

**1. A revoked phone kept working for the life of its credential.** After
*Stop sharing*, Brave typed *after stop* and sent it, and the agent answered
it (`events-b.jsonl` holds the third `prompt_submit`; `rc-after-start.png`
shows the answer). Revoking removed the device from the pairing map, and the
`agent.prompt` credential the page had minted a minute earlier lives in the
credential map for five minutes with no back-reference to the device. The
grant is what a request carries, so the grant is what has to go:
`remote_control::stop` now voids every credential confined to the
conversation before it revokes the device (`confine::purge_confined`, one
test). **Built after this run; verified in the second run below.**

**2. A text-only turn had no join key.** B's trace was *Warp's half alone*
(`trace-live.txt`): on the ACP path only tool lines carried
`linked_session_id`, so a conversation whose turns call no tool never named
the harness's file, and the phone saw Warp's frame lines and none of the
agent's words. The `session_mode` and `session_model` lines, written on every
turn after `session/new`, now carry the agent's session id. **Built after
this run; verified in the second run below.**

## One thing that cost time, and is not the fork's

The first launch could not open the wide listener: `netstat` showed
`192.168.254.3:41234` in `LISTENING` owned by the *phase 3* instance's pid,
which was dead (`tasklist` found nothing; `window close` had reported and
`instance list` was empty), with nine `CLOSE_WAIT` connections beside it. A
socket owned by a dead pid is one whose handle something inherited; no
`wsl.exe`, `node` or `claude-agent-acp` process was left on the Windows side
to name. This run used `41235`; the cause is open (`driver-attempt1.log`).

## Second run, on `v0.fork.e8a51f21d`

`rc2a.sh` and `rc2b.sh`, `driver2.log`; the two earlier attempts
(`driver2-attempt1.log`, `driver2-attempt2.log`) fell over the port and a
restored window with no terminal, and are kept.

**A text-only turn joins.** Conversation C, one turn, no tool: the trace's
header names `linked_session_id` and the harness's file inside the
distribution, 13 lines, and the agent's `ready` is a row. Before the fix the
same shape traced as Warp's half alone.

**A revoked phone is refused at once.** A device paired for C prompted it
(`before`, answered in the panel), then minted another `agent.prompt`
credential and kept it. *Stop sharing* was clicked in the footer
(`rc2-before-stop.png` at `(890,516)`, `rc2-after-stop.png`: the toast
*Remote control stopped; the phone was cut off* and the chip back to
`/remote-control`). Then:

| after the stop | answer |
|---|---|
| the kept credential | refused, `unauthorized_local_client`: *credential is invalid* |
| a fresh credential from the device | refused, `unauthorized_local_client`: *device is not paired with this instance* |
| C's exchanges | 2: `ready`, `before`; no `after` |

**The listener no longer outlives the instance.** `window close`, then
`netstat`: no `LISTENING` on `41234`. A relaunch on the same port bound it
(`local-control wide listener started at 192.168.254.3:41234`, 02:38:01).
Two orphaned `wsl.exe` relays from this instance were still alive after the
close, which is the half not fixed: they hold nothing now, and they should
not exist.

## The port, explained

The listener was inheritable. The `mio` this build locks (1.1.1) creates
Windows sockets without `WSA_FLAG_NO_HANDLE_INHERIT` (1.2.2 sets it), and
`std::process::Command` enables handle inheritance whenever it wires up
stdio, so every child Warp spawned held a copy of the listener. The
children that outlived the instance were `wsl.exe` relays for the git chip
and repository metadata, five per instance at launch and more per turn, and
`git branch` in a bash over WSL should not still be running an hour later.
Both listeners are marked non-inheritable after binding now
(`keep_from_children`). Upstream's `9282` fails to bind the same way for the
same reason. The orphans are recorded, not chased.

## Files

`rc.sh`, `driver.log`, `driver-attempt1.log`, `launch.txt`, `pair-show.json`
(spent), `pair.json`, `cred-prompt.json`, `list.json`, `prompt-*.json`,
`trace-*.json`, `read-b.json`, `trace-live.txt`, `events-b.jsonl`,
`conversation-b.txt`, `clipboard.txt`, `console-url.txt`, the second run's
`rc2a.sh`, `rc2b.sh`, `driver2*.log`, `trace-c.json`, `pair-show-2.json`,
`pair-2.json`, `cred-*.json`, `prompt-after-stop.json`, `read-c.json`,
`conversation-c.txt`, and the screenshots `rc-before.png`,
`rc-after-start.png`, `rc2-before-stop.png`, `rc2-after-stop.png`,
`console-confined.png`, `console-prompted.png`, `console-after-stop.png`.
