> Ticket T2, split out of `.fork/TASKS.md` on 2026-09-05. History; read the section you came for.

## T2 — Local voice transcription (replace Wispr Flow)

Cleanest seam in the codebase: `Transcriber` is a one-method trait and
`VoiceTranscriber::new` is the injection point. Upstream docstring states it
is built this way "to avoid the editor having a direct dependency on any
server api."

Privacy note: this is a fix, not a preference. `Provider::OpenAI` is **not** a
local path — `ServerVoiceTranscriber` POSTs base64 audio to `api.warp.dev`
regardless of provider. Voice currently leaves the machine either way.

- [x] **T2.1** `LocalTranscriber` implementing `voice::transcriber::Transcriber`
      — `app/src/voice/local_transcriber.rs`. Fail-closed: when it is installed
      it is the *only* transcriber, and a misconfiguration is an error rather
      than a fallback to the server. A fallback would paper over exactly the
      failure that matters — the audio went somewhere else.
- [x] **T2.2** Two backends, settings under `agents.voice.local_transcription`.

      `http` posts `multipart/form-data`. whisper.cpp's `whisper-server` and
      the OpenAI-compatible servers (speaches, faster-whisper-server, LocalAI)
      agree on the request shape and on `{"text": ...}` for the reply, and
      differ only in route — which is why the endpoint setting is a whole URL.
      Measured against a live server rather than assumed:

          POST /inference                 -> {"text":" List the files in this directory.\n"}
          POST /v1/audio/transcriptions   -> 404 File Not Found
          extra `model` / `language` form fields  -> tolerated
          empty body                      -> 400 "Invalid request" (plain text)

      So the default endpoint is whisper.cpp's `/inference`, the response
      parser trims the leading space and trailing newline, and a non-2xx body
      is surfaced verbatim (tail-truncated).

      `command` writes the recording to a 0600 temp file and runs a binary,
      reading the transcript from stdout. Arguments are split *before*
      placeholder substitution so a model path under `C:\Program Files` stays
      one argument.
- [x] **T2.3** Registration swapped at `lib.rs` via
      `fork::local_voice_transcription_enabled`. Settings are mirrored into a
      snapshot because `transcribe` runs off the main thread with no context; a
      test drives the real subscription and asserts an edited endpoint arrives
      without a restart. Settings groups emit a `ChangedEvent` rather than
      notifying observers — `ctx.observe` here would have silently never fired.
- [x] **T2.4** `WISPR_FLOW_URL` retained for the non-fork branch; under fork
      policy the description says audio is transcribed on this machine and
      links to whisper.cpp. The old text ("powered by Wispr Flow") would now be
      false, so the whole sentence changes, not just the URL.
- [~] **T2.5** No audio egress — *argued and unit-tested, not proxy-verified.*
      Under fork policy `ServerVoiceTranscriber` is never constructed
      (`fork_policy_installs_a_local_transcriber`), and `LocalTranscriber`
      contacts only the configured endpoint or spawns the configured binary.
      The default endpoint is asserted to be loopback. What is **not** done is
      a proxy capture during a real recording.

      **Still blocked, and now for a precise reason rather than a vague one.**
      The general egress question was closed with a rig that would answer this
      one too (see "Nothing escapes: measured, not argued"), so the only thing
      missing is a way to *start a recording*. There is none from outside the
      GUI: voice is behind the `voice_input` cargo feature, has no
      local-control action, and is triggered by a keybinding — and keystroke
      synthesis does not work under WSLg (T5.4: clicks land, keys do not). A
      microphone does exist here, WSLg forwards one as PulseAudio `RDPSource`,
      so the hardware is not the obstacle.

      **Update 2026-08-21: the keystroke half of that blocker is narrower than
      it looked.** T5.4's claim that `XSetInputFocus` does not stick is wrong —
      it does. See "The WSLg input wall is narrower than T5.4 recorded" under
      T8. Keys still do not arrive, but the remaining gap is activation rather
      than X focus, and `crates/computer_use` ships a real input stack that
      nobody here has tried against it. If that comes loose, this task
      unblocks itself with no person at the keyboard.

      **Recipe for whoever is at the keyboard**, since the rig is now written
      down and this is ten minutes of work:

          mitmdump --listen-host 127.0.0.1 --listen-port 8899 \
                   --set confdir=<dir> --flow-detail 2
          # then launch with HTTPS_PROXY/HTTP_PROXY at 8899 and
          # SSL_CERT_FILE=<dir>/mitmproxy-ca-cert.pem, hold the voice key,
          # say anything, and read the flow list.

      Expected: the configured loopback endpoint and nothing else. The words do
      not matter — this is a question about destinations, so silence into the
      mic would answer it just as well as speech.

**Verified**

    linux    31 unit tests; live HTTP against whisper-server -> "List the files in this directory."
    windows  29 unit tests (2 unix-only skipped); same live transcription
    windows  app builds, launches, runs with the settings.toml block in place

**Bug caught by clippy, would have shipped:** `std::process::Command` flashes a
console window on Windows on every invocation. `command::blocking::Command`
fixes that but defaults to `CREATE_BREAKAWAY_FROM_JOB`, which `CreateProcess`
refuses inside a job that disallows it — `Access is denied` when running any
binary. Clearing the flags keeps `CREATE_NO_WINDOW` and drops breakaway, which
is right anyway: breakaway exists so shells outlive Warp, and a transcriber
Warp is synchronously waiting on should not.

**Not done, deliberately:** the new settings are not reachable from `warpctrl`.
`setting.get`/`set` operate on `ALLOWLISTED_SETTING_KEYS`, a curated list of
ten appearance and input keys each with hand-written accessors. Adding voice
keys is plumbing, and belongs with T1, not here.

**Open, unexplained:** the dev build never creates
`%LOCALAPPDATA%\warp\WarpOss\data\logs\warp-oss.log` — it writes to
`warp-oss.log.recovery` instead, and the `.old.N` slots are frozen at 169
bytes. `setup_log_files_for_current_execution` routes to the `.recovery` path
when `is_from_crash_recovery_process`, and Warp launches a recovery child that
takes over when the parent dies (`crash_recovery::wait_for_parent_crash`,
"Parent has crashed; continuing execution"), so repeatedly `Kill()`ing the
process during testing leaves the recovery child in charge. But a launch with
zero prior `warp-oss` processes reproduced it, so that is not the whole story.

Further evidence 2026-08-18: two `warp-oss` launches an hour apart, one wrote
`warp-oss.log` normally and the next wrote `.recovery`, with no deletion in
between. So it is per-run state, not a one-way door — consistent with the
recovery-child theory and inconsistent with "the file is broken".

I deleted `warp-oss.log` during this testing before understanding any of the
above, so I cannot fully separate cause from coincidence — but recreating the
file did not fix it and the recovery-path evidence points elsewhere. The
installed stable Warp logs normally and was not touched. Worth a look before
relying on the OSS build's file logs for anything.

