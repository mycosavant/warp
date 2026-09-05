> The open-questions list, split out of `.fork/TASKS.md` on 2026-09-05. History; read the section you came for.

## Open questions

- [x] ~~Log spam on window move (`workspace:save_app` per window event).~~
      **Measured, and the premise was wrong. Nothing to silence.** See "The
      log-spam question, answered by counting" below.
- [ ] Windows Developer Mode so `.claude/skills` resolves as a symlink on the
      Windows checkout.
- [x] ~~Proxy-based verification that nothing escapes under real activity — only
      idle runs observed so far.~~ **Done, two ways, with a control for each.**
      See "Nothing escapes: measured, not argued" below.

### Nothing escapes: measured, not argued

The fork's headline claim, and until now the least verified thing in it — every
previous observation was of an idle app. Done properly on Linux, under real
load, by two methods that fail in different ways.

**Method 1 — every socket the process opens.** `ss -tnp state all` plus
`ss -unp`, polled five times a second for the life of the run. Proxy-independent:
an app that ignored `HTTP_PROXY` could not evade it, and `state all` includes
`SYN-SENT`, so even a connection that is *attempted and refused* would appear.

    workload: every panel and modal toggled; theme and appearance;
              setting list/get; tab create; pane split; a shell command
              via input.submit; drive object create x3 (folder, workflow,
              notebook); drive status; slash list; a full local-agent turn;
              then ten minutes idle

    7918 poll samples, ~25 minutes of uptime

    every socket warp-oss held, for the entire run:
      LISTEN 127.0.0.1:9282    upstream's http_server
      LISTEN 127.0.0.1:33711   warpctrl

**Two loopback listeners. Zero outbound TCP. Zero UDP** — so not even a DNS
lookup: warp-oss never resolved a hostname, let alone contacted one.

*Both were labelled "local control" here until 2026-08-26, and only one is.*
9282 is upstream's `crates/http_server` (`PORT_BASE` 9277 + the Oss channel
offset), started ungated by fork policy and answering unauthenticated; the
ephemeral port is `warpctrl`, which binds 0 and publishes what it gets. The
egress finding is unaffected — the count and the direction are what it rests
on — but the attribution mattered enough to correct, because T12 puts a page
on one of these two and picking the wrong one would undo T10.2.

*The control that makes that negative mean something.* A poller that detects
nothing is worthless unless it can detect something, so the same poll was run
watching the `claude` child during a turn:

    ESTAB 172.22.45.116:48878 160.79.104.10:443  users:(("claude",pid=383426,…))
    ESTAB 172.22.45.116:48892 160.79.104.10:443  users:(("claude",pid=383426,…))
    ESTAB 172.22.45.116:48908 160.79.104.10:443  users:(("claude",pid=383426,…))

`160.79.104.10` is `api.anthropic.com`. The method catches real traffic; the
traffic belongs to the child process, on the user's own subscription, which is
exactly the design. **warp-oss appears nowhere in that list.**

**Method 2 — a decrypting proxy, because polling samples.** Method 1's gap is
real: a beacon that opens, POSTs and closes inside 200ms could fall between two
samples. So the whole run again behind `mitmdump` on loopback, with its CA
trusted so TLS succeeded and bodies were readable rather than merely counted.

    every server connection the proxy saw, whole session:
      example.com:443        <- the curl that proved the proxy captures at all
      api.anthropic.com:443  <- GET  /v1/mcp_servers?limit=1000
      api.anthropic.com:443  <- POST /v1/messages?beta=true

    grep -icE 'warp\.dev|firebase|googleapis|segment|rudder|datadog|sentry|
               amplitude|posthog|mixpanel|statsig|bugsnag|crashlytics'
      -> 0

Two requests in the entire session, both the agent turn, both the `claude`
child. The proxy log is 105 lines.

**Why two methods and not one.** Each covers the other's blind spot, and
neither is sufficient alone:

| | misses | covered by |
|:--|:--|:--|
| `ss` polling | a connection shorter than the poll interval | the proxy, which sees every request |
| proxy | anything that ignores `HTTP_PROXY` | `ss`, which sees the socket regardless |

`ss` says there were no sockets; the proxy says nothing was requested. Together
they close both doors. The `claude` child routing through the proxy also proves
the environment was honoured by the process tree, so "nothing in the proxy log
from warp-oss" is not simply "warp-oss ignored the proxy".

**Why there is nothing to send, from the log rather than the source.** The
startup line names the channel config:

    channel: Oss, … telemetry_config: None, autoupdate_config: None,
    crash_reporting_config: None

Structurally absent rather than flagged off. `server_root_url`, the RTC URL and
a Firebase key are still *present* in that config — they are simply never
contacted, which is what the two captures above establish and what reading the
config alone could not.

#### What was deliberately not done

**The contrast run — fork policy off, to watch telemetry appear — was not
done.** It is the most persuasive demonstration available and it would mean
transmitting this user's data to a third party to make a rhetorical point.
Not my call to make. The argument that fork policy is what silences this is
covered by unit tests instead.

**The claim is "no telemetry", not "nothing leaves".** The agent's prompt goes
to Anthropic, in the clear in that `POST /v1/messages` body, because that is
what an agent on your own subscription *is*. What does not happen is Warp
learning anything about you.

**Still unverified: T2.5, audio.** No proxy capture during a real recording;
that needs a microphone and someone to speak into it. Unchanged by this.

**Platform: measured on both now (Windows added 2026-09-05).** The run above was
Linux. The same measurement was repeated on the Windows release binary the
maintainer lives in — `v0.fork.f6f59cfe8` — with two socket pollers
(`Get-NetTCPConnection` on Windows, `ss` inside WSL) and the same decrypting
proxy. Across 685 seconds and 1716 samples, **every `warp-oss.exe` socket was
loopback or a bound listener, zero remote**; a `curl.exe` control proved the
poller catches a real outbound connection. The panel agent runs inside WSL, so
its `api.anthropic.com` traffic is the WSL poller's control, and `warp-oss`
appears in neither capture as an outbound socket. Full account and the three
findings it turned up — voice audio going to loopback only (T2.5, now measured),
the cloud WebSocket gated pre-socket by the missing-credentials check (T19), and
upstream's `9282` failing to bind on this run — are in
`.fork/runs/egress-windows-2026-09-05/`. `tcpdump` still was not used (root), so
packet-level capture is not part of either run; the socket-plus-proxy pair
covers the same ground.

### The log-spam question, answered by counting

`workspace:save_app` is **29 to 46 lines per run**, and the number barely moves
between a 904-line log and a 4197-line one. It is not the spam. Six rotated
logs:

    warp-oss.log         total 1115   dispatching   67   save_app 46
    warp-oss.log.old.0   total 1626   dispatching  301   save_app 43
    warp-oss.log.old.1   total 1029   dispatching   62   save_app 44
    warp-oss.log.old.2   total 4197   dispatching 3078   save_app 41
    warp-oss.log.old.3   total  904   dispatching   47   save_app 29
    warp-oss.log.old.4   total 1110   dispatching   60   save_app 34

The volume is `warpui_core::core::app` logging **every dispatched action at
`INFO`**, and in `old.2` that is 2975 lines of
`EditorAction::UserInsert(UserInput(" "))` — one per character, over two
minutes, at 25–40 a second. A held space bar, or WSLg key repeat. An
environmental burst rather than a Warp defect, and not what the question was
about.

**Leave it.** That trace is not noise, it is the only record of what a person
did in the window, and T5.6 was solved entirely from it — two hundred
`SelectText` actions and a `CtrlC`, which no other log line in the app would
have told us. Silencing the dispatcher to save forty lines a run would trade
the app's only forensic trail for nothing.

#### The thing worth having found

`UserInsert` carries the character that was typed, so the obvious next thought
is that the log is a plaintext keylog. **It is not, and upstream had already
thought about it**: `warp_util::user_input::UserInput<T>` has a hand-written
`Debug` that prints the value only under `cfg!(debug_assertions)`.

What it did not have was a test. One `cfg!` in a hand-written impl is the whole
of the guarantee — a careless `#[derive(Debug)]` would compile, pass every
other test in the tree, and start writing what people type into
`~/.local/state/warp-oss/warp-oss.log`. Now pinned in both profiles, and
verified the way T5.6's was: with the redaction removed, the release run fails.

    debug    shows "hunter2"    (deliberate; the doc comment promises it)
    release  redacts it
    release, redaction removed  -> FAILED

Worth knowing while working on this fork, since `.fork/docs/manual.md` tells you to
run `target/debug/warp-oss`: **your development build's log does contain what
you typed.** That is upstream's stated intent for a dev build, not a leak, but
it is a reason not to hand that file to anyone without reading it first.

---

