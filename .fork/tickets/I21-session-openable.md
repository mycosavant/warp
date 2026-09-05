> Idea I21, split out of `.fork/IDEAS.md` on 2026-09-05. History; read the section you came for.

# I21 — a session must be openable even when it cannot be continued

Captured 2026-08-31, from a `claude --resume` that refused. **Not scoped, not
agreed.** Two questions arrived together and only the first is an idea; the
second is a threat-model question that is answered inline below so it does not
get filed as work it does not deserve.

## The ask, as given

> "the claudecode cli wouldn't let me just open this conversation when i used
> the `--resume` flag, saying that it was near 500k context and i needed to
> autocompact it. so, there is just a summary in the terminal now, instead of
> the transcript, which isn't great. **this is what this fork does not want to
> do. make sure a user is able to open any previous conversation to view it at
> bare minimum.** resume is a huge part of my workflow and this is the first
> time i've ever seen that."

Restated as the rule it wants: **compaction is a precondition for *continuing* a
session, never for *viewing* one.** Reading a conversation costs no context
window at all — it is a file on disk — so a tool that compacts before it will
show you anything has destroyed the artifact in order to display it.

## Why this is on-thesis and not a general grumble

T14.19 exists because **compaction discards the bulky incidental detail a working
session is made of**, and the fork's answer was to put the conversation on disk
so an agent could grep back what its own compaction dropped — measured twice
against real compactions, where the agent answered "I DO NOT HAVE IT" from memory
and then found the same detail in the file.

The transcript is therefore already this fork's stated position on the question:
*the file is the record, and the summary is a lossy view of it.* A resume path
that can only offer the summary is the exact failure `WARP_FORK_TRANSCRIPT` was
built to route around, appearing one layer up in the tool the fork is driven
from. Warp's own agent panel must not acquire the same shape.

## What already exists underneath it

**Unverified — read, not run.** Everything in this section is the current state
of the fork's own transcript path, and none of it has been exercised against a
"view an old conversation" flow, because no such flow exists yet.

- `app/src/ai/transcript.rs` already writes one Markdown file per conversation,
  named `<conversation-id>.md` (`path_for`, line 94), and rewrites it whole per
  conversation on each turn.
- Both agent paths feed it through the shared `BlocklistAIHistoryModel`, so the
  record is transport-independent.
- `fork::TranscriptLocation` already distinguishes `InSessionProject` from
  `Fixed(path)`, so "where the record lives" is a solved and tested decision.
- Warp's own asides are marked `[Warp]` and kept **out** of the file, so an agent
  never reads Warp's words as its own — which is also exactly what a *viewer*
  would need in order to render provenance honestly.

So a read-only viewer is close to the fork's usual shape: the artifact exists,
and what is missing is a way to look at it. `warpctrl` is the obvious surface and
would add one verb, not a subsystem.

## The security question that arrived with it, answered rather than filed

> "making sure that we're not opening ourselves up for things like seeded and/or
> tampered transcripts with potentially malicious instructions… would one hash
> the transcript? … i could see a case in certain, say, enterprise where one
> could justify encryption at rest. how difficult of a problem is that? compute
> intensive, at least. … is this even a credible threat, or am i trying to
> overengineer."

**It is a credible threat and hashing is the wrong first move.** Four points,
in the order they should be acted on:

1. **A hash does not defend against the principal who can write the file.**
   Whoever can rewrite the transcript can rewrite the digest beside it. Integrity
   without a key held somewhere the writer cannot reach is a checksum against
   *accident*, not against an adversary — and calling it "tamper detection" would
   be precisely the kind of overclaim `CLAUDE.md` collects. The nearest
   precedent in this fork is stated plainly in its own docs: the approval digest
   "binds server-state to server-state; it **never** binds what the human saw."
   A transcript hash would bind file-state to file-state.

2. **The credible case is provenance, not in-place edits.** A same-UID attacker
   editing your transcript already has your shell, your `~/.ssh` and your
   credential broker; the transcript is the least of it. The realistic "seeded
   transcript" is one **you did not author**: pulled from a repo, synced down
   from a remote or cloud session, restored from a backup, or handed over by a
   colleague. The control that helps there is *marking origin and warning on
   resume of a foreign file* — TOFU, warn-not-block, which this fork already
   implements in `ai/mcp/tool_digest.rs` and whose docs say in as many words that
   it is "deliberately not a block."

3. **Compute is not the hard part; key custody is.** BLAKE3 over a 5 MB
   transcript is on the order of a millisecond, and AES-NI runs north of a
   gigabyte a second — neither is a cost worth a sentence. The hard part is
   *where the key lives*, and a key stored beside the file it protects is
   theatre. Real answers are the OS keychain, a TPM, or DPAPI, and that is
   exactly the enterprise case the ask correctly identified. **So encryption at
   rest is not an expensive problem, it is a key-management problem wearing a
   performance problem's clothes.**

4. **A UUID filename is an identifier, not a control.** Guessing the path was
   never the threat; writing to it is. The two free things outrank every
   cryptographic option and this fork has already paid for one of them: file
   modes. `WARP_FORK_TRANSCRIPT` inherited the umask and shipped **`0644`** until
   2026-08-31, which was the entire exposure and cost nothing to close
   (`fork::create_private_file`, mode on the `open` rather than a chmod after
   it). Directory `0700` and file `0600` is the defence; the digest is a
   different, weaker claim about a different threat.

**Verdict: not overengineering to ask, and not worth building yet.** Nothing here
has a friction log behind it, which is the bar this board applies to everything
else.

## But one present exposure was found while answering it — and fixed hours later

**Fixed by `541a853f3` on 2026-08-31, a few hours after this entry was written.**
`fork::keep_dir_out_of_git` now writes a `.gitignore` containing `*` into the
transcript directory at creation, before the first transcript rather than after
it, and only for the location the fork chose — a caller who names a directory
owns it. Tests landed later still, in `17c81a366`, because the fix shipped
without any: found by reviewing the tests separately from the code. The section
below is kept as written because it is the argument for the fix, and because an
entry that quietly acquires a resolution reads as though the problem was never
real.

**Read 2026-08-31, not run — and it should be run before it is believed.**

`WARP_FORK_TRANSCRIPT=on` resolves to `InSessionProject`, which writes to
`.warp/transcripts/` **under the pane's own current directory**. This repo
gitignores it (`.gitignore:77`, `/.warp/transcripts/`). **`transcript.rs` writes
no `.gitignore` of its own** — grepped, there is no ignore handling in the file
at all — so that line protects *this* repo and nothing else.

A pane opened in any other git project therefore writes the user's verbatim
prompts into `<repo>/.warp/transcripts/<conversation-id>.md` with no ignore rule
in front of it, and `git add -A` stages them. In a non-Warp repo `.warp/` will
not previously exist, so it arrives as a new untracked directory — the case
`add -A` is most likely to sweep up.

That is a realistic path for a transcript to leave the machine and it requires no
attacker at all, which makes it strictly more urgent than anything in the section
above. **The fix is smaller than the finding**: write a `.gitignore` containing
`*` into the transcript directory at creation, next to the `0700`. One file, no
new surface, and it is the same instinct `discovery.rs` had from the start.

**Unverified and to be measured first:** whether a real `git status` in a
non-Warp repo actually shows the file, and whether `create_private_dir`'s
leaf-only `0700` leaves an intermediate `.warp/` at the umask.

## The smallest version that is still the idea

`warpctrl agent transcript <conversation-id>` — read the file, print it, stop.
No window, no pane, no new consent surface, and no context window consumed
because nothing is sent to a model. The viewing half of the ask is a **`cat` with
a lookup**, which is this board's own standard for what an idea should shrink to.

The seeded-transcript half stays unbuilt until something asks for it, and if it
is ever built the first move is point 4 above and point 2 after it — never the
hash.
