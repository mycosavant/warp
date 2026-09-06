# Remote control: a conversation handed to a phone

**As of 2026-09-06.** The fork's `/remote-control`: the chip in the agent
footer and the slash command, meaning what Claude Code's `/remote-control`
means. Upstream's chip of that name relays the pane through Warp's sharing
server behind a login (T19 read it; nothing to point elsewhere). Under fork
policy the same chip hands the pane's conversation to a phone through the
console the fork already had.

## What it does

1. **At the machine**, in a pane whose agent panel has a conversation, click
   `/remote-control` in the footer or type it. A block appears in the pane
   with a QR code, the link behind it, when the code dies, and what the phone
   will be able to do. The link is copied too, which is upstream's own gesture
   for the chip.
2. **On the phone**, scan it. The console opens straight onto that
   conversation: its record live (the phase 3 view), its permission requests
   with *No* and *Yes*, a *Stop* button while a turn runs, and a prompt box.
3. **Stop sharing**, the same chip once it is active, cuts the phone off. So
   does closing Warp, and so does deleting the conversation; pairings are
   process-local and never written down. There is no clock on it (2026-09-06).

The phone gets *that conversation and no other*. It cannot start a new one,
prompt another, answer another's requests, or see the others in the list.

## What it needs

- `WARP_FORK_CONTROL_BIND` set to an address the phone can reach: `ggwarpdev
  console` sets `192.168.254.3:41234`, the one the Windows firewall rule
  names (`manual.md`, "Reaching the console from a phone"). Without it the
  chip's click is a toast naming the variable; nothing else in the flow
  exists.
- `WARP_FORK_EVENT_LOG=on` for the record to draw from; the product profile
  sets it since phase 3.
- A conversation in the panel. A CLI agent in a pane is not one, and the fork
  hides the chip there: Claude Code in a pane has a `/remote-control` of its
  own, which is the one to use.

## How it is built, and where the authority sits

**One new thing in the pairing map and nothing new served.** A pairing code
carries a *scope* (`app/src/local_control/pairing.rs`, `Scope`): `Watch`,
which is what `warpctrl pair show` has always minted, or `Control` for one
conversation, which the chip and `warpctrl pair show --conversation <id>`
mint. The device that spends a control code holds the watch surface plus
`agent.prompt` and `agent.approve`, and every credential it mints from
`/v1/pair/credential` carries the conversation on its grant
(`CredentialGrant::conversation`).

**The confinement is on the grant, checked before every handler.**
`app/src/local_control/confine.rs`: a prompt, a trace or a cancel must name
that conversation; an approval answered must be one of that conversation's
(the ACP population carries `conversation_id` since phase 3; a pane agent's
request has none and is refused); a prompt with no conversation is refused,
because a new conversation is not the one handed over; anything outside that
small set is refused whatever the pairable list says. The two instance-wide
reads it holds, `agent.list` and `agent.approvals`, have their arrays filtered
to the one conversation, and the event stream is filtered on `session_id` the
same way. An unconfined grant, every local client and every watch device, is
untouched by all of it. Six tests, pure, no app.

**And a named conversation is continued in its own pane now.** `agent.prompt`
used to resolve the pane from the target selector and continue the named
conversation there, which was harmless from a CLI addressing the active pane
and wrong for a phone, whose prompt arrives with no target while the person
may be looking at another pane. The conversation's pane wins when it has
one; the target still decides when it does not.

## Why this is defensible, in the fork's own terms

The fork's rule is that authority follows credential strength, and a QR code
is weak: a bearer shown to a room, spendable for two minutes. That is why
`PAIRABLE_ACTIONS` has no `agent.prompt` and why `agent.approve` sits behind
`WARP_FORK_REMOTE_APPROVE`. This does not change the credential. What it adds
is two things the watch scope lacks.

The *gesture*: the code exists because a person at the keyboard pointed at a
conversation and said "drive this from my phone". That is a stronger consent
than a variable set once, and it is exactly Claude Code's shape for the same
feature. And the *confinement*: what a stolen control token buys is one
conversation the owner already handed to a phone, for as long as they leave
it handed over, with *Stop sharing*, deleting the conversation and a restart
as the ways back. The residual this page named until 2026-09-06, plaintext
HTTP on the LAN, is closed by "Trust on the wire" below for a phone that
installed the authority; what remains is reach, and a Tailscale address fits
`WARP_FORK_CONTROL_BIND` unchanged for that.

`WARP_FORK_REMOTE_APPROVE` is untouched: it is about a phone answering for
*every* agent, which is a different claim from answering for the one you
pointed at. The pinned test that grew is
`a_code_minted_for_one_conversation_buys_driving_it`, beside the watch list's.

## Trust on the wire (2026-09-06)

The second of the four decisions of 2026-09-06, built the same day. A LAN
address over plain HTTP is not a secure context, so a phone at the console
could be looked at and not told anything: the browser withholds the
Notification API, service workers, push and Chrome's install prompt. The wide
listener speaks TLS now, and the trust is an authority Warp mints itself
(`app/src/local_control/tls.rs`):

- **The authority** is minted once, under `fork::state_dir()` beside the
  discovery record: `console-ca.key`, owner-only from its first byte through
  `create_private_file`, and `console-ca.crt`. It is rebuilt on every launch
  from its key alone, so nothing else is persisted; its name carries four
  bytes of the key id, so two machines' authorities read differently on one
  phone. Ten years. If either file cannot be read the authority is minted
  again and the log says so, because the phone then has to install the new
  one.
- **The server certificate** is minted per launch, in memory, for the bind
  address as an IP subject alternative name, 397 days, `serverAuth`, signed by
  the authority. Changing `WARP_FORK_CONTROL_BIND` changes nothing the phone
  has to redo.
- **One port, two protocols.** `tls::serve` peeks the first byte of each
  connection: a TLS handshake goes to the acceptor and the router; anything
  else goes to a two-route plain router that answers `/ca.crt` with the
  authority and every other path with a constant install page
  (`console_install.html`, one section per platform). The authority has to
  reach the phone before the phone trusts the listener, and serving it only
  over TLS would be the tap-through the decision refused. Nothing under `/v1/`
  is reachable in the clear; a test asks for `/v1/state` without TLS and gets
  the install page.
- **The loopback listener stays plain.** `warpctrl` is not a browser and
  `127.0.0.1` is already a secure context; the discovery record still says
  `http://`. Pinned on the source by
  `the_loopback_listener_stays_plain_and_only_the_wide_one_speaks_tls`.
- **Failing to prepare the material is handled like a failed bind**: logged,
  no wide listener, loopback keeps serving. There is no fallback to
  plaintext.

Around it: the QR's URL is `https://`; `PairingResult` carries `ca_url`, the
plain address of the authority, which the block in the pane and `pair show`
print as the first thing to do on a new phone; the same-origin check compares
the scheme the listener speaks (`ListenerOrigin`), so a plaintext origin at the
TLS listener's address is refused; the pairing page reads
`window.isSecureContext` and says which it is, and links `/ca.crt` with one
sentence per platform.

What this does not do: it does not verify the phone. The credential story is
unchanged, and a phone that never installs the authority can still tap through
its browser's warning and reach the same console; it gets nothing a secure
context would have given it, and its token travels in the clear to whatever
answered its handshake.

Drivers: `curl.exe --cacert <ca.crt>` from WSL; Brave on the Windows side in a
scratch profile with `--ignore-certificate-errors` for a first look at the
page. The measurement that counts is a phone with the authority installed and
no warning page, and that needs a person with a phone.

## How long it lasts (2026-09-06)

The third decision. A device paired by `warpctrl pair show` still lapses after
twelve hours: a weak credential minted by a variable gets a clock. A device
paired by a code minted for one conversation has none (`PairedDevice::expires_at`
is `None`, `PairedDeviceResult::expires_at` is absent), and ends on:

- *Stop sharing*, which purges every credential confined to the conversation
  and then revokes the device (`remote_control::stop`);
- the conversation being removed from the history model, by deletion or by an
  empty conversation being cleared, through the same call from
  `BlocklistAIHistoryModel::remove_conversation_from_memory`;
- Warp closing, since the map is in memory.

The block's "dead at HH:MM:SS" describes the code, and says so. Pinned by
`a_control_pairing_outlives_the_working_day_and_a_watch_pairing_does_not`.

## What the record says (2026-09-06)

The fourth decision: recorded, not marked. A prompt or an answer that came
through a grant confined to one conversation, which only a phone paired by
`/remote-control` holds, is stamped `via: paired_device` on its `prompt_submit`
or `permission_replied` line in the event log. `answered_by` is unchanged and
still names the door (`control_plane`); `via` names who was at it. The bridge
derives it from the grant after the confinement check and threads it to
`agent_prompt` and `agent_answer`; `prompt_submit` is written from a status
transition with no request to read, so the handler leaves a per-conversation
note the writer takes (`event_log::set_prompt_origin`/`take_prompt_origin`,
taken so an unconsumed note cannot stamp the person's next prompt). The trace
and the console draw those rows with "from the phone"; the panel shows the
prompt as the person's own words.

## The pieces

| where | what |
|---|---|
| `app/src/local_control/tls.rs` | the authority, the per-launch server certificate, `serve` (TLS to the router, plain to `/ca.crt` and the install page), `CA_PATH`; `console_install.html` is the page |
| `app/src/local_control/mod.rs` | `ListenerOrigin` (scheme and authority per listener, for the `Host` and `Origin` checks), `handle_ca_request`, the wide bind refusing to open without TLS material |
| `app/src/local_control/pairing.rs` | `Scope`, `CONTROL_ACTIONS`, `revoke_conversation`, `is_controlling`, `ca_url`; a control device with no `expires_at` |
| `app/src/event_log/mod.rs` | `via` on every line, `VIA_PAIRED_DEVICE`, `set_prompt_origin`/`take_prompt_origin`; `bridge.rs` derives `via` from the grant |
| `crates/warp_cli/src/local_control/trace_render.rs`, `console.js` | `via_label`/`viaLabel`: "from the phone" on a stamped prompt or answer |
| `app/src/local_control/confine.rs` | the grant check and the result filters |
| `app/src/local_control/remote_control.rs` | `start`, `stop`, `is_active`: the panel's calls into the pairing map |
| `app/src/terminal/view/remote_control_block.rs` | the QR block in the pane; `start_fork_remote_control` and `stop_fork_remote_control` on the terminal view |
| `agent_input_footer/mod.rs` | the chip: no login gate under fork policy, state read off the pairing, hidden for a CLI agent |
| `crates/local_control` | `CredentialGrant::conversation`, `ControlPairParams`, `conversation_id` on both pairing results |
| `console.js` | the prompt box behind `can('agent.prompt')`, a confined device landing on its conversation; the pairing page's secure-context line and `/ca.crt` link |

## Measured 2026-09-05 (`.fork/runs/remote-control-2026-09-05/`)

On the Windows build: the scope, the grant, the filtered list, the three
refusals with their sentences, a prompt from the device landing in the
conversation's own pane, the console landing a confined device on its
conversation with the prompt box, a prompt sent from the box, the chip
reading *Stop sharing* and the block appearing in the pane on the second
click. Two defects found and closed the same night:

- **A revoked phone kept working for the life of its credential.** *Stop
  sharing* removed the device from the pairing map; the `agent.prompt`
  credential it had minted a minute earlier lived on for five minutes with
  no back-reference to the device, and a prompt sent after the stop reached
  the agent. `remote_control::stop` now voids every credential confined to
  the conversation before revoking the device (`confine::purge_confined`).
  The grant is what a request carries, so the grant is what has to go.
- **A text-only turn had no join key**, so the phone saw Warp's frame lines
  and none of the agent's words: on the ACP path only tool lines carried the
  agent's session id. `session_mode` and `session_model` carry it now.

## Being told, and what the record shows (2026-09-06, page only)

Steps 5 and 6 of the handoff, in `console.js`, photographed on the Windows
build in `.fork/runs/page-2026-09-06/` (a desktop Brave standing in for a
phone that installed the authority).

- **Foreground notifications.** A *notify me* button in the header, drawn
  only in a secure context where the Notification API exists and permission
  has not been decided, asks from that tap and never on load. With
  permission granted and the page hidden, a `permission_request` or a
  `stop`/`stop_failure` on the conversation this device is on posts a
  notification titled with the conversation's first prompt and buzzes. The
  events stream already carries both; nothing is sent anywhere. Web Push is
  **not built**: it would reach a locked phone through Google's or Apple's
  relay, which then learns when this machine had something to say. Recorded
  here as an opt-in behind its own variable, off, that does not exist yet; to
  be built only if a week says the foreground half is not enough.
- **The header reads the record.** Mode, model, agent and directory come off
  the `session_mode`, `session_model`, `session_agent` lines and any Warp
  line's `cwd`, refreshed on every trace poll, since "is the agent asking me
  or its classifier" is the first thing a phone wants to know.
- **An edit is a diff.** `Edit`'s `old_string`/`new_string`, `MultiEdit`'s
  list and `Write`'s content are drawn as `-`/`+` lines under the call, from
  the harness's own `tool_use` block, in the page (`callEdit`, text nodes
  only) and in `warpctrl agent trace`'s text and HTML (`Edit` in
  `trace_render.rs`, on the `similar` crate). The input line becomes the file.

## Measured 2026-09-06 (`.fork/runs/tls-2026-09-06/`, `.fork/runs/pairing-2026-09-06/`)

On the Windows release builds `v0.fork.dd68ff0de` and `v0.fork.a09fd38f2`:

- **The wire.** In the clear the wide port answers `/ca.crt` with the
  authority and every other path, `/v1/state` included, with the install
  page. Over TLS with the authority trusted, the console; without it, curl
  exit 60 and Chromium's warning page. The leaf read off the wire: TLS 1.3,
  `CN=Warp console 192.168.254.3`, an IP SAN, `serverAuth`, signed by
  `Warp fork console CA ed1cbed4`. The 09-05 flow ran over `https://`; the
  same prompt with an `http://` origin was refused. The authority was
  byte-identical across a restart. Brave with certificate errors ignored read
  the pairing page's *secure context: yes*.
- **The block** wraps beside the QR and stacks under it at about 400 points,
  with the certificate line in place; it reads *Waiting for a scan; the code
  dies at …*, then *Paired at …* four seconds after a redeem, then *The code
  expired unscanned …* 125 s after a click nobody answered.
- **No clock.** The redeem for a control code carried no `expires_at`; the
  device prompted and approved afterwards; Stop sharing refused its next
  credential with *credential is invalid*.
- **The record.** `prompt_submit via paired_device`, `permission_replied via
  paired_device · answered_by control_plane · decision allowed`, and the
  trace's *from the phone* on the prompt row and the decision line, text and
  HTML. The file the phone asked for existed in the distribution.
- **The page** (`.fork/runs/page-2026-09-06/`, Brave with certificate errors
  ignored, HTTP/1.1): paired and *live*, *notify me* in the header, an `Edit`
  drawn as `- beta` / `+ delta` under its file with *Warp: allowed by
  control_plane from the phone* on its decision line, and the header reading
  the record's mode, model, agent and directory. The chip flipped back to
  `/remote-control` on its own 125 s after a code nobody scanned.
- **Three things the runs found**: the chip did not flip back from *Stop
  sharing* when a code died, because nothing re-rendered the footer (a tick
  from the click now does, measured on the page run:
  `.fork/runs/page-2026-09-06/chip-after-expiry.png`); `warpctrl agent trace`
  on Windows needs `--harness-dir` naming the distribution, as its help says;
  and **the first browser to reach the TLS listener was refused every
  request**, *Host header is required*, because it negotiated HTTP/2 from
  the ALPN list and over h2 the authority is a pseudo-header the `Host` check
  never sees. `curl.exe` offers HTTP/1.1 only, so the morning's flow had
  passed under an instrument narrower than a phone. The listener advertises
  HTTP/1.1 only now (`e8005d252`).

## Unverified, as of writing

- **A phone.** Nothing so far has been one: Brave on the Windows side stood
  in. Scanning the block, installing the authority from `http://<bind>/ca.crt`
  on Android (Settings › Security › Encryption & credentials › Install a
  certificate › CA certificate) and on iOS (the profile, then Certificate
  Trust Settings), `https://<bind>` with no warning, *secure context: yes* on
  the pairing page, *notify me* offered and a notification arriving with the
  page in the background, adding to the home screen (Chrome should now offer
  a real install; iOS adds it either way), a lock and unlock mid-turn, and
  the pairing surviving overnight. Each is one line in the next run's README.
- Whether the `wsl.exe` relays Warp spawns for its git chip should outlive
  the instance at all. They held the wide listener until the listener was
  marked non-inheritable (`keep_from_children`, measured: a close and a
  relaunch on the same port); they still outlive it, holding nothing.
