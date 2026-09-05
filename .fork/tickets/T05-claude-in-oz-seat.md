> Ticket T5, split out of `.fork/TASKS.md` on 2026-09-05. History; read the section you came for.

## T5 — Claude in Oz's seat (the spike)

Making Claude the Warp Agent proper, not a CLI harness in a pane. This is the
genuinely hard one: the 70-method `AIClient` trait plus the SSE agent-event
stream.

- [x] **T5.1** Determine the true minimum viable `AIClient` subset
- [x] **T5.2** Map the SSE agent-event protocol
- [x] **T5.3** Decide: implement the trait, or shim at the transport layer
- [x] **T5.4** Prototype behind a fork flag, default off
- [x] **T5.6** Find out what cancelled a turn nobody cancelled. Somebody did:
      a person pressed ctrl-c, meaning to copy the agent's answer they had
      just selected with the mouse. Warp does not copy an AI block's selection
      on ctrl-c — on any platform — so it cancelled the turn instead. Both
      halves of the premise were wrong, the log was not empty, and the fix is
      two lines and a predicate. See "T5.6 — the mystery was the user, again"
      below.

### T5.1 — the premise was wrong, and that is the finding

The board says "the 70-method `AIClient` trait plus the SSE agent-event
stream", as though the trait were the obstacle. **`AIClient` is not on the
agent conversation path at all.** Nothing in the trait is required to hold a
conversation, so the minimum viable subset is *empty*.

The conversation goes through a different door entirely:

    app/src/ai/blocklist/controller/response_stream.rs  spawn_generate
      -> ai::agent::api::generate_multi_agent_output(server_api, params, cancel)
         -> warp_multi_agent_client::generate_multi_agent_output(BaseClient, Request)

`warp_multi_agent_client` takes a `BaseClient` — the HTTP/auth client — not an
`AIClient`. The trait's 70 methods are conversation *metadata* (list, rename,
fork, delete), agent-definition CRUD, memory stores, ambient/cloud tasks,
artifacts, and usage reporting. Every one is beside the conversation, not in
it.

Three things do call `AIClient` near the agent, and none blocks a turn:

| Method | Caller | What breaks without it |
| --- | --- | --- |
| `get_feature_model_choices` | `ai::llms` | the model picker falls back to `ModelsByFeature::default()` |
| `get_ai_credit_availability`, `get_request_limit_info` | `ai::request_usage_model` | the usage readout is blank |
| `list_ai_conversation_metadata` | `history_model`, after a stream | titles in the conversation list |

One method *is* load-bearing, and for the path you would least expect:
`create_agent_task`. `pane_group::pane::local_harness_launch` calls it to mint
a run id before launching a **local** Claude child pane. So upstream's "local"
harness still needs an account to start. That is the sharpest single fact in
T5: the existing local-harness feature is not actually account-free.

### T5.2 — the protocol is a mutation log, not a token stream

`POST {server_root}/ai/multi-agent`, protobuf body, `text/event-stream`
response, each `data:` line a **base64url-encoded protobuf `ResponseEvent`**
(`crates/warp_multi_agent_client/src/lib.rs`, 163 lines — the whole transport).

Three event types:

    StreamInit      conversation_id, request_id, run_id   (exactly once, first)
    ClientActions   repeated ClientAction
    StreamFinished  Done | QuotaLimit | ContextWindowExceeded | InternalError |
                    InvalidApiKey | ... + token usage    (exactly once, last)

The surprise is `ClientAction`. These are not chunks of text — they are remote
mutations against a store the *client* owns: `CreateTask`,
`AddMessagesToTask`, `UpdateTaskMessage` with a `FieldMask`,
`AppendToMessageContent` with a `FieldMask`, `BeginTransaction` /
`CommitTransaction` / `RollbackTransaction`, `StartNewConversation`,
`MoveMessagesToNewTask`. Applied by
`BlocklistAIHistoryModel::apply_client_actions`.

And the request carries `TaskContext { tasks }` — **the client's entire task
list, every turn**. So the server is not the keeper of the conversation; the
client is, and it re-presents the whole thing each time. That single fact is
what makes a local implementation possible: there is nothing to recover from a
server, because the server never held it.

A `Message` is one of 22 kinds — `UserQuery`, `AgentOutput`, `AgentReasoning`,
`ToolCall` (39 tools), `ToolCallResult`, `UpdateTodos`, `WebSearch`,
`ArtifactEvent`, and so on. Tool *results* return as request `Input`s, not as
a separate channel: the client executes, then replays.

The minimum well-formed stream, confirmed against upstream's own synthesizer
in `terminal::shared_session::replay_agent_conversations`:

    StreamInit
    ClientActions[ CreateTask { task, messages: [] } ]      (first turn only)
    ClientActions[ AddMessagesToTask { task_id, messages } ] (repeat)
    StreamFinished { Done }

A stream that ends without `StreamFinished` is turned into `UnexpectedEof` and
retried three times.

### T5.3 — shim at the transport layer, and the layer is one function

    ai::agent::api::generate_multi_agent_output(
        server_api: Arc<ServerApi>,
        params: RequestParams,
        cancellation_rx: oneshot::Receiver<()>,
    ) -> Result<ResponseStream, ConvertToAPITypeError>

In: the whole conversation plus what is new. Out:
`Stream<Item = Result<ResponseEvent, Arc<AIApiError>>>`. Everything the agent
surface does — blocks, diffs, todos, history, cost — hangs off this one call,
and nothing above it can tell whether the events came off a socket or a pipe.

So implementing the trait was never the choice. One `if` at the top of that
function is the entire integration.

### T5.4 as built

`app/src/ai/local_agent/` — a local implementation of that one function,
answering from the `claude` CLI. `fork::local_agent_enabled()`, and this one is
**default off**, unlike every other predicate in `fork.rs`: the others enlarge
what works, this one substitutes for something that already does. Opt in with
`WARP_FORK_LOCAL_AGENT=1`.

`local_agent::handles` claims only a plain `UserQuery`. Passive suggestions,
resume, code review and project init keep going upstream — they have
server-side behaviour this does not reproduce, and answering them locally
would be worse than not answering.

Session continuity needed no new state. `StreamInit.conversation_id` is stored
by the client as the conversation's server token and handed back as
`params.conversation_token` next turn, so reporting Claude's session id there
makes Warp's own round-tripping the session store: `--session-id <uuid>` on the
first turn, `--resume <uuid>` after. The id is read from Claude's `init` event
rather than reused from the spawn arguments, so that if `--resume` misses, the
token follows the session that actually exists.

**The mistake worth naming: never emit `ToolCall`.** A `ToolCall` message is an
*instruction* — Warp's action model executes it and returns a result. Claude
has already run the tool. Emitting one would run it a second time: a second
`rm`, a second push. Tool activity is therefore reported as `AgentOutput`
text. There is a test named after the failure it prevents.

13 tests over the translation layer, every fixture line copied from real
`claude --print --output-format stream-json --verbose` output rather than
invented — otherwise the test only checks that the code reads my guess.

What the spike does not do, and the shape of the next step: Claude runs its own
tools, so Warp's diff review and command approval do not participate. Wiring
Warp's own execution back in means `--input-format stream-json` so tool results
can be fed back mid-turn, and at that point the `ToolCall` messages become
correct rather than dangerous. Also absent: model selection, attachments, MCP
context.

#### Verified on Windows, 2026-08-19

In the real agent panel, in a **logged-out** client — the left panel still says
"Sign in to access Agent conversations" in both screenshots:

    New Agent Tab  ->  "New Warp Agent conversation"
    prompt         ->  "In one short sentence: what is the capital of France?"
    child process  ->  claude.exe (27800), parent warp-oss.exe (32260)
    rendered       ->  "Paris is the capital of France."

    follow-up      ->  "What did I ask you in my previous message? Quote it."
    rendered       ->  You asked: "In one short sentence: what is the capital
                        of France?"

The second turn is the one that matters. It proves the conversation token made
the full round trip — Claude's session id into `StreamInit`, stored by Warp as
the conversation's server token, handed back as `params.conversation_token`,
out again as `--resume` — through Warp's own storage rather than through
anything this fork keeps. There is no session state in `local_agent`, and the
agent still remembered.

Also confirmed by the same run: an account-free Warp *can* hold an agent
conversation. Upstream it cannot, and not because of a gate — because every
path leads to `{server}/ai/multi-agent`, which needs a bearer token.

**How the GUI was driven, since none of it is obvious.** No keyboard focus is
available from a background process on either platform, so this had to be done
without one:

| Mechanism | Works | Note |
| --- | --- | --- |
| `PostMessage` WM_KEYDOWN/UP | yes | plain keys reach Warp unfocused |
| the same, with Ctrl/Shift held | **no** | posted messages don't set the thread key state, so `Ctrl+Shift+Enter` arrived as a bare Enter and ran the prompt as a shell command |
| `PostMessage` WM_CHAR | **no** | typed characters never reached the editor |
| `warpctrl input replace` | yes | but it does not run the input classifier |
| command palette + Enter | yes | `warpctrl surface command-palette open --query ...` seeds it and Enter invokes the top entry |

So the way into agent mode without a modifier is the palette: seed it with
`New Agent Tab`, press Enter, then `input replace` and Enter again. Two facts
made that necessary — natural-language auto-detection is **off by default**
(`agents.warp_agent.input.ai_auto_detection_enabled`, default `false`), and
`input replace` sets the buffer without classifying it, so the prompt stays a
shell command however it is phrased.

WSLg is worse: `XGetInputFocus` returns `None` and `XSetInputFocus` does not
stick, because the RAIL window is not foreground on the Windows desktop and
Xwayland has no keyboard focus to give. Clicks work there, keys do not. That
is the WSLg counterpart of the Windows foreground lock, and it is why this was
verified on Windows.

> **Corrected 2026-08-21.** The conclusion holds — keys still do not arrive —
> but two of the three facts above do not. `XGetInputFocus` returns the **root
> window** (`0x438`), not `None`; and `XSetInputFocus` on the Warp toplevel
> **does** stick, confirmed by reading it back. The remaining gap is one layer
> higher than X focus. See "The WSLg input wall is narrower than T5.4 recorded"
> under T8, which also records that window-targeted screenshots work.

### T5.5 — the sign-in gate over a history that was already here

Spotted by the user in the verification screenshots above: the left panel said
"Sign in to access Agent conversations / Create an account and enable AI to
access your conversation history", sitting next to a working local conversation
the whole time.

That sentence was true while the only agent was Warp's, because the history was
Warp's. T5 made it false. Conversations are written to the local database and
read back at startup, and `AgentConversationsModel::unfiltered_entries` ends
with a loop over `get_local_conversations_metadata` that touches no server. The
two auth-dependent paths in that model are the cloud half — pulling ambient
tasks and filling the creator filter — and neither is the list.

The fix is one call site, the same shape as `is_warp_drive_available` in T4.2:
route the anonymity check in `ToolPanelView::availability` through
`fork::is_anonymous_for_ui`. The second branch already passes, because
`is_any_ai_enabled` carries the account bypass.

Worth knowing: the gate was not only cosmetic. `on_conversation_list_view_
visibility_changed` calls `register_view_open` only when the panel is
`Available`, so the list was never registered as a data consumer either. And
opening it account-free costs no network traffic — the cloud fetch early-returns
without a user id, leaving the load state at `WaitingForCloud`, and `can_poll`
is false for that state.

#### And two bugs the unlocked panel exposed

Both mine, both the same omission, and neither was caught by any test I had
written — the local agent wrote the agent's half of the transcript and nothing
else.

    Untitled conversation
    C:\dev\warp                                    58 years ago

**No user turn.** Upstream the *server* echoes the query back as a `UserQuery`
message. Live it is inert — `convert_from` maps it to
`NoClientRepresentation`, because the input already drew the prompt — so it
looks redundant. It is not: on the way back out of the database
`convert_conversation` turns it into the exchange's `AIAgentInput::UserQuery`.
Without it a restored conversation has the answers and not the questions, and
no `initial_query` for the title to fall back to.

**1970.** The same message, unstamped. A restored exchange's `start_time` comes
from that query's context time, then from any message timestamp, and then from
`unwrap_or_default()` — which for a `DateTime` is the Unix epoch. Every message
now carries the time the turn started, one time per turn because one turn is
one exchange.

`Task.description` is now set from the prompt too, since
`AIConversation::title` reads it before falling back to the initial query. Cut
by character rather than byte: `String::truncate` inside a glyph panics, and a
pasted prompt is exactly where one turns up.

Conversations recorded before this keep their empty title and their 1970 —
nothing rewrites history rows, and the fix is in what gets written.

#### Verified on Windows, 2026-08-19

Signed out, panel open, one new conversation and one restart:

    ACTIVE  Name three colours, comma...   C:\dev\warp    Just now
    PAST    Untitled conversation          C:\dev\warp    56 years ago

The second line is the conversation recorded before the fix, left exactly as
it was — which is the clearest statement of what the fix does and does not do.

Then closed with `CloseMainWindow` and relaunched:

    PAST    Name three colours, comma...   C:\dev\warp    1 min ago

    /agent Name three colours, comma separated, nothing else.
           Red, green, blue

Both halves. Before the fix a restored conversation showed only the answers.

### T5.6 — the mystery was the user, again

The task was written as "find out what cancelled a turn nobody cancelled",
with two supporting claims and a named suspect. All three were wrong, and the
way they were wrong is the useful part.

**The evidence was on disk the whole time.** `~/.local/state/warp-oss/` keeps
five rotated logs, and `warp-oss.log.old.0` is the instance that ran the failed
T7.2 turn. Reading it end to end instead of grepping it for "cancel":

    08:38:55   turn starts
    08:40:04   AIBlockAction::SelectText
    08:40:04   AIBlockAction::SelectText
               ... 205 of them, over four seconds
    08:40:12   EditorAction::CtrlC          <- the turn stops here
    08:40:27   BlockListContextMenu(RichContentTextRightClick { .. })
    08:40:28   ContextMenu(CopySelectedText)
    08:40:33   the poll that reported `cancelled`

Two hundred and five `SelectText` actions in four seconds is a mouse dragging
across the agent's output. Then ctrl-c. Then — fifteen seconds later, and this
is the line that settles it — right-click, context menu, **Copy selected
text**. That is what somebody does when ctrl-c did not copy.

So the turn was cancelled, honestly, with `ManuallyCancelled`, by the user. It
reported exactly what happened. The bug is upstream of the report: **ctrl-c
over selected agent output cancels the turn instead of copying it.**

This is the second time in this fork that an unexplained mid-run stop turned
out to be a person at the keyboard (`a3065d993`, "the mid-sweep exit was a
person clicking a button"). Both times the tempting explanation was a race in
code I had just written. Both times it was somebody using the app.

#### Why ctrl-c does not copy an agent's answer

`TerminalView::ctrl_c` asks "is anything selected?" through
`model.block_list().selection()` — the point-based selection. An AI block does
not use it. `set_rich_content_selection` records the selection in
`rich_content_selections` **and sets `self.selection = None`**, so the field
ctrl-c consults reads empty *precisely* when the user has selected agent text.
Not merely unset — actively cleared, by the code that knows about the
selection.

The consequence is worse than a missing feature. There is a `#[cfg(windows)]`
arm of `ctrl_c_internal` whose comment reads "Windows users expect ctrl-c to
copy if there is selected text" — and it is gated on the same wrong field, so
it never fires for agent output either. On Linux there is no copy branch at
all: the selection is cleared and the turn is cancelled.

The fix adds `BlockList::has_rich_content_selection()` and, in `ctrl_c`, one
early branch: copy, clear, return. Unconditional rather than `#[cfg(windows)]`,
because the Linux reading of ctrl-c is "interrupt the foreground process" and
in agent view there is no foreground process — the only thing ctrl-c can do to
a streaming turn is destroy it.

Clearing afterwards is load-bearing, not tidiness. A second ctrl-c has to still
stop the agent, and `clear_selections_when_shell_mode_without_focusing_input`
is a no-op while `AgentView` is enabled, so nothing else would drop the
selection and the keyboard could never reach Stop again. Copy, then stop, is
also what the Windows arm already does.

#### And the log was not empty — the wrong half of it was

`try_cancel_streams_for_conversation` logs the reason *and a full backtrace*.
`try_cancel_stream` — same file, twenty lines up — logged nothing. The plural
one is the conversation-wide path; the singular one is what
`status_bar::cancel_active_request_or_action` calls, and grepping for callers
says that is the **only** caller. So:

> The one cancellation a person can actually cause was the only one that left
> no trace.

Which is why the turn read as unexplained, and why the answer had to be
reconstructed from `dispatching typed action` lines that happen to record every
keystroke and are not a cancellation log. That is now one `log::info!`,
mirroring its sibling.

#### What was verified, and how

Reproduced as a unit test, which is the part that matters: with the fix
removed, `ctrl_c_over_selected_agent_output_copies_then_stops_on_the_second_
press` fails with `left: Some(Cancelled), right: Some(InProgress)` — the T7.2
failure, exactly, from a keystroke. The test drives the whole path
(`handle_action(&TerminalAction::CtrlC)`) against a live agent-monitored
command, so cancellation is genuinely reachable; without that it would pass for
want of anything to cancel, which is the failure mode a regression test exists
to avoid. It also asserts the trap directly — `selection().is_none()` *and*
`has_rich_content_selection()` at the same moment — and that the second press
still cancels and still writes `ETX` to the pty.

Separately, the T7.2 prompt was re-run under the same conditions on a scratch
`XDG_STATE_HOME`, and the turn finished `success`. That is the control: the
local agent does not stop on its own. Nothing in `app/src/ai/local_agent/` was
implicated or changed.

The `log::info!` was not exercised live: no `warpctrl` action reaches the
status-bar path (`agent cancel` goes through the conversation-wide sibling that
already logged), and driving ctrl-c through the GUI needs keystroke synthesis,
which does not work under WSLg.

#### Three claims retracted

| Claim in the task | Actually |
|:--|:--|
| "nobody cancelled it" | the user did, with ctrl-c, trying to copy |
| "an empty log" | the log had the keystroke; it lacked the cancellation |
| "`Cancelled` requires a real `StreamCancellation`" | true, and it was one — `ManuallyCancelled`, from the status bar |

The named suspect, `UserCommandExecuted`, was wrong. It was reasoned from the
enum ("which reason *could* fire unprompted") rather than from the log, and
reasoning about which arm of a match is plausible is not evidence. T1.7's rule
holds: run it, or in this case, read what it already wrote down.

#### Two things found and left alone

Both real, both out of scope, both untouched:

* **Restore relabels incomplete exchanges as cancelled.** `create_exchange_
  from_messages` reconstructs an exchange with no outputs — or a trailing tool
  call with no result — as `Cancelled { reason: ManuallyCancelled }`, and
  `derive_status_from_root_task` turns that into `ConversationStatus::
  Cancelled`. The reason is not persisted (`AgentConversationData` has no field
  for it), so it is hardcoded. Any turn interrupted by a crash or a quit reads
  back after restart as "you cancelled this". Ruled out as the T7.2 cause: that
  read was live, 41 seconds before the instance exited.
* **`cancel_conversation_progress` can stamp `Cancelled` with no stream and no
  log**, via the action model, when the conversation is not in
  `TransientError`. Not reached here, but it is the other silent path.

