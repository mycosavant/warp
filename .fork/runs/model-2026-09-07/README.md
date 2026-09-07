# The model picker and the notification's body, on the Windows build

2026-09-07, from two things the maintainer noticed walking the checklist on
a real Android phone with Firefox: the panel's agent was on Fable with no
apparent way to change it (`/model` opened an empty picker under a chip
reading *auto (cost-efficient)*), and a notification about a finished turn
read *the turn ended* under the conversation's first prompt. Both built the
same morning (`952d80a08`), then measured here on the Windows release builds
named in `driver.log`, the rig profile (`warpdev.ps1 -Instrumented -Console
-Bind 192.168.254.3:41234`), `claude-agent-acp` 0.73.0 inside Ubuntu.
`model-run.sh` is the driver, in parts, because the picker is clicked at
coordinates read off screenshots between steps.

## The picker, `952d80a08`

| step | seen |
|---|---|
| the chip 2 s into the first turn (`desk-1-during-first-turn.png`) | *Fable (Fable 5.1 · Most capable for your hardest and longest-running tasks)*: the agent's list, published from `session/new` before the answer |
| the chip clicked (`desk-3-picker-open.png`) | upstream's menu, *Base* tab, five rows: *Default (recommended)*, *Opus (1M context)*, *Fable (selected)*, *Sonnet*, *Haiku*. The agent's names and descriptions; *Full Terminal Use* stays empty |
| Sonnet clicked, *Which model are you? Reply with your model id only.* (`desk-5-after-second-turn.png`) | the chip *Sonnet (Sonnet 5 · Efficient for routine tasks)*; the answer `claude-sonnet-5`; `session_model · current claude-fable-5-1; requested sonnet, sent` |
| Claude Code's own transcript, the `model` field per assistant message | `claude-fable-5-1` at 15:38:52, `claude-sonnet-5` at 15:39:44 and 15:40:30 |
| the third turn's `session/load` reply | *current* `claude-fable-5-1[1m]`, with the harness running `claude-sonnet-5`: the resumed session reports one model until told again, which is why the pick is re-sent every turn, as the mode is |
| beside the chip afterwards | a *NEW Fable* pill: upstream's new-models announcement, fired by the list changing from the placeholder to the agent's |

## The notification, `952d80a08`

The emulator paired to the same conversation, the tab hidden behind
`about:blank`, a prompt asking for one sentence. Six seconds later
(`p2-shade.png`, `dumpsys notification`):

    android.title=Reply with exactly this sentence and nothing else: The quick brown fo…
    android.text=The quick brown fox jumps over the lazy dog.

The title is the turn's prompt, cut at 70 characters; the body is the
agent's last text after that prompt, read from the trace poll the stop
scheduled.

## The relaunch, and what it found

**`952d80a08`** (`desk-6-relaunched-before-any-turn.png`,
`desk-7-relaunched-after-a-turn.png`): after `window close` and a relaunch
the chip read *auto (cost-efficient)* again; a new conversation's first turn
ran on Fable with `session_model · current fable` and nothing requested; the
chip read *Sonnet* once that turn had published the list. The pick had
survived in the execution profile. The list had not:
`UserWorkspaces::workspaceless_models_by_feature` is an in-memory `Option`.
So `5c1a1c346` writes the list to `fork::state_dir()/acp-models.json` when
it changes and reads it back at startup.

**`5c1a1c346`**: the first launch wrote the store (3156 bytes,
`acp-models.json`), the relaunch **panicked at startup**: *Cannot get
singleton model of type AIExecutionProfilesModel that was never registered*.
The restore ran beside `LLMPreferences`, and the server-update path it goes
through reconciles the profile's pick, reaching a model registered later.
`58b449ccb` moves the install after that model and guards the write.

**`58b449ccb`** (`desk-11-b3-relaunch-before-turn.png`,
`desk-12-b3-relaunch-after-turn.png`): the store from the launch before on
disk, one launch. The chip read *Sonnet (Sonnet 5 · Efficient for routine
tasks)* before any turn, with no *NEW* pill; a new conversation's first turn
logged `session_model · current fable; requested sonnet, sent` and Claude
Code's transcript shows its one assistant message on `claude-sonnet-5`. So
the pick holds across a relaunch from the first turn, and the agent's own
default (Fable here) runs only on a profile that has never met the agent.

## Files

`model-run.sh`, `driver.log`, `build.txt`, `build2.txt`, `build3.txt`,
`launch*.txt`, `tab.json`, `prompt-*.json`, `pair-show.json`,
`conversation-{c,d,f}.txt`, `events-{c,d,f}.jsonl`, `trace-c.txt`,
`acp-models.json` (the store as written), `desk-1` to `desk-12`, `p1-paired.png`,
`p2-shade.png`.

## The label, later the same day (`8a6e64f81`)

`label-run.sh`, binary `v0.fork.8a6e64f81`. The chip had read *Fable (Fable
5.1 · Most capable for your hardest and longest-running tasks)* because the
agent's description went into `LLMInfo::description`, which upstream's chip
appends in parentheses. Now the label is the version before ` · `.

| | seen |
|---|---|
| before any turn, `label-1-before-any-turn.png` | a fresh tab; the previous build's store still held the old labels |
| after one turn, `label-2-after-turn.png` | chip *Sonnet 5* |
| the chip clicked, `label-3-picker-open.png` | *Default (recommended)*, *Opus 5 with 1M context*, *Fable 5.1*, *Sonnet 5 (selected)*, *Haiku 4.5*; the specs card still `?` on all three bars under upstream's "Warp's benchmarks" header, which is T21.2 |
| `acp-models.json` after the turn | the five new labels, `description` null on every row |

One trap: `model-run.sh`'s chip coordinate (70,872) hit the cwd chip on this
layout; the model chip was at (647,873). Read the coordinate off the
screenshot each time.


## The specs card, the same night (`5856078d9`, `21b8d2341`)

`specs-run.sh`. The wire carries no number, so the card's bars are a table
this fork keeps (`app/src/ai/acp_agent/specs.default.toml`, prices read off
the vendor's page on 2026-09-07), cost relative to the dearest model
offered, and the header says so.

| | seen |
|---|---|
| first build `v0.fork.5856078d9`, `specs-0-clipped-first-build.png` | header (three lines), the sentence, the price, the one word after every name; the Cost row clipped, because the inline menu's height does not follow its details pane |
| second build `v0.fork.21b8d2341`, `specs-2-picker-open.png` | header two lines, margins 8 and 6; Sonnet's card whole |
| `specs-3-{default,opus,fable,sonnet,haiku}.png` | each row's own card, reached with the arrow keys |
| `acp-models-specs.json` | the sentence as `description` and the three numbers as `spec` on every row |

Two instrument notes. A hover (`click.ps1 -Hover`, a `WM_MOUSEMOVE` with
no button) highlights the row and leaves the details pane on the selected
item, so the first run's five hover screenshots were all Sonnet's card;
the pane follows the keyboard, and `keys.ps1` knows the arrow keys now.
And the chip coordinate is still (647,873) on this layout.
