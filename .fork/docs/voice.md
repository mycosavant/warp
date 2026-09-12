# Voice, both directions

*As of 2026-09-12.* Speech **in** and speech **out** are two halves of one
surface, and only one of them has ever been on this fork's board. This page is
the fork's account of both: what exists, where it lives, what the fork would
have to build, and the engineering findings worth keeping whichever way it goes.

**Most of the work is not in this repository.** Read this page to know what is
settled and what the fork's own share is; read the sources named below for the
detail. Nothing here restates their documentation.

| half | where the work is | state |
|---|---|---|
| speech **out** — read-aloud | `mycosavant/pocket-tts` | built, on a device, tested end to end |
| speech **in** — transcription | `.fork/tickets/T02` | T2.1/T2.2 built and self-contained in this fork; the maintainer ranks it high; not chasing openwhispr/franken_whisper as dependencies |

---

## Why this page exists at all

On 2026-09-12 a session searched *this* repository for `text-to-speech`, found
zero hits, and filed read-aloud as an unexplored requirement. It is none of
those things: there is a built Android app, a device test with eight cases, and
two open pull requests. **The error was concluding from one tree's silence about
work that lives in another tree and in cloud artifacts** — the same shape as
reading a thin friction log and inferring an idle week, made twice in one
session. This page is the index that stops the third time.

## Speech out: what exists

`org.pockettts.android` — an Android app using Kyutai Pocket TTS voices through
sherpa-onnx, registered as a **system TTS engine**, so Select-to-Speak, Chrome
read-aloud and ebook readers all speak in a natural voice with the calling app
oblivious that it is installed. Alongside that it has its own reader —
`ReadAloudActivity` → `Reader.speak()` → `PlaybackService` — with real transport
controls and a scratchpad.

It already accepts `ACTION_PROCESS_TEXT` and `ACTION_SEND`, so the whole
transport UI sits behind an intent, and since PR #3 behind a broadcast as well.

**Tested on the device 2026-09-11/12** (SM-S938U1, Android 16, build 46):
transcript → script → broadcast → audio, with lock-screen controls and the
screen off. Tests 1–5 pass. Every first-contact failure was in the shell script
or in proot; none in the app. Two PRs open at the time of writing — **#7** the
script fixes, **#8** the app fixes, green in CI and not yet run on a phone.

**Why it is a `/command` and not a Stop hook**, which is the part that
generalises: Android 10+ bans *background* activity starts. A Stop hook fires
with nothing in the foreground and `am start` is refused; a user-typed command
fires while the person is looking at the terminal, so it is permitted. The
platform chose the trigger. An exported broadcast receiver is the way back to
screen-off operation, precisely because a receiver may never start an activity
and does not need to.

## The fork's own share, and it is small

**The tested path assumes Claude Code runs on the phone**, in proot Ubuntu under
Termux, where `am` is a local exec and the harness JSONL sits beside it.

**This fork's mobile surface is mosh+tmux into the desk.** The agent runs in
WSL, so the text originates on a machine with no Android on it and `am` in that
pane would run on the wrong side. The last assistant message has to reach a
**phone-local** shell before anything can speak it.

The primitive for that already exists and needs no Warp code:

```
warpctrl agent trace <conversation>
```

It joins the agent's own session file with Warp's event log and **needs no
running Warp** — the same property that made mosh+tmux worth having, since the
instance is usually what is being rebuilt. So the candidate is one line from a
phone-local Termux shell rather than the mosh pane: ssh to the desk, take the
trace, broadcast locally. In `speak-last.sh`'s own structure that is a second
**source** for the text, not a second script.

**Unbuilt on this side. Do not build it before #7 and #8 are confirmed on a
device** — they are green in CI and have never touched a phone, and wiring a new
text source into two unverified layers means debugging all three at once.

## Four constraints any bridge inherits

Measured on the device, properties of the environment rather than of the app,
and not negotiable from this side.

- **`am` cannot confirm delivery.** It exits 0 for a receiver that does not
  exist *and* for a package that is not installed; it exits 1 only when the
  platform refuses the broadcast outright. The app's own `utterances read` count
  is the only proof a read happened. Any fork-side bridge needs that same
  end-to-end confirmation and cannot take an exit code for it.
- **proot breaks process substitution.** `/dev/fd/63: No such file or directory`
  — proot binds `/dev/fd` to the *reader's* `/proc/self/fd`. The loop body never
  ran and **the script reported success while sending nothing**, which is this
  fork's most-logged defect appearing in someone else's codebase.
- **`tail` from a pipe hangs under proot**, unconditionally, not size-dependent.
  `awk 'END{print}'` is fine, and so is `head` and `grep` from a pipe.
- **`am broadcast` needs an explicit `--user 0` on Android 16.** termux-am
  passes `-2` (`USER_CURRENT`) by default and the platform now refuses to
  resolve it from an app uid; `--user current` fails identically.

## One finding that follows agent text wherever it goes

The engine has no length for a chunk in advance: it runs the language model
frame by frame and stops when its own end-of-speech head crosses a hard-coded
threshold. On a **trailing run of very short sentences** that head fires early
and the tail is lost — `One. Two. Three. Four. Five.` stopped after *"Two."*

**Position is the variable, not length.** Mid-text runts are harmless, and a
single trailing runt is harmless; a run of them at the end is not. PR #8 joins
such a run with commas in the spoken form only, leaving what is shown untouched.

The reason it matters here is #8's own: **that is how agent replies end** — in
countdowns and strings of statuses. Warp's replies have the same shape, so any
fork surface that speaks agent output meets this.

Documented and unfixed upstream: the same head fires early inside an ordinary
sentence holding a run of numbers. `Tests 1, 2 and 3 are green.` lost "green" in
2 of 3 runs; `Files A, B and C are updated.` never did. The threshold is the
engine's and the reference implementation uses the same value, so nothing at the
app layer can tell a premature ending from a real one.

## Three findings worth keeping for their shape

None is about voice. Each is a fresh instance of a rule `CLAUDE.md` already
carries, found in a different language on a different machine, which is the
argument for writing them down.

**The timer measured the consumer, not the producer.** The app reported
*"generation speed: 0.83x real time — slower than playback"* and that number was
wrong about what it named. The reader fed the speaker from inside the engine's
callback, and the write blocks when the buffer is full — so chunk N+1 could not
begin until chunk N had been *heard*. The figure timed a call that spent most of
its length waiting on playback. Back-solved, the model runs roughly **1.4–4×
real time** on that device. Underruns could not see it either, because a track
that is simply not written to between chunks is not running dry in a way that
counts. This is *measuring the wrong quantity* with a new twist: the wrong
quantity was **inside the right call**, so the instrument looked correctly
aimed.

**A bounded channel would have deadlocked where a semaphore does not.** The fix
made `Reader.play` a producer and a consumer, with the engine's callback
dropping pieces into a channel and a writer feeding the sink. The bound is a
**semaphore on chunks, not a bounded channel** — and the reason is worth
keeping: *the engine's callback cannot suspend*. A callback blocked waiting for
room would hold the engine exactly as the blocking write did, with nothing to
wake it after a stop. Same intent, opposite outcome, and only one of the two
survives cancellation.

**The pinned dependency was re-splitting text the caller had already split.**
Voice drift over long text was attributed for a month to independent
generations from one voice prompt, with audio-prompt chaining proposed as the
fix. The real cause was one behaviour in sherpa-onnx's C++: it cuts a chunk back
into sentences on `.!?` and generates each independently. The app chunked; the
engine re-split underneath it. Fixed from the *caller's* side in four lines —
two sentence-length bounds in sherpa's `extra` map, set above the app's own
chunk cap. On the device: *"much better, first syllable collapse is almost
none."*

That one has a second edge, recorded in the position paper: *a defect in code
you own is a bug; a defect in code you call is a mystery.* It took a month to
find because the behaviour had no setting, no log and no symptom except a voice
that sounded wrong in a way nobody could name.

## Speech in

`.fork/tickets/T02` is this fork's own work and T2.1/T2.2 are built:
`LocalTranscriber` implementing `voice::transcriber::Transcriber`, fail-closed
so that a misconfiguration is an error rather than a silent fallback to the
server. The privacy note in that ticket is the reason it exists —
`Provider::OpenAI` is *not* a local path, and `ServerVoiceTranscriber` POSTs
base64 audio to `api.warp.dev` regardless of provider.

`mycosavant/openwhispr` is the neighbouring work outside this repo, local
Parakeet/Whisper with BYOK cloud models.

**Not a fork dependency, and not worth chasing as one (2026-09-12).** The
maintainer doesn't own openwhispr and prefers this fork stay self-contained
rather than take on a dependency on another project — `LocalTranscriber`'s
`Http`/`Command` contracts already work with any local engine pointed at
them, unmodified. `franken_whisper` (a friend's project, stale) was checked
for the same reason and set aside for the same reason.

**A note on how this half was nearly mis-filed.** Grepping `voice` in this
repository returns ~23 hits and reads as covered; all of it is transcription.
For a while that led to the opposite error — treating T02 as pointing "the wrong
direction". It does not. The maintainer ranks input high, and speaking to the
fork and being read to by it are one surface. **Only one half was ever on the
board.**

## Sources

| | |
|---|---|
| `mycosavant/pocket-tts` | the app, `android/docs/`, PRs #1–#8 |
| artifact *Read Aloud Wiring* | the implementation handoff, 2026-09-11, with its ruled-out list |
| artifact *Eleven Utterances* | the device test, 2026-09-11/12, eight cases, revised after the follow-up |
| artifact *Pipeline, Not Inference* | the position paper on owning the pipeline rather than the inference |
| `.fork/tickets/T02-local-voice.md` | the fork's transcription work |
| `.fork/runs/run-live-2026-09/friction.md` | friction #2, and the two corrections that produced this page |
