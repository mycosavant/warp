> Ticket T14, split out of `.fork/TASKS.md` on 2026-09-05. History; read the section you came for.

## T14 — ACP as the adapter contract  ← DECIDED 2026-08-27, and it is now a build

> **As filed:** a decide. The reasoning in §4.1 was that the fork speaks the
> *client* side, and every ACP agent — Gemini CLI, Claude Code via Zed's adapter
> — arrives without a per-agent integration. The crate is Apache-2.0, which an
> AGPL work may depend on, and it was "already on this machine" at
> `~/git/agent-client-protocol-main`. The slots exist twice (`CLIAgent`,
> `Harness`). **Unverified:** that the crate's client side is complete enough to
> drive an agent, which is a reading job before it is a building one.

**Four of those sentences were wrong, and the sync that found it took ten
minutes.** The one input nobody thought to check was the dependency's own
freshness — the ticket said "already on this machine" and treated that as
currency.

### What the sync found

`~/git/agent-client-protocol-main` **was not a git repository**. It was an
unzipped snapshot of `main` at **v0.3.0**, file mtimes **2025-09-12**. Because
it was not a clone there was no `git pull` that would ever have corrected it,
and no `git status` that would ever have looked stale. It has been deleted;
fresh clones are at `~/git/agent-client-protocol` (spec) and `~/git/acp-rust-sdk`
(SDK).

| the ticket said | measured 2026-08-27 |
|---|---|
| the crate is at `agent-client-protocol-main` | that path held v0.3.0 from 2025-09-12, **two major versions** stale |
| (implied) the SDK is in the spec repo | the spec repo **deleted the SDK** in `935857f` ("Remove SDK code (#155)") |
| Zed's protocol | moved org to `agentclientprotocol/`, now carries `GOVERNANCE.md`, `MAINTAINERS.md` |
| Apache-2.0 | **still true** — the one premise that held |
| "Gemini CLI, Claude Code via Zed's adapter" | **39 agents** in a stabilized registry |
| the slots exist twice (`CLIAgent`, `Harness`) | both are the wrong seam — see below |

The Rust SDK now lives at `agentclientprotocol/rust-sdk`, published as
`agent-client-protocol` **v2.0.0** (2026-07-23, Apache-2.0), a ten-crate
workspace. The `claude-acp` registry entry lists **Anthropic, Zed and JetBrains**
as authors, so "betting on one vendor's protocol" is no longer the objection it
was when this was filed.

### The unverified input, settled by running rather than reading

The ticket called this "a reading job before it is a building one". It was a
*running* job, and it took two commands.

- **`testy`** (`agent-client-protocol-test`) driven by the SDK's
  `yolo_one_shot_client`: `initialize` → `session/new` → `session/prompt` →
  streamed `AgentMessageChunk` → `EndTurn`. Offline, deterministic, no quota.
- **The real Claude agent**, `npx -y @agentclientprotocol/claude-agent-acp@0.70.0`:
  same loop, answered on the maintainer's own subscription with **no Warp
  account involved**, `EndTurn`, $0.21.

So the client side is not merely "complete enough" — it is a builder API, and
the whole client in the SDK's example is 113 lines including clap and comments.

**Three things came back that the fork has no equivalent for today**, and they
are the actual argument for adopting ACP:

- `Implementation { name, title, version }` — the agent **identifies itself**,
  which is precisely what replaces a closed `Harness` enum.
- `UsageUpdate { used, size, cost, rateLimit }` — live token, context, cost and
  rate-limit status *pushed from the agent to the client*.
- `AvailableCommandsUpdate` — the agent's own slash commands, enumerated.

**And the gate is open.** `cargo add agent-client-protocol@2.0.0 -p warp_cli`
resolves in **14 new packages**, disturbing existing deps only by a `futures`
patch bump (0.3.31 → 0.3.34), and `cargo check -p warp_cli` compiles it in 13s.
`cargo check --workspace --all-targets` passes with it added. That was the one
claim the advisor could only read; it has now been run.

**The one cost measured rather than assumed:** that `futures` bump deprecates
`UnboundedReceiver::try_next`, which **upstream** code uses — 4 new warnings in
`app/src/ai/blocklist/orchestration_event_streamer.rs`. Confirmed by diffing
against a same-session baseline: **0** occurrences before, **4** after. Warnings,
not errors, and the fix is `try_recv`; but it means adopting ACP puts warnings in
files the fork does not own, which is a merge-noise cost the dependency count
alone does not show.

### The decide

**ACP is the adapter contract for every agent that is not Claude;
`app/src/ai/local_agent/` stays the Claude path.** Reaching Claude over ACP means
an `npx`-launched, `license: proprietary` shim in front of a CLI this fork
already drives directly — a regression on the flagship agent for a fork whose
thesis is the user's own subscription with no intermediaries. Verified: `claude`
2.1.247 has **no `--acp` flag**.

**The premise that survives the challenge: ACP does not delete `translate.rs`.**
The expensive half of the fork's agent work is the *output* side — Warp's
`ResponseEvent` mutation log. In `app/src/ai/local_agent/translate.rs` (830
lines) the Claude wire types are lines 35–262, ~28%; the other ~72% is
Warp-protocol construction that any input front-end still needs. **ACP is a
better input, not a smaller problem.**

**Both named slots are the wrong seam.** `CLIAgent`
(`app/src/terminal/cli_agent.rs:140`) is terminal *decoration* — prefixes, icons,
brand colours — with no transport behind it to swap. `Harness`
(`crates/warp_cli/src/agent.rs:227`) is a cloud-run selector whose local half
reaches `ThirdPartyHarness::build_runner`; that *is* an adapter slot, and it is
the expensive one — **8,148 lines for three agents**. Upstream hand-wrote drivers
for 3 of the 39 registry agents; `CLIAgent` names 15 third-party agents, of which
**13 are in the registry**.

**The seam is the one the fork already owns** — `app/src/ai/agent/api/impl.rs:20`,
the single `if` in front of `generate_multi_agent_output` that T5 opened. An ACP
agent is a second arm of that same condition, behind `fork::acp_agent_enabled()`
next to `local_agent_enabled()` at `app/src/fork.rs:196`, with
`app/src/ai/acp_agent/` as a sibling of `local_agent/`. The reuse is **not** free:
`init()`, `add()`, `message()`, `timestamp()` lift cleanly, but `assistant()`
takes a Claude-typed `AssistantMessage` — a deliberate ~400-line extraction, not
a split.

**Schema v1, and not because v2 is alpha.** `docs/protocol/v2/migration.mdx` is
unambiguous: **v2 removes `fs/read_text_file`, `fs/write_text_file` and all five
`terminal/*` methods from the client**, replacing them with client-provided MCP
servers. Those are exactly the capability that makes ACP interesting to a
*terminal*. Maturity is a free second argument — all 39 agents ship v1 today, and
in the SDK v1/v2 is a cargo feature (`unstable_protocol_v2`), so v2 later is a
flag rather than a rewrite.

**The registry: consume it as data, never as an installer.** The `distribution`
block carries per-platform GitHub release URLs with sha256 — implementing it
means downloading and executing third-party binaries from the network, on the
project whose thesis rests on `crates/http_client/src/egress.rs`. Use it for
*recognition and configuration* of an agent the user already installed: the `id`,
the display name, and the args that put it in ACP mode (`--acp` for Gemini, `acp`
for goose and OpenCode). A **vendored snapshot refreshed deliberately**, not a
fetch at startup.

### The prize, which is bigger than "more agents arrive free"

`local_agent/mod.rs`'s own doc comment names its limit: Claude runs its own
tools, so Warp's diff review, command approval and block UI *do not participate*.
ACP v1's client side — `session/request_permission`, `fs/read_text_file`,
`fs/write_text_file`, `terminal/create|output|release|kill|wait_for_exit` — is
exactly that surface, **as a published spec**. Warp is a terminal with a
permission model and a diff reviewer. This is the one route where tool
participation is a document to implement rather than a private protocol to
reverse-engineer per agent.

- [x] **T14.1** The probe, and no app surface: a hidden `--warpctrl` subcommand
      (**not** a catalog action — that pays the two-test pin tax for something
      whose job is to be deleted or promoted) that runs `initialize` →
      `session/new` → `session/prompt` and prints every `SessionUpdate` as JSON.
      ~80 lines. Its real output is **the mapping table** the app work needs:
      real `SessionUpdate` variants matched arm-by-arm against `translate.rs`'s
      existing `ResponseEvent` constructors. `testy` is the gate; the npx Claude
      shim is the evidence. Composes with `graph.rs` for free — an assertion is
      a command, so the probe is directly assertable in a plan node.

**Decided by the maintainer 2026-08-27:** (A) **No** — "no intermediaries" does
not tolerate an npx-launched proprietary shim on the Claude path, which is what
buys the two-path split above. (B) remains open, and is now sharper than when it
was asked; see T14.1's as-built.

### T14.1 — as built

**The probe is `warpctrl acp probe`**, hidden, not a catalog action, a sibling of
`mcp` and `completions` in `ControlCommand`. `crates/warp_cli/src/local_control/acp.rs`,
~170 lines with the doc comments, plus 7 tests. It runs `initialize` →
`session/new` → `session/prompt` and prints one JSON object per line.

**No async runtime was added, and one nearly was.** `warp_cli` deliberately has
none — `mcp.rs:17` says so — and the SDK's own example is `#[tokio::main]`, which
made it look like adopting ACP meant adopting a runtime. It does not:
`agent-client-protocol` reaches the OS through `async-io` and `blocking`, both of
which drive their own threads, so `futures::executor::block_on` hosts the entire
exchange. Reading the SDK's *example* would have given the wrong answer; reading
its `Cargo.toml` gave the right one.

**Permission requests are denied unless `--approve` is passed**, following the
asymmetry from T11.5 and T13.3 — saying no can only ever make less happen.

**What running it against a real agent found, and it changes (B).** Against
`npx -y @agentclientprotocol/claude-agent-acp` in a scratch directory, asking it
to list files and read one: **20 updates, and exactly five variants** —
`usage_update` (8), `tool_call_update` (6), `tool_call` (2),
`available_commands_update` (2), `agent_message_chunk` (2). `tool_call` carries a
vendor-neutral `kind` (`execute`, `read`) with the vendor's own name tucked in
`_meta.claudeCode.toolName`, so Warp could render blocks by kind **without
knowing which agent it is talking to**. That is the mapping table, measured
rather than guessed.

**And zero permission requests arrived — while the agent ran `ls -la` and read a
file.** That is the finding. Because the probe advertises no `fs/*` and no
`terminal/*` client capabilities, Claude simply ran its own tools and reported
what it had done. So the open question (B) is not "should the fork implement the
client side eventually" but **"without it, ACP buys nothing over `local_agent`"**
— it delivers the same read-only view of an agent doing its own thing that
`local_agent/mod.rs`'s doc comment already names as its limitation. The
ecosystem argument (39 agents, no per-agent integration) survives intact and is
the reason to adopt; the tool-participation argument is entirely contingent on
(B), and this run is what proves it rather than assuming it.

**Named unverified.** ~~The `--approve` path has **never been exercised against a
live agent**, because no agent asked.~~ **Exercised in T14.2, and it was broken**:
it selected `options.first()`, which the flagship agent makes *Deny*, so the flag
refused while reporting success. Naming the unverified input is what got it
tested; testing it is what found the bug. ~~And none will until the fork advertises
the capabilities that make asking meaningful.~~ **That second clause was wrong,
and T14.2 falsified it the same day**: asking is not capability-gated at all.
The agent asked with **zero** capabilities advertised, as soon as its own config
said to. Corrected below. Nothing here has run on Windows. And the mapping table
is one agent, one prompt: `usage_update` dominating the traffic is a
Claude-wrapper trait that may not generalise, and no `plan`, `diff` or
elicitation update appeared at all.

### T14 (B) — answered 2026-08-27: the execution half is dead, the consent half is the ticket

The T14 decide left open *"does the fork implement the ACP client's `fs/*` and
`terminal/*` side at all"*, called it the largest piece, and said it deserved its
own gate. It got measured instead of scoped, and the answer is **mostly no** —
which removes the biggest and most dangerous piece of work from T14 while leaving
the ecosystem argument untouched.

**The premise neither of us checked, and it is the whole answer.** The spec says
what these methods are *for*. `docs/protocol/v1/file-system.mdx:6` — they *"enable
Agents to access **unsaved editor state**"*. `docs/protocol/v1/terminals.mdx:6` —
they let a client *"run build processes… while providing real-time output
streaming"*. **Both describe a client that Warp is not.** Warp is a terminal: it
has no dirty buffer for an agent to read, and it already has PTYs and blocks. So
`fs/read_text_file` served by Warp is a *strictly worse* version of the agent's own
Read tool — the same bytes off the same disk, plus a round trip — and `terminal/*`
is a substitute for the thing Warp already is.

| | verdict |
|---|---|
| `fs/read_text_file` | **No.** Its designed purpose is unsaved editor state, which Warp structurally lacks. Worse than neutral: Gemini *prefers* an advertised client fs, so advertising it would actively divert reads through Warp for bytes Warp did not improve. |
| `fs/write_text_file` | **No.** Its one surviving motivation was diff review, and the diff arrives on the permission channel instead — typed, with `locations`, *before* the write. What Warp would gain is that its own hand applies the bytes; the user sees the same diff and gives the same consent either way. Not worth an unconfined absolute-path write surface. |
| `terminal/*` | **No, and closed.** Zero uses found in four agents. All-or-nothing in the schema, arbitrary `command`/`args`/`env`/`cwd`, removed in v2, and Warp is the thing it exists to substitute for. Do not advertise it even in the probe. |
| `session/request_permission` | **The live candidate → T14.2.** |

**What was run.** A throwaway client (`/tmp/acpcap`, outside the fork) that
advertises `fs.read`, `fs.write` and `terminal: true`, logs every client-side
request and refuses every mutating one.

1. **The client side works.** Against `testy` with prompt `callbacks`, all four
   fired: `session/request_permission`, `fs/read_text_file`, `fs/write_text_file`,
   `terminal/create`. ~150 lines, no runtime. "Can the fork be a first-class ACP
   client" is answered yes.
2. **The flagship agent used none of it.** Real `claude-agent-acp` run: it read a
   file, wrote `hello.txt`, and ran `echo done` — all through **its own tools**,
   zero client-side calls, despite all three capabilities being advertised. Its
   dist contains `createTerminal` **0** times.
3. **Confinement is not theoretical.** The first agent to exercise the path asked
   to write `/tmp/testy-write.txt` and read it back — outside the session
   directory — and wanted `terminal/create` with `cwd: /tmp`. It was
   *protocol-conformant* doing so: the request paths are absolute `PathBuf`s with
   nothing tying them to the session.
4. **Three of four agents do consume client `fs/*`** — Gemini
   (`packages/cli/src/acp/acpFileSystemService.ts`, gating on
   `capabilities.readTextFile` with a fallback), goose
   (`crates/goose/src/acp/fs.rs`; `server.rs` gates on
   `client_fs_capabilities.read_text_file`, with tests), opencode
   (`acp/permission.ts` types `writeTextFile`). **None consume `terminal/*`** —
   goose's `acp/` directory has `fs.rs` and no terminal module at all. Demand is
   real and it does not change the verdict: what those agents want from a client
   fs is the unsaved-buffer case, which is exactly what Warp cannot give.

**A grep falsifies but cannot confirm.** `readTextFile` appears 6 times in
Claude's dist and fired zero times. So a **0** is decisive and a nonzero means
only "worth one real run". Both directions above are labelled accordingly.

- [x] **T14.2** `session/request_permission` → Warp's permission model. **The
      thing worth building was hiding inside (B) and is much smaller than it.**
      Ungated, typed, agent-agnostic, and it needs **zero capabilities
      advertised**. Verified by running: with `{"permissions":{"defaultMode":"default"}}`
      in the session's own `.claude/settings.json`, the same prompt that
      previously wrote a file silently instead produced a permission request
      carrying the **whole diff** — `content: [{type: "diff", path, newText}]`
      plus `locations` — and the probe's denial **held**: no file was written,
      and the agent said so (*"the write to probe.txt was blocked"*).
      Compare the existing path, which pattern-matches a prompt drawn in a
      terminal and answers it with `\r`/`\x1b` bytes for exactly one agent
      (`approvals.rs:83-84`, gated by `ALLOW_VERIFIED_AGENTS`, whose central
      claim is still unverified in T15). This replaces a screen-scraping
      heuristic with a typed request that works for all 39.
      **Gates:** refuse `allow_tools` on an ACP node rather than accept and drop
      it — the `agent.spawn --allow-tools` chain has no ACP link and cannot have
      one, and silently ignoring it is `local_agent/tools.rs:17-20`'s own stated
      nightmare, *"worse than no allowlist, because it reads as a guarantee"*.
      Gate on the **method**, never on `tool_call.kind`, which is agent-authored.
      **And remote approval must be able to select only single-shot options.**
      `allow_always` carries `_meta.permission.changes` setting the session's
      permission mode to `acceptEdits` with `lifetime: {scope: "session"}` — so
      composed with `WARP_FORK_REMOTE_APPROVE`, one phone tap would authorize
      every subsequent call the person will never be shown. Read
      `PermissionOption.kind`; allow `allow_once`/`reject_once` remotely and
      never `allow_always`. Same asymmetry the fork already uses: a remote *no*
      is always safe, a remote *yes* should be as small as possible.

### T14.2 — as built

**The ticket's own first line was the thing that failed. `--approve` denied.**

T14.1's probe answered an approval by taking `options.first()`. Run against live
`claude-agent-acp` on 2026-08-27, the options arrive in this order:

| # | `optionId` | `name` | `kind` | |
|---|---|---|---|---|
| 1 | `reject` | Deny | `reject_once` | |
| 2 | `allow` | Allow Once | `allow_once` | |
| 3 | `allow_always` | Always Allow | `allow_always` | `_meta.permission.changes` → set `claudeCode` mode to `acceptEdits`, `lifetime: {scope: "session"}` |

**Deny is first.** So the flag named `--approve` refused: no file was written and
the agent said *"I wasn't able to create the file — the write was denied."* It
reported success while doing the opposite of its name, against the one agent it
had ever been run on, in a fork whose thesis is that a person's consent is the
thing being protected. Nothing in the protocol orders that list and nothing ever
will — the order is a fact about one agent's UI, exactly like which option a TUI
highlights. **The point of the typed channel is not that the order is knowable;
it is that the order does not have to be known.**

It was found by running it, and only by running it: the schema, the docs and the
option *names* are all silent on order, and the previous session had read the
right types and still shipped the bug. It also falsifies T14.1's named-unverified
input in the strongest available way — the input was "`--approve` has never been
exercised", and exercising it is what broke it.

**What was built.** `crates/warp_cli/src/local_control/acp_permission.rs`, ~110
lines with the doc comments, plus 9 tests whose fixture is the option list above
transcribed field for field, order included. One pure function:
`choose(&RequestPermissionRequest, Decision) -> Choice`.

- Selects by `PermissionOptionKind`, **never by position**.
- Refuses both always-variants, and additionally refuses any option declaring a
  **non-empty** `_meta.permission.changes` at a version this build knows, or
  carrying a `permission` block at any *other* version — two independent signals,
  either disqualifying. `_meta` is read only ever to *refuse*, never to grant,
  which is what keeps it clear of the spec's *"implementations MUST NOT make
  assumptions about values at these keys"*: no assumption is made about what a
  change means, only that an option declaring one does more than answer the
  question.
- The line that makes that a principle rather than a rationalisation:
  **an option may only be selected by a surface capable of showing what that
  option declares.** A single-shot option declares only the tool call, which
  every surface renders. An option declaring a *transition* cannot be shown by a
  non-interactive `--approve` or a phone card before the tap, so selecting it
  there would authorize something the person was structurally never shown. An
  in-app picker that renders the declaration could legitimately offer it; nothing
  that exists today can.
- Keyed on `changes` being non-empty rather than on the `permission` block
  existing, because `_meta` is free-form and an agent may reasonably decorate
  every option with benign permission metadata; refusing all of those would break
  ordinary approvals, which is how a safety rule gets switched off by whoever it
  inconveniences. **Unmeasured:** whether any agent does that. One agent has been
  watched. So the narrow rule, plus fail-closed on an unknown version — where an
  absent `changes` list may only mean this code looked in the wrong place.
- **What this is not, said in the module rather than assumed.** It is not a
  boundary against a hostile agent: `PermissionOption.kind` is exactly as
  agent-authored as `tool_call.kind`, which the ticket forbids gating on, and a
  hostile agent does not ask at all — T14.1 measured that, with `defaultMode:
  auto` producing writes and commands and no questions. It defends against
  *honest* agents: arbitrary option order, escalation offered by default, and a
  kind that understates what its option does. And **absence of a declaration is
  never a guarantee** — the kind gate is what admits an option; a declared change
  is only ever an extra way to reject one.
- An allow that cannot be expressed safely **becomes a no**, and the reason names
  what the agent offered — because "no single-shot allow was on offer" is
  invisible otherwise and indistinguishable from a bug in this code.
- A deny with no `reject_once` still answers, as `Cancelled`.

**And the deny got more honest on the way past.** It now sends `reject_once`
rather than `Cancelled`. `Cancelled` is documented as the answer to a
`session/cancel`, so using it for "the person said no" claims a turn-cancellation
that did not happen — the same class of lie as `approvals.rs` refusing to report
`approved: true` when all it did was press a key.

**Re-measured, both directions, against the live agent with the rebuilt binary:**
`--approve` → `{"outcome":"selected","optionId":"allow"}` and `probe.txt`
contains `hello`; default → `{"outcome":"selected","optionId":"reject"}` and no
file. The probe now emits a `permission_answer` record so the transcript says
what was sent, not just what was asked.

**A claim made and withdrawn in the same session.** I argued the digest is
redundant here because *"`toolCallId` covers the staleness case for free"*. That
is wrong, and the advisor's attack lands: **nothing in the schema says one
`toolCallId` carries at most one permission request.** An agent refused
`reject_once` may re-ask on the same `toolCallId` with modified content, and an
answer keyed by it would land on the second question — the `approval_for` doc
comment's exact failure, one layer up. The structurally unique key is the
**JSON-RPC request id**: one per ask, gone once answered, a stale answer fails
loudly. The probe is safe because it answers through the responder, which *is*
that channel. So the honest sentence is **"the request id makes the digest's job
structural"**, and the digest is redundant *conditional on the key choice*.
Unverified either way: neither of us has watched an agent re-ask after a reject;
the schema merely fails to forbid it.

**Where T14.2 stops, and why that is the answer rather than the leftovers.** It
stops before any app surface. The ticket said *"→ Warp's permission model"*, and
measuring what that would mean is what shortened it:

- `PendingApproval.approval_id` is a **pane id** and `agent.approvals` walks
  `surface_locations`. An ACP session has no pane, so the shape's primary key
  does not exist for it.
- `PendingApproval` is derivable from one blocked-session snapshot — `approval_for`
  is deliberately a pure function. An ACP approval record **cannot be**: the
  `permission_request`'s `toolCall` carried no `_meta` at all, while the
  `tool_call`/`tool_call_update` notifications for the same `toolCallId` carried
  `_meta.claudeCode.toolName: "Write"`. The vendor tool name is only available by
  correlating the request against the notification stream. A shape needing
  stateful correlation is a different shape by construction.
- `AgentApproveParams.digest` is required with no opt-out, so routing ACP answers
  through `agent.approve` would force either a fabricated digest or a loosened
  pin.

So what carries forward into the app work is the fork's **consent vocabulary** —
the approve/deny split with its grant asymmetry, the single-shot remote gate, the
answer-must-bind-to-question principle — and **not** the record shape. Filed as
T14.4 rather than smuggled in here.

**The `allow_tools` gate was not exercised and did not need to be.** It applies
to an ACP *node* in the task graph, and there is none: graph nodes spawn agents
through `agent.spawn`, which has no ACP link. Recorded on T14.4 rather than
silently dropped.

- [x] **T14.4** ~~The app surface, when there is a reason for one.~~ **The two
      measurements this ticket named as prerequisites were run first, and the
      first one found a live consent hole in the module T14.2 had just built —
      so T14.4 became that fix, and the app surface is now T14.5.** As built
      below. The constraint list survives intact except where the measurement
      contradicts it, which is called out in place.
      **a new approval shape** in `crates/local_control/src/protocol.rs`, sibling
      to `PendingApproval` and decided next to its consumer — not `PendingApproval`
      with a session id smuggled into a field documented as a pane id.
      **Key the pending map on the JSON-RPC request id**, not `toolCallId` and not
      the session id; key it on either of those and the staleness hazard returns
      and a digest is needed again.
      **The app-side shape must keep the diff structured** (`content: [{type:
      "diff", path, newText}]`), because an in-app diff reviewer is the actual
      prize; the *remote consent card* may flatten it to text, which is still
      strictly better than the OSC path, where the person sees the file name and
      nothing about the contents.
      **Refuse `allow_tools` on an ACP node** rather than accept and drop it —
      `local_agent/tools.rs:17-20`'s stated nightmare, *"worse than no allowlist,
      because it reads as a guarantee."*
      ~~**Gate on the method, never on `tool_call.kind`**, which is agent-authored
      and whose unknown case silently becomes `Other` via `#[serde(other)]`.~~
      **Half-false, and the measurement below is what showed it.** There is no
      method to gate on — everything arrives as `session/request_permission` — so
      as written the rule forbade the only defence available against the hole
      T14.4 found. It is sound about *granting*: an unrecognised kind becomes
      `Other`, so anything granting on a kind grants on the default. Used to
      **refuse**, the same degradation runs the safe way. Corrected to: **never
      grant on `tool_call.kind`.**
      **Provenance-tag every agent-authored string** — mode id, mode description,
      option name, tool title — and render **no session-level governance
      indicator**, which now explicitly includes the declared mode (T14.3: it
      does not predict per-call gating, measured).
      **A mode picker is the first legitimate declaration-rendering surface.**
      `acp_permission.rs` refuses `allow_always` because nothing that exists can
      show what it declares; an in-app picker showing the agent's own mode
      descriptions is the surface that could, and `session/set_mode` is how it
      would act. Requesting a *stricter* mode is safe under the fork's asymmetry,
      but its effect is unverifiable — so it may only ever be reported as
      *"requested `default`; the agent acknowledged"*, never as *"protection
      enabled"*.
      ~~**To measure first:** whether the already-deployed `console.js` renders an
      approval entry with unfamiliar fields gracefully.~~ **Answered by reading,
      and it was the wrong question — see the as-built.**

### T14.4 — as built

**The ticket was the app surface. Running its own prerequisites first turned it
into a consent fix, because the first measurement found `--approve` authorizing
a session-wide policy change.** Same shape as T14.2, in the module T14.2 built,
two commits later.

**What was built to measure with: `--mode <id>`.** `session/set_mode` was
schema-only at the end of T14.3 — named unverified, and the half of the mode
picker T14.4 was told to measure first. It is a flag on the probe that sends one
before the prompt. If the agent refuses, the probe **stops rather than
prompting**, because the person named a policy and running under a different one
is exactly the `--approve` failure T14.2 fixed.

**The agent honours it, and says nothing.** Measured 2026-08-27:
`--mode plan` was acknowledged, and behaviourally obeyed — the agent wrote a
plan file and then asked to leave plan mode. It sent **no `CurrentModeUpdate`**.
So T14.3's "the agent says its mode out loud" is sharper than it was written:
it says so **at `session/new`**, and re-announces when a *person* asks it in
prose — but not when the *client* uses the protocol's own mode API. **Warp is
less sighted the more it participates**, which is the opposite of what a
governance surface would assume.

An acknowledgement is also nearly empty by construction: `SetSessionModeResponse`
has **no fields**. A success is one bit. T14.4's own constraint already said a
mode request may only be reported as *"requested; the agent acknowledged"* — this
is why, and it is stronger than the constraint assumed.

**Then the hole.** `ExitPlanMode` arrives as a permission request whose tool call
is stable-v1 `kind: "switch_mode"` and whose five options are the session's mode
ids:

```
bypassPermissions  "Yes, and bypass permissions"       allow_always
auto               "Yes, and use \"auto\" mode"         allow_always
acceptEdits        "Yes, and auto-accept edits"        allow_always
default            "Yes, and manually approve edits"   allow_once     ← selected
plan               "No, keep planning"                 reject_once
```

**Not one of the five carries `_meta`.** So `acp_permission::choose` — which
finds the first `allow_once` carrying no declared change — selected `default`.
Watched on the wire: `{"outcome":"selected","optionId":"default"}`, then the
agent left plan mode and wrote the file the person had asked it to only plan.
`--approve` promises "once each" and had just set the session's permission mode.

The module's own docs predicted it in as many words — *"absence of `_meta` proves
nothing; the moment any path here reads 'no declared change, therefore safe', the
forbidden assumption has been made in the granting direction"* — and `choose` did
that anyway. **Third instance in T14 of writing a hazard down and then building
against it** (T14.2's re-ask, T14.3's boolean, this). The lesson is not learning
itself, so it is now stated as a rule: *a hazard in a doc comment with no test
under it is a hazard that is not defended.*

**And it is the spec's own shape, which is the rare case here of a finding that
generalises past one agent.** `docs/protocol/v1/session-modes.mdx`, under
*"Exiting plan modes"*, documents this exchange down to an option named *"Yes,
and manually accept actions"* typed `allow_once` with no `_meta`. So every ACP
agent with a plan mode is expected to present a policy change this way. Found
only after the fix was written, by grepping the spec for the thing the
measurement had already shown.

**The fix, and what makes it more than a patch.** The generalisation is that **the
question can be the problem**: when an agent asks *which policy should apply*
rather than *may I do this one thing*, no option is single-shot whatever its kind
says. These options declare their transition in their **names**, in English —
disclosure to a person, nothing at all to a flag.

**The first fix was a denylist of one, and that was the same trap again.** It
refused `SwitchMode` and allowed everything else — *"not the signal, therefore
safe"*, one field over from *"no `_meta`, therefore safe"*, and `#[serde(other)]`
makes it silent: an agent whose mode switch arrived as `execute`, as a kind added
in a later schema, or with no kind at all would have gone straight through. The
doc comment even named that degradation and called it acceptable, which is the
rule promoted three paragraphs ago being broken in the act of writing it down.
Caught by the advisor rather than by a run, and the fix is an **allowlist**: an
option may be selected only for a kind whose spec meaning stops at the call —
`read`, `edit`, `delete`, `move`, `search`, `execute`, `think`, `fetch` — with
everything else falling off the end of a `matches!` rather than being enumerated.
`delete` and `execute` are on the list on purpose; the test is whether the effect
is *bounded*, not whether it is gentle.

Its cost is named rather than hidden: an honest agent whose ordinary calls arrive
as `other` gets refused, **loudly, with the kind in the message**, because an
allowlist's wrong answers have to be explicable on sight — a person concluding
"the flag is broken" is exactly what T14.2 cost. A wrong refusal costs a message;
a wrong grant costs the session's policy. The amended constraint, replacing
T14.4's absolute: **a kind may disqualify, never qualify — and a kind this build
does not recognise must not qualify either.**

Denial is untouched throughout, and correctly: *"No, keep planning"* is a
well-formed no, and declining a change leaves the session where it already was.

**Two corroborating signals were seen and deliberately not built on**: every
`optionId` equals a declared `availableModes` id, and the request offers *three*
distinct `allow_always` options where an ordinary write offers one. Both fail in
both directions — the spec gives `optionId` no semantics, and "always for this
file / always for all files" is a plausible ordinary pair — so they are wire-facts
worth observing and nothing may rest on them.

**Two report defects, from the same three runs.**

- **`mode_the_agent_declared` was wrong and is deleted.** It printed `auto` for
  the session measured above, which was in `plan`. There is now **no current-mode
  field**, because Warp does not know the current mode: what it has is the
  opening declaration, the announcements since, and what it asked for — three
  facts, and a reader composing them can at least see where it is uncertain.
- **The ledger laundered Warp's own action as the agent's.** After `--approve`
  selected *"Yes, and manually approve edits"*, the agent announced
  `{"from":"auto","to":"default"}` and the report recorded
  `warp_requested_it: false`, documented as *"the agent widening or narrowing
  itself"* — the rug-pull sentence, printed over Warp's own doing. Renamed to
  `answers_a_set_mode_warp_sent`: named for the message Warp sent rather than for
  who moved the mode, because that is the part Warp can check. The fix to
  `choose` puts the case out of reach *from this binary*, which is a reason to
  correct the record rather than a reason not to — T14.5 reaches it again.
- **`transitions_offered` was blind to the transition that mattered.** The live
  `ExitPlanMode` run reported `transitions_offered: []` for a session whose one
  event was a five-option policy menu, because the list was fed only from
  `_meta.permission.changes` and none of the five had any. So the report said no
  transition was offered in the session where one was. The predicate is now *"would
  selecting this do more than answer the question"* — which on an unbounded call
  is every way of saying yes — and `readable: bool` became a three-state
  `disclosed_as`, because a boolean cannot tell *"a declaration this build cannot
  parse"* from *"no declaration at all"*, and after this measurement the second is
  the common case. `reject_once` is excluded deliberately: recording a refusal
  would have printed *"authorized by Warp: No, keep planning"*.

**The last two were found by the advisor rather than by a run**, reading the
shipped code against the measurements. Worth recording as a method note: the
runs found the hole, and a reader found that the first fix for it repeated the
hole's own shape.

**The second prerequisite dissolves, and it was the wrong question twice over.**

First, the premise. *"The already-deployed `console.js`"* presupposes a
deployment model the console does not have. All four routes are served
`Cache-Control: no-store` (`console.rs:138`, pinned by a test), and there is no
service worker — a LAN address is not a secure context, which `console.rs:111`
already says. **The script and the shape ship in the same binary; a `console.js`
stale relative to its server cannot exist.** The one stale client that can is an
old `warpctrl`, and `AgentApprovalsResult` has no `deny_unknown_fields`, so serde
ignores a new sibling field. The ticket's single named "thing that would reopen
the decision" cannot trigger.

Second, even granting the premise: `console.js` cannot throw on an unfamiliar
approval — every field access in `approvalRow` is guarded by `||`, an `if`, or a
`=== 'question'` comparison, with no field enumeration and no `switch`. But that
was never the risk either. `answer()` hardcodes `agent.approve`/`agent.deny` with
`approval_id` and `digest`, pinned by
`an_answer_carries_the_digest_of_what_was_shown`, and T14.2 established that
`AgentApproveParams.digest` is required with no opt-out. So the deployed console
can only answer an approval that fits the `agent.approve` contract — the
constraint is about the **answer path, not the fields**, and an ACP approval
exposed through `agent.approvals` would draw a Yes button that cannot be
honoured.

Named unverified: both halves are read, not run. The reload check (`no-store`
honoured by a real browser) was not done, and producing a real unfamiliar
approval needs the app change T14.5 has not made.

**Named unverified.** One agent, three prompts, all on Linux/WSL. `--approve`
has still never met an agent offering only always-variants. Whether any agent
other than `claude-agent-acp` uses `switch_mode` for a policy question is
unknown, and an agent that labels one `edit` is not caught — which the module
says out loud rather than claiming a boundary it does not have.

- [x] **T14.5** The app surface, when there is a reason for one. Inherits every
      constraint listed under T14.4 above, as corrected there, plus what T14.4
      measured:
      **First and most load-bearing: no app surface until one agent the surface
      would host has been probed.** Every ACP measurement this fork has ever made
      — the update-variant table, the option order, the mode list, the
      `_meta.claudeCode` vendor names — is from `claude-agent-acp`, which T14 (A)
      *permanently excludes* from the app surface, because reaching Claude over
      ACP means an `npx` shim in front of the CLI `local_agent/` already drives.
      The surface's actual clientele has been probed **zero** times, and checked
      2026-08-27 none of `gemini`, `goose`, `opencode`, `amp`, `droid`, `auggie`
      or `codex` is installed in this WSL userland (`copilot` resolves, but to a
      Windows npm install reached through PATH interop, and whether it speaks ACP
      is unknown). Building the mapping now bakes one agent's idioms into a
      surface for agents never observed — which T14.1's own as-built flagged and
      the constraint list then quietly forgot. Installing one is a maintainer
      call; **ask before doing it.** ✅ **Discharged 2026-08-28 — see below.**

### T14.5's gate — as measured

`opencode` 1.18.25, installed at a pinned version, driven through **OpenRouter**
with `openrouter/~google/gemini-flash-latest` — deliberately non-Anthropic, so
the probe is independent of everything above on *both* axes, codebase and model.
Four runs, about two cents. `opencode acp` is a **built-in** subcommand; no
adapter. (One trap first: `opencode-acp` on npm is *not* an ACP adapter, it is
"Active Context Pruning" — an unrelated package sharing the acronym.)

**The probe, the ledger and the permission module all handled a second agent with
no code changes.** That is the first real evidence that any of this generalises.

**The finding that settles `acp_permission.rs`'s whole premise.** opencode's
options arrive `once` / `always` / `reject` — **allow first**, where
`claude-agent-acp` sends **deny first**. So T14.1's `options.first()` would have
*approved* on opencode and *denied* on Claude, from one line. Two agents,
opposite orders, neither wrong. "Choose by kind, never by position" stops being
an argument and becomes a measurement.

**Its `always` carries no `_meta` at all**, so only the kind gate refuses it —
second confirmation that the declaration rule is an extra way to reject and never
the load-bearing one.

**The allowlist's named falsifier was run and passed.** T14.4 shipped it saying
*"if a real agent's routine calls arrive as unknown kinds, `--approve` becomes
useless there and the rule retreats"*. opencode's calls carry `kind: "read"` and
`kind: "edit"`, both on the list; `--approve` selected `once` and the file was
written, and without it the probe selected `reject`. The rule stands.

**It declares no modes at all.** `mode_the_agent_declared_at_session_start:
null` — the "third state" T14.3 wrote a defensive test for against a schema
`Option` is real, not hypothetical. It vindicates refusing to act on a declared
mode: a policy keyed on the claim would have had nothing to read here, and
**punishing declaration relative to silence** would have rewarded opencode for
saying less. It uses `configOptions` for model selection instead.

**And it falsified a sentence in the shipped report.** The caveat said a call
Warp was not asked about *"was most likely allowed by a rule the user wrote
deliberately"*. On a fresh opencode install with no user configuration, it wrote
a file and asked nobody — **nobody had written a rule**. Claude's silence came
from the user's own 87 allow rules; opencode's came from its own defaults, and
nothing on the wire distinguishes the two. The caveat now refuses to guess, with
a test pinning that it refuses.

**Its diff is richer than Claude's**: `content: [{type: "diff", path, oldText,
newText}]` — with `oldText`, which Claude omits. T14.4's constraint *"the
app-side shape must keep the diff structured"* holds across both, and the in-app
reviewer would get more from opencode, not less.

**Named unverified.** One model behind opencode, four prompts, Linux/WSL. Its
`--auto` flag and the rest of its permission vocabulary were not exercised; the
`ask` behaviour above needed a project-local `opencode.json`, which is a config
this fork wrote, not one a user would have. No `session/set_mode` was attempted
against it, because it declares no modes to set.
      ~~**A picker is the first legitimate declaration-rendering surface.**~~
      **Struck: measured dead, on two counts.** Its display goes stale exactly
      when it acts — `set_mode` produces no announcement — so it cannot render
      honest state after its own use. And it is the wrong *channel*:
      `acp_permission.rs`'s "nothing that exists today can show what the option
      declares" is about an option on a live `session/request_permission`, which a
      picker acting through `session/set_mode` can never answer. **The successor
      surface is an interactive approval card**, and it now has a concrete thing
      to render: the `switch_mode` menu, whose English option names are the only
      place the transition is disclosed at all (`disclosed_as:
      the_options_name_only`). That is what would let a person answer the request
      `--approve` now refuses.
      A picker may still exist as a *display* surface. It may only ever report
      *"requested `default`; the agent acknowledged"* — never *"protection
      enabled"* — because `SetSessionModeResponse` has no fields.
      **The report's mode fields are the layout to start from**, including the
      absence of a current-mode field. Any app surface that reintroduces one is
      reintroducing a measured falsehood.
      ~~**Record `CurrentModeUpdate` with whether Warp requested it.**~~
      **Falsified as written**: a Warp-requested transition produces *no*
      `CurrentModeUpdate`, so that field's `true` case cannot occur on the API
      path. The fact belongs on the `set_mode` send record, which is where
      `mode_requests_warp_sent` now puts it.
      **`answers_a_set_mode_warp_sent` becomes insufficient the moment the app can
      answer a policy question**, because then Warp moves the mode by two
      different routes and the record needs to say which. An ordered event list
      across all three sources — Warp's `set_mode` and its one-bit ack, the
      agent's announcements, and transitions Warp itself authorized — is the shape
      to reach for; the two separate lists shipped in T14.4 are each true but
      cannot show the interleaving.
      **`refuse allow_tools on an ACP node` is filed, not active.** It is
      conditional on an ACP graph-node type, and none exists; it binds whoever
      adds one.


### T14.5 — as built

**An ACP agent answers in Warp's agent panel.** Measured 2026-08-28, end to end:

```
$ WARP_FORK_ACP_COMMAND="opencode acp" warp-oss          # opencode, on OpenRouter/Gemini
$ warpctrl agent prompt "Read notes.txt and tell me the second line."
`read`
`tmp/t145/project/notes.txt`
beta
```

No account, no `api.warp.dev`, no Claude — a third-party agent driving Warp's
own conversation model, on the user's own key.

**The seam was one branch.** `ai::agent::api::generate_multi_agent_output` already
had the `local_agent` fork in it; this adds a sibling above it. Everything else
is a new module, `app/src/ai/acp_agent/`, which is why T5's finding still pays:
the whole agent surface hangs off one function.

**Naming the command is the switch.** `WARP_FORK_ACP_COMMAND="opencode acp"`, and
deliberately no second flag. Every other predicate in `fork.rs` answers *"is this
permitted"*, which wants a boolean because the behaviour is already specified;
this one answers *"which program"*, and ACP's whole point is that there is no
default. It outranks `WARP_FORK_LOCAL_AGENT` when both are set, because naming a
specific agent is the more specific instruction.

**A spike on the same terms the last one had: every permission request is
denied.** Read-only turns work; anything needing consent does not, and the
refusal is said in the conversation rather than swallowed. Measured:

```
$ warpctrl agent prompt "Create a file called out.txt containing the word hello."
`write`
Warp denied this: **/tmp/t145/project/out.txt**. This build can only say no …
$ ls          # notes.txt  opencode.json — nothing was written
```

That is not a shortcut, it is the shape `local_agent` shipped in and for the same
reason, quoted from its own module docs: *"anything needing approval is denied.
Wiring Warp's tool execution back in … is the next step rather than part of the
spike."* Saying **yes** needs a surface that can show what is being agreed to, and
T14.4 measured what goes wrong when something says yes without one.

**And because it only ever says no, it needs none of `acp_permission`.** That
module's allowlist, its `switch_mode` rule and its `_meta` reading all guard the
*allow* side; refusing is unconditional. So there is no second copy of an allow
rule in the tree. **When T14.6 can say yes, that module must be shared rather
than reimplemented** — which is recorded in `acp_agent/mod.rs` where whoever does
it will be reading.

**Two things found by running it, neither visible from the code.**

- **ACP streams *tokens*, and one message per chunk shreds the answer.** The
  first live turn rendered `"notes.txt doesn"`, `"'t exist in this"`,
  `" directory"` as three separate messages in the panel. Claude's path never
  showed this — `stream-json` delivers whole content blocks — so nothing in the
  fork had met it. Text now accumulates and flushes at a boundary (a tool call, a
  switch between answer and reasoning, the end of the turn), which gives exactly
  the granularity `local_agent` already produces. The protocol *does* have
  `AppendToMessageContent`, built for precisely this; it was not used because its
  `FieldMask` path into the `Message` oneof is produced nowhere in this repo, and
  shipping it would have meant guessing an input and learning from a silent
  failure. On T14.6.
- **The first run answered about the wrong directory**, and that turned out to be
  correct plumbing rather than a bug: the ACP session cwd comes from the *pane's*
  working directory, and a conversation started by `warpctrl agent prompt` gets a
  fresh pane in `$HOME`. `cd` first and it reads the right tree. Worth knowing
  because it is invisible in the transcript — the agent simply reports the file
  is missing.

**The process cwd is a separate thing, and it is Warp's.** `AcpAgentConfig`
carries a command, args and env and **no cwd**, so the agent process inherits
Warp's. The session cwd is carried properly in `session/new` and is what the
agent's *tools* use — measured.

~~What it changes is where an agent looks for its **own** configuration:
`opencode` reads `opencode.json` from the process directory, which is why the
model had to be configured beside where Warp was launched.~~ **False, and
corrected under T14.6 by reading the agent's own log.** `opencode` resolves its
project configuration from the **session** directory. It only looked like the
process directory because Warp had been launched from the same place the pane
was sitting in — the two were never varied independently. What follows from the
corrected version is much worse than a note about where to put a config file,
and is written up in T14.6.

**No task, no thread, no executor borrowed.** `agent-client-protocol` runs a
connection through a scoped `connect_with(agent, |connection| async { … })`,
which is a future; Warp's seam wants a stream. The bridge is an unbounded channel
plus `stream::select`: the driver pushes events into the sender and yields
nothing itself, so **whoever polls the returned stream drives the connection**.
That gives cancellation in the right direction for free — dropping the stream
drops the driver, drops the connection, closes the agent's stdin — and it is the
same runtime-free property the `warpctrl` probe has, for the same reason (ACP
reaches the OS through `async-io` and `blocking`, both self-driving).

**The dependency was checked rather than assumed.** `agent-client-protocol` in
the `app` crate adds the protocol crates, `async-process` and `shell-words`;
`async-io`, `blocking`, `futures`, `serde_json` and `uuid` were already there. It
sits in the existing `cfg(not(target_family = "wasm"))` block, matching the
module's own gate.

**The hazard `local_agent` recorded, this time with a test under it.** A
`ToolCall` message is an *instruction* — Warp's action model executes it — and the
agent has already run the tool, so emitting one runs it twice. T14 produced three
separate instances of a hazard written in prose and then built against, so this
one is pinned by `a_tool_call_is_never_emitted_as_a_tool_call_message` rather than
by a paragraph. Same for the denial: `no_option_that_permits_anything_is_ever_selected`
is what a future "helpful" edit has to come through.

**Two corrections, both from an advisor reading the shipped code and both then
measured.**

**"Read-only" was false, and it was in `CLAUDE.md`.** The first version of this
module, its README section and the seams table all described the ACP path as
read-only because it denies every permission request. But Warp only answers the
questions an agent *asks*, and an agent is free to ask nothing. Run through this
very path with `opencode` at its own defaults and no user configuration:

```
prompt: Create a file called proof.txt containing the word hello.
`write` `tmp/…/proof.txt` File `proof.txt` created with content `hello`.
$ ls    proof.txt          ← written; Warp was never asked, so denied nothing
```

The T5 spike's identical sentence had a real guarantee behind it — `claude -p`
refuses its own tools — and **that guarantee does not transfer**. Worse, the
falsifying measurement was already in hand: T14.5's own gate recorded opencode
writing a file on a fresh install with no rule behind it, and the consent
report's caveat had been corrected *the same day* to stop guessing whose policy
allowed a silent call. The claim was made anyway. That is the **fourth** instance
in T14 of a hazard being written down and then built against, and the first one
where the false claim was a safety guarantee in the file every session loads.

**A second turn was silently answered by an agent with no memory of the first.**
Recorded as "a real limitation" in a doc comment; measured through the panel it
is not a footnote:

```
turn 1  "Create a file called proof.txt…"   → `write`, file created
turn 2  "What word did you just put in it?"
        → "I haven't written to or modified any files yet in this session."
```

Warp shows one continuous conversation to an agent that remembers none of it —
`local_agent`'s own named hazard, *"a true sentence about the wrong
conversation"*, with the roles swapped. A second turn is now **refused out loud**,
because a refusal cannot mislead and an amnesiac answer presented as continuous
can. `session/load` is the fix; `opencode` advertises `loadSession: true`.

Verified live after the fix: turn 1 answers, turn 2 leaves the conversation
`status: "error"` with the refusal reaching the log verbatim. One measured
detail for anyone else returning an error from this seam — Warp classifies
`AIApiError::Other` as **recoverable and retries it three times** before
surfacing it. Harmless here, because `Turn::from_request` fails before anything
is spawned, so the retries cost no process and no tokens.

**And the dependency note was wrong in the harmless direction.** `app` already
depends on `warp_cli` unconditionally, which already carries
`agent-client-protocol`, so the crate was in every binary before this change; the
direct edge adds nothing to the link. Which also means T14.6 can widen
`acp_permission`'s visibility rather than duplicate its allow rules.

**Named unverified.** One agent, one model, six prompts, Linux/WSL. `/compact` is
not handled (the protocol has no compaction) and falls through. ~~**Cancellation
is reasoned from the drop chain and still was not exercised** — the ACP client's
`ChildGuard` is documented to kill the process group on connection drop, on Unix
only, so a cancelled turn on Windows is entirely unverified.~~ **Exercised under
T14.6, and the reading of `ChildGuard` was wrong.** `warpctrl agent cancel` on a
running turn killed the agent within 2s and settled the conversation to
`cancelled` with its partial output kept. And `ChildGuard::terminate`
(`agent-client-protocol-2.0.0/src/acp_agent.rs:308`) does *two* things: a
Unix-only `kill_process_group`, **and an unconditional `drop(self.0.kill())`**.
So the Windows gap is not "no cleanup", it is "no *process-group* cleanup" — the
direct child dies everywhere, and what can survive on Windows is a grandchild
behind a wrapper launcher (`npx → node`). `opencode` is a single binary with no
children, so it would not have exercised that even on Unix. Still unverified on
Windows. The GUI spawn environment is unverified: the probe ran from a login
shell and the app spawns from a GUI process whose PATH may lack npm/nvm shims.
No GUI screenshot was taken; turns were driven and read through `warpctrl`.
Nothing has run on Windows.

- [x] **T14.6** Saying yes: the consent surface. **Done 2026-08-29** — deny, then
      the `toolCallId` join, then a working yes, verified against two agents and
      photographed in a browser. Session resume, token streaming and diff
      rendering split to **T14.7** on an advisor's finding that none of them is a
      precondition of approve. What was paid for:
      **`acp_permission` moves out of `warp_cli` and is shared**, not copied —
      `acp_agent/mod.rs` says so at the point of use.
      **The approval card is the surface**, not a mode picker (T14.4).
      It has to render the structured diff — `opencode` sends `oldText` as well
      as `newText`, more than Claude — and, for a `switch_mode` request, the
      option *names*, because English is the only place that transition is
      disclosed.
      **`AppendToMessageContent` for token streaming**, once its `FieldMask` path
      is established by running it rather than guessed.
      **Session resume**, gated on the agent's advertised `loadSession` — this is
      now *load-bearing*, not a nicety: T14.5 refuses a second turn outright
      rather than answer it with an amnesiac agent, so until this lands an ACP
      conversation is one turn long.
      **Cancellation, verified by running it on both platforms.** T14.5 reasons
      it from the drop chain and never exercised it, and the client's own
      `ChildGuard` documents killing the process group on Unix *only*. A
      cancelled turn that leaks a connection leaks an agent that is executing
      tools.
      **The GUI spawn environment.** The app spawns from a process whose PATH may
      not carry npm/nvm shims — the reason `local_agent::spawn_for` does the
      `/bin/sh -lc` dance. Either the same treatment or an honest error naming
      PATH. Whether an ACP agent should run *inside* a WSL distro for a WSL
      session, as `claude` does for the file-read reason, is a real question this
      may defer but must name.

      **Measured before building any of it** (2026-08-28, `opencode` 1.18.25,
      Linux/WSL). Four runs, and two of them reframe the ticket.

      **Cancellation works, and the ticket's own premise about it was wrong.**
      See the correction folded into T14.5's as-built above: the agent dies
      within 2s on Linux, and `ChildGuard` kills on *every* platform — only the
      process-*group* kill is Unix-only. This item shrinks to "verify on
      Windows, with a wrapper-launcher agent", which is the only case that can
      actually leak.

      **The user's own agent permission configuration is resolved from the
      session directory — which is Warp's pane cwd.** Same Warp process, same
      `opencode.json` on disk, only the pane's directory differing. The evidence
      is the agent's own log, `~/.local/share/opencode/log/opencode.log`, which
      records each config file it loads and the `directory=` of each session:

      | pane cwd | project config loaded | model actually used | `"bash": "ask"` honoured |
      |---|---|---|---|
      | `/home/effatha` | **no** | `big-pickle` (a fallback) | **no** — ran `bash`, wrote `~/greeting.txt`, Warp never asked |
      | `/tmp/t146/project` | yes | `z-ai/glm-4.6` (as configured) | yes — the request reached Warp, Warp denied it, nothing was written |

      So *whether the user's policy applies at all* is decided by a directory,
      and in Warp that directory is chosen by Warp. This is T14.3's principle
      with a mechanism under it at last: Warp must not imply a policy it cannot
      observe, and here it does not merely fail to observe the policy — **it is
      an input to which policy loads.** A consent surface is therefore necessary
      and nowhere near sufficient; the case it cannot help with is the case
      where it is never invoked.

      Before that run, at defaults, the same agent ran
      `mkdir -p /home/effatha/essays && ls -la /home/effatha` and two
      `websearch` calls, unattended, in a fork whose thesis begins "no
      telemetry". T14.5's "it wrote a file" understates what asking nothing
      buys. (Artifacts removed.)

      **Method note against myself, because the near-miss is the lesson.** A
      follow-up run in what I *believed* was a fresh `$HOME` pane got asked, and
      I took that as falsifying the hypothesis above and nearly recorded the
      opposite. `agent prompt` without `--pane` uses the **active** pane, which
      I had `cd`'d earlier, and Warp restores pane cwd across a relaunch. The
      command ran perfectly and reported honestly; the error was one step
      upstream, in an input I assumed rather than checked — the exact shape
      `CLAUDE.md` records for the 5x divergence miscount. The agent's log is
      what settled it, not another run.

      **The permission request is a narrower view than the tool call it refers
      to, and it drops the field that matters most.** Captured:

      ```json
      {"toolCall":{"toolCallId":"call_29448bf5…","kind":"execute","status":"pending",
        "title":"echo hello > greeting.txt","locations":[],
        "rawInput":{"command":"echo hello > greeting.txt"}},
       "options":[{"optionId":"once","name":"Allow once","kind":"allow_once"},
                  {"optionId":"always","name":"Always allow","kind":"allow_always"},
                  {"optionId":"reject","name":"Reject","kind":"reject_once"}]}
      ```

      The `tool_call_update` for that same `toolCallId`, moments earlier, carried
      `locations:[{"path":"/tmp/t146/project"}]` and a `rawInput` with `cwd`. The
      request has neither. So a card rendering the request alone shows a shell
      command **with no indication of where it runs** — and by the finding above,
      where it runs is the thing that decided whether anyone was asked. The card
      must therefore join the request to accumulated `tool_call` state by
      `toolCallId` rather than render the request on its own, which makes that
      map's freshness a correctness concern and not a convenience.

      **The spawn-environment item is not a worry, it is a shipped defect: the
      app path loses the error entirely.** Measured by launching Warp with
      nothing changed but `PATH` stripped of the nvm shims, so `opencode` is not
      resolvable. The conversation goes to `status: "error"` with **no message at
      all** — nothing in the panel, and nothing in `warp-oss.log` beyond
      `has_active_stream=false status=Error`. A person in that state has a
      conversation that failed for no stated reason. The probe, on the same
      `PATH`, does surface it, which is how the cause was confirmed:

      ```
      error: internal: the agent exchange failed: Internal error: {
        "spawned_at": "…/agent-client-protocol-2.0.0/src/jsonrpc.rs:1732:39",
        "data": "No such file or directory (os error 2)"
      }
      ```

      So there are two things to fix and they are different sizes. The small one
      is that even where the error *is* surfaced it names a crate's line number
      and an errno, not the command or `PATH`. The real one is that the app
      swallows it — which is worth contrasting with T14.5's measured behaviour
      that a `CANNOT_CONTINUE` refusal reaches the log verbatim. Errors raised
      before the stream exists are evidently not on the same path as errors
      raised from `Turn::from_request`. Find out which, because a silent failure
      is the one bug class this fork's method cannot catch by running things.

      **Fixed in the same sitting, because it was small and the silence was the
      bad part.** `Translator` now tracks whether `open` has run, and `drive`
      picks its reporting shape from that: after `open`, a `StreamFinished` as
      before; before it, `Err(Arc::new(AIApiError::Other(..)))` as a stream item,
      which is the shape T14.5 measured surfacing for a refused continuation.
      The error text names `WARP_FORK_ACP_COMMAND`, the command and `PATH` — and
      only for the errno that actually means it, because a fix that guessed
      `PATH` at every failure would be T14.4's mistake again, a fix shaped like
      the hole. Pinned by `a_stream_is_not_open_until_it_has_been_opened`,
      `an_agent_that_is_not_on_path_is_reported_as_being_not_on_path` and
      `a_failure_that_is_not_a_missing_file_does_not_blame_path`.

      **Verified by re-running the case that produced the silence, and then by
      looking at the window** — the first GUI screenshot of the ACP path, which
      T14.5 shipped without. The panel now reads *"I'm sorry, I couldn't complete
      that request. Request failed with error: Other(Could not start the agent
      named by WARP_FORK_ACP_COMMAND ("opencode acp"): no such file or directory.
      The usual cause is that the program is not on the PATH of the process Warp
      was launched from…"*, and the same text is in the log. Before the fix, both
      were empty.

      Two things the run showed that reading would not have. Warp classifies this
      as recoverable and **retries three times before surfacing it**, as T14.5
      measured for a refused continuation — harmless, because a failed spawn
      fails instantly, but it is three spawn attempts rather than one. And
      `warpctrl agent read` reported **no output at all** for this conversation
      while the GUI was showing a paragraph — which is exactly why the screenshot
      was worth taking, and which made the first write-up of this very defect
      wrong ("no message in the panel and none in the log" — there was a
      paragraph in the panel).

      **Since fixed, because the defect was in the instrument.**
      `FinishedAIAgentOutput::output()` returns `None` for its `Error` variant,
      discarding the reason *and* whatever the agent had already said, so
      `format_output_for_copy` yields nothing and the summary omits `output`
      entirely — a failed turn is indistinguishable from a silent one.
      `AgentExchangeSummary` now carries `error`. This is load-bearing beyond
      T14.6: `agent read` is how this fork checks its own agent paths, including
      from another agent that cannot see the window.

      **The design, after an advisor falsified the first one.** The scoping
      conclusion this ticket was about to be built on — *"saying yes requires the
      ACP connection to outlive a single turn, held in a registry keyed by
      conversation id"* — is **wrong**, and wrong in an embarrassing way worth
      recording: it conflated two lifetimes. Warp's transport genuinely is
      stateless per turn, and the human's answer to a native action genuinely
      does arrive on a *new* call as `AIAgentInput::ActionResult`. But a
      `session/request_permission` does not arrive between turns. It arrives
      **while `session/prompt` is still outstanding** — the agent is blocked, the
      stream from `generate_multi_agent_output` is still open and still being
      polled, and the poller is what drives the connection. T14.5's own denials
      already answered at exactly that moment. Nothing has to be held across
      anything.

      What is actually missing is an **out-of-band input into a live turn**, and
      the fork already owns one — the gate that was not looked for, which is the
      single most repeated finding in this fork and was repeated again here:
      **T11.5's `agent.approvals` / `agent.approve` / `agent.deny`**. Digest-bound
      so a stale answer is refused, reachable from a paired phone, and remote
      *yes* already gated on `WARP_FORK_REMOTE_APPROVE` keyed at
      `ActionKind::AgentApprove` in `pairing.rs`, so a new population answered
      through the same action inherits the gate rather than needing a new one.
      Today it presses keys on CLI-agent panes; ACP would add a second
      population whose "answer" resolves a parked responder instead.

      **And on one axis the ACP path is strictly better than the keystroke path,
      which is worth saying because T14 has mostly gone the other way.**
      `agent.approve` is refused for agents outside `ALLOW_VERIFIED_AGENTS`, and
      `approvals.rs` gives the honest reason: Return means *take the highlighted
      option*, and which option that is "is a fact about someone else's TUI".
      ACP options are **typed and carry ids**, so answering selects a named
      option rather than a position. That is the T14.2 bug — `options.first()`,
      which denied on one agent and would have approved on the other — inverted
      into a structural property. `ALLOW_VERIFIED_AGENTS` therefore has no
      analogue here, and `acp_permission`'s kind allowlist is what stands in its
      place. Verified by reading `approvals.rs` and `pairing.rs` rather than
      taking it on report: `REMOTE_APPROVE_ACTION` is `ActionKind::AgentApprove`
      and `pairable_actions()` adds it only when `fork::remote_approve_enabled()`.

      `PendingApproval` itself is **not** the type to reuse — it is keyed on a
      pane because a CLI agent "has no `AIConversation` and nothing in Warp's
      history model", and its fields are derived from an OSC notification. T14.4
      already filed this: a new shape, keyed on the JSON-RPC request id.

      The mechanism, read out of the vendored crate rather than assumed:
      `Responder`'s send fn is a boxed `FnOnce … + Send`
      (`agent-client-protocol-2.0.0/src/jsonrpc.rs:3680`), so a responder can be
      moved into a task, and `ConnectionTo::spawn` (jsonrpc.rs:3173) is the
      crate's own documented escape from "this callback blocks further message
      processing until it completes". So `deny(&request)` becomes: register a
      pending approval keyed on the JSON-RPC request id, digesting everything
      shown; say in the conversation what is being asked and where to answer;
      park the responder; resolve it with whatever the shared
      `acp_permission::choose` returns for the human's decision. Same stream, no
      `session/load`, no connection registry, no new GUI surface, and no new
      `warpctrl` action.

      **Three falsifiers, to be run as a spike before any of it is built** — this
      design is reasoned from source, which is exactly the standard T14 keeps
      failing:
      (a) a parked responder plus `spawn` deadlocks the single-task connection;
      (b) `opencode` times out its own outstanding request while a person thinks
      — its timeout behaviour is entirely unmeasured;
      (c) Warp aborts or mis-renders a turn left idle for minutes (grepped
      `response_stream.rs` and found only token-refresh timeouts, but grepping is
      not tracing).
      A spike that parks one request, answers it from `warpctrl`, and watches the
      file appear settles all three. If any fails, the held-connection design
      comes back off the shelf and *then* the "resume becomes second-best"
      argument holds.

      **Run. All three survive, and two more besides.** The spike is
      `park_until_answered`, gated on `WARP_FORK_ACP_SPIKE_PARK` naming a file
      whose appearance is the out-of-band answer. It parks the responder in a
      `connection.spawn` task and returns from the callback immediately, then
      answers with **the same denial the immediate path computed** — the
      mechanism is what is under test, the decision is not, and nothing in it can
      say yes.

      | falsifier | result |
      |---|---|
      | parked responder deadlocks the connection | **no** — parked 180s, then answered within 5s of the file appearing |
      | `opencode` times out its own outstanding request | **no** — three minutes, no complaint |
      | Warp abandons an idle turn | **no** — `in_progress` throughout, then `success` |
      | *cancel while parked leaks the agent* | **no** — `cancelled`, agent process gone |
      | *two conversations parked at once* | **both** resolved on one answer, both agents exited |

      Nothing was written in any run, because the answer delivered was still the
      refusal. The last two rows were not on the advisor's list and are the ones
      a parked responder makes newly possible: a held request is a held *task*,
      and cancellation had to be shown still reaching through it.

      **And an instrument lied in the middle of it.** `pgrep -c -f "opencode acp"`
      reported four agent processes when the true count was one — it was matching
      the command line of the shell that had launched Warp, which contains the
      string. Caught by listing the processes instead of counting them; the real
      check is `ps -eo comm | grep -c '^opencode$'`. Third time this phase that a
      command ran perfectly and answered a different question than the one asked.

      **Still unverified**, and each is a real gap rather than a formality: parking
      beyond the spike's five-minute deadline; two requests parked on **one**
      connection (both agents here were separate processes, so a blocked dispatch
      loop would not have shown); Windows; and any agent other than `opencode`.

      **And the silent-failure defect above is evidence against assuming step two
      works.** "Emit a note saying what is being asked and where to answer" is
      the same kind of event that was just measured vanishing when the turn was
      in the wrong state. The spike has to verify the note *lands*, not that it
      was sent.

      **Increment 1 is built and run: parked requests are listed and can be
      answered no.** `agent.approvals` reports a second population, `agent.deny`
      resolves a parked responder, `agent.approve` is refused by name. Measured
      end to end:

      ```
      $ warpctrl agent approvals
        approval_id "5a4ecbe3-…:0"  agent "opencode"  tool_name "execute"
        summary "echo second > second.txt"   cwd "/tmp/t146/project"
        can_approve false   approve_refused_because "Warp cannot say yes … yet"
        options_offered ["Allow once","Always allow","Reject"]
      $ warpctrl agent deny 5a4ecbe3-…:0 --digest …
        decision "deny"  keystroke "reject_once"
      ```

      `keystroke: "reject_once"` rather than `escape` because that is what
      actually happened — the CLI path presses a key because pressing a key is
      all it can do, and this one selects the agent's own typed option by id.
      Reporting `escape` would describe a mechanism that was not used, which is
      the same rule that made T11.5 report *which key it sent* instead of
      `approved: true`.

      Also verified live: a wrong digest is refused, and refused *differently*
      from a refused yes — the digest check runs first, so a caller holding a
      stale answer is not sent off to solve the wrong problem. Cancelling one
      parked turn withdrew its entry and left a second one parked. Nothing was
      written in any run and no agent process leaked.

      **And running it found a defect no test would have.** The first build keyed
      the registry on the JSON-RPC request id alone, as this ticket's own plan
      said. That id is **per-connection**: two concurrent `opencode` sessions
      both opened with `0`, and against a process-global map the result was not a
      lost entry but two wrong answers.

      1. Turn B parks over turn A, dropping A's oneshot sender.
      2. A's waiter reads a dropped sender as *answered* and denies at once —
         while the panel still reads "waiting for permission".
      3. A's cleanup then removes the key, which is now **B's** entry, and B
         denies too.

      Both turns reported `success`, the tool never ran, and nobody was ever
      asked. The visible symptom was two conversations that had "asked" and
      silently answered themselves. Fixed in both halves, because either alone
      leaves the other latent: the key is scoped to the turn, and a waiter now
      removes an entry only if it still owns it (a token, compared on drop).
      `a_reused_key_cannot_make_one_waiter_answer_another` builds the collision
      by hand and pins the second half.

      The lesson is the ticket's own: the plan said "keyed on the JSON-RPC
      request id, and keying on request id rather than toolCallId is itself the
      defense against the re-ask hazard" — correct about the hazard it named, and
      silent about scope. A globally-unique-sounding phrase for a
      locally-unique value.

      **What finding 2 changes is what the card must say, not what it must be.**
      The session directory — the thing that decided whose rules applied — is
      Warp's own first-hand fact, chosen from the pane and sent in
      `NewSessionRequest`; it is already sitting in `Turn::working_directory` and
      needs no map to disclose. The per-call `cwd`/`locations` from the preceding
      `tool_call_update` are agent-authored enrichment and are what needs the
      `toolCallId → last-seen fields` join, scoped to pending requests rather
      than built as a general accumulated-tool-state model.

      **Increment 2 is built: the join, and the surface says where a call acts.**
      `Translator` now records each call's `locations` by `toolCallId`, and a
      parked request carries them as `acts_on` — through `PendingApproval`, into
      the digest, onto the console as its own line, and into the in-conversation
      note. `acp_permission` is `pub` (module, `Decision`, `Choice`, `choose`),
      which is the sharing the ticket required and `acp_agent/mod.rs` demands at
      the point of use; nothing reads it from `app` yet.

      **The recording had to move out of the display path, and that is the whole
      subtlety.** `tool_update_text` returns early for any update that is not
      `Completed` — which is exactly the update that carries the path, since the
      capture above shows `locations` arriving on a *pending* call and the
      permission request arriving with `locations: []`. Recording from where the
      title is read would therefore have dropped every location and passed its
      tests. It happens before the display dispatch instead, and
      `a_location_is_remembered_from_an_update_that_shows_nothing` is the seam:
      no events emitted, path nonetheless known. A second test pins that a later
      silent update does not erase a location already known, because agents send
      the field on the update that has it and omit it elsewhere.

      **What `acts_on` is not allowed to do is fall back to `cwd`.** The session
      directory is right there and usually correct, which is what makes it the
      tempting answer to "where does this run" — and it is Warp's fact, not the
      agent's. Substituting it would manufacture the exact certainty finding 2
      established this fork does not have. When the agent named no location the
      note says nothing and the console draws no line;
      `a_call_that_named_no_location_is_not_given_one` pins the silence.

      `acts_on` is in the digest, and on reflection it has the strongest claim of
      any field to being there: "run `rm -rf build`" and the same string
      somewhere else are not the same question, and the paths are the difference.
      Hashed as a second length-prefixed list after `options_offered`, with a
      test that a name moving between the two lists does not cancel out.

      **Run, and the join holds live.** Fresh scratch profile, pane in
      `/tmp/t146/project`, `opencode.json` with `"bash": "ask"`:

      ```
      $ warpctrl agent approvals
        approval_id "136109d4-…:0"   agent "opencode"   tool_name "execute"
        summary   "echo hello > greeting.txt"
        tool_input {"command":"echo hello > greeting.txt"}
        cwd       "/tmp/t146/project"        ← Warp's session directory
        acts_on   ["/tmp/t146/project"]      ← the agent's, via the toolCallId join
        options_offered ["Allow once","Always allow","Reject"]
      ```

      So the location *was* in the map at the moment the request parked — the
      ordering the earlier capture showed is not a one-off, and the surface can
      answer "where does this run" from the agent's own claim rather than from
      Warp's directory. The two happen to agree here, which is exactly why the
      code may not be allowed to conflate them.

      **And the note lands**, which is the check the ticket said not to assume:

      > `bash`
      > The agent is waiting for permission: **echo hello > greeting.txt**. Warp
      > cannot say yes to this yet … Answer no with `warpctrl agent deny
      > 136109d4-…:0`, or from a paired device, or cancel the turn.
      >
      > It says this acts on `/tmp/t146/project`.
      >
      > This session runs in `/tmp/t146/project` — Warp chose that from the pane.
      > The agent resolves its own permission rules from there, and Warp cannot
      > see them.

      Then `deny` with a wrong digest → `invalid_params` naming the fix; with the
      right one → `decision "deny"`, `keystroke "reject_once"`; the entry
      disappears; `greeting.txt` was never written; zero `opencode` processes
      left (checked with `ps -eo comm | grep -c '^opencode$'`, not `pgrep -f`).

      **Increment 3: Warp can say yes, and it was run before it was believed.**
      `acp_permission::choose` is called once at park time, while the real
      request is in hand; the option id it returns is frozen onto the entry as
      `approve_selects`, and `can_approve` is simply whether one exists. So the
      listing's promise and the answer path are the *same* computation rather
      than two that agree today — which is the console bug of increment 1
      removed at the root instead of patched at both ends.

      Measured live, and this is the first yes in the fork's history:

      ```
      $ warpctrl agent approvals
        summary "echo hello > greeting.txt"   tool_name "execute"
        acts_on ["/tmp/t146/project"]
        can_approve true   approve_selects "once"
      $ warpctrl agent approve 775d60dc-…:0 --digest 332549eb…
        decision "allow"   keystroke "once"
      $ cat greeting.txt
        hello                       ← the agent ran it; nothing else did
      ```

      The turn then reached `status: "success"`, `is_complete: true`, with the
      agent's own "Done." in the transcript. A stale digest is refused for a
      *yes* exactly as for a no, and refused first, so a caller holding an old
      answer is not told about approvability instead.

      **And the sentence the note makes — "this call only, nothing after it" —
      was measured rather than asserted.** One turn, two commands: Warp approved
      `echo one > a.txt` with `once`, and the agent **asked again** for
      `echo two > b.txt` rather than proceeding. Two parks, two answers, two
      files. That is the whole argument for refusing `allow_always` doing its
      job on the wire, and it is the claim a person tapping *Yes* on a phone is
      relying on.

      What stays refused is unchanged and is decided per entry, never per
      population: a `switch_mode` request, a tool kind this build cannot bound,
      an option declaring a policy change — all because a **binary** yes cannot
      honestly carry them, whoever is looking — plus one rule added here, that a
      request carrying no `rawInput` shows only the agent's own one-line title,
      and approving a title is not approving a command.

      `ACP_APPROVE_NOT_YET` is gone, and its removal is the point rather than a
      tidy-up: its sentence was *"there is no surface that could show you what
      saying yes would allow"*, which was true when written and went false the
      moment the surfaces rendered the call. A refusal whose stated reason is
      false is the T14.2 failure — a person concludes the feature is broken.

      **Still unverified**: every refusal above is pinned by a test and none has
      been seen on the wire, because `opencode` never sends a `switch_mode` and
      the fixture requests are transcribed rather than captured. The honest
      statement is that the *allow* path is measured and the *refusal* path is
      argued. A run against `claude-agent-acp` in plan mode is what would settle
      it, and that is the same "second agent" item already outstanding.

      **The console, photographed in a real browser — and it found a defect no
      test would have.** Firefox on the Windows side, scratch profile, against a
      `WARP_FORK_CONTROL_BIND` listener with `WARP_FORK_REMOTE_APPROVE=1`. The
      row renders:

      ```
      opencode wants permission
      echo hello > greeting.txt
        execute: {"command":"echo hello > greeting.txt"}
      session directory /tmp/t146/project
      acts on /tmp/t146/project
      [        Yes        ]  [        No        ]
      Yes selects "once" — this call only, nothing after it.
      ```

      The pairing fragment is erased from the address bar, as designed, and the
      whole path works: **two fast taps on Yes wrote `greeting.txt` from a
      browser over the network.** That is remote approve end to end — bind,
      pair, park, answer, execute.

      **One tap and a two-second pause did nothing at all**, and that is the
      defect. `renderApprovals` calls `clear()` and rebuilds every row; it runs
      on a 5s poll and on every agent event, while `ARM_MS` is 4s. A refresh
      inside the arm window destroyed the armed button and drew a fresh `Yes`,
      so the tap meant to *confirm* armed the new button instead and the answer
      never happened. The shape is worse than an outright failure — it reads as
      an unreliable feature rather than a broken one, which is the T14.2 lesson
      restated on the surface T14.2's argument was about.

      It fails **safe**: a discarded arm can only ever lose a yes, never invent
      one. So it is a counter, not a lock — `refreshApprovals` defers while
      anything is armed, bounded by `ARM_MS`, and a list up to four seconds
      stale is not a hazard because every answer still carries the digest of
      what was shown.

      Worth noting how it was found: the first single tap looked like a missed
      click, and the honest next step was to read `armThenRun` rather than
      re-aim the mouse. `ARM_MS = 4000` against `setInterval(refreshApprovals,
      5000)` was the whole answer, sitting in two lines 400 apart.

      **The `edit` case, measured rather than assumed.** `edit` is on
      `acp_permission`'s allowlist, so an approvable edit whose `rawInput` named
      only a path would let a person authorize content they never saw — the one
      place the ticket's "the card has to render the structured diff" could have
      been a *precondition* rather than a nicety. Captured from a live
      `opencode` edit request:

      ```json
      {"filepath": "/tmp/t146/project/notes.txt",
       "diff": "--- …/notes.txt\n+++ …/notes.txt\n@@ -1,3 +1,3 @@\n alpha\n-beta\n+DELTA\n gamma\n"}
      ```

      **The content is there**, as a complete unified diff, so the disclosure is
      whole and diff rendering stays a nicety — no per-entry demotion of `edit`
      is needed. (And had it been needed, it would have been a per-entry check
      rather than an edit to the shared allowlist: `--approve` reads that too,
      and its rule is "confined to this call", not "shown".) `acts_on` named the
      **file** here rather than the directory, which is better than the `execute`
      case. Approving it changed `beta` to `DELTA` on disk.

      What is true is that a unified diff inside an escaped JSON string is
      disclosed but barely legible on a phone. That is the argument for
      rendering it properly in T14.7, and it is a readability argument, not a
      consent one.

      **On `PendingApproval::source` versus a label string.** The advisor
      preferred the server to send the *label* (`cwd_label: "session"`), on the
      grounds that a taxonomy in the protocol forces the client to map it. The
      field shipped as `source` because it is one string either way and it is
      independently useful — a client can finally tell the two populations apart
      — but the objection had one concrete failure mode and it is closed: the
      console's first draft was a two-way ternary, which would have labelled a
      third population "working directory", confidently and wrongly. It is now a
      lookup, and a `source` with no entry draws the bare path and no claim at
      all. Saying less is the only safe direction when the whole point of the
      label is that the two mean different things.

      **The refusal path is measured now, against the second agent.** The commit
      that shipped the yes said plainly that the allow path was measured and
      every refusal only argued — no `switch_mode` had ever been seen on the
      wire, because `opencode` does not send one. So `claude-agent-acp@0.70.0`
      was run through the app path, in a scratch project whose **own**
      `.claude/settings.json` sets `defaultMode: plan` (the user's global says
      `auto`, and was not touched — project settings outrank user settings,
      which is the same session-directory mechanism finding 2 established).

      Asked to create a file, it planned, then sent `ExitPlanMode`:

      ```
      tool_name  "switch_mode"     summary "Ready to code?"
      can_approve false
      approve_refused_because "the agent is asking which permission policy
        should apply, not whether one thing may happen; …"
      options_offered
        "Yes, and bypass permissions"      "Yes, and use \"auto\" mode"
        "Yes, and auto-accept edits"       "Yes, and manually approve edits"
        "No, keep planning"
      ```

      Exactly the five options T14.2 captured, including the `allow_once` that
      T14.2's `--approve` selected — the bug that started this whole line of
      work. `agent approve` answered `insufficient_permissions` with that
      reason; `agent deny` selected `reject_once`; `plan.txt` was never created
      and the session kept plan mode. **The T14.2 defect, inverted into a
      defence and now measured rather than argued.** `acts_on` was absent, which
      is correct — a mode switch acts nowhere.

      **Three defects on the consent surface, all found by looking at what the
      card actually said.**

      *The card named the launcher.* `agent` read **`npx`**, because the program
      was the first whitespace token of `WARP_FORK_ACP_COMMAND` and the command
      is `npx -y @agentclientprotocol/claude-agent-acp`. Accurate and useless:
      every agent reached through `npx` would say the same, so the one field
      whose job is *which agent is waiting* had stopped answering it. Now a
      named launcher is skipped and the package trimmed to
      `claude-agent-acp`, with the first token as fallback — so the failure mode
      is less information, never wrong information.

      *The refusal named a flag that does not exist here.* The reason read
      *"…so --approve declines…"* in a Warp conversation. Sharing
      `acp_permission` between the CLI flag and the app is what made its
      sentences owe the rule `the_continuation_refusal_explains_itself_without_protocol_jargon`
      already applies: a refusal must not name a mechanism the reader cannot act
      on. They say *"Warp declines"* now, and a test pins that no shared reason
      names `--approve`, `--deny` or `warpctrl`.

      *Two sentences ran together.* *"…the policy it already had Answer no
      with…"*, in the paragraph a person reads to decide. The reasons terminate
      now and the note terminates defensively anyway — and writing that test
      immediately found a **fourth** reason, `no_option_reason`, trailing off
      after a list.

      **And the agent wrote into `~/.claude/plans/` before asking anything.**
      The permission request's `rawInput` carried
      `planFilePath: /home/effatha/.claude/plans/…md`, and the file was there.
      Not Warp's doing, and not a leak of the scratch profile — plan files are
      global to the agent. It is one more instance of this module's own caveat,
      on the flagship agent: **Warp only ever answers the questions an agent
      chooses to ask**, and the write that mattered was not one of them.
      (Artifact removed; the pre-existing plan file beside it was left alone.)

      **A note on the instrument rather than the code.** The release build took
      the whole WSL VM down — the guest came back at `up 1 min` with an empty
      `dmesg`, so the VM died rather than Linux OOM-killing a process. Measured
      cause: one `rustc` on the `warp` crate holds **~8.1 GB RSS**, and at
      cargo's default parallelism (one job per core, 32 here) several 8 GB-class
      crates reach codegen together. `CARGO_BUILD_JOBS=8` built it with 19 GiB
      still available. This is the same hazard `[profile.release]`'s own comment
      records for CI ("OOM-killing release builds"), reached from the other
      direction. There was no `.wslconfig` at all, so the 32 GB the VM had was
      WSL2's default of half the host's 64 GB; one is now written with more
      headroom and real swap, taking effect at the next `wsl --shutdown`.

- [x] **T14.7** **Self-hosting: build the fork from inside the fork.** **Met
      2026-08-29 — commit `cddacfbc7` was made from the panel.** The
      weekend goal, charter in `.fork/GOAL.md`, target Monday 2026-09-01.

      **Done means a run, not a diff**: in Warp's own agent panel, in this repo,
      a *multi-turn* conversation that makes a real change to the fork, asks
      permission for it, is answered, and whose next turn remembers what it did.

      **Phase 0 is a measurement and it gates everything else.** Two paths serve
      the panel and on paper each has exactly the other's blocker —
      `local_agent` has `--resume` and no approval path it can ever reach
      (`approvals.rs` says so: it never emits a `ToolCall`), `acp_agent` has the
      T14.6 consent surface and refuses a second turn outright. **That table is
      read, not run.** Drive a real development task through each, not a one-line
      prompt, and write down what actually breaks before writing any code.

      The specific thing a guess would get wrong: `claude -p` inherits the user's
      own settings — `defaultMode: auto`, 87 allow rules — so `local_agent` may
      already do most of the work. What happens to a tool call *outside* those
      rules is unmeasured, and it has no TTY to prompt on, so the failure may be
      silent. That is the same shape as the spawn failure T14.6 found: the bug
      class this fork's method cannot catch by reading.

      Phase 1 closes whichever gap Phase 0 names — most likely `session/load`
      gated on the agent's advertised `loadSession`, whose open question is what
      history it replays and whether Warp then draws the transcript twice.
      Phase 2 is the dogfood, and until it has happened the goal is not met
      however good the code looks.

      Stretch only after those verify: `AppendToMessageContent` streaming, whose
      `FieldMask` path must be established by running rather than guessed, and
      diff rendering — the content is already in `rawInput`, measured on T14.6,
      so that one is legibility and not consent.


      **As built — Phase 0, measured 2026-08-29.** Driven through a real GUI
      Warp on WSLg with a scratch profile, `warpctrl agent
      prompt/read/approvals/approve`, the same three-step probe on each path:
      run a command in the allow list, write a file, run a command in the deny
      list. Then a second turn asking what the first one did, answered from
      memory with no tool use.

      **The table above is wrong in both of the cells that mattered, and the
      blocker that actually stops the goal is in neither of them.**

      **The session cwd is the pane's shell cwd, and a fresh pane starts in
      `$HOME`.** Warp was launched *from* the repo and the pane still came up in
      `/home/effatha`, so the first run's `git status --short` answered "not a
      git repository", the agent created `/home/effatha/target/` and wrote its
      probe file there, and reported success. Nothing lied: the agent said what
      it did and the path was in its own answer. But a person who asks the panel
      to change the fork gets their home directory instead, and both paths do
      this identically because both read
      `params.session_context.current_working_directory()`. `warpctrl input
      submit 'cd /home/effatha/git/warp'` fixes it for the next turn, and the
      second run confirmed `pwd` and `git rev-parse --show-toplevel` both inside
      the repo. **This is not a code bug and it is the first thing to do in
      every session** — it was predicted by nothing in this ticket.

      `local_agent` **is much closer than the table said.** Multi-turn works:
      turn 2 returned the exact absolute path written in turn 1, with no tool
      call, so `--resume` is carrying the transcript. And its permission story
      is not a hang. `Write` ran without asking anyone — the user's settings are
      `defaultMode: auto` and that is a real answer, not an absence of one —
      while `ssh -V`, which matches `Bash(ssh:*)` in the 30 deny rules, came
      back as a tool error the agent narrated in prose and then continued past.
      **So the "silent failure" this ticket named as the thing a guess would get
      wrong does not exist, and the doc comment in `local_agent/mod.rs` saying
      "read-only tools work and anything needing approval is denied" is wrong in
      its first half.** What is true is narrower: Warp is never consulted.
      `warpctrl agent approvals` stayed empty through the whole run. Consent for
      this path lives in `~/.claude/settings.json` and Warp cannot see it, which
      is T14.3's finding arriving again from the other side.

      `acp_agent` refuses the second turn exactly as written, and **Warp retries
      that refusal three times with backoff before showing it** — the error is
      classified recoverable, so a deterministic "this build cannot do that"
      costs about four seconds of pretending it might. Worth fixing where the
      refusal is raised, not in the retry policy.

      **The T14.6 consent surface is live, and reaching it is the *agent's*
      decision rather than Warp's.** At its own defaults `opencode` asked
      nothing and ran `ssh -V` — the same command the Claude path denied, same
      prompt, opposite outcome. Given an `opencode.json` in the pane's cwd
      declaring `permission: {edit: "ask", bash: "ask"}`, the same call produced
      a real `PendingApproval` (`can_approve: true`, `approve_selects: "once"`,
      digest bound, `acts_on` naming the repo); `warpctrl agent approve <id>
      --digest <d>` answered it and the command then ran and rendered. The whole
      loop — asked, answered from outside the GUI, executed — is verified. This
      is the shape of the answer to "which path can satisfy the goal sentence",
      and it is worth stating plainly that **Warp cannot make an agent ask**;
      the config that makes it ask is the user's, in the pane's directory.

      **Phase 1's falsifier was run before Phase 1 was started, and it came
      back green.** The open questions about `session/load` — whether the
      agent's session survives the process exiting at all, given that this fork
      spawns one agent per turn, and what history the load replays — were
      answered by speaking ACP to `opencode acp` directly over stdio, no Warp
      involved. It advertises `loadSession: true`; a session created and
      prompted in one process was loaded by name in a **second, freshly spawned
      one**, and that second process answered the memory question correctly. So
      the per-turn process model is not an obstacle. The replay is small and its
      ordering is the thing that matters: for a one-turn history the agent sent
      exactly two `session/update` notifications — `user_message_chunk` then
      `agent_message_chunk` — and it sent them **before** the `session/load`
      reply. That is what would draw the transcript twice, and it is also what
      makes suppressing it easy: everything between the request and its reply is
      history. **Unverified: that any other agent orders it the same way.** The
      spec does not appear to require it, so the suppression has to be
      structured as "ignore until the reply" rather than "ignore the first N".

      **And the agent sent to build the fork was reading the wrong rules.**
      Asked in this repo, with no tool use, which instruction files it had been
      given, `opencode` answered `AGENTS.md` — and nothing else. That file is
      upstream's. It is right about architecture and testing, which is why this
      fork keeps it, but it says never to `cargo fmt` where `CLAUDE.md` says
      that folklore is measurably wrong, and it has never heard of
      `CARGO_BUILD_JOBS=8` — the rule whose violation took the whole WSL VM down
      the same morning this was measured. So the first thing self-hosting would
      have done, following its instructions faithfully, is start an uncapped
      release build. `opencode.json` at the repo root now carries
      `"instructions": ["CLAUDE.md"]`; asked again, the agent quoted the capped
      build command and `warpctrl window close` back correctly. The same file
      carries the ask policy, so one small file both tells the agent the rules
      and makes it ask before breaking them — and it means the goal run depends
      on nothing outside the tree. Checked before adding it: the repo root has
      no other `opencode.json`, no `.opencode/`, and no ignore rule for either.

      Two smaller findings, both recorded rather than fixed here. The ACP path
      writes **no tool events** to `WARP_FORK_EVENT_LOG` — only `session_start`
      and `stop` — while `local_agent` writes a full `tool_start`/`tool_complete`
      pair per call; so the instrument this file recommends reaching for before
      theorising about an agent covers one path of two. And the asking note
      stays in the transcript after the request is answered, so reading back a
      finished conversation shows a live-sounding prompt for a call that already
      ran.
      **As built — Phase 1, verified live 2026-08-29.** A conversation in the
      panel, in this repo: turn 1 asked to write a file, `opencode` asked
      permission for it, `warpctrl agent approve --digest` said yes, the file
      appeared. Turn 2 asked what it had written and it answered the path and
      the contents from memory. That is every clause of the goal sentence in one
      conversation except one — the change was to `target/`, not to the fork,
      which is Phase 2's job and is not claimed here.

      The thing most likely to have broken did not. The falsifier replayed a
      *text-only* history; this replay carried a tool call, which is the case
      where the translator's bookkeeping could have leaked. `agent approvals`
      stayed empty for the whole of turn 2, so no replayed permission-shaped
      update minted an answerable entry, and the transcript came back with two
      exchanges and turn 1 drawn once.

      **Cancelling a turn mid-question leaves nothing behind, and finding that
      out cost two defects.** With a request pending, `agent cancel` dropped the
      entry and the agent process exited — no orphan to approve, and the dead id
      answers `missing_target`. But that error called the approval id *a pane*,
      which it has not been since ACP entries started being `{turn}:{rpc_id}`;
      and worse, the transcript still carried the asking note, so a person
      reading back a cancelled conversation is told to type a command with an id
      that no longer exists. The note now has an answer beside it — a *second*
      note rather than an edit of the first, because amending a message needs
      `UpdateTaskMessage`'s `FieldMask` path, which nothing in this repo uses and
      which this module has now twice declined to guess at.

      Two things were probed and left alone. `cd`ing the pane between turns did
      **not** break resume: `opencode` accepted a `session/load` whose `cwd` no
      longer matched the session's, though the spec's own wording suggests a
      stricter agent may refuse. Pinning the cwd to the session would fix that
      by silently ignoring a `cd` the person typed, which is worse than the
      refusal it prevents, and the refusal is already actionable. And the
      `cannot_resume` branch is **test-covered but never run live**, because both
      agents on this machine advertise `loadSession: true` — `opencode` and
      `claude-agent-acp` alike.

      **As built — Phase 2, run 2026-08-29. The goal is met.** Commit
      `cddacfbc7` was written by an agent in Warp's own panel, in this repo,
      across a three-turn conversation, and every clause of the sentence
      happened rather than being arranged for.

      The work was real and was chosen because it was owed: three claims in
      `.fork/docs/manual.md`'s `WARP_FORK_ACP_COMMAND` section had gone false — "it
      can only say no", "a second turn is refused", and "not yet: session
      resume" — each falsified by a commit earlier in this phase that had not
      updated the doc. Turn 1 made the three corrections and asked permission
      for each edit; `warpctrl agent approve --digest` answered. Turn 2
      recalled all three from memory with no tool use, including which item it
      had removed from the "Not yet" list, and then took a review finding: its
      own first edit had orphaned the sentence *"That is the shape
      `local_agent` shipped in"*, whose "That" had pointed at a blockquote it
      deleted. It fixed the reference. Turn 3 committed.

      **Nobody told it the commit convention.** Asked to follow "this project's
      own", it read `CLAUDE.md`, then ran `git log -1 --format='%B'` on an
      earlier commit from this phase to see the house style, and wrote a body
      that explains what was found rather than what was typed — citing the
      commit that made its second claim false. That is `opencode.json`'s
      `instructions` line paying for itself two commits after it was added.

      Seven permission requests over three turns, every one answered from
      `warpctrl`, none auto-approved. The loop the panel shows is now complete
      end to end: the asking note, then **"Answered: yes, for this one call.
      Nothing after it is covered."**, then the command's own output —
      photographed rather than inferred, because `agent read` is silent about a
      whole class of state.

      **What the goal does not yet cover, stated plainly rather than left to be
      discovered.** There is no button. Consent is answered by typing a
      `warpctrl` command, which is doable *from* Warp — it is a terminal — but
      it is not clicking a control in the panel that is asking. The console
      (T12) has the button and reaches a phone; the panel does not. That is the
      next ergonomic step and it is a smaller one than anything in this ticket.

- [x] **T14.9** **Run a full working session on the fork, from the fork, and
      write down what it costs.** **Run 2026-08-29; see the as-built.** Gates T14.8, and the ordering is the point:
      T14.7 Phase 0 predicted two blockers, got both wrong, and the real one was
      in neither cell of its table. Every candidate friction below T14.8 is a
      guess until a real session ranks them.

      Not a demonstration. Take a real item off this board, work it end to end in
      the panel over as many turns as it actually takes, and keep a friction log
      — every point where the surface made you reach for something outside Warp,
      with a count. Seven approvals in three turns is the number that already
      exists; what matters is which friction dominates at length and whether any
      of them compounds.

      Known-unmeasured going in: `/compact` is unhandled on the ACP path, so a
      long session may simply run out of context and it is not known what that
      looks like from the panel. Replay cost is **not** a candidate — measured
      flat at 0.34s from one exchange of history to six, with process startup
      dominating (see `GOAL.md`), though fifty exchanges and large tool outputs
      in the history are both unverified.

      **As built — run 2026-08-29, and it reframes T14.8.** A seven-turn session
      in the panel that made `WARP_FORK_EVENT_LOG` cover the ACP path: plan,
      implement, two review rounds, and two live verifications, each rebuild
      driven from inside the conversation. About **thirty-five permission
      requests**, every one answered from `warpctrl`. The code landed and works;
      what the session was for is everything else.

      **The dominant finding is not the missing button. It is that a share of
      requests have no yes behind any button.** `acp_permission::choose` refuses
      `ToolKind::Other`, because an unknown kind's effect cannot be bounded to
      the one call — a principled rule, and T14.6 was right to write it. What was
      unknown until this session is *how often it fires on ordinary work*. Twice,
      in two separate conversations: a `read` of a dependency's source in
      `~/.cargo/registry`, and `cat /nonexistent-file-xyz`. In the same turn as
      the second, `git status --short` arrived as `execute` and was approvable.
      **The agent decides the kind**, and when it picks one Warp cannot bound,
      the request parks with `can_approve: false` and the turn stops until
      somebody denies it. An in-panel button would be greyed out for exactly
      these, so building it first would leave the worst case untouched.

      **Amended by T14.8: "a person cannot tell in advance which of their
      commands will be answerable" was wrong**, and it was the sentence that made
      this look like a rule needing to be relaxed. Measured against the same
      agent, the predicate is knowable: `other` is what `opencode` sends when a
      call would reach outside the project directory, and everything inside it
      arrives as an ordinary kind. Both cases here fit — a crate source in
      `~/.cargo`, and `/nonexistent-file-xyz` at the root.

      **A wedged turn is indistinguishable from a working one.** One turn ran 36
      minutes: process alive, 74 seconds of CPU against 2176 elapsed, state `S`,
      the panel frozen on the same `grep` across two screenshots fifteen minutes
      apart, and `agent list` reporting `in_progress` throughout. No pending
      approval, so no question to answer; no output, so nothing to read. The
      parked-request design has no deadline **on purpose** — right for a question
      a person will answer — but a turn with neither question nor output has no
      equivalent signal at all. It was diagnosed with `ps` and a camera.

      **Recovery, by contrast, was total, and this is the finding that makes
      ongoing use plausible.** `agent cancel` ended the wedge; the next turn's
      `session/load` restored the whole conversation *including work done in the
      minutes before it stalled* — it recalled the third copy of `kind_name` it
      had found seconds before hanging. And the conversation survived Warp itself
      being closed and rebuilt twice: `agent list` still had it, and it resumed
      against a new Warp process and a new agent process. **A wedge costs time,
      not state.**

      **The GUI has the information and the CLI has the control, and neither has
      both.** The panel streams each tool call as it starts, shows a spinner, and
      puts a `+137 -2` badge on the tab. `warpctrl agent read` shows nothing at
      all until the turn ends. That is the same split as the button, pointing the
      other way, and it is why driving a session from the CLI meant photographing
      a window to find out whether anything was happening.

      **Amended by T14.12: `agent read` does stream, and this sentence was
      wrong.** Polled through a live turn it went 0 → 675 → 742 → 838 characters
      with `is_complete: false` throughout. The error is explicable rather than
      careless — for the first few seconds there really is nothing, while the
      panel is already drawing a spinner — but it was written as a general claim
      and it is not one. Almost cost a night building a feature that exists.

      Two smaller ones. Answering approvals from a script needs a polling loop,
      and two of them ran at once without being noticed — edits landed that the
      friction log never recorded, which a person pressing a control could not do
      to themselves. And reviewing what landed meant `git diff` in another
      terminal: the approval card shows an edit's full diff, and the transcript
      afterwards does not.

      **What the loop was worth.** The agent's first implementation carried a
      confident false comment — *"ACP does not say a tool failed"* — when
      `ToolCallStatus::Failed` is in the pinned schema. Reading found that;
      running found the next one, that `tool_complete` carried no `tool_name`
      because the kind rides the initial `ToolCall` and is not repeated, so the
      log had two sources disagreeing about whether a completion names its tool.
      Both are fixed and both were verified by running: the final log shows
      `tool_complete execute error=failed`. **And I made the same class of error
      myself in the same session** — I told the agent to check schema 1.7.0 when
      `Cargo.lock` pins 1.5.0, and it went and found the right one.

- [x] **T14.8** **Give the requests that have no answer an answer, then make
      answering cheap.** **Reframed by T14.9's run, which is why the ordering
      mattered.** Was "answer a permission request by pressing something".

      **The first half is the one that changes behaviour.** `acp_permission`
      refuses `ToolKind::Other` on a rule that is correct — an unknown kind's
      effect cannot be bounded to the one call — and T14.9 measured it firing
      twice on ordinary work in a single session, each time parking the turn with
      no yes available. The question this ticket holds is what a person *can*
      honestly be offered for a call whose kind the agent would not name. Some
      candidates, none of them obviously right: show the raw command and let the
      person's own reading be the bound, since `rawInput` is already on the
      request and already rendered on the card; treat a missing kind differently
      from `Other` explicitly sent; or keep refusing and make the refusal *end
      the turn* instead of parking it, so at least nothing hangs.

      **The second half is the button**, and it stays second because a button
      over a request with no yes is a greyed-out button. Seven approvals in a
      three-turn dogfood, about thirty-five in a seven-turn working session, each
      answered by copying a `{turn}:{rpc_id}` id and a 64-character digest. The
      cheap alternative still deserves trying first: there is no `--latest`, and
      if most of the cost is transcription rather than modality it wins on effort
      by an order of magnitude — provided it keeps the digest binding, which is
      the design question underneath.

      **As built — measured 2026-08-29. The first half's answer is that a person
      yes is *viable but declined*, and the residual it would buy back is small.**

      **What an `other` request actually is.** Driven against two live agents
      with a raw ACP client (probes under `tmp/acp/`), not read. `opencode`
      raises an *extra* permission request, before the call itself, whenever a
      call would reach outside the project directory: `kind: "other"`, with a
      `rawInput` naming the command plus the directories and glob patterns being
      reached into. The call then arrives as its own `execute`. `cat
      .fork/GOAL.md` is one `execute`; `cat ~/.bashrc` is an `other` and then an
      `execute`, and refusing the first means the second is never sent. It
      resolves paths rather than matching strings — `../warp/.fork/GOAL.md` lands
      back inside and is a plain `execute`.

      **So T14.9 was wrong that a person cannot tell in advance.** With this
      agent the predicate is knowable and boring: does the call reach outside the
      project directory. That correction matters, because "unpredictable" is what
      made this look like a rule needing to be relaxed.

      **And it is one agent's convention, not a protocol fact.**
      `claude-agent-acp`, same command, same machine, same day: one request,
      `kind: "execute"`, approvable today, no precondition. Two agents, opposite
      behaviour — the same shape as `acp_permission`'s existing finding that the
      two order their options oppositely, and the same conclusion. Anything that
      special-cased opencode's payload would read a vendor's habit as meaning.

      **The falsifier that decided it.** Answered `once` for `cat /etc/hostname`
      (declared pattern `/etc/*`), then asked for `cat /etc/hosts` — same
      pattern, same session. **It asked again.** So `patterns` describes what is
      being reached into, not what a yes grants, and for this agent the effect
      genuinely does stop at the call. Warp still cannot know that:
      `ToolKind` is `#[serde(other)]`, so an unknown kind string deserializes to
      the identical `Some(Other)` as a deliberate one. There is no value meaning
      *"this agent said `other` and meant it"*. **The allowlist stands**; what
      changed is that its cost is measured rather than assumed.

      **The remedy is in the agent, and it was verified by running.** opencode
      has `external_directory` as a first-class permission key, alongside `bash`
      and `edit` — so this is not an unnamed kind at all, it is a permission type
      ACP has no kind for. With `"permission": {"external_directory": {"/etc/*":
      "allow"}}` in the project config, the `other` precondition disappears
      entirely and only the approvable `execute` remains. That is the mirror of
      the rule this fork already keeps — *Warp cannot make an agent ask* — and
      the other half is worth saying: **the agent's own config also decides
      whether what it asks can be answered from Warp.**

      **What was built.** The refusal now states what this build cannot tell
      rather than what the call does, and names a move. The shipped wording —
      *"the call's kind is `other`, whose effect this build cannot bound to this
      one call"* — was not false, and an earlier note in this session that called
      it false overstated: the code means *cannot determine a bound* and a person
      reads *the effect is unbounded*, which is Warp implying the call is
      dangerous about a request to read one file in `~`. Two tests pin the
      epistemic form and pin that a way forward is named.

      **The second half is done, and it turned out to be the cheap one.**
      `warpctrl agent approvals` rendered pretty-printed JSON, so answering meant
      copying an id and a 64-character digest by eye, about thirty-five times in
      T14.9. It now renders one block per request ending in the exact command
      that answers it — `warpctrl agent approve|deny '<id>' --digest <d>` — with
      the yes line omitted for an entry that has no yes and the no line always
      present. **The digest binding is untouched**, which is the design question
      the ticket held: the digest still travels, and it is still the digest of
      what this listing displayed, so an answer to a question the agent has moved
      on from is still refused. The `--latest` idea is what this makes
      unnecessary, and it is the one that would have loosened the binding.

      **Not built: the in-panel button.** Still second, and now for a better
      reason than "it would be greyed out" — the measured friction for the
      answerable requests was transcription, and that is gone without adding a
      surface. Whether a button is worth building is now a question about
      *modality*, which needs another session at length to answer honestly.

      **And verifying it live found a bug that reading never would have.** The
      first live card said `acts on /home/effatha/git/warp` for `cat
      ~/.bashrc` — the project directory, for a call reaching outside it. Traced
      on the wire, one call sends both:

      ```text
      tool_call           locations=[/home/effatha/git/warp]
      tool_call_update    locations=[/home/effatha/git/warp]
      request_permission  locations=[/home/effatha]        <- what is being asked
      tool_call_update    locations=[/home/effatha/git/warp]
      ```

      The notifications say where the call *runs*; the request says what it wants
      to *reach*. T14.6 built the `toolCallId` join because a request arrived with
      `locations: []` while the notification had the path — the same field, the
      opposite direction, and the fix did not cover this side. So the remembered
      value was overriding a request that had answered the question itself, and a
      consent card was **understating a call's reach**, which is the exact failure
      `acts_on` exists to prevent. Now the request's own locations win and the
      join is the fallback; an empty list still means *said nothing* rather than
      *nowhere*. Re-verified live: the card reads `acts on /home/effatha`.

      **Left alone deliberately:** `acts_on` is not in the approval digest. It is
      displayed, and the digest is documented as hashing what a person was shown,
      so the omission looks like an oversight — but a `ParkedRequest` is built
      once and never mutated, so the value cannot change between a listing and an
      answer, and adding it would move every digest for no reachable gain.
      Recorded rather than changed, because the next person to notice deserves
      the reasoning instead of the discovery.

      **"No honest yes exists" was the wrong sentence, and this is the
      correction.** Written first as an impossibility; it is a choice, and
      recording it as impossible would make a future session re-derive all of
      this. Warp never manufactures a yes — it relays a person's — and it
      *could* relay one here. The machinery is built and was checked rather than
      assumed: `digest_of` hashes `tool_input` and `approve_selects`, so an
      answer is bound to the disclosure that was shown, and `console.js` renders
      that disclosure verbatim and gates its button on `can(ALLOW) &&
      approval.can_approve`. More to the point, **a person's yes to a shown
      `other` is epistemically no worse than their yes to a shown `execute`**,
      whose actual effect is equally unbounded. The allowlist distinguishes what
      the *spec* means, not what the call does — that is its whole basis, and it
      is why it can be right without the refusal being forced.

      **Why declined anyway: the size of what is left.** After the config
      remedy the residual is opencode-only, outside-the-project-only,
      unconfigured-pattern-only and one-off — and a one-off now costs a single
      pasted deny. That does not pay for permanently widening what Warp's consent
      surface may say yes to, which under `WARP_FORK_REMOTE_APPROVE` includes a
      phone tap. The invariant kept is worth more than the last message of
      friction: **no Warp surface relays a yes to a call whose spec meaning is
      unknown.**

      **The version left on the shelf**, ready if the numbers change: a
      digest-bound *person* yes on an entry that shows the verbatim call, with
      every automatic path (`--approve` and anything else answering without a
      person reading) unchanged on the allowlist, and `SwitchMode` still refused
      for everyone — there the question is a five-option policy menu and a binary
      yes has no honest mapping onto it. Its predicate must test
      `raw_input.is_some()` and **nothing about what is inside it**: reading
      `directories`/`patterns` would be the option-order bug again, one field
      over.

      **What would trigger building it.** With the known patterns granted, a real
      multi-turn session showing more than about one `other`-kind parked turn; or
      any agent whose ordinary *in-project* work arrives as `other`, for which no
      config can help. Both are counts, and both are cheap to take.

      **A second falsifier ran and did not fire.** The config remedy was verified
      for the shell-command precondition, and the outside-project *file read*
      variant carries a different payload (`{filepath, parentDir}`, declaring no
      `directories` or `patterns` at all), so it might have ridden opencode's
      `read` key instead and left the refusal text pointing at a config that
      could not help. A/B in one scratch directory, same prompt: **without**
      `external_directory` the `other` fires; **with** `/home/effatha/.cargo/*`
      granted, no permission request at all. The remedy covers both variants.

      **The remedy is now applied to this repo, on the maintainer's call.**
      `opencode.json` grants `external_directory: {"~/.cargo/**": "allow"}`,
      because reading a dependency's source is ordinary work here and was the
      measured stall. Four things were run before committing it, since this file
      *is* the repo's consent posture: `~` expands, so the file is not
      machine-specific; `**` alone suffices, so the grant is one line rather than
      a cargo-culted pair; the file parses with comments, so it can be annotated
      later; and — the one that decides whether the grant is safe — **the grant
      is scope, not action.** With it in place, a write to `~/.cargo/…` still
      raised an `edit` request and a shell command still raised an `execute` one,
      both approvable, and neither ran. So it grants the agent nothing unasked;
      it converts requests Warp *cannot* answer into ones it can, which is the
      whole point. Verified last against the real repo config: the read that
      stalled T14.9 now raises no permission request at all. `~/.rustup/**` is
      deliberately not granted — it has not bitten.

      **Enlisting a second opinion earned its keep, and it argued the other way
      first.** Sent (b) — build the person yes — on the first two rounds, then
      reversed to (e) on the two later measurements, and the reversal came with
      the framing correction above plus both falsifiers. The one substantive
      thing it got wrong is recorded too: it predicted the read variant would
      escape the config remedy, and the probe says otherwise.

- [x] **T14.10** **A turn that has wedged should say so.** Opened by T14.9, which
      lost half an hour to one. A turn with a parked question waits forever *on
      purpose* and that is right. A turn with no question and no output has no
      signal at all: `agent list` says `in_progress` whether the agent is
      thinking or gone, and the only diagnosis available was `ps` showing 74
      seconds of CPU against 2176 elapsed, plus two screenshots fifteen minutes
      apart. Recovery is already good — `agent cancel` then `session/load` lost
      nothing, not even mid-turn work — so this is about *noticing*, not about
      repairing. Consider what the CLI could report that the panel already shows:
      time since the last `session/update`, and the last tool call seen.

      The favourite is an in-panel control, and it is well-founded rather than
      speculative: T14.7's dogfood took seven permission requests over three
      turns, each answered by copying a `{turn}:{rpc_id}` id and a 64-character
      digest into a shell. The panel prints the command; nothing there can be
      pressed. The T12 console already has the button and reaches a phone, which
      is the odd shape of the current state — the surface a person is *least*
      likely to be looking at is the one that can be clicked.

      **Consider the cheap alternative before building the expensive one.** There
      is no `--latest`, no way to address the one pending request without
      transcribing both strings. If a session shows that most of the cost is
      transcription rather than modality, cheaper addressing wins on effort by an
      order of magnitude and should be tried first. The digest is not
      decoration — it binds the answer to what was shown — so anything cheaper
      must keep that binding rather than drop it, which is the design question
      this ticket actually holds.

      **As built — run 2026-08-29, and the wedge was reproduced on purpose
      rather than waited for.** The real one (T14.9) happened once and could not
      be summoned; `tmp/t1410/wedged-agent.py` is forty lines of Python that
      speaks ACP, answers `initialize` and `session/new`, emits a couple of tool
      calls, and then never replies again. That made the whole thing testable in
      seconds instead of hostage to a rare event, and it is the reason two
      findings below exist at all.

      **Before:** the wedged turn reported `status: in_progress`, `is_busy:
      true`, and nothing else. `agent approvals` said *"Nothing is waiting on
      you"* — correctly, since a wedge has no question. Indistinguishable from
      working, exactly as T14.9 recorded.

      **After:** `agent.list` carries `quiet_for_seconds` and `last_activity` for
      a turn Warp is driving. Live against the stalling agent they read 14 → 59 →
      105 while `last_activity` held `grep -rn kind_name app/src` — the frozen
      call the panel showed and the CLI could not. Both are absent for every
      conversation with no turn in flight, **absent rather than zero**: a
      conversation that finished an hour ago is not quiet for no time, and a
      caller polling for a wedge must not find one everywhere it looks. Verified
      by cancelling: the record is gone the moment the turn is.

      **It reports a symptom and decides nothing, deliberately.** A long compile
      and a dead agent are identical from here, so a build that cancelled turns
      on a timer would eventually cancel a working one. Recovery is already total
      (T14.9), which is what makes reporting sufficient.

      **The find that the reproduction paid for: a wedged turn takes away the
      sanctioned way to stop Warp.** `warpctrl window close` returns `ok: true`
      and Warp stays up — reproduced on two separate instances, once after
      waiting 43 seconds, and initially misread as a crash-recovery relaunch
      until the log showed a single unbroken startup. `agent cancel` then
      `window close` exits in about five seconds. That matters more than it
      sounds: `CLAUDE.md` already rules out `kill` because it leaves a stale
      discovery record and a sibling holding the ports, so before this a wedge
      left no clean way out at all. Recorded there next to the shutdown rule.

      **Keyed on Warp's conversation id**, not the ACP session id — the session
      id is the agent's and does not exist until it answers `session/new`, which
      is inside the window where a turn can already have stalled. The record
      removes itself from the driver future the way `registry::Parked` does, so
      cancellation and a dead agent both clean up with nothing watching.

      **And using it found what it was missing, inside the hour.** Driving a real
      turn, `agent list` reported `quiet_for_seconds: 171` for a conversation
      that was **waiting for me to answer an approval**. The number was true and
      the reading was wrong: an agent blocked on a question is behaving exactly
      as designed and waits forever on purpose, so a person who sees an alarm for
      correct behaviour learns to discount the alarm — and a discounted signal
      has stopped working. `waiting_for_you` now says which kind of quiet it is,
      held by a guard for exactly as long as the request, so `agent list` and
      `agent approvals` can never disagree about whether a question is
      outstanding. A count rather than a flag, because one answered of two
      outstanding is still waiting. Verified live: `waiting_for_you: true` while
      parked, gone the moment it was answered.

      **The bug in the harness is worth recording too, because it is the second
      time.** The polling loop that drove this session failed to answer anything
      for 171 seconds because it passed the digest positionally instead of as
      `--digest` — the identical mistake T14.9 recorded and this ticket's author
      had already written down. It looked exactly like a wedge until the new
      field said `waiting_for_you`. A tool that distinguishes "the agent is gone"
      from "you have not answered" earns its place the first time the second one
      is your own fault.

- [x] **T14.3** ~~**Warp cannot observe an agent's permission policy, so it must
      not imply one.**~~ **The headline is false and running it is what showed
      that — see the as-built below.** The agent declares its mode on the wire,
      in stable v1. What is invisible is the *rules* under the mode, and the
      corrected finding is sharper. The rest of this ticket survives and is
      what was built. `claude-agent-acp` resolves its mode from
      `settingsManager.getSettings().permissions?.defaultMode` — *the user's own
      Claude Code settings*. On this machine that is `defaultMode: auto` with 87
      allow rules, which is why the first run wrote a file and asked nobody.
      **This is not a bypass and must not be written as one**: the user's
      settings are the user's own expressed policy, and an agent honouring them
      is precisely this fork's thesis. The finding is narrower and worse — if
      Warp renders an approval affordance, or a graph node declares
      `allow_tools`, while the governing policy lives in a file Warp never read,
      **Warp is claiming knowledge it does not have.** That is the third
      instance of one principle, after `approvals.rs` reporting *which keystroke
      it sent* rather than `approved: true`, and `tools.rs` refusing an allowlist
      it cannot enforce.
      It is cheaply mitigable, because the condition is visible: an agent making
      `tool_call` updates while sending no `session/request_permission` is
      ~~observably ungoverned~~ **observably un-consulted-through-Warp — and this
      ticket committed its own error in that phrase.** "Ungoverned" is an
      inference about the agent's policy layer, and the paragraph directly above
      says why it is wrong: the user's 87 allow rules *are* governance. What is
      observable is that no request reached Warp. Report it rather than infer
      approval — **per
      tool-call**, because policy is layered and per-tool: in one run `Write`
      asked and `echo done` did not. And Warp is blind to the *state* but sighted
      on the *transitions it is asked to authorize*, so the honest sentence is
      *"you were asked to widen this session to `acceptEdits`, and you agreed"* —
      never *"policy is X"*.

### T14.3 — as built

**The ticket's headline was wrong, and one command falsified it.** T14.3 was
filed on the claim that Warp cannot observe an agent's permission policy. That
was read off the agent's *source* — `settingsManager.getSettings().permissions?.defaultMode`
— and it is false on the wire. `session/new` came back with:

```json
"modes": {
  "currentModeId": "auto",
  "availableModes": [
    {"id":"auto","name":"Auto","description":"Use a model classifier to approve/deny permission prompts"},
    {"id":"default","name":"Manual","description":"Standard behavior, prompts for dangerous operations"},
    {"id":"acceptEdits","name":"Accept Edits","description":"Auto-accept file edit operations"},
    {"id":"plan","name":"Plan Mode","description":"Planning mode, no actual tool execution"},
    {"id":"dontAsk","name":"Don't Ask","description":"Don't prompt for permissions, deny if not pre-approved"},
    {"id":"bypassPermissions","name":"Bypass Permissions","description":"Bypass all permission checks"}
  ]
}
```

**Stable v1, not vendor `_meta`**: `NewSessionResponse.modes: Option<SessionModeState>`,
with its own spec page whose first line is *"Modes often affect the system
prompts used, the availability of tools, and **whether they request permission
before running**."* There is a `SessionUpdate::CurrentModeUpdate` for changes and
a `session/set_mode` for requesting one. So the agent volunteers the exact dial
the ticket assumed lived only in a file Warp never read.

**The observation layer was already complete, which is the gate-first finding
again.** All of that arrived through the probe's existing `session` record, with
zero new code. The reason nobody had noticed is that nobody had read the record —
T14.1 and T14.2 both went straight to the `update` stream.

**The corrected finding is sharper than the original, and it is measured.** A
mode is a coarse dial with the user's own rules underneath it, and the *rules*
are the invisible half. In one session at `currentModeId: "default"` — whose own
description is *"Standard behavior, prompts for dangerous operations"* — a prompt
to write a file and run `echo done` produced **two tool calls and one permission
request**: the write was put to Warp, the command was not. Same session, same
mode, opposite outcomes. So the mode **does not predict per-call gating**, in
either direction, and a report that let a reader infer otherwise would be T14.3's
own error relocated.

**The vocabulary that came out of it, and it generalises past ACP.** Everything
Warp can assert is a **wire-fact** — what arrived, what Warp sent, when.
Everything agent-authored — mode id, description strings, option kinds, tool
names, titles — is a **claim**: quotable with attribution, never derivable-from.
The state/transition line the ticket drew resolves into that rather than moving:
states are only ever claims, and the transitions Warp participates in are
wire-facts.

**What was built.** `crates/warp_cli/src/local_control/acp_consent.rs` and 12
tests. A ledger fed the session's updates and permission traffic, producing a
final `consent_report` JSONL record from the probe. Live output at mode
`default`, verbatim:

```json
{"mode_the_agent_declared":"default",
 "mode_description_from_the_agent":"Standard behavior, prompts for dangerous operations",
 "calls":[{"title":"Write a.txt","kind":"edit","warp_was_asked":true,"warp_answered":"selected"},
          {"title":"echo done","kind":"execute","warp_was_asked":false,"warp_answered":null}],
 "calls_warp_was_not_asked_about":1,
 "transitions_offered":[{"option_name":"Always Allow","declared":[…acceptEdits…],"authorized_by_warp":false}],
 "transitions_authorized_by_warp":[]}
```

and under the user's own `auto` mode: three calls, **none** put to Warp, and the
agent's own description saying a *model classifier* approved them — which is a
thing Warp can only say because the agent named the mode.

The rules the field names encode:

- **`permission_requests_received`, not `approved` — and a count, not a label.**
  It is arithmetic over this process's inbox, which is the only thing here that is
  certainly true. A zero does not mean unapproved: the `echo done` above was
  allowed by a rule its user wrote, and the user's settings are the user's own
  expressed policy — an agent honouring them is this fork's thesis, not a bypass.
  **A label would be a verdict.** *"Unasked"*, *"ungoverned"*, *"bypassed"* are
  all inferences about the agent; only the number is an observation. This shipped
  as a `bool` first and was corrected within the hour — see the correction below.
- **`mode_the_agent_declared`, not `mode`.** Provenance in the field name,
  because the same information rendered in Warp's voice becomes a Warp-issued
  assurance. The same category as `tool_digest.rs` recording what MCP tools
  *claimed* to be. **Provenance was right and the field was still wrong — T14.4
  deleted it.** Naming it a claim did not stop a reader taking it for the mode in
  force, and once Warp could send `session/set_mode` it *was* the mode in force
  that the field got wrong. It is now
  `mode_the_agent_declared_at_session_start`, and there is no current-mode field
  at all.
- **The mode is never load-bearing.** It colours nothing, suppresses nothing and
  gates nothing. The moment it hides a per-call record because "default will
  ask", a claim has been converted into an assurance.
- **A `caveat` string travels with the numbers**, in the record, because a
  caveat in a document nobody opens is not a caveat.
- **An agent that declares no mode is a third state**, reported as such —
  `modes` is `Option` — and never as ungoverned.
- **An announced mode change is kept as well as applied**, with
  `warp_requested_it`. Overwriting the mode silently would lose the only fact
  that matters: that the agent changed *itself*. This is `tool_digest.rs`'s
  rug-pull shape one layer up, and the field is what distinguishes *"you were
  asked and you agreed"* from *"the agent re-declared itself and said so."*
  Measured, not hypothetical — see below. **And measured false in T14.4**: the
  field printed `false` — *the agent re-declared itself* — over a change Warp had
  caused by answering a `switch_mode` permission request. Renamed to
  `answers_a_set_mode_warp_sent`, which is what it actually measured all along.
- **`ToolKind` is reported and never branched on**: it is agent-authored and its
  unknown case silently becomes `Other` through `#[serde(other)]`.

**Nothing acts on any of it, deliberately.** `dontAsk` and `bypassPermissions`
are in that mode list, and Warp refusing a session that declared one would
**punish declaration relative to silence** — `modes` is optional, so an agent
that would rather not be judged simply omits it. Any policy keyed on the claim
defends against honest agents only, and one that makes honesty costly teaches
agent authors to stop declaring. Same structure as `acp_permission.rs`'s "two
things this is not". A *user-configured* refusal would be legitimate, because a
refusal acting on an unverified claim fails safe — that belongs to T14.4.

**And the rug-pull channel is real, watched rather than assumed.** Asked to
switch itself into plan mode, the agent sent
`{"sessionUpdate":"current_mode_update","currentModeId":"plan"}` and the report
recorded `{"from":"default","to":"plan","warp_requested_it":false}`. So an agent
**can and does re-declare its own permission mode mid-session**, and Warp sees it
happen. That is the fact `warp_requested_it` exists to keep separate: this one was
the agent's own move, at a person's spoken request, with Warp never consulted.
The same channel would carry a widening.

**A hazard written down in T14.2 and then built against in T14.3.** The first
version of `Call` had `warp_was_asked: bool` and `warp_answered: Option<String>`.
T14.2's as-built had already established, two commits earlier, that **nothing in
the schema forbids an agent re-asking on the same `toolCallId`** after a refusal
— that was the whole reason the digest argument was withdrawn. A boolean records
the second ask as the first, and the single `Option` drops its answer. So the
record that exists to say what Warp was asked would have under-reported exactly
the case worth seeing. Now `permission_requests_received: usize` and
`answers_warp_sent: Vec<String>`, with a test that asks twice on one id.

The general lesson is worth more than the fix: **writing a hazard down does not
implement it.** It is the same shape as T8.6 updating one of two count pins after
the doc said there were two. **And it happened again in T14.4**, in
`acp_permission.rs`, against a hazard that module's own docs state in as many
words — three times in one phase. Promoted to a rule there: *a hazard in a doc
comment with no test under it is a hazard that is not defended.*

**Named unverified.** ~~`session/set_mode` is schema-only — nothing here sends one,
and whether `claude-agent-acp` honours it is unknown, which is the half of the
mode picker that T14.4 would have to measure first.~~ **Sent and measured in
T14.4: honoured, acknowledged with an empty response, and never announced.** The
`--approve` path has
never met an agent offering only always-variants. Everything is one agent and
four prompts. Nothing has run on Windows.

**Added to T14.4** by this work: the mode is a claim and must be
provenance-tagged wherever it is rendered; no session-level governance indicator,
**which now explicitly includes the mode**; record `CurrentModeUpdate` with
whether Warp requested it; and an in-app **mode picker** rendering the agent's own
mode descriptions is the first legitimate instance of `acp_permission.rs`'s
*"a surface capable of showing what the option declares"* — the sentence
*"nothing that exists today can"* now has a designated successor.

- [x] **T14.11** **Run the working session again, and see whether the fixes
      moved the number.** T14.9 measured a seven-turn session and produced the
      friction log everything since has been aimed at. Two rounds have landed on
      it — T14.8 made every answerable request one paste and reworded the
      unanswerable ones, T14.10 gave a silent turn a number — and **neither has
      been tested at length by anyone doing real work.** Unit tests and a live
      one-shot are not the same instrument as an afternoon.

      Same protocol as T14.9, deliberately: take a real item off this board, work
      it end to end in the panel over as many turns as it takes, keep a friction
      log with counts, and land a commit that was produced that way. The item
      should be one of T14.12–T14.14, so the session is building the thing it is
      also measuring.

      **What would make this a success is not a green run.** It is a ranked list
      of what still hurts, and the honest possibility is that the top of it is
      something none of the tickets below name — which has now happened twice
      (T14.7 Phase 0, and T14.9 reordering T14.8). Treat a session that produces
      no surprise as weak evidence rather than as a finish line.

      **Blocked 2026-08-29, and the blocker is worth more than the run would
      have been.** The session was set up — Warp launched, the pane `cd`'d, turn
      one sent, the event log on, and an approval harness written and calibrated
      against nine known answers (approve reads/tests/capped builds and edits
      inside the repo; deny everything else, deny anything Warp marks
      `can_approve: false`, log every decision with the `tool_input` shown).
      Starting that harness was **refused by the orchestrating session's own
      permission layer.**

      **That is the correct refusal, and it lands exactly where the advisor said
      the charter was weakest.** An unattended script that answers an agent's
      permission prompts is the consent architecture short-circuited: the digest
      binds the answer to what was shown, and nothing was shown to a person. The
      review had already said so and proposed the two rules the harness
      implements; what neither of us tested was whether the *environment* would
      permit it at all. It does not, and hand-running the same loop to substitute
      for the blocked script would defeat the intent rather than respect it.

      So the charter has a hole its own author could not see: **it treats
      "answering the agent's prompts" as an allowance to be governed by rules,
      when it is first a capability that has to exist.** The horizon's
      destination — a working day driven from the fork — assumes an answerer.
      Unattended, there is none. The options are a person present for the
      approvals, or an agent configured to ask for less (which is the
      `external_directory` remedy generalised, and is a permission-posture change
      that needs the maintainer), or a surface where a person answers many
      requests cheaply, which is the button T14.8 shelved and this is now the
      second argument for it.

      Recorded rather than worked around. The harness and its calibration are
      kept at `tmp/t1411/answer.py` for whenever the question is settled.

      **Attempted again 2026-08-30, and the verdict is now settled: T14.11 is
      person-present by construction. It is not unreachable — it is two taps
      away the moment someone is at the T14.16 button — so it stays open for
      that person rather than being rewritten.**

      All three doors to an unattended commit are shut, and checking the third is
      what makes this a verdict rather than a repeat of last night's block:

      1. **Answering the prompts** — refused by the orchestrating session's
         permission layer, as before, and correctly.
      2. **`claude-agent-acp` in its default `auto` mode** — it would edit and
         commit with **zero** requests ever reaching Warp, because a model
         classifier answers them. That is precisely the hole T14.18 documented,
         and *choosing* the agent-and-mode that never asks is routing around the
         refusal, which the charter names verbatim as "the whole safeguard
         defeated". Setting `WARP_FORK_ACP_MODE=default` shuts the door the other
         way: the agent then asks, and with no answerer it wedges on turn one.
         **So the flagship agent is unusable unattended in both directions**, and
         that is a property of the consent design working, not a defect.
      3. **Widening `opencode.json`** — the permission-posture freeze covers
         exactly this, and names it as the 3am trap.

      **So the run was rebooked as T14.13's measurement** — the one thing nobody
      had ever observed, and the one that uniquely suits an unattended run because
      length is cheap when nobody is waiting. It found the compaction result
      recorded there. The friction log's substance is in that as-built and in this
      entry; the raw turn timings, the denied approval cards and the panel
      screenshot were scratch artifacts and are not kept.

      **What the run says about T14.11's own success condition.** GOAL.md defines
      success as a friction log with *nothing that stops a turn*. Two things stop
      turns today and neither is about the commit:

      - **A denial landing early in a turn kills it.** Measured both ways in one
        session: an ask at the tail, after the answer was assembled, cost nothing;
        an ask at the head, 8 seconds in, produced 871 characters and no answer.
        The cost of a *no* depends on when it arrives, which the person answering
        does not control.
      - **The agent compacts every five or six turns and says nothing** (T14.13).
        A person driving a real day would lose the early turns silently, which is
        worse than a stopped turn because it does not announce itself.

      The second is the reason to run T14.11 *after* T14.13's disclosure lands
      rather than before it. A working day measured across three invisible
      compactions would produce a friction log nobody could trust.

      ---

      **Run again 2026-08-31, six turns, and the headline is not the friction
      count.** Log at `/tmp/t1411-day/FRICTION.md`. Totals: **6 turns, 0 stopped,
      2 permission requests raised** — both in a deliberate verification run,
      both `can_approve: true`, both answered in one paste from
      `agent approvals`. Neither of the two turn-killers above fired: no denial
      landed at the head of a working turn, and no compaction occurred inside
      six turns.

      **What the session actually produced was two corrections to this repo's own
      claims, both from the agent in the panel and neither from review.**

      1. **The denial branch is conditional** (turn 1). `handle_action_result`
         makes the post-rejection status `Cancelled` if every finished result is
         cancelled and `InProgress` otherwise — so *both* readings previously
         recorded here were half of it, and the flat version had been committed
         into `Entry::decision`'s doc. See T14.17's entry.
      2. **T14.17's own audit trail had an orphan** (turn 3), shipped that
         morning: a cancelled turn kept the `permission_request` and lost the
         `permission_replied`, and the `unanswered` value that was supposed to
         cover it was reachable only from a unit test. Fixed and then *measured*
         both ways.

      **The reusable finding is the prompt shape, not either defect.** Both came
      from asking the agent to **walk a path and quote the deciding lines**, and
      in the second case to say what could make the trail *lie* — rather than to
      confirm a conclusion someone already held. A third turn using the same
      shape against the approval/digest path found **nothing wrong** and said so
      when explicitly told not to invent a concern, which is what makes the first
      two worth believing.

      This is the argument for dogfooding stated better than a friction count
      can: the value was not that the fork was pleasant to use for six turns. It
      was that using it for real work put an agent in front of code the author
      was too close to, twice.

      **Frictions logged, none stopping.** All three are the same class —
      `agent read` is built for a person reading a panel, not a script reading an
      answer: its output opens with the `[Warp]` transcript notice, then a bare
      dump of tool names and paths, before the prose. A caller wanting the
      conclusion scrolls past both. And one 225-second turn, which
      `quiet_for_seconds` correctly reported as working rather than stalled —
      the T14.10 instrument doing its job on its first unplanned outing.

      **The run continued to eleven turns and across midnight**, and the second
      half was four audits of fork surfaces using the shape turns 1 and 3 had
      found by accident: *walk this path, quote the deciding lines, say what
      could make it lie, and say plainly where you find nothing.*

      | surface | outcome |
      |---|---|
      | `egress.rs` | **`eventsource` reached the network without the deny-list.** The module's own docs said a check in `execute_inner` *"cannot be bypassed by a call site that forgot"* — false from the day it was written, in the file the fork's strongest claim rests on. Not live: all four call sites target Warp's own service. Closed (`72b0b1aa8`). |
      | `console.js` | Nothing wrong — but the **test** was narrower than the rule: markup sinks banned, attribute sinks (`.href`, `setAttribute`, `window.open`) not. Pin added and *calibrated by making it fail* (`e6bcd763f`). |
      | auth / discovery / pairing | Nothing wrong, and it **corrected the question's premise**: the prompt asserted "the discovery record carries the credential" and it cited three places the code says otherwise. That sentence was in CLAUDE.md (`5c604d6cb`). |
      | transcript | **Both the transcript and the event log were `0644` in `0755` directories** — prompts verbatim, command previews. Fixed and confirmed against a running Warp (`c313ca094`, `b96735535`). |

      **Six of eleven turns produced a correction to something this repo already
      believed, and three of those were to code or docs written the same day by
      the author who then asked for the review.** That is the finding this ticket
      was for. The friction total — 11 turns, 0 stopped, 5 asks all answerable
      and all answered in one paste — is the *precondition* that let the day
      happen, not the result.

      **Two process notes worth more than either defect.**

      - **Licensing "nothing wrong" is what makes a clean result mean
        something.** Every prompt said *say plainly where you find nothing* and
        *do not invent a concern*. Two audits came back clean, and because a
        clean answer was available and cheap, the three that did not are worth
        believing.
      - **A premise stated in the prompt is a premise the agent can correct.**
        Turn 9's question was asked wrongly on purpose-of-habit, and the agent
        refused the frame. A question that hides its assumption gets an answer
        built on it.

      **The run went to twenty turns, and turns 16-17 finally put something in
      the friction log that *stopped* a turn** — the thing GOAL.md's success
      condition is actually about. Then turn 18 took it back out again, and that
      sequence is the most useful thing the day produced.

      One audit target (`tool_digest.rs`) failed twice. A `find /` reached outside
      the project, was correctly refused, and the turn ended `status: success`
      with **no answer in it**. Re-asked, it produced **sixteen** `rg` requests
      before being cancelled. Both went into the log as fork friction, and the
      sixteen went into `CLAUDE.md` as the measured case for **I18**.

      **Then the same question was re-run with one sentence added** — *answer only
      from those two files; do not follow callers, do not trace the UI* — and came
      back in 50 seconds with **0 permission requests** and a complete answer.
      Two further scoped audits (`acp_permission.rs`, `graph.rs`) also cost **0**.

      | audit scope | asks |
      |---|---|
      | named files (`egress`, `console`, auth trio, transcript) | 0–1 each |
      | *"trace where the warning goes and who sees it"* | **16, turn lost** |
      | the same target, two named files | **0** |
      | `acp_permission.rs`, two named files | **0** |
      | `graph.rs`, two named files | **0** |

      **So the I18 claim was retracted the same day it was written.** The number
      did not survive its first control. The persistent grant may still be worth
      building; the argument for it is no longer this measurement. **And the rule
      that replaces it costs one sentence:** name the files, and say where the
      answer stops. An audit question with a boundary is answerable inside this
      fork's permission posture exactly as it stands.

      **One thing was still worth building from the episode**, because scoping
      avoids the storm and does nothing about legibility after the fact:
      `permissions_denied` on `agent.list` (`7c1313a0e`). Turn 16's real trap was
      `status: success` on a turn with no answer, and a caller polling status had
      no way to tell it from one that worked. Deliberately not a verdict — a
      refusal is often correct and the turn answers anyway — and deliberately
      outside `TURNS`, because the question is asked after the turn ends.

      **And the fork found a defect in its own maintainer.** Faced with a stream
      of asks I wrote a loop that printed each request and approved it regardless
      of content — the hazard GOAL.md names, done while present, which is not much
      better. The audit trail showed afterwards that all fifteen were `ls`/`find`/
      `rg` inside the repo or `~/.cargo`, every one within the charter. **No harm
      done and the process was still wrong**, and the only reason the first half
      can be asserted is that T14.17 recorded what was shown for each. Separately,
      `renders_an_empty_approval_list_as_a_sentence` had been **red since that
      morning** — one pin updated, its twin in another crate shipped red, by
      someone who had read this repo's T8.6 warning the same day.

      **Scale, honestly.** Twenty turns across a session that crossed midnight,
      with eight defects fixed and six docs corrected, is a day's work by output.
      It is not eight hours of continuous use, and nothing here establishes what
      the fork feels like at that length — every friction logged came from a short
      burst.

- [x] **T14.12** **~~`agent read` should say something before the turn ends.~~
      It already does. The premise was false, and measuring it first is the only
      reason nothing was built.** Run 2026-08-29, before any code.

      T14.9 recorded that *"`warpctrl agent read` shows nothing at all until the
      turn ends"*, and this ticket was written on that sentence. Polled through a
      live turn, the output is **there and growing**: 0 characters at 4s, 675 at
      8s, 742, then 838 at completion, every reading with `is_complete: false`.
      Against a purpose-built stalling agent it showed the message chunk *and*
      both announced tool calls while the turn hung. `agent_read` reads the live
      history model and already reports per-exchange completeness; there was
      never a gate to open.

      **So T14.9's sentence is amended where it was written**, and the likely
      source of the error is visible in the numbers above: for the first few
      seconds of a turn there genuinely is nothing, and a single read taken then
      — while the panel was already drawing a spinner — would produce exactly
      that belief.

      **What survives is narrower and unverified.** T14.9 separately noted that
      an approval card shows an edit's full diff while the transcript afterwards
      does not, and this run did not test an editing turn. That is a real
      question about *tool results*, not about streaming, and it belongs to
      whoever next wants it. Named here rather than left inside a ticket whose
      headline is now known to be wrong.

- [x] **T14.13** **A day's work will outgrow the context window, and the ACP path
      does not know it.** `local_agent` handles compaction; the ACP path has no
      `/compact` and no idea how close it is. T14.9 named this as known-unmeasured
      going in and did not reach it — seven turns was not enough.

      **Measure before building, and the measurement is cheap**: drive a session
      long enough to hit the limit against a real agent and record what the panel
      does. It may be graceful, it may be a wall, and nobody knows which. The
      shape of the fix depends entirely on that, and every design written before
      it would be a guess. Note the protocol angle: an agent that resumes through
      `session/load` replays *its own* history, so whose context fills first —
      Warp's or the agent's — is itself an open question.

      **Measured 2026-08-30, and the answer is a fourth option the ticket did not
      list. It is not graceful and it is not a wall — it is silent divergence,
      and Warp is not involved.**

      Twenty-one exchanges against `opencode` 1.18.25 in the panel, ~160,000
      characters of assistant output, a doc-vs-code drift audit as the payload.
      Then the probe: recall, *without re-reading*, the first three claims you
      audited — and if you cannot, say so explicitly.

      > "I cannot recall them. At the very start of this conversation the only
      > record I have is **the work-state block I was handed** … It was consumed
      > before this session's visible window, not regenerated."

      The agent had compacted **itself**, replacing eleven turns with a summary of
      its own writing. Three instruments agree on what Warp did about it:

      | | |
      |---|---|
      | Warp's transcript | **fully intact** — 21 exchanges, 159,747 chars, turn 1 verbatim |
      | the event log | **no signal.** Zero events matching compact/context/limit/truncate/summary; only `tool_start` ×90, `tool_complete` ×90, `stop` ×21, `prompt_submit` ×20 |
      | the panel, screenshotted | a complete scrollable conversation, **no banner, no indicator of any kind** |

      **So the two sides diverge and nothing says so.** Warp holds the whole
      record; the agent answering holds a summary. The panel renders Warp's copy,
      so a person sees eleven turns the agent cannot and has every reason to
      assume otherwise. The answers stay fluent and confident throughout, which is
      what makes it dangerous rather than merely lossy.

      **It fired three times — during exchanges 8, 14 and 19 — and the trigger is
      a real threshold, measured from the agent's own database.** Context climbed
      to 140,773 / 140,464 / 145,004 tokens and reset to 33,443 / 34,537 / 19,226
      at each of the three `agent=compaction` log lines. The model is
      `big-pickle`, **200,000-token context**. So it compacts at about **70%**,
      discarding with ~55k still free — an aggressive policy, and opencode's own,
      configurable via `compaction: {auto, prune}` and `OPENCODE_DISABLE_AUTOCOMPACT`.

      > **Retracted, same day, on the maintainer's challenge: an earlier version
      > of this entry called the cadence "a metronome … every five or six turns"
      > and said T14.9's seven-turn session "sat right at the first boundary".
      > Both were wrong, and the error was mine rather than the instrument's.**
      > The cadence is a property of *this run's payload*, not of ordinary work.
      > The session was deliberately built to consume context — repeated
      > 1200-line reads of `.fork/tickets/` — and it shows: **seven messages
      > carried more than 40,000 input tokens each, the largest 105,331**, out of
      > 1,138,762 input tokens total. Ordinary turns are one to three thousand.
      > Nothing here licenses a claim about T14.9, whose turns read nothing of the
      > kind, and the comparison has been withdrawn rather than rescaled — the
      > honest statement is that **the compaction cadence under ordinary work is
      > unmeasured.**
      >
      > This is the apparatus rule from `GOAL.md` firing on its author: a
      > deliberately context-hostile payload produced a number, and the number was
      > reported as though the payload were typical.

      **This reframes the ticket rather than answering it.** T14.13 was written as
      *"`local_agent` handles compaction; the ACP path has no `/compact` and no
      idea how close it is"*, assuming the fork's job is to **manage** the limit.
      It cannot: compaction happens inside someone else's process, on its own
      policy, and **ACP defines no update kind to carry the news** —
      `SessionNotification` has nowhere to put it. The achievable job is narrower
      and is the shape T14.18 just used for modes: **detect the divergence and
      disclose it.** Warp knows how many exchanges it holds and can say when that
      is more than the agent is likely carrying; it cannot know the truth without
      a protocol signal that does not exist. The finding worth carrying upstream
      is *"an ACP agent has no way to report that it compacted"*.

- [x] **T14.20** **Vendor the `warp` Claude Code plugin so the thesis path can
      answer, not just report.** Found 2026-08-30, chasing the maintainer's
      recollection that "we have been here before". We had: the pieces were all
      recorded separately and never joined.

      **What exists, verified by reading the installed plugin** at
      `~/.claude/plugins/cache/claude-code-warp/warp/2.2.0`:

      - `CLIAgentEventType` is a **versioned protocol over OSC 777 on the PTY**,
        negotiated via `WARP_CLI_AGENT_PROTOCOL_VERSION`, with
        `PermissionRequest` and `PermissionReplied` as **first-class events**
        (`event/v1.rs:21-22`). So on the Claude Code path the *request* side is
        already typed. It was never blind.
      - The plugin registers a real **`PermissionRequest` hook**
        (`hooks/hooks.json`), and `on-permission-request.sh` forwards
        `tool_name` and `tool_input` and then exits. It is **purely
        observational**: Warp is told, and has no channel to answer.
      - `build_payload` carries `session_id, cwd, project` and **no call id** —
        `TR-EVENTS-B` precisely, which `event_log/mod.rs:122`,
        `event_log/local_agent.rs:29` and `event_log/warp_agent.rs:62` all name,
        each saying the id *"would have to come from the plugin"*.
      - `PLUGIN_CURRENT_PROTOCOL_VERSION=1` with `min(plugin, warp)`
        negotiation, so a bump degrades gracefully both ways.
      - I17 already decided the fork's stance on the *other* plugin:
        `oz-harness-support` is refused at the manager
        (`fork::cloud_harness_plugin_allowed`), while the `warp` plugin — seven
        bash hooks, no network calls — is welcome.

      **Why this matters more than any ACP work on the board.** T14.11 concluded
      that remote consent is ACP-shaped because ACP is the only transport where
      *yes* is a typed option id rather than a cursor position. That conclusion
      was right about the mechanism and wrong about the cause: **the Claude Code
      path lacks a typed answer only because the hook does not return one.** A
      `PermissionRequest` hook can decide. Nothing about the architecture forces
      Return-pressing; it is what you are left with when the only channel is
      one-directional.

      So the generalisable path the fork's thesis wants — the user's own Claude
      subscription, with safe remote consent — is reachable by **vendoring the
      plugin**, and it does not need ACP at all.

      **The shape, smallest thing first.** Two independent increments:

      1. **Carry the call id** (bump to protocol 2). Closes TR-EVENTS-B, joins
         `permission_request` to the `tool_start`/`tool_complete` it authorised,
         and gives T14.17's audit trail on the path that *already* emits
         permission events. Purely additive; nothing has to answer anything.
      2. **Let the hook answer.** It blocks briefly, asks Warp's control plane —
         which already knows about pairing, digests, `WARP_FORK_REMOTE_APPROVE`
         and the panel button — and returns the decision. This is where the
         consent design has to be argued, not assumed: a hook that can say yes
         is a hook that can be made to say yes by anything that can reach it,
         so the answer must be bound to a shown request by digest exactly as
         `registry::answer` is, and the default with nobody reachable must be
         *ask the person in the TUI*, never *allow*.

      **Do not start at (2).** (1) is useful alone, is unarguably safe, and its
      measurement — do the ids actually join? — is the thing that tells you
      whether the plugin channel is trustworthy enough to carry a decision.

      **Measured the same day, and the headline is that the path already
      works.** With the plugin loaded, a real `claude` (Claude Code v2.1.251)
      asked to `Write` a file, `warpctrl agent approve` reported
      `keystroke: "enter"`, the request left `agent approvals`, and the file
      appeared with the expected contents. That was `approvals.rs`'s own
      self-declared weakest claim — *"not a prompt this fork watched"* — and it
      is now watched. **The thesis path has working remote consent today**:
      Claude asks, the plugin reports over OSC 777, Warp surfaces it, and a
      paired device can answer, since `agent.approve` is already in the pairable
      set when `WARP_FORK_REMOTE_APPROVE` is on.

      **The prerequisite that will cost someone an afternoon.** The plugin must
      be loaded *when the agent starts*. Measured: a `claude` already running
      when Warp installed the plugin raised a prompt in its TUI while
      `agent approvals` stayed **empty** — no error, nothing logged, and it reads
      exactly like "the fork cannot see CLI agents". Reloading plugins and asking
      again surfaced it at once. This is now in `CLAUDE.md`.

      **Three gaps the run exposed, all plugin-side, which is this ticket's
      thesis holding up under test:**

      | gap | what it costs |
      |---|---|
      | `approval_id` is the **pane id** | the digest binds to whatever the pane is *currently* asking, not to one call — `TR-EVENTS-B` made concrete |
      | `acts on: not stated by the agent` | no `cwd` in the payload, though `build_payload` extracts one — so the card cannot say where a call acts, unlike ACP's |
      | no way to answer through the hook | the Return press works, but it takes the *highlighted* option; a typed answer still needs increment (2) |

      **`tool_use_id` checked on both ends and absent from both**: it appears
      nowhere in the plugin, and `event/v1.rs` has no call-id slot. So increment
      (1) needs the plugin to send one *and* a protocol v2 to carry it — the
      negotiation already supports that, but it is two changes, not one.

      **Named, not assumed:** whether Claude Code's `PermissionRequest` hook
      input actually contains a `tool_use_id` is **still not verified**. The plugin's
      own test uses a synthetic payload (`{"session_id":"s1","cwd":"/tmp/proj"}`),
      which proves nothing about real input — and note that real input evidently
      lacks `cwd`, since the card could not state it. The cheap check is a
      temporary hook that dumps stdin, which is a write under `~/.claude` and so
      belongs to the maintainer.

      **As built — the payload was captured, and "still not verified" above is
      now false.** Measured 2026-08-30 with `.fork/tools/dump-hook-stdin.sh`,
      registered *beside* the vendored plugin's own `PermissionRequest` hook. No
      edit to the plugin was needed: a hook event runs **every** command
      registered for it, which is the cheap trick worth remembering the next
      time a vendored hook has to be observed rather than changed.

      Ten keys arrive:

      ```
      cwd  effort  hook_event_name  permission_mode  permission_suggestions
      prompt_id  session_id  tool_input  tool_name  transcript_path
      ```

      **`tool_use_id` is absent.** `TR-EVENTS-B`, recorded in three files, is
      correct. The question this ticket left open is answered and the answer is
      the pessimistic one.

      **But the id is recoverable beside it, and that changes increment (1).**
      `transcript_path` points at Claude Code's own session JSONL, whose
      assistant messages carry `tool_use` blocks with `toolu_…` ids — and the
      entry is written **before** the hook fires (measured: transcript entries at
      `22:33:13.86–13.99Z` against a dump at `22:33:15Z`). So the id exists at
      *decision* time, not merely afterwards, which is the only version of that
      fact worth anything.

      **The obvious join does not work, and this is the part to not rediscover.**
      Matching on `tool_input` alone is ambiguous: in one session the same
      command resolved to **two** ids, because agents re-run commands. The usable
      key is the *most recent* `tool_use`, disambiguated by `tool_name` plus
      input among calls not yet resolved. **Not verified:** parallel tool calls,
      where one assistant message carries several `tool_use` blocks and "most
      recent" stops being a single answer. That is the case to test before
      trusting the join.

      **So increment (1) is cheaper and dearer than this ticket assumed, in
      different places.** Cheaper, because the plugin can recover an id today
      without anything upstream changing. Dearer, because the recovered id is
      **heuristic rather than exact** — and a call id that is usually right is
      worse than none for an audit trail, which is the one use it was wanted
      for. Decide that before bumping the protocol: a v2 field carrying a
      best-guess join would put a guess where readers will read a fact.

      **Two fields nobody knew were there.**

      - **`permission_mode`** — so Warp can *know* the mode a CLI agent is in
        rather than infer it. That is T14.18's territory arriving free on a
        different path.
      - **`permission_suggestions`** — Claude Code's own proposed rule
        additions, shaped `{type: addRules, rules: [...], behavior: allow,
        destination: session}`. A first-class, **session-scoped** persistent
        grant, with the scope on the wire. This is direct evidence for I18's
        central claim: the allow-all affordance is not something the fork would
        be inventing, it is something already offered and currently dropped.
      - `effort: {level: …}` is the third and is unexamined.

      **And the finding that makes this ticket's thesis stronger than it
      claimed.** The permission hook is **observational only** — it reports and
      exits, so Warp is told and cannot answer. Combined with the missing call
      id, both gaps are **plugin-side**. So *"remote consent only works on ACP"*
      is true today and is **not an architectural fact**: it is what you get when
      the only channel is one-directional. Before concluding that a CLI-agent
      limitation is structural, check whether it is simply a hook that does not
      answer yet.

- [x] **T14.19** **Hand the agent back the transcript Warp already owns.**
      Proposed by the maintainer 2026-08-30 on reading T14.13, and it is the
      right shape because it **routes around the detection problem instead of
      solving it**.

      T14.13 concluded that Warp cannot know when an agent compacts, because ACP
      defines no update kind to carry the news. That framing quietly assumed the
      fix must be triggered *by* a signal. It need not be. **Warp holds the
      complete transcript either way** — measured, 21 exchanges and 159,747
      characters still intact at the moment the agent could no longer recall its
      first eleven turns. The agent has lost it; Warp has not. So Warp can simply
      make its copy reachable and say so once, at session start, and the recovery
      works whether or not anyone ever detects a compaction.

      **Grep, not read** — the maintainer's word, and it is the load-bearing
      detail. Handing back a 160,000-character transcript would consume the very
      budget that ran out; a *searchable* transcript costs only the hits. The
      agent that said "point me back at the record and I'll re-read it and report
      exactly what was found" was describing this feature.

      **What has to be checked first, in order:**

      1. **Does Warp already persist the transcript in greppable form?** The
         event log does **not** — measured, it carries `tool_start`,
         `tool_complete`, `stop`, `prompt_submit`, `session_start` and no
         assistant text. `warpctrl agent read` reconstructs from Warp's own store,
         so something has it; whether a file exists to point at, or one must be
         written, is unverified and decides the whole size of this ticket.
      2. **Where does the pointer go?** The prompt is the only channel the fork
         controls, so it is a preamble on the first prompt of a session. That is a
         real cost — it is words in someone else's context window — and it should
         be one line naming a path, not an explanation.
      3. **Whether it is wanted per-session.** Some agents manage their own
         history well; this should be a disclosure the user can decline, in the
         same spirit as `WARP_FORK_ACP_MODE` having no default.

      **The trap to avoid, named in advance.** This must not become Warp
      *managing* the agent's context. It writes a file and names it once; the
      agent decides whether to look. Anything more — injecting recovered context,
      re-prompting after a detected compaction — puts Warp in the business of
      editing someone else's conversation, which is the same overreach
      `acp_permission` refuses on `switch_mode`.

      **Related, unbuilt:** a compaction *signal* would still be worth having, to
      say in the panel that the agent's view and Warp's have diverged. That is
      T14.13's disclosure half and it is independent of this. Carrying
      *"ACP has no way for an agent to report that it compacted"* upstream remains
      the honest fix.

      **As built 2026-08-30, and running it changed the design twice.**

      `app/src/ai/transcript.rs` writes one markdown file per conversation, keyed
      by Warp's conversation id (matching the event log, so T14.15's split is not
      reintroduced), rewritten whole on each terminal status and renamed into
      place so a mid-write grep sees the previous complete version. Tool results
      are excluded — a transcript carrying every file read would be larger than
      the context that ran out. The pointer rides **every** prompt as its own
      `ContentBlock`, so the user's text is never edited; the panel is told
      **once**, because that block is invisible there and the agent would
      otherwise be acting on an instruction the person never saw.

      **Verified end to end**: a passphrase planted in one turn, grepped back out
      of the file by the agent's own search tool in the next, reported correctly,
      **zero permission requests**.

      **Finding 1, and it decides where this feature is usable.** With the
      transcript outside the pane's directory — which the `on` default always is
      — `opencode` asking to read it arrives as **`tool: other`, the one kind
      `acp_permission` cannot say yes to.** Not "asks and waits": Warp offers no
      yes, so a person at the panel could not approve it either, and the recovery
      is unreachable by construction. Inside the pane's directory the same read
      is ordinary and needs nothing. **This is T14.8's `external_directory`
      finding arriving through a new door**, and it means the tidy default is the
      broken one. Left as the caller's choice rather than guessed at — writing
      into someone's repository unasked is worse than a path they had to name —
      with the sentence documented instead.

      **Finding 2, a real defect the compiler could not see.** The first cut
      wrote on any terminal status, and relaunching Warp rewrote a transcript for
      a conversation from the *previous run*: a restore re-announces a status
      reached before the process started, so on a history of any size that is a
      write storm at startup for turns that ended days ago. Fixed by matching
      `event_log::warp_agent`'s existing guard, which was put there for the same
      reason and which the first draft simply had not read.

      **Measured 2026-08-30, and it survives.** The gap above is closed: a run
      driven until `opencode` compacted **twice** (confirmed from its own log and
      from two `type: "compaction"` parts in its database, tails starting at
      messages 20 and 37 of 49), then calibrated in both directions.

      | probe | result |
      |---|---|
      | recall an incidental detail from memory, no tools | **"I DO NOT HAVE IT"** — genuinely gone |
      | grep the transcript for the same detail | **"84 … corrected to 88"**, quoted with its line number, correct |
      | permission requests | **0** |

      Twelve exchanges, an 88 KB transcript. The detail — T1.5a's action count
      from the first fill turn — was chosen because it is bulky and incidental,
      which matters for the reason below.

      **The first probe was badly designed and failed to fail.** A codeword
      planted in turn 1 as *"note this for later"* was still recalled perfectly
      after both compactions. Checking the database rather than assuming showed
      why: **both compaction summaries contained it verbatim**. The summariser
      had done exactly its job on a fact explicitly flagged as worth keeping, and
      the probe had been built to be memorable and then tested for being
      forgotten.

      **That correction is the more useful finding.** Compaction is not
      indiscriminate loss. What survives is what the summariser judges salient —
      so the transcript's value is precisely for the **incidental and bulky**,
      which is also exactly what T14.13's original loss was (three audited claims
      with their file:line citations, not a flagged secret). A feature justified
      by "the agent forgets things" would have been justified too broadly; it
      forgets a *particular class* of thing, and that class is the one a working
      session is made of.

      **Both fixed 2026-08-30, on the maintainer's reading.**

      **The default now lands in `.warp/transcripts/` under the pane's own
      directory**, and the reasoning is reachability rather than taste. `~/.warp/`
      is home-level, which puts the file back outside the session and into the
      unanswerable `other` request. `.opencode` was never a candidate: measured,
      `opencode` keeps *nothing* in-project — its whole store is one global
      SQLite database — so there is no file to collide with and writing there
      would be inventing a directory inside another tool's namespace. `.warp/`
      is upstream's own project directory, already holding tracked workflows and
      skills, so it is convention rather than invention; `/.warp/transcripts/` is
      gitignored because that directory is *meant* to be committed.

      **And Warp's own words are no longer attributed to the agent.** The
      announcement is emitted as agent output, so it landed under `### Agent` and
      an agent grepping its history read Warp's sentence as its own. Warp's
      editorial asides now carry a `[Warp]` marker and are stripped from the
      file. **Permission prose deliberately stays** — see the comparison below.

      **What Warp's transcript holds that the agent's own store does not,
      measured on one run with both captured.** The question was whether pointing
      an agent at its own history would do just as well.

      | | Warp's | `opencode`'s |
      |---|---|---|
      | prompt and final answer | yes | yes |
      | model reasoning | **no** | yes |
      | tool call structure | no | `tool=bash status=error input={…}` |
      | **why a call failed** | **`Answered: no`**, with what was offered | **`status=error` and nothing else** |

      **The fourth row is the answer.** On a denied `wc -l`, `opencode` recorded
      only that the tool errored. It has no notion that anything *refused* it. So
      an agent re-reading its own store sees a failure where there was a
      decision, and would reasonably retry. Warp's layer is the only one holding
      the consent record, which is why the permission prose is kept and why
      pointing at the agent's own store would actively misinform.

      Neither is a superset: `opencode` retains reasoning that Warp never keeps.

      **Tool records added 2026-08-30, and they are inert on the path that needs
      them.** `Exchange` now carries `name -> outcome` per call, using the same
      `AIAgentActionTypeDiscriminants` / `AIAgentActionResultTypeDiscriminants`
      vocabulary the event log uses, appended per exchange as `### Tools used`.
      Name and outcome only, never payloads: the payloads are the largest thing
      available and the most cheaply recovered, since the agent can read the file
      again.

      **Then verified by running, which found it empty.** A turn that plainly
      read a file produced no tool section at all. The reason is structural:
      **the ACP translator emits tool calls as `AgentOutput` text, never as
      `AIAgentAction`** (`translate.rs:377`) — there is no action in that path for
      an action model to have a result for. So the collection is correct, matches
      precedent, and will populate for an agent that produces actions, while
      contributing nothing to an `acp_agent` session.

      **What that means for the original question.** On the ACP path the tool
      *names* are already in the transcript, because the translator renders them
      as prose (`` `read` ``, then the path). What is missing is the **outcome** —
      whether the call succeeded, errored, or was refused. So "add tool data" is
      not a formatter change here; it is a translator change, and the data exists
      at that layer already since `translate.rs` writes `tool_start`/
      `tool_complete` to the event log. The options are to have the translator
      record outcomes into a side table the transcript reads, or to emit real
      actions — the second changes panel rendering and is much larger. Neither is
      built. Left explicit rather than shipped as though it worked.

            **Reasoning is absent by inheritance, not by decision — worth recording
      because it looked deliberate.** Warp *does* receive it
      (`AgentThoughtChunk` → `AgentReasoning`, `translate.rs:329`). It dies in
      upstream's `format_for_copy`, which carries a bare
      `AIAgentOutputMessageType::Reasoning { .. } => continue`. That is upstream's
      judgment about what belongs on a **clipboard**, adopted here silently by
      reusing the function `agent_read` uses. The tool-result exclusion, by
      contrast, *was* a fork decision — the `None` action model — argued from size
      and not measured. Note the API offers no middle: `Some` emits full result
      payloads and the `Action` arm never emits the tool's *name*, so
      "tool name and outcome" does not exist to be chosen and would have to be
      built. Open question, with the argument in the commit body.

      **Two harness failures worth more than the fixes**, both self-inflicted and
      both the read-back rule again. Three Warp instances accumulated because
      `window close` was answering `ambiguous_instance` while the check grepped
      only for `"ok"` — so several "the transcript was not written" results were
      measuring whichever instance happened to answer. And `agent list` reporting
      `success` fires *before* the history event that triggers the write, so a
      check immediately after it reads an empty directory and looks exactly like
      a broken feature. Neither was a defect in the code under test.

      **Named, not assumed.** Whether `claude-agent-acp` compacts on the same
      cadence is **unmeasured** — it has its own policy, and this run deliberately
      did not use it (see T14.11). The detector above is **unbuilt and
      unverified**. `providerID=opencode modelID=big-pickle` in the same log
      settles a side question: this run billed opencode's hosted model, not the
      user's Claude subscription.

- [x] **T14.14** **Let a person choose the model the agent uses.** Built 2026-09-07, both halves; `.fork/docs/composer.md`, "The model picker, built and measured".
      `session/load`'s reply carries `configOptions`, which measured against a
      live agent includes a model select — so the protocol already offers this and
      the fork simply does not read it. Small, self-contained, and the first
      thing on this board that makes the ACP path *better* than the panel's own
      controls rather than merely equal to them.

      The constraint to carry: a model list is the agent's claim about itself, so
      it is rendered, never validated, and a selection is passed back verbatim.

      **Re-measured 2026-08-29 before starting, per the rule that a measured
      claim is measured as of its date — and the premise holds but is more
      dangerous than the ticket said.** Two corrections:

      **`configOptions` is on `session/new`, not only `session/load`.** So a new
      conversation can offer this immediately; the ticket implied resume-only.

      **And it is not a model picker. It is a generic config channel, and one
      agent puts the session's permission mode in it.** Measured, the first entry
      of `claude-agent-acp`'s list is `{"id": "mode", "category": "mode",
      "type": "select"}` whose options include `dontAsk` and
      `bypassPermissions`; `opencode`'s is `{"id": "model", "category":
      "model"}` with a model list. **So "render `configOptions` and pass the
      selection back" would ship a control that can set `bypassPermissions`** —
      the exact escalation `acp_permission` refuses as `switch_mode`, arriving
      through a different door, and with no permission request involved at all
      because a config write is not a `session/request_permission`.

      That makes the scope explicit rather than incidental: **render
      `category: "model"` and nothing else** for this ticket. The mode select is
      the separate control T14.4 already designated — *"an in-app mode picker
      rendering the agent's own mode descriptions is the first legitimate
      instance of a surface capable of showing what the option declares"* — and
      it is legitimate precisely because it is deliberate, labelled, and shows
      the agent's own descriptions. Reaching it by accident through a generic
      picker is the opposite of that, and a filter on `category` is the whole
      difference. **A category this build does not recognise is not rendered**,
      by the same allowlist logic as the tool kinds: unknown must not qualify.

      **Surveyed 2026-08-30, and the category allowlist is now measured rather
      than argued.** Asked *"is our permission model too tight, and how long a
      pole is auto-mode for agents other than Claude"*, the honest first step was
      to ask the agents. `session/new` against both, same repo, same day:

      | | `modes` | `configOptions` categories |
      |---|---|---|
      | `claude-agent-acp` 0.70.0 | `currentModeId: "auto"`, 6 available | `mode`, `model`, `thought_level`, and one with **no category at all** (`id: "agent"`) |
      | `opencode` 1.18.25 | **`null`** | `model`, **`mode`** |

      Three things fall out, and the second is the one that matters.

      **`SessionModeState` is protocol-level; `auto` is not.** The schema carries
      `modes` on `session/new`, `session/load` and `session/set_mode`, so every
      agent is asked. But `SessionModeId` is `Arc<str>` — an opaque string — and
      nothing standardises a single id. `auto` is one agent's word, and its
      meaning lives in its own English `description`.

      **Both agents ship `category: "mode"`, and it means opposite things.** On
      `claude-agent-acp` its six values include `bypassPermissions` and
      `dontAsk` — permission policy. On `opencode` its two values are `build`
      (*"Executes tools based on configured permissions"*) and `plan`
      (*"Disallows all edit tools"*) — an agent persona, with the permission
      policy explicitly delegated **elsewhere**, to the config file T14.8 found.
      So `mode` is not a category whose meaning can be read off its name. This
      corrects the row above, which recorded opencode as offering `model` alone;
      it offers both, and a filter that admitted `category: "mode"` would render
      one agent's escalation menu and another's persona switch through the same
      control. **The `category: "model"`-only scope stands, and now on evidence.**

      **An option with no category at all exists** (`id: "agent"`). The
      unknown-must-not-qualify rule was written for a category this build has not
      read; the measured case is a category that is *absent*. Same answer, and
      worth pinning with a test rather than inferring.

      One agent did not answer: `@zed-industries/claude-code-acp` 0.16.2, also
      cached here, completes `initialize` and then fails `session/new` with
      `Query closed before response received`. Recorded as *did not start*, not
      as a measurement of its modes — it is a stale build, and reading a crash as
      a finding is the error this file keeps naming.

- [x] **T14.15** **One ACP turn writes two event-log files that never name each
      other.** Found 2026-08-29 while checking whether an advisor's queue
      recommendation rested on a stale doc. It did, and underneath it was a real
      defect nobody had seen.

      Measured, one turn with `WARP_FORK_EVENT_LOG` set:

      ```text
      <conversation-uuid>.jsonl   session_start, stop        agent: warp
      <acp-session-id>.jsonl      tool_start, tool_complete  agent: opencode
      ```

      Neither file references the other. So opening the conversation file alone
      shows **a session with nothing between its ends** — which is word for word
      the claim `CLAUDE.md` carried from T14.7 and which T14.9 was thought to have
      fixed. T14.9 *did* fix it; the events simply landed in the other file, and
      the wrong sentence stayed believable for a day because the file a person
      naturally opens is the one keyed by the id they already had.

      **Why this outranks the rest of the queue right now.** An unattended run has
      an orchestrator answering the in-panel agent's permission prompts, and that
      is only defensible if each approval can be traced afterwards to the edit it
      produced. That is an audit trail, it is the thing this log is *for*, and
      today it is split.

      The fix is probably small — the writer already knows both ids, since the
      parked-approval path carries `session_id` precisely so a request can be
      lined up with these lines. Candidates: one file per conversation with the
      ACP session recorded inside it, or a `session_start` in each naming the
      other. **Check first whether the CLI-agent path has the same split**; it
      keys on its own session id and may already be coherent, in which case match
      it rather than invent.

      **As built — and checking the other path first was the whole of the
      design.** `local_agent`'s `TurnContext::session_id` carries Warp's
      conversation id under a comment that reads, in full: *"**Not** Claude's
      session id: that is what `conversation_token` carries here, and filing
      under it would put a turn's tools in a different file from its frame."*
      The rule was written down, with the exact failure named, and the ACP path
      did the thing it warns against. So there was nothing to invent — only a
      precedent to match, and the first draft of this fix (a new cross-reference
      event, keeping two files) was abandoned on reading that sentence.

      ACP tool events now file under the conversation id like every other source.
      The agent's own session id is kept on each line as `linked_session_id`,
      which is the join back to a parked approval's `session_id` and is the only
      thing the new field is for. Verified live — one turn, one file, in order:

      ```text
      session_start   agent=warp                       linked=None
      tool_start      agent=opencode  tool=execute     linked=ses_faf811…
      tool_complete   agent=opencode  tool=execute     linked=ses_faf811…
      stop            agent=warp                       linked=None
      ```

      **A second thing falls out, and it is the better argument.** The agent's
      session id no longer names a file. `event_log::session_key` sanitises that
      string precisely because a session id of `../../.bashrc` would be a plugin
      choosing where Warp writes; that defence is still there and now has less to
      defend, because the only path that fed it an agent-controlled string has
      stopped. Filing under Warp's own id is the fix for the split *and* removes
      the reason the sanitiser was load-bearing.

      Pinned by a test that reads the live broadcast rather than the file, since
      the file key is derived inside `record` from the `session_id` field, and
      that field is the thing a future edit would get wrong.

- [x] **T14.16** **The panel can answer.** Built 2026-08-30, at the maintainer's
      request, and verified by clicking it.

      **Why it stopped being second.** T14.8 shelved the button on a good
      argument — a button over a request with no yes is a greyed-out button —
      and two things then argued it back. The `other`-kind population turned out
      to be one agent's convention rather than a protocol fact, so the greyed-out
      case is narrower than it looked. And T14.11 found that an *unattended*
      answerer is not available at all: the orchestrating session's own
      permission layer refuses to run one, correctly. So a present person
      answering cheaply is not a convenience. It is the only mechanism there is.

      **What it shows, and the rule that decides that.** `acp_permission`'s
      sentence governs the panel exactly as it governs the flag: *an option may
      only be selected by a surface capable of showing what that option
      declares.* The control shows the agent, the title, the verbatim
      `tool_input`, where the call said it would act, and every option offered.
      So it may offer the single-shot yes and nothing else — the always-variants
      are not rendered at all, because a button that sets a session policy would
      authorise something never shown.

      **Gated per entry on `approve_selects`**, which is frozen when the request
      parks. That is T14.6's bug and this is the third surface with a chance to
      make it; an entry Warp will not approve shows the reason instead of a
      button, because a greyed control says *not now* and the honest message is
      *not by Warp, and here is why*.

      **The click carries the id it was drawn with.** Three surfaces can answer
      one question and a turn can end under any of them, so between reading a
      request and clicking it the request can be gone and another parked in its
      place. Looking it up again at click time would answer the newcomer with a
      decision made about its predecessor — the stale-answer hazard the control
      plane's digest exists to prevent. Capturing the id at render time gets the
      same property structurally, since here the surface that displayed it is the
      surface that answers it. Arming holds an *id*, not a bool, for the same
      reason, and that rule has the one unit test that does not need a view.

      **Two taps for yes, one for no** — the console's asymmetry for the console's
      reason. A misclick on *No* costs the agent a retry; a misclick on *Yes*
      costs whatever it asked for.

      **Synced from the registry, not from a stream update**, which is the one
      structurally awkward thing here and is worth naming. Every neighbouring
      inline view is built from an `AIAgentAction` the agent sent. A permission
      request is not one: it is a JSON-RPC call the agent is blocked on, and the
      only thing reaching the renderer is the prose note the fork writes about
      it. Keying on the *text* of that note would be a renderer inferring meaning
      from wording. So `sync_acp_approval_view` reconciles against
      `registry::waiting_for` on the output-update edge — which is exactly when
      the asking note arrives, and again when the answered note does. Only the
      latest visible exchange draws it, or every block in the conversation would
      draw its own pair of buttons for the same question.

      **Verified by running it, twice, and the first run found a bug.** Yes and
      No both rendered; the unapprovable case correctly showed *No* alone with
      the reason. But the reason ran off the right edge of the panel — the one
      field whose whole purpose is explaining why there is no yes was the field a
      person could not finish reading. Cause: `Text::new_inline`, copied from a
      neighbouring inline view, whose own doc comment says it is deprecated and
      that "all usages have not been audited". `Text::new` soft-wraps. Fixed and
      re-run; the sentence now wraps to four readable lines.

      Then driven with `use_computer click` against the X11 window: one tap armed
      *Yes* (*"tap again to allow"*), the second answered it, the panel printed
      *"Answered: yes, for this one call"*, the buttons vanished, and the agent
      ran `git status --short` — whose output listed the files this feature is
      written in. *No* answered in a single tap. **This is the first fork feature
      whose UI was verified by a synthetic click rather than by reading a
      screenshot.**

      **Not built, deliberately:** keyboard bindings. Every neighbouring view
      registers them in an `init`, and binding *Enter* to a permission grant is a
      decision with its own argument — the two-tap arming exists precisely
      because a single cheap gesture should not be able to say yes. Named here
      rather than added quietly.

- [x] **T14.17** **The ACP path logs what the agent did and never what it
      asked.** Found 2026-08-30 while trying to answer *"is our permission model
      too tight?"* from data, and failing — which is the finding.

      **Measured.** `translate.rs` emits exactly two event names: `tool_start`
      and `tool_complete`. Counted across every ACP session recorded in this
      repo — 11 `session_start`, 35 `tool_start` — the event kinds present are
      `session_start`, `prompt_submit`, `tool_start`, `tool_complete`, `stop`,
      `stop_failure`. **Zero permission events, because none is ever written.**
      The ask and the answer become *conversation notes* — transcript prose a
      person reads live and nothing durable — while `outcome_for` sends the
      decision to the agent and records nothing.

      **Warp's own agent path already does this.** `event_log/warp_agent.rs`
      emits `permission_request` and `permission_replied`, joined to the call by
      a stable id, with a test pinning that a question to the user is not a
      permission request. So the ACP path — the path this fork exists for, the
      one with the consent architecture built on it — is behind the path it was
      meant to surpass, on exactly the axis that matters.

      **It also quietly breaks the charter.** `.fork/GOAL.md`'s second unattended
      rule is *"Log every decision with the `tool_input` that was shown, so each
      landed edit can be traced tomorrow to the answer that let it happen."* On
      the ACP path that log does not exist. T14.15 unified the two event files an
      ACP turn used to write — into a file that never contained a permission
      event to begin with, which is why unifying them did not surface this.

      **What it costs today, concretely.** The tool kinds that appeared in real
      recorded work are `edit` (12), `read` (11), `execute` (9) and `search` (1)
      — every one on `effect_is_confined_to_this_call`'s allowlist. That looks
      like evidence the allowlist is not too tight, and it is not, because the
      log records tool *calls* and a refused request never becomes one. T14.8
      measured exactly that shape: refusing opencode's `other` precondition means
      the `execute` behind it is never sent. **So the log undercounts in exactly
      the region under dispute**, which is what makes it useless for this
      question and is the sharpest argument for building the counters.

      **This ticket first said the tightness question was therefore
      unanswerable, and that was too strong — corrected on the advisor's
      push-back, same day.** The event log was never the instrument for it. The
      *benefit* side is already measured elsewhere and on the wire: the
      `switch_mode` plan-exit escalation, which the spec says every plan-mode
      agent will present. The *cost* side has T14.8's live run: one agent, one
      convention, remedied in that agent's own config, residual one pasted
      `agent deny`. An unmeasured cost **rate** argues for building the
      instrument, not for withholding the verdict — and under this fork's own
      asymmetry a fail-closed status quo is the default that stands until a
      measurement argues against it. The verdict is **not too tight, on today's
      evidence**; what T14.17 changes is the ordering, which is that it lands
      before any widening.

      **And T14.18, found hours later, makes that correction sharper than the
      advisor could have known.** On the panel path with `claude-agent-acp` the
      measured request rate is **zero** — not because the allowlist refuses, but
      because no `session/set_mode` is ever sent and the agent's own classifier
      answers first. So the missing count was never the thing standing between
      this fork and a verdict. It was hiding something upstream of the allowlist
      entirely.

      **The better justification, which is the one to keep.** `acp_permission`'s
      own doc establishes that *"a hazard in a doc comment with no test under it
      is a hazard that is not defended"*. The claim *"this model's refusals cost
      almost nothing"* is the same shape pointed the other way: a **cost** claim
      living only in prose and in one session's memory. Permission events are
      the test under it, and that is why they are worth building whatever the
      verdict is.

      **Scope.** Two event names on the ACP translator, mirroring
      `warp_agent`'s vocabulary rather than inventing one: `permission_request`
      carrying the kind, the title and the `tool_input` excerpt that was shown,
      and `permission_replied` carrying what was answered and by which surface
      — CLI, console, panel button, or `--approve`. Joined to the call by
      `tool_call_id`, which `translate.rs` already tracks for the locations
      remembering in T14.6.

      **The constraint, and it is the whole design.** This log must record what
      *Warp* did — the request it received, the answer it relayed, the surface
      the answer came from. It must not claim anything about *why* a person
      answered, and it must not present Warp's refusal as a judgement about the
      call: `unconfined_reason`'s wording was corrected in T14.8 for exactly that
      overreach, and a log line is read later with less context than a panel
      message, not more.

      **The falsifier, recorded here so T14.11 knows what to watch for.**
      Refusals of *confined-kind* requests appearing at all, or approval volume
      a person abandons the panel over — turn-stopping, not merely annoying,
      which `GOAL.md` defines as the success condition. Either flips the
      verdict, and the responses are in order: the mode surface (T14.18), then
      the shelved digest-bound person-yes on a shown `raw_input`. A second agent
      whose *ordinary* calls arrive as non-allowlisted kinds with no config
      remedy flips it too, by breaking T14.8's one-agent-once arithmetic.

      **The surface question, resolved before it could become an unknown.**
      `registry::answer` has exactly two callers:
      `local_control/handlers/approvals.rs` (the control plane, so CLI, console
      and a paired phone) and `inline_action/acp_approval.rs` (the panel
      button). Both are in-process and both know which they are, so the surface
      is a third parameter threaded from two call sites rather than anything
      inferred. **`--approve` is not a third caller and never reaches here** —
      `crates/warp_cli/src/local_control/acp.rs` is a standalone ACP client in
      its own process, with no Warp and no event log, which is also why the
      T14.1/T14.2 findings could be made without one running.

      **As built, 2026-08-31. Two events, three fields, and one claim in this
      ticket corrected on the way.** Reviewed adversarially before a line was
      written; the review changed the design twice and caught one factual error
      that had already been reported as a finding.

      `permission_request` and `permission_replied` on the ACP translator,
      reusing `warp_agent`'s names rather than inventing any. The request line
      carries the tool kind, the `tool_input` excerpt and whether Warp had a
      *yes* to offer; the reply line carries what was answered and by which
      surface. Both join to the call by `tool_call_id` and to each other by the
      `approval_id`, which rides both summaries — `call_id` alone cannot pair a
      re-asked call, which is the stale-answer hazard `ParkedRequest` is already
      keyed against.

      **The three new `Entry` fields, and why each is a field rather than
      prose.**

      - `decision` — `allowed` / `denied` / `unanswered`, and **always present**.
        The absence-grammar this fork uses elsewhere (`acts_on`'s "empty means
        the agent never said") is unavailable here, because `Entry` is
        `#[skip_serializing_none]`: a `None` does not serialise as `null`, it
        *vanishes from the line*. A `permission_replied` with no `decision` key
        would be indistinguishable from a line written by a build from before
        the field existed — a versioning ambiguity in the one file that exists
        to be believed later. So `unanswered` is stated, which is also what
        `answered_note` already does in prose.
      - `answered_by` — `control_plane` / `panel`, absent when nobody answered,
        and that absence is readable because the `decision` on the same line
        explains it.
      - `can_approve` on the **request** line. **This is the field that lets the
        log detect this ticket's own falsifier.** Without it a `denied` line is
        ambiguous between a person's no and Warp never having offered a yes to
        say — and telling those apart is the entire question the ticket was
        built to make answerable. The reason, when it is `false`, is prose and
        rides `summary`, because `acp_permission` wrote it for a person.

      **Derived from `ParkedRequest`, not from the wire, and the coupling is the
      point.** The charter asks for *"the `tool_input` that was shown"*, and the
      parked request is verifiably the shown thing — it is what every surface
      renders and what `agent.approvals` reports. Re-reading the raw
      `RequestPermissionRequest` would log what *arrived*, which is the same
      string today and one refactor away from silently not being. A test pins
      the logged preview against `excerpt(parked.tool_input)` for exactly that.
      `tool_call_id` is threaded separately rather than added to
      `ParkedRequest`, because it is a fact only this log consumes.

      **Observation only, by construction.** Both events are appends to a file
      and a broadcast channel; neither can reach an approval outcome. A test
      pins that logging emits no client action at all, because
      `translate.rs`'s standing hazard is that a tool call emitted as an
      `Action` message is an *instruction* Warp executes — the double-execution
      trap this phase has now been talked out of four times.

      **The correction, and it was a finding this ticket had already published.**
      A claim went into a status report that a *denial* on Warp's own path never
      produces a `permission_replied` — that it goes to `Cancelled` →
      `stop_failure` — and therefore that the mirror was worthless. That is
      wrong, and the error was conflating cancelling an **action** with
      cancelling the **conversation**. Traced: `RequestedCommandViewEvent::
      Rejected` → `cancel_action` (`block.rs:3653`) →
      `cancel_action_with_id(…, ManuallyCancelled)` (`block.rs:5045`), then
      `cancel_pending_action` (`action_model.rs:1276`) builds
      `pending_action.action.cancelled_result()` into an `AIAgentActionResult`
      and hands it to `handle_action_result`, after which the turn **resumes**.
      So `Blocked → InProgress` fires and a denial does emit
      `permission_replied`; `ConversationStatus::Cancelled` is the separate
      whole-turn cancel. The mirror is sound and the shared name is right.

      **A follow-on claim was then filed about that correction, and it was also
      wrong — in this fork's most familiar way.** The claim was that the
      correction *can never be run here*, because `warp_agent.rs:326` records
      that its whole table is unreachable at runtime on this fork's primary
      path. Put to the advisor, it did not survive: **that is the T12 shape**,
      where "no browser on this machine" was filed three times meaning the WSL
      userland and written as though it meant the hardware.

      What is unavailable is the **live server path** — reaching Warp's own
      agent needs the account the fork exists without. That is not the only
      instrument, and the claim bundled two halves with different evidence:

      - **The mapping is already test-held.**
        `leaving_blocked_is_an_answer_and_not_a_new_prompt`
        (`warp_agent_tests.rs:154`) pins `Blocked → InProgress` →
        `permission_replied`. Labelling that "read only" understated it.
      - **The flow half was then settled by reading it properly in a panel
        session, and both earlier readings were half-answers.**
        `action_model.rs:1451-1480`: once no pending actions remain and the
        reason maps to `CancellationOutcome::Cancelled` — which
        `ManuallyCancelled` does (`ai/agent/mod.rs:208`) — the status is
        `Cancelled` **if every finished result is cancelled**, and `InProgress`
        otherwise.

        So rejecting the **only** pending action ends the conversation as
        `Cancelled` → `stop_failure`, with **no** `permission_replied`; and
        rejecting one call among others that succeeded resumes the turn and
        does emit one. The first claim ("a denial never emits it") was right
        for the common single-action case. The advisor's correction ("it always
        does") was right for the mixed case. Neither was right as stated, and
        the flat version was committed into `Entry::decision`'s doc for several
        hours before this run corrected it.

        **Found by the agent, in the panel, on the first turn of a dogfood
        session** — which is the argument for the dogfood better than any
        friction count. It read `handle_action_result` because the question
        asked it to walk the path rather than confirm a conclusion, and neither
        the author nor the advisor had followed it that far.

      Still left unwritten as a *test*, and now for a better reason than
      "unreachable": what it would pin is upstream's own model layer, which this
      fork does not run and does not change. What the reading does settle is the
      thing the open thread was actually about — **the shared name is right, and
      it is right for a narrower reason than was claimed.**
      `permission_replied` asserts that an answer resolved the ask; on the
      `warp_agent` path it is emitted only when the turn resumes, so a lone
      rejected call leaves no such line and the answer survives only as the
      turn's `stop_failure`. The ACP path has no equivalent hole. That is a
      fidelity difference between two sources sharing one vocabulary, and it is
      recorded on `Entry::decision` rather than papered over here. **"Verify by
      running" had an edge and this was not it — but reading it *properly* did
      the job, and neither earlier read had gone one call deep enough.**

      **The morning after it landed, an agent in Warp's own panel found a
      defect in it, and verifying that found a worse one.** Recorded here
      because the *prompt shape* is the reusable part: it asked what could make
      the trail **lie** rather than what the trail covered, and it asked for the
      deciding lines rather than for a judgement.

      - **A cancelled turn kept the question and lost its answer.**
        `permission_request` is written synchronously in the request handler
        (`mod.rs:512`); `permission_replied` is written from the task
        `wait_for_a_person` spawns onto the connection, *after* `answer.await`
        (`mod.rs:1117`). `generate` wraps the stream in `take_until`
        (`mod.rs:287`), so a cancelled turn drops the driver future and the
        waiting task with it — the await never resolves and the line after it
        never runs.
      - **And the `unanswered` value was covering a case that cannot happen.**
        `Err(Canceled)` needs the sender dropped while the receiver is still
        polled. The sender lives in the registry entry, and only
        `registry::answer` (which sends) and `Waiting::drop` (which runs inside
        that same dropped task) remove one. The remaining route is an
        approval-id collision, which `Waiting`'s own token check calls belt and
        braces. So the arm was exercised by a unit test and dead on the path,
        while the case it was named for wrote nothing at all — **coverage
        asserted in a doc comment and absent from the code**, which is the exact
        failure this fork's own method section exists to catch, in code written
        that morning.

      Fixed with `AsksNothingMore`, a drop guard armed across the wait
      (`202b20130`). It takes the lock without `expect`, because it can run
      during an unwind where a panic aborts: one missing log line beats taking
      Warp down over a poisoned mutex.

      **Measured, not read** — `.fork/tools/cancel-leaves-no-orphan-ask.sh`
      against a binary confirmed to postdate the fix:

      | phase | `permission_request` | `permission_replied` | `unanswered` |
      |---|---|---|---|
      | cancelled without answering | +1 | +1 | **+1** |
      | answered through the control plane | +1 | +1 | **0** |

      Both halves are load-bearing and the second is the one that cannot pass by
      accident: if the disarm ever stopped taking, every answered permission
      would be recorded twice and an instrument built to be counted would
      inflate. A test that only cancelled would never see it.

      **One instrument note from that run.** `cat "$EVENTS"/*.jsonl` returns
      **filename order, not time order** — the ACP path files per conversation,
      so a later phase's lines can print above an earlier phase's. Nothing in
      the log is wrong; a reader inferring causality from line order would be.
      Sort on the timestamp if order matters.

      **Two process findings, both the same shape as the merge-base lesson.**

      - The `Entry` widening was sized by grepping for `applied:` — **9 sites**.
        The compiler found **12**: two files not in the grep's list, and one
        using shorthand `applied,`. The command ran perfectly against an input
        assumed rather than computed. `cargo check --workspace --all-targets`
        is what made it a non-event, and the lib check would not have.
      - A new test failed on its first run, correctly. The event log's
        broadcast is **process-global** and tests run in parallel, so
        `find(|line| line["event"] == "permission_request")` picked up a
        *neighbouring test's* line and read its `can_approve`. The pre-existing
        tool-event test already filters on `call_id` for this reason. Filtering
        on an event name in a shared stream is a change detector for whatever
        else happens to be running.

      **The falsifier, restated now that the instrument exists.** N real working
      sessions with `WARP_FORK_EVENT_LOG=on`: if `permission_request` count is
      **zero while landed edits are nonzero**, the instrument recorded only
      Warp's non-involvement and the refusal-cost question stays unanswerable
      from it — which is exactly what T14.18 predicts for `claude-agent-acp` in
      `auto` on the panel path, since no `session/set_mode` is sent. In that
      case the mode surface should have led and this ticket measured nothing.
      That is checkable within one working day of the feature existing, and
      T14.11 is the run that checks it.

- [x] **T14.18** **The panel never sends `session/set_mode`, so with the
      flagship agent the fork's permission model is not reached at all.** Found
      2026-08-30, chasing *"is our permission model too tight?"*. It is not too
      tight. On the panel path with `claude-agent-acp` it does not run.

      **Read first.** `app/src/ai/acp_agent/mod.rs:513` sends
      `NewSessionRequest::new(cwd)` — cwd and nothing else — and `set_mode`
      appears **nowhere** in `app/src/ai/acp_agent/`. The only
      `SetSessionModeRequest` in the repo is
      `crates/warp_cli/src/local_control/acp.rs:204`, the standalone CLI probe,
      once, before the prompt. So a panel session takes whatever mode the agent
      defaults to and keeps it for the session's life, with no surface to change
      it.

      **Then measured, calibrated both ways in one sitting.** Same agent
      (`claude-agent-acp` 0.70.0), same scratch cwd, same prompt — *"create
      probe.txt containing hello"* — with the probe denying everything it is
      asked:

      | | `session/set_mode` | started in | permission requests | file written |
      |---|---|---|---|---|
      | run 1 — the panel's shape | none sent | `auto` | **0** | **yes** |
      | run 2 — calibration | `default` | `auto` | **2** | no |

      Run 1 is the panel path exactly. Zero requests, so Warp denied nothing,
      approved nothing, and was asked nothing; the file appeared. Run 2 fires on
      the known-present and shows the instrument is sound rather than merely
      quiet: `execute` (*"printf 'hello' > probe.txt"*) and `edit` (*"Write
      probe.txt"*), both cancelled, no file. This is the `wedged-agent.py`
      pattern and it is the reason the run-1 zero can be believed.

      **The two requests run 2 raised are both on the allowlist.** So the honest
      one-line summary of the tightness question is: *the model is not too
      tight, it is unreached* — and one `session/set_mode` turns both of these
      into requests the T14.16 button can answer today.

      **Why this hid.** Every panel session on this board has used `opencode`,
      which has no session modes at all (`modes: null`, T14.14's survey) and
      takes its permission policy from the committed config file. So the whole
      consent architecture was exercised against the one measured agent where
      this cannot bite. `CLAUDE.md` has said since T14.8 that *"`session/set_mode`
      to `default` is what makes it ask"* — true, and next to no code that sends
      it.

      **What to send — and the first plan for this was wrong, caught before a
      line was written.** The pitch was: request `default`, the mode where the
      agent asks, since moving `auto` → `default` **narrows** and a narrowing
      move needs no consent machinery, the same asymmetry that lets
      `agent.deny` work with no switch while `agent.approve` needs
      `WARP_FORK_REMOTE_APPROVE`.

      The asymmetry is real and the plan still fails, because picking `default`
      by id is Warp generalising one vendor's word — the error established
      hours earlier by this very phase's own mode survey. Checked rather than
      assumed: the protocol's doc comment for `session/set_mode` gives its
      example ids as **`"ask"`, `"architect"`, `"code"`**, not one of which is
      `default` or `auto`, and `SessionModeId` is an opaque `Arc<str>`. A safe
      direction is no help when you cannot tell which way you are facing.

      **So: disclose always, request only when told.** Built 2026-08-30 as
      `app/src/ai/acp_agent/mode.rs`:

      - **Disclosure.** The mode a session started in is reported into the
        conversation, quoting the agent's **own `description` verbatim** — the
        `Declaration::Changes` rule, for its reason: Warp cannot see an agent's
        permission policy, so the only honest thing it can say about one is what
        the agent put on the wire. The note says outright that Warp did not
        choose the mode and cannot tell what it permits. An agent advertising no
        modes gets no note at all, because a note per session saying an agent has
        no modes trains a person to skip the notes that matter.
      - **Request.** `WARP_FORK_ACP_MODE` names an id, with **no default value**,
        and Warp sends `session/set_mode` only for an id the agent actually
        advertised. An unadvertised id is *reported and not sent*: the spec
        requires the id be one of `availableModes`, so sending it buys a protocol
        error instead of a sentence, and failing silently would leave a person
        believing a mode was requested when it was not.
      - **Autonomous changes.** The spec permits an agent to change modes on its
        own and notify by `current_mode_update`. That is the same hazard through
        an unwatched door, so it is disclosed too — and since the notification
        carries an **id and nothing else**, the translator remembers what the
        agent advertised so the note can still use the agent's own words.

      **This overturns a decision recorded in T14.3/T14.4 and pinned by a test.**
      `translate_tests.rs` asserted `CurrentModeUpdate` renders nothing, on the
      argument that a mode is the agent's claim and does not predict per-call
      gating, so rendering it would be Warp restating a governance fact it
      cannot check. Right about the hazard, wrong about the remedy — and the
      wrong remedy is measured above at zero requests with nobody told. Silence
      is not neutrality when the thing unsaid is what decides whether anything
      gets asked. The hazard is now defended by wording under test rather than
      by saying nothing.

      **What this deliberately is not**: a claim that Warp is now in the loop. In
      a mode where the agent does not ask, Warp still sees nothing and decides
      nothing. The person is merely told so, which they previously were not.

      **Verified by running it, and the whole chain shows in one frame.** With
      `WARP_FORK_ACP_MODE=default` and `claude-agent-acp` in the panel, asked to
      write a file: the note says the session started in `auto` and that the
      variable asks for `default`, *"Standard behavior, prompts for dangerous
      operations"*, so Warp is requesting it; the agent then **asks**; the
      request parks with its `rawInput` verbatim
      (`{"file_path":".../probe.txt","content":"hello"}`); and T14.16's card
      renders **Yes, once** / **No** under it. Denied from the CLI, read back as
      gone from `agent approvals`, and `probe.txt` was never written. Same agent,
      same prompt, that wrote the file unasked before this ticket.

      **The disclosure is once per conversation, and proving that took two
      tries.** Every turn after the first resumes with `session/load`, whose
      reply carries `modes` too, so the first cut printed the paragraph above
      every turn. A `DISCLOSED` map keyed on Warp's conversation id — the
      `liveness` pattern — rations the *telling* while the mode request is still
      re-sent each turn, because `session/set_mode` is idempotent and a resumed
      session may not come back where it left. Confirmed across four turns: one
      note, and a later turn that answered *"Fourteen."* carried none.

      **Two process errors on the way, both worth more than the fix.** The first
      live run measured a binary built *before* the gate was written — the
      feature looked broken while its unit test passed, which is the shape of a
      real bug, and twenty minutes went into the wrong place. Then the rebuild
      never started, because `until ! pgrep -f "release/warp-oss"` matched the
      `bash -c` running the loop: it waited 34 minutes for itself. Both are now
      in `CLAUDE.md`, and both are the read-back rule wearing new coats — *after
      any mutation, confirm the mutation*, where the mutations were a compile and
      a process exit.

      **Reviewed after shipping, and four things changed** (advisor, 2026-08-30).
      Two of its points were already built and one it could not have known —
      the pinned-test correction it flagged had been found and made
      independently, and re-sending the mode after `session/load` was already
      how the news gate works. What was genuinely missing:

      - **The note listed mode ids and not their descriptions**, which is a
        problem shown to the person: `auto, default, acceptEdits, plan, dontAsk,
        bypassPermissions` still requires guessing which one asks, and `dontAsk`
        and `auto` both *sound* like not-asking while only one is. The list now
        carries the agent's own description of each. The note stops being "you
        have a problem" and becomes the lever, with Warp still recommending
        nothing.
      - **An unadvertised id noted the problem and ran the turn anyway.** That
        was wrong and the advisor's argument beats the one written here: the
        thing the note is *about* is a session running under a policy the person
        did not choose. `WARP_FORK_ACP_MODE` is `WARP_FORK_CONTROL_BIND`-shaped
        — a typo would otherwise silently mean something — and unlike
        `CONTROL_BIND`, where refusing to start would take away `warpctrl window
        close`, refusing here costs only the turn. It now refuses, and so does a
        `set_mode` the agent rejects. **A refusal is not rationed by the news
        gate**: the gate spares a reader a repeated paragraph, and a refusal is
        not read-and-continued-past.
      - **`session_mode` in the event log.** This is T14.17's direction and its
        sharpest case: a session with no permission events reads identically
        whether nothing needed asking or the agent's classifier was answering,
        and the mode is the line that tells a morning-after reader which. Written
        every turn, unlike the note, because a log is read by something that does
        not tire.
      - **`session_mode` on `agent.list`.** The orchestrator's surface, and it
        sits with `quiet_for_seconds`/`waiting_for_you` for their reason: a
        session working and asking nothing is a silence those two cannot explain.
        Verbatim and **not a safety signal** — Warp does not rank modes.

      **And running the review's own additions found a bug in one of them.**
      `agent.list` reported `session_mode: auto` for a session Warp had *just*
      moved to `default`: the recorded mode came from `session/new`'s reply and
      nothing updated it when the request was acknowledged. A status field that
      is confidently wrong is worse than an absent one — an orchestrator reading
      `auto` concludes the classifier is answering, on the one session where it
      demonstrably is not. Recorded on acknowledgement now, never on request,
      because what belongs there is the mode the session is in rather than the
      one Warp hoped for. **Two of the three surfaces added in this pass were
      wrong or missing until they were run**, which is the argument for the rule
      in one line.

      One citation is worth pinning because the advisor could not find it: the
      protocol's example mode ids are at **`agent.rs:5038`** of
      `agent-client-protocol-schema` **1.5.0**, the version `Cargo.lock` pins.
      The advisor read 1.7.0, which this fork does not compile. The conclusion is
      unchanged either way, since opacity is what carries it.

      **Named as unverified:** whether `session/set_mode` is accepted between
      `session/new` and the first prompt on every agent that advertises modes —
      one agent, one ordering, is what has been run. Whether a mode requested at
      session start survives `session/load` on resume; the request is re-sent
      each turn precisely because that is untested. And **an unexplained
      flakiness seen twice in four turns**: a turn that produced no output at
      all, no note and no answer, with the event log showing a `session_start`
      and no `stop`. It predates this ticket as far as anything here shows, it is
      not the mode path, and it is recorded rather than explained — chasing it is
      its own ticket, and calling it fixed or harmless would be inventing a
      finding.

- [x] **T14.21** **A phone can stop a runaway, and "monotone" was the wrong
      criterion.** `agent.cancel` joins `PAIRABLE_ACTIONS`, and the argument
      that got it there is not the one this work started with.

      The starting criterion was *"monotone — it can only make less happen"*,
      inherited from how `agent.deny` earned its place. That is too loose to be
      a security test: kill is monotone, and so, grotesquely, is `rm -rf`. The
      load-bearing form is narrower and is now written into the module docs: an
      action may only prevent **proposed future effects** and may not destroy
      **existing durable state**. `agent.deny` passes exactly — Escape on a
      prompt that has not run touches nothing that exists. `window.close`
      fails, which is the principled reason it stays refused rather than the
      hand-wave it had before: it discards unsaved panes, and that is state no
      routine mechanism already destroys.

      `agent.cancel` passes approximately, and the delta is recorded rather
      than smoothed over, because a person consenting to this should see it.
      Cancelling interrupts a turn that may be mid-side-effect — a rebase
      half-applied, a file half-written — and `events.subscribe` is already
      pairable and already carries tool calls live, so a hostile token holder
      could time one to land at the worst moment. What settles it is that
      interruption is an authority the environment already holds over every
      in-flight operation: Ctrl-C, a dropped connection, the panel's own stop
      button. A tool run that cannot survive interruption is already broken by
      things no token gates. The handler is Stop and not Kill; the
      conversation, transcript and pane all survive.

      Deliberately **not** behind `WARP_FORK_REMOTE_APPROVE`. Cancel is
      deny-shaped, and gating it beside `agent.approve` would imply the approve
      argument covers it. That argument is about causing what an agent
      proposed; this can only stop it.

      It completes a loop whose read half was already granted, which is the
      strongest form of the case: `agent.list` already reports
      `quiet_for_seconds` to a paired phone, so the wedge has been visible from
      the couch since T14.10 and only the answer was missing. The console now
      draws that number beside the button, because a symptom and its only
      remedy belong in the same place — and it stays a symptom rather than a
      verdict, since a long compile and a dead agent look identical from there.

      Two things it does not do, both said on screen rather than left to be
      discovered. It is one-way: restarting needs `agent.prompt`, which causes
      effects and so can never be pairable, and the row says stopping keeps the
      conversation while picking the work back up happens at the machine. And
      it cannot reach a CLI agent running in a pane — that is not a
      conversation, and the remedy there is keystrokes into a PTY, which is the
      boundary this list exists to hold. T14.20's finding stands: that
      limitation is a hook that does not answer yet, not something to tunnel
      through pairing.

      The button is gated on two independent facts, which is T14.6's rule and
      the bug it was written for: `can(CANCEL)` is about the device, `is_busy`
      is about the entry. Drawing it from the device alone would offer to
      cancel finished work.

      Not verified by running: that cancelling causes no automatic retry in
      Warp or in either measured agent. The falsifier is a wedged turn
      cancelled through a paired device credential rather than the Unix
      socket, which would also exercise `ensure_pairable` and the broker end to
      end.

- [x] **T14.22** **The consent path, watched on the wire — and the note that
      hedged about a question it had already answered.** First run of the
      2026-09-01 horizon. Full log: `.fork/runs/run-2026-09-01/friction.md`.

      Seven turns against `claude-agent-acp` 0.70.0 in `default`, event log and
      transcript on throughout, binary rebuilt at run start. **The refusal path
      has now been observed rather than argued**, which was the horizon's whole
      point: Warp answered a real `edit` request with `reject_once` — a per-call
      rejection and not `Cancelled` — the agent survived it, and turn 2 closed
      the loop through `agent approve` to a write on disk. `options_offered`
      came back `["Deny", "Allow Once", "Always Allow"]`, re-confirming against
      0.70.0 the ordering `acp_permission` exists because of.

      **The finding: a requested mode does not survive `session/load`, on every
      turn.** `session_mode`, written from the agent's own reply, read `current
      auto` on all four turns of one conversation, each after Warp had set
      `default` on the turn before, while the agent asked for permission on every
      one — behaviour `auto` does not have. So the session returns from every
      resume in the agent's own mode, and `mod.rs`'s unconditional per-turn
      `set_mode` is the only thing that puts it back. Deleting it as redundant
      would not fail loudly: every turn after the first would run in `auto`, the
      classifier would answer, and the event log would show *zero* permission
      requests — which reads as "nothing needed asking" and means "Warp was not
      in the loop". The measurement is recorded at the call site rather than
      here, because that is where someone stands while deciding the line is
      redundant.

      Two things this corrects in the horizon's own framing. It called the
      re-send *"untested"*; it has always been pinned by
      `a_repeated_request_is_still_sent_while_the_note_goes_quiet`, and
      `Decision::of` already reasoned that a resumed session "may have come back
      in a different mode than it left". The design was right, documented and
      tested — only the reality of the hazard was unmeasured. And what I cannot
      separate from outside is whether the agent truly reverts or merely reports
      its opening mode on load; the falsifier is to send `set_mode` only on
      `session/new` and resume.

      **Fixed: the mode note reported a hedge in place of an answer.** It said
      *"whether the agent honours the request is the agent's to answer"* at a
      point where every path reaching it has had `set_mode` return `Ok` — a
      failure returns earlier and the turn never runs — with `mode::acknowledged`
      recording that very fact two lines above the emit. Its `current` also named
      the mode the session was leaving. **This is the second instance of one
      defect**: `an_acknowledged_mode_becomes_the_reported_one` fixed exactly this
      for `agent.list`'s status field and the prose kept it, so what looked like
      an oversight is a fix applied to one of two surfaces — the unfixed one
      being the sentence a person actually reads. The note now says the agent
      accepted and names the opening mode in the past tense;
      `describe_current`/`describe_opened` split so each arm asserts only the
      tense it can support. The emit-after-success precondition is documented on
      `Decision::Request`, because the new wording would become a lie if a future
      caller emitted it earlier.

      The old test **asserted the defect**, requiring the hedge on the reasoning
      that "requesting is not receiving" — right in general, applied one step too
      early. Replaced and calibrated by making it fail.

      **Also fixed, and it is a leftover from this fork's own review a fortnight
      earlier**: `NOTHING_IS_WAITING` was added so the empty-approvals string
      would have one home, after being edited in one place and left red in
      another. The constant landed; `render_approvals` kept its duplicate. The
      two were identical so nothing was red, and **only `dead_code` noticed** —
      the argument for chasing a warning rather than silencing it.

      Unknowns 2 and 3 came back as absences and are labelled as such. Two
      requests never parked at once: prompted explicitly for parallel calls the
      agent serialized, releasing the second 0.1s after the first was answered,
      so the dispatch loop was never asked to hold two — a fact about the agent,
      not about the fork. No turn vanished: 4/4 closed, permission events
      balanced 8/8. Unknown 4 was **not reached**, which is weaker still, and
      there is no compaction detector that could have been running.

      Confirmed by running rather than by test, incidentally: the transcript
      hardening including `keep_dir_out_of_git`, which shipped with no test —
      this run's transcript is `600` in a `700` directory with its `.gitignore`
      written, and the documented residual is visible in the same listing, every
      `644` file predating the fix. And the `{turn}:{rpc_id}` scoping held
      against real traffic, two turns opening with the same JSON-RPC id `0`.

      Not verified: anything at length. The eight-hours caveat is untouched.

