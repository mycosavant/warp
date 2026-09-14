# Handoff: the phone surface

Written 2026-09-06 for a fresh session. Repo `/home/effatha/git/warp`, branch
`dev`, HEAD at or after `29308327a`. Everything below is either measured and
says so, or read and says that.

## Where it stands, 2026-09-08 evening

Reach, the last unbuilt half of "a phone anywhere", is on a stop-gap:
Tailscale on the PC (`100.82.213.46`) and the Android phone
(`100.100.201.63`), Tailscale's own coordination server, one free account.
`warpdev.ps1 -Console` binds the console to the tailnet address by default
now (`-Bind tailnet`, resolved at launch; the launch stops if Tailscale gives
no address). Measured with ProtonVPN connected: the two nodes reach each
other only through Tailscale's DERP relay, 240-590 ms, because the Proton
exit is a symmetric NAT (`.fork/runs/reach-2026-09-08/`). The destination is
headscale on a VPS with a relay of your own; the choice, the refusals
(Cloudflare Tunnel by name), the provider shortlist and the steps are
`.fork/reach.html`, and the maintainer owns that ops. The Windows release
binary was deleted on 2026-09-07 for disk and was being rebuilt as this was
written; the end-to-end run from cellular is what that build is for.

## Where it stands, 2026-09-07 morning

The overnight line is measured. Conversation D, paired on the emulator at
02:06 UTC, took a prompt from the phone at 12:45 with no re-pair
(`prompt_submit · via paired_device` in
`.fork/runs/phone-2026-09-06/events-d.jsonl`). The first turn failed, and
the failure was not the fork's: the agent's Claude OAuth session had expired
overnight and could not be refreshed, and Claude Code in a terminal refused
the same way at the same moment. `/login` there, nothing touched in Warp,
and the retry from the phone at 12:50 answered. So every line of the
checklist that a browser or Android decides is now measured on the
emulator; what remains needs a phone (camera, lock screen, iOS) and is
listed below.

One thing worth knowing from it: an agent's own credential expiring looks,
from the phone, like the fork refusing a prompt. The record says
`stop_failure` with the agent's error text, and the remedy is at the desk,
in the agent's own login, not in Warp.

## Where it stood, 2026-09-06 night

The checklist below was run on an Android emulator (`.fork/tools/phone.sh`,
`.fork/runs/phone-2026-09-06/`, and `remote-control.md`'s "Measured on the
emulator") and every line a browser or Android decides is measured: the
authority installed through Settings, the link opened with no warning, the
page paired over TLS, a notification arriving with the tab hidden, Yes and a
prompt from the phone in the record, the install to the home screen opening
standalone. Four defects found and fixed the same night, all page-side or in
the stream (a fresh code lost to a remembered device; Stop sharing leaving
the phone `live` for five minutes; a scroll pulled back to the tail; the
notification body spent on an id). One limit stands and is not a defect:
**with Chrome in the background as an app, Android does not run the page**,
so the foreground notification reaches a phone whose Chrome is open on
another tab and not a phone in a pocket. That is the push opt-in's case,
still unbuilt by decision. What a person with a real phone still owns: the
camera, the lock screen, iOS.

## Where it stood, 2026-09-06 evening

Steps 1 (its code half), 2, 3, 4, 5 and 6 below are built and committed
(`5a954bf7e`..`fbb75ca19` and after). Measured on the Windows release build:
the wire (`.fork/runs/tls-2026-09-06/`), the block's layout and its three
states, the no-clock pairing and the record (`.fork/runs/pairing-2026-09-06/`).
`remote-control.md`'s "Measured 2026-09-06" and "Unverified" sections are the
current truth. **What needs a person with a phone is unchanged and is the
first thing left**, and it is now one checklist rather than step 1's
paragraph:

1. `ggwarpdev console` (or `warpdev.ps1 -Console`); on the phone, on the LAN,
   open `http://192.168.254.3:41234/ca.crt` in its browser and install the
   authority (Android: Settings › Security › Encryption & credentials ›
   Install a certificate › CA certificate; iOS: allow the profile, install it
   under VPN & Device Management, then Certificate Trust Settings › full
   trust). Record which screens it took and whether the download was
   offered at all.
2. In a pane with a conversation, click `/remote-control`; scan the block.
   Expect `https://…` with **no warning page**; the pairing page will not be
   shown because the code pairs on load, so open `https://192.168.254.3:41234/`
   in a second tab once to read *secure context: yes* or *no*.
3. Tap *notify me* in the header, allow. Put the phone down with the page in
   the background; at the desk, prompt the conversation with something that
   asks (the rig profile, `-Instrumented`, makes a Write ask). Expect a buzz
   and a notification titled with the first prompt. Note whether it arrives
   with the screen locked (it should not; that is push, not built).
4. Answer the request from the phone; send a prompt from the box; look at
   the block on the desk (*Paired at …*) and at `warpctrl agent trace <id>
   --harness-dir '\\wsl.localhost\Ubuntu\home\effatha\.claude\projects'`
   for *from the phone*.
5. Add to home screen. Expect Chrome to offer a real install now (the page
   is a secure context with a manifest); iOS adds it either way.
6. Leave it overnight. Expect the pairing to hold (no clock) and the page to
   reconnect on wake; the five-minute credentials turn over on their own.
7. Stop sharing at the desk; expect the phone refused on its next tap.

Each line is one row in `.fork/runs/phone-<date>/README.md`. Still not
built, by decision: Web Push. Still later: several pairings on one phone
(step 7 below).

## What you are doing

Making the fork's `/remote-control` good enough to live on: a phone that was
handed one conversation can be told when the agent needs it, keeps the
conversation for as long as it runs, and shows the desk what it did. The
shape is fixed and not up for redesign: a person at the keyboard points at
one conversation and hands it to a phone, the phone gets that conversation
and nothing else. The spec is `.fork/mobile.html`; the four decisions it
asked for were all answered with its recommendations on 2026-09-06 and are
binding: `.fork/decisions/2026-09-06-the-phone-surface-four-decisions.md`.

## Read these, in this order

1. **`CLAUDE.md`**, the working method. Long; navigate by heading. What
   matters most here: verify by running, the Windows build is a *different
   checkout* that you sync and build, and the launcher runs the **release**
   binary. Yesterday's item-5 measurement was taken against the wrong binary
   because a debug build was made and the launcher's own output saying so was
   written to a file and read afterwards (`.fork/runs/handoff-chip-2026-09-06/README.md`).
2. **`.fork/docs/remote-control.md`**, what is built, as of 2026-09-05. The
   pieces table at its end is the map of the code you will touch.
3. **`.fork/mobile.html`**, the spec. Open it in a browser. Seven gaps ranked,
   the "decided" boxes are the decisions, the sequence at the bottom is the
   order of work below.
4. **`.fork/runs/remote-control-2026-09-05/README.md`**, the two measured runs,
   with the drivers `rc.sh`, `rc2a.sh`, `rc2b.sh` you will copy the shape of.
5. `.fork/docs/manual.md`, five sections by heading: "Letting a phone watch",
   "The console", "Driving one conversation from a phone", "Reaching this
   machine from a phone", "Driving the Windows build from WSL".
6. `.fork/docs/observability.md`, for the event log and the trace the phone
   draws; `.fork/decisions/2026-09-05-remote-control-is-one-conversation-handed-over.md`
   for why the phone may prompt at all.

## The pieces, with where they are

| what | where |
|---|---|
| the two listeners, routes, credential issue, pairing routes | `app/src/local_control/mod.rs`: `bind_wide_listener` ~453, `axum::serve` ~399/410, routes ~365-395, `issue_credential` ~949, `handle_pair_request` ~1197, `handle_pair_credential_request` ~1249, `keep_from_children` (non-inheritable sockets, Windows) |
| the bind parser (`WARP_FORK_CONTROL_BIND`) | `app/src/fork.rs` `control_bind` ~736, `control_bind_from` ~745; `state_dir()` ~475 and `create_private_file` ~1359 for anything secret on disk |
| pairing map: `Scope`, code and device lifetimes, revoke | `app/src/local_control/pairing.rs`: `Scope` ~193, `CODE_LIFETIME` 236, `DEVICE_LIFETIME` 242, `MAX_PAIRED_DEVICES` 248, `PairedDevice` ~276, `issue_code` ~302, `revoke_conversation` ~324, `is_controlling` ~336, `redeem` ~354, `verify_device` ~390; tests in `pairing_tests.rs` |
| the grant confinement | `app/src/local_control/confine.rs` + `confine_tests.rs` (8 tests, pure) |
| the chip's calls into the map | `app/src/local_control/remote_control.rs`: `start` 30, `stop` 46, `is_active` 65 |
| the QR block in the pane | `app/src/terminal/view/remote_control_block.rs`: `render` ~131, `expires_at` ~70; `start_fork_remote_control` ~250 |
| the chip | `app/src/ai/blocklist/agent_view/agent_input_footer/mod.rs`, `sync_remote_control_button`, `fork_remote_control_active` |
| the console page and script | `app/src/local_control/console.{html,js,rs,webmanifest}`; in `console.js`: `loadDevice` 225, `saveDevice` 238, `showPairing` 260, `handleFrame` 709, `keepStreaming` 770, `openConversation` 798, `renderConversationHead` 865, `pollTrace` 914, `callInput` 996, `traceItem` 1115; tests `console_tests.rs` (pins `textContent`-only and the no-markup sinks: keep both green, and run `node --check app/src/local_control/console.js` after every edit) |
| the trace merge and rendering | `crates/warp_cli/src/local_control/trace.rs`, `trace_render.rs`; the in-instance handler `app/src/local_control/handlers/trace.rs` |
| the event log record | `app/src/event_log/mod.rs` (`linked_session_id` 119, `answered_by` 228), `warp_agent.rs` (`prompt_submit` derived from conversation status ~393), the ACP path in `app/src/ai/acp_agent/` |
| the prompt handler a phone reaches | `app/src/local_control/handlers/agent.rs` `agent_prompt` ~459; approvals in `handlers/approvals.rs` (`conversation_of_approval`) |
| the launcher | `C:\dev\warp\.fork\tools\warpdev.ps1` (`-Console`, `-Bind`, default `192.168.254.3:41234`); `ggwarpdev console` from bash |

Line numbers are as of `29308327a`; grep the names.

**One stale doc to fix on the way in:** `mod.rs` ~1245 says
`pairing::ensure_pairable` runs before `issue_credential`. That function is
now `ensure_pairable_under(action, &Scope)`; the sentence is otherwise right.

## The work, in order

Each step is measured the way the last runs were: the phone's side from
`curl.exe` (WSL `curl` cannot reach the Windows wide listener), the desk's
side from `shot.ps1 -Process warp-oss` and `click.ps1`, a run directory
under `.fork/runs/<name>-<date>/` that keeps the driver, the log, the JSON
and the screenshots, and a README that says what was found. Commit as
`fork: <lowercase subject> (T19)`; the body says what was found.

### 1. A real phone, once (measure, no code)

Nothing so far has been a phone; Brave on the Windows side stood in. Launch
`ggwarpdev console`, click `/remote-control` in a pane with a conversation,
scan the block with the phone on the LAN. Then: add to home screen, lock the
phone mid-turn, unlock, answer a request, send a prompt, Stop sharing from
the desk and confirm the phone is refused. Record what iOS or Android did at
each step, including the install: expect Chrome to offer a shortcut and not
an install, because plain HTTP does not meet its install criteria. Fix the
QR block's layout on a narrow pane while you are there (the text column
clips at the right in a 300-pixel pane; `rc-before.png` in the 09-05 run).
If the phone cannot scan the block at all, that is the first finding.

### 2. Trust on the wire: a fork-minted CA, TLS on the wide listener

The decision, and why it comes first: a LAN or tailnet address over plain
HTTP is not a secure context (MDN, "Secure contexts", read 2026-09-06:
`http://localhost`, `127.0.0.1`, `*.localhost` and `file://` are; nothing
else without `https://`), so browsers withhold the Notification API, service
workers, push and Chrome's install prompt. Step 4 cannot start without this.

Shape:

- Mint a CA key and certificate once, under `fork::state_dir()` through
  `create_private_file`, beside the discovery record. Sign a server
  certificate for the bind address (an IP SAN; the bind is one literal IP by
  the parser's rule) and re-sign when the bind changes. `rcgen` is **not** in
  `Cargo.lock`; `rustls` 0.23 is a workspace dependency already in `app`,
  `tokio-rustls` 0.26 and `rustls-pemfile` 2 are in the lock; `axum-server`
  is not. `axum` is 0.8.4, so serve TLS with a `tokio-rustls` acceptor around
  the `TcpListener` and `hyper-util`'s serve, or add `axum-server` with its
  rustls feature; check what the lock already resolves before adding.
- Wrap the **wide** listener only. The loopback listener stays plain:
  `warpctrl` is not a browser and localhost is already a secure context. Pin
  that with a test. `keep_from_children` must still run on the raw listener
  before the wrap.
- Serve the CA's public half at one more constant route on the wide
  listener (`/ca.crt`, `application/x-x509-ca-cert`), and have the pairing
  page (`showPairing` in `console.js`) show the install link and one sentence
  per platform. Android: install from the downloaded file under security
  settings, Chrome honours a user CA. iOS: install as a profile, then enable
  full trust under certificate trust settings. The URL in the QR is built at
  `pairing.rs` 466 (`format!("http://{origin}{path}#{}")`); the loopback
  origin in the discovery record (`crates/local_control/src/discovery.rs` 82)
  stays `http://`, and `mod.rs` ~1523 strips that prefix, so touch the wide
  origin only. The block's URL becomes `https://`. `console.js` fetches are same-origin and need no change; the
  CSP is unaffected.
- Drivers: `curl.exe --cacert <the CA>`; Brave on the Windows side needs the
  CA in the user's Root store (`certutil -user -addstore Root ca.crt`) or a
  scratch profile launched with `--ignore-certificate-errors` for a first
  look, but the measurement that counts is a phone with the CA installed and
  **no warning page**.
- Measure: the phone reports `window.isSecureContext === true` (put it on
  the pairing page), `Notification.requestPermission` is offered, Chrome
  offers a real install. Then re-run the 09-05 drivers over `https://` so
  nothing regressed.

Refused, do not build: a self-signed server certificate without a CA (a
warning page and a tap-through), a public certificate (a DNS name and an
account). headscale for reach is ops on a VPS, not code, and is not in this
handoff; the manual's "Reaching this machine from a phone" has the recipe.

### 3. A control pairing with no clock

`DEVICE_LIFETIME` (12 h) is applied in `redeem` and checked in
`verify_device` and `is_controlling`. A device whose scope is
`Scope::Control` should carry no expiry: it ends on `revoke_conversation`
(Stop sharing, which already purges its credentials through
`confine::purge_confined`), on the conversation being deleted (find where a
conversation is removed from the history model and call
`remote_control::stop` there; this hook does not exist yet), and on the
process ending (the map is in memory, nothing to do). The watch scope keeps
12 hours. Update `pairing_tests.rs`, including
`a_code_minted_for_one_conversation_buys_driving_it`, and add one test that
a control device is still valid after 13 hours and a watch device is not.
The block's "expires" line then describes the code, not the device; say so.

### 4. The desk knows what the phone did

Three visible states on the block (`remote_control_block.rs`): *waiting for a
scan*, *paired at HH:MM* once `redeem` spends the code, *expired, click
again* once `CODE_LIFETIME` passes unscanned (the chip already flips back
through `is_controlling`; the block keeps showing a dead code). The block is
rich content re-rendered from the model, so either the pairing map notifies
the view on redeem or the block reads the map on each render; the second is
smaller.

Provenance goes in the record, not the conversation. `answered_by` already
exists on `permission_replied` (`event_log/mod.rs` 228); fill it with
`paired_device` when the approval came through `handlers/approvals.rs` on a
confined grant. `prompt_submit` is derived from conversation status in
`warp_agent.rs`, so it has no request to read from; give the event log a
per-conversation "origin of the next prompt" set by `agent_prompt` when the
grant is confined and consumed by the writer. Then the trace stamps those
rows (`trace_render.rs` and `traceItem` in `console.js`) with a small "from
the phone". The prompt in the panel stays exactly the person's words.

### 5. Being told, foreground only

After step 2. In `console.js`, `handleFrame` already receives every event for
the viewed conversation over the stream. On `permission_request` and on
`stop` for the conversation the page is viewing, when `document.hidden`,
post a `Notification` (title: the conversation's first prompt, body: what is
asked) and `navigator.vibrate`. Ask for permission from a tap the person
already makes (the pair button), never on load. Nothing server-side. Web Push
is **not** built: record it in `remote-control.md` as an opt-in behind its
own variable, off, with the disclosure that it is a nudge through Google's or
Apple's relay, and build it only if a week says the foreground half is not
enough.

### 6. What the record shows

Page only. `renderConversationHead` draws mode, model and cwd from the
record's `session_mode`, `session_model` and `session_start` rows, since "is
the agent asking me or its classifier" is the first thing a phone wants to
know (a zero on permission requests means Warp was not in the loop, per
CLAUDE.md, never that nothing was decided). Then an edit tool's input (old
and new strings in `callInput`) drawn as a diff rather than two strings, in
both `trace_render.rs` and the page. Header first; it is an hour.

### 7. Later, not now

Several pairings on one phone (`loadDevice`/`saveDevice` hold one; make it a
map keyed by conversation with a list to pick from; the server already
allows eight devices). Push as the disclosed opt-in above.

## Rules that bite here

- Build: `CARGO_BUILD_JOBS=8 cargo build --release --features gui,warp_control_cli`.
  Never build on WSL and Windows at once. Windows: sync first
  (`git -C /mnt/c/dev/warp fetch /home/effatha/git/warp dev && git -C /mnt/c/dev/warp merge --ff-only FETCH_HEAD`),
  then `powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'C:\dev\build.ps1' -Release`,
  and **read the launcher's output** (`warpdev: binary …`, `predates the
  tree`) before believing a screenshot.
- Tests that pin this surface: `cargo test -p warp --lib --features gui,warp_control_cli -- local_control`
  (163 as of 09-05), `-p local_control` (42), `-p warp_cli` (399), and
  `-- fork::` (46). `PAIRABLE_ACTIONS` is pinned by
  `a_paired_device_gets_the_read_surface_and_the_safe_half_of_answering`
  (the whole list) and the consent asymmetry by
  `saying_yes_does_not_travel_by_default_and_saying_no_does`; the catalog
  count (115) by `catalog_has_exactly_115_retained_actions` and
  `capabilities_advertises_the_complete_catalog`. Adding a route is not
  adding an action; adding an action means both pins.
- `./script/format`, then `git checkout -- crates/remote_server/src/manager_tests.rs`
  (reformatted every time, never meant to be touched).
- Stop Warp with `warpctrl window close` (`"$EXE" --warpctrl window close`),
  never kill; end any CLI agent in a pane first. After a close, `instance
  list` must be empty before the next launch.
- A launch that inherits the driver's stdout never returns: detach it
  (`warpdev.ps1 … > launch.txt 2>&1 &`) and poll `instance list`.
- `type.ps1` doubles characters in Brave; `keys.ps1` and `click.ps1` do not.
- Read `agent read` beside a screenshot; it is silent about error blocks.
- Every doc you touch is dated *as of*; update `remote-control.md`'s
  "Unverified" list as each item is measured, and its pieces table as files
  change. Update `.fork/mobile.html`'s table when a row's colour changes.

## What done looks like

A phone with the CA installed opens `https://<bind>` with no warning, scans a
block, is buzzed when the agent asks, answers, keeps the conversation
overnight without re-pairing, and is cut off the moment Stop sharing is
clicked; the desk's block says when it was paired; the event log says which
prompts and answers came from the phone; every one of those sentences has a
run directory behind it.
