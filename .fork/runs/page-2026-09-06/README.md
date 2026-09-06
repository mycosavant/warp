# The page on the Windows build, and the h2 refusal

2026-09-06, Windows release builds `v0.fork.fbb75ca19` (first pass) and
`v0.fork.e8005d252` (second and third), the rig profile
(`warpdev.ps1 -Instrumented -Console`), so an Edit asks and the phone answers
it. The phone's calls are `curl.exe` over TLS with the authority; the page is
Brave in a scratch profile on the Windows side with `--ignore-certificate-errors`,
which is what a phone that installed the authority sees as far as a desktop
can say. Steps 5 and 6 of `.fork/HANDOFF-MOBILE.md`, and the chip fix from
the pairing run.

## What was done

`page.sh`: a conversation asked to change one line of a three-line file
with its Edit tool; the chip clicked; the code spent by curl; the Edit's
permission request listed and approved from the device; a second control
code minted by `pair show` and opened in Brave; the conversation view
photographed. Then a second conversation, the chip once, 125 seconds, and
the footer photographed. `head.sh`: the same page scrolled to its top, for
the header.

## The first pass found the listener refusing a browser

`attempt1-h2/page-conversation.png`: Brave reached `https://192.168.254.3:41234`,
read *secure context: yes*, and then *pairing failed: HTTP 403: Host header
is required for local-control requests*. Brave had negotiated HTTP/2 from
the ALPN list the listener advertised, and over h2 the authority travels as
the `:authority` pseudo-header, which the `Host` check does not read.
`curl.exe` offers HTTP/1.1 only, so the whole morning's flow over TLS had
passed under an instrument narrower than a phone. The listener advertises
HTTP/1.1 only since `e8005d252`, pinned by a test; the second pass is on that
build.

## What was found on the second and third passes

**The page, paired over TLS** (`page-conversation.png`, `page-head.png`):

| the handoff asked for | seen |
|---|---|
| *notify me* offered from a tap, only in a secure context | a *notify me* button in the header beside *unpair*, the page *live* |
| the header reading the record | *mode current `auto`; offered …* · *model `model` "Model", current `fable`* · *agent @agentclientprotocol/claude-agent-acp 0.73.0* · *in /home/effatha/git/warp*, under the title; the mode and model were the whole disclosure line in this photograph and are trimmed to the current id since |
| an edit as a diff | the `Edit` call: its file on the input line, then `/tmp/page-….txt`, `- beta`, `+ delta`, and *Warp: allowed by control_plane from the phone* on its decision line |
| the answer from the phone | `approve.json` `ok: true`; the file read `alpha delta gamma` afterwards |

What a desktop cannot show: the notification itself. The permission prompt
has to be tapped by a person and the page has to be in the background of a
phone; that row is on the checklist at the top of the handoff.

**The chip flips back on its own** (`chip-waiting.png`, `chip-after-expiry.png`):
*Stop sharing* while the code waited, `/remote-control` again 125 s later,
with the block's first line reading expired. This is the fix from the
pairing run (`8c08afe63`) measured.

## Files

`page.sh`, `head.sh`, `build.txt`, `build2.txt`, `driver.log`,
`driver-head.log`, `run2.out`, `launch*.txt`, `ca.crt`, `tab.json`,
`prompt-*.json`, `conversation-c.txt`, `pair.json`, `approvals.json`,
`approve.json`, `pair-show-*.json`, `close.json`, `page-conversation.png`,
`page-head.png`, `chip-waiting.png`, `chip-after-expiry.png`;
`attempt1-h2/` is the first pass on the h2-advertising build.
