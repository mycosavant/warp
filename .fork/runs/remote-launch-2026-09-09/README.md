# Launching Warp from a remote shell, 2026-09-09

The maintainer's question, asked from the phone while at work: if Warp dies
while I am out, can I start it again from here, or does that need the desk?

**Answered: it can be started from a remote shell, and the Windows session
being locked does not stop it.** Measured both ways within four minutes.
`rc.sh` is the driver, `driver.log` its log.

## What was run

Two phases, the second being the case that actually matters.

| | phase A, unlocked | phase B, **locked** |
|---|---|---|
| old instance closed by `window close` | attempt 1 | attempt 1 |
| listener released with it | yes | yes |
| instance record after remote launch | **3 s** | **3 s** |
| wide listener rebound on `100.82.213.46:41234` | yes | yes |
| TLS to the console | `Tls13`, leaf `CN=Warp console 100.82.213.46` | same |
| `failed to render` / panic in the log | **0** | **0** |

Phase B locked the workstation with `rundll32.exe user32.dll,LockWorkStation`
and confirmed `LogonUI` was present before launching, so "locked" is measured
rather than assumed.

The thing that was expected to fail did not. Warp on a locked session still
enumerated adapters and took the discrete GPU:

```
Using Dx12 DiscreteGpu (NVIDIA GeForce RTX 5070) for rendering new window.
```

No `Failed to render a frame 3 times in a row`, which is the deliberate exit
`CLAUDE.md` warns about and the reason this was worth measuring rather than
reasoning about.

## What "remote" means here, and what it does not

"Remote" is simulated with `env -i`: a stripped environment, launching a
Windows binary by absolute path. That shape was taken from the maintainer's
own live phone sessions, read the same morning at `pts/6` and `pts/7`:

- **`WSL_INTEROP` is unset in an sshd session**, and interop works anyway --
  `/run/WSL/1_interop` is a symlink the binfmt handler falls back to. Verified
  by running `powershell.exe` from `env -i` and getting its output back.
- **An sshd session's `PATH` carries no Windows entries at all.** So from
  Termux, `powershell.exe` is not on the path and must be named in full:
  `/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe`.

**The residual, stated rather than smoothed over:** `env -i` is a stand-in for
an sshd session, not one. A real session differs in ways this does not model
(its own pty, cgroup and session keyring). The stand-in is narrower than the
thing, which is the failure shape this fork already records twice, so it is
named here rather than credited. One command typed from Termux closes it:

```
/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe -NoProfile \
  -ExecutionPolicy Bypass -File \
  '\\wsl.localhost\Ubuntu\home\effatha\git\warp\.fork\tools\warpdev.ps1' -Console
```

## The phone needs nothing after a relaunch

The leaf is minted per launch, so each of the three instances presented a
different certificate. All three were signed by the same authority:

```
leaf issuer : O=warp fork, CN=Warp fork console CA ed1cbed4
```

and the authority served at `/ca.crt` is byte-identical to the copy saved in
`.fork/runs/reach-2026-09-08/ca.crt`, which is what the phones installed. So a
relaunch costs the phone nothing: same address, same trust, no reinstall.

Pairing works from the remote shell too. `pair show` against the
locked-session instance returned a code on the tailnet origin:

```
url     : https://100.82.213.46:41234/#<code>
ca_url  : http://100.82.213.46:41234/ca.crt
actions : 7 pairable
```

Seven, matching `PAIRABLE_ACTIONS`. So the loop closes: Warp dies, a phone
SSHes in, relaunches it, mints its own pairing code and scans or taps the URL,
with nobody at the machine.

## What `window close` did, and a fix holding

Both closes released the wide listener immediately -- `listener none` in the
snapshot straight after. That is the 2026-09-05 `keep_from_children` fix
working: before it, orphaned `wsl.exe` children held an inheritable socket and
the next launch on 41234 failed with *only one usage of each socket address*.
Three launches on the same port in four minutes, no bind failure.

## Found on the way: the live launcher was five days stale

`C:\dev\warpdev.ps1` was the pre-2026-09-04 two-state version -- `-On`/`-Off`,
a `~/.warpdev` state file, no `-Console`, no `-Bind`, no tailnet resolution. It
could not have opened the console at all.

This did not affect the 2026-09-08 reach run, and the reason is worth keeping:
that run invoked the **tracked** copy over its UNC path
(`\\wsl.localhost\Ubuntu\...\.fork\tools\warpdev.ps1`), which is always
current because it is the file in the repository. Anyone typing `warpdev.ps1`
at the desk got the stale one, with no error -- the same two-checkouts-read-as-
one-tree hazard `CLAUDE.md` records for `build.ps1`, and the same remedy it
states: change one, copy to the other.

Copied from the tracked file; the old one is at
`C:\dev\warpdev.ps1.bak-2026-09-09`.

**Prefer the UNC path in any driver anyway.** It cannot go stale, and this run
used it for exactly that reason.

## Files

| file | what |
|---|---|
| `rc.sh` | the driver, `a` and `b` phases |
| `driver.log` | its log, timestamps UTC |
| `launch-a.txt`, `launch-b.txt` | the launcher's own output for each phase |
