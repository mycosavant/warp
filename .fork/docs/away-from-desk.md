# Working away from the desk

*As of 2026-09-09. Every command here has been run; the ones that have not are
marked **untested**.*

A command reference for driving this machine from a phone. If you are looking
for *why* any of it is shaped this way, `.fork/reach.html` has the network
argument and `.fork/docs/manual.md` has the surfaces.

---

## The one thing to get straight: where a command runs

Three places, and mixing them up is the single biggest time sink.

| # | where | how you get there | what it can do |
|---|---|---|---|
| 1 | **Termux**, on the phone | you are typing in it | ssh, mosh, curl |
| 2 | **WSL shell** (Ubuntu) | `ssh` / `mosh` from Termux | the repo, git, cargo, everything Linux |
| 3 | **Windows** | `powershell.exe` *from* the WSL shell | the Warp GUI, `warpctrl`, builds, screenshots |

**The trap that makes step 3 look broken:** an SSH session's `PATH` has no
Windows entries, so `powershell.exe` is *not found* even though interop works
perfectly. Name it in full, every time:

```bash
/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe
```

Put these in `~/.bashrc` on the WSL side once and stop typing them:

```bash
alias ps1='/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe -NoProfile -ExecutionPolicy Bypass'
alias warpctrl='/mnt/c/dev/warp/target/release/warp-oss.exe --warpctrl'
```

Everything below assumes those two aliases.

---

## Connect

```bash
# from Termux. `warp` is the LAN alias and fails on cellular; `warp-ts` is the tailnet one.
ssh warp-ts -t tmux new -As mobile

# same thing over mosh, once the UDP rule exists (see Desk tasks)
mosh --ssh="ssh -p 22" effatha@100.82.213.46 -- tmux new -As mobile
```

## Setting it up, the first time

Set up in the order that does not lock you out — moved here 2026-09-12 from
`CLAUDE.md`, which had it as an account rather than a recipe:

1. Key on the phone (`ssh-keygen -t ed25519` in Termux), its **public** half
   appended to `~/.ssh/authorized_keys` — `700` on `~/.ssh`, `600` on the file.
2. **Open the port before disabling passwords**, not after. The password
   fallback is the safety net that lets you debug a failing key path; removing
   it first means the only way to test is also the way that can strand you.
3. `New-NetFirewallRule … -LocalPort 22 -RemoteAddress 192.168.254.0/24` — scope
   it to the subnet rather than `Any`.
4. Verify the host key fingerprint **out of band** (`ssh-keygen -lf
   /etc/ssh/ssh_host_ed25519_key.pub`) instead of accepting the TOFU prompt
   blind.
5. Only then `/etc/ssh/sshd_config.d/10-keys-only.conf` with
   `PasswordAuthentication no`, keeping one session open while you reload.

**Four traps, and three of them produced a confident wrong diagnosis first:**

- **A firewall rule is per *port*.** The console's rule did nothing for 22,
  and a *dropped* packet gives no refusal — just a silent hang that reads
  exactly like a broken key or a hung shell. Check `Get-NetFirewallRule …
  LocalPort -eq <port>` before debugging anything above the network.
- **`~/.ssh/authorized_keys` was a *directory*** containing a copied pubkey, so
  sshd had never read it and key auth had never worked. `[ -f authorized_keys ]`
  reports "missing" for a directory, which reads as "not set up" rather than
  "set up wrongly". Look at the target before writing to it — `chmod 600` on a
  directory strips its traversal bit.
- **`BatchMode=yes` blocks the passphrase prompt, not just the password one.**
  Chosen to prove a key rather than a password, it produced a false negative on
  a passphrase-protected key: `debug1: Server accepts key` followed by failure
  means `authorized_keys` is *correct* and the client could not sign.
- **`-tt` is not needed for an interactive login** — ssh allocates a TTY when
  stdin is one. `-t` is for a *remote command* that needs a terminal, which is
  how the TUI would be run over SSH.

`tmux new -As mobile` = attach to `mobile`, create it if absent. Same line every
time, however you got dropped. **Run any agent inside that tmux**, never in the
bare SSH shell, or it dies with the link.

Dropped? Re-run the identical line.

---

## Is anything actually running?

Ask this before diagnosing the network. `no_instance` is the whole diagnosis.

```bash
warpctrl instance list                 # is Warp alive, and which pid
tmux ls                                # what sessions are waiting for you
ps1 -Command "if (Get-Process LogonUI -EA SilentlyContinue) {'LOCKED'} else {'UNLOCKED'}"
```

---

## Warp is dead. Restart it from the phone

Measured 2026-09-09, works with the desk **locked**
(`.fork/runs/remote-launch-2026-09-09/`).

```bash
ps1 -File '\\wsl.localhost\Ubuntu\home\effatha\git\warp\.fork\tools\warpdev.ps1' -Console
```

Give it ~3 seconds, then confirm and pair:

```bash
warpctrl instance list                 # expect one instance_id
warpctrl pair show                     # prints a QR and a url with the code in its fragment
```

Tap the `url` from the phone, or scan the QR. The code lives **2 minutes**.

**Use the UNC path above, not `C:\dev\warpdev.ps1`.** The UNC path is the file
in the repository and cannot go stale; the `C:\dev` copy was found five days out
of date on 2026-09-09 and could not open the console at all.

Profiles: bare is the product, `-Instrumented` is the measurement rig,
`-Stock` is upstream. `-Console` is what makes it phone-reachable and defaults
to `-Bind tailnet`.

### Stop it

```bash
warpctrl window close                  # ALWAYS this, never kill
warpctrl instance list                 # verify: `ok: true` only means "asked"
```

A close can be refused. If it stays up: end any CLI agent running in a pane,
`warpctrl agent cancel <id>` any in-flight turn, then close again.

---

## Drive Warp from the shell

All 115 actions are here, against 7 through a paired console.

```bash
warpctrl pane list
warpctrl session inspect               # "where": host = routed into WSL; local = not
warpctrl agent list                    # quiet_for_seconds tells you if a turn is wedged
warpctrl agent approvals               # what is waiting on a yes
warpctrl agent cancel '<conversation>'
warpctrl input submit 'cd /home/effatha/git/warp'
warpctrl agent prompt '<conversation>' 'your prompt'
```

**Use `--output-format json` for anything a script reads.** The pretty format
has cost a false security investigation.

---

## Work on the fork

Everything in this section is plain WSL. No GUI needed.

```bash
cd /home/effatha/git/warp
cargo check --workspace --all-targets           # the real gate for shared types
cargo test -p local_control                     # fast
cargo test -p warp --lib                        # slow, and the one people skip
./script/format
node --check app/src/local_control/console.js   # after ANY console.js edit
```

Builds. **Never run both sides at once** — that is what took the VM down.

```bash
.fork/tools/build.sh --release                  # WSL build, caps jobs at 8
ps1 -File 'C:\dev\build.ps1' -Release           # Windows build (the GUI you drive)
```

Start a build, detach from tmux, come back. That is the pattern this setup is
actually for.

The Windows build is a **separate checkout** that nothing syncs. Before trusting
a Windows run:

```bash
git -C /mnt/c/dev/warp log --oneline -1         # compare to your own HEAD
# sync, from WSL (use the Linux path; the `origin` UNC path only resolves from Windows git):
git -C /mnt/c/dev/warp fetch /home/effatha/git/warp dev
git -C /mnt/c/dev/warp merge --ff-only FETCH_HEAD
```

---

## What works with the desk locked

Measured 2026-09-09 with `LogonUI` confirmed present.

| | locked? | note |
|---|---|---|
| SSH / mosh / tmux | **yes** | unaffected |
| `warpctrl` reads (`instance`, `pane`, `session`, `agent`) | **yes** | |
| `warpctrl` writes (`input submit`, `agent prompt`) | **yes** | verified by screenshot |
| Launching Warp | **yes** | takes the discrete GPU, renders, 3 s to a record |
| The console from the phone | **yes** | TLS, pairing, the lot |
| Screenshots (`shot.ps1 -Process warp-oss`) | **yes** | full content, not a black frame |
| Builds, tests, git | **yes** | never needed a desktop |
| Synthetic clicks/drags (`click.ps1`, `drag.ps1`) | **untested** | the input desktop reads `Default`, but no click was fired |

**So you can leave the machine locked.** Nothing in the mobile workflow needs it
unlocked. The only open question is GUI gestures, which are desk work anyway.

---

## Symptom → cause → command

| symptom | first suspect | command |
|---|---|---|
| phone reaches nothing at all | Tailscale VPN off on the phone | look for a `tun` device. **A `100.x` address is not proof** — the carrier hands out `100.65.x` from the same CGNAT range |
| console page will not load | Warp not running | `warpctrl instance list` |
| console says "not secure", crossed padlock | you are on `http://`, or a stored cert exception | check the scheme; `https://100.82.213.46:41234/` |
| full-page cert interstitial | that browser lacks the authority | install `http://100.82.213.46:41234/ca.crt` |
| console loads but will not pair | code expired (2 min) | `warpctrl pair show` again |
| `ambiguous_instance` on every call | stale/extra live instances | `warpctrl instance list`, close the extras |
| Warp will not close | CLI agent in a pane, or a wedged turn | end the agent; `warpctrl agent cancel`; close again |
| port 41234 refused on launch | listener held by a dead pid | `netstat -ano \| findstr 41234` |
| Windows build "succeeded" but changed nothing | the checkout is behind | `git -C /mnt/c/dev/warp log --oneline -1` |
| `powershell.exe: command not found` | SSH session `PATH` has no Windows entries | use the full path or the `ps1` alias |

---

## Facts worth memorising

- **The QR code is single-use and lives 2 minutes.** The device token it buys
  does not expire for a control pairing, and lives in `localStorage`. So closing
  the tab costs nothing; reopen the URL and you are still paired.
- **Warp restarting kills every pairing.** The map is in memory. That is the
  only thing that forces a fresh QR — and you can mint one from the phone.
- **The certificate is re-minted every launch, the authority is not.** A
  relaunch never costs the phone a reinstall.
- **`http://host:port` and `https://host:port` are different origins**, so the
  token does not carry between them.
- **Android's DNS:** Private DNS must be Off, and Tailscale's "Use Tailscale
  DNS" Off, or the phone treats the network as down.
- **Firefox on Android keeps its own trust store** and does not read Android's
  user CA store. Chromium browsers do.

---

## Desk tasks still outstanding

```powershell
# elevated PowerShell, at the machine. Enables mosh.
New-NetFirewallRule -DisplayName 'Mosh from tailnet' -Direction Inbound -Action Allow `
  -Protocol UDP -LocalPort 60000-61000 -RemoteAddress 100.64.0.0/10 -Profile Any
```

Everything else in this document works without it.
