# The egress rig

The instruments that measured "nothing escapes" on Windows
(`.fork/runs/egress-windows-2026-09-05/`), kept as a recipe so the next run is a
sequence of commands rather than a reconstruction. First built and driven
2026-09-05; the reasoning behind each choice is in that run's README.

The claim under test is direction and destination: **no `warp-oss` socket leaves
the machine.** The agent's own API traffic is expected and is not Warp's; on the
Windows build the agent runs *inside WSL*, so it is on the other side of the
boundary from Warp and is the control that proves the pollers see real traffic.

## The three instruments, and why each

Two pollers and a proxy, each blind where another sees. Neither poller alone is
enough and neither is the proxy.

| instrument | file | catches | blind to |
|---|---|---|---|
| Windows socket poll | `egress-poll.ps1` | every socket `warp-oss.exe` holds, `SynSent` included | a beacon shorter than the 200 ms interval; anything owned by a WSL process |
| WSL socket poll | `egress-poll-wsl.sh` | every socket Warp's WSL-side processes hold (daemon, terminal-server, the ACP agent) | the same sub-interval gap; Windows-side sockets |
| decrypting proxy | `mitmdump` (system) | every request regardless of duration, body readable | anything that ignores `HTTP_PROXY` — which the pollers catch anyway |
| loopback endpoint stub | `whisper-stub.py` | the body of a POST the app makes to a local endpoint (voice) | nothing it is not pointed at |

`Get-NetTCPConnection` lists `SynSent`, so a connection *attempted and refused*
still appears; `ss -tnp state all` does the same on Linux. That is what makes a
poll that finds nothing mean something.

## Run it

All paths are the live ones from the 2026-09-05 run. The Windows copies of the
two pollers live at `C:\dev\egress-poll.ps1`; keep them in step with the tracked
originals here (`cp .fork/tools/egress-poll.ps1 /mnt/c/dev/`).

### 1. Start the proxy (WSL side, loopback)

```bash
mitmdump --listen-host 127.0.0.1 --listen-port 8899 \
         --set confdir=<scratch>/mitm --flow-detail 2 -w <scratch>/flows.mitm \
         > <scratch>/mitm.log 2>&1 &
```

Prove it decrypts before trusting a later silence — one control request through
it that must appear:

```bash
curl.exe -x http://127.0.0.1:8899 -k -s -o NUL -w "%{http_code}\n" https://example.com
```

Under mirrored WSL networking (`.wslconfig networkingMode=mirrored`) a Windows
process reaches this loopback listener directly.

### 2. Start both pollers

```powershell
# Windows, from PowerShell or via powershell.exe from WSL:
powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\dev\egress-poll.ps1 `
  -Out C:\dev\egress\sockets.tsv -StopFile C:\dev\egress\stop
```

```bash
# WSL:
.fork/tools/egress-poll-wsl.sh <run>/wsl-sockets.tsv <run>/stop-wsl 0.2
```

Optionally clear the Windows DNS cache first (`Clear-DnsClientCache`) so
`Get-DnsClientCache` at the end is a weak third witness for name lookups, which
Windows does in a `svchost` and not against Warp's pid.

### 3. Launch Warp through the product profile, with the proxy in its env

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -Command `
  "$env:HTTPS_PROXY='http://127.0.0.1:8899'; $env:HTTP_PROXY='http://127.0.0.1:8899'; \
   $env:NO_PROXY='127.0.0.1,localhost'; \
   & 'C:\dev\warp\.fork\tools\warpdev.ps1' -Exe 'C:\dev\warp\target\release\warp-oss.exe'"
```

The proxy env is set inside the launcher process only, so it reaches the Warp it
starts and nothing else. The product profile names the ACP agent and starts it
inside WSL — the reason the agent's traffic lands on the WSL poller.

### 4. Exercise it

Drive a real workload through `warpctrl` (`… --warpctrl <verb>`): surfaces,
tabs, panes, a shell command, Drive creates, and — the load that matters — a
couple of agent turns. `agent prompt` needs an active terminal pane, so close a
Settings tab if it grabbed focus, and `input submit 'cd <repo>'` before the
first prompt.

For voice, click the mic button in the agent footer with a posted click (the
window need not be foreground):

```bash
use_computer.exe --pid <hwnd-owner-pid> --window-id <hwnd> click <x> <y>   # start
# ...a few seconds...
use_computer.exe --pid <hwnd-owner-pid> --window-id <hwnd> click <x> <y>   # stop -> transcribe
```

Run it once with **nothing** on `127.0.0.1:8080` to see the fail-closed error
naming only the loopback URL, then start the stub and click again to capture the
POST body:

```bash
.fork/tools/whisper-stub.py <run>/whisper-stub.log 8080 &
```

### 5. Stop and read

```bash
echo stop > /mnt/c/dev/egress/stop      # Windows poller footer: samples, elapsed, distinct tuples
touch <run>/stop-wsl                     # WSL poller footer
```

Then:

- **Windows:** every `warp-oss.exe` tuple should be loopback (`127.0.0.1`) or a
  bound listener (`0.0.0.0:*`). One line finds any leak:
  `grep warp-oss sockets.tsv | awk -F'\t' '{print $6}' | grep -vE '127\.0\.0\.1|0\.0\.0\.0:0'` —
  empty is the pass. The `curl.exe` control must be present and remote.
- **WSL:** the only non-loopback remotes should be the agent's
  (`api.anthropic.com`, `160.79.104.10:443`, owned by `claude`). Warp's WSL
  processes should show none.
- **Proxy:** one server connection, the `curl` control. `grep -icE
  'warp\.dev|firebase|googleapis|segment|rudder|datadog|sentry|amplitude|posthog|mixpanel|statsig|bugsnag'`
  over the log is 0.
- **Voice:** `whisper-stub.log` shows one `POST /inference … body_bytes=<n>` to
  loopback and nothing else.

## Teardown

Trash any probe Drive objects (`drive object trash <id>`), `warpctrl window
close`, stop the pollers, kill the proxy and the stub, and check for orphaned
WSL `remote-server`/`terminal-server` daemons — the auto-connect leaves them and
they outlive the Warp that spawned them.

## Traps this run hit, so the next does not

- **Poll both sides.** A Windows-only poll finds nothing during an agent turn
  and reads as "the agent made no calls", because on this build the agent is a
  WSL process. That is the measuring-the-wrong-side error; the WSL poll is where
  the agent's control traffic is.
- **The WSL poller must keep whole `ss` lines.** The first pass split on
  whitespace and dropped the process column, filtering the agent out before
  writing — a silent capture bug that looked like a clean result. Calibrate
  against the known-present agent traffic before trusting a clean WSL capture.
- **Voice needs the endpoint to answer.** A dead port proves fail-closed but
  captures no body; the stub is what turns "it went to loopback" into "it went
  to loopback and here is what it sent".
- **`9282` may already be held.** Upstream's HTTP server fails to bind if
  another `warp-oss` on the machine holds it, so the listener count can be one,
  not two. Not an egress finding; note it and move on.
