# Handoff: security, safety and privacy for a user who is not the maintainer

Written 2026-09-13. Replaces `HANDOFF-SECRETS.md` from earlier the same
evening, which had three claims the code contradicts (the authority key is
already `0600` on Unix; the owner-only fallback writer already exists; the
pairing token is not memory-only, the phone keeps it). The assessment below is
the fable-advisor's, adopted by the maintainer as the record's posture.
**Start after `HANDOFF-MERGE.md` lands.** Tasks are ranked by likelihood times
impact for a stranger on default settings; take them in order.

Decided by the maintainer, 2026-09-13
(`decisions/2026-09-13-security-safety-and-privacy-go-further.md`): the
fable-advisor's assessment is the record's posture, and the fork goes further
on security, safety and privacy wherever that is reasonably possible, because
it will have users other than the maintainer. They accept that a third-party
harness such as Claude Code or Codex, talking to its vendor's models, limits
what the fork can control.

Claims are tagged RAN, READ, TOLD or ASSUMED. "READ" below was read by the
advisor and the cited lines re-read by the session that wrote this file.

---

## Task 1: a session where Warp is never asked needs a one-time acknowledgement

Decision: `decisions/2026-09-13-a-session-warp-is-not-asked-in-is-acknowledged-first.md`.

`fork::acp_mode()` has no default on purpose (`app/src/fork.rs:430-455`, READ).
A stranger who names `claude-agent-acp` and never learns `WARP_FORK_ACP_MODE`
runs in the agent's `auto` mode, where its own classifier approves (TOLD, the
fork's T14.8/T14.18 runs). A prompt injection in a cloned repo's README then
drives a shell as the user, and the event log shows zero permission requests,
which reads as "nothing happened".

**Done means:** before the first turn of a session in a mode where Warp will
not be asked, the panel stops and asks once, naming the mode in the agent's own
words (T14.18 already reports it), what that means, and how to change it
(`WARP_FORK_ACP_MODE`). The turn does not run until acknowledged. Warp still
never picks a mode.

**Design questions to settle by running, not reading:**

- Which modes count as "Warp not asked"? Mode ids are opaque per agent. Key it
  on agents **measured** (claude-agent-acp: `auto`, and check its other five
  modes with `acp probe`) and, for an unmeasured agent that advertises modes,
  on "no permission request arrived in a turn that wrote a file" rather than on
  a guessed id. Say which in the commit.
- Is the acknowledgement remembered per agent, per mode, or per workspace? The
  stranger case argues per agent name and mode.
- It gates a turn, so it is a consent surface: read `docs/composer.md`'s
  approval-card section and T14.16's argument about cheap gestures first.
- Measure live on Windows with a scratch profile: unset mode asks, a set
  `default` does not, the acknowledgement survives a relaunch.

## Task 2: other accounts can read the history database, logs and state

READ: `init_db` makes the database owner-only only for
`PersistenceScope::RemoteServerDaemon` (`app/src/persistence/sqlite.rs:367-378`).
RAN (`stat`, 2026-09-13): `~/.local/state/warp-oss` is `755`, `warp.sqlite`
and its `-wal` are `644`, as are the logs and `hook-dumps/`.

**Done means:** on Linux the app's state directory is `0700` and the database,
WAL, SHM, logs and hook dumps are `0600`, created that way and tightened on
launch for existing files. `ensure_owner_only_dir` / `ensure_owner_only_file`
and `fork::create_private_file` / `tighten_existing` already exist; reuse
them. Also the WSL transcripts written by the Windows build (disclosed at
`app/src/fork.rs:1564-1580`, READ): decide whether they can be made private
from the Windows side or must stay disclosed.

## Task 3: Linux and WSL keys stop sitting at 644 under a public key

READ: `write_value` falls back to `write_fallback_value`, a plain
`std::fs::write` (`crates/warpui_extras/src/secure_storage/linux.rs:236-238`);
the AES key is a constant string (`:109`). **The owner-only writer already
exists**, `write_owner_only_fallback_value` (`:240`), called from one place
(`app/src/settings/local_control.rs:106`, READ by the advisor). RAN: WSL runs
no Secret Service; `dev.warp.WarpTui.tui-AiApiKeys` and
`dev.warp.WarpOss-FileBasedMcpCredentials` are `644`.

**Done means:** every fallback write is owner-only, existing files are
tightened when read, and wherever a key is entered the user is told plainly
when it is stored without a Secret Service. No new crypto: a random key beside
the ciphertext answers no threat better than `0600`. Note in
`docs/secrets.md` that a Secret Service provider in WSL is the route to real
protection, untested. The crate is upstream's; keep the change small and argue
it in the commit body.

## Task 4: a paired phone's control token expires after twelve hours

Decision: `decisions/2026-09-13-a-control-pairing-expires-after-twelve-hours.md`.

READ: `Scope::Watch => Some(now + DEVICE_LIFETIME), Scope::Control { .. } => None`
(`app/src/local_control/pairing.rs:436-439`). A control token ends only on Stop
sharing, on the conversation being deleted, or on Warp closing. The phone keeps
it in `localStorage` (`console.js`, `DEVICE_KEY`). `console.js:98` says *"the
server bounds the token to twelve hours"*, which is false for a control
pairing.

**The history, so nobody re-derives it:** no clock was deliberate, commit
`ae694127a` implementing `decisions/2026-09-06-the-phone-surface-four-decisions.md`,
now superseded. What the maintainer remembers as single-use is the pairing
**code**, which still is (`pairing.rs:425-432`, READ).

**Done means:** control tokens also expire after twelve hours, the phone is
told why it must pair again, `console.js`'s comment is true, and a test pins
both scopes. Without it, a lost phone keeps a live credential for as long as
Warp runs with the conversation shared, and with `WARP_FORK_REMOTE_APPROVE` set
it can approve agent actions.

## Task 5: the console authority can sign for any site for ten years

READ: `IsCa::Ca(BasicConstraints::Unconstrained)`, no name constraints,
`AUTHORITY_LIFETIME` 3650 days (`app/src/local_control/tls.rs:95, 296-299`).
The key is written `0600` on Unix through `create_private_file`
(`tls.rs:225`); **on Windows no ACL is set**, the file inherits its parent's
(`fork.rs:1553-1556`). The authority's certificate is served over plain HTTP
for installation (`tls.rs:335-341`, READ by the advisor), so whoever is on the
path at install time can substitute their own.

1. **Measure the browser problem first**, on the emulator (`.fork/tools/phone.sh`).
   `reach.html` (2026-09-09) says Firefox trusted the installed authority and
   Chromium browsers did not; the maintainer reports Firefox also shows the
   page as not secure (TOLD, 2026-09-13).
2. **Name constraints**: permit `100.64.0.0/10` (the tailnet range, kept by
   headscale), loopback, and what `WARP_FORK_CONTROL_BIND` may legitimately
   name. rcgen 0.14.10 has `NameConstraints` (READ). Measure on the emulator
   that Chrome and Firefox still accept the leaf.
3. **Install-time fingerprint check**: the desk shows the authority's
   fingerprint, the install page shows the same, and the doc says to compare
   before installing. This closes the plain-HTTP substitution for anyone who
   compares.
4. **Lifetime**: shorten it, and write down the cost (a phone reinstall each
   time) that the choice was weighed against.
5. **Windows key at rest**: DPAPI through `secure_storage`, or an explicit
   user-only ACL.
6. **Migration**: remove the old authority from the phone, delete the old key,
   install the new one.

The maintainer, 2026-09-13 (TOLD): this risk is part of why they have moved to
mosh and tmux for remote work, which they also prefer, and it strengthens the
case for the native app
(`decisions/2026-09-13-a-native-android-app-is-a-destination.md`).

## Task 6: the clients the egress backstop never sees

`HANDOFF-MERGE.md` task 2b pins the list before the merge; its table is the
current inventory (READ 2026-09-13, corrected by a second reader the same
night). This task decides, per production client, whether it should pass
through `egress_policy` or stay outside with its reason written beside it:
the MCP HTTP and SSE transports (`crates/mcp/src/runtime.rs:175`,
`sse_transport/reqwest_impl.rs:88`), which reach whatever server a user
configures; the tracing exporters (`app/src/tracing/local_export.rs:28`, gated
to loopback, and `cloud_agent_auth.rs:112`); and `warpctrl`'s loopback client
(`crates/local_control/src/client.rs:54,114`). The telemetry module's client is
**not** on the list: it is wrapped by `http_client::Client::from_client_builder`.

## Task 7: `docs/secrets.md` and the header fingerprint

- A measured inventory of every secret Warp or an agent it spawns touches, per
  platform: path, `stat` or `icacls`, protection, which threat it answers.
- `include_warp_http_headers` returns `true` on every non-wasm target
  (`crates/http_client/src/lib.rs:302-304`, READ), so the verb builders attach
  the client id, app version and OS details to every host. Find the third-party
  call sites and move them to `get_without_warp_headers`.

## Threat model for this handoff

In scope: a copy of the files, another account on the machine, a stolen
authority key or phone token, the agent acting without Warp being asked, and a
client that bypasses the egress backstop. **Out of scope for a store, not for
the fork**: anything running as the user. Encryption at rest cannot answer it;
task 1 narrows it for agents and running agents in a sandbox is the real answer
later.

Commits `fork: <subject> (SECURITY)`. Run records for emulator and Windows
measurements under `.fork/runs/security-<date>/`.
