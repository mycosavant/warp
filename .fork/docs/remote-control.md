# Remote control: a conversation handed to a phone

**As of 2026-09-05.** The fork's `/remote-control`: the chip in the agent
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
   does closing Warp; pairings are process-local and never written down.

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
it handed over, with *Stop sharing* and a restart as the two ways back. The
residual is the one the console's docs already name: plaintext HTTP on the
LAN. A Tailscale address fits `WARP_FORK_CONTROL_BIND` unchanged and retires
it.

`WARP_FORK_REMOTE_APPROVE` is untouched: it is about a phone answering for
*every* agent, which is a different claim from answering for the one you
pointed at. The pinned test that grew is
`a_code_minted_for_one_conversation_buys_driving_it`, beside the watch list's.

## The pieces

| where | what |
|---|---|
| `app/src/local_control/pairing.rs` | `Scope`, `CONTROL_ACTIONS`, `revoke_conversation`, `is_controlling` |
| `app/src/local_control/confine.rs` | the grant check and the result filters |
| `app/src/local_control/remote_control.rs` | `start`, `stop`, `is_active`: the panel's calls into the pairing map |
| `app/src/terminal/view/remote_control_block.rs` | the QR block in the pane; `start_fork_remote_control` and `stop_fork_remote_control` on the terminal view |
| `agent_input_footer/mod.rs` | the chip: no login gate under fork policy, state read off the pairing, hidden for a CLI agent |
| `crates/local_control` | `CredentialGrant::conversation`, `ControlPairParams`, `conversation_id` on both pairing results |
| `console.js` | the prompt box behind `can('agent.prompt')`, a confined device landing on its conversation |

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

## Unverified, as of writing

- The QR block's layout beside a long URL on a narrow pane; in the run it
  was drawn in a 300-pixel-wide pane and the text column was clipped at the
  right, readable but not whole.
- A phone actually scanning the block, as opposed to Brave opening the link.
- Whether the `wsl.exe` relays Warp spawns for its git chip should outlive
  the instance at all. They held the wide listener until the listener was
  marked non-inheritable (`keep_from_children`, measured: a close and a
  relaunch on the same port); they still outlive it, holding nothing.
