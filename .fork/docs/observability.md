# The viewer: one trace, from the record the agent already keeps

**Filed 2026-09-04. Phase 0 done 2026-09-05**, `7bb323cbc` and the run in
`.fork/runs/viewer-phase0-2026-09-05/`. Every field named below was read off
a real file on this machine; the section headed *Unverified* is the list of
what was not.

## The frame

The maintainer's position, stated 2026-09-04 and adopted here: **third-party
harnesses already keep their transcripts on disk, so agent observability should
parse those rather than instrument the wire, and Warp's part of it collapses
mostly to a UI.** The plan graph is the same principle already in the tree:
`graph.rs` is a TOML file and a loop over verbs that existed.

This file takes that frame and asks the only two questions it leaves open: what
does Warp hold that the harness's file does not, and what is the smallest thing
that shows both.

## The measurement that decides the shape

Run 2's classifier work did this without naming it (`.fork/runs/classifier/README.md`).
Warp's event log had truncated 14 of 44 commands at its 320-character preview.
Claude Code's own session file held the full input for all 59 calls, and the two
joined on the `toolu_…` id Warp already writes as `call_id`. **59 of 59
matched, none missing from either side.** That is the frame in one number: the
harness's file is the record, and Warp's log is a thin index into it.

## The three records, as they are on disk today

### Claude Code: `~/.claude/projects/<slug>/<session-id>.jsonl`

Read 2026-09-04 in this repo's project directory: 116 files, 261 MB. One JSON
object per line. The fields the viewer needs, all present on every line of the
kind named:

| line `type` | what it carries |
|---|---|
| `user`, `assistant` | `uuid`, `parentUuid` (the conversation is a tree, not a list), `timestamp`, `sessionId`, `cwd`, `gitBranch`, `version`; `message.content` is an array of blocks: `text`, `thinking`, `tool_use` (`id` = `toolu_…`, `name`, `input`), `tool_result` (`tool_use_id`, `is_error`, `content`). `assistant` lines carry `message.usage`. `user` lines carrying a `tool_result` also carry `toolUseResult` with the structured outcome (`stdout`, `stderr`, …). |
| `system` / `compact_boundary` | **the compaction**, with `compactMetadata`; the line *after* it is a `user` line with `isCompactSummary: true` holding the summary the agent continued from (this row said "before" until 2026-09-05; the fixture in `trace_tests.rs` is the correction). 49 across this project's sessions. |
| `system` / `turn_duration`, `stop_hook_summary`, `away_summary`, `local_command` | turn timing and the hooks that ran. |
| `permission-mode` | `permissionMode`, written whenever it changes; the user's sessions here are wall-to-wall `auto`. |
| `attachment`, `queue-operation`, `last-prompt`, `mode`, `bridge-session`, `atis-latch`, `file-history-snapshot` | bookkeeping. Opaque to the viewer; rendered as nothing. |

Two facts about it that matter more than the field list. **Compaction is a
boundary in the file, not a replacement of it**: everything before the boundary
is still there, so I21's rule that *compaction is a precondition for continuing
a session, never for viewing one* is met by construction by any tool that reads
this file. And **the file is written before the hook fires** (TR-EVENTS-B,
2026-08-30), so it is a live record, not an after-the-fact export.

### Warp: `WARP_FORK_EVENT_LOG`, one JSONL per conversation

Nine event kinds, read from `.fork/runs/run-2026-09-01/events/`: `session_start`,
`session_mode`, `session_model`, `prompt_submit`, `tool_start`, `tool_complete`,
`permission_request`, `permission_replied`, `stop`. Every line carries `ts`,
`seq`, `agent`, `source`, `session_id` (Warp's conversation id), `cwd`,
`project`. Tool and permission lines carry `call_id` (the `toolu_…` id) and
**`linked_session_id`**, the agent's own session id, which is the join to the
file above. Permission lines carry what the harness never records: `decision`,
`answered_by` (`control_plane`, the panel, a paired device), `can_approve`, and
the reason a request was refused.

**One defect, and it was phase 0, closed 2026-09-05.**
`app/src/event_log/local_agent.rs` wrote `linked_session_id: None`.
`local_agent` knows Claude's session id (it is the conversation token it
round-trips for `--resume`), so the join key existed and was not written. It
is now taken off the stream's `system/init`, which is the first line Claude
writes and precedes every tool event, so the first tool line of a turn already
carries it; a `--resume` that misses relinks to the session Claude actually
ran. Pinned in `translate_tests.rs` and `event_log/local_agent_tests.rs`.

**And on the ACP path the log carries no tool input at all**, measured on the
phase 0 run: `tool_name` is the ACP kind (`read`, `execute`) and
`tool_input_preview` is `None`. Run 2's "truncated at 320 characters" was the
`local_agent` shape. For the ACP path the harness file is the only record of
what ran, which makes the join load-bearing rather than convenient.

### opencode: `~/.local/share/opencode/opencode.db`

SQLite, not files. Tables `session` (125 rows here; `directory`, `title`,
`agent`, `model`, token counts, `time_compacting`), `message` and `part` (`data`
as JSON), and a `permission` table. Named so the multi-agent claim is honest:
the viewer's second harness is a database with a different shape, and the
first version should read Claude Code's file and treat opencode as the test of
whether the abstraction was right.

## What Warp holds that no harness file does

- **What Warp decided, and which surface answered.** Measured: `opencode`
  records a denied command as a plain error with no notion that anyone refused
  it. A reader of the harness's file alone sees a failure where there was a
  decision.
- **Why a request was refused** (`approve_refused_because`), and what Warp would
  never have selected (`warp_can_select` on each offered option, T20.2).
- **Live state.** A parked approval, a turn quiet for N seconds, a wedge. A
  transcript is history; `agent list` and `agent approvals` are the heartbeat,
  and they stay where they are.
- **Warp's own words.** The `[Warp]` notes and the mode disclosure are kept out
  of the transcript on purpose so an agent never reads them as its own; the
  viewer is the one place they belong beside the agent's, labelled.
- **The join itself.** Two agents, two formats, one conversation id.

Everything else the fork's instruments record is a copy of something the
harness already wrote. That includes `WARP_FORK_TRANSCRIPT`, which exists so an
agent can grep back what compaction dropped: Claude Code's file already holds
it. The transcript keeps one advantage, that it sits under the pane's cwd where
the agent can read it without asking, so it stays for now. Whether it stays
after the viewer exists is a question this file raises and does not answer.

## The design, in one paragraph

**Three records, one join, read-only.** The harness's file is authoritative for
what was said and done. Warp's event log is authoritative for what Warp
decided. The viewer merges the two on `linked_session_id` and `call_id`, orders
by timestamp, labels every line by who authored it, and never writes anything.
It sends nothing to a model and consumes no context window, which is the whole
of I21's ask. Nothing new is captured: if the viewer wants a field, the question
is which of the two files already has it.

## The smallest version that is still the idea

**`warpctrl agent trace <conversation-id>`.** Find Warp's event log for the
conversation, read `linked_session_id` off it, locate the harness's session
file, merge, print. One verb, no window, no pane, no consent surface.

Output is JSON lines by default, one object per merged row:

```
{"ts": …, "who": "person" | "agent" | "warp" | "harness", "kind": …, "call_id"?: …, "text"?: …, "raw": {…}}
```

`--pretty` renders the same rows as text with a one-character gutter for `who`.
`--html` writes a self-contained static page: the escaping happens in Rust at
generation time, so it takes on none of the console's DOM-sink discipline and
adds no route. Open it in the browser that is already on the Windows side.
That is the UI, and it is a file.

A live view in the console (T12) was deliberately **not** in this version,
on the argument that a full transcript on a phone is a disclosure the pairing
credential was never sized for. **It is in phase 3, built 2026-09-05 at the
maintainer's ask**, as the new pairable action that paragraph predicted;
`pairing.rs` carries the argument beside the entry and the decision is in
`.fork/decisions/`. Short form: the event stream a phone already holds carries
tool names, input previews and directories for every agent, live, so the
trace is the same material at full resolution rather than a new kind of
disclosure; it is a read; and *observing runs remotely* is what the phone is
for.

## Rendering rules

- **Provenance first.** Four authors, always shown: the person, the agent, Warp,
  and the harness itself (compaction boundaries, hook summaries). Warp's notes
  never read as the agent's, which is the same rule the transcript enforces by
  omission and the viewer enforces by label.
- **A compaction is a boundary row**, drawn from `compact_boundary`, with the
  `isCompactSummary` text behind a fold. What the agent could see afterwards is
  visible; what it could no longer see is above the line, still there.
- **A tool call is one row that changes state**, the same shape the composer
  settled on (`tool_row`): `tool_use` opens it, `tool_result` closes it,
  `permission_request`/`permission_replied` from Warp's log attach to it by
  `call_id`. `is_error` and Warp's `denied` are different states and drawn
  differently.
- **Thinking is shown if present** and its absence is not a defect; the ACP
  agent emits none (T20.4) while Claude Code's file has 67 `thinking` blocks
  in one 695-line session.
- **Usage is a footer, not a feature.** `message.usage` per assistant turn,
  summed. The context ring already exists in the panel; this is the same number
  after the fact.

## On Windows, the file is inside the distribution

The agent runs in WSL (T18), so its session file is at
`/home/<user>/.claude/projects/<slug>/` inside the distribution, and the Warp
that would render it is on Windows. That is the T20.1 shape exactly, two
processes and two filesystems, and the fix already exists:
`session::filesystem::native_path` turns the guest path into the form this
process can open (`\\wsl.localhost\<distro>\…`). The viewer reuses it and must
not invent a second conversion.

## The format is not ours, so parse it like a guest

Claude Code's file layout is not versioned for third parties. Two versions of
the ACP shim already gave opposite answers to one question in a week. The rules:

- Record the `version` field on every line read, and print it in the header.
- Parse the handful of fields named above. Everything else is `raw`.
- A line that does not parse becomes a row with `who: "harness"` and the raw
  text; it never fails the render.
- When the shape changes, the failing case is the calibration: keep one real
  session file per version seen under `.fork/viewer/fixtures/` and pin the
  parse against it.

## What already exists, so nothing is duplicated

- `warpctrl agent read` reads exchanges from Warp's live model and needs the
  owning surface alive for tool results. The viewer reads files and works with
  Warp closed. They answer different questions and both stay.
- `app/src/ai/transcript.rs` writes the fork's own Markdown record. The viewer
  does not read it; the harness's file is richer and the transcript's job is
  the agent's, not the person's.
- `.fork/runs/classifier/build_eval_set.py` already performs the join in Python for
  one run. It is the working prototype of phase 1's logic, and the first thing
  to read before writing it in Rust.
- `session::filesystem::native_path` for the Windows read.

## As built: `warpctrl agent trace`

`.fork/docs/manual.md`, "One trace of a conversation". Rows carry `from`
(`warp` | `harness`) beside `who`, because the person's prompt is in both files
and the viewer shows that rather than deduplicating it. `harness_lines_skipped`
in the header counts the bookkeeping kinds rendered as nothing. `usage` is the
last harness row.

**Phase 2, the same night.** `trace_render.rs`: text with the gutter for
`--output-format pretty` (the default, as for every other `warpctrl` verb;
`ndjson` keeps phase 1's rows) and `--html FILE` for the page. A call is one
item drawn once at its earliest stamp with both files' timestamps beside it,
which is how the clock finding below is shown rather than hidden. The page
loads nothing, runs nothing, and escapes everything at generation time; a test
feeds it a prompt containing `<script>` and asserts on the entity. Written
owner-only, because it holds the whole conversation. The text form drops Warp's
copy of the prompt when the harness has the same text; the JSON keeps both.

**The two files are stamped by two clocks, and "order by timestamp" above was
written as if they shared one.** Found on the first live trace of the phase 0
run: Warp's `tool_start` for the Read call is stamped 22:42:55.955 and the
harness's `tool_use` for the same `toolu_` id 22:43:01.279, about five seconds
later, and the same for the Bash call. That is not latency, since the agent
writes its line before it tells Warp about the call; it is the Windows clock
against the distribution's, the same skew the cancel run's README recorded
between the app log and the WSL shell. So a plain merge puts every Warp row
before every harness row, including Warp's `stop` before the agent's first
word. The header now carries `joined_calls` and `clock_offset_ms`, the median
of harness-minus-Warp over the calls both files hold. **Disclosed, not
applied**: shifting one file's clock by a median is a guess dressed as a
correction, and the `call_id` join is the one ordering that is actually true.
Phase 2's renderer should draw a call as one row from both files and let the
clocks disagree around it.

## Phases

0. **Done 2026-09-05.** `linked_session_id` written on the `local_agent` path
   (`7bb323cbc`). Measured live on the Windows build, the user's own profile,
   `warpdev.ps1 -EventLog` (product plus the log, agent in its own `auto`):
   two tool calls, two `tool_start`/`tool_complete` pairs each carrying
   `linked_session_id`, zero permission lines, both `call_id`s found as
   `tool_use`/`tool_result` in `~/.claude/projects/-home-effatha-git-warp/<id>.jsonl`
   (Claude Code 2.1.257). The join does not depend on permissions being
   exercised. Slug rule measured: `[^A-Za-z0-9]` → `-`, case kept, so it is
   lossy and is computed from the `cwd` on Warp's lines, never inverted.
   `.fork/runs/viewer-phase0-2026-09-05/`.
1. **Done 2026-09-05.** `warpctrl agent trace <conversation>`,
   `crates/warp_cli/src/local_control/trace.rs`: the merge, the join, the four
   authors, a header row naming both files and every harness `version` seen.
   Not a catalog action, so the count is unchanged and nothing is gated or
   paired. Pinned against the phase 0 files, copied unedited to
   `.fork/viewer/fixtures/` (`claude-code-2.1.257.jsonl`,
   `warp-events-acp-2026-09-05.jsonl`), plus synthetic lines in the real
   shapes for a compaction, a denial, a failed result and an unparseable line.
   Ten tests. The Windows read takes `--harness-dir` rather than `native_path`,
   because the verb has no session to ask; the caller owns reachability.
2. **Done 2026-09-05.** Text for the default output format, `--html FILE` for
   the page; sixteen tests across the two modules. The page is the deliverable
   the frame asked for, and it is a file.
3. **Done 2026-09-05, the live view.** `agent.trace`, a catalog action (115
   now; both pins) and the seventh pairable one, running phase 1's merge
   inside the instance -- `app/src/local_control/handlers/trace.rs` calls
   `warp_cli::local_control::trace` unchanged. The console's conversation
   view polls it for the *tail* of both files (line-count cursors, exact
   because both files are append-only) and redraws with the phase 2 folding
   rules ported to `console.js`, still through `textContent` only. The
   instance finds the harness's file itself, including inside the
   distribution for a WSL session; `WARP_FORK_HARNESS_DIR` overrides. The
   launcher's product profile carries `WARP_FORK_EVENT_LOG=on` since this,
   because the log has a reader. `warpctrl agent trace --live` asks the
   instance the same way. opencode's database stays where it was: only if a
   friction log asks. **And a text-only turn joins now**: found the same
   night while measuring remote control, a conversation whose turns called
   no tool traced as Warp's half alone, because on the ACP path only tool
   lines carried `linked_session_id`. The `session_mode` and
   `session_model` lines carry it since. **Measured 2026-09-05 on the Windows build**
   (`.fork/runs/viewer-phase3-2026-09-05/`): a device paired the way a phone
   does got the seven actions, asked for the record mid-turn and as a tail
   after it, and the instance found the harness's file inside the
   distribution on its own (`\\wsl$\ubuntu\home\effatha\.claude\…`);
   the page drew both calls with both clocks, 6.3 s apart.

## Unverified, as of filing

- ~~The slug rule for `~/.claude/projects/<slug>` beyond `/` → `-`.~~ Measured
  2026-09-05: `/`, space, `_` and `.` all become `-`. Non-ASCII letters not
  tried.
- ~~Whether `local_agent` has the session id at the moment the first tool
  event is written, or only after `init`.~~ Read 2026-09-05: `init` is the
  first line of the stream, a `tool_use` never shares a line with it, and the
  loop links the log before draining tool events. It also has one *before*
  `init`, the `--session-id`/`--resume` argument, and deliberately does not
  use it: a `--resume` that misses starts a fresh session.
- The first line of a session file can be a bookkeeping kind with no `cwd`,
  `version` or `gitBranch` (seen: `operation`, `sessionId`, `timestamp`,
  `type`). Take those from the first line that carries them.
- What `part.data` in opencode's database looks like; not read.
- Whether `native_path` returns `Some` for a `~/.claude/…` path when the
  session is not routed (T16's `Host`/`Local` distinction should not matter
  here, and that is the assertion to test).
- Nothing here has been timed. A 261 MB project directory is read by `ls -t`
  and one file, never scanned.
