# Run 2 — 2026-09-02, terminated by the maintainer at 50 minutes

**Log: this file. Conversation `27357def-2174-47ff-b260-b8ce3918dea6`, on the
Windows build, in a WSL pane, `claude-agent-acp` in `default`, both instruments
on.** Terminated deliberately, not crashed.

> **Verdict: the horizon's eight-hours caveat is answered, and the answer is no.**
> Not "no with caveats" and not "not observed" — a maintainer who has used many
> coding agents stopped a run at 50 minutes because the consent posture made it
> unworkable. That is a result, and a stronger one than a clean pass.

---

## What was measured

Read off the event log, not recalled:

| | |
|---|---|
| wall clock | `02:02:00.229Z` → `02:52:29.067Z` — **50m 29s** |
| event lines | 228 |
| prompts submitted | 5 |
| tool calls | 59 started, 57 completed |
| **permission requests** | **44** |
| **permission replies** | **44** — balanced, none unanswered |
| turns ended | 4 `stop`, 2 `stop_failure` |
| cwd, every line | `/home/effatha/git/warp` |

**One approval every 69 seconds. 8.8 per prompt.** Held flat, an eight-hour day
is **~420 approvals**, each costing a read and two clicks.

That is the finding. Everything below is detail.

---

## 1. Approval density is disqualifying, and the boilerplate makes it worse

44 requests in 50 minutes is the number, but the shape is what stopped the run.
Every one — including `Edit` on a file inside the session directory — renders
four paragraphs before the two controls:

```
Edit crates/warp_cli/src/local_control/acp.rs. Answer yes with
warpctrl agent approve 95b9589a-…:0 or no with warpctrl agent deny 95b9589a-…:0
— both take the digest that warpctrl agent approvals reports. A yes covers this
one call and nothing after it. A paired device can answer too, though yes only
travels there when WARP_FORK_REMOTE_APPROVE is set.

It says this acts on /home/effatha/git/warp/crates/warp_cli/src/local_control/acp.rs.
This session runs in /home/effatha/git/warp — Warp chose that from the pane. The
agent resolves its own permission rules from there, and Warp cannot see them.
```

Every sentence there is true and was written for a reason. Together, forty-four
times, they are the friction. **The text is sized for the first request of a
session and is paid on every one.**

**This is the measured case I18 never had.** `CLAUDE.md` retracted the earlier
one — the sixteen-ask storm that evaporated when the question was re-asked with
a boundary — and recorded that the persistent grant's argument was no longer that
measurement. This run supplies a different one: the asks here are not an
unbounded audit wandering the filesystem, they are **ordinary scoped work inside
the session directory**, and there are 44 of them in under an hour. A boundary
does not fix this, because the boundary was never crossed.

Recorded, **not acted on**. Permission posture is frozen and that is the
maintainer's call.

## 2. The transcript writes to a filesystem the agent cannot read

`WARP_FORK_TRANSCRIPT=on` wrote today's conversation to:

```
C:\home\effatha\git\warp\.warp\transcripts\27357def-….md      41,964 bytes
C:\home\effatha\.warp\transcripts\56fd996c-….md               (the $HOME session)
```

`C:\home\` did not exist before this run. Warp created the whole tree,
`.gitignore` included. The repository's real `.warp/transcripts/` has nothing
newer than 2026-09-01.

**This is T18's bug one layer up, and the layer matters.** T18 fixed where the
*agent* is started; this is where *Warp* writes. The pane reports a Unix cwd, the
Windows GUI process joins `.warp/transcripts` onto it, and Windows resolves
`/home/…` to `C:\home\…` — silently, because creating a directory succeeds.

Two consequences, the second worse than the first:

- **The feature is inert on this platform.** The transcript exists so an agent can
  grep back what its own compaction discarded. The agent lives in
  `/home/effatha/git/warp`. It cannot see this file. So for **unknown 4** — the
  reason the long run exists — the recovery half was silently absent the whole
  time.
- **The privacy mode is void.** Transcripts were made owner-only on 2026-08-31
  because they hold the user's prompts verbatim, through
  `fork::create_private_file` so the mode rides the `open`. DrvFs carries no Unix
  mode; the file is `-rwxrwxrwx`. **The fix is correct and the filesystem is not
  listening.** Nothing in the fork can detect this, because `create_private_file`
  succeeded.

**And it is not the bug it looks like.** The maintainer's reading was *"the agent
isn't on the WSL side"*. The agent **is** on the WSL side and T18 is working:
`session/new` was accepted (it fails outright under T18's bug), every event line
carries cwd `/home/effatha/git/warp`, and the agent's own commands treat Windows
as the far side — *"sync Windows checkout **from the WSL** origin remote"*. The
transcript is written by **Warp**, a different process on the other side of the
boundary. Two processes, two filesystems, one path string. Filing this as "the
agent is on the wrong side" would have produced a fix in the wrong file.

## 3. `options_offered` renders as a menu and means a receipt

The approval surface lists *"Yes"*, *"Yes, and don't ask again for similar
commands"*, *"No"*. **The middle one can never be chosen.**

That is deliberate and documented. `acp_approval.rs`: *"it may offer the
single-shot yes — and nothing else. The always-variants are not rendered at all,
because a button that sets a session policy would be authorising something never
shown."* `acp_permission::choose` refuses any option where `changes_policy` is
true — `AllowAlways`, `RejectAlways`, or anything declaring a policy change in
`_meta`. `registry.rs` says `options_offered` is kept *"as data rather than as
controls"*.

So the code is right and the rendering is wrong. The list is an **audit record of
what the agent offered**, and it is drawn where a person reads a **menu**. Nothing
on screen says which entries are unavailable or why.

This is the fork's own recurring defect wearing new clothes: a surface whose
presentation claims more than the code does. It is worse here than in a doc
comment, because this is the consent surface and the thing being misrepresented
is what a *yes* buys.

## 4. The composer, in the maintainer's words

> *"it's so hard to tell what's actually happening in the composer, it's unreal.
> I've used a lot of coding agents and tools and that was the least ergonomic
> experience I've ever had. I had no idea what was really going on."*

What renders is tool labels — `Terminal`, `Read File` — and Warp's own permission
blurbs. No thinking, and little of the agent's prose. Whether that is Warp
dropping `agent_message_chunk`s or the agent emitting little during a tool loop
is **not established** and must not be guessed: this file has no measurement
separating them, and the obvious next step is to compare the panel against the
transcript for one turn — which this run could not do, because of finding 2.

Recorded verbatim because it is the one thing here no instrument captured, and
the person who hit it has the relevant comparison set.

## 5. An agent can relaunch its own host into a duplicate window

Approval `e0c15631-…:7` — *"Enable instrumentation and launch the Windows Warp
build"* — was answered at `02:08:35.072Z`. A second `warp-oss` (PID 23088) was
created at `02:08:35`. Same second.

The agent ran `ggwarpdev launch` against a Warp that was already running. Warp
restores session layout, so the duplicate came up with **identical panes and
tabs** and took foreground — indistinguishable, from the user's seat, from
everything having crashed and restarted. It cost a diagnosis before anyone
realised there were two.

Then it compounds: two instances make every `warpctrl` call without `--instance`
answer `ambiguous_instance`, including the agent's own. It was parked on
*"List Warp instances with full detail to distinguish the two discovery records"*
when the confusion was noticed — the agent had found the symptom and was working
back toward a cause it had itself created.

**The `Parent has crashed; continuing execution` line in `warp-oss.log.old.0` is
a red herring** and `CLAUDE.md` already says so: it marks the recovery sibling's
log, not a crash.

---

## What this does and does not settle for the horizon

**Settled.** The eight-hours caveat carried forward from the last horizon. The
answer is no at this posture, from a real attempt at real work, and the number is
44 approvals in 50 minutes.

**Not settled — and now known to have been unmeasurable.** Unknown 4, the
compaction cadence. Five prompts did not come near compacting, and finding 2
means the recovery half was absent regardless. **Run 2 could not have answered
unknown 4 even if it had run all day**, which is the same class of mistake run 1
made on the wrong platform: *a horizon measured where the instrument does not
work is not measured.* Fix finding 2 before run 3 is scheduled.

**Untouched.** Unknowns 2 and 3 — no two requests were parked at once, and all
four turns closed.

## Not fixed, deliberately

Nothing above was patched during or after the run. `GOAL.md` names fixing
frictions mid-run as a failure mode, and three of these five are permission
posture or consent rendering, which are frozen and the maintainer's to decide.
Findings 2 and 5 are ordinary bugs and want tickets, not a hotfix at the end of a
session that ended badly.
