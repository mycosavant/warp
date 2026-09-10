# Why `session/new` was refused, said out loud (T21 item 5, half 1)

**2026-09-10, from WSL, binary `v0.fork.c25c8fbc2`. No Warp process, no GUI, no
credential.** Everything here reproduces in about four minutes.

Point `WARP_FORK_ACP_COMMAND` at Codex or Gemini and every turn dies at
`session/new` with a raw JSON-RPC error and no advice. `grep -rn "auth"
app/src/ai/acp_agent/` was empty. This run closes the half that needs no
credential: the refusal is now explained, in the agent's own words, with the
agent's own list of ways in.

**What is deliberately not built**: Warp does not send `authenticate` and does
not read `WARP_FORK_ACP_AUTH`. The sentence *names* that variable as one that
does not exist yet, which is a different thing from parsing one that does
nothing.

---

## 1. The refusals, re-measured rather than trusted

`probe.sh` — `warpctrl acp probe` against both agents, output in
`codex.jsonl` / `gemini.jsonl` and their `.err` siblings.

| agent | `initialize` | `session/new` |
|---|---|---|
| `@zed-industries/codex-acp@0.16.0` | answered, 3 `authMethods` | **refused**, `Authentication required` |
| `@google/gemini-cli@0.58.0 --acp` | answered, 4 `authMethods` | **refused**, `Gemini API key is missing or not configured.` |

Identical to 2026-09-07. The re-run cost four minutes and is the reason this
run does not open with a three-day-old claim.

## 2. The fact the survey did not have: both send `-32000`

`rawprobe.py` speaks JSON-RPC over stdio directly, because **`warpctrl acp
probe` prints an ACP `Error`'s `Display`, which is its `message` alone and
drops the `code`.**

```
codex  -> {"error":{"code":-32000,"message":"Authentication required"}}
gemini -> {"error":{"code":-32000,"message":"Gemini API key is missing or not configured."}}
```

`-32000` is `ErrorCode::AuthRequired`. Gemini's *message* is its own prose and
its *code* is the protocol's — which is what makes a code test work across
agents where a message test could not.

## 3. The control, and why the obvious rule is the wrong one

`rawprobe2.py` — `claude-agent-acp@0.73.0`, signed in, `session/new` with a
directory that does not exist:

```
authMethods: []
error:       {"code":-32602,"message":"Invalid params: `cwd` does not exist on the
              machine running the agent: /home/effatha/definitely-not-a-real-directory-xyz"}
```

The handoff proposed keying the disclosure on *"the agent advertised
`authMethods` and then refused a session"*. On this machine that rule gives the
right answer three times out of three, and it is still wrong, because
**`initialize` is answered before the agent has seen the request**: Codex names
all three of its methods whether or not a key is set.

So the first `session/new` failure after a credential arrives on this machine
is the one `CLAUDE.md` has recorded since 2026-09-02 — a WSL pane's Unix cwd
handed to a Windows-side agent — and the advertised-list rule answers it with
*"sign in"*, sending someone to a login flow that is already complete. The code
is the agent's verdict about *this* request; the list is a menu.

`claude-agent-acp`'s empty list is what makes the list rule *look* safe today.
Both halves of that sentence are load-bearing, which is why the control is kept.

## 4. What shipped

`app/src/ai/acp_agent/auth.rs`, plus a `match` at the `session/new` call site
where there was a `?`. Three outcomes:

| the agent said | what a person sees |
|---|---|
| `-32000` and named ways in | its own message, its own list (id, name, description), and that there is no `WARP_FORK_ACP_AUTH` yet |
| `-32000` and named none | its own message, and that it named no way to sign in — no heading over an empty list |
| anything else | unchanged: handed back to `spawn_failure_or`, which owns the non-credential failures and is what names the WSL boundary |

The event log gets one `session_auth_required` line with
`error_type: auth_required`, written **before** the return, for the reason
`mode::Decision` logs before its own refusal: the turn that did not run is the
one a later reader is asking about. Without it the log would end on
`session_agent` and say nothing about why.

## 5. Calibration — `calibration.txt`

Three deliberate breaks, each run, each reverted.

| break | reddened |
|---|---|
| 1 · key on the advertised list instead of the error code | `a_refusal_that_is_not_about_a_credential_gets_no_credential_advice`, `the_measured_control_is_left_to_the_explainer_that_owns_it` |
| 2 · the second arm returns the first arm's text | `an_agent_that_names_no_way_to_sign_in_says_that_instead_of_listing_nothing` |
| 3 · stop naming `WARP_FORK_ACP_AUTH` | `a_credential_refusal_quotes_the_agent_and_lists_what_it_named` |

**Break 2 is the one worth reading.** A test named
`the_two_credential_arms_are_distinguishable` **passed it** — it asserted only
that the two arms produce different strings, and they do, because their method
lists differ, whether or not the second arm is real. It was removed rather than
repaired: the property is already held by the test that checks there is no list
heading with nothing under it, and one working test beats two where one is
decoration. The argument is in `auth_tests.rs`'s own header.

That is the handoff's *"calibrated by making them fail, not by watching them
pass"* earning its keep on its first use in this run.

## 6. Two stale doc comments in the file being edited

Found by asking `CLAUDE.md`'s question of `acp_agent/mod.rs` before calling
anything done. Both were in the module header's *"Also not done"* section, both
were true when written, and both were falsified by code added to this same
module:

- *"A second turn is refused"* — T14.7 gave agents that declare `loadSession` a
  `session/load`. `cannot_resume`'s own doc records that correction **twenty
  lines below**, and the header did not. The file contradicted itself.
- *"model selection … falls through untouched"* — T14.14 built the model chip on
  2026-09-07, three days before this was read.

Corrected in place rather than deleted, because what they used to say is the
design that was tried. That is fifteen and sixteen for `CLAUDE.md`'s table.

## What is left of item 5

Half 2: sending `authenticate` with a chosen method id. Still the maintainer's,
still unbuilt, and still untestable here — there is no credential on this
machine, so its success criterion would be *"it compiles"*.
