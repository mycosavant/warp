# Handoff: the auth disclosure, the housekeeping, and three decisions that are not yours

**Written 2026-09-10, after `.fork/runs/profile-2026-09-09/`. Paste-target:
start a new session and say *"read `.fork/HANDOFF-AUTH.md` and run it to
completion"*.**

Read `CLAUDE.md` first, as always. This file is the run, not the method.

---

## Read these before touching anything

| | why |
|---|---|
| `.fork/GOAL.md` | the standing horizon. It outranks every ticket's ordering while it stands |
| `.fork/next.html` | the board. Items 1-4 and 7b done, 5-8 open |
| `.fork/runs/acp-survey-2026-09-07/README.md` | **the measurements this run rests on.** Both other agents refuse `session/new` without a credential, and it was measured on the wire, not read |
| `.fork/decisions/2026-09-09-the-j-cap-is-not-what-holds-the-line.md` | **the `-j` question is closed and so is the profile question.** Both have been litigated. What is still open is named there |

**Four spent handoffs sit in `.fork/` and none of them is your run**:
`HANDOFF-PROFILE.md` (done 2026-09-09, banner at the top), `HANDOFF-SPECSFETCH.md`,
`HANDOFF-LOCALMODEL.md`, `HANDOFF-MOBILE.md`. Their open threads are carried into
this file. Do not run one of them because it was the first file you opened.

---

## Where things stand

| item | state |
|---|---|
| 1 · the picker against a non-Anthropic list | done 2026-09-07 |
| 2 · a model runtime reachable from both sides | done 2026-09-07 |
| 3 · a local model answers the panel | done 2026-09-09 |
| 4 · the specs card's fetch | done 2026-09-09, `87d4d2049` |
| 5 · `authenticate`, disclosed now and sent later | **open — this run** |
| 6 · housekeeping that has recurred | **open — the second half of this run** |
| 7 · the clean-build test of `-j 8` | one part left, and it is a decision |
| 7b · bound the `warp` crate | **done 2026-09-09**, `c25c8fbc2`, −28.3% |
| 8 · kode-rs as a third harness | recon done, the maintainer's call |

---

## The run, part 1: say why `session/new` was refused

### What is measured, and it is the whole reason this run exists

`warpctrl acp probe` on 2026-09-07, from WSL, against the two agents the fork
has never run in the panel:

| agent | `initialize` | `session/new` |
|---|---|---|
| `@zed-industries/codex-acp@0.16.0` | answered; `authMethods`: `chatgpt`, `codex-api-key`, `openai-api-key` | **refused, `Authentication required`** |
| `@google/gemini-cli@0.58.0 --acp` | answered; `authMethods`: `oauth-personal`, `gemini-api-key`, `vertex-ai`, `gateway` | **refused, `Gemini API key is missing or not configured`** |

**Neither credential exists on this machine** and that is not a blocker for this
run, because the half being built needs none.

**With `claude-agent-acp` this never mattered** — Claude Code reads its own login
from disk and the panel has no auth step, which is why the fork has run for
weeks without noticing. Point `WARP_FORK_ACP_COMMAND` at either agent above and
every turn dies at `session/new` with a raw JSON-RPC error and no advice.

### The gap, located exactly

`grep -rn "auth" app/src/ai/acp_agent/` is **empty**. Confirmed again 2026-09-10.

| | where |
|---|---|
| the `initialize` reply, whose `auth_methods` is dropped on the floor | `app/src/ai/acp_agent/mod.rs`, the `hello` binding (~line 605) |
| `session/new`, whose error propagates raw through `?` | `mod.rs:636`, `send_request(NewSessionRequest::new(cwd))` |
| **the pattern to copy** | `cannot_resume` at `mod.rs:168` and `resume_failed` at `mod.rs:182` |

`cannot_resume` is the shape this wants: a named function returning one
sentence that says what happened, what it means for the turn, and which
variable to change. Note that the resume path deliberately uses
`match loaded { Err(error) => … }` rather than `?`, precisely so it can say
something. **`session/new` uses `?` and therefore cannot.** That is the change.

### One thing verified so nobody re-derives it

The fork imports `agent_client_protocol::schema::v1::*` and sends
`ProtocolVersion::V1`. **Both halves are reachable on that path**:
`v1::InitializeResponse.auth_methods` exists (schema 1.7.0, `src/v1/agent.rs:147`)
and so does `v1::AuthenticateRequest { method_id: AuthMethodId }`
(`src/v1/agent.rs:295`). There is no v1/v2 trap here. The crate is
`agent-client-protocol = "2.0.0"` in `app/Cargo.toml:312`.

### What to build

**Only the half that needs no credential.** When `session/new` is refused and
the agent advertised `authMethods`, the panel says so **in the agent's own
words**, lists the method ids the agent named, and names
`WARP_FORK_ACP_AUTH` as the thing that does not exist yet.

Three properties, and the third is the one that gets lost:

1. **The agent's own error text is quoted, not paraphrased.** `Gemini API key is
   missing or not configured` is more useful than anything Warp would write.
2. **The method ids come from the wire**, not from a table. They are opaque
   vendor strings — the same argument that gives `WARP_FORK_ACP_MODE` no
   default. Warp lists what the agent said and picks nothing.
3. **Say it only when it is true.** An agent that advertised no `authMethods`
   and still refused `session/new` has a different problem, and telling that
   person to authenticate is worse than saying nothing. Two arms, like
   `cannot_resume` / `resume_failed`, and the doc comment on each says why they
   are separate.

### What NOT to build, and it is written down for a reason

**Do not send `authenticate`.** That is half 2, and it is the maintainer's:
the credential is theirs to supply or decline, and a method id chosen by Warp
is the mistake `WARP_FORK_ACP_MODE` exists not to make. Building it blind
would also be untestable here — there is no credential on this machine, so
the success criterion would be "it compiles", which this repo has a name for.

**Do not add a `WARP_FORK_ACP_AUTH` reader.** Half 1 *names* the variable in a
sentence; it does not parse one. A variable that is read and does nothing is
worse than one that does not exist.

### Done when

- A panel session against an agent that refuses `session/new` for auth reasons
  says so, quoting the agent, listing its method ids, naming the variable.
- Tests calibrated **by making them fail**, not by watching them pass — the two
  arms must be shown to be distinguishable, or the second one is decoration.
- The event log gets the same fact if that is one line; if it is more, say so
  and leave it.
- `.fork/runs/auth-2026-09-<dd>/` in the shape of the existing run READMEs, and
  `.fork/next.html` item 5 updated to say which half shipped.

**You can measure this without a credential.** `warpctrl acp probe` reproduces
both refusals with no Warp process at all — see
`.fork/runs/pricefetch-2026-09-09/optout-probe.sh` for the shape, four minutes,
no GUI and no relaunch. Do that before touching the panel.

---

## The run, part 2: the housekeeping that keeps recurring

From the 2026-09-07 inventory, with the two that closed struck out. Each is an
evening; none needs the desk. **Do these between builds, not instead of part 1.**

| what | where | note |
|---|---|---|
| A discovery record and broker socket outlive three clean `window close` shutdowns; **unbisected** | T15 | the most-mentioned open one. WSL's discovery directory is empty right now, so **reproduce it before bisecting** — and read `CLAUDE.md`'s "stale discovery record" correction first, because the observation was right and the mechanism recorded under it was invented twice |
| `did_change_watched_files` advertised and never sent; two candidate fixes | T18 | **the maintainer picks.** Do not choose one |
| The drift-check cron line, never installed | `GOAL.md` board item 1 | the line is in `manual.md`, not the README — `board.html` pointed at the wrong file and that was fixed 2026-09-09 |
| Windows Developer Mode so the `.claude/skills` symlink resolves in `C:\dev\warp` | open-questions | needs an elevated prompt, so it is the maintainer's hand |

---

## Standing constraints, from `.fork/GOAL.md`

- **No push, no PR, no upstream merge without explicit say-so.**
- **Permission posture is frozen.** Do not measure it further.
- `CARGO_BUILD_JOBS=8`. **Never build on both sides of the VM at once.**
- **Leave no Warp or agent processes running** — except that the maintainer is
  remote and wants a working instance left up. Leave exactly one, and say which.

---

## What is running right now (2026-09-10, start of session)

- **One Warp instance**, `inst_d7141e7ff2834bb6b5bdb1e51f882e57`, pid **5808**,
  Windows-side, product profile with `-Console`. Console verified serving
  **HTTP 200** on `https://100.82.213.46:41234` (tailnet bind, TLS, private
  authority). Its `app_version` is `v0.fork.2e1552fc0` — **eleven commits behind
  `dev`**, because `C:\dev\warp` is a separate checkout nothing syncs.

- **`llama-server` is DOWN.** Second silent disappearance in two days, and
  `c26879b66` already recorded the first: no crash line, no cause, and
  `C:\dev\llama` contains **no log file at all**, so there is nothing to read.
  Nothing in Warp announces its absence. **The four small AI features are dead
  until it is restarted**, and a panel session pointed at the local model will
  fail in a way that reads as a Warp fault.

  ```
  restart:  powershell.exe -File 'C:\dev\llama\serve.ps1'      # -Ctx defaults to 98304 since 2026-09-09
  check:    curl -s -m 5 http://127.0.0.1:8080/v1/models
  ```

  It holds ~7-10 GB of host RAM and of VRAM by design. Count it in every memory
  budget. **That it has now died twice unobserved is itself the argument for
  the watcher T21 named and nobody built.**

- **Host at 12.4 GB used of 65.4**, `vmmemWSL` down to 2.6 GB — WSL has fully
  reclaimed, so this is the cleanest starting state in days. No `rustc`, no
  `cargo`, no `rust-analyzer`.

- `target/release` holds **1,124 rlibs** and `target/release/warp-oss` is
  `v0.fork.c25c8fbc2`, 652,445,672 bytes, built under the new profile. **Do not
  `cargo clean`** — `~/.warp-dev/remote-server/warp-oss` is a symlink into it,
  so cleaning uninstalls the WSL daemon, and a warm target is what makes an
  app-crate rebuild cost five minutes instead of thirty.

---

## Traps this run will walk into

Ordered by how much time each has cost. **Every one has been walked into.**

- **Read the *host's* memory, not the guest's.** `free -m` inside WSL said
  22 GB available while the host was at 60 GB of 64.
  ```bash
  powershell.exe -NoProfile -Command '$os=Get-CimInstance Win32_OperatingSystem;
    ($os.TotalVisibleMemorySize-$os.FreePhysicalMemory)/1MB'
  ```
- **Kill `rust-analyzer` before measuring anything.** One over this workspace
  holds 15-16.5 GB — three times what the whole profile sweep bought. An editor
  session spawns it silently.
- **For any memory comparison use `/usr/bin/time -v`, not a sampler.** New as of
  2026-09-09: a 10-second sampler read the *same build* as 15,587 and 14,978 MB.
  `getrusage(RUSAGE_CHILDREN)` has no sampling window. `memsample.sh` is still
  what gives the sum and the `MemAvailable` trace, and it takes
  `MEMSAMPLE_INTERVAL` now.
- **Never `pgrep -f` a pattern your own command line contains.** A waiter
  written as `until ! pgrep -f 'cargo build'` never fires because the shell
  running it matches itself. Wait on a **pid** or a **marker file**. In
  `CLAUDE.md` since 2026-08-30 and walked into three times since.
- **Build with `.fork/tools/build.sh`, not a bare `cargo build`.** Only the
  script writes the `warp-oss.version` sidecar; without it the daemon reports no
  version and the handshake line goes blank. Over a finished build it costs
  0.5 s.
- **`./script/format` makes a drive-by into
  `crates/remote_server/src/manager_tests.rs` every time.** Check `git status`
  after and revert it. It has now been reverted on two consecutive runs.
- **A live run measures the binary, not your source**, and on Windows the
  timestamp check does not work — `C:\dev\warp` is a separate checkout.
  **Check the commit**: `git -C /mnt/c/dev/warp log --oneline -1` against your
  HEAD. It is currently 11 behind. To sync, the command depends on which side's
  `git` you hold:
  ```bash
  git -C /mnt/c/dev/warp fetch /home/effatha/git/warp dev && git -C /mnt/c/dev/warp merge --ff-only FETCH_HEAD
  ```
- **`cargo check --workspace --all-targets` is the gate, not the binary build.**
  `--bin warp-oss` compiles neither test code nor `warp_tui`, and a stale
  `assert_eq!` compiles perfectly.
- **Diff test-failure membership, not counts.** Six runs of `-p warp --lib` gave
  19-28 failures against a union of 26 names. Baseline from **two** runs, or a
  flaky pass promotes an old failure to a fresh regression.
- **Read a `.fork/docs/` finding to the end before acting on it.** These pages
  carry their own retractions inline, two paragraphs below the claim.

---

## Three threads that are the maintainer's, not yours

**Do not build any of these on your own judgment.** Each is a choice, and each
has evidence already gathered.

### 1. The agent phones `api.anthropic.com` during a turn a local model answers

Measured twice. `claude-agent-acp` opens a TLS connection to
`api.anthropic.com` **before** it opens the one to `llama-server`, during a turn
answered entirely on this machine. `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`
does not stop it. Anthropic's docs name exactly two exemptions;
`CLAUDE_CODE_DISABLE_OFFICIAL_MARKETPLACE_AUTOINSTALL=1` was **tested and
excluded**, and the WebFetch preflight (`skipWebFetchPreflight`) is **untested**
because it fires only when WebFetch is used and its switch lives in the
maintainer's own Claude Code settings.

The choice is **accept and document / contain / disclose**.
`.fork/runs/pricefetch-2026-09-09/README.md` §2 has the evidence. Two things
worth knowing if it comes up: it reproduces with **no Warp process at all**
(`warpctrl acp probe` inside the distribution, four minutes), and **the fix, if
one is wanted, is a firewall rule or a network namespace — not a `warpctrl`
change**. Also: that run's socket poller **never caught its own positive
control**, so it is trustworthy for what it saw and worthless for what it did
not; any *absence* claim must come from `.fork/runs/localmodel-panel-2026-09-09/`,
whose control did fire.

### 2. The uncapped front half — the only part of item 7 left

Eight jobs averaged ~470 MB. What thirty-two do together is the question the cap
was chosen for in 2026-08-29 and it has **never been run**. It carries the risk
it measures, needs an explicit say-so and a quiet host. The profile work does
not change this: `-j` was never the binding constraint on the *single-crate*
phase, and the parallel front is still unmeasured uncapped.

### 3. kode-rs as a third harness

`~/dev/kode-rs`, HEAD `7c0cbc5`, **2026-08-10, a month stale**, 58 GB on disk.
Its ACP side is real and on main (`crates/kode-engine/src/acp.rs`, 1,176 lines)
and implements `session/request_permission`, which is the one thing the fork's
consent surface needs. It lacks `configOptions`/`session/set_config_option`, so
**the model chip is dead against it**, and it has no session modes. The board
item asks the question to settle first: is this *"an agent we control end to
end"* or *"a better agent than opencode"*? The OpenAI Codex fork raised
alongside it answers only the second.

---

## Gates, before you call anything done

```bash
cargo check --workspace --all-targets
cargo test -p warp --lib acp_agent::            # the module this run touches
./script/format && git status --short           # then revert the manager_tests.rs drive-by
```

And the one this repo keeps paying for: **ask of every file you edit, "does any
doc comment here now claim something the code below it does not do?"** Fourteen
of those have been found, every one written carefully and falsified later by a
change beside it. `cargo check`, `cargo test` and `./script/format` are all
silent on every one.
