# The cancel path, measured (board item 4)

Windows debug build, scratch profile `wslauto`, `claude-agent-acp@0.73.0`
started inside WSL, `WARP_FORK_ACP_MODE=default`, event log and transcript
on. Recipe, driven with `warpctrl` (the `--pane` form resolves only in the
active tab, so the active pane is used):

1. `agent prompt 'Write the numbers one to five hundred in English words, one per line, with no tools and no commentary.'`
2. five seconds later, `input submit 'sleep 300'` -- a command the person
   typed into the pane behind the panel. It must come *after* the prompt:
   with a long-running command already in the pane, `agent prompt` refuses
   with *"the agent is monitoring a long-running command"* (run 2).
3. nine seconds later, `pgrep -x sleep`, a screenshot, `agent cancel`.
4. four seconds later, `pgrep -x sleep`, a screenshot, `agent list`,
   `agent read --last 1`.

Runs 1 and 2 measured nothing: the turn (one to eighty) finished before the
cancel, and the prompt was refused. Run 3 is the build before the fix, run 4
the build with it.

| | run 3, `03dd3c639` | run 4, `2ff77def8` |
|---|---|---|
| the person's `sleep 300` after `agent cancel` | killed; `^C` at the prompt | alive; pane untouched |
| the panel 14-16 s into a text-only turn | the prompt and Warp's two notes, nothing of the agent's | streaming, past "one hundred ninety" |
| `agent read` after the cancel | 586 chars: the two notes | 4055 chars, 215 lines, to "two hundred twelve" |
| `agent read` mid-stream | -- | 3960 chars, 210 lines |
| conversation status | `cancelled` | `cancelled` |
| event log after the cancel | `stop_failure`, `error_type: cancelled` | same |

Screenshots: `C:\dev\shots\cancel-run3-before.png`, `cancel-run3-after.png`,
`cancel-run4-before.png`, `cancel-run4-after.png`.

The app log's clock reads about four seconds behind the WSL shell's in these
files (`Canceling active stream` at 20:16:26Z against a cancel sent at
20:16:30 by `date -u`); read the two against each other, not against the
wall.
