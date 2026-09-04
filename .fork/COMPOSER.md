# The composer — a top-of-class agent surface

**Filed 2026-09-03, as its own file rather than a board entry, because it is not
a defect and it is not small.** The board tracks things that are wrong. This is
a thing that is *not good enough*, on the surface a person looks at all day, and
the fork is currently failing it — not by a little.

**Status, 2026-09-03: every step in the handoff is built and measured on the
Windows build.** Warp's own message kind; the asking note's mechanics said
once; tool calls as rows that change state; a turn's elapsed time and the
context ring fed; the approval card layered with the decision first. Record
9.4 : 1 → 6.0 : 1 and drawn 10.1 : 1 → 2.4 : 1 on same-mood runs, with
Warp's side byte-identical across the last three runs while the agent's prose
swung ±30% — so read the numbers as Warp's characters first. Details under
*As built*. What remains is listed at the end of that section.

---

## The measurement that started it

T20.4 asked whether Warp's composer drops the agent's prose. The first answer was
"no", taken at **zero** permission requests against a run that had forty-four —
answering an easier question than the one asked. Re-run 2026-09-03 with asks, on
the Windows build, `claude-agent-acp@0.73.0`, `WARP_FORK_ACP_MODE=default`:

The prompt asked for four steps with *"one short sentence"* of narration before
each. Three needed a file write, so three raised asks. A screenshot was taken
with an ask parked, then every ask was approved and the transcript compared with
what had been on screen.

**The agent's narration exists in the record and was never visible.** *"Creating
the first file with contents A."* is in the transcript. It is nowhere in the
screenshot — pushed off the top by Warp's own asking note plus the approval card.
What filled the visible panel was Warp's prose and two buttons.

| author | characters in that turn |
|---|---|
| **Warp** — mode note, asking notes, `Answered:` notes, tool labels | **2558** |
| **the agent** — all four narration sentences and its closing summary | **271** |

**9.4 : 1. The agent's share of its own turn is 9.6%.** At run 2's forty-four
asks the arithmetic is far worse. Nothing is dropped; almost nothing is shown.
From the seat those are indistinguishable, which is exactly why the first answer
was wrong.

---

## The architectural root, and it is one line

`Translator::note` (`app/src/ai/acp_agent/translate.rs:864`) emits Warp's own
words as:

```rust
api::message::Message::AgentOutput(api::message::AgentOutput { text })
```

**Warp's chrome and the agent's prose are the same message type in the same
stream.** There is no second channel. The renderer cannot style Warp's voice
differently, cannot collapse it, cannot let a reader hide it, and cannot count it
separately — because as far as it is concerned the agent said it.

The `[Warp]` marker is not a channel. It is text chrome that exists so
`transcript::strip_chrome` can keep Warp's asides *out of the transcript* and
stop an agent grepping its own history and reading Warp's words as its own. It
does nothing for the display.

Every one of these goes through that one path
(`app/src/ai/acp_agent/mod.rs`): the mode note (`:741`), the transcript
announcement (`:790`), the asking note (`:553`, built at `:1186`), and the
answered note (`:1345`, built at `:1389`).

**So the first piece of work is a channel, not a redesign.** Until Warp's voice
is a distinguishable message kind, every improvement below is a string-length
argument.

---

## What is wrong, specifically

Each of these was seen in a real session, and each is cheap to re-observe.

### 1. Warp outweighs the agent roughly 10:1 during any turn with asks ✅ **moved, see As built**

Measured above. The asking note alone is ~590 characters and says: what the agent
wants, the full `warpctrl agent approve <uuid> --digest <64 hex>` command, the
matching deny command, what a yes covers, that a paired device can answer, and
that *yes* only travels there when `WARP_FORK_REMOTE_APPROVE` is set. Then two
more paragraphs: what the call acts on, and which directory the session runs in
with an explanation of how Warp chose it.

Every sentence is true and was added for a reason. **It is sized for the first
request of a session and paid on every one.** Saying the mechanics once per
conversation and abbreviating afterwards returns ~500 characters per ask to the
agent and changes **no permission** — so it is not a posture change and is not
inside the freeze.

### 2. The approval card buried the one sentence written for a person

**Partly fixed 2026-09-03; recorded because it is the shape of the whole
problem.** The agent sends `{"command": "...", "description": "..."}` and the
card rendered `raw_input.to_string()` — so *"Compare local HEAD to Windows
checkout HEAD"* appeared inside an escaped JSON blob next to a multi-line shell
command. 29 of 29 parseable asks in the measured session carried a filled
`description`. Meanwhile `acts on` read **"not stated by the agent" for all 36
execute asks**, because ACP sends no `locations` for a shell call.

So the field that should say what a call does was empty, and the field that did
say it was the hardest thing on the card to find. `describe_tool_input`
(`acp_approval.rs`) now leads with it. **Still open:** `acts on` remains
structurally empty for every shell call, and nothing yet derives reach from the
command itself.

### 3. Tool calls render as bare labels ✅ **moved, see As built, step 4a**

`Terminal`. `Read File`. `Preparing file…`. No command, no path, no result, no
duration, no status marker on failure. The information exists — the transcript
carries `wc -l CLAUDE.md` and the file path — and the panel shows a noun.

Compare what the ACP stream actually delivers, captured with `acp probe`:
`tool_call` with `title`, `kind`, `rawInput`; `tool_call_update` with `content`,
`locations`, a diff for edits, `status`, and `_meta.claudeCode.toolResponse`
carrying stdout and stderr. Warp renders the title.

### 4. Nothing shows a turn's shape ✅ **moved, see As built, step 4b**

No progress, no step count, no elapsed time, no token or context indication —
`usage_update` arrives on the wire (`used`, `size`, and a `cost` object) and is
read for nothing on the display path. A long turn is an unmoving wall of text.

### 5. Thinking is absent, and that is currently not Warp's fault

`translate.rs:345` maps `AgentThoughtChunk` to `AgentReasoning`, documented
*"rendered as thinking, not as output"*. Measured twice at 0.73.0, once under an
explicit ultrathink prompt: **zero** thought chunks, `reasoningOutputTokens: 0`.
So there is nothing to render today. **Do not treat that as settled** — it is a
fact about one agent at one version, and the moment an agent sends thinking the
composer needs somewhere to put it that is not the output stream.

### 6. The approval card is a wall, and its controls are the smallest thing on it ✅ **moved, see As built, step 4c**

Four labelled lines of disclosure, then `Yes, once` / `No`. The disclosure is
good and should not be deleted. It should be *layered* — the decision visible,
the reasoning available.

### 7. A cancelled turn plausibly loses its last sentence

`translate.rs` buffers text and flushes on the next non-text update, on turn end,
or on the failure path. `mod.rs`'s `take_until` drops the driver future on
cancellation and nothing flushes `pending` there. **Read, not run** — re-read
2026-09-03 and the reading holds: the only flush after the prompt is at
`mod.rs:809`, inside the future `take_until` drops. Worth a test before it is
worth a fix, and the test needs a fake agent that streams text and is then
cancelled, which is more than a unit test on the translator.

---

## Three paths, and the ticket covers all of them

The composer must not become a thing that only works on one transport. That is
how the fork ended up with a transcript feature that was inert on Windows.

| path | how output arrives | what it has today |
|---|---|---|
| **ACP** (`app/src/ai/acp_agent/`) | `SessionUpdate` over the wire, translated in `translate.rs` | everything described above |
| **native CLI panel agent** (`app/src/ai/local_agent/`) | `claude --print --output-format stream-json` on stdin/stdout | the same `Translator` shape, its own `translate.rs`; the transcript pointer only landed here 2026-08-31, so it has a history of being the forgotten half |
| **CLI agent in a pane** (`app/src/terminal/cli_agent_sessions/`) | OSC 777 from the vendored Claude Code plugin — a *versioned* protocol | not a composer at all: the agent draws its own TUI in the pane and Warp sees events. `agent list` reports nothing for it and `agent approvals` sees it only through the hook |

**Third-party agents are the fourth case and are already reachable** —
`WARP_FORK_ACP_COMMAND` names any ACP agent — so anything built here should
assume the agent is not Anthropic's and degrade legibly when a capability is
absent.

---

## What "top of class" means, concretely

The maintainer named references worth studying rather than guessing at: **T3
Code**, **Cursor**, **VS Code + the GitHub Copilot chat extension**, and
**opencode v2 desktop** (specifically its animations). **Studied 2026-09-03**,
each by an agent reading the primary source — the repository at a named commit
where there is one, official docs and the vendor forum where there is not —
with every claim marked *verified* or *not verified*. The four reports are
below, followed by what they agree on and what this fork takes from each.

### The references

#### T3 Code

Repo `pingdotgg/t3code` (verified, default branch `main`). Not a desktop app in
the Warp sense: a Node WebSocket server wraps provider CLIs (Codex, Claude Code,
Cursor, Grok, OpenCode, Antigravity) and serves a React/Vite web client, an
Electron wrapper and a React Native mobile app (verified, `AGENTS.md`).
Everything below is the web client, `apps/web/src`. Stack: Tailwind 4,
`@base-ui/react`, `lucide-react`, `react-markdown`; no framer-motion (verified,
`apps/web/package.json`).

1. **Host vs agent voice.** Verified. The timeline is a discriminated union of
   row kinds: `message` (user/assistant), `assistant-meta`, `work`, `work-live`,
   `work-toggle`, `turn-fold`, `working`, `thinking`, `proposed-plan`
   (`components/chat/MessagesTimeline.logic.ts`, rendered by `TimelineRowContent`
   in `MessagesTimeline.tsx`). Host-originated activity (approval resolved,
   runtime error, tool denied, user-input requested) is a *work entry*, never
   prose; the server stamps a `tone` of `info | tool | approval | error`
   (`packages/contracts/src/orchestration.ts:432`). Rows render in
   `text-secondary-label` at `text-sm`; only `runtime.warning` gets
   `text-warning` and only severe failures get `text-destructive`, with a
   comment: *"Ordinary tool failures stay muted; only runtime errors and
   warnings get color"* (`MessagesTimeline.tsx` ~3071). Thread-level errors do
   not enter the timeline at all: `ThreadErrorBanner` is an `Alert
   variant="error"` above the composer, line-clamped to 3 with the full text in
   a tooltip, dismissable per thread+message.
2. **Tool call rows.** Verified. Collapsed: a 16px icon (`zap`/`brain`/`check`/
   `circle-alert` by tone, or the tool's own favicon), a single truncated label,
   and a chevron that is `invisible` when nothing can expand
   (`PlainWorkEntryRow`). Labels are verb phrases with tense following state:
   *"Running vp"* / *"Clicked"* / *"Failed to …"* / *"Declined to …"* /
   *"Stopped …"* (`packages/client-runtime/src/work-log/presentation.ts:107-120`;
   `docs/user/tool-activity.md`). Consecutive calls collapse into a group summary
   *"Ran 4 commands"*, *"Read 3 files"*, *"Used browser 18 times"*
   (`presentation.ts:411-417`); expanded groups scroll inside
   `max-h-[min(18rem,50dvh)]` with fade edges rather than growing the page.
   Expanded entry: a `<pre>` at 11px mono, `max-h-64`, containing in order the
   MCP JSON, the raw command, the `detail` (stdout), and changed-file paths
   (`buildToolCallExpandedBody`). Diffs are not inline in the row; changed files
   are a separate `AssistantChangedFilesSection` using `@pierre/diffs`. Failure
   marker: icon swaps to `circle-alert`, `aria-label="Tool call failed"`, no red
   unless severe. **No per-call duration is shown**; `durationMs` exists only for
   plan steps (`session-logic.ts:159`).
3. **Approvals.** Verified. Not a timeline card: a one-line `ComposerBanner.Row`
   docked to the composer (`ChatComposer.tsx:4649-4670`). Left: optional app
   name, then a `<code>` block with the command/file, `max-h-20`, scrollable,
   `whitespace-pre`; a `1/N` counter when several are queued
   (`ComposerPendingApprovalPanel.tsx`). Right: ghost micro-buttons, default set
   `Cancel / Decline / Always allow this session / Approve`,
   provider-overridable; Decline in destructive colour; a provider `warning`
   (the comment cites *"a prompt injection warning on 'allow always'"*) shown as
   a triangle icon plus tooltip *"so the row stays one line"*
   (`ComposerPendingApprovalActions.tsx`). Disclosure is one layer: what you see
   is the full command. Session-wide "always allow" is documented as Grok-only
   (`docs/user/permission-modes.md`). Four modes (Supervised, Auto-accept edits,
   Auto, Full access) are per-thread; **Full access is the default**.
4. **Turn shape.** Verified. A `working` row reads *"Working for 12s"*, a
   self-ticking text node (`WorkingTimer`), or *"Setting up worktree…"* with a
   shimmer. Below it a live row shows the current tool label, or *"Thinking"*
   with a brain icon when no tool is active. Context usage is a ring in the
   composer, red above 90%, tokens in a hover popover (`ContextWindowMeter.tsx`).
   Streaming prose is plain `ChatMarkdown` with `isStreaming` only disabling the
   highlight cache; no cursor. No step count, no per-turn token count in the
   thread.
5. **Thinking.** Verified, and the answer is: dropped. Adapters emit
   `reasoning_text` / `reasoning_summary_text` deltas (`ClaudeAdapter.ts:1400`,
   `CodexAdapter.ts:1562-1600`), but ingestion returns early on any delta that
   is not `assistant_text` (`ProviderRuntimeIngestion.ts:1514`). The only
   "thinking" the UI shows is the live label.
6. **Animations.** Verified, all CSS. `AnimatedHeight` transitions `height`
   200ms ease-out with a 250ms fallback; the live-activity shimmer is a masked
   `translateX` at 2.2s linear infinite, off under reduced-motion
   (`index.css:427-495`); chevrons rotate 200ms; hover reveals of
   copy/timestamp fade 200ms; the context ring 500ms. `@formkit/auto-animate` is
   a dependency but no import was found in the files read (not verified).
   `AGENTS.md` explicitly audits *"css animations causing gpu spikes"*.
7. **What it costs.** Verified from source: reasoning is invisible; no per-call
   timing; approval detail is a 5-line scroll box, so a long diff or script is
   read through a slot; failed calls are deliberately muted so a failure inside
   a collapsed *"Ran 12 commands"* is one `aria-label` and an icon swap; the
   default mode asks nothing. Density is the design: one line per call, 11px
   mono when expanded.

#### Cursor

Closed source. Sources: cursor.com/docs (agent overview, prompting, tools,
checkpoints, security, run-modes), cursor.com/changelog (1.3, 1.4, 1.6, 2.5,
3.0, 3.1, 3.4, auto-review), forum.cursor.com. `docs.cursor.com` now redirects
to `cursor.com/docs`. Nothing below is from code.

1. **Host vs agent voice.** *Not verified* in docs: no page describes styling or
   position of Cursor's own status lines against model prose. What exists: a
   *"Summarizing chat context"* status line appears in the thread while the
   model keeps working (*forum*, thread 102148); connection errors render as an
   inline message with a Request ID (*forum*, 163759). Staff note there is *"no
   way to view the summary"* after compaction (*forum*, 102148).
2. **Tool call rows.** Verified: tool activity is folded into a per-turn group
   and its verbosity is a setting. Changelog 3.4: *"customize tool call
   density… Compact shows concise results with minimal tool traces, Balanced
   includes important intermediate steps, Detailed provides near-complete
   step-by-step context"*. Changelog 1.4: compact mode *"hides tool icons,
   collapses diffs by default, and auto-hides the input when idle"*, which also
   establishes that edit diffs render inline in the thread by default. Grouping
   (*forum*, 165292): reads, searches and MCP calls collapse under an *"Explored
   (x) tools"* header even at Detailed; staff confirmed MCP calls *"do get
   folded into the 'Explored' group alongside reads and searches, at every
   density including Detailed"*; users mention *"Worked for…"* sections that
   collapse after the response. Per-row content (command/path, status,
   duration): *not verified* officially. Terminal output is shown inline in chat
   (indirectly verified: docs warn Powerlevel10k *"can interfere with the inline
   terminal output"*). Hidden or lost terminal output is a recurring *forum*
   complaint (147162).
3. **Approvals.** Verified: *"By default… terminal commands need your
   approval"*; file reads and searches do not (docs, agent/security). Run
   Modes: **Auto-review** (default since 3.6) — *"Allowlisted calls run
   immediately. Other shell commands run in the sandbox when possible"*; the
   rest go to *"a classifier subagent that decides whether to allow the call,
   try a different approach, or ask for your approval"* (changelog auto-review,
   2026-05-29). **Allowlist**: *"Actions in your allowlist run without
   approval."* **Run Everything**: every tool call runs. Auto-review is
   *"best-effort guardrails rather than a hard security boundary"* (security
   page). Controls on the prompt, *forum* staff: *"Skip, Run, and Add to
   Allowlist"* (151839). Matching is prefix-based, no wildcards (*forum*,
   102782). Layered disclosure of the command before deciding: *not verified*.
   OS notifications fire *"when shell commands require user approval"*
   (changelog 1.6).
4. **Turn shape.** Verified: a context ring — *"The context ring next to your
   prompt input shows how full the window is at a glance"*; clicking it opens a
   breakdown tray split into System prompt, Tools, Rules, Skills, MCP,
   Subagents, Summarized conversation, Conversation (docs, agent/prompting).
   Auto-compaction near the limit, `/summarize` on demand (changelog 1.6).
   Checkpoints are created *"before making significant changes"*, restore
   *"reverts files only; it does not remove messages"* (docs,
   agent/chat/checkpoints). Queued messages sit below the active task, drag to
   reorder; Enter queues, Cmd+Enter interrupts; *"Send now"* steers without
   interrupting (docs, agent/overview). Elapsed time and step counters: *not
   verified* beyond the *forum* *"Worked for…"* header.
5. **Thinking.** Verified only that thinking blocks exist and expand/collapse.
   *Forum*: collapsed by default, auto-collapse when new steps begin even if
   manually opened; staff: *"it's unfortunate that we don't respect the user
   having manually expanded"* (164278). Reasoning is dropped on compaction
   (164847).
6. **Animations.** Verified, performance-framed only: *"Large edits stream more
   smoothly now after cutting dropped frames by ~87%"* (changelog 3.1). No
   documented deliberate transition design.
7. **What it costs.** Grouping hides work: the *"Explored (x) tools"* fold
   ignores the Detailed setting and users call it a regression (*forum*,
   165292). Compaction is opaque and can loop (*forum*). Allowlist prefix
   matching is over-permissive (*forum*, 102782). Auto-review moves the decision
   to a classifier the docs themselves call not a security boundary.

#### VS Code + Copilot Chat

Read 2026-09-03 from `microsoft/vscode` `main`; the Copilot extension is now
in-tree under `extensions/copilot/` and the old `microsoft/vscode-copilot-chat`
paths 404. Source under `src/vs/workbench/contrib/chat/`; `PARTS` =
`browser/widget/chatContentParts/`.

1. **Host vs agent voice.** Verified. A response is a list of typed parts.
   `IChatProgress` (`common/chatService/chatService.ts`) is a 45-member
   discriminated union. Host-voice kinds: `progressMessage` (`shimmer?`),
   `warning`, `info`, `confirmation` (`title, message, buttons?, isUsed?`),
   `toolInvocation`, `thinking`, `systemNotification`, `undoStop`, `hook`,
   `mcpServersStarting`. The model's own words are `markdownContent` only.
   Styling: progress is a `.progress-step` row, spinner suppressed when
   shimmering; confirmation is a bordered card, `1px solid
   var(--vscode-chat-requestBorder)`, heading3 semibold title + icon, scrollable
   message, wrapping button row (`PARTS/chatConfirmationWidget.ts`);
   warning/info are banners.
2. **Tool call rows.** Verified. `chatToolInvocationPart.ts` picks a sub-part by
   state: Streaming, WaitingForConfirmation (terminal / sandbox / modified-files
   / generic), WaitingForPostApproval, or result parts. Generic row:
   `invocationMessage` while running, `pastTenseMessage` when done. Icons
   (`getProgressIcon`): check on Completed, `Codicon.error` on Failed **and
   Denied**, `circleSlash` on Cancelled, no spinner while running because
   *"progress rows convey activity via shimmer instead"*. Terminal row: verb
   Ran/Running/Skipped + `<code>` command truncated at 50 chars, a decoration
   dot whose *hover* carries exit code and duration; output is a live xterm
   mirror clamped 1–10 rows, `max-height: 300px`; actions Focus Terminal /
   Continue in Background / Show Output. Auto-expand
   (`terminalToolAutoExpand.ts`): open only if output is still arriving after
   50 ms/500 ms (*"prevent flickering for fast commands like `ls`"*),
   auto-collapse on exit 0, stay open on non-zero, remember manual toggles.
   Grouping: tool calls pin under the current thinking part, retitled
   *"Finished with N steps"* on completion; `chat.agent.thinking.collapsedTools`
   = `off | withThinking (stable default) | always (Insiders)`; completed parts
   fold into `<details class="completed-response-disclosure">`.
3. **Approvals.** Verified. Terminal: title, the command in an *editable* Monaco
   block, an AI risk badge green/orange/red with hover *"Risk assessments are
   AI-generated and may be inaccurate"*, a disclaimer for unsandboxed/network
   reasons; buttons **Allow**, **Skip**; dropdown **Enable Auto Approve…** /
   rule actions / **Configure**. Generic tool: input as minified JSON with *"See
   more"*, editable, live schema-validated; **Allow Once**, Skip; dropdown
   (`languageModelToolsConfirmationService.ts`): **Allow in this Session / Allow
   in this Workspace / Always Allow**, plus per-server and per-tool+arguments —
   three granularities × three scopes. Layered: several pending asks merge into
   one carousel, *"1/3"*, prev/next, **Allow All** / Skip All, 300 px collapsed
   (`chatToolConfirmationCarouselPart.ts`). Settings:
   `chat.tools.global.autoApprove` (`/yolo`), `chat.tools.terminal.autoApprove`
   allow/deny map matched per subcommand.
4. **Turn shape.** Verified. `ChatWorkingProgressContentPart` renders
   *"Working"* + `.chat-animated-ellipsis` (1 s, `steps(4)`), rotating phrases
   with a 1.2 s dwell, suppressed when a streaming tool or thinking part
   exists. The input border runs a "comet". Footer: elapsed time
   (`formatChatResponseElapsedTime`, 160 ms fade) and a model + credits stat
   whose hover lists input/output/cached tokens per model. Turn pills above the
   input: *"N files +ins −del"* (`chatTurnPills.ts`). Request cap
   `chat.agent.maxRequests` → confirmation *"Continue to iterate?"* with
   **Continue / Cancel**; the commonly cited default of 25 is **not verified**.
   Checkpoints: hover a request → **Restore Checkpoint**, reverts files and
   truncates history, Redo offered.
5. **Thinking.** Verified. `chatThinkingContentPart.ts`: title extracted from
   the first `**bold**` line, replaced by an LLM-generated title on completion;
   `chat.agent.thinkingStyle` = `collapsed | collapsedPreview | fixedScrolling`
   (default: 200 px pinned viewport with top/bottom fade masks); title shimmers
   `2s linear infinite`; icon `circleFilledCompact` → `checkCompact`.
6. **Animations.** Verified. One curve everywhere: `grid-template-rows 180ms
   cubic-bezier(0.2,0,0,1), opacity 140ms` for collapsibles, confirmation,
   thinking and `<details>`; shimmer 2 s; ellipsis 1 s; footer/timing fades
   140–160 ms; edit flash 350 ms; reduced-motion honoured with `transition:
   none !important`. The terminal part CSS has no transitions at all.
7. **What it costs.** Verified: checkmarks were removed by default and hidden
   behind `accessibility.chat.showCheckmarks` (#297207). Tracker complaints:
   whimsical rotating status (*"Bribing the hamster"*) mixed into real progress
   (#317024); thinking auto-collapse hides reasoning (#292119); the *"N files
   changed"* pill swallows the panel (#261081); checkpoint restore intermittently
   leaves terminal history and memory files unreverted (#262313, #316483,
   #307617). Visible in source: 45 part kinds and 28 tool sub-part files is a
   lot of surface, and the terminal row needs a 10×100 ms poll loop just to
   decide whether to open.

#### opencode v2 desktop

Canonical repo `anomalyco/opencode` (`sst/opencode` redirects), read at commit
`8a6cf2c9` (2026-09-03). Desktop is Electron 42 (`packages/desktop/README.md`);
the UI is SolidJS in `packages/app` + `packages/session-ui` + `packages/ui`. No
desktop docs page exists.

1. **Host vs agent voice.** Verified. The model's prose is a `text` part; the
   app never speaks as a message. Host notices are separate components, not
   parts: `MessageDivider`, a hairline with a muted 12px label, for compaction
   and *"Interrupted"* (`session-ui/src/components/message-part.tsx`
   L1635–1652); provider errors as `<Card variant="error">` under the turn
   (`session-turn.tsx` L528–532); retry/backoff as `SessionRetry`, an error card
   with a spinner and a live *"retrying in Ns · attempt N"* countdown.
   Permission prompts live in the composer dock, not the thread. Protocol
   chatter (`step-start`, `step-finish`, `patch`) is dropped before render
   (`event-reducer.ts` L20).
2. **Tool call rows.** Verified. `BasicTool` (`basic-tool.tsx`) is one
   collapsible row: icon, title through `TextShimmer` (shimmers while
   `pending|running`), subtitle (filename/path), up to three `key=value` args,
   chevron. Shell rows show `$ cmd` collapsed and a `<pre>` of command+stdout
   expanded (L2086–2150). Edits open into a pierre diff. No duration on rows;
   duration/model/agent are a footer under the last text part (*"Build · Model
   · 12s · Interrupted"*, L1676–1703). Failure: `ToolErrorCard`, error-variant
   card with a `circle-ban-sign` icon, tool name, first clause as subtitle, body
   + copy button when expanded. Unknown tools fall back to `GenericTool`
   (*"Called `{tool}`"*).
3. **Approvals.** Verified. `SessionPermissionDock`
   (`packages/app/src/pages/session/composer/session-permission-dock.tsx`)
   mounts above the composer inside `DockPrompt`: warning icon + *"Permission
   required"* header, an optional per-tool description, then the request's
   `patterns` as `<code>` lines. Footer: Deny (ghost) / Allow always
   (secondary) / Allow once (primary), disabled while responding. Disclosure is
   flat: patterns only. A session/directory "auto-accept" toggle exists
   (`context/permission.tsx` L161–177).
4. **Turn shape.** Verified. No step count, no elapsed timer during the turn.
   While busy: a *"Thinking"* `TextShimmer` line, plus a `TextReveal` of the
   latest reasoning heading when summaries are hidden (`session-turn.tsx`
   L420–432). Context usage is a `ProgressCircle` in the header whose tooltip
   lists cost, %, tokens (`session-context-usage.tsx`). A 5×5 dot-grid marks
   running subagents and busy tabs.
5. **Thinking.** Verified. `reasoning` parts render as muted 13px markdown,
   inline, not collapsible, gated by `showReasoningSummaries` (`message-part.tsx`
   L1759–1775).
6. **Animations.** Verified. Library: `motion` 12.34.5 (springs only) + hand
   CSS; every animated component has a `prefers-reduced-motion` block.
   - *Streaming text*: `createPacedValue` (`message-part.tsx` L252–334) drips
     deltas at 24 ms, chunk size scaled to backlog, snapping to punctuation,
     bypassed under 512 chars. Markdown patches per block through morphdom
     keyed by hash, so unchanged blocks are untouched and code blocks keep
     identity (`markdown.tsx` L589–615); shiki runs in a worker.
   - *Expand/collapse*: `animate(el, {height:"auto"|"0px"}, {type:"spring",
     visualDuration:0.35, bounce:0})` (`basic-tool.tsx` L47, L152–173). Bodies
     below the fold mount deferred over rAF, bottom first. The plain
     `Collapsible` has its slide keyframes commented out.
   - *Row appearing*: a shell subtitle springs from width 0 and un-blurs
     (`{duration:.32, ease:[0.16,1,0.3,1]}`) only if the row was first seen
     pending (`ShellSubmessage`, L94–125).
   - *Status text*: `TextShimmer` gradient sweep, 1200 ms linear, per-char 45
     ms stagger via `background-clip:text`, 220 ms opacity swap on stop.
     `ToolStatusTitle` cross-fades active→done with `cubic-bezier(.22,1,.36,1)`
     480 ms width + 240 ms blur/opacity, sharing the common prefix.
     `TextReveal` wipes via `mask-position`, 450–700 ms.
   - *Docks*: `useSpring` wraps motion's `attachSpring`, driving `max-height`
     of the todo/follow-up dock (`visualDuration 0.3`). The permission dock
     itself mounts with no transition (`session-composer-region.tsx` L46–58).
   - *Scroll*: `createAutoScroll` re-pins on ResizeObserver in the same frame,
     uses `scrollTop=` (no smooth), flips `overflow-anchor` none↔auto on user
     scroll. The timeline is `@tanstack/solid-virtual` with `anchorTo:"end"`,
     `followOnAppend`, overscan 50, a prepend anchor that corrects for up to 180
     frames (`message-timeline.tsx` L381–494).
7. **What it costs.** Verified. `message-part.tsx` is 2,642 lines with a v1/v2
   fork on `newLayout()`; a 5-spec Playwright *"timeline-stability"* suite
   exists because virtualization + streaming + springs jank (its README admits
   it cannot see compositor glitches). `packages/app/AGENTS.md` demands a
   benchmark baseline before touching timeline code. Approval discloses only
   patterns; agent narration during a turn is a single shimmer line.

### What the four agree on

Read side by side, the references converge more than they differ, and the
convergence is the useful part.

| | T3 Code | Cursor | VS Code | opencode | **Warp before this ticket** |
|---|---|---|---|---|---|
| host voice is a distinct kind | yes, `tone` on work rows | *not verified* | yes, 45-part union | yes, never a message | **no — same type as prose** |
| tool call = one collapsible row | yes | folded into *"Explored (x) tools"* | yes, state-picked sub-part | yes, `BasicTool` | **a bare label** |
| verb tense follows state | yes | — | `invocationMessage` → `pastTenseMessage` | title cross-fades active→done | no |
| per-call duration on the row | **no** | *not verified* | on hover only | **no**, turn footer | no |
| failure marker | icon swap, muted | — | error icon, Failed **and Denied** | error card | none |
| consecutive calls grouped | *"Ran 4 commands"* | *"Explored (x)"* | pinned under thinking, *"Finished with N steps"* | no | no |
| approval lives | composer dock, one line | prompt in thread | card in thread, carousel for many | composer dock | card after the output |
| approval disclosure | the command, 5-line box | *not verified* | editable command + risk badge | patterns only | four labelled lines |
| "always" control | yes (session), warned | Add to Allowlist | session / workspace / always × 3 granularities | Allow always | **none, by policy** |
| working indicator | *"Working for 12s"* | *"Worked for…"* | *"Working…"* + comet border | *"Thinking"* shimmer | none |
| context usage | ring, red > 90% | ring + breakdown tray | footer stat on hover | circle + tooltip | none |
| thinking | **dropped** | collapsed, auto-collapses | pinned 200 px viewport | inline muted, not collapsible | mapped, never arrives |
| animation curve | CSS 200 ms | perf-framed only | one `cubic-bezier(0.2,0,0,1)` 180 ms | springs, `visualDuration .35` | none |

Three things fall out of the table.

**Every reference that could be read had a distinct kind for the host's
voice, and Warp was the only one without.** That is what step 2 below built, and
it is why the ticket ordered it first: three of the four are structurally unable
to produce the 9.4 : 1 dilution, because the renderer knows which words are
whose.

**Nobody shows per-call duration on the row, and two of the four deliberately
mute failures.** T3 Code's comment — *"Ordinary tool failures stay muted"* — and
VS Code's hover-only exit code are the same judgement: a failed `ls` is not an
event. What every reference *does* put on the row is the verb, the object and a
state icon. The one row-level thing Warp should copy from VS Code is that
**Denied gets the same icon as Failed**, because on this fork a denial is the
common case and it must be visible in the row rather than only in a note.

**The approval surface is the one place the references disagree, and the
disagreement is about what this fork has frozen.** Docked one-liners (T3 Code,
opencode) work because *"Allow always"* exists to make the line short; VS Code's
card is large because it offers three scopes by three granularities. Warp offers
one selectable *yes* by policy, so it has neither reason. The layering worth
copying is VS Code's **carousel** — *"1/3"*, prev/next — for the case this fork
measured at forty-four asks, and T3 Code's discipline of **one line docked with
the full command scrollable**. Not the buttons.

### What this fork takes, and from whom

- **From all four: the host speaks in its own kind.** Built (step 2). Warp's
  rows are dimmer than the agent's, collapsed by default, with the sentence a
  person needs on the row — T3 Code's `text-secondary-label` and opencode's
  `MessageDivider` are the same choice.
- **From T3 Code and VS Code: tool rows as verb + object + state**, with the
  tense following the state and consecutive calls foldable into a count. The
  wire already carries `title`, `kind`, `rawInput`, `locations`, `status` and
  stdout; Warp renders the title. **Not** duration on the row.
- **From VS Code: Denied is drawn like Failed.** On this fork a refusal is a
  decision, not an error, so the row says which — but it gets an icon either
  way.
- **From opencode: the working indicator is a shimmer on the current tool's
  title, not a spinner**, and streaming text is paced rather than dumped. Its
  spring for expand/collapse (`visualDuration .35, bounce 0`) and VS Code's
  single 180 ms curve are the two defensible answers; pick one and use it
  everywhere. Its scroll discipline — re-pin on resize in the same frame, no
  smooth scroll, respect a user who scrolled up — is what stops a streaming turn
  from fighting the reader.
- **From Cursor and T3 Code: a context ring beside the input.** `usage_update`
  arrives on the wire with `used`, `size` and `cost` and is read for nothing.
- **From VS Code: the approval carousel for many asks.** Layering is
  *decision first, disclosure one interaction away*; the four labelled lines
  stay, behind the same chevron the notes use.
- **From nobody: an "always" control.** Every reference has one and every
  reference's own docs or forum carry the warning that comes with it (T3 Code's
  prompt-injection warning, Cursor's *"not a hard security boundary"*, VS
  Code's *"may be inaccurate"* risk badge). That is I18 and it is frozen.

What the fork already knows it wants, from its own measurements:

- **Warp's voice visually distinct from the agent's**, and collapsible.
- **Tool calls as rows with substance** — the command or path, the outcome, the
  duration — expandable to the full output that is already on the wire.
- **A turn with visible shape** — steps, progress, elapsed, context used.
- **Approvals layered**, decision first, full disclosure one interaction away,
  and the disclosure not repeated verbatim every time.
- **Thinking rendered as thinking** when an agent finally sends some.

---

## Constraints this must not break

These are not preferences. Each has a measurement or a shipped defect behind it.

1. **No permission changes.** `GOAL.md` freezes permission posture. Everything
   above is presentation; the moment a design changes what a *yes* buys, it is a
   different ticket. Abbreviating the asking note is presentation. Adding an
   "always" button is not.
2. **A surface must not claim more than the code does.** This fork's
   most-tracked defect, thirteen instances and counting. T20.2 is the live
   example: the panel listed three options and drew two buttons with nothing
   saying the third could never be selected.
3. **Warp's words must stay out of the transcript.** `strip_chrome` exists
   because an agent grepping its own history read Warp's asides as its own words.
   A new channel must keep that property, and it will be *easier* with a channel
   than with a text marker.
4. **Never emit tool calls as `Action` / `ToolCall` messages.** On these paths the
   agent has already run the tool; an `Action` is an *instruction* and Warp's
   action model would run it a second time. `acp_agent/translate.rs`'s module
   docs record three separate occasions where this hazard was written down and
   then built against anyway. The prose is the record on these paths, by design.
5. **Verify by running.** A composer change is invisible to `cargo test`. Every
   claim in this file came from a screenshot or a wire capture; keep that.
   `shot.ps1 -Process warp-oss` and `warpctrl acp probe --output-format ndjson`
   are the two instruments.

---

## Where to start

1. ~~**Study the reference apps** and write the comparison into this file.~~
   **Done 2026-09-03**, above.
2. ~~**Give Warp its own message kind**~~ **Done, `a4a5cb53a`.**
3. ~~**Abbreviate the asking note after the first ask per conversation.**~~
   **Done, `b2493ef75`.**
4. ~~**Then** tool rows~~ **Done, `146265e37`**; ~~turn shape~~ **Done,
   `03fbacb24`**; ~~layered approvals~~ **Done, `002482c13`.**

Re-measure the 9.4:1 ratio after each step. It is the number this ticket exists
to move.

---

## As built

### Step 2 — Warp's own message kind (`a4a5cb53a`)

**The channel is a field, not a variant.** `api::Message` is generated from the
`warp-proto-apis` git dependency, so the `oneof` cannot grow. It carries
`server_message_data`, which `task.proto` documents as *"an opaque payload that
the client should simply roundtrip"* and which nothing in this workspace reads —
measured, every reference sets it to the empty string. A note is an
`AgentOutput` whose payload is `warp-fork/note`; `convert_from` maps that to a
new `AIAgentOutputMessageType::WarpNote { headline, detail }`, and everything
downstream of the conversion sees a distinct kind. It had to be proto-level:
conversations persist as `api::Task` and the panel is rebuilt through the same
conversion on restore. A build that predates the tag renders the note as text,
which is what it did before — and the restored pre-tag conversation on the
measurement machine did exactly that, beside the new one.

**Wire form is headline, blank line, detail.** The panel draws the headline as a
dimmed row labelled `Warp` and the detail behind a chevron, collapsed by
default. A headline-only note draws one row and no chevron. The detail goes
through `render_text_sections` exactly when `all_text` counts it, which is the
invariant link detection depends on. `format_for_copy` writes the note back out
in wire order, so the transcript and the clipboard carry the same words and
`strip_chrome` keeps deciding on the same text — constraint 3 held without
touching the transcript.

**Where it lives**: `app/src/ai/warp_note.rs` (the type, the tag, the wire
form), `convert_from.rs` (the one arm that decides), `view_impl/output.rs`
`render_warp_note` (the row), and one arm in each of the six exhaustive matches
over the enum, the TUI's included. Both translators' `note()` now take a `Note`.

**Pinned and calibrated.** A tagged output converts to the note and the same
text untagged is the agent's; both translators tag their notes and never the
agent's words; copy-format writes headline, blank, detail. Disabling the tag
check reddened exactly the five tag-dependent tests and nothing else.

### Step 3 — the mechanics once per conversation (`b2493ef75`)

`asking_note` takes `first_ask`, answered by a process-scoped per-conversation
set shaped like `transcript`'s `TOLD`. The first ask is unchanged. A later ask
keeps the headline, the two commands with their id, the digest's source in one
clause, and what the call acts on; it drops the digest explanation, what a yes
covers, the paired-device sentence and the session-directory paragraph. A
refusal's reason is never abbreviated. **No permission changes**: the same id
and the same commands answer the same request either way.

On the test fixture (no session directory) 584 characters → 232; in the
measured session, with the directory paragraph, the second and third asks each
came back ~350 characters shorter than the first.

### The measurement, re-run

Same prompt as the 9.4 : 1 turn, verbatim. Same machine, same window size
(778 × 1396), `claude-agent-acp@0.73.0` inside the distribution,
`WARP_FORK_ACP_MODE=default`, three asks, a screenshot with each ask parked,
every ask approved from `warpctrl`. Transcript
`.warp/transcripts/9a88f765-….md`; screenshots
`C:\dev\shots\composer-after-{ask1,ask2,ask3,done}.png`.

Two numbers, because the record and the screen are now different things:

| | before (`86b763d0`) | after (`9a88f765`) |
|---|---|---|
| **record** — Warp chars : agent chars in the transcript | 2558 : 271 = **9.4 : 1** | 1825 : 326 = **5.6 : 1** |
| **drawn** — headlines only, detail folded | 2738 : 271 = **10.1 : 1** | 824 : 326 = **2.5 : 1** |
| agent's share of the screen | **9%** | **28%** |

Counted by the same classifier over both transcripts (Warp = mode note, asking
and answered notes, their detail paragraphs, tool labels; agent = everything
else). The announcement is drawn in full both times and counted at its length.
The agent's own output differs between runs (271 vs 326) because it narrates
slightly differently each time; the Warp column is the like-for-like one.

**A counting mistake worth recording**: the first pass at the *after* number
reported 2.1 : 1 in the record, which was wrong in this ticket's favour. Once
the headline was split from the how-to sentence, the how-to line no longer
started with *"The agent is waiting for permission"* and the classifier filed
368 characters of Warp's words as the agent's. The rule stands: a number that
moves in the direction you wanted is the one to re-check.

**What the screenshots show, which the numbers cannot.** With the first ask
parked, *"Step 1: writing t204-a.txt with the content A."* is on screen in the
agent's colour, directly above `Preparing file…` and a one-line folded
`Warp  The agent is waiting for permission: Write t204-a.txt ›`. That sentence
was the one the ticket opened with as never visible. With the second ask parked
the whole preceding step is visible above it — narration, tool label, ask,
answer, narration, tool label, ask. The approval card is unchanged in the
binary measured (built at `b2493ef75`, before `2c914dc98`'s description-first
card landed on `dev` from another session) and is now the largest
Warp-authored thing on the screen, which is the argument for the next step.

### Step 4a — tool rows (`146265e37`)

**Item 3 understated the defect.** It said the panel shows *the title*. It
showed the *first* title, and on `claude-agent-acp` 0.73.0 that is a
placeholder the agent corrects immediately: `tool_call` says *"Terminal"* or
*"Preparing file…"*, the real title and `rawInput` ride the next
`tool_call_update` with no status, and the completion carries content and
status but no title. `tool_update_text` showed a corrected title only on a
`Completed` update, so it never showed one. The transcript of the step 3
measurement reads *"Preparing file…"* three times and nothing else — the
panel was drawing the one line the agent had not meant anyone to read, and
this file had counted those lines as tool labels without noticing what they
said.

**The transport question above is settled by reading, not guessing.**
`UpdateTaskMessage` with a `FieldMask` is applied by `Task::upsert_message`
through `crates/field_mask`, and `Exchange::upsert_output_for_message`
re-converts the merged proto in place while the exchange streams. So a row can
be appended once and rewritten in place. The pin that matters is the path
name: the mask must say `agent_output`, the oneof member's own field name.
Naming the oneof (`message`) is **not an error** — `apply_path` skips an
unknown segment with `Ok(())` — so a wrong path would have left every row
`Running` for ever, with nothing to search for. That case is in the test.

**Built**: `ai::tool_row`, a second tag family on the same field
(`warp-fork/tool/running|done|failed|denied|interrupted`) mapped at
`convert_from` to a `ToolRow` kind, drawn in `output.rs` as a state icon, a
headline, and the description and output behind a chevron. The icons are the
history panel's own map for an exchange's status, so a row and its
conversation say "done" and "failed" the same way. Never `Message::ToolCall`;
the existing test now covers the rewrites too.

**What the row says, and where each word comes from**:

| state | headline | example |
|---|---|---|
| running | *verb-ing object* | Running `CARGO_BUILD_JOBS=8 cargo --version` |
| done | *verb-ed object* | Wrote `notes/a.txt` |
| failed | *Failed to verb object* | Failed to run `cargo test` |
| denied | *Denied: verb object* | Denied: run `rm -rf build` |
| interrupted | *Interrupted while verb-ing object* | Interrupted while running `cargo test` |

The verb is `ToolKind`'s, or `_meta.claudeCode.toolName`'s where the agent
names one — a `Write` is `ToolKind::Edit` on the wire and *"Wrote"* is the
truer word. The object is read from `rawInput` (`command`, `file_path`,
`pattern`, `url`, …) and a path is said from the session directory. When
neither is known the agent's own title is used **whole and never re-tensed**,
prefixed only for the states that need saying: re-tensing someone else's
sentence is how a row starts lying. *Denied* is a separate state because a
person reading *"Failed to run"* looks for a fault in the command and not in
their own answer; the translator learns it from the same `permission_replied`
it already logs.

**A row may not say *Running* after Warp has stopped listening.**
`end_of_turn` sweeps open rows to *interrupted* before `finished`/`failed`,
and the renderer draws a `Running` row in a settled exchange as interrupted
regardless — a turn cancelled from outside ends without the sweep (item 7),
and a restored conversation carries whatever the stream left. A spinner over a
process nobody is watching is the exact thing constraint 5 forbids.

**Deliberately not built, from the study**: no duration on the row; no
folding of consecutive calls into *"Ran N commands"* (worth doing once a turn
shows enough calls for it to matter — the measured prompt has three); no
rendered diff (the detail carries the written text in a fence, which is what
the transcript can keep); no `_meta.claudeCode.toolResponse` reading, because
the agent's `content` already carries the output formatted as it meant it
and the meta is vendor-shaped.

**Measured 2026-09-03 on the Windows build at `146265e37`**, same prompt,
same window, three asks, `claude-agent-acp@0.73.0`, `WARP_FORK_ACP_MODE=default`
(`ratio.py` v2, which puts tool rows and their detail in a third bucket; the
step 3 transcript re-run through v2 gives 2.4 : 1 drawn where v1 said 2.5):

| | after step 3 | after tool rows |
|---|---|---|
| record, (Warp + tool) : agent | 5.6 : 1 | **6.0 : 1** |
| drawn, headlines : agent | 2.4 : 1 | **2.4 : 1** |
| agent's share of what is drawn | 30% | 30% |
| what the three tool lines say | *Preparing file…* ×3 | *Writing t204-a.txt* with a spinner while the ask is parked; *Wrote t204-a.txt* with a check after |

**The number did not move and was not expected to**: a row replaces a label
one for one and is about as long. What moved is what the line *says*, and
whether it is true — the record went up by 0.4 because the transcript now
keeps each written file's text behind its row, which is the detail bucket
and a real cost the earlier number did not carry.

Verified by running, not by the tests alone: the ask-1 screenshot shows the
narration, then a spinner row *Writing t204-a.txt ›*, then Warp's folded
note, then the card; the final screenshot shows *✓ Wrote t204-c.txt ›*. So
the in-place rewrite works on the live path. Zero `UpdateTaskMessage`,
`ExchangeNotFound` or `FieldMask` errors in the app log; the event log holds
three asks, three `allowed`, three `tool_complete`. After two relaunches,
`agent read` on the restored conversation returns *Wrote* three times and
*Writing* never, so the rewrite reached the stored proto. **What that run does
not show** is the kind on restore — `agent read` prints text, and a Done
row's text is the same whether or not the tag survived — so the tag's
round-trip rests on the unit tests (`into_message`/`state_of`, and the
`convert_from` arm), not on this run.

One thing seen on the way: rows *b* and *c* sit adjacent in the transcript,
before either ask, because the agent narrated steps 2 and 3 in one sentence
and announced both writes before asking about the first. The transcript is
faithful. It is also the first live instance of the case the *"Ran N
commands"* fold is for.

### Step 4b — turn shape (`03fbacb24`)

**Look for the gate first, again.** Item 4 named three things — a working
indicator, elapsed time, a context ring — and two of them were already built
upstream and fed by nothing on the fork's path:

- **The context ring.** `agent_input_footer` draws
  `icon_for_context_window_usage(conversation.context_window_usage())`, which
  is written from the `StreamFinished`'s `conversation_usage_metadata`,
  which `acp_agent`'s `finished()` never set — while `translate.rs` dropped
  every `usage_update` with the note *"Warp's own accounting is on the
  `StreamFinished`"*. Nothing was. The last `usage_update` of the turn
  (`used`/`size`; this agent reports a 200,000 window) now rides the finish
  as `context_window_usage`, and the ring the footer has always drawn at
  *100% remaining* becomes true. A `size` of zero leaves it alone rather
  than reading *full*; calibrated by removing that guard.
- **Elapsed time.** `AIAgentExchange::time_since_start` existed with no
  caller in the workspace. It is now the number after the working label
  (*Warping… • 12s*), in the non-shimmering slot the summarization timer
  already uses so the shimmer does not reset each second, and the status bar
  ticks once a second while the exchange streams — the summarization timer's
  own shape, stopping itself the first time it wakes to nothing streaming.
  Gated on `fork::is_active()`.
- **The working indicator** upstream has: the shimmering label, and since
  step 4a a spinner on the running tool row.

**Measured 2026-09-03 on the Windows build at `03fbacb24`**, same prompt,
same driver, three asks. The ratio this time is **8.0 : 1 record, 3.2 : 1
drawn** — and Warp's side is byte-identical to the step 4a run (1,774 chars
of notes, 168 of tool rows). The agent narrated 244 characters instead of
324. **The denominator is the agent's mood on the day**, which is worth
writing down beside every one of these numbers: a ±25% swing in the agent's
prose moves the ratio more than any single change in this file has, so the
number is for comparing *Warp's* side across runs, and the columns above
should be read as Warp's chars first.

What the run shows:

- **Elapsed is on screen**: the ask-1 screenshot's status row reads
  *• 10s* beside the working label. In that window (657 px wide, against
  778 px for the step 4a run) the label itself clipped to *Wa* — the row is
  two equal `Shrinkable`s, upstream's own layout for its summarization
  timer, so a narrow window costs the label and the suffix alike. Left as
  is, and named: the fix is a shrink weight in upstream's
  `render_warping_indicator_base`, which deserves its own measurement rather
  than a drive-by.
- **The ring is fed**: the persisted conversation carries
  `context_window_usage: 0.062732` where the step 4a conversation carries
  `0.0` — read from `agent_conversations` in the app's SQLite, not inferred.
  The footer re-reads it on `UpdatedConversationStatus`, which fires after
  the controller has applied the finish's usage. **Not shown**: the icon
  step. At 6% used the icon is `ContextRemaining90` against `…100`, and the
  two SVGs differ by four path elements in a 16 px glyph — a 4× crop of both
  final screenshots looks the same, so whether the *drawing* changed is
  unsettled here and would need a fuller context to tell. The data path is
  settled.
- Zero errors of any relevant kind in the app log; files written; three
  asks answered.

### Step 4c — layered approvals (`002482c13`)

**Decision first, disclosure one interaction away, nothing dropped.** The card
drew four labelled lines and then the buttons, and the buttons were the
smallest thing on it (item 6). `acp_approval::layered` now splits a parked
request into what is always drawn — who is asking, the agent's own title as
the headline, the sentence it wrote for a person (*it says*), and the reason
when there is no yes — and what sits behind a *details* toggle: the verbatim
call, the rest of the payload, *acts on*, *offered*. The detail lines are the
same lines in the same order; a test pins that the description is never
behind the toggle and the no-yes reason is never behind it either.

**The toggle is keyed to the approval id, like arming.** A request that
replaces the one a person opened arrives closed, because what they opened was
a different question — the stale-answer hazard this file's card already
argues at length, applied to disclosure.

**No carousel, on purpose.** VS Code's carousel lets a person answer any of
several asks; this card answers the oldest first and says *N more waiting*
beside the agent's name. The order rule (*an agent that asks twice should not
have its second question jump the first*) is the reason there is no
navigation, and `claude-agent-acp` asks serially anyway — the parallel
announcement seen in step 4a still arrived as two asks in sequence.

**What is unchanged, so nobody reads this as I18**: the buttons, what a yes
buys, which options are selectable, and every word on the card. The
always-visible half is the agent's own title and description; Warp adds a
count and a chevron.

**Measured 2026-09-03 on the Windows build at `002482c13`**, same prompt,
same driver, with a pause at the first ask so the toggle could be clicked
(`click.ps1`, window-relative, no cursor taken).

- **Closed**, with the ask parked: *wsl.exe asks* / *Write t204-a.txt* /
  *[Yes, once] [No] › details* — three lines where the card was nine, and
  above it the agent's narration (*"Step 1: writing t204-a.txt with A."*), the
  spinner row, and Warp's folded note, all on screen at once in a 657 px
  window. The *it says* line is absent because a `Write` carries no
  description; a shell call would show it.
- **Open**, after one click: the same four lines the card drew before —
  *the call*, *also content: A*, *acts on*, *offered* with its
  *(Warp never selects this)* note — below the buttons, in order. Nothing
  missing, and the buttons stayed where they were.
- **Ratio**: Warp's side byte-identical to the two previous runs
  (1,774 chars of notes, 168 of tool rows). The agent wrote 424 characters
  this time, so the number reads 4.6 : 1 record, 1.8 : 1 drawn, agent share
  36% — which is the denominator moving, not Warp. The card is not in the
  transcript, so this step's gain is on screen only: six fewer lines under
  the buttons every time an ask is parked.
- Zero relevant errors in the app log; three files written; three asks
  answered; the synthetic click left a stray tooltip on the footer, which is
  the pointer's doing and not the card's.

### Step 4c, corrected — the call belongs on the closed card (`087764d89`)

**Found by reviewing 4c rather than by using it**, twice independently, and
the two reviews landed on the same seam. The layering put the agent's title
and the agent's `description` in the always-visible half and the verbatim
command — and a write's `content` — behind the toggle. So the split ran
exactly along *authored by the agent* / *checkable against the agent*, with
the checkable half hidden: everything a person read before pressing *Yes*
was the agent's own account of itself.

**Measured against the corpus rather than the run it was seen in**, which is
the correction `57f0e866a` asked for: of the 52 asks in
`.fork/classifier/eval-set.jsonl`, **36 carry a `command` and 15 a
`content`** — so on **51 of 52** the operative fact was one click away. The
single exception is a `read`. That is a claim about what the payloads
contain, which is what that corpus is sound for; it says nothing about how
the asks were answered.

**And it falsified `acp_approval.rs`'s own header**, which justifies offering
the single-shot yes on the grounds that *"this surface shows the agent, the
title, the verbatim tool input…"*. That paragraph is the argument for the
button existing. The fourteenth instance of this fork's most-tracked defect,
and the first to land on the file that states the consent rule — written
carefully, true when written, falsified by a change one screen below it
hours later. The header now says **"shows" means without a further
interaction**, and names the one exception it tolerates: a payload's tail
past `CONTENT_PREVIEW_CHARS` (400), with the closed card saying how much is
under *details* rather than trailing off.

**Nothing about what a yes buys moved**: `choose`, `is_selectable`, the
digest and every option's wording are untouched, verified by both reviews.
The closed card is four lines where 4c made it three and the original nine.

Four more from the same review, each calibrated by breaking it:

- **The card headlined `parked.title` unfiltered** while the sibling commit's
  `usable_title` refuses the placeholders `claude-agent-acp` sends —
  *"Terminal"*, *"Preparing file…"*. Two surfaces disagreeing about one
  string, and the credulous one was the one a person answers. The rule is now
  `tool_row::is_placeholder_title`, shared by both; a refused title falls back
  to the call. **Unmeasured**: whether a permission request ever carries a
  placeholder. The measured write ask did not, and no instrument records the
  title at the ask (below).
- **The renderer re-tensed a headline it did not write.** Demotion rewrote any
  headline whose first word ended in *"ing"*, so an agent title used whole
  became *"Interrupted while ping the host"* — the hazard `translate.rs` names
  where it picks that fallback, repeated one file over by the half that cannot
  tell Warp's verbs from the agent's sentence. Prefixed now, never re-tensed,
  in `tool_row::demoted_headline` so it is testable at all.
- **`end_of_turn` swept a denied row to Interrupted**, pointing at the turn
  ending when the reason was the person's answer. Does not fire on
  `claude-agent-acp`, which sends `failed` after a rejection; `opencode` is
  unmeasured.
- **Five stale doc comments**, each true when written: the `FieldMask` path
  *"nothing in this repo uses"* (`rewrite()` uses it — in the commit that
  declined it); the same claim in `translate.rs`; *"the notification stream has
  no `rawInput`"* (`row_announced` reads it); cost *"which this path does not
  have"* (`claude-agent-acp` sends `total_cost_usd`; the omission is deliberate
  and now says so); and `tool_update_text`, which no longer exists, cited in
  `translate_tests.rs` and in `CLAUDE.md`.

**Measured 2026-09-03 on the Windows build at `299ff4af2`**, same prompt,
same driver, the toggle clicked at the first ask (`click.ps1`):

- **Closed**: *wsl.exe asks* / *Write t204-a.txt* / *the call
  /home/effatha/git/warp/t204-a.txt* / *content A* / *[Yes, once] [No] ›
  details*. Five lines: the decision, the path the bytes go to, and the bytes,
  all before the buttons — which is what the module header promises and what
  4c had put one click away. Above it the agent's narration, the spinner row
  and Warp's folded note are all on screen at once.
- **Open**: *acts on* and *offered* (with its *Warp never selects this* note)
  under the buttons. Nothing else was left to disclose for a one-character
  write; a long `content` would put its tail here with the closed card saying
  how much.
- **The status row survived this window**: *Warping… • 8s* and, later,
  *• 36s*, label intact. The run at `03fbacb24` clipped it to *Wa* in a 657 px
  window; this window was wider, so the shrink-weight item below stands as
  written and is not contradicted.
- **Ratio**: Warp's side byte-identical for the fourth run running (1,774
  chars of notes, 168 of tool rows); the agent wrote 475 characters this time,
  so 4.1 : 1 record, 1.6 : 1 drawn, agent share 38% — the denominator again.
  Zero relevant errors in the app log; three files written; three asks
  answered.

**The paragraph that stood here until this measurement said the headline's
fidelity could not be checked from any corpus, because nothing records the
title at the ask. That was false, and its author withdrew it the same
evening**: `ask_summary` (`translate.rs`) writes the title into every
`permission_request` line's `summary` as *approval `<id>` · `<title>`*, and it
is there in the real logs. The field list was read and the summary's contents
were not — a doc claim about an instrument, made without running the
instrument. So no field needs adding, and the placeholder question is
answerable from disk:

| recorded asks on this machine | with a placeholder title |
|---|---|
| **64** across 27 event-log files (`edit` 27, `execute` 36, `read` 1) | **0** |

No *Terminal*, no *Preparing file…*, nothing ending in an ellipsis. And for
every `execute` ask the title at the ask is the agent's **description**
(*"Compare local HEAD to Windows checkout HEAD"*), not the command — so the
permission request carries the corrected title from the start, and the
placeholder fallback `is_placeholder_title` guards against is, on this agent,
a property of the notification stream and not of the ask. The guard costs
nothing and stays; it is not doing anything measured.

**Left for a next session, in the order they are worth doing**: the
transcript announcement and the mode note shortened (below); the *"Ran N
commands"* fold once a turn shows enough rows; the status row's shrink
weights so the label survives a narrow window; a diff view behind an edit
row; and item 7, the cancelled turn's last sentence, which now also leaves a
tool row that the renderer demotes but the transport never closes.

**What is still drawn in full and should not be**: the transcript announcement
(~230 characters, once per conversation) and the mode note's headline (~200).
Both are headline-only because the announcement must stay a single line for
`strip_chrome`, and the mode headline carries the agent's own description of
the mode. Neither repeats, so neither moved the number; both are candidates for
a shorter headline with the sentence behind the chevron, once the transcript
decides by kind rather than by marker.
