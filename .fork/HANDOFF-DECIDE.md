# Handoff: a decide run — seven open questions, at the desk, in one sitting

**Written 2026-09-11. Paste-target: start a new session and say *"read
`.fork/HANDOFF-DECIDE.md` and run it with me"*.**

Read `CLAUDE.md` first, as always.

---

## What this run is, and what it is not

**The agent-actionable backlog is empty.** Every board item is landed or is
waiting on a person. This run is that person at the desk, going through the
waiting list with the evidence in front of them.

**So the deliverable is decisions, not code** — except where a decision is
cheap enough to execute in the same sitting, which is true of at least two
below. Work each item in order, stop at the question, get the answer, then
either do it or record the refusal and move on. **Do not build past a
question.**

The maintainer is at the desk for this one, so everything is on the table,
including the run that carries real risk (item 2).

---

## Read these before the first question

| | why |
|---|---|
| `.fork/GOAL.md` | the standing horizon. Its board is 7/7; its central criterion is 1/7. §0 below |
| `.fork/next.html` | the next-horizon board, items 5-8 |
| `.fork/tickets/I20-tui-account-gated.md` | **read this in full before item 1.** It has already answered most of what item 1 would otherwise re-derive |
| `.fork/decisions/2026-09-09-the-j-cap-is-not-what-holds-the-line.md` | before item 2. The `-j` question is closed; item 2 is the one part that is not |

---

## §0 · The horizon's own scoreboard, which is the frame for everything else

`GOAL.md` asks for **seven working days living in the Windows build**, with a
friction log, one dated line per day.

| | |
|---|---|
| the board | **7 of 7 done** |
| the maintainer's friction table | **1 line.** 2026-09-05, the WSL diff panel |
| the agent's table, a separate section | 23 lines across 09-07, 09-09, 09-10 |

**The building has run ahead of the living.** That is not a reproach — the
agent's 23 lines are real findings — but the horizon's own criterion is about
days at the desk, and it has one. Worth naming out loud at the start of a
decide run, because several decisions below are really *"is this what the next
week is for?"*

**One input that has changed since the horizon was written**, from the
maintainer, 2026-09-11: mosh + tmux over the tailnet, reached from the phone,
**two days with zero issues**, and the desk locking after several hours breaks
nothing — screenshots included. Friction line a9 (*"sessions die when the link
drops"*) is superseded by that. The remote story is now better than the docs
say, and item 1 is the question of how much further it can go.

---

## Item 1 · The TUI as the mobile surface — and it is further along than anyone remembers

**Ask first: has this already been built?** It had. Twice the answer was in the
files and once the index gave the wrong one.

- **`warp_tui` was built and run on 2026-08-30**, `--features standalone`,
  9m18s at `-j 8`.
- **The account gate was found and lifted the same day**, `e4f52077a` (I20).
  `initial_login_phase` now consults
  `fork::account_gate_bypassed() && fork_agent_will_answer()`. Measured with it
  in place: **the TUI opens on "Not signed in", 22 skills discovered, a fork ACP
  agent answers, and a permission request parks correctly.**
- **`CLAUDE.md` said the opposite until 2026-09-11** — *"not a fork surface … the
  account-gate bypass simply absent there"* — on the strength of a grep over
  `crates/warp_tui/src/`. That library **depends on the app crate**
  (`warp = { workspace = true, features = ["tui"] }`), the binary's entry is
  `app/src/tui/mod.rs`, and `fork::` is in scope there and always was.
  Corrected in `d3e6b209c` and added to the stale-doc table as the seventeenth.

### So the real question is narrower than "can we have a TUI"

What is genuinely missing is **`warpctrl` inside the TUI process**, and I20
names both the cause and the cheap path:

- `LocalControlServer` is registered behind a `matches!` on `LaunchMode` with
  **no TUI arm** (`app/src/lib.rs`).
- `warp_control_cli` is **not among `crates/warp_tui`'s features**, which
  forward to the app crate — `voice_input = ["warp/voice_input"]` is the shape
  to copy.

Consequence today: a permission request in the TUI parks correctly and then
names a command that cannot exist in that process. `fork::local_control_serving()`
already makes the note say *"Nothing in this session can answer it"* instead,
and **Ctrl-C is measured to escape** — the turn cancels and the next message can
be typed.

### The decision

> **Does the TUI become a first-class mobile surface — and does `warpctrl` go
> into it?**

**The argument for, in the maintainer's own words:** the GUI needs relaunches
and rebuilds, and those kill the console. A TUI over mosh+tmux survives both.
That is a friction the console structurally cannot fix, because the console is
*a browser onto a GUI* — friction line a9 says exactly that.

**The argument that held it back, and why it may no longer hold.** I20 deferred
on the advisor's ruling: *"build nothing a friction log has not asked for. The
panel earned its button after 35 measured copy-paste approvals. The TUI has zero
sessions of friction log."* **That condition is arguably met by the maintainer's
own ask** — but note the shape of the precedent: the panel's button was earned
by *use*, not by anticipation. A defensible middle is to **use the TUI from the
phone for a few sessions first** and let the friction log say whether `warpctrl`
inside it is wanted, which also feeds §0.

**The hazard, named in I20 so it is not discovered later:** **type-ahead.** A
TUI prompt appears in the same terminal the person is typing into. An Enter
already sitting in the input buffer when the prompt takes focus is **a yes
nobody gave** — consent manufactured by the terminal's line discipline rather
than by a decision. This has no analogue in the panel. Anything that puts an
answerable prompt in the TUI must answer it.

**If the answer is yes**, the path is two small edits plus that hazard, and it
would also serve the T12 console with **zero** new consent-surface code.

**Not in scope for this decision, and the maintainer has already said so:** the
Android APK stays on the table as a thing they want to build for its own sake —
Rust on Android, a surface they enjoy designing. The TUI postpones the *need*,
not the *want*, and nobody should argue it away.

---

## Item 2 · The uncapped build — the one run that carries the risk it measures

**Open since 2026-08-29 and never run.** The maintainer is at the desk and has
said it is possible, so this is the sitting to do it in.

**What is settled, so it is not re-litigated:** `-j` is *not* what stands
between this build and the wall. Measured — the `-j 8` parallel front peaks at
a **summed 3,287 MB** across seven compilers, while the `warp` crate compiling
**alone** hits **15,809 MB** (exact, `/usr/bin/time -v`), and the closest the
machine ever came to the edge was during that single-crate phase. The app crate
is now bounded to **11,342 MB** by `[profile.release.package.warp]`.

**What is not settled:** eight jobs averaged ~470 MB each. **Thirty-two of them
together has never been run**, and that is the case the cap was chosen for after
an uncapped build took the whole VM down (guest back at `up 1 min`, empty
`dmesg` — the VM dying, not Linux OOM-killing a process).

### Preconditions, all three, none optional

1. **Read the host, not the guest.** `free -m` inside WSL said 22 GB available
   while the host was at 60 of 64. The guest structurally cannot see the hazard.
   ```bash
   powershell.exe -NoProfile -Command '$os=Get-CimInstance Win32_OperatingSystem;
     ($os.TotalVisibleMemorySize-$os.FreePhysicalMemory)/1MB'
   ```
2. **Kill `rust-analyzer` first.** One over this workspace holds 15-16.5 GB —
   more than the whole profile sweep saved, and a confound that makes the
   measurement meaningless even if the build finishes.
3. **Nothing building on the Windows side.** That pair is what took the guest
   down.

**And it needs a clean `target/release`, which costs twice**: it also
uninstalls the WSL daemon (`~/.warp-dev/remote-server/warp-oss` is a symlink
into it), and every WSL pane's auto-connect then fails with a banner naming
neither WSL nor the daemon. Budget the rebuild, and expect the panes to degrade
to 9p until it lands.

**Instrument:** `.fork/tools/memsample.sh` at `MEMSAMPLE_INTERVAL=2` for the
*sum* and the `MemAvailable` trace — the sum is the quantity that kills the VM,
not the max. `/usr/bin/time -v` gives the exact single-process peak.

> **Decision: run it, or close it as "keep the cap, stop asking"?** Either is
> defensible. What is not defensible is leaving it open for a fourth round of
> re-deriving the numbers.

---

## Item 3 · `authenticate` — half 2, which needs a credential

Half 1 shipped 2026-09-10 (`79b3d2deb`, `app/src/ai/acp_agent/auth.rs`): when
`session/new` is refused for a credential, the panel quotes the agent's own
error, lists the method ids it advertised, and names `WARP_FORK_ACP_AUTH` as a
variable **that does not exist**. Verified this session: the string appears only
in that sentence and its tests, never passed to `env::var`.

**Measured on the wire, 2026-09-07:**

| agent | refuses `session/new` with | offers |
|---|---|---|
| `@zed-industries/codex-acp@0.16.0` | `Authentication required` | `chatgpt`, `codex-api-key`, `openai-api-key` |
| `@google/gemini-cli@0.58.0 --acp` | `Gemini API key is missing or not configured` | `oauth-personal`, `gemini-api-key`, `vertex-ai`, `gateway` |

> **Decision: is there a credential for either, and do you want Warp to send
> `authenticate`?**

Two sub-questions, and the second is the real one. Choosing the *method id* is
choosing where a credential comes from, which is why `WARP_FORK_ACP_MODE` has no
default either. If yes, the variable gets a reader and the panel gets a
`session/authenticate` call; if no, half 1 is the whole feature and item 5
closes as done-as-intended.

**Also unmeasured either way:** whether an ACP agent process finds the same
credential file the vendor's own CLI wrote, when logged in out of band. I20's
own note calls that *"the first thing to measure once a credential exists."*

---

## Item 4 · `did_change_watched_files` — two candidate fixes, you pick

T18. Advertised and never sent. The ticket has both candidates; this is a pick,
not an investigation. **Read T18's two options and choose one**, or strike the
item with a reason.

---

## Item 5 · kode-rs — the framing question comes first

`~/dev/kode-rs`, HEAD `7c0cbc5`, **2026-08-10 — a month stale**, 58 GB on disk
mostly target output. Its ACP side is real and on main
(`crates/kode-engine/src/acp.rs`, 1,176 lines) and implements
`session/request_permission`, which is the one method the fork's consent
surface needs. It lacks `configOptions`/`session/set_config_option`, so **the
model chip is dead against it**, and it has no session modes.

> **Decision, and the board says settle this before anything else: is kode-rs
> "an agent we control end to end" or "a better agent than opencode"?**

The OpenAI Codex fork raised alongside it answers only the second. If the answer
is the first, the two missing methods are small additions to a file that already
speaks the protocol. If the second, `claude-agent-acp` + `WARP_FORK_ACP_MODE=default`
is already the measured-best pairing and kode-rs has to beat it.

**Gate before it becomes work:** does it still build against a month of moving
toolchain? Unmeasured.

---

## Item 6 · The `api.anthropic.com` phone-home — accept, contain, or disclose

Measured twice: `claude-agent-acp` opens a TLS connection to `api.anthropic.com`
**before** it opens the one to `llama-server`, during a turn answered entirely
by a local model. ~~`CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` does not stop it.~~
*It does. The measuring cwd's `.claude/settings.local.json` set it back to
`""`; corrected 2026-09-13, `.fork/runs/vendorcalls-2026-09-13/`.*
Of Anthropic's two named exemptions, the marketplace one was **tested and
excluded**; the WebFetch preflight (`skipWebFetchPreflight`) is **untested**
because it fires only when WebFetch is used, and its switch lives in the
maintainer's own Claude Code settings.

Two things that bound the decision. It reproduces with **no Warp process at
all** (`warpctrl acp probe` inside the distribution, four minutes) — so it is a
fact about the agent, never about Warp. And **the fix, if one is wanted, is a
firewall rule or a network namespace around the agent, not a `warpctrl`
change.**

> **Decision: accept and document, contain, or disclose?**

Caveat on the evidence: that run's socket poller **never caught its own positive
control**, so it is trustworthy for what it saw and worthless for what it did
not. Any *absence* claim must come from `.fork/runs/localmodel-panel-2026-09-09/`,
whose control did fire.

---

## Item 7 · Windows Developer Mode — needs your hand, not a decision

So the `.claude/skills` symlink resolves in `C:\dev\warp`. An elevated prompt;
nobody else can do it. Two minutes, or strike it.

---

## Standing constraints

- **No push, no PR, no upstream merge without an explicit say-so.**
- **Permission posture is frozen.** Do not measure it further. (Item 3 is about
  a credential, not about posture.)
- `CARGO_BUILD_JOBS=8` **unless item 2 is explicitly authorised in this
  sitting**, which is the one exception this file creates.
- **Never build on both sides of the VM at once.**
- Leave no Warp or agent processes running, except one working instance if the
  maintainer wants it left up — and say which.

## What is running (2026-09-11, start of session)

- **No Warp instance, no `llama-server`, no `rustc`, no `rust-analyzer`.** Host
  at 17.0 GB of 65.4 — the cleanest start in a week.
- Restart the model with `powershell.exe -File 'C:\dev\llama\serve.ps1'`
  (defaults to `-Ctx 98304` since 2026-09-09); check with
  `curl -s -m 5 http://127.0.0.1:8080/v1/models`. **It has been stopped
  deliberately before and read as a crash** — friction a17 was retracted for
  exactly that, so do not diagnose a death before asking.
- `C:\dev\warp` is a **separate checkout that nothing syncs**; check the commit,
  not the clock:
  ```bash
  git -C /mnt/c/dev/warp fetch /home/effatha/git/warp dev && git -C /mnt/c/dev/warp merge --ff-only FETCH_HEAD
  ```
- Cloud routines: the pocket-tts re-check chain was **disabled 2026-09-11**
  after 19 spent one-shots. PR #6 is `OPEN`/`CLEAN`/`MERGEABLE`, the
  maintainer's to merge.

## Traps, ordered by what each has cost

- **Read the host's memory, not the guest's.** Item 2 depends on this.
- **Kill `rust-analyzer` before measuring anything.** 15-16.5 GB.
- **Never `pgrep -f` a pattern your own command line contains** — the shell
  running the loop matches itself. Wait on a **pid** or a marker file. In
  `CLAUDE.md` since 2026-08-30 and walked into three times since.
- **Build with `.fork/tools/build.sh`**, not bare `cargo` — only it writes the
  version sidecar. And a build killed between link and stamp leaves a *working*
  binary reporting the **previous** commit, which defeats the one recorded test
  for whether your code ran (friction a22).
- **`./script/format` drive-bys into `crates/remote_server/src/manager_tests.rs`
  every time.** Check `git status` and revert it.
- **`cargo check --workspace --all-targets` is the gate**, not the binary build.
- **Diff test-failure membership, not counts** — 19-28 failures across six runs
  of the same tree. Baseline from two runs.
- **A grep over a crate answers a question about that crate.** Item 1 exists
  because one was read as an answer about a binary. When the question is "can
  the fork reach X", check what X *depends on*, not only what it contains.

## Gates, before anything is called done

```bash
cargo check --workspace --all-targets
./script/format && git status --short     # revert the manager_tests.rs drive-by
```

And the question this repo keeps paying for, asked of every file touched:
**does any doc comment here now claim something the code below it does not do?**
Seventeen of those have been found. The newest is item 1's, and it was load-
bearing on the very question this run opens with.
