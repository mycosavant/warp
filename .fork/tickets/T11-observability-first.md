> Ticket T11, split out of `.fork/TASKS.md` on 2026-09-05. History; read the section you came for.

## T11 — Observability first, then the surface  ← DONE (2026-08-26)

> Ratified 2026-08-24. The goal, in the maintainer's words: a project you can
> *"spin up, check in on, fire a session from my phone and keep things moving
> semi-autonomously"* — in the pilot's seat rather than the labourer's.
>
> **The ordering is load-bearing and has two independent reasons.**
> `tusk/docs/handoffs/remote-control-feature-mining.md` argues read-only-first
> because nearly all the *value* is in the read path and nearly all the *risk* is
> in the write path — the engine spawns agents and runs tools, so the write path
> is RCE. This fork adds a second: the failure that cost the maintainer a month
> of kode-rs work was a **silent** one — a swarm of agents running without
> permissions, nothing surfacing it, features implemented but never wired, and
> docs written as though they were. An event taxonomy is the detector for exactly
> that class, and a stream with nothing structured on it is a pipe with no
> protocol. So events come before the surface that carries them.
>
> Migration tiers and their reasoning: `.fork/archive/CONSOLIDATION.md` §4.1.

- [x] **T11.1** A structured event taxonomy (`TR-EVENTS`). The taxonomy already
      existed; what was missing was anywhere to keep it. Built as a projection:
      `WARP_FORK_EVENT_LOG`, one JSONL file per session.
- [x] **T11.1b** Warp's own agent into the same log. World 1
      (`BlocklistAIHistoryEvent` / `BlocklistAIActionEvent`) projected onto the
      same vocabulary, so one `jq` filter answers for every agent in the window.
- [x] **T11.1c** The local agent's tools into the log. **Found by running
      T11.1b, not planned**: on this fork's primary agent path the log carried
      the turn frame and no tools at all, because neither world sees them. Built
      as a third source — `source: "local_agent"` — read off the `stream-json`
      `translate.rs` was already parsing, filed under Warp's conversation id so
      a turn's frame and its tools are one file. It carries `parent_call_id`,
      which no other source can: subagent nesting, also found by running it.
- [x] **T11.2** The first slice of the read surface, end to end and deliberately
      small: **one** `GET` route on `warpctrl` returning current agent/task
      state, **one** SSE endpoint carrying T11.1's events, still bound to
      `127.0.0.1`. No LAN bind, no QR pairing, no web page. It proves transport,
      auth and fan-out against a running app; everything after it is additive.
- [x] **T11.3** Constant-time token comparison — filed as a **prerequisite for
      any bind wider than loopback**, and **the reason given for it was wrong**.
      As filed: `AuthToken(String)` derives `PartialEq`, so
      `verify_authorization_header` short-circuits on the first differing byte
      and leaks a prefix. Two things were assumed rather than checked, and both
      failed:
      - **`verify_authorization_header` has no production callers.** It is
        exercised only by `auth_tests.rs`. The live path is
        `app/src/local_control/mod.rs::lookup_credential`, a
        `HashMap<String, CredentialGrant>::get(secret)`.
      - **A hash lookup is not a prefix oracle.** `RandomState` seeds SipHash
        per process, so a near-miss hashes to somewhere unrelated and reveals
        nothing about how many leading bytes were right. `String == String` is
        not a byte loop either; it lowers to `bcmp`.

      Done anyway, and the entry is corrected rather than dropped, because the
      function is public crate API named exactly what an auth check is named,
      and T11.2 and T11.4 are precisely when someone reaches for it. See the
      as-built for what the test does and does not pin.
- [x] **T11.4** LAN bind behind an explicit flag, plus QR pairing. `warpctrl`
      hardcoded `[127, 0, 0, 1], 0`. Built as a **second** listener rather than a
      moved one, and as a three-step pairing flow whose only displayed secret
      lives two minutes. The must-have that was not on the list turned out to be
      the important one: the catalog contains `input.submit` and `agent.prompt`,
      so a pairing path that mints any credential *is* the RCE the ticket was
      trying to prevent. A paired device gets three read actions.
- [x] **T11.5** `GB-APPROVE` — answer a waiting-input approval remotely. The
      first *write* capability. Two findings reshaped it. **The approval that
      matters was invisible**: `agent.list` reports Warp's own conversations, and
      on this fork the agent a person actually blocks on is a `claude` in a pane,
      which has none — so `agent.approvals` had to reach a different map
      entirely. **And approval here is a keystroke, not a verdict**: Warp has no
      channel to tell a CLI agent "yes", so `agent.approve` presses Return and
      `agent.deny` presses Escape, which is why they are two actions — a paired
      device holds a list of actions, and `deny` travels to a phone while
      `approve` does not without `WARP_FORK_REMOTE_APPROVE`.
      **`GB-GRANTS` was not built, and the as-built argues it should not be.**

**T11 is closed.** All five items shipped and the phase's own framing —
*observability first, then the surface* — has had its first half delivered and
its second half only half-built: there are routes and there is no client.
That is what T12 is.

---

