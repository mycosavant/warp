# Handoff: secrets hygiene, for users other than the maintainer

Written 2026-09-13, from a walkthrough the maintainer accepted the same
evening. **Start after `HANDOFF-MERGE.md` lands**; task 2 touches upstream's
`crates/warpui_extras`, which a merge can move. Retire when all three tasks
are done.

**The frame, in the maintainer's words:** *"Eventually there will be users
other than me, so we def need more robust security practices and hygiene."*
So judge every default here by what it does for a stranger who never reads
`.fork/`, not for a maintainer who knows where the files are.

## The threat model, chosen 2026-09-13

| # | threat | in scope? |
|---|---|---|
| 1 | a **copy** of the files: a backup, a synced folder, a lost disk | **yes** |
| 2 | **another account** on the same machine or distro | **yes** |
| 3 | **anything running as you**, including an agent you named with a shell | **no**. No encryption at rest helps, since a process as you can call DPAPI too. This is `agent-transports.md`'s "naming an agent is trusting it", and its real answer is running agents as another OS user or in a sandbox, not a store. Say so wherever a user could assume otherwise |
| 4 | a **stolen console authority key**, which impersonates HTTPS to a phone that trusts it | **yes**, and it is the worst single file |

**A full vault with a master passphrase is out of scope, deliberately.** It
helps against threat 3 only while locked and costs a prompt every launch.

## What was found, 2026-09-13 (read and `stat`, not exploited)

| secret | where | protection as found |
|---|---|---|
| API keys, Windows | one file per key in Warp's state dir (`secure_storage/windows.rs`) | DPAPI, bound to the Windows login |
| API keys, Linux/WSL and TUI | Secret Service if one runs; **WSL runs none** (no keyring process, no `/run/user/1000/keyring`), so fallback files | AES-GCM under a key that is a **constant string in upstream's source** (`secure_storage/linux.rs:109`). Files are **`644`**: `~/.local/state/warp-terminal-tui/dev.warp.WarpTui.tui-AiApiKeys`, `~/.local/state/warp-oss/dev.warp.WarpOss-FileBasedMcpCredentials` |
| console authority private key | `fork::state_dir()/console-ca.key`, on the desk `%LOCALAPPDATA%\warp\WarpOss\data\fork\` | plain PEM. **No name constraints** in `app/src/local_control/tls.rs` (grep). ACL not checked |
| pairing tokens, action credentials | memory only | purged on stop. Fine |
| agent credentials | the agents' own configs | not Warp's; list them, do not move them |

Also seen and not opened: `~/.local/state/warp-oss/rudder_telemetry_events.json`,
3,419 bytes, dated 2026-08-17. Say what it is in task 1.

---

## Task 1: `docs/secrets.md`, a measured inventory

A surface page in the usual shape (*as of*, state table, what is open). Every
secret Warp or an agent it spawns reads or writes, on each platform: path,
permissions (`stat` on Linux, `icacls` on Windows, including the authority key
and the DPAPI files), what protects it, and which of the four threats that
protection answers. The table above is the start; **re-measure it, don't copy
it.** Add the check to the merge gates in `HANDOFF-MERGE.md`'s successor
(T10) so a merge that adds a secret gets noticed.

## Task 2: the Linux fallback stops looking encrypted

It is upstream code, so keep the change small and argue it in the commit body.

- **Create fallback files `0600`**, and tighten an existing file to `0600` when
  it is read. Test both; calibrate by breaking.
- **Stop presenting the constant-key AES as protection.** Either keep the
  format and say plainly where the key is entered that the key is stored
  unencrypted because no Secret Service is running, or refuse to persist and
  say why. Pick the one a stranger is least likely to misread, and say which
  you picked in the commit. **Do not invent new crypto** (a random key in a
  file beside the ciphertext answers none of the four threats better than
  `0600` does).
- Note in `docs/secrets.md` that a Secret Service provider in WSL (e.g.
  gnome-keyring) is the path to real protection there, untested.

## Task 3: the console authority, constrained and kept better

**The certificate is correct; the phone's browsers are the problem, and that is
why the maintainer lives in mosh+tmux today.** `reach.html` (2026-09-09) says
Firefox trusted the installed CA and Chromium browsers (DDG, Chrome) check
Android's system store instead. **The maintainer reported on 2026-09-13 that
Firefox also shows the page as not secure**, despite the CA being installed.
Those two disagree; measure on the emulator (`.fork/tools/phone.sh`) before
building anything on either. Possible causes, read not measured: Firefox for
Android only uses user-installed CAs with its third-party CA setting on, and
the DDG browser may not accept user-added roots at all.

Then:

1. **Name constraints on the authority.** rcgen 0.14.10 has `NameConstraints`
   and `GeneralSubtree`. Permit `100.64.0.0/10` (the tailnet range, which
   headscale keeps by default, so the switch needs no new authority), loopback,
   and whatever `WARP_FORK_CONTROL_BIND` can legitimately name; exclude
   nothing else explicitly. **Measure on the emulator that Chrome and Firefox
   still accept the leaf with the constraint present**, since some clients
   treat an unfamiliar critical extension as a failure.
2. **A new authority means the phone reinstalls it.** Write the migration: the
   old authority is removed from the phone (Settings › Encryption &
   credentials › User credentials), the old key deleted, and the new one
   installed. The console should say which authority fingerprint it serves, so
   a person can tell the two apart in that list.
3. **Keep the key better at rest.** On Windows, DPAPI through the existing
   `secure_storage`, or at minimum confirm the file ACL is user-only. On Linux,
   `0600`.
4. **Write down what this does not fix:** a private CA stays friction in every
   browser that ignores user roots. That is the maintainer's stated motivation
   for a native app, which can pin the fork's authority inside the app and
   never ask the phone to trust a root system-wide
   (`decisions/2026-09-13-a-native-android-app-is-a-destination.md`).

Commits `fork: <subject> (SECRETS)`. Run records for the emulator measurements
under `.fork/runs/secrets-<date>/`.
