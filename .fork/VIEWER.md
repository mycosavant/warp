# The viewer: one trace, from the record the agent already keeps

**Filed 2026-09-04.** Specified, not built. Every field named below was read off
a real file on this machine the same day; the section headed *Unverified* is the
list of what was not.

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

Run 2's classifier work did this without naming it (`.fork/classifier/README.md`).
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
| `system` / `compact_boundary` | **the compaction**, with `compactMetadata`; the line before it is a `user` line with `isCompactSummary: true` holding the summary the agent continued from. 49 across this project's sessions. |
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

Nine event kinds, read from `.fork/run-2026-09-01/events/`: `session_start`,
`session_mode`, `session_model`, `prompt_submit`, `tool_start`, `tool_complete`,
`permission_request`, `permission_replied`, `stop`. Every line carries `ts`,
`seq`, `agent`, `source`, `session_id` (Warp's conversation id), `cwd`,
`project`. Tool and permission lines carry `call_id` (the `toolu_…` id) and
**`linked_session_id`**, the agent's own session id, which is the join to the
file above. Permission lines carry what the harness never records: `decision`,
`answered_by` (`control_plane`, the panel, a paired device), `can_approve`, and
the reason a request was refused.

**One defect, and it is phase 0.** `app/src/event_log/local_agent.rs:102`
writes `linked_session_id: None`. `local_agent` knows Claude's session id
(it is the conversation token it round-trips for `--resume`), so the join key
exists and is simply not written. Until it is, the viewer works for the ACP
path and not for the other one.

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

A live view in the console (T12) is deliberately **not** in this version. The
console is served to paired devices, and a full transcript on a phone is a
disclosure the pairing credential was never sized for (`PAIRABLE_ACTIONS` is
narrow because a QR code is weak). If it is ever wanted, it is a new pairable
action and therefore a posture question, which is frozen.

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
- `.fork/classifier/build_eval_set.py` already performs the join in Python for
  one run. It is the working prototype of phase 1's logic, and the first thing
  to read before writing it in Rust.
- `session::filesystem::native_path` for the Windows read.

## Phases

0. **Before any rendering.** Write `linked_session_id` on the `local_agent`
   path. Then launch the product profile, run one panel turn, and confirm on a
   live session that the event log carries `linked_session_id` and a
   `tool_start` per call under `auto`, where Warp is never asked; the join must
   not depend on permissions being exercised. Measure the slug rule for the
   project directory name on a cwd containing characters other than `/`.
1. **`agent trace`, JSON lines.** The merge, the join, the four authors. Pinned
   against one real fixture per harness version.
2. **`--pretty` and `--html`.** The static page is the deliverable the frame
   asked for.
3. **Only if a friction log asks:** opencode's database, and a live view.

## Unverified, as of filing

- The slug rule for `~/.claude/projects/<slug>` beyond `/` → `-`.
- Whether `local_agent` has the session id at the moment the first tool event
  is written, or only after `init`.
- What `part.data` in opencode's database looks like; not read.
- Whether `native_path` returns `Some` for a `~/.claude/…` path when the
  session is not routed (T16's `Host`/`Local` distinction should not matter
  here, and that is the assertion to test).
- Nothing here has been timed. A 261 MB project directory is read by `ls -t`
  and one file, never scanned.
