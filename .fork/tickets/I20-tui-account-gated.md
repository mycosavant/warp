> Idea I20, split out of `.fork/IDEAS.md` on 2026-09-05. History; read the section you came for.

# I20 — the TUI is account-gated, and the fork's bypass cannot reach it

**Measured 2026-08-30.** `crates/warp_tui` builds `warp-tui-oss`, "Warp Agent
CLI". Run from a phone over SSH, it **demands a Warp login**. That also explains
the spinner recorded in `CLAUDE.md`: it was waiting on authentication.

**`CLAUDE.md` corrected 2026-09-01** to state this measurement instead of the
retracted model-credential hypothesis.

## Why this is not a one-line fix

This is the fork's most repeated finding — *the feature exists and is switched
off* — with a twist that decides the whole scope. The account gate is bypassed by
`account_gate_bypassed` in `app/src/fork.rs`, and **`crates/warp_tui` contains
zero `fork::` references** (verified by grep, along with `acp_agent`,
`local_agent` and `generate_multi_agent_output`). So the bypass is not disabled
there — it is *structurally absent*. The GUI is un-gated because the app crate
asks; the TUI never asks.

Which kind of gate this is matters, and `CLAUDE.md`'s own distinction applies:
`FORCE_ENABLED` outranks a flag-list entry but cannot conjure code that was never
written. This is the second kind. **Unverified:** whether the login demand is a
feature flag the TUI consults at all, or a hard requirement in its startup path.
That is the first thing to find out and it is a reading job.

## The cheap alternative, tried — and it does not work

`--set-provider-api-key <openai|anthropic|google|grok>` was the hope: a user's
own provider key is this fork's thesis nearly verbatim, so if it removed the
login demand the TUI would be on-thesis today with no fork changes at all.

**Measured 2026-08-30, and it does not.** With a key stored (a deliberately
invalid one, since the question was about the *gate* and not about model
access), the TUI still opens on:

```
Welcome to Warp
● Waiting for login...
Visit https://app.warp.dev/device?user_code=…&source=warp-agent-cli
```

A **device-code OAuth flow against `app.warp.dev`**. So the gate is Warp's own
**account**, not model access, and no provider key will ever satisfy it. The
more interesting branch is the one that happened.

**And the fork does not block it, which is the right scoping.** `warp.dev` is
**not** on `BLOCKED_HOST_SUFFIXES` — that list is telemetry and analytics
(sentry, segment, amplitude, posthog, datadog, statsig, …). So the login would
succeed for anyone holding an account. **This is therefore a policy question and
not a technical one**: the TUI is usable today by signing in, and the reason not
to is the fork's thesis rather than any obstacle.

**What it also proves: the TUI itself works on a phone.** Rendered over SSH in
Termux it lays out correctly, the type is legible, the key row is usable. The
account is the *only* blocker between here and a working phone client.

## What is already true, and worth not losing

**The telemetry deny-list reaches it anyway.** `egress::is_active()` reads only
`WARP_FORK_ALLOW_TELEMETRY_EGRESS` and lives in `crates/http_client`, which
`warp_tui` links. So the fork's strongest claim holds in a binary the fork has
never edited — because that policy was put in the shared HTTP client rather than
in the app's policy seam. Recorded here because it is the argument for where the
*next* policy should live.

## What it would cost — **corrected, because the first estimate was wrong**

This entry originally said un-gating would mean "a second fork surface in a crate
the fork has never touched." **That is wrong**, found by doing the reading job
the entry itself asked for. The decision point is not in `crates/warp_tui` at
all:

```rust
// app/src/tui/mod.rs:240
fn has_validated_identity(auth_state: &AuthState) -> bool {
    auth_state.is_logged_in() && auth_state.user_id().is_some()
}
fn initial_login_phase(auth_state: &AuthState) -> TuiLoginPhase {
    if has_validated_identity(auth_state) { LoggedIn } else { SignedOutWelcome }
}
```

That is the **app** crate, where `app/src/fork.rs` already lives and is trivially
reachable. `TuiUserInfoSnapshot` (`app/src/tui/user_info.rs:24`) takes
`is_logged_in` from the same function at `:76`. And `app/src/tui/*.rs` has zero
`fork::` references, so `account_gate_bypassed()` is simply never asked — **no
feature flag is consulted anywhere in this path.** It is a plain predicate on
`AuthState`.

So the shape is the fork's most familiar one after all, and in reach.

**But the cost that actually matters is not where the code lives.** Two things
stand, and the second is the one to settle before anyone edits a line:

1. **A TUI session would carry the fork's telemetry posture and none of its
   consent posture** — no ACP, no permission parking, no console, no pairing,
   none of T14. "Warp on my phone" would look like the same product and be
   something else. That is a disclosure problem before it is a code problem.
2. **Unknown, and it decides everything: is the gate cosmetic or load-bearing?**
   `warp_tui` has no fork agent seam, so its agent is presumably Warp's *own*
   cloud agent, which would need the account regardless of what the UI shows. If
   so, lifting the gate buys a nicer failure rather than a feature. Note that
   `logged_in` also guards `activate_global_mcp_servers` and
   `load_current_account` in the same function, which is evidence it gates real
   machinery rather than a screen. **Under review; do not build against either
   answer until it is settled.**

And if the login turns out to be a commercial control rather than a technical
one, the answer is to leave it alone and say so.

## The smallest version that is still the idea

1. ~~Try the API key.~~ **Done — it does not lift the gate.**
2. **Read the startup path** for what the login actually gates: whether any
   feature flag is consulted, or whether it is unconditional. This is the open
   step, and it is a reading job.
3. Only then decide whether a second fork surface is worth a phone-sized
   terminal, given SSH already gives a shell with all 114 `warpctrl` actions and
   the full fork behaviour behind it.

## As found — step 2 done, and question 2 is settled

**Measured 2026-08-30, landed as `e4f52077a`.** The reading job above was done and
the answer is the one this fork keeps finding: **no feature flag is consulted
anywhere in the path.** `initial_login_phase` is a plain predicate over
`AuthState`, in the app crate, two lines from a `fork::` call that had never been
made.

**Question 2 — cosmetic or load-bearing — resolves to *both*, split by
transport, and the split is the whole answer.** The gate is load-bearing for
Warp's own cloud agent, which needs the account whatever the screen says. It is
**cosmetic for the fork's transports**, because `generate_multi_agent_output` is
intercepted before `ServerApi` is ever reached. So the correct predicate is not
`has_validated_identity` — it is the account gate **conjoined with the fork
actually having an agent that will answer**:

```rust
fn fork_agent_will_answer() -> bool {
    crate::fork::acp_agent_command().is_some() || crate::fork::local_agent_enabled()
}
```

Deliberately *not* widened to `has_validated_identity` itself, which also feeds
`TuiUserInfoSnapshot::is_logged_in` and guards `load_current_account` and
`activate_global_mcp_servers`. Lifting those would be claiming an account exists.
This lifts one screen for one case that can be served without one.

**Measured with it in place:** the TUI opens (`Not signed in`, 22 skills
discovered), a fork ACP agent answers, and a permission request **parks
correctly**. The consent architecture is intact in a binary the fork had never
run.

## The blocker it exposed, which was never about the gate

The parked request produced TUI prose instructing the user to run `warpctrl agent
approve 7847a317…` — **a command that cannot exist in that process.** The
`LocalControlServer` is registered behind a `matches!(launch_mode, LaunchMode::App
{..} | LaunchMode::Test {..})` at `app/src/lib.rs:2621` (read, not run), which has
no TUI arm, and `warp_control_cli` is absent from `warp_tui`'s features. So the
note named the one instrument guaranteed to be missing.

**That bug never needed the bypass to exist.** A signed-in TUI with
`WARP_FORK_ACP_COMMAND` set had it too — which is why the fix is not part of the
bypass and stands on its own. `fork::local_control_serving()` is now recorded at
the single site that registers the server and read by `asking_note`; where nothing
can answer, the note says **"Nothing in this session can answer it"**, drops the
paired-device sentence, and names Ctrl-C.

**Ctrl-C is measured to escape** — it closes the permission, cancels the turn, and
the next message can be typed. That is what made keeping the predicate
defensible: a disclosed, escapable refusal is honest, where a wedge with no exit
would have been worse than no bypass at all.

One test finding worth keeping, because it is the general shape. The first
version of the test asserted the note must never contain the string `warpctrl` —
and it **failed against copy that was right**, because the note names the tool *as
absent*. Someone who knows `warpctrl` is exactly the person who will reach for it,
so naming it is the useful thing to do. The test now forbids a runnable *command*
(`agent approve`/`deny`/`approvals`), not a mention.

## Not built, and the hazard to name before anyone does

**Deferred on the advisor's ruling: build nothing a friction log has not asked
for.** The panel earned its button after **35 measured copy-paste approvals**
(T14.9 → T14.16). The TUI has zero sessions of friction log. The sequencing is
this fork's own, and it has been right every time it was honoured.

If a TUI answerer is ever built, the cheap path is known — one `LaunchMode` arm at
`app/src/lib.rs:2621` plus `warp_control_cli` in `warp_tui`'s features, which
would also serve the T12 console and its already-built per-entry gating, adding
**zero** new consent-surface code. That is this fork's most repeated finding
wearing its usual clothes, and it is why (C) is the option to falsify first.

**But it carries a hazard the panel never had, and it is named here so it is not
discovered later as a fourth-surface T14.6: type-ahead.** A TUI prompt appears in
the same terminal the person is typing into. An Enter already sitting in the input
buffer when the prompt takes focus is a **yes nobody gave** — consent
manufactured by the terminal's line discipline rather than by a decision. The
panel's two-tap arming is the answer in different clothes, and the reason it
exists is stated in T14.16's own "not built, deliberately": *binding Enter to a
permission grant is a decision with its own argument, because a single cheap
gesture should not be able to say yes.*

In a TUI the gesture is cheaper still, because the person did not even have to be
looking. Any answerer here must drain the input buffer before arming, or require a
key that no buffered newline can supply. **Unverified**, and it is the first thing
to measure rather than reason about: whether the buffered Enter actually lands, on
which terminal, and whether SSH latency widens the window. Measure it before
designing against it.

---

