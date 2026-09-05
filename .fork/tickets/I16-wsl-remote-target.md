> Idea I16, split out of `.fork/IDEAS.md` on 2026-09-05. History; read the section you came for.

# I16 — WSL as a remote target, the way Zed does it

> *"This level of WSL integration would be almost game-changing and gets me
> closer to how I want things to feel and work, and I just haven't found it
> elsewhere."* — 2026-08-22

**Selected. This is the largest thing on the board that is mostly already
built, and unlike every other entry here the account question has been settled
by running it rather than reading it.**

## What Zed actually does

Zed for Windows treats WSL as a **remote target**, not a place to install the
editor — you are explicitly told not to install Zed inside the distro, where it
fails for want of GPU packages. Instead the Windows client spawns a headless
`remote_server` process **under `wsl.exe`** and proxies all I/O to it. Remote
entities like `Worktree` implement the same interfaces as their local
counterparts and delegate over RPC.

| stays on Windows | runs in WSL |
|---|---|
| UI, GPU rendering, themes, keymaps | source files |
| Tree-sitter parsing and highlighting | language servers |
| language-model integration | terminals, tasks, debuggers, git |
| unsaved buffers, recent projects, extensions | |

That table is the whole reason it feels native: **nothing that shapes the feel
of the app ever left Windows.** And file I/O never crosses the 9p boundary,
which is why ext4 is fast — this fork measured the same thing from the other
side, where the Windows build pointed at a WSL checkout left the file tree
loading, because 9p is a cost paid *per file* across 209,644 of them
(`README.md`, "Why you might actually want this build").

It is not a container. WSL2 is a lightweight VM with a real Linux kernel; Zed
adds no containerisation and runs a native Linux binary in your existing
distro. (Zed has a separate Docker transport — that one *is* containers.)

## Warp has the same architecture already

`crates/remote_server/src/transport.rs:209` defines `RemoteTransport`: a
seven-method, object-safe trait, documented as boxed "so implementations can be
stored as `Arc<dyn RemoteTransport>`". `detect_platform` works *"by running
`uname -sm`"* — the identical probe Zed's WSL connection uses. Two teams, same
seam.

**There is exactly one implementation: `SshTransport`**
(`app/src/remote_server/ssh_transport.rs:118`). Fifth instance of this fork's
recurring finding.

The tiers, which matter for how small this is:

    client  ──(ssh/wsl stdio pipe)──►  remote-server-proxy  ──(unix socket)──►  remote-server-daemon

* **`SSH is just a pipe.`** `connect` spawns `Command::new("ssh")` with piped
  stdin/stdout/stderr and hands the three pipes to
  `RemoteServerClient::from_child_streams`. Nothing downstream knows what
  spawned it.
* The remote command is `{binary} remote-server-proxy --identity-key {key}` —
  a `#[clap(hide = true)]` subcommand of the same binary.
* The proxy is "a thin byte bridge"; the daemon is long-lived, per-identity-key,
  on a 0600 unix socket.

The server binary is **buildable here**: `script/deploy_remote_server` runs
`cargo build -p warp --bin warp --features standalone,… --target
x86_64-unknown-linux-musl`. The CDN download in `install_remote_server.sh` is
the convenience path, not the only one — which is the part that matters for a
fork with no account.

## Verified by running, 2026-08-22

The decisive question was whether any of this is account-gated. It is not.

* `warp-oss remote-server-proxy --identity-key fork-probe`, **logged out, no
  API key**: proxy started, found no daemon, spawned one, socket ready in
  1.10s, bridged stdio, exited cleanly. Daemon persisted afterwards.
* Then, speaking the protocol straight to the daemon's unix socket by hand —
  framing is `[4-byte LE length][protobuf]` — a **credential-free `Initialize`,
  which is a zero-byte message in proto3**: no token, no user id, no email.

  The daemon answered twice: a `RemoteAgentContextSnapshot` carrying
  `/home/effatha`, then an `InitializeResponse` echoing the request id and
  returning `host_id` `503cfc29-…`. **The handshake completed.**

This matches the code — `handle_initialize` *stores* the token and replies;
there is no validation branch in it. The only auth check in the daemon is
`validate_remote_codebase_index_auth`, scoped to remote codebase indexing, the
one genuinely cloud-dependent sub-feature. Upstream's proto says so directly:

```protobuf
// Optional bearer token used by the daemon for Warp-server requests.
string auth_token = 1;
// User identity for Sentry crash reports. Empty when not logged in.
string user_id = 2;
```

The token is a pass-through credential for optional cloud calls. **The protocol
was designed to tolerate a logged-out client.**

## The gate, found 2026-08-22 — and it is not an account

With `sshd` installed and keys in place, the next step was to trigger a real
connection. `warpctrl input submit 'ssh localhost'` runs the command in a pane,
which is exactly what upstream's own integration test does
(`enter_remote_server_ssh_command` types the command and presses enter) — so
the WSLg keystroke wall is irrelevant to this test.

It fired the `PreInteractiveSSHSession` warpify hook and then stopped.

The reason is one line in `app/src/features.rs:25`:

```rust
if ChannelState::is_release_bundle() {
    flags.extend(RELEASE_FLAGS);
}
```

`is_release_bundle()` is `cfg!(feature = "release_bundle")`, and
`release_bundle` is **not** in `app/Cargo.toml`'s default list. So the whole of
`RELEASE_FLAGS` — including `SshRemoteServer` — is compiled in and switched off
in **every build you make yourself**, `--release` included. `script/deploy_remote_server`
passes the feature explicitly (`FEATURES="release_bundle,…"`); nothing else does.

Sixth instance of this fork's recurring finding, and a gate shape not seen
before: not `DOGFOOD_FLAGS`, not an account, but a *packaging* feature.
`FeatureFlag::SshRemoteServer` is now in `fork::FORCE_ENABLED`.

## What is still not proven, and what blocks it

With the flag on, the same submit still produced only
`PreInteractiveSSHSession`. Reading the event handlers explains why: that hook's
handler is an **empty block** (`view.rs:12674`). The trigger is a different
event —

```rust
// Handled by RemoteServerController via model subscription.
ModelEvent::SshInitShell { .. } => {}
```

— which comes from the `InitSubshell` shell hook, emitted when **Warp's
bootstrap runs inside the remote shell**. Counting hooks in the log for that
run: 16 `InitShell`, 32 `Precmd`, 1 `PreInteractiveSSHSession`, and **zero
`InitSubshell`**. The ssh session was never warpified on the far side, so the
trigger never fired.

`EnableSshWarpification` defaults to `true`, so the setting is not the cause.
The hypothesis was that warpification rewrites the ssh command as it is
submitted, and `input.submit` — which replaces the buffer and runs it (T1.8) —
takes a path that skips that rewrite.

### Confirmed the same day, by typing it by hand

The user ran `ssh localhost` in the rebuilt build from the keyboard, and got
the host-key prompt, a password prompt, and then a bottom-sheet modal:

> **Install Warp's SSH extension** *(recommended)* — "Install Warp's extension
> to enable features like file browsing, code review, intelligent command
> completions in this session" / **Continue without installing**

That is `app/src/terminal/view/ssh_remote_server_choice_view.rs:78-81`, word
for word. **It is the remote-server install choice block.** So:

* **The `FORCE_ENABLED` change works.** That modal cannot render with
  `SshRemoteServer` disabled.
* **The trigger fires** when the command is typed. The path from warpify →
  `InitSubshell` → `SshInitShell` → `RemoteServerController` is live.
* **`input.submit` does not warpify an `ssh` command.** Confirmed, and a real
  limit of the action worth knowing: T1.8 verified it *runs* a command, and it
  does, but it bypasses whatever rewrites `ssh` on the normal submission path.
  Anything an agent drives through `input.submit` gets a plain SSH session.

### And then it connected — the whole stack, live, with no account

Staging the binary at the bare path was the last missing piece:

```bash
mkdir -p ~/.warp-dev/remote-server
ln -sf ~/git/warp/target/release/warp-oss ~/.warp-dev/remote-server/warp-oss
```

(`~/.warp-dev` rather than `~/.warp-oss` is upstream's own OSS fallback, and
`warp-oss` is `Channel::Oss.cli_command_name()`.)

With that in place, `ssh localhost` prompted for nothing and dropped straight
into a session — which *looks* like the wrapper-only fallback and is the
opposite. `check_binary` succeeded, so the manager skipped the install pipeline
entirely and went to connect. **The absent prompt is the success signal.**

The process tree during that session:

```
remote-server-daemon --identity-key 4216d34b-5771-4b55-8f98-700f4c98af37
  └─ terminal-server --parent-pid=<daemon>
ssh -q -o PasswordAuthentication=no -o ForwardX11=no \
    -o ControlPath=~/.ssh/14848256040867250719 placeholder@placeholder \
    ~/.warp-dev/remote-server/warp-oss remote-server-proxy --identity-key 4216d34b-…
  └─ warp-oss remote-server-proxy --identity-key 4216d34b-…
```

Every layer of the design, running: the long-lived per-identity daemon with its
own `terminal-server` child, the `ssh` child Warp spawned with a ControlMaster
path (`placeholder@placeholder` because the real host is multiplexed through
the existing master), and the proxy bridging its stdio to the daemon's unix
socket. Matching identity keys throughout. `~/.warp-dev/remote-server/` holds
the daemon's `server.pid`, its 0600 `server.sock`, and a per-identity `data/`
directory.

Note the daemon outlived the GUI that spawned it and was reused by the next
one — daemon at 14:32:56, a later GUI at 14:33:37, its ssh+proxy pair at
14:34:03 attaching to the *existing* daemon. That is the intended lifecycle,
observed rather than read.

**So Warp's remote-development stack runs end to end on a fork with no account,
no API key and no CDN.** The claim is no longer "the handshake is not gated" —
it is that the product works.

What that leaves for [I16's shopping list](#what-is-actually-missing) is item
1 alone: a `WslTransport`. Items 2–4 are still true but are now the difference
between "works over SSH" and "works over `wsl.exe`", which is one `Command`
and a distro picker.

## Correction: WSL does have an ambient trigger

Written twice in this entry and in two source files: that WSL "has no
equivalent trigger and cannot have one". **Wrong.** `wsl` and `wsl.exe` are
already warpify subshell commands on Windows — `WSL_SUBSHELL_REGEX` in
`terminal/warpify/settings.rs`, paired with a `WSL_IGNORE_REGEX` that filters
out `--list`, `--shutdown` and the rest so only interactive launches count.
Typing `wsl` warpifies the session exactly as `ssh` does.

What a warpified WSL session does *not* get is a remote server, and the reason
is structural rather than missing: the attach is keyed on
`IsSSHWrapperSession::Yes`, whose payload is a **ControlMaster socket path**. A
WSL session has no such socket and cannot have one — the same fact that lets
`WslTransport` report `ControlPath::None`.

So the ambient path is a WSL arm beside the SSH one, not a new hook, and
`Session::wsl_name()` already carries the distribution. The explicit action
stays useful regardless: it is repeatable, agent-drivable, and testable from
outside the GUI.

## The flag needs no further opening

`FeatureFlag::SshRemoteServer` sits in both `DOGFOOD_FLAGS` and `RELEASE_FLAGS`
behind `#[cfg(not(windows))]`, commented "Remote server binary is not yet
supported on Windows". **That cfg is already bypassed for this fork**, because
`fork::FORCE_ENABLED` sets a *user preference* and `FeatureFlag::is_enabled`
resolves in this order:

```rust
overrides::get_override(*self)
    .or(USER_PREFERENCE_MAP[*self as usize].get())    // fork::FORCE_ENABLED
    .or(Some(FLAG_STATES[*self as usize].load(..)))   // RELEASE_FLAGS, cfg'd
    .unwrap_or(false)
```

User preference is consulted first, and the enum variant itself is not
cfg-gated. So there is nothing to remove, and removing it would be an upstream
edit for no behavioural gain — against this fork's rebasability rule.

Worth knowing what that turns on, though: the flag gates *both* transports, so
a Windows client also gets the SSH remote-server path, which upstream disabled
there. The WSL path is unaffected by whatever motivated that — it touches no
ControlMaster.

## What is actually missing

0. **Run it on Windows.** The runbook is in `README.md` under "Warp's remote
   server, in a WSL distribution". Everything below is downstream of it.
1. **A `WslTransport`.** Seven trait methods, and simpler than the SSH case:
   `Command::new("wsl.exe").args(["-d", distro, "--", …])` replaces
   `Command::new("ssh")`, and there is no ControlMaster, no socket lifecycle
   and no auth to manage. `ControlPath` and `warp_owns_control_master` are
   SSH-only concerns a WSL transport simply does not have.
2. **The Windows gate.** `FeatureFlag::SshRemoteServer` is in `RELEASE_FLAGS` —
   on by default — wrapped in `#[cfg(not(windows))]`, commented *"Remote server
   binary is not yet supported on Windows."* That is exactly the client side
   WSL needs. The flag name will want widening too; "Ssh" is no longer the only
   transport.
3. **Distro enumeration and a picker.** `wsl.exe -l -q`. Zed's equivalent is
   "Add WSL Distro" under Open Remote.
4. **An OSS server binary and an install path that is not the CDN.** Native
   Linux build rather than musl cross-compile, since the target is the same
   machine.

## Two rough edges found while probing

* **`server_version` came back empty.** The OSS build reports no version
  (`--dump-debug-info` → `Warp version: None`), and `RemoteServerManager`
  compares client and server versions after the handshake — disagreement means
  *remove the binary and reinstall*. With `None` on both sides this may be
  benign or may loop; it needs checking before anything ships.
* **Upstream already noticed the OSS gap** and left it, in
  `remote_server_dir()`:

  ```rust
  Channel::Oss => {
      // TODO(alokedesai): need to figure out how remote server works with warp-oss
      // For now, return what Dev returns.
      ".warp-dev"
  }
  ```

  Which is why the daemon's state landed in `~/.warp-dev/`. Nobody wired this
  for OSS; it works anyway.

## Why this is worth the size

Every other selected item makes Warp nicer. This one changes what it *is* on
Windows: a native-feeling client whose files, terminals, language servers and
agents all live on the fast side of the 9p boundary. It is also unusually
well-matched to this fork — the remote server is the one large Warp subsystem
that turns out to need no account, and `warpctrl` plus the run-scale graph
already assume an agent driving panes and sessions that could just as easily be
remote ones.

The honest caveat: everything above the "Verified" section is read, and the
verification covered the **server** side end to end but never a real
client→server round trip (no `sshd` on this machine, and `sudo` is denied). The
client half — `RemoteServerManager`, install, reconnect — has not been run.

Three of the five questions came back the same day, and two of the answers
changed the work rather than confirming it.

---

