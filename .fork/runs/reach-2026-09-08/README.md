# The console reached over a tailnet, 2026-09-08

The phone surface left the home LAN today. This directory is the run: the
mesh brought up, the path between the two devices measured, Warp launched
on the tailnet address and a conversation driven from the phone, the record
saying so. Everything here is as measured; the one input a person supplied
is marked.

## What was done, in order

1. Tailscale 1.102.3 installed on the PC through winget (`Tailscale.Tailscale`;
   the id is case-sensitive and the lowercase form the manual carried is
   *No package found*). `tailscale up`, a browser login; the first link
   failed on the login page with *Error 401 bad tailscale-authstate2 cookie*
   from GitHub's redirect, a second link minted after `tailscale logout`
   worked. The PC is `100.82.213.46`.
2. The Tailscale app on the Android phone (`s25`), same account:
   `100.100.201.63`.
3. `tailscale ping` and `netcheck` with ProtonVPN connected
   (`tailscale-with-protonvpn.txt`): the PC's public endpoint is a Proton
   exit, `MappingVariesByDestIP: true`, four pongs *via DERP(ord)* at 240 to
   590 ms, *direct connection not established*. The relay is Tailscale's, in
   Chicago; the nearest is Toronto at 45 ms.
4. The Windows firewall read: the console's 41234 rule is Private-profile,
   any remote address, and the Tailscale adapter is a Private network, so it
   admits the mesh with no change. The SSH rule was LAN-only; an `SSH from
   tailnet` rule for `100.64.0.0/10` was added with one elevated command.
5. `warpdev.ps1` given `-Bind tailnet` and made it the default: the address
   is read from `tailscale ip -4` at launch, the launch stops if there is
   none. `-Status -Console` printed `WARP_FORK_CONTROL_BIND =
   100.82.213.46:41234`.
6. The Windows release build, cold, because `C:\dev\warp\target` had been
   deleted for disk the day before: 16 minutes, 353 MB, `v0.fork.2e1552fc0`.
7. `rc.sh`: launch with the product profile and the console; the published
   origin is the tailnet address; `https://100.82.213.46:41234/` answers 200
   from the Windows side (`curl.exe -k`, Schannel refusing the private
   authority as it always has) and `/ca.crt` in the clear is byte-identical
   to the authority in the state directory; a text-only conversation
   started; `pair show --conversation` run in the pane so the QR stood on
   the monitor.
8. The phone scanned it. 97 seconds after the QR was printed the record
   carried `prompt_submit via paired_device` (`events-c.jsonl`, 22:27:31Z),
   the agent answered in 4.7 s, and the desk showed the phone's words as
   the person's own with *Stop sharing* in the footer
   (`desk-after-phone-prompt.png`). `tailscale status` afterwards:
   `active; relay "ord", tx 185584 rx 95288`, so the exchange went through
   the relay (`tailscale-status-after.txt`).

## What the person supplied

The phone was on cellular with Wi-Fi off during step 8 (the maintainer,
afterwards; the driver cannot see the radio). The relay state in step 3 and
step 8 says the path did not depend on the LAN either way.

Two more things from the phones, the same evening:

- **Android with Tailscale on reported *Private DNS server cannot be
  accessed* and treated the network as down**, while the pairing above was
  working through it. Android's DNS-over-TLS setting cannot reach its named
  server through the tunnel. Settings → Network & internet → Private DNS →
  Off fixed it; the tailnet pushes no resolvers (`tailscale dns status`), so
  nothing was lost.
- **The iPhone (`iphone-13`, `100.74.53.3`, Wi-Fi only, no mobile plan)
  joined, dropped off for a while, and then installed the authority.** The
  steps that worked, in order: Safari on `http://100.82.213.46:41234/ca.crt`
  offers the profile; Settings shows *Profile Downloaded* near the top
  (else General → VPN & Device Management); install; then **General → About
  → Certificate Trust Settings → full trust**. The last step is the one the
  page still needs after the install appears to succeed: until it was
  done, Safari called the console insecure. iOS's rows of the checklist
  that this leaves: Add to Home Screen from the share sheet, and whether an
  installed page there may post a notification.

## The relay is ProtonVPN's doing, measured both ways

Later the same evening, at the maintainer's hand:

- **`tailscaled.exe` excluded from ProtonVPN (exclude-mode split tunneling),
  Proton reconnected**: `tailscale ping` timed out six times, the admin
  console showed the node not connected, and the daemon's health line read
  *register request ... all connection attempts failed (... dial tcp
  [2606:b740:49::105]:443: A socket operation was attempted to an
  unreachable network)*. A `tailscale down`/`up` (this session's, and it
  cost a re-registration that failed and left the node logged out) did not
  change it. The exclusion gives a SYSTEM service no working route.
- **ProtonVPN disconnected** (`tailscale-without-protonvpn.txt`): `up`
  came back with no new login, public endpoint the home WAN
  (`71.28.77.93`), `MappingVariesByDestIP: false`, and both phones
  **direct**: the Android over cellular via its carrier's NAT
  (`174.202.64.15:5402`, 276 ms) and the iPhone on the LAN
  (`192.168.254.18:41641`, 5 ms). First pongs went via DERP while the
  direct path was being found, then switched.

So the line supports direct paths, Proton is the sole cause of the relay,
and Proton's app exclusion is not the remedy. The exclusion was removed and
Proton reconnected afterwards; the state the stop-gap lives in is the relay.

## What it means

- The fork needed no change for reach. `WARP_FORK_CONTROL_BIND` takes one
  literal IP; `tls.rs` mints the leaf per launch for whatever is bound and
  the authority the phone installed on 2026-09-07 signs it, so the phone
  trusted the new address with nothing done on it. The only code touched is
  the launcher's default.
- With ProtonVPN connected the stop-gap is relay-only. Encrypted end to end
  and the console's TLS inside that, so the words are not exposed, but every
  packet crosses Tailscale's box and the round trip is a quarter to half a
  second. `.fork/reach.html` has the three ways out; the first to try is
  Proton's split tunneling for `tailscaled.exe`, if the plan has it.
- The pairing shown here came from `pair show` typed into a pane, not from
  the `/remote-control` chip; both mint the same code and the footer chip
  read *Stop sharing* either way. The QR in the screenshot carries a code
  spent at 22:27 and dead at 22:27:54; it is kept because the earlier runs
  keep theirs.
- Seen in passing and not chased: the first turn's `session_model` row
  requested `opencode/muse-spark-1.3-c…`, a model from a different agent's
  list, restored from `acp-models.json` after last night's opencode probe.
  The agent ran `fable` regardless. That is the model picker's restore
  (T14.14) mixing two agents' lists in one file: read after the run, the
  file is one list (`default_id`, `choices`), not keyed by agent.

## Files

| file | what |
|---|---|
| `rc.sh` | the driver |
| `driver.log` | its log, timestamps UTC |
| `launch.txt` | the launcher's own output (binary, profile, bind) |
| `tailscale-with-protonvpn.txt` | status, ping, netcheck before the run |
| `tailscale-status-after.txt` | the peer's relay and byte counts after the phone's prompt |
| `pair-show-watch.json` | the watch code minted to read the origin (never scanned) |
| `prompt-c.json`, `conversation-c.txt` | the conversation the phone was handed |
| `ca.crt` | the authority as served in the clear at `/ca.crt` |
| `events-c.jsonl` | the record, with the phone's row |
| `read-c.json` | the conversation read back |
| `desk-after-phone-prompt.png` | the desk after the phone's prompt |
