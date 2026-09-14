# Handoff: bound the app crate, and the four open threads behind it

> **The run is done, 2026-09-09.** `[profile.release.package.warp]` with
> `debug = 0` and `codegen-units = 64`: **15,809 MB → 11,342 MB (−28.3%)**,
> 378 s → 305 s, binary −125 MB. Applied, with the argument in `Cargo.toml`.
> `opt-level = 2` and `split-debuginfo = "unpacked"` were measured and refused.
> Full account in `.fork/runs/profile-2026-09-09/README.md`; the answer is
> folded into the decision file this handoff pointed at.
>
> **Its baseline was wrong and the run's first act was to catch that.** The
> 15,587 MB below is a 10-second sampler's reading of a peak whose adjacent
> ticks swing 1.7 GB. Read §1 of the run README before trusting any memory
> number in this file.
>
> **The four threads at the bottom are still open** and are why this file is
> kept rather than deleted.

**Written 2026-09-09, after `.fork/runs/pricefetch-2026-09-09/`. Paste-target:
start a new session and say *"read `.fork/archive/HANDOFF-PROFILE.md` and run it to
completion"*.**

Read `CLAUDE.md` first, as always. This file is the run, not the method.

---

## Read these three before touching anything

| | why |
|---|---|
| `.fork/decisions/2026-09-09-the-j-cap-is-not-what-holds-the-line.md` | **the `-j` question is closed. Do not reopen it.** It has been litigated three times. What is still open is named in that file and it is not `-j 8` |
| `.fork/runs/pricefetch-2026-09-09/README.md` | the measurements this run rests on, including the two instruments that were wrong and how they were caught |
| `.fork/next.html` | the board. Items 1-4 done, 5-8 open |

---

## Where things stand

| item | state |
|---|---|
| 1 · the picker against a non-Anthropic list | done 2026-09-07 |
| 2 · a model runtime reachable from both sides | done 2026-09-07 |
| 3 · a local model answers the panel | done 2026-09-09 |
| 4 · the specs card's fetch | **done 2026-09-09**, `87d4d2049` |
| 5 · `authenticate`, disclosed now and sent later | open |
| 6 · housekeeping that has recurred | between builds |
| 7 · the clean-build test of `-j 8` | **mostly answered**; one part left, below |
| 8 · kode-rs as a third harness | recon done, the maintainer's call |

---

## The run: bound the `warp` crate

**The question the maintainer asked, verbatim: "Is there a way to bound the
compilation for just the one, most demanding crate?"**

### What is measured, and it is the whole reason this run exists

Clean build, corrected sampler, 2026-09-09
(`.fork/runs/pricefetch-2026-09-09/memsample-fixed.tsv`, 31 samples):

| phase | held |
|---|---|
| the `-j 8` parallel front, 7 concurrent compilers | **summed 3,287 MB**, heaviest single 911 MB |
| the `warp` crate, **alone** | 1.5 GB → **14,975 MB** over four minutes, still climbing when stopped |
| lowest `MemAvailable` | **8,198 MB, during the single-crate phase** |

So the app crate is the whole problem and `-j` is irrelevant to it.

**The true peak is 15,587 MB**, from a repair build that finished the same
evening (`memsample-repair.tsv`, 36 samples, 5m58s with the graph warm). The
14,975 above was a floor, as expected. **That is your baseline — do not
re-measure it.**

**And the largest single lever is not in the profile at all.** The same crate,
same flags, bottomed out at `MemAvailable` **22,795 MB** on the repair run
against **8,198 MB** on the clean one, because no `rust-analyzer` was resident.
14.6 GB, for free, from killing an editor's language server. Any profile
saving you measure should be reported against that, or it will look more
impressive than it is.

### The lever, and why it reaches only the crate that matters

`[profile.release.package.warp]` in the root `Cargo.toml`. Stable cargo,
per-package profile override; the repo already uses the pattern at
`[profile.dev.package]`. Everything below goes there and touches nothing else in
the graph.

**`[profile.release]` already fought this fight once and its comment is the best
evidence in the repo.** It sets `debug = 1` rather than full debuginfo, saying
in as many words that dropping type info *"significantly reduces rustc's peak
memory during ThinLTO + codegen (which was OOM-killing release builds on CI)"*.
Read that comment before you start; it tells you the mechanism is DWARF and
codegen, not the number of jobs.

### The four candidates, none of them measured

**Nothing below is a finding. They are candidates and the run is the point.**

| lever | expected | costs |
|---|---|---|
| `debug = 0` | the largest single win, on the existing comment's own reasoning | no file/line in panic backtraces for this crate |
| `opt-level = 2` (or `1`) | LLVM peak scales hard with opt-level 3 inlining | runtime speed, in the app crate only |
| `codegen-units = 32` (default is 16) | smaller LLVM modules; may cut peak or may raise it by running more at once | compile speed, maybe runtime |
| `split-debuginfo = "unpacked"` | the current `"packed"` is a macOS setting; unpacked moves DWARF out of the link | Linux only |

**`debug = 0` has a fork-specific argument the upstream comment cannot make.**
That comment justifies keeping line tables by *"symbolicate panics and Sentry
stack traces"*. **This fork force-disables Sentry** (`FORCE_DISABLED` in
`app/src/fork.rs`), so half of the stated justification does not apply here. The
other half — local panic backtraces — does, and it is a real cost. Measure the
saving before arguing about the trade.

### Why this run is cheap, which is the thing to exploit

**With the dependency graph warm, one variant is one app-crate compile: about
four minutes.** `target/release` currently holds 1,123 built rlibs. So a sweep
of all four levers plus a baseline is roughly half an hour of wall time, not a
day. Do **not** `cargo clean` — that throws away the only thing making this
affordable.

Touch `app/src/main.rs` (or `touch app/src/lib.rs`) between variants to force
the app crate to rebuild without disturbing its dependencies.

### The method

```bash
# Before every build, and this is not optional -- see "Traps".
powershell.exe -NoProfile -Command '$os=Get-CimInstance Win32_OperatingSystem;
  ($os.TotalVisibleMemorySize-$os.FreePhysicalMemory)/1MB'
pgrep -x rust-analyzer && echo "KILL THIS FIRST"

.fork/tools/memsample.sh /tmp/variant.tsv &
CARGO_BUILD_JOBS=8 cargo build --release --features gui,warp_control_cli
```

Read `sum_rss_mb` against `avail_mb`. **A row whose `max_crate` is `?` is not
describing a compiler** — see the traps.

### Done when

- Each lever's saving is stated against the 15,587 MB baseline.
- Each of the four levers has a number beside it and a one-line cost.
- A recommendation, or an argued refusal to change anything.
- `.fork/runs/profile-2026-09-<dd>/` in the shape of the existing run READMEs,
  and the answer folded into `.fork/decisions/2026-09-09-the-j-cap-is-not-what-holds-the-line.md`
  rather than a new decision file — that file already names this as the thing to
  attack instead.

---

## Standing constraints, from `.fork/GOAL.md`

- No push, no PR, no upstream merge without explicit say-so.
- Permission posture is **frozen**. Do not measure it further.
- `CARGO_BUILD_JOBS=8`. Never build on both sides of the VM at once.
- **Leave no Warp or agent processes running** — except that the maintainer is
  remote and wants a working instance left up. Leave exactly one, and say which.

## What is running right now

- **One Warp instance**, `inst_d7141e7ff2834bb6b5bdb1e51f882e57`, product
  profile with `-Console`, console on the tailnet bind
  `https://100.82.213.46:41234`. Relaunched at 19:51 after the previous one was
  found down.
- **`llama-server`** on `127.0.0.1:8080`, `gemma-4-12b`, `n_ctx_slot = 98304`,
  holding **10.2 GB** of VRAM and of the host's RAM. It is resident by design;
  count it in every memory budget. `Get-Process llama-server | Stop-Process`
  frees it, and `ps1 -File 'C:\dev\llama\serve.ps1'` restores it — the
  context default was raised to 98304 on 2026-09-09, so no `-Ctx` argument is
  needed any more.

  **Check it is actually up before trusting the four small AI features.**
  ~~It died silently once on 2026-09-09~~ — corrected 2026-09-10: it did not
  die, the maintainer stopped it, both that time and the next. The durable
  half is unchanged and is the reason to run the check: nothing in Warp
  announces its absence, whoever caused it.
  `curl -s -m 5 http://127.0.0.1:8080/v1/models`.

---

## Traps this run will walk into

**Every one of these was walked into on 2026-09-09.** They are not
hypothetical and they are ordered by how much time each cost.

- **Read the *host's* memory, not the guest's.** `free -m` inside WSL said
  22 GB available while the host was at **60 GB of 64**. The guest structurally
  cannot see the hazard. The one-liner is in the method above and in
  `CLAUDE.md`.
- **Kill `rust-analyzer` before sampling anything.** One over this workspace
  holds **15-16.5 GB**. It is not part of the build, it is the single largest
  avoidable consumer on the machine, and it is a confound that makes a memory
  measurement meaningless. An editor session spawns it silently.
- **`ps -C rustc` also matches `rust-analyzer`** on this procps. That is fixed
  in `.fork/tools/memsample.sh` as of this date, but any number from an older
  run, or any ad-hoc `ps -C rustc` you type yourself, is suspect. **The tell is
  the crate column: rust-analyzer has no `--crate-name`, so it reads `?`.**
- **Never `pgrep -f` a pattern your own command line contains.** A waiter
  written as `until ! pgrep -f 'cargo check'` never fires, because the shell
  running it matches itself. This is in `CLAUDE.md` from 2026-08-30 and was
  walked into twice more on 2026-09-09 by someone who had read it that session.
  Wait on a **pid** or a **marker file**.
- **`cargo clean --release` breaks the WSL remote-development server.**
  `~/.warp-dev/remote-server/warp-oss` is a **symlink to
  `target/release/warp-oss`**. Delete the target and every WSL pane's
  auto-connect fails with *"Response channel closed before receiving a reply"*,
  which Warp's UI shows as **"Failed to start SSH extension"** and which reads
  like a network fault. Rebuild and it returns. This is why the sweep above says
  do not clean.
- **Build with `.fork/tools/build.sh`, not a bare `cargo build`.** Only the
  script writes the `warp-oss.version` sidecar, and without it the daemon
  reports no version. It still connects — `version_is_compatible` answers
  `true` unconditionally on Oss since `1a42ecdb8` — but the handshake line that
  says which two builds are talking goes blank. Over a finished build the
  script costs 0.55s.
- **Read a `.fork/docs/` finding to the end before acting on it.**
  `wsl.md`'s run-1 finding 2 says the version repair deletes the staged
  symlink; two paragraphs later it records the commit that stopped it. Half of
  that section, read in a hurry, produced a wrong warning to the maintainer on
  2026-09-09. These pages carry their own retractions inline.
- **A live run measures the binary, not your source**, and on Windows the
  timestamp check does not work because `C:\dev\warp` is a separate checkout
  nothing syncs. `git -C /mnt/c/dev/warp log --oneline -1` against your HEAD.
- **Diff test-failure membership, not counts.** Six runs of `-p warp --lib` gave
  19-28 failures against a union of 26 names. Baseline from **two** runs.

---

## Four threads left open, in the order they are worth picking up

### 1. Decision 1 is back with the maintainer and nothing should be built for it

**The agent's process opens a TLS connection to `api.anthropic.com` during a
turn answered entirely by a local model**, before it opens the one to the model.
~~`CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` does not stop it.~~ *It does. The
measuring cwd's `.claude/settings.local.json` set it back to `""`; corrected
2026-09-13, `.fork/runs/vendorcalls-2026-09-13/`.* Anthropic's docs
name exactly two things exempt from that variable, and
`CLAUDE_CODE_DISABLE_OFFICIAL_MARKETPLACE_AUTOINSTALL=1` was **tested and
excluded** on 2026-09-09. The other, the WebFetch domain safety check
(`skipWebFetchPreflight`), is **untested** — it fires only when WebFetch is
used, and its switch lives in a settings file belonging to the maintainer's own
Claude Code, which is why it was deliberately not touched.

The choice is **accept and document / contain / disclose** and it is the
maintainer's. `.fork/runs/pricefetch-2026-09-09/README.md` §2 has the evidence.
**Do not build any of them on your own judgment.**

Two things worth knowing if you touch this. It reproduces with **no Warp process
at all** — `warpctrl acp probe` inside the distribution, four minutes, script at
`.fork/runs/pricefetch-2026-09-09/optout-probe.sh`. And the socket poller used
there **never caught its own positive control**, so it is trustworthy for what
it saw and worthless for what it did not; any absence claim must come from
`.fork/runs/localmodel-panel-2026-09-09/`, whose control did fire.

### 2. The uncapped front half — the only part of item 7 left

Eight jobs averaged ~470 MB. What thirty-two do together is the question the cap
was chosen for in 2026-08-29 and it has never been run. **It carries the risk it
measures** and needs an explicit say-so, plus a quiet host. Read the decision
file before proposing it.

### 3. kode-rs, if the maintainer wants it

`.fork/next.html` item 8 has the recon. Short version: its ACP side is real and
on main (`~/dev/kode-rs`, `crates/kode-engine/src/acp.rs`, 1,176 lines) and
implements `session/request_permission`, which is the one Warp's consent surface
needs. It lacks `configOptions`/`session/set_config_option`, so **the model chip
is dead against it**, and it has no session modes. A month stale; unbuilt.

The board item asks the question that should be settled first: is this "an agent
we control end to end" or "a better agent than opencode"? The OpenAI Codex fork
the maintainer mentioned alongside it answers only the second.

### 4. Item 5, `authenticate`

Untouched. `.fork/next.html` item 5 and T21.3d have it. Half needs no
credential: when `session/new` is refused and the agent listed `authMethods`,
say so in the agent's own words and name the variable.

---

## Two corrections made on 2026-09-09 that a reader of older files will hit

- **"The fork's first outbound request to a party that is not the person's own
  agent"** appears in older drafts of `specs.rs`, T21 and `next.html`. It reads
  as a claim that the machine makes no third-party requests, which is false —
  the agent's process makes many. It meant **the first host Warp's own HTTP
  client dials by Warp's own choice**. Corrected in all four places.
- **`http_client::Client::get` fingerprints the machine to every host**, not
  just Warp's: client id, app version (`v0.fork.<sha>`, which names the commit)
  and four OS fields down to the kernel version, because
  `include_warp_http_headers` is unconditionally `true` off wasm. Use
  `get_without_warp_headers` for anything that is not Warp's. It still goes
  through `execute_inner`, so the egress policy sees it.
