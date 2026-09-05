# Self-hosting run — 2026-09-01 evening

The fork, run from a binary the fork built, doing fork work. Launched
`18:51`, binary rebuilt at run start (`18:51:25`) so it carries the two
post-horizon commits — the mode note's de-hedge and the `describe_turn_start`
anchor from the review.

Instruments: `WARP_FORK_ACP_MODE=default`, event log and transcript on,
`claude-agent-acp` 0.70.0 (same build every prior measurement used).

---

## The loop closed, and it is legible in the first exchange

The first agent turn's own preamble, quoted from `agent read`:

> This turn began with the session in the agent's `auto` mode, which the agent
> describes as "Use a model classifier to approve/deny permission prompts".
> `WARP_FORK_ACP_MODE` asked for the agent's `default` mode, which the agent
> describes as "Standard behavior, prompts for dangerous operations", and the
> agent accepted, so that is the mode this session is running under.

Both halves of last night's fix are in that sentence: the anchor is
`This turn began with`, not `This session opened in`, and the note reports the
answer instead of hedging about it. Written last night, compiled this evening,
read back out of the running product.

## Finding 1 — `PAIRABLE_ACTIONS` is pinned twice, and CLAUDE.md said it was pinned none

Filed last night as a ticket-shaped gap: *"a list that decides what a weak
credential may reach is the wrong one to leave unpinned"*. It is not unpinned.

`a_paired_device_gets_the_read_surface_and_the_safe_half_of_answering` asserts
the **whole list** against a literal slice — membership, not a count, so it is
strictly stronger than the catalog pin that CLAUDE.md holds up as the good
example two paragraphs earlier.

Calibrated rather than read: adding `ActionKind::AgentApprove` to the list
reddens **two** tests, not one. The second is
`saying_yes_does_not_travel_by_default_and_saying_no_does`, which holds the
consent asymmetry — so a widening that somehow slipped past the membership
assert still hits a guardrail aimed squarely at that widening.

**What actually went stale was the count in CLAUDE.md, and no test can pin
prose.** The observation was real; the mechanism invented under it was not. Same
shape as the discovery-record retraction already in the file, and the second
time in two days that the fork's own docs were the most dangerous input
available.

Cost of finding out: one `sed`. Cost of not finding out: a panel task building a
test that already existed.

## Finding 2 — the pane-cwd rule is backwards, and I nearly published the wrong correction

CLAUDE.md: *"an agent in the panel works in the pane's directory, and a fresh
pane starts in `$HOME`. Not in the directory Warp was launched from."*

First clause right. Second backwards.

| Warp's process cwd | pane origin | pane shell | agent's `pwd` |
|---|---|---|---|
| the repo | new tab | the repo | **the repo** |
| `$HOME` | restored from last launch | the repo | **the repo** |
| `$HOME` | scratch profile, nothing to restore | `$HOME` | **`$HOME`** |

Row 2 separates the candidates — Warp's cwd and the pane's cwd disagree and the
agent follows the pane. Row 3 is the only `$HOME` case, and it is `$HOME`
because Warp was *launched* there. **Restored panes keeping their previous
directory across launches is undocumented** and is what the original sentence
was really watching.

The remedy is unchanged; its reason is stronger. `cd` first not because there is
a `$HOME` default, but because the pane's directory has two sources nobody
chose — where Warp happened to be launched, and where that pane pointed days
ago. Restore is the nastier: it survives a reboot and is invisible in the launch
command.

### Two ways I got this wrong before I got it right

**I measured the wrong quantity.** I read the shell's cwd out of
`/proc/<pid>/cwd` and was one edit away from publishing a correction on it. The
claim is about what the *agent* sees. The two coincide often enough to look like
confirmation. What caught it was a contradiction I could not explain away: if
shell cwd were the answer, T14.7 could not have happened, because that run's
shell was in the repo too.

**Then I invented a mechanism.** Reading the code gave
`working_directory` → `None` → `std::env::current_dir()`, so I predicted a
`$HOME`-launched Warp would put the agent in `$HOME`. Ran it: the agent said
*the repo*. The prediction was wrong because the pane was **restored**, not
fresh, and had a reported cwd all along. The read was accurate about a path that
was not the path being taken.

Both are the same failure the fork keeps cataloguing, an hour apart, in someone
who had just written up an instance of it.

---

## Setup notes worth keeping

- `warpctrl pane inspect` does **not** report a cwd; neither does `pane list`.
  There is no control-plane read for the quantity this whole finding is about.
- `agent read` takes the conversation id **positionally**; `--conversation` is
  `agent prompt`'s flag and `agent read --conversation <id>` exits with
  `unexpected argument`.
- `pane split` requires `--direction`.
- `window close` returned `close: "requested"` with the `verify` sentence on
  every one of three shutdowns, and all three exited inside ~2s. T14.10's
  refusal to claim an unobserved effect, working.
- A scratch profile needs `XDG_CONFIG_HOME/warp-oss/user_preferences.json` with
  `{"prefs": {"HasCompletedOnboarding": "true"}}` — directory name and the
  nesting under `prefs` both load-bearing, both correct in the docs, and
  `has_workspace: true` on first launch is how you know they took.

## Unknown 4 — not advanced

Three conversations, one turn each, all single-exchange probes. Nothing near
compaction. The instrumented run for unknown 4 has not started; this session was
setup plus two doc corrections earned on the way in.
