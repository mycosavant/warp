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

**Caught live and measured 2026-09-05, and it settles the "documented refusal"
half.** During the Windows egress run, `CloudObjects::Listener` — the Warp Drive
cloud-sync GraphQL subscription over exactly this WebSocket path — retried every
~30 s, each attempt logging *"missing authentication credentials"*. It **never
reached a socket**: the 1716-sample poller with `SynSent` included recorded zero
non-loopback `warp-oss` sockets. The reason is in the source, not inferred:
`get_or_refresh_access_token`
(`crates/warp_server_client/src/auth/session.rs:101`) bails with that string
**before any dial** when `auth_state.credentials()` is `None`, which it always
is in this accountless fork. So the login gate is a real *pre-socket*
enforcement point today: no credentials, no request built, no WebSocket. The
egress deny-list's blind spot is unreachable without an account.
`.fork/runs/egress-windows-2026-09-05/`.

### Still open

- [x] **Third egress enforcement point — built 2026-09-09.**
      `websocket::WebSocket::connect` consults the deny-list before
      `imp::connect`, and refuses with an error naming the host and the switch
      rather than blackholing, because this path returns `anyhow::Result` and
      can say what refused it where `reqwest` cannot.

      **The shape this ticket recommended was right and its reason was too
      weak.** It said an `http_client` dependency *"would invert the
      layering"*, which is an opinion a later reader can weigh and overrule.
      It is a **cycle**: `http_client` → `warp_core` → `websocket`, so the
      dependency does not compile. The lists, the switches and the argument
      moved to `crates/egress_policy`, a leaf with no dependencies, and both
      clients consult it. `http_client::egress` keeps only what needs a
      `reqwest::Request` in hand.

      **The consumer-list pin found a second door on its first run**, which is
      the whole case for pinning by count rather than trusting a guard.
      `a_websocket_is_dialled_from_this_crate_alone` asserts that no crate but
      `websocket` declares a WebSocket dialler, and `crates/graphql` does:
      `ws_stream_wasm = "0.7"` at `Cargo.toml:34`, under
      `cfg(target_family = "wasm")`. It is a listed exception, not a hole —
      wasm-only in a fork that ships native, its real subscription path is the
      guarded `WebSocket::connect_with_headers`, and its own source never names
      the dialler, which a second assertion re-checks every run. Not removed:
      deleting a dependency on a target family nothing here builds cannot be
      verified from this side.

      Four tests, each calibrated by making it fail rather than by watching it
      pass, and the calibration earned its keep: deleting the guard from
      `connect` left the structural test **green**, because the file's own
      `fn refuse_if_blocked(` sat above both dials and satisfied the
      "checked first" assertion. A structural test that cannot fail is worse
      than none, because it gets cited.

      What has not changed is the urgency argument recorded here on 2026-09-05,
      and it is worth keeping: this was never a live leak. No host on either
      list is a WebSocket target for any call site, and Warp Drive's
      subscription fails in `get_or_refresh_access_token` before any dial
      because this fork holds no credentials. The point of building it is to
      make "cannot happen" independent of "has no token".
- [x] **May a paired device submit a prompt?** Answered 2026-09-05 by the
      maintainer, and the answer is *a device paired for one conversation
      may*: not the watch scope, which stays as this ticket described it, but
      a code minted at the machine for one conversation, which buys
      `agent.prompt` and `agent.approve` confined to it on the grant.
      `.fork/docs/remote-control.md` carries the argument;
      `.fork/decisions/2026-09-05-remote-control-is-one-conversation-handed-over.md`
      the decision.
- [x] **Repoint `ShareSession`'s dispatch under fork policy.** Done the same
      day: `use_agent_footer/mod.rs` and `terminal/view.rs`'s
      `InputEvent::StartRemoteControl` arm both go to
      `start_fork_remote_control` under fork policy, the login gate in
      `sync_remote_control_button` is bypassed there, and the chip's
      start/stop state reads the pairing map. The seam this ticket named as
      `terminal/view.rs:22356` had moved to 22562 by then; line numbers in a
      read-only ticket are a fact about the day they were read.
- [x] **Hide `HandoffToCloud` under fork policy.** Nothing to point it at.
      Done 2026-09-06: `FeatureFlag::HandoffLocalCloud` in `fork::FORCE_DISABLED`.
      One flag rather than a branch in the footer, because the same flag gates
      the `&` prefix, `/move-to-cloud`, the one-time toolbar migration and
      auto-handoff on sleep, and each of those is the same cloud machine by
      another door. `OzHandoff` was left alone: twenty-odd call sites, most of
      them about cloud panes this ticket has no opinion on. The cargo feature is
      in `default`, so the runtime force is the removal and not a backstop;
      `the_footer_does_not_offer_to_hand_a_conversation_to_a_cloud_it_cannot_reach`
      pins both halves.
      Measured the same day on the Windows release build
      (`.fork/runs/handoff-chip-2026-09-06/`): the `&` hint above the agent
      input and the `/handoff` registry entry both went; the chip itself was
      not in this footer on either binary, because the toolbar layout here
      does not include it. The first attempt screenshotted last night's
      release binary, which the launcher had said in its own output.
- [x] **Re-run the whole ticket before building.** The one empirical claim,
      that the chip renders, held; the rest was built against the code as
      read and then run on Windows (`.fork/runs/remote-control-2026-09-05/`).

---

