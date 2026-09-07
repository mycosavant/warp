# The fork, mapped

Personal fork of `warpdotdev/warp`: no telemetry, no account requirement,
agents on the user's own Claude subscription, API keys and local models. The
method and the working rules are in `CLAUDE.md` at the repo root. This file is
the map of `.fork/`, one line per thing, laid out on 2026-09-05.

**Read `GOAL.md` first if it exists.** It is the standing horizon and it
outranks everything else here while it stands.

**`board.html` is the ranked board the horizon points at**: static,
self-contained, status chips remembered per browser. Open it, do not publish
it; the maintainer asks for a cloud copy when one is wanted.

**`next.html` is the horizon drafted beside it on 2026-09-07**: what the agent
works while the maintainer lives in the build (a local model in the panel, the
picker against a non-Anthropic list, the other agents from the wire, the
recurring housekeeping). Same rules: static, self-contained, a map not a
tracker; the boxes are ticked in `tickets/T21-the-agents-models.md`.

**`mobile.html` is the spec for the phone surface**, drafted 2026-09-06 in the
same shape: what `/remote-control` does today against Claude Code's, seven gaps
ranked, and four decisions marked as the maintainer's with a recommendation
each. A spec for discussion, not a record; `docs/remote-control.md` is what is
built.

**`HANDOFF-MOBILE.md` is the handoff for that work**, written 2026-09-06 for a
fresh session after the maintainer answered the spec's four decisions
(`decisions/2026-09-06-the-phone-surface-four-decisions.md`). Its steps 1
(the code half) to 6 were built and measured the same day; what remains is a
person with a phone, and the checklist at its top is that run. Live while the
work is open; it moves to `archive/` when the last step is measured.

## docs/ — one page per surface, current truth

Each opens with *as of <date>* and a state table, then how to use the thing,
where its code is, and what is open. Rewritten in place when a measurement
changes the answer; the story of the finding stays in the ticket it cites.

| page | surface |
|---|---|
| `docs/wsl.md` | a WSL pane as a remote session: what routes through the server, what still goes over 9p, what to build |
| `docs/composer.md` | the agent panel as drawn: Warp's notes, tool rows, the approval card, the measured chrome-to-words ratio |
| `docs/observability.md` | the viewer spec: the harness's own transcript is the record, Warp adds what it alone knows |
| `docs/remote-control.md` | `/remote-control` the fork's way: one conversation handed to a phone, which may drive it and nothing else |
| `docs/classifier.md` | a local permission classifier, and why "a model deciding is not consent" does two jobs |
| `docs/manual.md` | the operating manual, whole: building and launching on each platform, `warpctrl`, Warp Drive, remote access, the gotchas. Large; navigate by heading. To be cut into surface pages one at a time as each is next touched |

Pages not yet written, whose content is in the manual and the tickets for now:
the agent transports, `warpctrl`, the console, Drive, egress, the build, remote
access, upstream merging.

## tickets/ — the work, one file each, history

Split out of the old `TASKS.md` and `IDEAS.md` at their headings with no
sentence changed. A citation of the form `.fork/tickets/` followed by a ticket
number means the file whose name starts with that number.

| | ticket | state at split |
|---|---|---|
| `T00-the-board.md` | the board's preamble and the pre-board checklist | |
| `T01-warpctrl.md` | the local control plane | done |
| `T02-local-voice.md` | local voice transcription | |
| `T03-small-ai-features.md` | the four small AI features, locally | done |
| `T04-local-drive.md` | account-free Warp Drive, with the T4.4 git-sync scope and as-built | |
| `T05-claude-in-oz-seat.md` | the spike: Claude as the agent | |
| `T06-wsl-integration.md` | WSL integration, first pass | |
| `T07-work-that-outlives-a-turn.md` | the plan graph and the runner | |
| `T08-the-app-you-use.md` | visor, tab-to-pane drag, main pane | done 2026-08-23 |
| `T09-verifying-the-pixels.md` | screenshots and gestures an agent can drive | done 2026-08-24 |
| `T10-staying-current.md` | merging upstream | ongoing |
| `T11-observability-first.md` | the event taxonomy before the surface | done 2026-08-26 |
| `T12-the-console.md` | the browser-reachable surface | done 2026-08-27 |
| `T13-run-gate.md` | the run gate | active |
| `T14-acp-adapter.md` | ACP as the adapter contract; the largest ticket, 3,900 lines | built |
| `T15-loose-ends.md` | loose ends carried, not forgotten | |
| `T16-wsl-explorer-9p.md` | the WSL explorer walked over 9p; routing through the server | phases 1–3 built |
| `T17-lsp-for-agents.md` | is LSP for agents already built and switched off? | answered |
| `T18-wsl-pane-turns.md` | every turn in a WSL pane died before it began | fixed |
| `T19-agent-footer-chips.md` | the two cloud chips in the agent footer | closed 2026-09-06 |
| `T20-what-run-2-left.md` | what run 2 left | |
| `T21-the-agents-models.md` | the agent's models: labels, the specs card, which agents say any of it | filed 2026-09-07 |
| `open-questions.md` | the open-questions list from the old board | |

`I00-idea-board.md` through `I22-openrouter-provider.md` are the idea board:
each entry says what already exists under the idea and argues for or against
building it. Roughly half turned out to be already built. `I00` holds the
preamble and the selection.

## decisions/ — one paragraph each, dated, binding

The five from the old board's *Decisions on record*, split by bullet. Two are
undated in the source and say so in their filename. Later decisions live in
`GOAL.md` until it is retired, then here.

## runs/ — data, never edited after the run

| | |
|---|---|
| `runs/run-live-2026-09/friction.md` | **the live friction log**, the backlog under `GOAL.md` |
| `runs/run-2026-09-01/` | run 1: the consent path on the wire; events, transcripts, friction |
| `runs/run-2026-09-02/` | run 2: 44 approvals in 50 minutes, terminated |
| `runs/run-selfhost-2026-09-01/` | the self-hosting weekend |
| `runs/classifier/` | the permission-classifier eval set, probes and their README |
| `runs/viewer-phase0-2026-09-05/` | the viewer's phase 0: the join between Warp's log and the agent's session file, measured live under `auto` |
| `runs/viewer-phase3-2026-09-05/` | the viewer's phase 3: the record served to a paired device on the Windows build, the harness file found inside the distribution, the page photographed |
| `runs/remote-control-2026-09-05/` | `/remote-control` handed to a device: the scope, the confinement, two defects found and closed |
| `runs/handoff-chip-2026-09-06/` | the cloud handoff switched off, measured against a baseline taken by accident |
| `runs/tls-2026-09-06/` | the wide listener over TLS: the authority in the clear, the console over TLS, the block wrapped and stacked, Brave on three pages |
| `runs/pairing-2026-09-06/` | a control pairing with no clock, the block's three states, `via: paired_device` in the record and "from the phone" in the trace |
| `runs/page-2026-09-06/` | the console's header, diff and notify button on the Windows build; the h2 refusal found and closed; the chip flipping back on its own |
| `runs/model-2026-09-07/` | the panel's model chip picking the ACP agent's model, measured to Claude Code's own transcript; a stop notification carrying the agent's answer; the list lost on relaunch and kept on disk |
| `runs/localmodel-2026-09-07/` | a model on this machine: the hardware measured, the 2026 open-weight field against 9.6 GB of free VRAM, llama-server on the Windows side with Gemma 4 12B, and the four small features measured against it; three stale points from one upstream merge found and fixed on the way, and the commit message in a routed WSL pane found dead |
| `runs/wsl-move-2026-09-07/` | the WSL image was already on X: behind Store-relocation junctions and this session said it was on C:; the `wsl --manage --move` it suggested lifted the vhdx out of app-container EFS and WSL could not attach it. Written by the session that repaired it; `re-read.md` beside it checks two of its claims against the machine |
| `runs/phone-2026-09-06/` | the phone checklist on an Android emulator: the authority installed through Settings, the page paired over TLS, a notification through a worker, Yes and a prompt from the phone, the home-screen install; four defects found and fixed, and Android not running a backgrounded Chrome recorded as the push case |

## viewer/ — fixtures the trace is pinned against

`viewer/fixtures/` holds one real session file per harness version seen
(`claude-code-2.1.257.jsonl`) and the Warp event log it joins to, copied
unedited from the run that produced them. `crates/warp_cli/src/local_control/trace_tests.rs`
parses them; when Claude Code changes its file shape, the new version's file
goes here and the failing test is the calibration.

## tools/ — scripts

`build.sh`, `warpdev.ps1` (the Windows launcher; product by default, with the
event log since 2026-09-05; `-Instrumented` for the rig, `-Console` for the
wide listener so a phone can pair), `phone.sh` (the Android emulator on the
Windows side, driven from WSL; `manual.md`, "A phone that is not a phone"),
`drift-check.sh`, `memsample.sh`, three
measurement scripts, and `reorg-2026-09-05.py`, which produced this layout and
is kept as its record. `launch.sh` beside this file is the Linux daily driver.

## archive/ — finished, kept for the reasoning

`SPEC.md` (the original de-telemetry plan; its survey findings are still the
best account of why the fork is shaped this way), `CONSOLIDATION.md` (the
soft-fork decision and the corrected divergence measurement), the two handoff
notes, and the August friction logs.
