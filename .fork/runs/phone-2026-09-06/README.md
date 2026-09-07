# The phone checklist, on an Android emulator

2026-09-06 night. The maintainer asked for the emulator so the checklist at
the top of `.fork/HANDOFF-MOBILE.md` could be run without a phone, and so it
could stay for development. `.fork/tools/phone.sh` is the result; the
manual's "A phone that is not a phone" says what it is, where it runs and
why (a Windows process, because WSL cannot reach the wide listener), and
the traps. Android 16 (API 36, Google APIs x86_64), Chrome 133, on the
Windows release builds `v0.fork.e8005d252`, `b6191f914` and the one the
final pass names in `driver.log`. Warp launched with `warpdev.ps1
-Instrumented -Console -Bind 192.168.254.3:41234`, the rig profile, so a
Write asks.

Three passes. The first by hand, step by step, on `e8005d252` (screenshots
`boot-first`, `chrome-ca-url`, `s1`–`s7`, `p1`–`p12`, `desk-*`); the second
and third with `phone-run.sh`, on `b6191f914` and then on the build carrying
the four fixes below (`driver.log` has all three in order, `run.out` the
last).

## The checklist, line by line

| the handoff asked | seen |
|---|---|
| the authority from `http://<bind>/ca.crt` | Chrome downloaded `warp-console-ca.crt`, 672 B, with no prompt (`chrome-ca-url.png`). Settings › Security & privacy › More security & privacy › Encryption & credentials › Install a certificate › CA certificate › *Your data won't be private · Install anyway* (`s4-ca-warning.png`) › the file in the picker › Trusted credentials, User tab: *warp fork · Warp fork console CA ed1cbed4* (`s7-user-tab.png`). Android then posts a permanent *Certificate authority installed · By an unknown third party* notification (`p4-shade.png`). Tapping the `.crt` from Downloads instead opens the certificate installer, which refuses CA certificates and says to use Settings (`s6-installed.png`, misnamed: nothing was installed by it). |
| the block's link, no warning | `https://192.168.254.3:41234/#…` opened by intent; Chrome's lock reads *Connection is secure*; the page paired on load and drew *notify me*, which only a secure context gets (`p1-pairing-page.png`) |
| *notify me*, allowed, the page in the background, the agent asks | the site prompt, Allow, `POST_NOTIFICATIONS granted=true` for Chrome. **`e8005d252`: nothing in 90 s.** `new Notification(...)` is not implemented by Chrome on Android and the catch said nothing. **`b6191f914`, posting through `/sw.js`: with Chrome in the background (HOME), nothing in 90 s; with the tab hidden behind another tab, 9 s** — `Chrome · 192.168.254.3:41…` / *Say the single word ready and nothing else.* / *asks: approval b865…:0 · Write /tmp/phone-1788744625.txt* (`n5-shade.png`), and tapping it brought the console tab forward (`n6-after-tap.png`) |
| answer from the phone | Yes is arm-then-confirm: one tap reads *tap again to allow* (`p9-armed.png`), the second answers; `permission_replied · via paired_device · answered_by control_plane · allowed`; the file inside the distribution, six bytes (`events-c.jsonl`, `trace-c.txt`) |
| a prompt from the box | `prompt_submit · via paired_device`; the panel drew it as the person's words (`desk-after-phone.png`); the trace `P from the phone: Reply with the single word phone.` |
| the block on the desk | *Waiting for a scan* (`desk-waiting.png`), *Paired at 00:48:17 UTC* (`desk-paired.png`), the chip's tooltip back to *Hand this conversation to a phone* after Stop sharing (`desk-after-stop.png`) |
| add to home screen | Chrome's menu: *Add to Home screen*; the sheet: *Install app · Warp — console* (`n23-sheet.png`); the icon on the home screen with the console's own image (`n26-home.png`); launched from there it opened in `WebappActivity`, standalone, no URL bar (`n28-standalone.png`). No WebAPK package: this image has no Play Store to mint one, so it is Chrome's shortcut-style install, which is what a phone without Google's minting gets too |
| Stop sharing | the desk's chip flipped; **the phone read `live` for another five minutes and `reconnecting` after six** (`n29-after-stop.png`, `n32-six-minutes-after-stop.png`): the second defect below |
| overnight | the final pass leaves a conversation paired (`conversation-d.txt`, `p13-overnight.png`); the morning is the measurement |

## The final pass, on `v0.fork.0fedbfda5` (02:00–02:07 UTC, 2026-09-07)

`phone-run.sh` against the build carrying the four fixes, with the previous
instance's dead device still remembered by the page:

- **The fresh code won.** The page paired on load and Chrome read
  *Connection is secure*; on the build before, the same situation read *pair
  again* (pass two's `p1-paired.png` was overwritten by this pass's; the
  record is `driver.log`, where pass two's *chrome says:* line is empty and
  this one's reads *Connection is secure*).
- **HOME: 0 notifications in 60 s. The tab hidden behind another tab: one in
  6 s**, body *asks: Write /tmp/phone-1788746528.txt* with the id gone
  (`p2-shade.png`); tapping it landed in the console tab (`p2b-after-tap.png`).
- **The phone's Yes missed this time.** After the notification tap the page
  was at its tail, the driver's fourteen quick swipes did not move it
  (`p3-ask.png` and `p4-armed.png` are the same tail), and the two taps hit
  the trace. A driver fault, not the page's: the two hand-driven answers
  earlier in the night stand. What it does show is that a tap on a
  notification about an ask lands the reader at the tail of the record and
  not at the ask; recorded below as a leftover.
- **The panel answered instead, again.** `ArmAllow` then `Allow` at 02:04:26,
  one second apart, with *active window changed* the second before and the
  driver having sent nothing; by then Warp's window measured 594×730, which
  the driver's chip clicks then fell outside of (`OUTSIDE client area`). A
  window that gets focused, resized and clicked in is a person at the desk;
  the record says `answered_by panel`, which is right either way.
- **Stop sharing and the second hand-over** therefore did not happen from the
  chip in this pass. Stop sharing was measured by hand an hour earlier
  (`n29-after-stop.png`, `n32-six-minutes-after-stop.png`, on the build
  before the fix; the fix is pinned by tests and not yet photographed).
  Conversation D was handed over with `pair show --conversation` instead and
  the emulator paired at 02:06:49 UTC (`p13-overnight.png`), *live*; Warp and
  the emulator are left running for the morning.

Leftover, not built: a notification about an ask should land on the ask
when tapped, not on the tail of the record.

## Four defects the emulator found, all fixed the same night

Each is a page or stream change with a test, in `b6191f914` and the commit
after it; `remote-control.md`'s "Measured on the emulator" carries the
short form.

1. **Chrome on Android has no `Notification` constructor.** The page posted
   with it and swallowed the error. It registers a service worker now, from
   the *notify me* tap or a start with permission already granted, and posts
   through `showNotification`; the worker handles no fetch and no push. A
   browser refusing both paths is told so in the header.
2. **A fresh code lost to a remembered device.** The second scripted pass
   opened a fresh link and the page read *NOT PAIRED · the pairing expired
   or this instance restarted — pair again* (`driver.log`, pass two): `boot` tried
   the remembered device first and had already erased the code from the URL.
   After every restart of Warp that is every phone's second scan. The code
   is redeemed first now.
3. **Stop sharing left the phone `live`.** The event stream held a copy of
   its grant and checked only its expiry; the page reused cached credentials
   until they aged out, and the reconnect reused the cached stream credential.
   The stream asks the broker on every tick and line now and ends with
   `revoked`; the page drops a cached credential on a 401 and mints once,
   and the mint is where a cut-off device learns it was cut off.
4. **A thumb scrolling up was pulled back to the tail** (`n12-top.png` then
   `n13-top.png`, twenty seconds apart): the trace poll measured *at bottom*
   when it was sent, and a poll sent from the bottom landed after the scroll.
   Measured when the list changes now.

And a small one: the notification's body spent its two lines on the
approval's id; it reads the call now.

## What the emulator cannot say

The camera (the link is opened by intent; whether the block's QR decodes on
a real sensor is unmeasured), the lock screen and doze, how a buzz feels,
and iOS entirely. **And Chrome in the background is not a defect to fix
here**: Android does not run the page while Chrome is not the foreground
app, so the foreground notification reaches a phone whose Chrome is open on
another tab and never a phone in a pocket. That is the push opt-in's case,
recorded in `remote-control.md` as unbuilt by decision.

## One thing that was not the fork

During the second pass the panel's own Yes was armed and clicked at
01:21:25 (`ArmAllow` then `Allow` in the app log, one second apart), with
the driver sending no click and Warp logging *active window changed* just
before. That is what a person at the desk looks like; the answer is in the
record as `answered_by panel`, which is correct.

## Instrument notes

- `adb exec-out screencap` through the wrapper's `tr -d '\r'` gave corrupt
  PNGs; `phone.sh shot` bypasses the wrapper.
- `uiautomator` sees Settings, the picker and Chrome's own bars, not the
  page; taps inside the console are coordinates read off screenshots, and
  the Yes button sits at 280,1130 with the page at its top, 280,920 when the
  approval is the first thing under the header.
- Gboard's *Try out your stylus* tutorial took the first typed text;
  `settings put secure stylus_handwriting_enabled 0`.
- Chrome's DevTools port (`adb forward tcp:9222
  localabstract:chrome_devtools_remote`) connected and answered nothing,
  from either side, twenty minutes; dropped.
- The page's own disclosure did the work DevTools would have: the header
  stayed silent on the worker build because nothing had failed, which is
  what pointed at Android not running the page rather than at the post.

## Files

`phone-run.sh`, `driver.log`, `run.out`, `build.txt`, `build2.txt`,
`launch.txt`, `tab.json`, `prompt-*.json`, `pair-show-2.json`,
`conversation-c.txt`, `conversation-d.txt`, `stamp*.txt`, `events-c.jsonl`,
`trace-c.txt`, `clipboard.txt`, and the screenshots named above plus the
final pass's `p1`–`p13` and `desk-*`.
