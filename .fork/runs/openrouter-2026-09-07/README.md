# The picker against opencode's list, 2026-09-07 (T21.3b, I22)

`run.sh`, binary `v0.fork.21b8d2341`, the rig profile with
`warpdev.ps1 -Agent` naming `opencode acp` inside the distribution. The
picker had been measured against five rows from one vendor; opencode on
this machine holds an OpenRouter credential and advertises hundreds.

| | seen |
|---|---|
| attempt 1, `or-0-attempt-1-*.png` | the turn failed at `initialize`, *Incoming transport closed*, retried three times by upstream's recovery and given up. Cause: `wsl.exe --shell-type login -- opencode` finds no `opencode`, because `.bashrc` returns for a non-interactive shell before it loads nvm. The absolute path fixed it; nothing in the panel said why |
| first turn, `or-1-after-turn.png` | chip *OpenCode Zen/Big Pickle*, the agent's own default; a *Thinking* block above the answer, opencode's model sends thought chunks and the panel has a home for them |
| the chip clicked, `or-2-picker-open.png` | **365 rows** (358 `OpenRouter/…`, 7 `OpenCode Zen/…`), the selected row highlighted, the card `?` on all three bars under the fork's header with no sentence and no price, which is what an id the table does not know is meant to draw |
| the event log, first turn | `requested sonnet, not offered`: the pick from the previous agent's list was not sent, and the agent's own model ran |
| typing `sonnet`, `or-3-search-sonnet.png`, `or-3b-click-then-type.png` | **nothing reached the search box**, with or without a click into it first. `keys.ps1 -Text` posts characters and the input does not take them; the search box is unmeasured, not broken |
| Home, then Down four times, `or-3c-home-down4.png` | Home did nothing; Down moved the highlight four rows past the selected one, to *OpenCode Zen/Muse Spark 1.3 Free* |
| the row clicked, second turn, `or-5-after-second-turn.png` | event log `requested opencode/muse-spark-1.3-contributor-free, sent`; chip *OpenCode Zen/Muse Spark 1.3 Free*; the answer `opencode/muse-spark-1.3-contributor-free`; opencode's own `opencode.db` records the assistant message with `providerID opencode`, `modelID muse-spark-1.3-contributor-free`, after two on `big-pickle` |
| `acp-models.json` | 231 KB, 365 rows, `default_id` the picked model |

So the picker is plug and play across the two agents that speak
`configOptions`, at 365 rows as at five: the list, the pick, the per-turn
send and the store all held. What it costs at this size is the specs card,
which cannot be hand-covered and draws `?` everywhere; that is the
opt-in fetch on `.fork/next.html`, item 4.

Two things about the store worth a look later, not measured here: whether
opencode reorders its list between launches (a reordered list is a changed
list to `publish`, and a 231 KB rewrite per turn would be worth a sort), and
the *NEW* pill, which fires once per change of list and so once per agent
switch.
