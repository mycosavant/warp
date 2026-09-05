# Nothing escapes, measured on Windows — 2026-09-05

Board item 3. The egress claim was measured twice on Linux
(`.fork/tickets/open-questions.md`, "Nothing escapes: measured, not argued") and
the same section said in as many words that Windows was unmeasured. Windows is
where the product runs. This is that run, against the merged release binary the
maintainer lives in: `v0.fork.f6f59cfe8`, launched through the product profile
of `warpdev.ps1`.

The build under test is the release exe at `C:\dev\warp\target\release\warp-oss.exe`,
its checkout at `f6f59cfe8` (the version sidecar says so), which contains the
dev tree's work through `e5bff3436`.

## Method, three instruments

Two socket pollers and one decrypting proxy, each blind where another sees.

1. **Windows sockets** — `.fork/tools/egress-poll.ps1`, `Get-NetTCPConnection`
   plus `Get-NetUDPEndpoint` every 200 ms, keeping every distinct
   `(pid, proto, local, remote, state)` tuple. `Get-NetTCPConnection` lists
   every state including `SynSent`, so a connection *attempted and refused*
   still appears. Watched by process name: every `warp-oss.exe` (the window and
   its crash-recovery sibling), anything descended from one, and `curl.exe` as
   the control.
2. **WSL sockets** — `.fork/tools/egress-poll-wsl.sh`, `ss -tnp state all` plus
   `ss -unp` every 200 ms inside the distribution, filtered to the processes
   Warp starts there: the remote-development daemon and its terminal-server and
   proxy (`WARP_FORK_WSL_AUTO_CONNECT`), and the ACP agent (`claude-agent-acp`
   under node). This side matters because the panel's agent runs *inside* WSL,
   not in the Windows Warp.
3. **A decrypting proxy** — `mitmdump` on `127.0.0.1:8899`, with
   `HTTPS_PROXY`/`HTTP_PROXY` set in the launcher process so Warp's reqwest
   client tunnels through it. Polling samples; a beacon that opens, POSTs and
   closes inside 200 ms could fall between two `Get-NetTCPConnection` calls. The
   proxy sees every request regardless of duration.

Ran 685 seconds, 1716 Windows samples, 1112 WSL samples.

## Workload

Launch; every surface toggled (Drive, resource center, AI assistant, both side
panels, vertical tabs, project explorer, conversation list, code review, each
twice); settings opened and the command palette; a theme list and a settings
list; a tab created and a pane split twice; a shell command through
`input submit`; three Warp Drive object creates (folder, notebook; a workflow
needs a JSON body and was skipped) and a drive status; two full agent turns in
the panel; and a voice recording (below). Then the probe Drive objects were
trashed and the window closed.

## Result: every warp-oss socket is loopback

Nine distinct `warp-oss.exe` tuples across the whole run, **every one loopback
or a bound listener**, zero remote:

```
warp-oss.exe  tcp  127.0.0.1:56710  Listen                 <- warpctrl control server
warp-oss.exe  tcp  127.0.0.1:56710 <-> 127.0.0.1:56711     <- internal loopback pair
warp-oss.exe  tcp  127.0.0.1:56710 <-> 127.0.0.1:63641     <- internal loopback pair
warp-oss.exe  tcp  127.0.0.1:63685  ->  127.0.0.1:8080     <- voice, to the local transcriber
warp-oss.exe  tcp  0.0.0.0:*        Bound                  <- bound, never connected
```

`windows-sockets.tsv` is the whole capture. The classifier
`remote not matching 127.0.0.1 or 0.0.0.0:0` returns **nothing** for
`warp-oss.exe`.

**The control that makes that negative mean something.** `curl.exe` on Windows,
no proxy, rate-limited to `example.com:443`, appears in the same capture as
`10.2.0.2:63653 -> 172.66.147.243:443 Established`. The poller catches a real
outbound connection; `warp-oss` is not one.

**The proxy saw zero warp-oss requests.** Its log (`proxy-8899.log`) holds one
server connection, `example.com:443`, which is the `curl` that proves the proxy
decrypts at all. A grep of the log for every blocked vendor plus `warp.dev` and
`analytics` returns 0. Warp made no request for the proxy to carry, because it
opened no outbound socket for one to ride.

**The agent's own traffic is on the WSL side, and it is expected.** The panel
agent is `claude-agent-acp` spawned inside Ubuntu, so its API calls are the WSL
poller's job. During the turns the agent (`claude`, pid 3638256) held
connections to `160.79.104.10:443` — `api.anthropic.com` — the user's own
subscription, which is what an agent on your own key *is*. `warp-oss` appears
nowhere in the WSL capture either. This is the same split the Linux run found:
the model traffic belongs to the child process, not to Warp.

## Voice: the audio goes to loopback and nowhere else (T2.5)

T2.5 stood as *"argued and unit-tested, not proxy-verified"* because there was
no way to *start a recording* from outside the GUI. There is now: the mic button
in the agent footer accepts a posted `use_computer` click, so a recording can be
driven without a keystroke.

Driven twice.

- **First, against a dead endpoint.** With nothing behind `127.0.0.1:8080`, the
  socket poller caught `warp-oss` connecting to `127.0.0.1:8080` at the moment
  the recording stopped, and the app log reported
  *"Failed to transcribe voice input: Could not reach the local transcription
  server at http://127.0.0.1:8080/inference. Is it running?"* — fail-closed at
  the app level: it tried the configured loopback endpoint, gave up, and **did
  not fall back to `api.warp.dev`**.
- **Then, against a logging stub** (`whisper_stub.py`, returning whisper.cpp's
  `{text}` shape). The captured request:

  ```
  POST /inference  host=127.0.0.1:8080
  content-type: multipart/form-data
  body_bytes: 151069
  ```

  151 KB of audio, multipart, to `127.0.0.1:8080` and nothing else. The
  transcription `egress probe transcription` rendered in the composer
  (`egress-voice-transcribed.png`), so the end-to-end path completed through
  loopback.

So T2.5 is now measured, not argued: the audio's only destination is the
configured local endpoint. What is still not proved is the *words* of a real
recording, because the mic click captured whatever WSLg's `RDPSource` held, not
speech — but the egress question is about destinations, and silence answers it
as well as speech would.

## The WebSocket the deny-list cannot see, and why the gate holds today (T19)

T19 found that `crates/websocket` opens sockets through `async-tungstenite`
directly and takes no `http_client` dependency, so neither egress enforcement
point covers it. The board carried it under this item: *"either add the third
enforcement point or write down why the gate is enough."*

This run caught the concern live. `CloudObjects::Listener` — the Warp Drive
cloud-sync GraphQL subscription over WebSocket — retried every ~30 s throughout
the session, each attempt logging
*"websocket connection failed to connect or finished with an error; trying
again: missing authentication credentials"*.

**It never reached a socket, and that is measurable.** The 1716-sample poller,
`SynSent` included, recorded zero non-loopback `warp-oss` sockets. The reason is
in the source: `get_warp_drive_updates` builds its request through
`get_or_refresh_access_token` (`crates/warp_server_client/src/auth/session.rs:101`),
which bails with exactly that string **before any dial** when
`auth_state.credentials()` is `None`. In this accountless fork it is always
`None`, so the token fetch fails locally, the request is never built, and no
socket opens.

So the login gate is a real pre-socket enforcement point *today*: no
credentials, no request, no WebSocket. The egress deny-list's blind spot to
WebSocket traffic is unreachable without an account.

**The residual, stated so nobody credits the gate with more than it does:** this
is enforcement by absence-of-credential, not a deny-list. If the fork ever holds
a real Warp token, the WebSocket path would bypass egress with nothing to stop
it. The durable fix T19 describes — a check at socket construction in
`crates/websocket` plus a test pinning the consumer list — is still the right
one. The gate is sufficient only as long as the fork stays accountless, which is
its thesis, so this is written down as "why the gate is enough for now" rather
than built.

## One honest discrepancy from the Linux run

The Linux measurement found **two** loopback listeners: upstream's HTTP server
on `9282` and the ephemeral `warpctrl` port. This run found **one** — the
`warpctrl` server on `56710`. Upstream's server failed to bind:

```
[ERROR] Failed to bind local HTTP server on 127.0.0.1:9282:
        Only one usage of each socket address ... is normally permitted.
```

`9282` was already held by another process on the machine (a stale sibling, or
the maintainer's own Warp), so the main window ran without it. Not a fork change
and not an egress finding — the direction and destination of every socket is
what the claim rests on, and those are unchanged. Recorded because a reader
comparing the two runs would otherwise see a missing listener and wonder.

## Files

- `windows-sockets.tsv` — every Windows socket tuple, first-seen.
- `wsl-remote-endpoints.txt` — the WSL side's distinct non-loopback remotes:
  every one is `160.79.104.10:443` (`api.anthropic.com`), owned by the agent
  (`claude`), the control that proves the poller sees real traffic.
  `wsl-sockets.tsv.raw.gz` is the full sample stream behind it. (A first poller
  pass whose awk dropped the process column, filtering the agent out before
  writing, was discarded after being caught — the calibration failure this
  file's method warns about.)
- `proxy-8899.log` — the decrypting proxy; one line of `example.com`, the
  control.
- `whisper-stub.log` — the captured voice POST.
- `timeline.txt` — every step with a UTC stamp.
- `egress-turn.png`, `egress-voice-1.png` (Listening…),
  `egress-voice-transcribed.png` — the panel through the run.
- `agent-prompt.json`, `agent-read.json` — the first turn's request and answer.
