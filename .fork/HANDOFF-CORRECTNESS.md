# Handoff: four defects, each with a user-visible wrong answer

Written 2026-09-13, approved by the maintainer the same evening. **Start after
`HANDOFF-MERGE.md` has landed**: the merge may move `acp_agent`, `code_review`
and the repo-metadata code these touch. Each task is independent; do them in
the order below, which is value over effort. Retire this file when all four
are done or deliberately closed.

## Read these first

1. `CLAUDE.md`, *Method: run it*. Two of the four below were reasoned from
   code and never run. Run each before fixing it.
2. `.fork/docs/composer.md` (what the panel draws) and
   `.fork/docs/agent-transports.md` (a fact about the agent vs a fact about
   the fork).

---

## Task 1: the panel says the agent changed mode when it only echoed Warp

`app/src/ai/acp_agent/translate.rs:485` (line as of 2026-09-13) calls
`mode::changed` for every `CurrentModeUpdate`. When Warp sends `set_mode`
(`WARP_FORK_ACP_MODE`, re-sent every turn) and the agent answers with a
`CurrentModeUpdate` naming the mode Warp asked for, the disclosure reports it
as a change the agent made on its own.

**Why it matters:** the mode notice is a consent disclosure. A false "changed
on its own" teaches the reader to ignore the one notice that is meant to catch
a real change.

**Not established:** that any agent actually sends the echo. Neither measured
agent has triggered it. So first run `warpctrl acp probe` against
`claude-agent-acp@0.73.0` with a mode set and read the wire for a
`current_mode_update` after `set_mode`. If none arrives, write a test with a
synthetic echo anyway, fix it, and record that no known agent sends it.

**Fix shape:** compare against the mode Warp requested (the session already
records `known.current` or the requested id) and say nothing, or say
"confirmed", when they match. **The permission posture is frozen**; this
changes what is said about a mode, not what is permitted, but name that in
the commit body so a reader does not have to check.

## Task 2: an agent that dies during `initialize` leaves no reason

Friction a1. When the agent process exits before answering `initialize` (bad
command, missing node, wrong cwd side of WSL), the panel shows a closed
transport and nothing the agent printed. `grep -n stderr app/src/ai/acp_agent/`
is empty.

**Done means:** the spawn captures stderr (bounded, the last N lines), and a
transport closed before `initialize` completes quotes it in the panel. Verify
with three real failures on Windows: a misspelled command, `wsl.exe -d
NoSuchDistro`, and a WSL cwd handed to a Windows-side agent (the refusal quoted
in `CLAUDE.md` under the WSL agent note). The auth disclosure in `auth.rs`
(T21) quotes agent error text already; match its shape.

**Watch for:** stderr from a live agent is not an error channel for every
agent, so do not surface it on a healthy session. Only on early close.

## Task 3: a chain that fails at `gh` loses the PR title and body it generated

`.fork/runs/pockettts-pr-2026-09-12/README.md:109`. `Commit and create PR`
generates a title and body, then fails at `gh pr create`, and the generated
content is not shown or kept anywhere, so the person has to write it again.
`76d8b07e6` fixed only the stage label.

**Done means:** on a failure after generation, the dialog shows the generated
title and body (copyable), or keeps them for the retry. Reproduce against a
scratch repo with no GitHub remote first; that failure is cheap and reaches
exactly this path (friction a33's "No GitHub remote" arm).

## Task 4: `@` offers an empty Code section in a routed WSL repo

T16 recorded that `RepoOutlines` and `file_mcp_watcher` never see a routed
repository, so the `@` menu offers a Code section with nothing in it. **Not
re-run since 2026-09-02**, and indexing now defaults off, which may hide it or
change it. So:

1. Re-run on Windows with a routed WSL pane (`warpctrl session inspect` says
   `host`), indexing on and off, and photograph the `@` menu.
2. If it still shows an empty section: the smaller fix is to not offer the
   section when there are no outlines for that repo; the larger is routing
   outline building through the remote server. Take the smaller one, and file
   the larger in T16 with what the run showed.
3. If it does not reproduce, strike the T16 claim with the run as evidence.

---

## Commits

`fork: <subject> (T14)` for tasks 1-2, `(COMPOSER)` for 3, `(T16)` for 4. Run
records under `.fork/runs/<name>-<date>/` for any task driven live.
