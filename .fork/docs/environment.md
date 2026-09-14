# Environment variables, as built

*Current as of 2026-09-10. The **index** is the table in `CLAUDE.md`: every
variable, its default, and the one fact a session must not get wrong. This page
is the **account** — why each parser has the shape it has, what was measured,
what was retracted.*

*Split out of `CLAUDE.md` on 2026-09-10. Two sentences changed, both because
they pointed at the page they were on: `WARP_FORK_ACP_MODE`'s "this line said
reported, not sent" now names `CLAUDE.md`, and `WARP_FORK_EVENT_LOG`'s "recorded
here from T14.7" now names T14.7 alone. Everything else is verbatim. The list there
had grown to 10,760 characters of parenthetical asides inside a single
paragraph, 88% of it inside brackets, and all of it was being read into every
session in this repo. The rule is the part that has to ride the prefix; the
argument for the rule does not.*

---

## Two parser shapes, and which one a variable gets is a decision

`WARP_FORK_CONTROL_BIND` **refuses a typo loudly** — a hostname, a wildcard or a
misspelling leaves the wide listener shut and loopback serving, because a
silently-misread value would otherwise mean something other than what was
intended.

`WARP_FORK_REMOTE_APPROVE` treats a typo as **simply not consent** — it acts only
on a literal `1`/`on`/`true`/`yes`. Nothing loud is needed, because the failure
direction is toward less happening.

`WARP_FORK_ACP_MODE` takes the first shape and `WARP_FORK_MODEL_PRICES` the
second. When you add a variable, say which one you chose and why.

---

## `WARP_FORK_ACP_COMMAND`

Names an agent and it answers the agent panel; naming the command *is* the
switch, there is no second flag, and it outranks `WARP_FORK_LOCAL_AGENT`.

Which agent to name, the Windows-plus-WSL cwd failure, and what the recommended
pairing gets you are all still in `CLAUDE.md` — they are operating rules a
session needs before it touches this path, not accounts of one. This paragraph
also listed "pinning the `npx` version" until 2026-09-14; agents are installed
from a lock and named by absolute path now, `docs/agents-supply-chain.md`.

## `WARP_FORK_LOCAL_AGENT`

Set to `1`, `on` or `true` to answer agent conversations from the local `claude`
CLI instead of `api.warp.dev`. Default off, and `fork.rs` argues the default at
length: the policy predicates *enlarge* what works, and this one *substitutes*
for something that works. Claude runs its own tools, so Warp's diff review and
command approval do not participate and only a plain user query is handled at
all. Switching it on by fork policy would take working behaviour away from
anyone signed in.

## `WARP_FORK_POLICY`

Set `0`/`off`/`false` to run stock upstream behaviour without rebuilding — use
this to A/B a suspected fork regression.

It cannot reach `http_client`, which is why the first-party egress block needs a
switch of its own; see `WARP_FORK_ALLOW_WARP_EGRESS`. And it is a trap in one
direction worth planning for: a policy-off instance runs with a visible window
and holds a port while the discovery directory stays empty, so
`warpctrl window close` answers `no_instance` and there is no sanctioned way to
stop it. Often you do not need a GUI run at all —
`WARP_FORK_POLICY=0 warp-oss --warpctrl instance list` resolves the whole flag
set in a process that opens no window and binds no port, which is enough to A/B
any *flag*.

## `WARP_FORK_ACP_MODE`

**The session mode to ask the ACP agent for, by that agent's own id for it** —
`default` for `claude-agent-acp`, which is how you make it ask rather than let
its `auto` classifier answer. Unset by default and deliberately so: ids are
opaque and vendor-specific, so Warp discloses the mode in force and never
chooses one. An id the agent did not advertise **refuses the turn** — it is not
sent and the turn does not run.

`CLAUDE.md` and `mode.rs`'s own module header both said "reported, not sent"
until 2026-08-31; both were describing a first cut that `Decision::Refuse`
records as wrong and replaced, because a note scrolls and what it is a note
*about* is a session running under a policy nobody chose. The parser shape here
is `WARP_FORK_CONTROL_BIND`'s — a typo would otherwise silently mean something —
and unlike that one, refusing costs only the turn.

**And the mode does not survive a resume, measured 2026-09-01: it is re-sent on
every turn because every `session/load` brings the session back in the agent's
own mode.** `session_mode`, written from the agent's reply, read `current auto`
on all four turns of one conversation, each after Warp had set `default` the
turn before, while the agent asked for permission on every one — which `auto`
does not do. So that per-turn re-send is load-bearing, and removing it as
redundant would put every turn after the first back under the agent's classifier
with an event log of **zero** permission requests, which reads as "nothing
needed asking" and means "Warp was not in the loop". Not separable from outside:
whether the agent truly reverts or merely reports its opening mode on load.

## `WARP_FORK_AGENT_SPAWN_DEPTH`

A number: how deep `warpctrl agent spawn` may nest. Default **2**, so the shape
the fork was asked for fits and one more does not — a lead agent scopes work and
delegates it (depth 1), and a delegated agent may hand its result to a reviewer
(depth 2). A conversation a person started is depth 0.

`fork.rs` calls it the weaker of the two guardrails and says why: a tool
allowlist governs what the *model* may reach for, which is a harder guarantee
than any counter, but `warpctrl` is a second path and a lead agent that can run
`agent spawn` can run it in a loop whatever its own tool list says. It bounds
depth and not breadth. Ten siblings at depth 1 are within it; the honest reading
is *"a runaway cannot recurse"*, not *"a runaway cannot happen"*.

## `WARP_FORK_ALLOW_TELEMETRY_EGRESS`

Lifts the telemetry-vendor deny-list in `crates/egress_policy`. There is no
legitimate reason to set it; it exists so the backstop is a policy with a switch
rather than a hard-coded refusal nobody can audit.

## `WARP_FORK_ALLOW_WARP_EGRESS`

**Lifts the first-party block only** — `warp.dev` and its subdomains. Separate
from the telemetry switch on purpose, and needed because `WARP_FORK_POLICY=0`
cannot reach `http_client`: without it, the documented way to A/B a suspected
fork regression would fail at the socket with no clue why.

The two make different claims. Vendors must never receive data; Warp's own hosts
are ones the product legitimately uses and this fork replaced one at a time. One
switch would have silently broken the A/B recipe.

## `WARP_FORK_HARNESS_DIR`

The agent's `.claude/projects` directory for `agent.trace`, when the instance's
own search -- this home, then a WSL session's guest homes -- does not find it;
unset by default.

## `WARP_FORK_QUAKE_VISOR`

The one that defaults **on** — set it off to get upstream's terminal in the
hotkey window.

## `WARP_FORK_WSL_AUTO_CONNECT`

**Also defaults on**, same parser: a WSL pane attaches Warp's
remote-development server to its own distribution when its shell bootstraps, so
the tree, the buffer, search and the git chip route inside the distribution
instead of over 9p. Set it off to get the manual behaviour back — palette action
or `warpctrl remote wsl connect`. A failed connect is a log line and a
not-routed pane; `warpctrl session inspect` says which you have. Built
2026-09-05, `.fork/docs/wsl.md`.

## `WARP_FORK_WSL_LSP`

**Defaults on**, same parser: a language server for a `\\wsl$\<distro>\...`
workspace runs *inside* the distribution, spawned as
`wsl.exe -d <distro> --shell-type login -- rust-analyzer`, and a routed buffer
gets a path the LSP stack accepts. Set it off for upstream's Windows-side server
over 9p. Built 2026-09-05, measured end to end the same night;
`.fork/docs/wsl.md`, "Language servers, as built".

## `WARP_FORK_MODEL_PRICES`

`fetch`, and nothing else — the parser is `WARP_FORK_REMOTE_APPROVE`'s, so a
typo is simply not consent and costs a stale number. **The only variable that
makes Warp's own HTTP client dial a host of Warp's choosing**: one request per
launch to OpenRouter's public model list, which fills prices into the Model
Specs card's existing id→row mapping and never replaces the mapping. Off by
default. The deny-list is *not* the layer that decides this — `openrouter.ai` is
on neither list and would pass without being considered — which is the general
point about a deny-list stated as a switch. T21.4,
`.fork/runs/pricefetch-2026-09-09/`.

## `WARP_FORK_FRAME_LOG`

`on`, or a threshold in ms — slow-frame accounting to the local log; **reach for
this before theorising about why something feels slow**.

## `WARP_FORK_EVENT_LOG`

`on`, or a directory — one JSONL file per agent session, appended as events
arrive; **reach for this before theorising about what an agent did**.

T14.9 gave the ACP path tool events, so the "no tool events at all" recorded
from T14.7 is **no longer true**. It was also briefly true-looking for a worse
reason: until T14.15 an ACP turn wrote **two** files that never named each other
— `session_start`/`stop` under the conversation id, tool events under the
agent's session id — so opening the obvious one showed a session with nothing
between its ends. Now everything for a turn is filed under **Warp's conversation
id**, the way `local_agent` always did, with the agent's own id on each line as
`linked_session_id`. One turn, one file.

**T14.17 added `permission_request`/`permission_replied` to that path** — the
`tool_input` that was shown, what was decided
(`allowed`/`denied`/`unanswered`), which surface answered, and on the ask
whether Warp had a *yes* to offer at all.

**Read a zero here carefully**: T14.18 measured a panel session producing zero
permission requests because the agent's own classifier answered first, so no
lines means *Warp was not in the loop*, never that nothing was decided.

**`unanswered` is what a cancelled turn leaves**, and it did not exist until an
agent reviewing T14.17 in the panel found that the value was written by a unit
test and unreachable on the real path: the ask is logged synchronously and the
answer from a task cancellation drops mid-`await`, so the trail kept the
question and lost its ending. Measured both ways after the fix — a cancelled ask
writes it, an answered one does not.

**And read the file in timestamp order**: the path files per conversation, so
`cat *.jsonl` gives filename order and a reader inferring causality from line
order will be wrong.

## `WARP_FORK_TRANSCRIPT`

`on` writes to **`.warp/transcripts/` under the pane's own directory** — not
`state_dir`, because outside the session's directory the agent's read of the
file arrives as `tool: other` and *no* answer exists, so the tidy location is
the unusable one. `.warp/` is upstream's project directory and is tracked, so
`/.warp/transcripts/` is gitignored. Any other value is taken as the directory,
and the caller owns reachability.

**Writes the conversation to disk so the agent can grep back what its own
compaction discarded — measured across two real compactions: the agent answered
"I DO NOT HAVE IT" from memory and then found the same detail in the file, with
zero permission requests.** Note *what* it recovers: compaction is not
indiscriminate, and a fact flagged as important survives inside the summary.
What is lost, and what this is for, is the bulky incidental detail a working
session is actually made of.

Off by default, because persisting what was said is not something a
no-telemetry fork should start doing unasked.

**Owner-only since 2026-08-31, and `0644` before that** — the file holds the
user's prompts verbatim and inherited the umask, as did the event log's
`*.jsonl` with its `tool_input` previews; both now go through
`fork::create_private_dir`/`create_private_file`, which put the mode on the
`open` rather than chmod-ing after it, because the window between the two is
exactly when the first line is written. `discovery.rs` had the right instinct
from the start with `0700`/`0600`; these two never got it. **Verified by running,
which exposed a residual reading would have missed:** a transcript written by a
*pre-fix* build keeps `0644` until that conversation is next written, because the
transcript is rewritten whole per conversation and a dormant one is never
rewritten. An active conversation self-heals on its next turn; the event log
self-heals via `tighten_existing` on reopen. Deliberately **not** swept: a sweep
would chmod files the fork is not otherwise touching, in a directory that
follows the pane's cwd and can therefore be anywhere. `chmod 600` on an old
transcript is the user's call, and this sentence is how they learn it is theirs
to make.

The pointer rides every prompt as its own content block, so your text is never
edited; the panel says once that it is happening **on both agent paths, and only
since 2026-08-31**. The writer (`transcript::observe`) always hung off the shared
`BlocklistAIHistoryModel`, which both paths feed, while the pointer and the
announcement were injected only in `acp_agent`. Measured both ways before the
fix: an ACP conversation carried one `[Warp]` line, a `local_agent` one carried
**zero**, and the file was written either way — so that path put the user's
prompts on disk, told nobody, and handed the agent nothing. **The fix's first cut
also measured zero**, and the reason is worth keeping: a note is an
`AddMessagesToTask`, and on this transport the task is created by the agent
stream's own `init` event, so a note queued ahead of the stream names a task
that does not exist and is dropped. Its unit test passed throughout. Ordering
against a stream is not something a unit test on the message can see.

And Warp's own asides are marked `[Warp]` and kept out of the file so an agent
never reads them as its own words.

**What it holds that the agent's own store does not is the reason a call
failed**: measured, `opencode` records a denied command as `status=error` with no
notion that anything refused it, so an agent reading its own history sees a
failure where there was a decision. Warp keeps the refusal.

## `WARP_FORK_CONTROL_BIND`

**The only one that reaches off the machine** — one literal IP address,
optionally with a port (`192.168.1.5:41234`, `[fd00::1]:41234`; pin one if you
want the console on a home screen, because an ephemeral port makes a saved URL
dead on the next launch). A hostname, a wildcard, or a typo leaves the wide
listener shut and loopback serving, because refusing to start would take out
`warpctrl window close`.

Pin the port for a second reason too: the inbound Windows firewall rule names
one port, so an *ephemeral* port is refused outright rather than merely
producing a stale saved URL. A Tailscale address is still one literal IP and so
fits this parser unchanged — binding it is **narrower** than a LAN bind, not
wider. Prefer it as a *replacement* bind, never an addition, and never a
port-forward.

## `WARP_FORK_REMOTE_APPROVE`

Lets a *paired* device run `agent.approve` — say **yes** to a CLI agent's
permission prompt from a phone. Off unless it is literally
`1`/`on`/`true`/`yes`; `agent.deny` needs no switch, because saying no can only
ever make less happen. Note the opposite parser shape to
`WARP_FORK_CONTROL_BIND`: there a typo must be *refused loudly* because it would
otherwise silently mean something, here a typo is simply not consent.

---

## `WARP_FORK_WINDOW_BOUNDS`

Added 2026-09-12, after a live Windows run left a window wherever the platform
put it and the maintainer said that finding and rearranging windows between
launches cost both of us. `1400x900+100+100` opens every normal window at that
size and place: new ones, a first launch, and restored ones, whose saved
position it overrides.

The coordinates are logical pixels on the virtual screen, not "the primary
monitor". On Windows the two agree, because the virtual screen's origin is the
primary monitor's top-left corner. It is not resolved against a monitor
because on Windows that lookup goes through an existing window
(`crates/warpui/src/windowing/winit/window/windows_wm.rs`), and a launch's first
window is the one with none. A rect that overlaps no monitor is dropped by the
windowing layer and the platform places the window.

Parser shape: a value that does not parse is **ignored with a warning in the
log**, and the window opens where upstream would put it. Neither of the two
shapes above fits exactly: nothing is consented to and nothing is exposed, so
refusing to start would be louder than a misplaced window deserves, and the log
line is what separates a typo from a setting that did nothing.

Not gated on `WARP_FORK_POLICY`, because it changes where a window is rather
than what Warp does, and an A/B run is where a fixed place helps most. It does
not touch the hotkey window or a window torn off by dragging a tab.

Driven live on Windows the same night, at 100% scale only: a restored window
whose saved position had been moved opened at the pinned rect, and with the
variable unset the same profile reopened at the moved one
(`.fork/runs/window-bounds-2026-09-12/`).

## Not a variable

Tab→pane drag has no variable of its own; `WARP_FORK_POLICY=0` puts the tab's
horizontal-only drag axis back.
