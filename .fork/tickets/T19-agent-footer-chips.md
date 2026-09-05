> Ticket T19, split out of `.fork/TASKS.md` on 2026-09-05. History; read the section you came for.

## T19 — The two chips in the agent footer: `/remote-control` and "hand off to cloud"

**Asked 2026-09-02**, from the maintainer noticing both chips in the agent input
footer — one tooltipped *"Log in to use /remote-control"*, one *"Hand off to
cloud"* — and asking whether they can be wired up local-first.

**Read, not run.** Everything below is from the source. Nothing here was measured
against a running instance, and the one empirical input is that the chips render
on screen, which is what settles the flag question. Re-run before building on it.

**Answer: they are two different questions.** `/remote-control` names something
the fork already has most of and cannot reach through *this* path.
"Hand off to cloud" has nothing local to point at.

### The gate is login, and the flag lists would have said otherwise

`FeatureFlag::HOARemoteControl` appears in **no** channel list in
`crates/warp_features/src/lib.rs` — line 844 is the enum declaration and there is
no other mention. Resolved from `DOGFOOD_FLAGS`/`PREVIEW_FLAGS`/`RELEASE_FLAGS`
the answer is "off, dead surface".

It is on. `hoa_remote_control` is in `default` (`app/Cargo.toml:524,693`) and
`app/src/features.rs:495` puts the flag into `enabled_features()` behind that
cargo feature. **Third instance of this trap in a week**, after
`AIContextMenuCode`/`FileBasedMcp` and `full_source_code_embedding`, and the
cheapest possible check caught it: the chip is on the screen.

The gate is login, at `agent_input_footer/mod.rs:2050`.

### What `/remote-control` actually is

`ToolbarItem::ShareSession` (`toolbar_item.rs:127`) → `StartRemoteControl`
(`terminal/view.rs:22356`) → `attempt_to_share_session`
(`terminal/view/shared_session/view_impl.rs:578`). Three properties make this
path un-forkable rather than merely gated:

- **It is a relay.** The scrollback *and* the agent conversation stream over a
  WebSocket to `ChannelState::session_sharing_server_url()`
  (`crates/warp_core/src/channel/state.rs:260`). Not peer-to-peer.
- **The protocol is a closed external dependency** — `session-sharing-protocol`,
  a pinned git rev off `warpdotdev` (`Cargo.toml:282`). Not vendored, not in
  `crates/`, no local build path.
- **A real account is structural, not cosmetic.** `firebase_uid: UserUid` is
  threaded into `on_session_share_started`.

The server URL *is* overridable (`--session-sharing-server-url`,
`ChannelState::override_session_sharing_server_url`, consumed at
`app/src/lib.rs:779`). That looks like a seam and is not one: pointing it
somewhere else only helps if you can speak the closed protocol.

### The fork's account bypass does not reach it, and that is deliberate

`sync_remote_control_button` reads `AuthStateProvider::…is_anonymous_or_logged_out()`
**directly**, not through `fork::is_anonymous_for_ui`, whose doc says the wrapper
is *"deliberately not applied to call sites that decide whether to talk to the
server; those should keep seeing the real auth state so they fail fast instead of
issuing credential-less requests"* (`app/src/fork.rs:1073`).

So the chip is inert by construction and nothing is leaking. Worth recording as a
case where the existing seam got a call site right without anyone revisiting it —
the opposite of this board's usual finding.

### What "hand off to cloud" is

`ToolbarItem::HandoffToCloud`, gated by `AISettings::is_cloud_handoff_enabled`
(`app/src/settings/ai.rs:2359`), which requires `OzHandoff` **and**
`HandoffLocalCloud` **and** `PrivacySettings::is_cloud_conversation_storage_enabled`.
It moves the conversation onto an Oz cloud VM.

There is no local variant to wire it to. "Hand off to a machine I own" is
`remote_server`/`wsl_transport` — a different feature that happens to share a
word. Recommend hiding this chip under fork policy rather than wiring it.

### The local `/remote-control` is ~90% built, and the missing 10% is a decision

Already there: SSH reaches all 114 `warpctrl` actions; the console plus pairing
reaches the six in `PAIRABLE_ACTIONS` (`app/src/local_control/pairing.rs:112`),
seven with `WARP_FORK_REMOTE_APPROVE`.

So the on-thesis move is repointing the chip's **dispatch** at the fork's pairing
flow — one seam at `terminal/view.rs:22356`, no new subsystem, which is the shape
this fork keeps recommending.

**But the paired surface is a *consent* surface, not a *control* surface.**
`PAIRABLE_ACTIONS` is list / approvals / deny / cancel. There is no "submit a
prompt". `/remote-control` means *drive the agent*, which is `input.submit`, and
that is SSH-only today.

Adding prompt submission to a QR-paired device is a real authority increase
against this fork's own stated principle that **authority follows credential
strength** — a QR code is a bearer token displayed to a room and spendable for
two minutes, against an SSH key held by one device. That is the maintainer's
call, and if it is taken it should carry its argument in `pairing.rs` the way
`REMOTE_APPROVE_ACTION` already carries its own.

### Side finding: the egress backstop cannot see WebSocket traffic

Larger than the question that produced it. `crates/websocket` has **no
`http_client` dependency** — it opens sockets through `async-tungstenite` and
`async-net` directly (`crates/websocket/Cargo.toml:32-35`). Both documented
enforcement points live inside `http_client`: `Client::execute_inner`
(`lib.rs:386`) and `RequestBuilder::redirect_if_blocked` (`lib.rs:540`), the
latter added precisely because `eventsource` reached neither.

Raw-socket consumers: the shared-session sharer and viewer,
`app/src/terminal/remote_tty/event_loop.rs`, `crates/graphql`'s subscriptions,
and `crates/warp_server_client/src/iap.rs`.

**Not a live leak** — no host in `BLOCKED_HOST_SUFFIXES` (`egress.rs:59`) is a
WebSocket target. That is exactly the reasoning under which `eventsource` was
closed rather than noted: a
backstop whose coverage depends on where today's call sites point is a fact about
today, and this one is structural rather than a forgotten call.

### Still open

- [ ] **Third egress enforcement point, or a documented refusal.** Independent of
      both chips and the piece most worth doing, because it is the fork's
      strongest claim. Needs a decision on shape first: `crates/websocket` taking
      an `http_client` dependency inverts the layering, so the likelier answer is
      a check at socket construction plus a test pinning the consumer list — the
      way `the_symbol_map_leaves_by_exactly_one_call_site_and_it_is_guarded`
      pins a count rather than trusting a guard.
- [ ] **May a paired device submit a prompt?** Authority question, not plumbing.
      Blocks any repoint of the `/remote-control` chip, because without it the
      chip would open a surface that cannot do what its label says.
- [ ] **Repoint `ShareSession`'s dispatch under fork policy** once the above is
      answered — `terminal/view.rs:22356`.
- [ ] **Hide `HandoffToCloud` under fork policy.** Nothing to point it at.
- [ ] **Re-run the whole ticket before building.** Read-only as recorded.

---

