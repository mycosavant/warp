# No clock, three states, and the record: measured

2026-09-06, Windows release build `v0.fork.a09fd38f2`, the maintainer's own
profile, `warpdev.ps1 -Instrumented -Console -Bind 192.168.254.3:41234`: the
rig, so the agent runs in `default` mode and a Write asks, which is what the
phone answers here. Driven by `pairing.sh` from WSL; the phone is `curl.exe`
over TLS with the authority from the morning's run (`ca.crt`, byte-identical:
the authority survived the restart). Steps 3 and 4 of `.fork/HANDOFF-MOBILE.md`.

## What was done

One conversation C (`ready`). The chip clicked once; the block photographed.
The link read off the clipboard and its code spent over TLS. The block
photographed again. From the device: a prompt asking the agent to write
`/tmp/phone-<stamp>.txt`, the approvals listed until the Write appeared, a
yes with its digest, the turn waited out. The event log for C copied, the
trace rendered in text and HTML. Stop sharing clicked; a prompt on a fresh
credential from the device. A second conversation D, the chip once, and 125
seconds of nothing; the block photographed. Close.

## What was found

**A control pairing has no clock** (`pair.json`): the redeem answered a
device token, `conversation_id` C, and **no `expires_at`**. The device
prompted and approved a turn afterwards on that token. What still ended it
is below.

**The block says what happened** (`state-waiting.png`, `state-paired.png`,
`state-expired.png`):

| moment | the block's first line |
|---|---|
| chip clicked, nothing scanned | *Waiting for a scan; the code dies at 20:09:49 UTC.* |
| four seconds after the redeem | *Paired at 20:07:53 UTC.* |
| 125 s after a click nobody answered | *The code expired unscanned, or sharing was stopped. Click /remote-control again for a new one.* |

**The record says which prompt and which answer were the phone's**
(`events-c.jsonl`, `trace-c.txt`, `trace-c.html`):

```
20:07:57.905 prompt_submit       via paired_device
20:08:07.932 permission_request  (Write /tmp/phone-1788725277.txt)
20:08:10.830 permission_replied  via paired_device · answered_by control_plane · decision allowed
```

The trace draws the prompt row as `P  from the phone: Create the file …` and
the call's decision line as `✓ done · Warp: allowed by control_plane from the
phone`, in the text and the page alike (two occurrences in the HTML). The
panel shows the prompt as the person's own words (`state-after-stop.png`).
The file exists inside the distribution with its six bytes.

**Stop sharing still cuts the phone off**: the toast, the block gone, the
chip back to `/remote-control` with its tooltip now reading *Hand this
conversation to a phone* and no *(needs WARP_FORK_CONTROL_BIND)*
(`state-after-stop.png`), and a prompt on a fresh credential minted by the
device refused, `unauthorized_local_client`, *credential is invalid*
(`prompt-after-stop.json`).

## One thing found and fixed after this run

**The chip did not flip back when the code died.** `state-expired.png` shows
the block saying the code expired while the footer still reads *Stop
sharing*. Which chip is drawn is decided at render from the pairing map, and
nothing re-rendered the footer at that moment; the block has its own tick,
the footer had none. The footer now schedules a two-second tick from the
chip's click that stops once the code is spent or dead
(`schedule_fork_remote_control_tick`). Built after this run and not yet
photographed.

## One thing about the instrument

`warpctrl agent trace` on Windows needs `--harness-dir
'\\wsl.localhost\Ubuntu\home\effatha\.claude\projects'`: without it the CLI
looked in `C:\Users\onemind\.claude\projects\-home-effatha-git-warp\`, which
exists on the Windows side with the same slug and does not hold the session
the agent wrote inside the distribution. The CLI's own `--help` says so; the
driver's first trace call did not pass it and logged zero. The console's own
view goes through the in-instance handler, which searches the guest homes.

## Files

`pairing.sh`, `driver.log`, `run.out`, `launch.txt`, `ca.crt`, `tab.json`,
`prompt-c.json`, `prompt-d.json`, `conversation-c.txt`, `pair.json`,
`prompt-phone.json`, `approvals.json`, `approve.json`, `read-c.json`,
`events-c.jsonl`, `trace-c.txt`, `trace-c.html`, `prompt-after-stop.json`,
`close.json`, and the screenshots `pairing-footer.png`, `state-waiting.png`,
`state-paired.png`, `state-after-stop.png`, `state-waiting-d.png`,
`state-expired.png`.
