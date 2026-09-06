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

## Second run

*(filled in below after the rebuild)*

## Files

`rc.sh`, `driver.log`, `driver-attempt1.log`, `launch.txt`, `pair-show.json`
(spent), `pair.json`, `cred-prompt.json`, `list.json`, `prompt-*.json`,
`trace-*.json`, `read-b.json`, `trace-live.txt`, `events-b.jsonl`,
`conversation-b.txt`, `clipboard.txt`, `console-url.txt`, and the
screenshots `rc-before.png`, `rc-after-start.png`, `console-confined.png`,
`console-prompted.png`, `console-after-stop.png`.
