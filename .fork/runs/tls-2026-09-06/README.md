# Trust on the wire, measured: the wide listener over TLS

2026-09-06, Windows release build `v0.fork.dd68ff0de`, the maintainer's own
profile, `warpdev.ps1 -Console -Bind 192.168.254.3:41234` (the product
profile plus the wide bind). Driven from WSL by `tls.sh` with `curl.exe` for
the phone's side; the block by `click.ps1` and `shot.ps1`; the pages by Brave
in a scratch profile on the Windows side. Step 2 of `.fork/HANDOFF-MOBILE.md`,
and the code half of step 1.

## What was done

Three passes on one binary. `tls.sh`: launch, fetch the authority in the
clear, ask for three control paths in the clear, fetch the console over TLS
with the authority trusted and without, read the leaf off the wire, run the
09-05 flow over `https://` (a conversation, `pair show --conversation`,
redeem, a prompt credential, a prompt with the page's own `Origin`, the same
with the wrong scheme, a read with the wrong scope), click the chip, split
the pane, photograph Brave on the install page, the warning page and the
pairing page. `block.sh`: the chip clicked once on an unpaired conversation,
for the block. `narrow.sh`: the same with the panel narrowed by two splits.

## What was found

**The one port speaks two protocols, and the plain half hands out the
authority and nothing else** (`driver.log`, `ca-headers.txt`,
`install-page.html`):

| in the clear, `http://192.168.254.3:41234` | answer |
|---|---|
| `GET /ca.crt` | `200`, `application/x-x509-ca-cert`, one certificate: `CN = Warp fork console CA ed1cbed4, O = warp fork`, `CA:TRUE`, ten years |
| `GET /v1/state`, `GET /`, `GET /console.js` | `200`, the install page, every time |

**Over TLS with the authority trusted, the console; without it, the warning
page** (`tls-status.txt`, `leaf.txt`, `tls-untrusted.txt`):

| over TLS | answer |
|---|---|
| `GET /` with `--cacert ca.crt` | `200`, `verify=0`, the console document |
| `GET /ca.crt` with `--cacert` | `200` |
| `GET /` with no authority | curl exit 60, no response: what a fresh phone's browser turns into a warning page |
| the leaf, read by the Windows side | TLS 1.3; `CN=Warp console 192.168.254.3`, issuer the authority above, `Subject Alternative Name: IP Address=192.168.254.3`, `Enhanced Key Usage: Server Authentication`, `notAfter 2027-10-08` |

**The 09-05 flow runs over `https://` unchanged** (`pair-show.json`,
`pair.json`, `prompt-tls.json`, `prompt-wrong-scheme.json`, `read-c.json`):
`pair show` answers a `https://` URL and `ca_url`
`http://192.168.254.3:41234/ca.crt`, and its pretty form prints *first time
on this phone: open … and install the certificate, then scan*. The redeem, a
prompt credential and a prompt with `Origin: https://192.168.254.3:41234`
all worked; the conversation had its two exchanges. The same prompt with
`Origin: http://192.168.254.3:41234` was refused, *browser-origin
local-control requests are allowed only from this instance's own console*:
the scheme is part of the origin check now. A read with the prompt
credential was `403`, as before.

**The block wraps and stacks** (`block-after-click.png`, `block-half.png`,
`block-third.png`). Beside the QR at full width the text wraps inside its
column and the certificate line is there; at about 400 points the QR is on
top and the text under it, the URL broken across three lines and every
sentence whole. Step 1's layout finding is closed. The block is drawn in the
panel's view of the conversation, which is why `block-wide.png`, taken after
Escape had moved to the terminal, shows no block: not a defect, a fact about
where the block lives.

**Brave stood in for a phone three ways** (`brave-install-page.png`,
`brave-warning-page.png`, `brave-pairing-page-secure.png`): the install page
in the clear, readable at 430 pixels; `https://` in a profile that trusts
nothing, Chromium's *Your connection is not private*,
`NET::ERR_CERT_AUTHORITY_INVALID`; and `https://` with certificate errors
ignored, the console's pairing page reading *secure context: yes — this
browser trusts the console's certificate, and notifications can work*. The
last is what a phone with the authority installed sees, as far as a
screenshot on a desktop can say, and no further.

## Two things that cost time, neither the fork's

**`curl.exe` is Schannel, and Schannel refuses the chain.** With `--cacert`
alone every TLS request answered nothing:
`CertGetCertificateChain trust error CERT_TRUST_REVOCATION_STATUS_UNKNOWN`
(`schannel-revocation.txt`). The leaf names no CRL distribution point,
because the authority is private and has none. `--ssl-no-revoke` is the
driver's remedy; phones do not check revocation against a user-installed
root and OpenSSL-built curls do not either. The first pass of this driver
recorded `000` on every TLS line for this reason (`attempt1/`).

**WSL cannot reach the wide listener at all** (`curl` exit 7,
`openssl s_client` silent), as the handoff said. The leaf was therefore read
by a PowerShell `SslStream` on the Windows side, and every phone-side call is
`curl.exe`.

## Three things about the drivers

- `slash run remote-control` answers *not available in this build*: the
  registry entry is upstream's GUI-only command and the fork's chip is the
  door. The chip was at `(905,516)` in this window (`attempt1/probe-footer.png`).
- The first click landed on *Stop sharing*, because the curl device was
  already paired for the conversation (`tls-after-click.png`, the toast
  *Remote control stopped; the phone was cut off*). That is the stop path
  working; `block.sh` clicked once on an unpaired conversation.
- The live log is `…\warp\WarpOss\data\logs\warp-oss.log`, not
  `…\warp\Warp\…`; the first two passes read a stale file and logged *0
  listener lines*. `log-listener.txt` has the line from the right file:
  `local-control wide listener started at 192.168.254.3:41234, serving TLS`.

## One close was a dialog

`narrow.sh`'s `window close` left one instance record. The screenshot
(`after-close-refused.png`) is Warp's own *Quit Warp? You have 1 process
running* dialog, with the third split pane's shell still starting. A second
`window close` ended it. So a cancellable close can be refused by that
dialog, which is one mechanism for the case CLAUDE.md records as not yet
established by running.

## Files

`tls.sh`, `block.sh`, `narrow.sh`, `build.txt`, `build2.txt`, `driver.log`,
`driver-block.log`, `driver-narrow.log`, `launch*.txt`, `ca.crt`,
`ca-headers.txt`, `install-page.html`, `console-over-tls.html`,
`tls-*.txt`, `schannel-revocation.txt`, `leaf.txt`, `log-listener.txt`,
`tab.json`, `prompt-*.json`, `conversation-c.txt`, `pair-show.json`,
`pair-show-pretty.txt`, `pair.json`, `cred-prompt.json`, `read-c.json`,
`split*.json`, `panes.json`, `close.json`, `netstat-after.txt`,
`clipboard-block.txt`, the screenshots named above and `tls-footer.png`,
`tls-block.png`, `tls-block-narrow.png`, `block-footer.png`,
`block-narrow.png`; `attempt1/` is the first pass.
