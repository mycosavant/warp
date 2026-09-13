# Voice, both directions

*As of 2026-09-13, evening.* Speech **in** and speech **out** are two halves of one
surface, and only one of them has ever been on this fork's board. This page is
the fork's account of both: what exists, where it lives, what the fork would
have to build, and the engineering findings worth keeping whichever way it goes.

**Most of the work is not in this repository.** Read this page to know what is
settled and what the fork's own share is; read the sources named below for the
detail. Nothing here restates their documentation.

| half | where | state |
|---|---|---|
| speech **in**, at the desk | OpenWhispr, installed on Windows | **in daily use**; its paste reaches Warp's composer (maintainer, 2026-09-13) |
| speech **in**, inside Warp | `.fork/tickets/T02`, `LocalTranscriber` | built, egress measured; spoken words through the mic never measured |
| speech **out**, at the desk | `pocket-speak`, branch `desk-cli` of `mycosavant/pocket-tts`; palette entry in Warp | **built and measured 2026-09-13** on WSL and Windows; ~~decided, unbuilt~~ (morning) |
| speech **out**, on the phone | `mycosavant/pocket-tts` Android app | built, on a device, tested end to end |

~~This table had two rows until 2026-09-13, and the speech-in row read *"T2.1/T2.2
built and self-contained in this fork."*~~ True of the code, and it read as
"done". What falsified the reading: the 2026-09-05 egress run posted to
`whisper-stub.py`, which returns canned text, and T02 itself says the words of
a real recording were never proved. No whisper engine is installed on this
machine today.

---

## The desk is primary: decisions, 2026-09-13

Taken by the maintainer after the facts below were measured.

- **The desk (Windows) is the primary target.** The Android app is the
  maintainer's system-level TTS for that device, used for much more than this;
  wiring it to Termux was a stop-gap for this work, not its goal.
- **Dictation at the desk is OpenWhispr**, and Warp needs no code for it.
- **Read-aloud at the desk is a pocket-tts CLI the maintainer owns**, living in
  `mycosavant/pocket-tts`. Warp names it as a command, the way `LocalTranscriber`
  names a transcriber binary, and never links the engine.
- **Windows `System.Speech` is rejected** on quality.
- **Mobile leans agnostic**: use whatever TTS the device already has, rather
  than Warp pushing voice to a device. A leaning, not a decision.

**What was not established:** that OpenWhispr's paste works in every Warp input
(only the agent composer was tried). ~~…and anything about the CLI beyond
reading the code it will be ported from.~~ The CLI was built and measured the
same evening; see *As built* below.

## Speech in

### OpenWhispr, at the desk

Upstream's release is installed (the fork `mycosavant/openwhispr` is behind it
and is not what runs), with local OpenAI Whisper Base active and the hotkey
Ctrl+Win. Read off the running processes 2026-09-13:
`windows-key-listener.exe Control+Super`, `windows-mic-listener.exe` and a
`qdrant` instance are its children, and no whisper engine is running between
dictations.

**Why it reaches Warp**, read from the fork's `resources/windows-fast-paste.c`:
it pastes into the foreground window, sending Ctrl+Shift+V to windows it
recognises as terminals and Ctrl+V to everything else. `warp-oss.exe` is on
neither of its terminal lists, so Warp receives Ctrl+V, which it binds on
Windows (`app/src/terminal/view/init.rs:274`). The installed release may carry
different lists; the maintainer's test is what settles it.

**Do not point `LocalTranscriber` at OpenWhispr's engine.** It is not a
contract:

- `src/helpers/whisperServer.js` starts `whisper-server` on demand, on a port
  picked from 8178–8199 at each start, bound to `127.0.0.1`.
- OpenWhispr's main process holds `127.0.0.1:8200`, which answers `401` to `/`,
  `/health` and `/v1/models`. Not identified.

### `LocalTranscriber`, inside Warp

`.fork/tickets/T02` is this fork's own work and T2.1–T2.5 are built:
`LocalTranscriber` implementing `voice::transcriber::Transcriber`, installed
unconditionally under fork policy (`app/src/lib.rs:2160`), fail-closed so that
a misconfiguration is an error rather than a silent fallback to the server. The
mic button is reachable without an account, because `is_any_ai_enabled` passes
through `fork::account_gate_bypassed()`. The privacy note in T02 is the reason
it exists: `Provider::OpenAI` is *not* a local path, and
`ServerVoiceTranscriber` POSTs base64 audio to `api.warp.dev` regardless of
provider.

It stays as it is. It is harmless while OpenWhispr covers dictation, and it is
the path if OpenWhispr ever goes away.

~~*"`LocalTranscriber`'s `Http`/`Command` contracts already work with any local
engine pointed at them, unmodified"* (2026-09-12).~~ Corrected 2026-09-13: the
only engine it has been run against is whisper.cpp. The OpenAI-shaped servers
(speaches, faster-whisper-server, LocalAI) were read about, not run. Probably
true of any server answering `{"text": ...}`; not shown.

**Not a fork dependency, and not worth chasing as one (2026-09-12).** The
maintainer doesn't own openwhispr and prefers this fork stay self-contained.
`franken_whisper` (a friend's project, stale) was checked for the same reason
and set aside for the same reason. Using OpenWhispr as a desktop app beside
Warp is not a dependency: nothing in Warp knows it exists.

**A note on how this half was nearly mis-filed.** Grepping `voice` in this
repository returns ~23 hits and reads as covered; all of it is transcription.
For a while that led to the opposite error, treating T02 as pointing "the wrong
direction". It does not. The maintainer ranks input high, and speaking to the
fork and being read to by it are one surface.

## Speech out at the desk: the CLI

**Where it lives: `mycosavant/pocket-tts`, as the first Rust shell** of the
core that repo's `android/docs/owning-the-pipeline.md` already argues for
(pipeline in one Rust crate, shells for JNI, Obsidian and CLI). Reasons it is
not a crate in this fork:

- **Warp's default build does not compile ONNX Runtime in.** `ort` (rc.10) is
  optional in `crates/input_classifier`, and the NLD classifiers in the default
  build use candle. Linking it for one feature adds its runtime to every build
  and a second `ort` version to reconcile (the reference adapter uses rc.12).
- **A separate binary serves Termux, a Claude Code `/speak` command and Warp
  alike**, and Warp's builds never wait on it.

~~**Warp's share** is a speaker seam shaped like `LocalTranscriber`: Warp decides
*what* is spoken (the last agent reply, with code blocks and tables stripped)
and hands text to the command the user names. The trigger (keybinding,
`warpctrl` action, panel button) is undecided.~~ As built, Warp hands over the
reply as Markdown and the command strips it, because what should be said
depends on the voice saying it; the trigger is two palette entries with no
default keys. See *As built* below.

**What it is ported from**, read 2026-09-13, not run:

| | |
|---|---|
| engine | speech-kit's `native/src/adapters/pocket_tts.rs`, **840 lines** including tests, **MIT**; `ort` + `sentencepiece-rs` over `tokenizer.model`, 24 kHz output. ~~This row said ~300 lines on 2026-09-13, copied from the position paper; `wc -l` on the file the same day says 840.~~ |
| models | `KevinAHM/pocket-tts-onnx` `english_2026-04` int8 (text conditioner, flow LM main and flow, Mimi decoder), ~125 MB; **CC-BY-4.0** |
| voice | `alba.safetensors` from `kyutai/pocket-tts-without-voice-cloning`; **CC-BY-4.0**. No encoder in this bundle, so no cloning |
| pins | speech-kit's `native/catalog.json` pins each file to a commit and a sha256; reuse them |

Both model licences require attribution in the CLI.

~~**The tokenizer risk the position paper ranks highest mostly does not apply to
this path.** This port loads the real `tokenizer.model` through a library, so
it inherits a tokenizer already in use.~~ Wrong the same day it was written.
`sentencepiece-rs` 0.2.2 (Apache-2.0, no dependencies, one author) describes
itself as *"a Rust runtime reimplementation"* that follows the C++ behaviour
*"but is not a line-by-line rewrite"*, and its tests build nine-piece models by
hand; none compares its output with Google's on a real model. So the risk moved
into that crate rather than going away. The paper's first step stands: **diff
its tokenisation against Google's SentencePiece over a few thousand sentences
before anything downstream.** The only evidence it works on this model is
speech-kit's CI, which gates on a Whisper round-trip word error rate.

**Measured 2026-09-13: identical to Google's.** `sentencepiece-rs` 0.2.2
`encode_to_ids` against Google's `sentencepiece` 0.2.2 `encode` (the call
`conditioners/text.py` makes: `encode(text, out_type=int)`, no BOS), on the
pinned `english_2026-04` `tokenizer.model`:

| corpus | rows | mismatches |
|---|---|---|
| sentences from `.fork/docs`, `.fork/runs`, `.fork/tickets` and pocket-tts's docs (258 files), plus 22 hand-written edge cases: URLs, Windows paths, hashes, code signatures, emoji, fullwidth, zero-width and non-breaking spaces | 13,529 raw + 1,283 as the adapter's `prepare_text` form | **0** |
| random strings, 1–80 characters, from eleven pools: Latin, digits, whitespace, punctuation, accented, Greek, Cyrillic, CJK, emoji with modifiers and ZWJ, fullwidth and ligatures, combining marks | 5,000 (seed 20260913) | **0** |

The bundle's `tokenizer.model` is **byte-identical** to the one the reference
implementation loads (`kyutai/pocket-tts-without-voice-cloning@d4fdd22`, both
sha256 `d461765a…`), so this is the tokenizer the model was conditioned on.
**Not established:** decoding (the CLI never decodes), sampling or
`nbest` modes, other languages' tokenizers, and texts longer than 400
characters. The harness was session scratch; it becomes the CLI's first test.

**Build-time download.** `ort` rc.12's `download-binaries` fetches ONNX Runtime
1.24.2 from `cdn.pyke.io` and checks it against a sha256 pinned in
`ort-sys/build/download/dist.txt`. That is pinned but third-party-hosted; a
Microsoft release pointed at by `ORT_LIB_LOCATION` is the alternative.

~~**Order:** build the CLI on Windows … before any Warp code; then the Warp
seam; then the phone.~~ Followed, and done the same day except the phone.

### As built, 2026-09-13

**`pocket-speak`** (`rust/` on `desk-cli`, commits `005cce7`..): reads
Markdown on stdin, strips what should not be spoken, chunks, and plays
through the default output device or writes a WAV. `pocket-speak install`
fetches the pinned bundle and voices and keeps a file only if its sha256
matches. The engine is the speech-kit port with three changes: voice state
loaded once, latents decoded in batches of twelve as they are generated (same
boundaries, same samples, earlier audio), and each chunk reporting whether it
ended on the EOS head or the frame budget.

**Measured**, faster-whisper base.en as judge:

| | result |
|---|---|
| cutoff cases, `sentence` and `packed`, seeds 1-3 | 54 of 54 kept the final word, none doubled it |
| same cases, `packed-raw` (tail join off), seeds 1-5 | 44 of 45; the loss dropped a whole countdown after one prose sentence |
| speed, 2 threads | 3.2-4.2x real time on WSL, 3.6-3.9x on Windows; first audio 280-450 ms |
| 168-word agent reply, `sentence` vs `packed` | 340 vs 344 ms first audio, 3.46x vs 3.49x |
| Windows | 30 s MSVC build; 22 MB exe runs relocated alone, plays through WASAPI, WAV transcribed word for word |

So **the early-EOS failure is the model's**, not sherpa-onnx's: it appears on
direct ONNX Runtime too, and the trailing-short-sentence join still earns its
place. `packed` is the default because it costs nothing measurable against
`sentence` on real replies. ~~**Not established:** how it sounds against the
Android build or speech-kit, which needs ears~~ **Heard by the maintainer,
2026-09-13, on the Windows smoke tests:** *"sounds at least as good as the
speech-kit impl (as good or better than the android impl). all runts
articulated perfectly."* Still not established: other languages; aarch64.

**Warp's share** is two palette entries, "Read last agent reply aloud" and
"Stop reading aloud" (no default keys), gated on `fork::read_aloud_enabled`.
The reply is every non-empty output from the most recent exchange with a user
query to the end, the same span the AI block's "Copy output" takes, and it is
written as Markdown to the stdin of `agents.voice.read_aloud.command` with
`agents.voice.read_aloud.args`. Starting a reading stops the one in progress;
stopping kills the process. Warp holds no engine and no voice.

**Run end to end on Windows, 2026-09-13** (debug build of `68cc117c2`,
`WARP_DATA_PROFILE=readaloud`, `claude-agent-acp` 0.73.0 started Windows-side
because the scratch pane was PowerShell, `pocket-speak --stats` as the reader).
The palette entry dispatched; Warp logged each of pocket-speak's stats lines
through the stderr reader, four chunks, all ending on the EOS head, the last
the agent's own *"…Patching, Testing, Done."* with the tail join applied.

**And it read Warp's notes aloud first.** The reply text came from
`format_output_for_copy`, which writes `WarpNote` and `ToolRow` messages into
the text: the ACP mode disclosure took three of the four chunks, about forty
seconds, before the agent's two sentences. Fixed in `ac5f6b25f`: only `Text`
messages, and within them only plain-text sections, are spoken.
`warp_notes_are_not_read_aloud` was calibrated by letting `WarpNote` through
(fails) and restoring (8/8). Speed in that run was 0.94-2.14x real time against
3.6-3.9x standalone, with the emulator and a debug Warp on the same machine;
contention is the likely cause and was not isolated.

**Heard fixed on the rebuilt binary** (`v0.fork.ac5f6b25f`, same profile and
agent, emulator off). The stored reply was 657 characters and still began with
the mode notice, so the test had something to leave out; pocket-speak was
handed one 41-character chunk, *"The fix is in the build. One, Two, Three."*,
ended on the EOS head, first audio 258 ms, 4.15x real time. The 4.15x against
the earlier run's ~1x is consistent with the contention explanation above.

```toml
[agents.voice.read_aloud]
command = 'C:\path\to\pocket-speak.exe'
args = "--voice alba"
```

### Why not speech-kit's own sidecar

It is running on this machine (`local-dictation-sidecar.exe`, Speech Kit
`2026.8.7`, spawned by Obsidian), and it is the wrong door:

- It speaks a framed stdin/stdout protocol (JSON `StartSynthesis { voice_id,
  chunks }` in, binary PCM16 frames out). Its ADR 0001 chose that framing to
  keep the plugin–sidecar boundary internal.
- The maintainer pins its version because a fast-moving sidecar is a security
  exposure. Warp speaking its protocol would track a private format of a
  project the maintainer forks but does not lead.

## Speech out on the phone: what exists

`org.pockettts.android`, an Android app using Kyutai Pocket TTS voices through
sherpa-onnx, registered as a **system TTS engine**, so Select-to-Speak, Chrome
read-aloud and ebook readers all speak in a natural voice with the calling app
oblivious that it is installed. Alongside that it has its own reader
(`ReadAloudActivity` → `Reader.speak()` → `PlaybackService`) with real
transport controls and a scratchpad.

It already accepts `ACTION_PROCESS_TEXT` and `ACTION_SEND`, so the whole
transport UI sits behind an intent, and since PR #3 behind a broadcast as well.

**Tested on the device 2026-09-11/12** (SM-S938U1, Android 16, build 46):
transcript → script → broadcast → audio, with lock-screen controls and the
screen off. Tests 1–5 pass. Every first-contact failure was in the shell script
or in proot; none in the app. Two PRs open at the time of writing: **#7** the
script fixes, **#8** the app fixes, green in CI and not yet run on a phone.

**Why it is a `/command` and not a Stop hook**, which is the part that
generalises: Android 10+ bans *background* activity starts. A Stop hook fires
with nothing in the foreground and `am start` is refused; a user-typed command
fires while the person is looking at the terminal, so it is permitted. The
platform chose the trigger. An exported broadcast receiver is the way back to
screen-off operation, precisely because a receiver may never start an activity
and does not need to.

## The phone bridge

~~This section was titled *"The fork's own share, and it is small"* until
2026-09-13.~~ The maintainer said the Termux wiring was a stop-gap and the desk
is primary, so the fork's share is now the desk seam above. The account below
stands for whenever the phone comes back.

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
running Warp**, the same property that made mosh+tmux worth having, since the
instance is usually what is being rebuilt. So the candidate is one line from a
phone-local Termux shell rather than the mosh pane: ssh to the desk, take the
trace, broadcast locally. In `speak-last.sh`'s own structure that is a second
**source** for the text, not a second script.

**Built 2026-09-13, as a source in `speak-last.sh`, not in Warp** (pocket-tts
PR #13): `CLAUDE_TTS_REMOTE=<ssh host>` copies the newest transcript under the
desk's `~/.claude/projects` over the phone's existing ssh access, and
`CLAUDE_TTS_TRANSCRIPT=<file>` names one outright. It reads the agent's own
session file rather than `warpctrl agent trace`, because `agent list` and
`agent read` carry no timestamps to pick "the latest" by, and the session file
needs no running Warp either. **Measured on the `warp_phone` emulator**
(x86_64, Termux 0.118.3 and Termux:API 0.53.0, both sha256-checked): `engine`
mode spoke a staged reply through Google's TTS, the only engine installed,
bound and dispatched twice (heading, then paragraph), 6 s, exit 0, which is the
agnostic path: whatever engine the device has. **Not measured:** the ssh hop
from a real phone, and the Pocket TTS app on the emulator, whose APK is
arm64/armv7 only. ~~**Unbuilt on this side. Do not build it before #7 and #8
are confirmed on a device**~~ (both were, on build 52): they are green in CI and have never touched a phone, and wiring a new
text source into two unverified layers means debugging all three at once.

## Four constraints any phone bridge inherits

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
and the tail is lost: `One. Two. Three. Four. Five.` stopped after *"Two."*

**Position is the variable, not length.** Mid-text runts are harmless, and a
single trailing runt is harmless; a run of them at the end is not. PR #8 joins
such a run with commas in the spoken form only, leaving what is shown untouched.

The reason it matters here is #8's own: **that is how agent replies end**, in
countdowns and strings of statuses. Warp's replies have the same shape, so any
fork surface that speaks agent output meets this.

Documented and unfixed upstream: the same head fires early inside an ordinary
sentence holding a run of numbers. `Tests 1, 2 and 3 are green.` lost "green" in
2 of 3 runs; `Files A, B and C are updated.` never did. The threshold is the
engine's and the reference implementation uses the same value, so nothing at the
app layer can tell a premature ending from a real one.

**speech-kit does it differently, and the maintainer hears less of the cutoff
there** (2026-09-13, by ear, not measured). Its adapter stops on an EOS logit
above `-4.0` and then generates a tail: **5 more frames when the text has 4
words or fewer, 3 otherwise**, capped by a frame budget of `tokens / 3 + 2`
seconds (`pocket_tts.rs:223`, `:283`, `:321`). The Android build goes through
sherpa-onnx's own stopping rule. ~~The longer tail on short text is a plausible
cause of the difference.~~ Superseded the same day by a larger difference
upstream of the engine: **speech-kit generates one sentence at a time.**
`segmentSpeakableText` flushes a chunk once it reaches `minimumCharacters`,
which defaults to 1, and `extractAndSegmentMarkdown` passes only a locale, so
every sentence is its own generation. A trailing "Two." is generated alone,
with the 5-frame tail. ~~…and the leading-space padding the bundle asks for
short inputs, which sherpa strips.~~ `english_2026-04`'s `bundle.json` sets
`pad_with_spaces_for_short_inputs: false` (and `remove_semicolons: false`,
`model_recommended_frames_after_eos: null`), so no padding happens for this
model; read off the pinned file the same day. The Android app chunks to ~200 characters and
measured the per-sentence approach as costing *"a full voice conditioning pass
per word"* (`TextChunker.kt`). Both are **read, not measured**. For the CLI
this is a choice between latency and the cutoff, and the *Eleven Utterances*
set is where to make it.

## Three findings worth keeping for their shape

None is about voice. Each is a fresh instance of a rule `CLAUDE.md` already
carries, found in a different language on a different machine, which is the
argument for writing them down.

**The timer measured the consumer, not the producer.** The app reported
*"generation speed: 0.83x real time — slower than playback"* and that number was
wrong about what it named. The reader fed the speaker from inside the engine's
callback, and the write blocks when the buffer is full, so chunk N+1 could not
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
**semaphore on chunks, not a bounded channel**, and the reason is worth
keeping: *the engine's callback cannot suspend*. A callback blocked waiting for
room would hold the engine exactly as the blocking write did, with nothing to
wake it after a stop. Same intent, opposite outcome, and only one of the two
survives cancellation.

**The pinned dependency was re-splitting text the caller had already split.**
Voice drift over long text was attributed for a month to independent
generations from one voice prompt, with audio-prompt chaining proposed as the
fix. The real cause was one behaviour in sherpa-onnx's C++: it cuts a chunk back
into sentences on `.!?` and generates each independently. The app chunked; the
engine re-split underneath it. Fixed from the *caller's* side in four lines,
two sentence-length bounds in sherpa's `extra` map, set above the app's own
chunk cap. On the device: *"much better, first syllable collapse is almost
none."*

That one has a second edge, recorded in the position paper: *a defect in code
you own is a bug; a defect in code you call is a mystery.* It took a month to
find because the behaviour had no setting, no log and no symptom except a voice
that sounded wrong in a way nobody could name.

## Why this page exists at all

On 2026-09-12 a session searched *this* repository for `text-to-speech`, found
zero hits, and filed read-aloud as an unexplored requirement. It is none of
those things: there is a built Android app, a device test with eight cases, and
two open pull requests. **The error was concluding from one tree's silence about
work that lives in another tree and in cloud artifacts**, the same shape as
reading a thin friction log and inferring an idle week, made twice in one
session. This page is the index that stops the third time.

The same shape recurred on 2026-09-13 in the other direction: a summary called
dictation "done" from the code, while the only engine ever run was a stub, and
the dictation actually in use lived in a desktop app nobody had asked about.

## Sources

| | |
|---|---|
| `mycosavant/pocket-tts` | the Android app, `android/docs/` (`direct-ort.md`, `owning-the-pipeline.md`), PRs #1–#8; home of the desk CLI |
| `mycosavant/speech-kit-obsidian-plugin` | `native/src/adapters/pocket_tts.rs`, `native/src/protocol.rs`, `native/catalog.json`, `docs/adr/0001-*` |
| `mycosavant/openwhispr` | `src/helpers/whisperServer.js`, `resources/windows-fast-paste.c`; behind the installed upstream release |
| artifact *Read Aloud Wiring* | the implementation handoff, 2026-09-11, with its ruled-out list |
| artifact *Eleven Utterances* | the device test, 2026-09-11/12, eight cases, revised after the follow-up |
| artifact *Pipeline, Not Inference* | the position paper; its text is `owning-the-pipeline.md` above |
| `.fork/tickets/T02-local-voice.md` | the fork's transcription work |
| `.fork/runs/egress-windows-2026-09-05/` | voice egress, measured against `whisper-stub.py` |
| `.fork/runs/run-live-2026-09/friction.md` | friction #2, and the two corrections that produced this page |
