# The composer — a top-of-class agent surface

**Filed 2026-09-03, as its own file rather than a board entry, because it is not
a defect and it is not small.** The board tracks things that are wrong. This is
a thing that is *not good enough*, on the surface a person looks at all day, and
the fork is currently failing it — not by a little.

**Status: nothing built. This is the ticket, not the plan.**

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

### 1. Warp outweighs the agent roughly 10:1 during any turn with asks

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

### 3. Tool calls render as bare labels

`Terminal`. `Read File`. `Preparing file…`. No command, no path, no result, no
duration, no status marker on failure. The information exists — the transcript
carries `wc -l CLAUDE.md` and the file path — and the panel shows a noun.

Compare what the ACP stream actually delivers, captured with `acp probe`:
`tool_call` with `title`, `kind`, `rawInput`; `tool_call_update` with `content`,
`locations`, a diff for edits, `status`, and `_meta.claudeCode.toolResponse`
carrying stdout and stderr. Warp renders the title.

### 4. Nothing shows a turn's shape

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

### 6. The approval card is a wall, and its controls are the smallest thing on it

Four labelled lines of disclosure, then `Yes, once` / `No`. The disclosure is
good and should not be deleted. It should be *layered* — the decision visible,
the reasoning available.

### 7. A cancelled turn plausibly loses its last sentence

`translate.rs` buffers text and flushes on the next non-text update, on turn end,
or on the failure path. `mod.rs`'s `take_until` drops the driver future on
cancellation and nothing flushes `pending` there. **Read, not run.** Worth a test
before it is worth a fix.

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

1. **Study the reference apps** and write the comparison into this file.
2. **Give Warp its own message kind**, so its voice is separable at the
   renderer. This is the enabling change and nothing good is possible before it.
3. **Abbreviate the asking note after the first ask per conversation.** Smallest
   change with the largest measured effect: ~500 characters per ask, no posture
   change.
4. **Then** tool rows, turn shape, and layered approvals — in that order, because
   each is worth more once Warp's chrome is out of the way.

Re-measure the 9.4:1 ratio after each step. It is the number this ticket exists
to move.
