> Idea I9, split out of `.fork/IDEAS.md` on 2026-09-05. History; read the section you came for.

# I9 — The context is already yours

> *"A version of `vscode-prompt-tsx`. Agents assign a relevance/priority to
> transcript so low-priority/unrelated queries and results can be pruned. Also
> 'context masking' — something Manus is reported to do — along with an agent
> scratchpad, and having the agent repeat the current task objective and/or user
> query to a scratchpad periodically to keep the context fresh for long-horizon
> tasks. I'm honestly not sure exactly how this affects caching."*

This is the most interesting idea on the page and the one I most want to slow
down.

## The enabling fact, from T5.2

> the request carries `TaskContext { tasks }` — **the client's entire task list,
> every turn**. So the server is not the keeper of the conversation; the client
> is, and it re-presents the whole thing each time.

That changes what this feature *is*. In most applications, context management
means intercepting a prompt-assembly pipeline you do not own. Here, the client
already holds the whole transcript and hands it over whole, every turn, through
**one function** — `generate_multi_agent_output`
(`app/src/ai/agent/api.rs`), the same single choke point T5.3 found.

So pruning is not a framework. It is a filter on one argument. `vscode-prompt-tsx`
exists because VSCode has to *build* a prompt from fragments with a budget; here
the prompt already exists and the question is only what to drop. Those are very
different problems, and the second one is much smaller. **Do not port
prompt-tsx.** Write a filter.

## Your caching question, answered

You were right to flag it, and the answer is sharp enough to design around.

Prompt caching works on an **exact prefix match**. The cache stores a prefix of
the token stream; a request reuses it up to the first token that differs, and
pays full price from there on.

Therefore: **pruning message N invalidates the cache for everything after N.**
If you prune continuously — drop a stale tool result each turn — you move the
divergence point backwards on every single turn and you will pay full input
price essentially always. You would spend more than you save, and the saving is
not even the point.

Which gives the design rule:

> **Prune rarely, in large chunks, and never in the middle if you can prune a
> prefix instead.**

One large compaction that drops the first 60% of a long conversation costs you
one uncached turn and then re-establishes a stable prefix that caches for the
rest of the session. Sixty small prunes cost you sixty uncached turns. Same
tokens removed, wildly different bills.

This is measurable rather than arguable, which is how it should be settled.

## The scratchpad, and why it is the same mechanism

"Have the agent repeat the objective to a scratchpad periodically" and "prune
low-relevance turns" are the same operation seen from two ends: **the scratchpad
is what survives a prune.** If you write the objective and the live state
somewhere durable, you can drop the transcript that produced it. If you do not,
you cannot prune anything, because everything might be load-bearing.

So the scratchpad comes *first*, and it is not a model feature — it is a file.
Which is the `graph.rs` shape again, and this fork already has the precedent:
`UpdateTodos` is a message kind in the protocol
(`app/src/ai/agent/todos/`, `app/src/ai/agent/task.rs:977` replays them in
order to derive current state). There is already a durable, replayable summary
of intent inside the conversation.

## Why it is not selected yet

Because the first move is **measurement, not construction**, and the fork has a
bad experience with skipping that step (T5.5, T5.6, and the log-spam question,
where the premise turned out to be wrong and the real finding was one step
further in).

Nobody here currently knows: how many tokens a long conversation actually
carries, what fraction is tool results, where the cache actually breaks today,
or whether pruning would save anything worth the risk. Pruning the wrong thing
does not throw — it silently makes the agent worse, in a way that is very hard
to attribute later. That is the worst failure mode on this page.

**Proposed first task, small:** a way to see, per turn, what `TaskContext`
contains and how big it is — message count and rough token count by kind
(`UserQuery` / `AgentOutput` / `ToolCall` / `ToolCallResult` / `UpdateTodos`).
It is a read-only inspector at a function that already exists, plausibly a
`warpctrl` action rather than any UI at all. It answers the caching question
with numbers, it tells you whether the feature is worth building, and if the
answer is "tool results are 85% of it" then the whole feature is one rule rather
than a relevance model.

Build the measurement. Then decide.

---

