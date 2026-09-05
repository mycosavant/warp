> Idea I22, split out of `.fork/IDEAS.md` on 2026-09-05. History; read the section you came for.

# I22 — OpenRouter as a first-class provider, and a place to keep secrets

**Renumbered from I18 on 2026-09-01.** Two entries carried that number: this one,
added 2026-08-28, and the persistent grant, added 2026-08-30. Every other
reference in the repo — `GOAL.md`, both friction logs, `.fork/tickets/`, `CLAUDE.md`
and `acp_permission.rs`'s module docs — means the *persistent grant*, so the
number stayed with the entry that is load-bearing in prose and in code, and this
one moved despite being older. Chronologically odd, cheapest by a factor of ten,
and recorded here rather than silently corrected.

Captured 2026-08-28, from the maintainer, while probing `opencode` over
OpenRouter for T14.5. **Unverified against the code — nothing below has been
grepped, let alone run.** It is written down so it is not lost, not because it
has been scoped.

## The ask, as given

Full OpenRouter provider integration alongside the fork's existing providers,
with:

- **dynamic model availability** — the catalogue is fetched, not compiled in;
- **pricing** — per-model, shown where a model is chosen;
- **service tiers**;
- and, **optionally for the user**, full API credential and secrets management,
  **encrypted at rest**.

## Why it is on-thesis, and where it is in tension

Squarely on-thesis for the first half. The fork exists so agents run on *the
user's own* subscription, API keys and local models; OpenRouter is one key that
reaches ~356 models across a dozen vendors, which is the widest possible version
of that with the fewest credentials. It also turns out to be the credential
already in use here — the `opencode` probe that discharged T14.5's gate ran on
it, and the model catalogue arrived over the wire in the ACP `session/new`
response as `configOptions`, 356 entries deep.

The tension is the second half. **A secrets store is the one thing in this fork
that would hold something worth stealing**, and the fork's own security posture
has to be re-argued around it rather than inherited:

- `warpctrl` already has `auth.rs`, a credential broker and a peer-UID check;
  upstream's 9282 server answers **unauthenticated** and must never see this.
- `WARP_FORK_CONTROL_BIND` is the one variable that reaches off the machine, and
  the console is the only browser-reachable surface. Neither may become a path
  to a key. The console's *"the page is a constant and names no secret"* test
  is the shape to extend, not to weaken.
- "Encrypted at rest" needs a named threat model before an implementation.
  Encrypted against whom, with a key kept where? A key sitting beside the
  ciphertext under the same UID buys very little against the attacker who has
  the UID, and buys real protection against a synced dotfile, a backup, or a
  shared `$HOME`. Say which, or the feature reads as a guarantee it does not
  have — `local_agent/tools.rs:17-20`'s stated nightmare, one layer down.

## Look for the gate first

Not done. Before any of this is scoped, the standard question: upstream is a
commercial AI terminal and **already has a provider abstraction, a model picker
and a credential path** — probably account-gated, which is exactly the shape
T4 and I16 turned out to have. Grep for the provider enum and the secret store
before writing a line. `warpctrl secret` already exists in the CLI surface,
which is itself evidence the storage half may be mostly built.

## The smallest version that is still the idea

Guessing, and marked as such: a provider entry plus a catalogue fetch, with
pricing rendered from the response rather than a table this fork maintains.
Secrets management is a **separate** entry and should not ride along — it is the
part with the threat model, and bundling it means the cheap half waits for the
argument.

---

