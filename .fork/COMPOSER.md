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

### 2. Tool calls render as bare labels

`Terminal`. `Read File`. `Preparing file…`. No command, no path, no result, no
duration, no status marker on failure. The information exists — the transcript
carries `wc -l CLAUDE.md` and the file path — and the panel shows a noun.

Compare what the ACP stream actually delivers, captured with `acp probe`:
`tool_call` with `title`, `kind`, `rawInput`; `tool_call_update` with `content`,
`locations`, a diff for edits, `status`, and `_meta.claudeCode.toolResponse`
carrying stdout and stderr. Warp renders the title.

### 3. Nothing shows a turn's shape

No progress, no step count, no elapsed time, no token or context indication —
`usage_update` arrives on the wire (`used`, `size`, and a `cost` object) and is
read for nothing on the display path. A long turn is an unmoving wall of text.

### 4. Thinking is absent, and that is currently not Warp's fault

`translate.rs:345` maps `AgentThoughtChunk` to `AgentReasoning`, documented
*"rendered as thinking, not as output"*. Measured twice at 0.73.0, once under an
explicit ultrathink prompt: **zero** thought chunks, `reasoningOutputTokens: 0`.
So there is nothing to render today. **Do not treat that as settled** — it is a
fact about one agent at one version, and the moment an agent sends thinking the
composer needs somewhere to put it that is not the output stream.

### 5. The approval card is a wall, and its controls are the smallest thing on it

Four labelled lines of disclosure, then `Yes, once` / `No`. The disclosure is
good and should not be deleted. It should be *layered* — the decision visible,
the reasoning available.

### 6. A cancelled turn plausibly loses its last sentence

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
**opencode v2 desktop** (specifically its animations). None of this has been
looked at yet. That study is the first task, not an afterthought — and the
output of it should be a written comparison in this file, naming what each does
well and what it costs, before any pixel is moved.

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
