> The idea board's preamble and its selection, split out of `.fork/IDEAS.md` on 2026-09-05. History; read the section you came for.

# Fork idea board

Captured 2026-08-21 from a spoken-shape brain dump. **Nothing here is a
commitment.** `.fork/tickets/` is the board of work that has been agreed; this is the
holding pen in front of it, and the point of the pen is that an idea has to earn
its way out. `CLAUDE.md` has the method these entries are graded by.

The bar for leaving: someone has found the *smallest version of the idea that is
still the idea*. Not a cut-down version — the same idea, built out of parts that
already exist.

The standard is `crates/warp_cli/src/local_control/graph.rs`. A run-scale task
graph that added **zero new app surface**: the action count is identical before
and after T7.1, because a graph turned out to be a TOML file and a `while` loop
over verbs T6.6 had already built. Ideas below are graded partly on how close
they can get to that.

## A warning about everything below

Every "this already exists" claim in this file is a `file:line` I read on
2026-08-21. **None of it has been verified by running.** That is not this fork's
standard — T1.7 documented 88 actions by executing all 88 and found three
documented facts wrong; T5.6 found a "mystery cancellation" was a person with a
mouse; T1.11 exists because reading was not enough. Twice now, reading has
produced a confident wrong answer here.

So: read these as *"the code says"*, and note that the first step of every
scoped item is to run the thing and find out what the code left out.

**Three times now.** I8 said, twice, that the visor was `PanesLayout::
AmbientAgent` plus a match arm. `AmbientAgent` is the *cloud* agent setup tab —
following it would have wired the fork's hotkey to the account-gated path.
Retracted inline where it appears. The variant *name* was read; the arm it
resolves to was not.

## How these were graded

1. **Does the mechanism already exist?** The fork's most repeated finding is
   that it does, and is gated, unreachable, or wired to the wrong entry point.
2. **How much new surface?** New panes, settings, dependencies, protocol
   messages, actions. Fewer is better, and zero is the target.
3. **Does it serve the thesis?** No telemetry, no account, your agents on your
   keys. An idea can be good and still not be this project's.

---

# The selection

Five, in the order I would do them. Reasoning in each entry; the full list is
below and includes the ones I am arguing *against*.

| | Idea | Why it is first | Rough size |
|---|---|---|---|
| **1** | [Quake visor for the lead agent](I08-visor.md) | Both halves already exist and have never been pointed at each other | A day, if the verification passes — **on Windows**; see the Wayland catch |
| **2** | [Tab → pane drag, with a drop target you can see](I03-panes-flexible-illegible.md) | Quadrant splitting is *implemented*; it is driven from the wrong handle and shows you nothing | Two or three days |
| **3** | [The thread inbox, and `settled`](I01-inbox.md) | The list, the model and a persisted per-conversation flag all exist | A week |
| **4** | [Pin what a tool claims to be](I11-pin-tool-claims.md) | Small, on-thesis, and defends against an attack that is live right now | Two days |
| **5** | [A main pane, and the CWD following it](I13-main-pane-in-group.md) | One `Option<PaneId>` with three consumers; fixes the thrash that made plain CWD-follow wrong | A day, plus I6 |

And one added 2026-08-22 that outranks all five on impact, at a larger size:

| | Idea | Why | Rough size |
|---|---|---|---|
| **★** | [WSL as a remote target, the way Zed does it](I16-wsl-remote-target.md) | The seven-method transport trait exists with one implementation, the server binary builds here, and **the handshake is not account-gated — verified by completing one, logged out** | Weeks, not months |

And one captured 2026-08-28, unscoped and unverified:

| | Idea | Why | Rough size |
|---|---|---|---|
| **+** | [OpenRouter as a first-class provider, and a place to keep secrets](I22-openrouter-provider.md) | One credential reaches ~356 models across a dozen vendors — the widest version of "the user's own keys" — and it is already the key in use here | Unknown; **the gate has not been looked for**, and the secrets half needs a threat model before an implementation |

Two that are **deliberately not on the list yet**, both for stated reasons
rather than by omission:

* [Context pruning](I09-context-is-yours.md) — because the honest
  first step is measurement, not construction. That entry also answers your
  caching question.
* [Computer use](I15-computer-use.md) — found while
  answering the browser question, and the strongest unselected item here. A
  complete screenshot/input/recording stack sitting behind the same dogfood
  flag `WarpControlCli` was behind. Wants its own scope and one keyboard
  question answered first.

---

