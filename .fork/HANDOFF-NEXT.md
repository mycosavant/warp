# Handoff: where this fork stands, 2026-09-12

Written at the end of the session that ran `HANDOFF-DECIDE.md` to completion.
**That run is finished — 7 of 7 — and this file replaces it.** Retire this one
the same way when its own list is empty.

---

## Read these first, in this order

1. **`CLAUDE.md`** — and its new top section, *How to write in this file*. It is
   the index; accounts live in `.fork/docs/` and `.fork/runs/`. The file is
   **over its character budget** and every addition needs a removal to pay for
   it. The retraction convention is stated there and this project depends on it.
2. **`.fork/GOAL.md`** — the standing horizon, *live in the build*. It outranks
   ticket ordering while it stands.
3. **`.fork/next.html`** — the board, with status chips. Open it in a browser.
4. **`.fork/runs/run-live-2026-09/friction.md`** — two tables: the maintainer's
   and the agent's. **Add to the agent's one as a habit, not as a task.**

## The one thing to understand about this project before touching it

**The maintainer works from a phone, on the road, over mosh+tmux on a tailnet,
every day.** That is not a side channel; 39 of the 119 commits since 2026-09-04
name the phone or a remote shell as the working surface. Two consequences you
will otherwise get wrong:

- **A rebuild or relaunch takes the surface away.** Anything that needs the GUI
  up is out of reach mid-rebuild; anything that reads files is not. This is why
  `warpctrl agent trace` explicitly needs no running Warp.
- **A thin log is not a quiet week.** This session read a one-line friction table
  as evidence that little was happening and was flatly wrong. The maintainer
  fixes things in situ; the filing is *ours*. Log what you found, and never
  infer their activity from our records.

## State of the machine, end of session

Nothing running — no `rustc`, `rust-analyzer`, `warp-oss`, `warp-tui-oss`,
`llama-server`, `codex-acp`, `cargo`. Discovery dir empty. Host **16.1 GB of
63.8**; guest 38 GB free. Working tree clean. **213 commits unpushed**, which is
normal and deliberate — see constraints.

---

## What is actually open

### 1 · `authenticate` half 2 — smaller than the board says (T21.3d, board item 5)

Half 1 shipped 2026-09-10 (`auth.rs`): the refusal is disclosed in the agent's
own words, keyed on the **error code** (`-32000`) rather than the advertised
method list.

**Half 2 partly dissolved on 2026-09-11.** Point codex-acp at a `model_provider`
with an `env_key` and `session/new` is never refused — it runs a full three-tool
turn on the maintainer's OpenRouter key and **no `authenticate` is ever sent**.
So `authenticate` is the door for an agent whose *only* way in is a vendor login.
**Gemini is untested and must not be assumed to match.** Evidence:
`.fork/runs/codex-wire-2026-09-11/`, `.fork/runs/auth-2026-09-10/`.

### 2 · The routed-WSL commit message — **built 2026-09-12**

The daemon inside the distribution generated the commit message with its own
client, where the fork's config is not installed — so on the *recommended*
configuration this one feature was dead. ~~**A protocol addition**, so not an
evening.~~ It was **one request field**: `return_diff_only` on
`GitGenerateCommitMessageRequest`. The proto is in-tree
(`crates/remote_server/proto/`), so there is no upstream dependency and no
generated code to check in, and the daemon already computed the diff beside
the files — the only half it could not do was call a model it has no
configuration for.

**Verified live 2026-09-12**, `.fork/runs/routedcommit-2026-09-12/`: routed
pane (`"where": "host"`), dialog fills in, model requests 3 → 4, and the
message names a function added minutes before the run, so the diff travelled
rather than the sentence merely reading well. Unrouted re-run as a control,
because the fix refactored a function the local path shares.

**The two PR-content paths had the same defect and were fixed the same day**:
`title`/`body`/`return_inputs_only`
on `GitCreatePrRequest`, `return_pr_inputs` on `GitCommitChainRequest`. They
hid five days longer than the commit message because `create_pr` falls back to
`gh pr create --fill` — a fallback is not a fix, it is a defect that stopped
reporting itself. **Not driven live**: doing so means opening a real pull
request, which the standing constraints forbid. Pinned instead by
`pr_create_args` tests and calibrated source pins.

### 3 · kode-rs framing (board item 8) — parked, the maintainer's call

Their context, recorded 2026-09-11: a swarm of agents built a lot in a
misconfigured `/loop`, and parts were lost when worktrees were torn down
unmerged. There is a "built-not-wired" document and a trail of notes, none
tested lately. Main branch there is **`dev`**. **Read the built-not-wired doc
against the tree before any build** — a build that succeeds against a
partly-merged swarm tree proves less than it looks like.

### 4 · CLAUDE.md reduction — measured, not started

`Working rules` is **95,119 characters, 62% of the file**. That is where a real
reduction lives. The obvious first move is a `.fork/docs/build.md`: the `-j` cap
history, `memsample`, `[profile.release.package.warp]`, host-versus-guest memory
— all account, all currently inline. Leave the rules, cite the page. **Do not do
this hastily**; the whole project reads that file.

### 5 · Voice — **the maintainer is working this; do not touch it**

They asked explicitly to be left to it and will report back. `.fork/docs/voice.md`
is the fork's index of both halves. The fork-side bridge (`agent trace` from a
phone-local shell) is **deliberately unbuilt** and must not be started before
pocket-tts PRs #7 and #8 are confirmed on a device.

### 6 · Never verified, from earlier work

- **End-to-end park-and-approve inside a TUI session** — a real ACP permission
  request parked in the TUI and answered from another shell. The channel is
  proved (`f2e1558c6`); that loop is not.
- **Codex's telemetry claim rests on a polling census.** `.fork/runs/codex-wire-2026-09-11/`
  states its own limit: a connection inside one poll interval would be missed.
  The static read and the negative control agree with it, which is why it stands.

---

## Standing constraints — all still in force

- **No push, no PR, no upstream merge without an explicit say-so.** 213 commits
  sit unpushed by design.
- **Permission posture is frozen.** Do not measure it further.
- **Never build on both sides of the VM at once.** This is now the *only* build
  rule — the `-j` cap was lifted 2026-09-11 after a clean uncapped run, because
  `-j` was never what stood between the build and the wall. `CARGO_BUILD_JOBS`
  remains the opt-in knob for a remote session or a busy desk.
- **Never `wsl --manage --move` this distro.**
- **Leave the maintainer's config alone** — `settings.toml`, and `~/.codex`,
  which still has no `config.toml`. Use `XDG_CONFIG_HOME`/`XDG_STATE_HOME` or
  `CODEX_HOME` scratch dirs.
- **Leave no Warp or agent processes running**, except one working instance if
  the maintainer wants it — and say which.
- **Credentials: print provider and field *names*, never values.**

## Traps this session walked into, newest first

Each is in `CLAUDE.md` already. They were walked into anyway, which is the point.

| trap | what it cost |
|---|---|
| **Concluding from one tree's silence.** Grepped this repo for `text-to-speech`, got 0, filed it as unexplored. It is a built app with a device test and two open PRs, in another repo and in artifacts. | a wrong board item, twice |
| **Reading a thin log as evidence about a person.** See above. | a false claim about the whole week |
| **A narrow stand-in that *fails* where the real client works.** PowerShell said symlinks needed admin under Developer Mode; Git for Windows creates them fine. | nearly abandoned a fix that worked |
| **Reading HEAD when a pin is what runs.** codex-acp vendors `rust-v0.137.0`; a config block read from HEAD was refused by the binary. | one failed probe |
| **Editing a board and leaving it stale.** The T18 row still said "the maintainer picks" after I landed it in `2a18a2dc8`. | caught in this handoff |

## Gates, before anything is called done

```
cargo check --workspace --all-targets     # catches what --bin never compiles
./script/format                           # then check git status and revert drive-bys
cargo test -p warp --lib fork::           # 48 fork tests at last count
node --check app/src/local_control/console.js   # if the console was touched
```

**Adding a `warpctrl` action?** Two pinned counts in two crates — grep
`fn catalog_has_exactly`, never paste the number. **Diff test-failure
membership, not counts**: the flaky set here is nine wide.

## What "done" looks like for a finding

Landed as a **rule** in `CLAUDE.md` (few lines), its **account** on a
`.fork/docs/` page or in a `.fork/runs/` record, a line in the agent's friction
table, and a commit whose body says *what was found* — including what it
contradicts. **Say what you did not establish.** A finding with no stated limit
reads stronger than it is.
