> Idea I17, split out of `.fork/IDEAS.md` on 2026-09-05. History; read the section you came for.

# I17 — The agent trajectory, traced locally

**Raised 2026-08-23.** The ask: trace a whole agent trajectory — messages, tool
calls, failures, plans, every agent event — and eventually render it in a thin
UI. Prompted by Zed's Delta beta and DeltaDB, which replicate the conversation
and the worktree together so a thread carries the full history of how code came
to be.

The fork's standard question first: **how much of this is already built?** More
than expected, and one gate is already open.

## What exists

**The trajectory is already instrumented.** `app/src/ai/agent_sdk/` is covered
in `#[tracing::instrument]` — `AgentDriver::new`, `AgentDriver::run_internal`,
`agent_sdk::run`, environment setup, attachments, git credential refresh —
carrying `conversation_id` and `environment_id` as span fields. This is
upstream's own work; nobody has to add spans to get a trajectory.

**The export path exists and the fork already unauthenticated it.**
`app/src/tracing/local_export.rs` is a `LocalHttpClient` that sends OTLP with
no `Authorization` header, selected only for loopback endpoints
(`native::use_local_export`). So a collector on `127.0.0.1` needs no
cloud-agent credential, which no local user could obtain anyway.

**And the interesting gate is already open.** Upstream's exporter drops every
span not marked `tags.cloud_agent = true` (`filter_cloud_agent_span`), which is
almost all of them. Commit `4951304e2` added `export_all_spans`: when the
endpoint is local, keep everything the `EnvFilter` admitted — "what surfaces
local agent/harness spans, which are created but discarded under upstream's
filter."

So the pipe is: run a collector on loopback, set
`WARP_CLOUD_AGENT_OTLP_ENDPOINT=http://127.0.0.1:4318`, and local agent and
harness spans flow to it. **Unverified end to end** — no collector has been run
against it in this fork. That is the first thing to do, and it is an
afternoon, not a project.

## Run against a collector, 2026-08-23 — and half the section above is wrong

> **Retraction.** "The trajectory is already instrumented" was a claim about
> `#[tracing::instrument]` attributes, made by grepping. A collector on
> loopback and a real local-agent turn say otherwise: **the agent path emits no
> spans at all.** The instrumented code in `app/src/ai/agent_sdk/` is the
> cloud/CLI driver, not the in-app conversation the fork's local agent serves.
> `app/src/ai/local_agent/` has **zero** instrumentation. I also read one
> arriving span named `run_internal` as `AgentDriver::run_internal`; its
> `code.file.path` is `app/src/lib.rs`. It is app startup. I matched on a name.

**The pipe itself works, and this is the first time anyone ran it.** A
hand-written OTLP/HTTP receiver on `127.0.0.1:4318` (no collector installed,
nothing left the machine) took **128 spans** from one session:
`persistence::initialize`, `launch`, `initialize_app`, `run_internal`, and a
lot of terminal-server IPC — `read_socket`, `write_commands`, `authenticate`.
`export_all_spans` does exactly what its doc comment says.

**And none of it is the trajectory.** A real turn — prompt in, `Bash` tool
call, `status: success` — added **zero** spans. Repeated with
`RUST_LOG=warp=trace,ai=trace`: still zero, and the span count *dropped* to 4,
which incidentally confirms the directive was applied. No span anywhere
carried a `conversation_id`.

## What is persisted, and in what shape

`agent_tasks.task` is a protobuf of **rendered message parts**, not a
structured trajectory. For a turn whose prompt was `Run: echo
HELLO_FROM_TOOL_42`, the whole record is three strings:

```
-1   Run: echo HELLO_FROM_TOOL_42        <- the user's input
-2   `Bash`                              <- the tool call. The name. That is all.
-3   Output: `HELLO_FROM_TOOL_42`        <- the model narrating, not a captured result
```

`agent read --tools` changes nothing here, because tool results are read from
the **live surface's action model**, not from the record — so they are gone the
moment the pane is, and were never there for a local-agent turn anyway.

## The gate, and it is one struct we own

Claude Code's `stream-json` delivers the whole thing. The fork's own test
fixture is the proof:

```json
{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"rm -rf build"}}
```

And `translate.rs:139` keeps one field of it:

```rust
#[serde(rename = "tool_use")]
ToolUse { name: String },
```

`id` and `input` are dropped at parse time. `tool_result` is not handled at
all — it sits in the deliberately-ignored list in `translate_tests.rs`, for the
good reason that Claude's stream is versioned independently and unknown lines
must not take a turn down.

**So the structured trajectory arrives at our door and we throw it away**, in
about ten lines of fork-authored code. That makes the capture problem small,
but it is a *capture* problem, not the rendering problem this page assumed.

**One trap, already documented in the code that would have to change.**
`ToolUse` is deliberately rendered as text and **not** emitted as a
`ToolCall`, because a `ToolCall` is an *instruction* — Warp's action model
would execute it, and Claude has already run it. A second `rm`, a second push.
Anything that captures the arguments must put them in the record without
turning them into an instruction; the existing test pins that and should stay
pinned.

## The plugin idea, 2026-08-23 — upstream already ships it

> **Second correction, this one in the idea's favour.** "Shipping a plugin to
> Claude Code might give us all the data we need" is not a proposal. **Warp
> already does it**, the plugin is already installed for you, and the events
> already arrive parsed. Everything below was found by reading after that was
> pointed out, and the reading was prompted by a single sentence about a
> sidebar showing activity.

**The plugin.** `ClaudeCodePluginManager`
(`terminal/cli_agent_sessions/plugin_manager/claude.rs`) installs
`warp@claude-code-warp` from the marketplace repo `warpdotdev/claude-code-warp`,
minimum version 2.1.0, plus an `oz-harness-support` plugin. There are siblings
for Codex, Gemini and OpenCode.

**The transport is OSC 777 through the PTY.** Not a socket, not a port, not a
collector — the plugin writes an escape sequence and Warp's terminal parses it
out of the stream. Nothing to configure and nothing to leak. (Codex has an
OSC 9 plain-text fallback, dropped once a rich notification arrives.)

**The protocol is the trajectory.**
`warp_core::cli_agent_protocol::CLIAgentNotification` carries `event`,
`session_id`, `cwd`, `project`, `query`, `response`, **`transcript_path`**,
`summary`, `tool_name`, **`tool_input`** (a full `serde_json::Value`),
`plugin_version` and `error_type`. The event vocabulary is
`session_start`, `prompt_submit`, `tool_complete`, `stop`, `stop_failure`,
`permission_request`, `permission_replied`, `question_asked`, `idle_prompt` —
which is messages, tool calls, failures *and* approvals, i.e. the list this
page opened by asking for.

**And it is not gated.** `pluggable_notifications`, `cli_agent_rich_input` and
`agent_harness` are all in `app/Cargo.toml`'s `default` list, and the only
`FeatureFlag` check in the listener is Codex-specific. This is what the
sidebar is showing.

**What is missing is retention, and only retention.** `CLIAgentSession` holds
*current state* — status, input state, plugin version, whether a rich
notification has been seen. **There is no history.** Events drive the UI and
are dropped. And `event/v1.rs` narrows the full `tool_input` JSON to a
`tool_input_preview`, keeping whichever of `command` or `file_path` it finds
and discarding the rest — **the same gate as `translate.rs`, in a different
file.** Twice now, the fork has found the trajectory arriving intact and being
flattened at the boundary.

## Better than capture: Claude already wrote it down

`transcript_path` is the part that changes the size of this idea. It points at
Claude Code's own session JSONL, which already holds everything. Measured
against a real transcript on this machine:

| record | count |
|---|---|
| `assistant` | 2,423 |
| `user` | 1,477 |
| `tool_use` — `Bash` | 1,124 |
| `tool_use` — `Edit` | 163 |
| `tool_use` — `Read` | 113 |
| `tool_use` — `Write` | 18 |
| `file-history-snapshot` / `-delta` | 34 / 37 |

Each `tool_use` carries its full `input`. And **the diff is already there,
exactly**: an `Edit` call's input is `{file_path, old_string, new_string}`,
which *is* the change, and a `Write` call's input is the whole new content.
No reconstruction, no separate diff store. (`file-history-delta` records point
at per-file backups rather than inlining a patch, so the tool calls are the
better source of the two.)

So the three elements really are all owned, and none of them is a capture
problem:

1. **Claude writes the complete record**, tool inputs and diffs included.
2. **Warp's plugin already says where it is**, on every event.
3. **`warpctrl` is already the read surface.**

**The smallest version that is still the idea** is therefore not
instrumentation and not a new store: *keep the `transcript_path` a session
already reports, and add a verb that reads it.* No protocol change, no new
capture, nothing extra written to disk.

**Decided by the user, 2026-08-23:** the tool *call* is the wanted data — name
plus input — and **not** tool-result bodies. That kills the disk-footprint
question this page raised: no file contents or command stdout need to be
retained, because the interesting part is what the agent decided to do.

## Run, 2026-08-23 — the join key is already in the database

> **Third correction, and the largest.** Hours earlier this page called the
> fork's local agent "the poorer of the two substrates", with only a tool's
> name available. **That is wrong.** The local agent has the complete
> trajectory, on disk, already linked to Warp's own record by a field the fork
> itself writes.

The two open questions were run rather than read, and both answered.

**1. The fork's local agent writes full Claude transcripts.** `claude --print`
is still a Claude session, so it still writes
`~/.claude/projects/<cwd-slug>/<session-id>.jsonl`. The three local-agent turns
driven during this investigation produced three transcripts, timestamped to the
minute they ran.

**And the file name is a key Warp already stores.** `agent_conversations.
conversation_data` for the `HELLO_FROM_TOOL_42` turn holds
`"server_conversation_token":"90115094-0a55-4761-a73d-52eac05a3f06"`, and the
transcript is `90115094-0a55-4761-a73d-52eac05a3f06.jsonl`. Not a coincidence:
`translate.rs:401` takes Claude's `session_id` from the `system` init event and
puts it into `StreamInit.conversation_id`, which Warp persists as the server
conversation token. **The fork already threads Claude's session id into its own
record.** It simply does not know that this makes the whole transcript
addressable.

Reading that transcript gives, for the turn whose Warp record was the single
string `` `Bash` ``:

```
[user]        Run: echo HELLO_FROM_TOOL_42
[tool_use]    name=Bash input={"command": "echo HELLO_FROM_TOOL_42",
                               "description": "Echo test string"}
[tool_result] "HELLO_FROM_TOOL_42"
[assistant]   Output: `HELLO_FROM_TOOL_42`
```

Tool name, full input, result, and the assistant's reply — all four, for free,
for a path this page had written off an hour earlier.

**So there is no capture work in this idea at all.** Not for CLI-agent
sessions (`transcript_path` on every event) and not for the local agent
(`server_conversation_token` + the cwd slug). The whole feature is a reader.
`translate.rs` and `event/v1.rs` still drop fields, and both are still worth
fixing on principle, but neither is on the path to a trajectory view.

**2. The Warp plugin is not installed on this WSL machine.** `claude plugin
list` shows no `warp@claude-code-warp`, and `claude plugin marketplace list`
has no `warpdotdev/claude-code-warp` entry. The sidebar activity is real, but
on this box it would have to be installed first — so the OSC 777 half of this
page remains **read, not run**, and the local-agent half above is what is
actually verified here.

**A hazard worth stating before anybody installs it to find out.** The plugin
installs at *user* scope and works by adding hooks to Claude Code, so it fires
on **every** Claude session on the machine — including whichever one is being
used to do the installing. That is not a reason not to do it; it is a reason
not to do it absent-mindedly mid-session.

## Reviewing the plugin against the fork's thesis, 2026-08-23

Read before installing, from a clone of `warpdotdev/claude-code-warp`. **The
repo ships two plugins, and only one of them belongs on this fork.**

### `warp` — clean, and it would work here

Seven hooks: `SessionStart`, `UserPromptSubmit`, `PostToolUse`,
`PermissionRequest`, `Notification` (matcher `idle_prompt`), `Stop`,
`StopFailure`. All of it is bash and `jq`.

**No network calls anywhere in it.** The only `https://` strings are the
`author.url` and `homepage` fields in `plugin.json`. The output path is a
single OSC 777 sequence — `\033]777;notify;warp://cli-agent;<json>\007` —
written to `/dev/tty`, or on Claude Code ≥ 2.1.141 returned as
`{terminalSequence: …}` for Claude to write. It reaches Warp through the PTY
and goes nowhere else. For a fork whose central claim rests on an egress
deny-list, this is about as good as a third-party integration gets.

**What each hook actually sends** — and this corrects what this page said
after reading only the Rust side:

| event | payload beyond `v/agent/session_id/cwd/project` |
|---|---|
| `session_start` | — |
| `prompt_submit` | `query`, **truncated to 200 chars** |
| `tool_complete` | **`tool_name` only** |
| `permission_request` | `tool_name`, **full `tool_input` JSON**, `summary` (preview ≤120) |
| `stop` | `query` + `response` (each ≤200), **`transcript_path`** |
| `stop_failure` | `error_type` |

> **Correction.** This page said "the protocol carries `tool_input`, a full
> `serde_json::Value`", and treated `event/v1.rs` narrowing it to a preview as
> the loss. The protocol does carry it — but **the plugin only ever populates
> it on `permission_request`**. `PostToolUse` sends the tool's name and
> nothing else. So on an auto-approving setup, which is the interesting case
> for this fork, the notification stream contains **no tool arguments at all**.
> Which is another argument that the transcript, not the event stream, is the
> substrate: the events are a *status* feed by design, and the truncation
> limits say so out loud.

**And it is not gated out of our builds — measured.** `should_use_structured`
requires both `WARP_CLI_AGENT_PROTOCOL_VERSION` and `WARP_CLIENT_VERSION`, and
skips builds at or below a known-broken release per channel. A live shell
under our own release binary reports:

```
WARP_CLIENT_VERSION=local
WARP_CLI_AGENT_PROTOCOL_VERSION=1
TERM_PROGRAM=WarpTerminal
```

`local` matches none of the `*dev*`/`*stable*`/`*preview*` patterns, so no
threshold applies. The protocol version comes from `FeatureFlag::HOANotifications`,
whose cargo feature is in `default` and which is added unconditionally rather
than by channel — so it is on here. **The plugin would function on this fork
as-is.**

### `oz-harness-support` — do not install this one

`DEFAULT_SERVER_ROOT = "https://app.warp.dev"` in
`skills/factory-files/scripts/validate_factory_files.py`, plus
`oz-parent-listener.sh`, `drain-mailbox.sh`, and skills named `oz-report-pr`,
`oz-upload-file`, `oz-notify-user`, `oz-finish-task`. This is the cloud harness
integration — precisely the surface this fork exists to remove.

It installs through a **separate** method (`install_platform_plugin`, distinct
from `install`), called from exactly one place:
`ensure_local_claude_child_plugins` in `pane_group/pane/local_harness_launch.rs`,
on the path that spawns a child running a *third-party CLI harness in a
terminal* — `StartAgentExecutionMode::Local { harness_type: Some(_) }`.

**The fork does not currently reach it**, because `warpctrl agent spawn` goes
through `spawn_hidden_child_agent_for_local_control` with
`orchestration_harness: Some(Harness::Oz)` — the transport seat, not a
terminal harness. So a spawned child never triggers the install.

**But that is off by accident, not by policy**, which is the distinction this
fork usually insists on. Nothing stops an in-app agent requesting a harness
child and quietly installing a cloud integration into the user's Claude Code
at user scope. A `fork::` predicate refusing `install_platform_plugin` is one
line and turns an architectural accident into a guarantee — the same move
`FORCE_DISABLED` makes for the telemetry flags.

### What is worth changing, and what is not

**Done 2026-08-23:** the refusal. `fork::cloud_harness_plugin_allowed()` is
`!is_active()`, and it is checked **inside the manager** —
`install_platform_plugin` *and* `update_platform_plugin` in
`plugin_manager/claude.rs` — rather than at the caller. Guarding the call site
would have closed today's single path; guarding the manager means a future
call site cannot reopen it by not knowing. `update` needed its own guard
because it re-adds the marketplace and reinstalls: it is an install by another
name, and covering only the one spelled "install" would read as covered while
leaving a gap.

The refusal returns `Ok(())` rather than an error — nothing downstream needs
the platform plugin, and reporting a failure would warn about a thing the user
never asked for. Pinned by a test that asserts the predicate *tracks*
`is_active()` rather than asserting a constant, so `WARP_FORK_POLICY=0` still
gives back upstream behaviour; run both ways.

**Still worth doing:** keeping `tool_input` in `event/v1.rs` instead of
flattening it to a preview string, on the principle that the fork should not
discard its own data — while remembering it only arrives on permission
requests.

**Not worth doing:** forking the plugin to lift the 200-character truncation.
The truncation is correct for what the events *are* — a status feed for a
sidebar — and the full content is in the transcript the `stop` event already
points at. Forking it would take on maintenance of a moving target to
duplicate something we can already read.

## What is still missing after all that

**1. The transcript is the right substrate, and there are two of them.**
"Perhaps that's simpler via transcripts" survived the measurement better than
the tracing half of this page did — and better than expected, because there
turn out to be two candidates. Warp's own conversation record is the poorer
one: it keeps rendered message parts and a tool's *name*. **Claude's session
JSONL is the richer one**, and it is complete without anyone doing anything.
Prefer it where a `transcript_path` exists; fall back to Warp's record where
one does not. Traces remain the right home for *timing and causality* and the
wrong home for message bodies — but note that with the agent path emitting
nothing, "join the two on `conversation_id`" currently joins a full table to
an empty one.

**2. ~~A decision this page cannot make: does the record keep tool output?~~**
**Settled 2026-08-23: no.** The tool *call* — name plus input — is the wanted
data; tool-result bodies are not. That removes the disk-footprint worry this
entry raised, since nothing has to hold file contents or command stdout, and
it means `MAX_PERSISTED_CONVERSATION_COUNT = 200` (the T8.3 trap) is a much
smaller consideration than it looked.

**3. The UI.** Deliberately last, and deliberately thin. `warpctrl agent read`
plus the store already makes the data inspectable without any UI at all, which
is the cheapest way to find out what a UI should show.

## On Delta, honestly

DeltaDB replicates a worktree and a conversation together in real time for
multiple participants. That is a distributed-systems product, not a tracing
feature, and this fork should not pretend the two are the same size. What
overlaps is the *premise* — that the conversation and the code belong to the
same record — and that premise is already available here for free, because
this fork's whole thesis is that the record is local. Whether any of DeltaDB
is open source has not been checked.

The version of this idea that fits this fork is the small one: **capture what
Claude already tells us, keep it in the record that already persists, and read
it back with a verb that already exists.** Multiplayer is somebody else's
problem, and single-player is the case this fork actually has.

## What this changes for T8.3

The reason for running this before the inbox, and it did change something.

**An inbox row can show the full trajectory, for both kinds of session, with
no capture work first.** This section said the opposite twice today before the
transcripts were actually looked at; what follows is what survived running it.

- **A CLI-agent session** (Claude in a pane with the plugin) reports a
  `transcript_path` on every event.
- **A Warp agent conversation on the fork's local agent** stores Claude's
  session id as `server_conversation_token`, which names the transcript file.

Both resolve to the same thing: Claude's own JSONL, holding every tool call
with its full input, and `Edit` inputs that *are* the diff. The inbox needs a
reader, not a capture pipeline. `translate.rs` and `event/v1.rs` still discard
fields and are still worth fixing, but neither blocks this.

The one thing a row does have to handle is **a missing transcript** — an old
conversation whose session file has been cleaned up, or a path that predates
the fork writing the token. Degrade to what Warp's own record holds rather
than assuming the richer source is there.

Nothing about T8.3's own plan is invalidated: `settled` is still a field on
`AgentConversationData`, eviction is still the trap, and the conversation list
view is still the place it goes.

## Related, already done

The slow-frame log (`warpui::frame_log`, T8.2) is the same pattern at a much
smaller scale, and it is worth noting as precedent: upstream had the
capability, it reported to Sentry, the fork force-disabled it, and the
replacement kept the measurement and dropped the network path. A local-first
Sentry replacement — crash and error reporting to a machine you own — is the
same move again, and is not yet written down as its own entry.

| | answer | what it changed |
|---|---|---|
| **I1** | Beside, not instead of | Removed the only large unknown. The inbox stays where the code already is. |
| **I10** | Dev-server preview, and an agent that instruments a build and watches you use it | Killed the browser and surfaced [I15](I15-computer-use.md). What is left of I10 is *a pane that renders an image and refreshes*. |
| **I13** | `main` is the CWD-follow anchor, and possibly the lead agent | Turned a layout nicety into an **anchor with three consumers**, and **corrected I6**, where "follow the focused pane" was my answer and the wrong one. |

Still open, neither blocking:

* **I7** — `view-as`. Will surface again during use; most of these did in the
  first two days.
* **I2** — composer. Running notes in progress.

## A note on how two of those went

I1 and I7 confirmed what was here. I10 and I13 did not — and both times the
correction came from *one sentence about what you were actually doing*, not
from more analysis on my side. I10 I had scoped as an expensive thing to argue
against; the real ask was two cheap things and a capability that already
exists. I13 I had scoped as cosmetic; it is structural, and it caught a defect
in a neighbouring entry I had already called settled.

Worth writing down because it generalises: the ideas in this file are graded on
what the code can do, and that grading is only as good as knowing what the
moment was. Where an entry still says "needs a sentence from you", that is not
politeness.

---

